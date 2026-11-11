//! The server check run when a server is added: every part of the API the app
//! uses, one step at a time, then how much metadata the library carries.
//!
//! Things that stop the app from working (no library, streams that don't
//! play) fail the check. Things that only make it poorer (missing artwork,
//! no sign-in) are warnings the user can accept.
use std::sync::mpsc::{self, Receiver, Sender};

use super::{
    agent, normalize, Client, Episode, InfoJson, Item, Library, Missing, Mode, ServerInfo, Session,
    Unauthorized,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Pass,
    /// Checked after signing in.
    Later,
    Warn,
    Fail,
}

pub enum Event {
    /// A step started.
    Begin(&'static str),
    /// The running step finished.
    End(Level, String),
    Metadata(Vec<Group>),
    /// Nothing left to check. Holds the client unless a step failed.
    Done(Option<Client>),
}

/// How many titles have one piece of metadata.
#[derive(Clone)]
pub struct Field {
    pub name: &'static str,
    pub have: usize,
    pub total: usize,
    /// Missing everywhere fails the check instead of warning.
    pub required: bool,
}

impl Field {
    pub fn level(&self) -> Level {
        if self.have == self.total {
            Level::Pass
        } else if self.required && self.have == 0 {
            Level::Fail
        } else {
            Level::Warn
        }
    }
}

#[derive(Clone)]
pub struct Group {
    pub title: &'static str,
    /// "titles", "episodes", …
    pub noun: &'static str,
    pub total: usize,
    pub fields: Vec<Field>,
}

pub enum Job {
    /// Start from scratch with the address the user typed.
    Address(String),
    /// Finish the checks that needed a sign-in.
    SignedIn(Client),
}

/// Run `job` on a background thread. `wake` is called after each event, so
/// the UI knows there's news (the Iced UI also polls on its own ticks).
pub fn start(wake: impl Fn() + Send + 'static, job: Job) -> Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    let mut run = Run {
        tx,
        wake: Box::new(wake),
        failed: false,
    };
    std::thread::spawn(move || {
        let client = match job {
            Job::Address(a) => run.connect(&a),
            Job::SignedIn(c) => Some(c),
        };
        if let Some(c) = client
            .as_ref()
            .filter(|c| !c.info.auth || c.token.is_some())
        {
            run.library(c);
        }
        let ok = !run.failed;
        run.send(Event::Done(client.filter(|_| ok)));
    });
    rx
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn has(s: &Option<String>) -> bool {
    s.as_deref().is_some_and(|s| !s.trim().is_empty())
}

struct Run {
    tx: Sender<Event>,
    wake: Box<dyn Fn() + Send>,
    failed: bool,
}

impl Run {
    fn send(&self, e: Event) {
        let _ = self.tx.send(e);
        (self.wake)();
    }

    fn step<T>(&mut self, title: &'static str, f: impl FnOnce() -> (Level, String, T)) -> T {
        self.send(Event::Begin(title));
        let (level, detail, out) = f();
        self.failed |= level == Level::Fail;
        self.send(Event::End(level, detail));
        out
    }

    /// The checks that work without signing in.
    fn connect(&mut self, address: &str) -> Option<Client> {
        use Level::*;
        let base = normalize(address);
        if base.len() <= "http://".len() {
            let msg = "Enter the server's address, e.g. 192.168.1.20:8080".to_owned();
            return self.step("Server address", || (Fail, msg, None));
        }
        let agent = agent();
        let host = base.split("://").nth(1).unwrap_or(&base).to_owned();

        // Some(true): the root is a page with links, maybe a folder listing.
        let listing = self.step("Reach the server", || {
            match agent.get(&format!("{base}/")).call() {
                Ok(r) => {
                    let body = r.into_string().unwrap_or_default();
                    (
                        Pass,
                        format!("{base} answered."),
                        Some(body.contains("href=")),
                    )
                }
                Err(crate::net::Error::Status(code, _)) => {
                    (Pass, format!("{base} answered ({code})."), Some(false))
                }
                Err(e) => (
                    Fail,
                    format!(
                        "No answer from {base}: {e}. Check the address and port, that the \
                         server is running, and that no firewall blocks it."
                    ),
                    None,
                ),
            }
        })?;

        let info = self.step("Valo API · GET /api/info", || {
            let why = match agent
                .get(&format!("{base}/api/info"))
                .set("Accept", "application/json")
                .call()
            {
                Ok(r) => match r.into_json::<InfoJson>() {
                    Ok(i) => return (Pass, "Found.".to_owned(), Some(i)),
                    Err(_) => "It answered, but not with JSON".to_owned(),
                },
                Err(crate::net::Error::Status(code, r)) => {
                    format!("It answered {code} {}", r.status_text())
                }
                Err(e) => return (Fail, format!("{e}"), None),
            };
            if listing {
                let msg = format!(
                    "{why}. The server shares a plain folder listing, so Valo will read \
                     that instead: titles are guessed from file names, and descriptions, \
                     genres, ratings and cast won't be available."
                );
                (Warn, msg, None)
            } else {
                let msg = format!(
                    "{why}, and the server doesn't share a folder listing either. Add the \
                     Valo API (see the API guide) or turn on directory listings."
                );
                (Fail, msg, None)
            }
        });
        if self.failed {
            return None;
        }

        let Some(info) = info else {
            self.step("Sign-in", || {
                let msg = "Folder listings can't ask for a sign-in, so anyone on your network \
                           who knows the address can watch."
                    .to_owned();
                (Warn, msg, ())
            });
            return Some(Client::new(
                base,
                ServerInfo {
                    name: host,
                    auth: false,
                    mode: Mode::Directory,
                },
            ));
        };

        let name = self.step("Server name", || {
            match info.name.filter(|n| !n.trim().is_empty()) {
                Some(n) => (Pass, format!("“{n}”"), n),
                None => {
                    let msg =
                        format!("/api/info has no \"name\", so the server is shown as {host}.");
                    (Warn, msg, host.clone())
                }
            }
        });

        self.step("Sign-in required", || {
            if info.auth {
                let msg = "The server asks for a username and password.".to_owned();
                (Pass, msg, ())
            } else {
                let msg = "/api/info says \"auth\": false, so anyone on your network can browse \
                           and play the library. Return \"auth\": true and add /api/login to \
                           make it private."
                    .to_owned();
                (Warn, msg, ())
            }
        });

        let client = Client::new(
            base.clone(),
            ServerInfo {
                name,
                auth: info.auth,
                mode: Mode::Api,
            },
        );
        if !info.auth {
            return Some(client);
        }

        self.step("Sign-in API · POST /api/login", || {
            let r = agent
                .post(&format!("{base}/api/login"))
                .set("Accept", "application/json")
                .send_json(serde_json::json!({ "username": "", "password": "" }));
            match r {
                // 422: a validation error for the empty fields (Laravel)
                Err(crate::net::Error::Status(400 | 401 | 403 | 422, _)) => {
                    let msg = "Found; it turns down an empty username and password.".to_owned();
                    (Pass, msg, ())
                }
                Err(crate::net::Error::Status(404 | 405 | 501, _)) => {
                    (Fail, Missing::Login.to_string(), ())
                }
                Err(crate::net::Error::Status(code, r)) => {
                    let msg = format!("It answered {code} {}.", r.status_text());
                    (Fail, msg, ())
                }
                Err(e) => (Fail, format!("{e}"), ()),
                Ok(r) => match r.into_json::<Session>() {
                    Ok(_) => {
                        let msg = "It signed in with an empty username and password. Return \
                                   401 for wrong credentials."
                            .to_owned();
                        (Warn, msg, ())
                    }
                    Err(_) => {
                        let msg = "It answered, but not with JSON. It should return \
                                   {\"token\": \"…\"}, or 401 for a wrong password."
                            .to_owned();
                        (Fail, msg, ())
                    }
                },
            }
        });

        self.step("Sign-out API · POST /api/logout", || {
            let r = agent
                .post(&format!("{base}/api/logout"))
                .set("Accept", "application/json")
                .call();
            match r {
                Ok(_) | Err(crate::net::Error::Status(400 | 401 | 403 | 419 | 422, _)) => {
                    (Pass, "Found.".to_owned(), ())
                }
                Err(crate::net::Error::Status(404 | 405 | 501, _)) => {
                    let msg = "Missing, so signing out only forgets the token on this device \
                               and it keeps working on the server. Add POST /api/logout to \
                               invalidate it."
                        .to_owned();
                    (Warn, msg, ())
                }
                Err(crate::net::Error::Status(code, r)) => {
                    let msg = format!("It answered {code} {}.", r.status_text());
                    (Warn, msg, ())
                }
                Err(e) => (Warn, format!("{e}"), ()),
            }
        });

        self.step("Progress API · GET /api/progress", || {
            match agent
                .get(&format!("{base}/api/progress"))
                .set("Accept", "application/json")
                .call()
            {
                Err(crate::net::Error::Status(401 | 403, _)) => (
                    Later,
                    "Protected. Checked after you sign in.".to_owned(),
                    (),
                ),
                // some frameworks hide protected routes, so look again signed in
                Err(_) => (Later, "Checked after you sign in.".to_owned(), ()),
                Ok(_) => {
                    let msg = "Anyone can read watch progress without signing in. Return 401 \
                               without a valid token."
                        .to_owned();
                    (Warn, msg, ())
                }
            }
        });

        self.step("Library API · GET /api/library", || {
            match agent
                .get(&format!("{base}/api/library"))
                .set("Accept", "application/json")
                .call()
            {
                Err(crate::net::Error::Status(401 | 403, _)) => (
                    Later,
                    "Protected. Checked after you sign in.".to_owned(),
                    (),
                ),
                Err(crate::net::Error::Status(404 | 405 | 501, _)) => {
                    (Fail, Missing::Library.to_string(), ())
                }
                Err(crate::net::Error::Status(code, r)) => {
                    let msg = format!("It answered {code} {}.", r.status_text());
                    (Fail, msg, ())
                }
                Err(e) => (Fail, format!("{e}"), ()),
                Ok(_) => {
                    let msg = "Anyone can read the library without signing in, even though the \
                               server asks for a sign-in. Return 401 without a valid token."
                        .to_owned();
                    (Warn, msg, ())
                }
            }
        });
        (!self.failed).then_some(client)
    }

    /// Everything that needs the library: loading it, playing, artwork, metadata.
    fn library(&mut self, client: &Client) {
        use Level::*;
        let title = match client.info.mode {
            Mode::Api => "Library · GET /api/library",
            Mode::Directory => "Read the folder listing",
        };
        let lib = self.step(title, || match client.raw_library() {
            Ok(lib) => {
                let eps: usize = lib.series.iter().map(episode_count).sum();
                let msg = format!(
                    "{}, {} and {}.",
                    plural(lib.movies.len(), "movie", "movies"),
                    plural(lib.series.len(), "series", "series"),
                    plural(eps, "episode", "episodes"),
                );
                (Pass, msg, Some(lib))
            }
            Err(e) => {
                let msg = if e.is::<Unauthorized>() {
                    "The server turned down the sign-in token (401).".to_owned()
                } else if e.is::<Missing>() && client.info.mode == Mode::Directory {
                    "The folder listing has no video files in it.".to_owned()
                } else {
                    format!("{e:#}")
                };
                (Fail, msg, None)
            }
        });
        let Some(mut lib) = lib else { return };

        self.step("Movies", || {
            let n = lib.movies.len();
            let bad = lib.movies.iter().filter(|m| !has(&m.stream)).count();
            if n == 0 {
                (Pass, "The library has no movies.".to_owned(), ())
            } else if bad == n {
                let msg = format!(
                    "None of the {} has a \"stream\" URL, so none can be played.",
                    plural(n, "movie", "movies")
                );
                (Fail, msg, ())
            } else if bad > 0 {
                let msg = format!("{bad} of {n} movies have no \"stream\" URL and will be hidden.");
                (Warn, msg, ())
            } else {
                (
                    Pass,
                    format!("{}, all playable.", plural(n, "movie", "movies")),
                    (),
                )
            }
        });

        self.step("Series and episodes", || {
            let n = lib.series.len();
            let empty = lib.series.iter().filter(|s| episode_count(s) == 0).count();
            let eps = || {
                lib.series
                    .iter()
                    .flat_map(|s| &s.seasons)
                    .flat_map(|s| &s.episodes)
            };
            let total = eps().count();
            let bad = eps().filter(|e| e.stream.trim().is_empty()).count();
            if n == 0 {
                (Pass, "The library has no series.".to_owned(), ())
            } else if total == 0 {
                let msg = "No series has any episodes in \"seasons\", so none can be shown.";
                (Fail, msg.to_owned(), ())
            } else if bad == total {
                let msg = "No episode has a \"stream\" URL, so none can be played.";
                (Fail, msg.to_owned(), ())
            } else if bad > 0 || empty > 0 {
                let mut msg = Vec::new();
                if empty > 0 {
                    msg.push(format!(
                        "{empty} of {n} series have no episodes and will be hidden."
                    ));
                }
                if bad > 0 {
                    msg.push(format!(
                        "{bad} of {total} episodes have no \"stream\" URL and won't play."
                    ));
                }
                (Warn, msg.join(" "), ())
            } else {
                let msg = format!(
                    "{} with {}, all playable.",
                    plural(n, "series", "series"),
                    plural(total, "episode", "episodes")
                );
                (Pass, msg, ())
            }
        });

        lib.finish(&client.base);
        if lib.movies.is_empty() && lib.series.is_empty() {
            self.step("Something to watch", || {
                let msg = "Nothing in the library can be played, so it would be empty.";
                (Fail, msg.to_owned(), ())
            });
            return;
        }

        let stream = lib
            .movies
            .iter()
            .find_map(|m| m.stream.clone())
            .or_else(|| {
                lib.series
                    .iter()
                    .flat_map(|s| &s.seasons)
                    .flat_map(|s| &s.episodes)
                    .find(|e| !e.stream.is_empty())
                    .map(|e| e.stream.clone())
            });
        if let Some(url) = stream {
            self.step("Streaming · Range requests", || {
                match client.get(&url).set("Range", "bytes=0-1").call() {
                    Ok(r) => {
                        let ct = r.content_type().to_owned();
                        if ct.starts_with("text/html") || ct.contains("json") {
                            let msg = format!("{url} sent {ct} instead of a video file.");
                            (Fail, msg, ())
                        } else if r.status() == 206 {
                            (Pass, "Videos play, and seeking works.".to_owned(), ())
                        } else {
                            let msg = format!(
                                "{url} ignores Range requests (answered {}), so seeking and \
                                 resuming won't work.",
                                r.status()
                            );
                            (Warn, msg, ())
                        }
                    }
                    Err(crate::net::Error::Status(code, r)) => {
                        let msg = format!("{url} answered {code} {}.", r.status_text());
                        (Fail, msg, ())
                    }
                    Err(e) => (Fail, format!("{url}: {e}"), ()),
                }
            });
        }

        self.step("Artwork", || {
            let still = lib
                .series
                .iter()
                .flat_map(|s| &s.seasons)
                .flat_map(|s| &s.episodes)
                .find_map(|e| e.still.clone());
            let kinds = [
                ("posters", lib.all().find_map(|i| i.poster.clone())),
                ("backdrops", lib.all().find_map(|i| i.backdrop.clone())),
                ("title logos", lib.all().find_map(|i| i.logo.clone())),
                ("episode thumbnails", still),
            ];
            let no_posters = kinds[0].1.is_none();
            let (mut ok, mut bad) = (Vec::new(), Vec::new());
            for (name, url) in kinds {
                let Some(url) = url else { continue };
                let img = client
                    .fetch(&url)
                    .and_then(|b| image::load_from_memory(&b).map_err(Into::into));
                match img {
                    Ok(_) => ok.push(name),
                    Err(e) => bad.push(format!("Couldn't load {url}: {e:#}.")),
                }
            }
            let failed = !bad.is_empty();
            let mut msg = Vec::new();
            if no_posters {
                msg.push("No title has a poster, so tiles show colored placeholders.".to_owned());
            }
            msg.append(&mut bad);
            if !ok.is_empty() {
                let mut s = list(&ok);
                s[..1].make_ascii_uppercase();
                msg.push(format!("{s} load."));
            }
            let level = if no_posters || failed { Warn } else { Pass };
            (level, msg.join(" "), ())
        });

        if client.info.mode == Mode::Api {
            self.progress(client, &lib);
        }

        if client.admin {
            self.step("Admin · GET /api/admin/users", || {
                match client.admin_users() {
                    Ok(u) => (Pass, format!("Found, {} users.", u.len()), ()),
                    Err(e) => {
                        let why = if e.downcast_ref::<crate::api::NoAdminApi>().is_some() {
                            "the server has no admin API".to_owned()
                        } else {
                            format!("{e:#}")
                        };
                        let web = if client.has_admin_page() {
                            format!("only its web page at {}/admin can be opened", client.base)
                        } else {
                            "it can't manage anything".to_owned()
                        };
                        let msg = format!(
                            "You signed in as an admin, but {why}, so the app's Admin page \
                             can't manage users and {web}."
                        );
                        (Warn, msg, ())
                    }
                }
            });
        }

        let groups = coverage(&lib, client.info.mode);
        let fields = || groups.iter().flat_map(|g| &g.fields);
        let level = fields()
            .map(Field::level)
            .max_by_key(|l| *l as u8)
            .unwrap_or(Pass);
        let incomplete = fields().filter(|f| f.level() != Pass).count();
        self.send(Event::Metadata(groups.clone()));
        self.step("Metadata", || {
            let msg = match level {
                Pass => "Every title has every field.".to_owned(),
                Fail => "No title has a title.".to_owned(),
                _ => format!(
                    "{} missing on some or all titles, see below.",
                    plural(incomplete, "field is", "fields are")
                ),
            };
            (level, msg, ())
        });
    }
}

impl Run {
    /// Watch progress sync, signed in: reading it, and that the server takes
    /// changes. The write test sends an entry no server should store, so the
    /// user's real progress is left alone.
    fn progress(&mut self, client: &Client, lib: &Library) {
        use Level::*;
        let url = format!("{}/api/progress", client.base);
        let found = self.step("Progress sync · GET /api/progress", || {
            let r = client.get(&url).set("Accept", "application/json").call();
            let list = match r {
                Err(crate::net::Error::Status(401 | 403, _)) if !client.info.auth => {
                    let msg = "Without a sign-in there's no user to keep watch progress for, \
                               so resume points stay on each device."
                        .to_owned();
                    return (Warn, msg, false);
                }
                Err(crate::net::Error::Status(code @ (401 | 403), _)) => {
                    let msg = format!(
                        "It turned down the sign-in token ({code}), so resume points and \
                         watched marks stay on this device. Accept Authorization: Bearer \
                         <token> on /api/progress like on /api/library."
                    );
                    return (Warn, msg, false);
                }
                Err(crate::net::Error::Status(404 | 405 | 501, _)) => {
                    let msg = "Missing, so resume points and watched marks stay on this \
                               device. Add GET and POST /api/progress to sync them between \
                               devices."
                        .to_owned();
                    return (Warn, msg, false);
                }
                Err(crate::net::Error::Status(code, r)) => {
                    let msg = format!(
                        "It answered {code} {}, so progress won't sync.",
                        r.status_text()
                    );
                    return (Warn, msg, false);
                }
                Err(e) => return (Warn, format!("{e}"), false),
                Ok(r) => r.into_json::<Vec<super::Watched>>(),
            };
            let list = match list {
                Ok(l) => l,
                Err(e) => {
                    let msg = format!(
                        "It answered, but not with a list the app can read ({e}), so progress \
                         won't sync."
                    );
                    return (Warn, msg, true);
                }
            };
            let mut ids: std::collections::HashSet<&str> =
                lib.movies.iter().map(|m| m.id.as_str()).collect();
            ids.extend(
                lib.series
                    .iter()
                    .flat_map(|s| &s.seasons)
                    .flat_map(|s| &s.episodes)
                    .map(|e| e.id.as_str()),
            );
            let unknown = list
                .iter()
                .filter(|w| w.key().is_none_or(|k| !ids.contains(k.as_str())))
                .count();
            let n = plural(list.len(), "entry", "entries");
            if unknown > 0 {
                let msg = format!(
                    "{unknown} of {n} don't match a movie or episode in the library and are \
                     ignored. \"type\" must be \"movie\" or \"episode\" and \"id\" the \
                     library's id for it."
                );
                (Warn, msg, true)
            } else {
                (Pass, format!("{n} for your account."), true)
            }
        });
        if !found {
            return;
        }

        self.step("Progress sync · POST /api/progress", || {
            let mut r = client.agent.post(&url).set("Accept", "application/json");
            if let Some(h) = client.auth_header() {
                r = r.set("Authorization", &h);
            }
            match r.send_json(serde_json::json!({ "type": "valo-check" })) {
                Err(crate::net::Error::Status(400 | 422, _)) => {
                    let msg = "Found; it turns down an invalid entry.".to_owned();
                    (Pass, msg, ())
                }
                Err(crate::net::Error::Status(code @ (404 | 405 | 501), _)) => {
                    let msg = format!(
                        "It answered {code}, so progress made in the app never reaches the \
                         server. Add POST /api/progress, and answer 422 for a body it can't \
                         store."
                    );
                    (Warn, msg, ())
                }
                Err(crate::net::Error::Status(code @ (401 | 403), _)) => {
                    let msg = format!(
                        "It turned down the sign-in token ({code}), so progress made in the \
                         app never reaches the server."
                    );
                    (Warn, msg, ())
                }
                Err(crate::net::Error::Status(code, r)) => {
                    let msg = format!("It answered {code} {}.", r.status_text());
                    (Warn, msg, ())
                }
                Err(e) => (Warn, format!("{e}"), ()),
                Ok(_) => {
                    let msg = "It accepted an entry with \"type\": \"valo-check\" and \
                               no id. Answer 422 for a body it can't store."
                        .to_owned();
                    (Warn, msg, ())
                }
            }
        });
    }
}

/// "a", "a and b", "a, b and c"
fn list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn episode_count(s: &Item) -> usize {
    s.seasons.iter().map(|s| s.episodes.len()).sum()
}

fn field<T>(name: &'static str, items: &[&T], f: impl Fn(&T) -> bool) -> Field {
    Field {
        name,
        have: items.iter().filter(|i| f(i)).count(),
        total: items.len(),
        required: false,
    }
}

/// Which metadata the titles have. Folder listings only get what can be
/// guessed from names and found next to the files.
fn coverage(lib: &Library, mode: Mode) -> Vec<Group> {
    let api = mode == Mode::Api;
    let titles: Vec<&Item> = lib.all().collect();
    let movies: Vec<&Item> = lib.movies.iter().collect();
    let series: Vec<&Item> = lib.series.iter().collect();
    let episodes: Vec<&Episode> = lib
        .series
        .iter()
        .flat_map(|s| &s.seasons)
        .flat_map(|s| &s.episodes)
        .collect();

    let mut all = vec![Field {
        required: true,
        ..field("Title", &titles, |i| !i.title.trim().is_empty())
    }];
    if api {
        all.push(field("Description", &titles, |i| {
            !i.overview.trim().is_empty()
        }));
    }
    all.extend([
        field("Poster", &titles, |i| has(&i.poster)),
        field("Backdrop", &titles, |i| has(&i.backdrop)),
        field("Year", &titles, |i| i.year.is_some()),
    ]);
    let mut groups = vec![Group {
        title: "Movies and series",
        noun: "titles",
        total: titles.len(),
        fields: all,
    }];
    if api {
        groups[0].fields.extend([
            field("Genres", &titles, |i| !i.genres.is_empty()),
            field("Age rating", &titles, |i| has(&i.rating)),
            field("Date added", &titles, |i| i.added.is_some()),
            field("Title logo", &titles, |i| has(&i.logo)),
            field("Cast", &titles, |i| !i.cast.is_empty()),
            field("Release date", &titles, |i| has(&i.released)),
            field("Score", &titles, |i| i.score.is_some()),
        ]);
        groups.push(Group {
            title: "Movies",
            noun: "movies",
            total: movies.len(),
            fields: vec![
                field("Runtime", &movies, |i| i.duration.is_some()),
                field("Director", &movies, |i| has(&i.director)),
                field("Budget", &movies, |i| i.budget.is_some()),
                field("Box office", &movies, |i| i.revenue.is_some()),
                field("File info", &movies, |i| i.file.is_some()),
            ],
        });
        groups.push(Group {
            title: "Series",
            noun: "series",
            total: series.len(),
            fields: vec![
                field("Creators", &series, |i| !i.creators.is_empty()),
                field("Network", &series, |i| has(&i.network)),
                field("Status", &series, |i| has(&i.status)),
            ],
        });
    }
    let mut ep = vec![field("Title", &episodes, |e| !e.title.trim().is_empty())];
    if api {
        ep.extend([
            field("Description", &episodes, |e| !e.overview.trim().is_empty()),
            field("Thumbnail", &episodes, |e| has(&e.still)),
            field("Runtime", &episodes, |e| e.duration.is_some()),
            field("Air date", &episodes, |e| has(&e.aired)),
            field("Score", &episodes, |e| e.score.is_some()),
            field("File info", &episodes, |e| e.file.is_some()),
        ]);
    }
    groups.push(Group {
        title: "Episodes",
        noun: "episodes",
        total: episodes.len(),
        fields: ep,
    });
    groups.retain(|g| g.total > 0);
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists() {
        assert_eq!(list(&["posters"]), "posters");
        assert_eq!(list(&["a", "b", "c"]), "a, b and c");
    }

    #[test]
    fn metadata_levels() {
        let f = |have, required| Field {
            name: "x",
            have,
            total: 4,
            required,
        };
        assert_eq!(f(4, true).level(), Level::Pass);
        assert_eq!(f(2, true).level(), Level::Warn);
        assert_eq!(f(0, true).level(), Level::Fail);
        assert_eq!(f(0, false).level(), Level::Warn);
    }

    #[test]
    fn coverage_counts() {
        let lib: Library = serde_json::from_str(
            r#"{"movies": [
                {"title": "A", "stream": "/a", "poster": "/p", "genres": ["x"]},
                {"title": "B", "stream": "/b", "overview": "…"}
            ], "series": []}"#,
        )
        .unwrap();
        let g = coverage(&lib, Mode::Api);
        assert_eq!(g.len(), 2, "empty series and episode groups are left out");
        let get = |n| g[0].fields.iter().find(|f| f.name == n).unwrap();
        assert_eq!((get("Title").have, get("Title").total), (2, 2));
        assert_eq!(get("Poster").have, 1);
        assert_eq!(get("Description").have, 1);
        assert_eq!(get("Genres").have, 1);
        let dir = coverage(&lib, Mode::Directory);
        assert!(dir[0].fields.iter().all(|f| f.name != "Genres"));
    }
}
