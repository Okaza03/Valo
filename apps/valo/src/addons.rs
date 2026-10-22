//! Stremio addons the server's admin added (`GET /api/addons`): rows of
//! titles for the home page (catalogs), details for titles (metadata) and
//! subtitles, all looked up by IMDb id where it's our title. Streams are
//! never asked for; the videos always come from our server.
use std::collections::HashMap;
use std::time::Duration;

use anyhow::{anyhow, Result};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use crate::api::{
    encode_segment, Addon, AddonInfo, CastMember, Client, Episode, Item, Kind, Season,
    SubtitleTrack,
};

/// Titles kept per catalog (the home page shows fewer).
const ROW_MAX: usize = 100;
/// Search results per catalog.
const SEARCH_MAX: usize = 30;
/// How long a search waits for an addon; the others show meanwhile.
const SEARCH_WAIT: Duration = Duration::from_secs(8);

/// A movie or episode to find subtitles for.
#[derive(Clone)]
pub struct Lookup {
    pub client: Client,
    /// "movie" or "series".
    pub kind: &'static str,
    /// The movie's or series' IMDb id, when the server knows it.
    pub imdb: Option<String>,
    /// Without an IMDb id, it's looked up by title and year.
    pub title: String,
    pub year: Option<u32>,
    /// (season, episode)
    pub episode: Option<(u32, u32)>,
}

impl Lookup {
    pub fn movie(client: &Client, item: &Item) -> Self {
        Self {
            client: client.clone(),
            kind: "movie",
            imdb: item.imdb_id.as_deref().and_then(imdb_id),
            title: item.title.clone(),
            year: item.year,
            episode: None,
        }
    }

    pub fn episode(client: &Client, series: &Item, season: u32, episode: u32) -> Self {
        Self {
            client: client.clone(),
            kind: "series",
            imdb: series.imdb_id.as_deref().and_then(imdb_id),
            title: series.title.clone(),
            year: series.year,
            episode: Some((season, episode)),
        }
    }

    /// Stremio's id: "tt1375666", or "tt0903747:1:2" for an episode.
    fn id(&self, imdb: &str) -> String {
        match self.episode {
            Some((s, e)) => format!("{imdb}:{s}:{e}"),
            None => imdb.to_owned(),
        }
    }
}

/// An IMDb id as "tt0111161", from "tt0111161", "111161" or an IMDb link.
pub fn imdb_id(s: &str) -> Option<String> {
    let s = s.trim();
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
        return Some(format!("tt{s:0>7}"));
    }
    // the first "tt" followed by digits ("https" has a "tt" too)
    let lower = s.to_ascii_lowercase();
    lower.match_indices("tt").find_map(|(at, _)| {
        let digits: String = lower[at + 2..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        (digits.len() >= 5).then(|| format!("tt{digits}"))
    })
}

/// "The Backrooms!" and "backrooms" alike: lowercase letters and digits,
/// without a leading "the" or "a".
pub fn title_key(t: &str) -> String {
    let words: Vec<String> = t
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    let skip =
        matches!(words.first().map(String::as_str), Some("the" | "a" | "an")) && words.len() > 1;
    words[usize::from(skip)..].concat()
}

pub fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Movie => "movie",
        Kind::Series => "series",
    }
}

/// `GET <addon>/<resource>/<type>/<id>.json`
fn call<T: DeserializeOwned>(a: &Addon, resource: &str, kind: &str, id: &str) -> Result<T> {
    get(a, &format!("{resource}/{kind}/{}.json", encode_segment(id)))
}

/// `GET <addon>/<path>`
fn get<T: DeserializeOwned>(a: &Addon, path: &str) -> Result<T> {
    get_within(a, path, Duration::from_secs(15))
}

fn get_within<T: DeserializeOwned>(a: &Addon, path: &str, limit: Duration) -> Result<T> {
    let base = a
        .url
        .trim_end_matches("/manifest.json")
        .trim_end_matches('/');
    Ok(agent()
        .get(&format!("{base}/{path}"))
        .timeout(limit)
        .set("Accept", "application/json")
        .call()
        .map_err(|e| anyhow!("{}: {e}", a.name))?
        .into_json()?)
}

/// One agent for every addon request, so connections are reused instead
/// of a new TLS handshake each time.
fn agent() -> &'static ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .max_idle_connections_per_host(8)
            .user_agent(concat!("Valo/", env!("CARGO_PKG_VERSION")))
            .build()
    })
}

/// An addon's settings page: `https://host/<settings>/configure`.
pub fn configure_url(manifest: &str) -> String {
    let u = manifest.trim();
    let u = match u.strip_prefix("stremio://") {
        Some(rest) => format!("https://{rest}"),
        None => u.to_owned(),
    };
    let base = u.trim_end_matches('/').trim_end_matches("/manifest.json");
    format!("{base}/configure")
}

/// Ask each addon's manifest whether it has a settings page, for
/// servers that don't say.
pub fn mark_configurable(addons: &mut [Addon]) {
    let found = each(addons, |a| {
        let m: Value = get(a, "manifest.json").ok()?;
        let hint = |k: &str| {
            m.pointer(&format!("/behaviorHints/{k}"))
                .and_then(Value::as_bool)
        };
        Some(
            hint("configurable").unwrap_or(false) || hint("configurationRequired").unwrap_or(false),
        )
    });
    for (a, c) in addons.iter_mut().zip(found) {
        a.configurable |= c.unwrap_or(false);
    }
}

/// `f` for each item at once, results in order.
fn each<T: Sync, R: Send + Default>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    std::thread::scope(|s| {
        let jobs: Vec<_> = items.iter().map(|i| s.spawn(|| f(i))).collect();
        jobs.into_iter()
            .map(|j| j.join().unwrap_or_default())
            .collect()
    })
}

// ------------------------------------------------------------- subtitles

#[derive(Deserialize)]
struct SubsAnswer {
    #[serde(default)]
    subtitles: Vec<AddonSub>,
}

#[derive(Deserialize)]
struct AddonSub {
    #[serde(default)]
    url: String,
    /// ISO 639-2 ("eng") as a rule, though some send "en" or a name.
    #[serde(default)]
    lang: String,
}

/// Every addon's subtitles for `l`, in the admin's order. An addon that
/// fails or is slow is left out; the others still count. Err says why the
/// addons couldn't be asked at all.
pub fn subtitles(l: &Lookup) -> std::result::Result<Vec<SubtitleTrack>, String> {
    let addons = l
        .client
        .addons()
        .map_err(|_| "Couldn't reach the subtitle addons".to_owned())?;
    if !addons.iter().any(|a| a.serves("subtitles", l.kind, "tt1")) {
        return Ok(Vec::new());
    }
    let imdb = match &l.imdb {
        Some(i) => i.clone(),
        // the admin can set it under Admin › Library
        None => find_imdb(&addons, l.kind, &l.title, l.year)
            .ok_or_else(|| "No IMDb id, so addons can't find it".to_owned())?,
    };
    let id = l.id(&imdb);
    let wanted: Vec<&Addon> = addons
        .iter()
        .filter(|a| a.serves("subtitles", l.kind, &id))
        .collect();
    Ok(
        each(&wanted, |a| fetch_subs(a, l.kind, &id).unwrap_or_default())
            .into_iter()
            .flatten()
            .collect(),
    )
}

/// The IMDb id of a title the server didn't give one for: searched for in
/// the addons by title, the year a year apart at most.
pub fn find_imdb(addons: &[Addon], kind: &str, title: &str, year: Option<u32>) -> Option<String> {
    let cats: Vec<Searchable> = searchable(addons)
        .into_iter()
        .filter(|(_, k, _)| k == kind)
        .collect();
    let key = title_key(title);
    let found = each(&cats, |(i, k, id)| {
        let a = &addons[*i];
        let path = format!(
            "catalog/{k}/{id}/search={}.json",
            encode_segment(title.trim())
        );
        let ans: CatalogAnswer = get_within(a, &path, SEARCH_WAIT).ok()?;
        ans.metas
            .iter()
            .filter_map(|m| to_item(m, k, &a.name))
            .find(|it| {
                title_key(&it.title) == key
                    && match (year, it.year) {
                        (Some(a), Some(b)) => a.abs_diff(b) <= 1,
                        _ => true,
                    }
            })
            .and_then(|it| it.imdb_id)
    });
    found.into_iter().flatten().next()
}

fn fetch_subs(a: &Addon, kind: &str, id: &str) -> Result<Vec<SubtitleTrack>> {
    let ans: SubsAnswer = call(a, "subtitles", kind, id)?;
    // "English · OpenSubtitles", then "English 2 · OpenSubtitles", …
    let mut seen: Vec<String> = Vec::new();
    Ok(ans
        .subtitles
        .into_iter()
        .filter(|s| s.url.starts_with("http://") || s.url.starts_with("https://"))
        .map(|s| {
            let (code, name) = language(&s.lang);
            let n = seen.iter().filter(|l| **l == name).count();
            seen.push(name.clone());
            let name = if n == 0 {
                name
            } else {
                format!("{name} {}", n + 1)
            };
            SubtitleTrack {
                id: format!("{}:{}", a.id, s.url),
                language: code,
                label: Some(format!("{name} · {}", a.name)),
                url: s.url,
                external: true,
                ..Default::default()
            }
        })
        .collect())
}

// ------------------------------------------------------ catalogs and meta

/// A catalog as a row of titles. Its items are addon titles ([`remote_id`]);
/// the home page swaps in our own where the IMDb id matches.
pub struct Row {
    /// Its [`catalog_key`], for hiding it from the home page.
    pub key: String,
    /// "Netflix · Movies"
    pub title: String,
    /// The catalog's name: "Netflix".
    pub source: String,
    /// From an addon that only lists titles (Streaming Catalogs), so it's a
    /// streaming service's list, not a database's "Popular".
    pub service: bool,
    pub kind: Kind,
    pub items: Vec<Item>,
}

/// How the admin's home page layout names a catalog's row:
/// "catalog:ba3e09ce:movie:top".
pub fn catalog_key(addon: &str, kind: &str, catalog: &str) -> String {
    format!("catalog:{addon}:{kind}:{catalog}")
}

/// The id an addon title goes by in the app: "addon:movie:tt1375666".
pub fn remote_id(kind: &str, id: &str) -> String {
    format!("addon:{kind}:{id}")
}

/// "movie" and "tt1375666" back from a [`remote_id`].
pub fn split_remote(id: &str) -> Option<(&str, &str)> {
    id.strip_prefix("addon:")?.split_once(':')
}

#[derive(Deserialize)]
struct CatalogAnswer {
    #[serde(default)]
    metas: Vec<Value>,
}

#[derive(Deserialize)]
struct MetaAnswer {
    #[serde(default)]
    meta: Option<Value>,
}

/// The addons, and their catalogs as rows.
#[derive(Default)]
pub struct Catalogs {
    pub addons: Vec<Addon>,
    pub rows: Vec<Row>,
    /// Catalogs that answer searches.
    pub search: Vec<Searchable>,
}

/// A catalog that takes `search=…`: (addon index, "movie"/"series", catalog id).
pub type Searchable = (usize, String, String);
/// Details by "kind:id"; None where no addon has the title.
pub type Metas = Vec<(String, Option<Item>)>;

/// The addons, and every catalog of theirs as a row (empty ones left out).
pub fn catalogs(client: &Client) -> Result<Catalogs> {
    let addons = client.addons()?;
    let wanted: Vec<(&Addon, &crate::api::AddonCatalog)> = addons
        .iter()
        .flat_map(|a| a.catalogs.iter().map(move |c| (a, c)))
        .filter(|(_, c)| c.kind == "movie" || c.kind == "series")
        .collect();
    let rows = each(&wanted, |(a, c)| {
        let ans: CatalogAnswer = match call(a, "catalog", &c.kind, &c.id) {
            Ok(r) => r,
            Err(_) => return None,
        };
        let items: Vec<Item> = ans
            .metas
            .iter()
            .filter_map(|m| to_item(m, &c.kind, &a.name))
            .take(ROW_MAX)
            .collect();
        let what = if c.kind == "movie" {
            "Movies"
        } else {
            "TV shows"
        };
        let name = if c.name.trim().is_empty() {
            &a.name
        } else {
            &c.name
        };
        // "Prime  Video" -> "Prime Video"
        let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
        (!items.is_empty()).then(|| Row {
            key: catalog_key(&a.id, &c.kind, &c.id),
            title: format!("{name} · {what}"),
            source: name,
            service: !a.resources.iter().any(|r| r.name == "meta"),
            kind: if c.kind == "movie" {
                Kind::Movie
            } else {
                Kind::Series
            },
            items,
        })
    });
    let search = searchable(&addons);
    Ok(Catalogs {
        rows: rows.into_iter().flatten().collect(),
        search,
        addons,
    })
}

/// One searchable catalog per addon and type, from the addons' manifests
/// (the server only lists the catalogs that make rows).
fn searchable(addons: &[Addon]) -> Vec<Searchable> {
    let found = each(addons, |a| {
        let m: Value = get(a, "manifest.json").ok()?;
        let mut out: Vec<(String, String)> = Vec::new();
        for c in m
            .get("catalogs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let field = |k: &str| c.get(k).and_then(Value::as_str);
            let (Some(kind), Some(id)) = (field("type"), field("id")) else {
                continue;
            };
            if !matches!(kind, "movie" | "series") || out.iter().any(|(k, _)| k == kind) {
                continue;
            }
            let extra: Vec<&Value> = c
                .get("extra")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .collect();
            let name = |e: &Value| {
                e.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            };
            let supported: Vec<String> = c
                .get("extraSupported")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            let required: Vec<String> = extra
                .iter()
                .filter(|e| e.get("isRequired").and_then(Value::as_bool) == Some(true))
                .map(|e| name(e))
                .chain(
                    c.get("extraRequired")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|v| v.as_str().map(str::to_owned)),
                )
                .collect();
            let searches = extra.iter().any(|e| name(e) == "search")
                || supported.iter().any(|s| s == "search");
            if searches && required.iter().all(|r| r == "search") {
                out.push((kind.to_owned(), id.to_owned()));
            }
        }
        Some(out)
    });
    found
        .into_iter()
        .enumerate()
        .flat_map(|(i, cats)| {
            cats.unwrap_or_default()
                .into_iter()
                .map(move |(k, id)| (i, k, id))
        })
        .collect()
}

/// Search every searchable catalog at once. Each catalog's titles are
/// sent as soon as it answers, as (index in `cats`, titles), and `done` is
/// called after each; the channel closes when all have answered.
pub fn search(
    addons: Vec<Addon>,
    cats: Vec<Searchable>,
    q: String,
    done: impl Fn() + Clone + Send + 'static,
) -> std::sync::mpsc::Receiver<(usize, Vec<Item>)> {
    let (tx, rx) = std::sync::mpsc::channel();
    let addons = std::sync::Arc::new(addons);
    for (n, (i, kind, id)) in cats.into_iter().enumerate() {
        let (tx, addons, q, done) = (tx.clone(), addons.clone(), q.clone(), done.clone());
        std::thread::spawn(move || {
            let a = &addons[i];
            let path = format!(
                "catalog/{kind}/{id}/search={}.json",
                encode_segment(q.trim())
            );
            let items = get_within::<CatalogAnswer>(a, &path, SEARCH_WAIT)
                .map(|ans| {
                    ans.metas
                        .iter()
                        .filter_map(|m| to_item(m, &kind, &a.name))
                        .take(SEARCH_MAX)
                        .collect()
                })
                .unwrap_or_default();
            let _ = tx.send((n, items));
            drop(tx);
            done();
        });
    }
    rx
}

/// Search results from several catalogs as one list: they take turns, so
/// the best movie and the best series both come first; no repeats.
pub fn merge(parts: &[Vec<Item>]) -> Vec<Item> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let longest = parts.iter().map(Vec::len).max().unwrap_or(0);
    for n in 0..longest {
        for it in parts.iter().filter_map(|p| p.get(n)) {
            let key = (it.kind, it.imdb_id.clone().unwrap_or_else(|| it.id.clone()));
            if seen.insert(key) {
                out.push(it.clone());
            }
        }
    }
    out
}

/// Full details from the metadata addons that have this title, the first
/// one's winning. Addons that only know TMDB ids ("tmdb:27205", like the TMDB
/// addon) are asked too, once another addon has told us the TMDB id; their
/// cast comes with photos.
pub fn meta(addons: &[Addon], kind: &str, id: &str) -> Option<Item> {
    let ask = |a: &Addon, id: &str| -> Option<Item> {
        let ans: MetaAnswer = call(a, "meta", kind, id).ok()?;
        to_item(ans.meta.as_ref()?, kind, &a.name)
    };
    let mut found: Option<Item> = None;
    for a in addons.iter().filter(|a| a.serves("meta", kind, id)) {
        if let Some(m) = ask(a, id) {
            merge_meta(&mut found, m);
        }
    }
    let tmdb = found.as_ref().and_then(|f| match f.tmdb_id.as_ref()? {
        Value::String(s) => Some(s.clone()),
        v => Some(v.to_string()),
    });
    if let Some(t) = tmdb.filter(|t| !t.is_empty() && !id.starts_with("tmdb:")) {
        let tid = format!("tmdb:{t}");
        for a in addons
            .iter()
            .filter(|a| a.serves("meta", kind, &tid) && !a.serves("meta", kind, id))
        {
            if let Some(m) = ask(a, &tid) {
                merge_meta(&mut found, m);
            }
        }
    }
    found
}

/// Add another addon's details: gaps filled, and a cast with photos (or a
/// longer one) beats a short list of names.
fn merge_meta(found: &mut Option<Item>, m: Item) {
    let Some(f) = found.as_mut() else {
        *found = Some(m);
        return;
    };
    let photos = |c: &[CastMember]| c.iter().filter(|c| c.photo.is_some()).count();
    if (photos(&m.cast), m.cast.len()) > (photos(&f.cast), f.cast.len()) {
        f.cast.clone_from(&m.cast);
    }
    fill(f, &m);
}

/// [`meta`] for many titles at once, by "kind:id" (None: no addon has it).
pub fn metas(addons: &[Addon], wanted: &[(String, String)]) -> Metas {
    let found = each(wanted, |(kind, id)| Some(meta(addons, kind, id)));
    wanted
        .iter()
        .zip(found)
        .map(|((k, id), m)| (format!("{k}:{id}"), m.flatten()))
        .collect()
}

/// A Stremio meta object as one of our items (id: [`remote_id`]).
fn to_item(m: &Value, kind: &str, source: &str) -> Option<Item> {
    let s = |k: &str| {
        m.get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    };
    let list = |k: &str| -> Vec<String> {
        m.get(k)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    // newer addons put genres, cast and directors in "links"
    let links = |cat: &str| -> Vec<String> {
        m.get("links")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter(|l| l.get("category").and_then(Value::as_str) == Some(cat))
                    .filter_map(|l| l.get("name").and_then(Value::as_str).map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    let or_links = |k: &str, cat: &str| {
        let v = list(k);
        if v.is_empty() {
            links(cat)
        } else {
            v
        }
    };
    let id = s("id")?;
    let title = s("name")?;
    let mut it = Item {
        kind: if kind == "series" {
            Kind::Series
        } else {
            Kind::Movie
        },
        id: remote_id(kind, &id),
        title,
        overview: s("description").unwrap_or_default(),
        poster: s("poster"),
        backdrop: s("background"),
        logo: s("logo"),
        genres: {
            let g = or_links("genres", "Genres");
            if g.is_empty() {
                list("genre")
            } else {
                g
            }
        },
        imdb_id: s("imdb_id")
            .and_then(|i| imdb_id(&i))
            .or_else(|| id.starts_with("tt").then(|| imdb_id(&id)).flatten()),
        tmdb_id: m
            .get("moviedb_id")
            .or_else(|| m.get("tmdb_id"))
            .filter(|v| !v.is_null())
            .cloned()
            .or_else(|| {
                id.strip_prefix("tmdb:")
                    .map(|t| Value::String(t.to_owned()))
            }),
        released: s("released").map(|r| r.chars().take(10).collect()),
        ..Default::default()
    };
    let info = s("releaseInfo").or_else(|| m.get("year").map(|y| y.to_string()));
    if let Some(info) = info {
        let years: Vec<u32> = info
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| p.len() == 4)
            .filter_map(|p| p.parse().ok())
            .collect();
        it.year = years.first().copied();
        it.end_year = years.get(1).copied();
    }
    if it.year.is_none() {
        it.year = it
            .released
            .as_deref()
            .and_then(|r| r.get(..4)?.parse().ok());
    }
    if let Some(r) = s("imdbRating").and_then(|r| r.parse::<f32>().ok()) {
        it.score = Some(r);
        it.score_source = Some("IMDb".into());
    }
    // the TMDB addon's cast has characters and photos
    let photos: Vec<CastMember> = m
        .pointer("/app_extras/cast")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    let get = |k: &str| {
                        c.get(k)
                            .and_then(Value::as_str)
                            .filter(|v| !v.trim().is_empty())
                            .map(str::to_owned)
                    };
                    Some(CastMember {
                        name: get("name")?,
                        character: get("character"),
                        photo: get("photo"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    it.cast = if photos.is_empty() {
        or_links("cast", "Cast")
            .into_iter()
            .map(|name| CastMember {
                name,
                ..Default::default()
            })
            .collect()
    } else {
        photos
    };
    it.cast.truncate(30);
    let directors = or_links("director", "Directors");
    let writers = or_links("writer", "Writers");
    if kind == "series" {
        it.creators = writers.clone();
    }
    if !directors.is_empty() {
        it.director = Some(directors.join(", "));
    }
    let trailer = m
        .get("trailers")
        .and_then(Value::as_array)
        .and_then(|t| {
            let id = |v: &Value| v.get("source").and_then(Value::as_str).map(str::to_owned);
            t.iter()
                .find(|v| v.get("type").and_then(Value::as_str) == Some("Trailer"))
                .and_then(id)
                .or_else(|| t.first().and_then(id))
        })
        .or_else(|| {
            let t = m.get("trailerStreams")?.as_array()?.first()?;
            t.get("ytId")?.as_str().map(str::to_owned)
        });
    it.addon = Some(AddonInfo {
        writers,
        country: s("country"),
        awards: s("awards"),
        trailer,
        source: source.to_owned(),
    });
    // "148 min", "2h 28min"
    if let Some(rt) = s("runtime") {
        let mut secs = 0.0;
        let mut num = String::new();
        for c in rt.chars() {
            if c.is_ascii_digit() {
                num.push(c);
            } else if !num.is_empty() {
                let n: f64 = num.parse().unwrap_or(0.0);
                secs += if c == 'h' { n * 3600.0 } else { n * 60.0 };
                num.clear();
            }
        }
        it.duration = (secs > 0.0).then_some(secs);
    }
    // a series' episodes, to fill in ours
    let mut seasons: HashMap<u32, Vec<Episode>> = HashMap::new();
    for v in m
        .get("videos")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let n = |k: &str| v.get(k).and_then(Value::as_u64).map(|n| n as u32);
        let (Some(se), Some(ep)) = (n("season"), n("episode").or(n("number"))) else {
            continue;
        };
        let vs = |k: &str| {
            v.get(k)
                .and_then(Value::as_str)
                .filter(|x| !x.trim().is_empty())
                .map(str::to_owned)
        };
        seasons.entry(se).or_default().push(Episode {
            number: ep,
            title: vs("name").or(vs("title")).unwrap_or_default(),
            overview: vs("overview").or(vs("description")).unwrap_or_default(),
            still: vs("thumbnail"),
            aired: vs("released").map(|r| r.chars().take(10).collect()),
            ..Default::default()
        });
    }
    let mut seasons: Vec<Season> = seasons
        .into_iter()
        .map(|(number, episodes)| Season {
            number,
            title: None,
            episodes,
        })
        .collect();
    seasons.sort_by_key(|s| s.number);
    it.seasons = seasons;
    Some(it)
}

/// Fill what `dst` is missing from `src`; anything `dst` has stays.
pub fn fill(dst: &mut Item, src: &Item) {
    fn opt<T: Clone>(d: &mut Option<T>, s: &Option<T>) {
        if d.is_none() {
            d.clone_from(s);
        }
    }
    if dst.overview.trim().is_empty() {
        dst.overview.clone_from(&src.overview);
    }
    if dst.genres.is_empty() {
        dst.genres.clone_from(&src.genres);
    }
    if dst.cast.is_empty() {
        dst.cast.clone_from(&src.cast);
    }
    if dst.creators.is_empty() {
        dst.creators.clone_from(&src.creators);
    }
    opt(&mut dst.year, &src.year);
    opt(&mut dst.poster, &src.poster);
    opt(&mut dst.backdrop, &src.backdrop);
    opt(&mut dst.logo, &src.logo);
    opt(&mut dst.duration, &src.duration);
    opt(&mut dst.director, &src.director);
    opt(&mut dst.released, &src.released);
    opt(&mut dst.end_year, &src.end_year);
    opt(&mut dst.imdb_id, &src.imdb_id);
    opt(&mut dst.tmdb_id, &src.tmdb_id);
    if dst.score.is_none() {
        dst.score = src.score;
        dst.score_source.clone_from(&src.score_source);
    }
    // an addon's own title: the full details beat the catalog's short ones
    let remote = split_remote(&dst.id).is_some();
    let photos = |c: &[CastMember]| c.iter().filter(|c| c.photo.is_some()).count();
    if remote && (photos(&src.cast), src.cast.len()) > (photos(&dst.cast), dst.cast.len()) {
        dst.cast.clone_from(&src.cast);
    }
    match (&mut dst.addon, &src.addon) {
        (None, s) => dst.addon.clone_from(s),
        (Some(d), Some(s)) => {
            if d.writers.is_empty() {
                d.writers.clone_from(&s.writers);
            }
            opt(&mut d.country, &s.country);
            opt(&mut d.awards, &s.awards);
            opt(&mut d.trailer, &s.trailer);
        }
        _ => {}
    }
    if remote && dst.seasons.is_empty() {
        dst.seasons.clone_from(&src.seasons);
    }
    for season in &mut dst.seasons {
        let Some(from) = src.seasons.iter().find(|s| s.number == season.number) else {
            continue;
        };
        for e in &mut season.episodes {
            let Some(f) = from.episodes.iter().find(|f| f.number == e.number) else {
                continue;
            };
            if e.title.trim().is_empty() {
                e.title.clone_from(&f.title);
            }
            if e.overview.trim().is_empty() {
                e.overview.clone_from(&f.overview);
            }
            opt(&mut e.still, &f.still);
            opt(&mut e.aired, &f.aired);
        }
    }
}

/// A language as our code: "eng", "English" and "en" are all "en".
pub fn lang_code(lang: &str) -> String {
    language(lang).0
}

/// Stremio's language ("eng", "pob", "en") as our code ("en", "pt-BR") and a name.
fn language(lang: &str) -> (String, String) {
    const LANGS: &[(&str, &str, &str)] = &[
        ("eng", "en", "English"),
        ("nor", "no", "Norwegian"),
        ("nob", "nb", "Norwegian Bokmål"),
        ("nno", "nn", "Norwegian Nynorsk"),
        ("swe", "sv", "Swedish"),
        ("dan", "da", "Danish"),
        ("fin", "fi", "Finnish"),
        ("ice", "is", "Icelandic"),
        ("isl", "is", "Icelandic"),
        ("ger", "de", "German"),
        ("deu", "de", "German"),
        ("dut", "nl", "Dutch"),
        ("nld", "nl", "Dutch"),
        ("fre", "fr", "French"),
        ("fra", "fr", "French"),
        ("spa", "es", "Spanish"),
        ("spn", "es", "Spanish (Latin America)"),
        ("ita", "it", "Italian"),
        ("por", "pt", "Portuguese"),
        ("pob", "pt-BR", "Portuguese (Brazil)"),
        ("pol", "pl", "Polish"),
        ("cze", "cs", "Czech"),
        ("ces", "cs", "Czech"),
        ("slo", "sk", "Slovak"),
        ("slk", "sk", "Slovak"),
        ("hun", "hu", "Hungarian"),
        ("rum", "ro", "Romanian"),
        ("ron", "ro", "Romanian"),
        ("bul", "bg", "Bulgarian"),
        ("gre", "el", "Greek"),
        ("ell", "el", "Greek"),
        ("tur", "tr", "Turkish"),
        ("rus", "ru", "Russian"),
        ("ukr", "uk", "Ukrainian"),
        ("hrv", "hr", "Croatian"),
        ("scc", "sr", "Serbian"),
        ("srp", "sr", "Serbian"),
        ("slv", "sl", "Slovenian"),
        ("est", "et", "Estonian"),
        ("lav", "lv", "Latvian"),
        ("lit", "lt", "Lithuanian"),
        ("ara", "ar", "Arabic"),
        ("heb", "he", "Hebrew"),
        ("per", "fa", "Persian"),
        ("fas", "fa", "Persian"),
        ("hin", "hi", "Hindi"),
        ("tha", "th", "Thai"),
        ("vie", "vi", "Vietnamese"),
        ("ind", "id", "Indonesian"),
        ("may", "ms", "Malay"),
        ("msa", "ms", "Malay"),
        ("chi", "zh", "Chinese"),
        ("zho", "zh", "Chinese"),
        ("zht", "zh-TW", "Chinese (Traditional)"),
        ("jpn", "ja", "Japanese"),
        ("kor", "ko", "Korean"),
    ];
    let l = lang.trim();
    let found = LANGS.iter().find(|(three, two, name)| {
        l.eq_ignore_ascii_case(three) || l.eq_ignore_ascii_case(two) || l.eq_ignore_ascii_case(name)
    });
    match found {
        Some((_, two, name)) => ((*two).to_owned(), (*name).to_owned()),
        None if l.is_empty() => ("und".into(), "Unknown".into()),
        None => (l.to_lowercase(), l.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages() {
        assert_eq!(language("eng"), ("en".into(), "English".into()));
        assert_eq!(language("EN"), ("en".into(), "English".into()));
        assert_eq!(language("pob").0, "pt-BR");
        assert_eq!(language(""), ("und".into(), "Unknown".into()));
        assert_eq!(language("xyz"), ("xyz".into(), "xyz".into()));
    }

    #[test]
    fn metas_and_filling() {
        let m: Value = serde_json::from_str(
            r#"{"id":"tt0903747","type":"series","name":"Breaking Bad","releaseInfo":"2008–2013",
                "imdbRating":"9.5","poster":"https://p/bb.jpg","description":"A teacher.",
                "links":[{"name":"Drama","category":"Genres"},{"name":"Bryan Cranston","category":"Cast"}],
                "videos":[{"season":1,"episode":1,"name":"Pilot","thumbnail":"https://p/1.jpg"}]}"#,
        )
        .unwrap();
        let it = to_item(&m, "series", "Cinemeta").unwrap();
        assert_eq!(it.id, "addon:series:tt0903747");
        assert_eq!(split_remote(&it.id), Some(("series", "tt0903747")));
        assert_eq!((it.year, it.end_year), (Some(2008), Some(2013)));
        assert_eq!(it.imdb_id.as_deref(), Some("tt0903747"));
        assert_eq!(it.genres, ["Drama"]);
        assert_eq!(it.cast[0].name, "Bryan Cranston");

        let mut ours = Item {
            title: "Breaking Bad".into(),
            overview: "Ours.".into(),
            seasons: vec![Season {
                number: 1,
                title: None,
                episodes: vec![Episode {
                    number: 1,
                    ..Default::default()
                }],
            }],
            ..Default::default()
        };
        fill(&mut ours, &it);
        assert_eq!(ours.overview, "Ours."); // the server wins
        assert_eq!(ours.poster.as_deref(), Some("https://p/bb.jpg"));
        assert_eq!(ours.score, Some(9.5));
        assert_eq!(ours.seasons[0].episodes[0].title, "Pilot");
    }

    #[test]
    fn configure_urls() {
        assert_eq!(
            configure_url("https://a.b/eyJ9/manifest.json"),
            "https://a.b/eyJ9/configure"
        );
        assert_eq!(
            configure_url("stremio://a.b/manifest.json"),
            "https://a.b/configure"
        );
        assert_eq!(configure_url("https://a.b/"), "https://a.b/configure");
    }

    #[test]
    fn imdb_ids() {
        assert_eq!(imdb_id(" tt0903747 ").as_deref(), Some("tt0903747"));
        assert_eq!(imdb_id("111161").as_deref(), Some("tt0111161"));
        assert_eq!(
            imdb_id("https://www.imdb.com/title/tt1375666/").as_deref(),
            Some("tt1375666")
        );
        assert_eq!(imdb_id("tt"), None);
        assert_eq!(imdb_id(""), None);
    }
}
