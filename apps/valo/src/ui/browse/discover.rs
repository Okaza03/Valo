//! What the admin's Stremio addons add to browsing: catalog rows on the home
//! page, details for titles that aren't on the server (with a Request
//! button), and details our own titles are missing (filled in, never
//! replaced).
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use eframe::egui::{self, pos2, vec2, Rect, RichText, Sense, Stroke, Ui};

use super::details::{
    backdrop, caps, cast_grid, column, date_long, dot, fill_button, info_rows, panel, placed,
    play_colors, poster_tile, similar, tag, title_art, GOLD, NARROW, TEAL,
};
use super::{grid, Browse, Card, Click, GAP, SIDE};
use crate::addons::{self, imdb_id, kind_name, split_remote, title_key, Row};
use crate::api::{Addon, Client, Episode, Item, Kind, Library, MediaRequest, NoAdminApi};
use crate::config::Progress;
use crate::filmography::{self, Filmography};
use crate::images::Images;
use crate::ui::chrome::{self, Icon};
use crate::ui::{bold, image_cover, mono, placeholder, segmented};
use crate::ui::{ERROR, LINE, MUTED, OK, TEXT, WARN};
use crate::util::{fmt_runtime, Task};

/// Metadata lookups per batch.
const BATCH: usize = 24;
/// Search the addons once typing pauses for this long.
const TYPING: Duration = Duration::from_millis(300);

#[derive(Default)]
pub(super) struct State {
    load: Option<Task<Result<addons::Catalogs>>>,
    /// The addon list has come in (possibly empty).
    loaded: bool,
    addons: Vec<Addon>,
    rows: Vec<Row>,
    /// Each row as item ids: ours where the IMDb id matches, else the addon's.
    pub shown: Vec<Shown>,
    /// The Streaming page: which catalog, and movies, series or both.
    service: Option<String>,
    service_kind: Option<Kind>,
    /// Addon titles, by [`addons::remote_id`].
    pub remote: HashMap<String, Item>,
    /// From metadata addons, by "movie:tt1375666" (None: none of them has it).
    metas: HashMap<String, Option<Item>>,
    want: Vec<(String, String)>,
    asked: HashSet<String>,
    fetching: Option<Task<addons::Metas>>,
    /// The user's own requests; Err: the server doesn't take requests.
    requests: Option<std::result::Result<Vec<MediaRequest>, String>>,
    requests_task: Option<Task<Result<Vec<MediaRequest>>>>,
    sending: Option<Task<Result<MediaRequest>>>,
    send_error: Option<String>,
    /// Filmographies by person name.
    films: HashMap<String, Films>,
    /// Cast photos from Wikidata, by name (None: it has none).
    photos: HashMap<String, Option<String>>,
    photos_task: Option<Task<PhotoAnswer>>,
    photos_want: Vec<String>,
    film_filter: FilmFilter,
    /// The season shown on an addon series' page: (item id, season index).
    season: (String, usize),
    searchable: Vec<addons::Searchable>,
    /// What's typed in the search box, and since when.
    typed: (String, Option<Instant>),
    searching: Option<Search>,
    /// Addon titles for a search, as ids; ours left out.
    results: Option<(String, Vec<String>)>,
}

impl State {
    pub fn reload(&mut self, ctx: &egui::Context, client: &Client) {
        let c = client.clone();
        self.load = Some(Task::spawn(ctx, move || addons::catalogs(&c)));
        self.requests = None;
    }

    fn has_meta(&self) -> bool {
        self.addons
            .iter()
            .any(|a| a.resources.iter().any(|r| r.name == "meta"))
    }

    /// Fill a freshly loaded library from what's known, and ask for the
    /// rest of what its cards are missing.
    pub fn on_library(&mut self, lib: &mut Library) {
        self.apply(lib);
        self.queue_gaps(lib);
        self.rebuild(lib);
    }

    fn apply(&self, lib: &mut Library) {
        for it in lib.movies.iter_mut().chain(lib.series.iter_mut()) {
            let key = it
                .imdb_id
                .as_deref()
                .and_then(imdb_id)
                .map(|id| format!("{}:{id}", kind_name(it.kind)));
            if let Some(Some(m)) = key.and_then(|k| self.metas.get(&k)) {
                addons::fill(it, m);
            }
        }
    }

    /// Titles without a poster, backdrop or overview.
    fn queue_gaps(&mut self, lib: &Library) {
        if !self.has_meta() {
            return;
        }
        let gaps: Vec<(Kind, String)> = lib
            .all()
            .filter(|i| i.poster.is_none() || i.backdrop.is_none() || i.overview.is_empty())
            .filter_map(|i| Some((i.kind, imdb_id(i.imdb_id.as_deref()?)?)))
            .collect();
        for (k, id) in gaps {
            self.want(kind_name(k), &id);
        }
    }

    /// Ask the metadata addons about this title (once).
    pub fn want(&mut self, kind: &str, id: &str) {
        let key = format!("{kind}:{id}");
        if self.loaded && !self.has_meta() {
            return;
        }
        if !self.metas.contains_key(&key) && self.asked.insert(key) {
            self.want.push((kind.to_owned(), id.to_owned()));
        }
    }

    /// [`want`](Self::want), ahead of the rest: the page on screen needs it.
    pub fn want_now(&mut self, kind: &str, id: &str) {
        self.want(kind, id);
        if let Some(i) = self.want.iter().position(|(k, x)| k == kind && x == id) {
            let w = self.want.remove(i);
            self.want.insert(0, w);
        }
    }

    /// Still loading the addons or details.
    pub fn busy(&self) -> bool {
        self.load.is_some() || self.fetching.is_some()
    }

    /// Photos for a title's cast members who have none.
    pub fn want_photos(&mut self, item: &Item) {
        for c in item.cast.iter().take(15).filter(|c| c.photo.is_none()) {
            if !self.photos.contains_key(&c.name) && !self.photos_want.contains(&c.name) {
                self.photos_want.push(c.name.clone());
            }
        }
    }

    /// Fill in cast photos we've found, ours and the addons' titles alike.
    fn apply_photos(&mut self, lib: &mut Option<Arc<Library>>) {
        let photos = &self.photos;
        let fill = |it: &mut Item| {
            for c in it.cast.iter_mut().filter(|c| c.photo.is_none()) {
                if let Some(Some(p)) = photos.get(&c.name) {
                    c.photo = Some(p.clone());
                }
            }
        };
        self.remote.values_mut().for_each(fill);
        let needs = |l: &Library| {
            l.all().any(|i| {
                i.cast
                    .iter()
                    .any(|c| c.photo.is_none() && matches!(photos.get(&c.name), Some(Some(_))))
            })
        };
        if let Some(l) = lib.as_mut().filter(|l| needs(l)) {
            let l = Arc::make_mut(l);
            l.movies
                .iter_mut()
                .chain(l.series.iter_mut())
                .for_each(fill);
        }
    }

    /// Our title's full details, the first time its page opens.
    pub fn want_item(&mut self, item: &Item) {
        self.want_photos(item);
        if let Some(id) = item.imdb_id.as_deref().and_then(imdb_id) {
            self.want_now(kind_name(item.kind), &id);
        }
    }

    /// The rows as ids, matching addon titles to ours by IMDb id.
    fn rebuild(&mut self, lib: &Library) {
        let ours = Ours::new(lib);
        self.shown = self
            .rows
            .iter()
            .map(|r| {
                let ids = r
                    .items
                    .iter()
                    .map(|it| match ours.find(it) {
                        Some(id) => id.to_owned(),
                        None => {
                            let mut it = it.clone();
                            if let Some(Some(m)) = split_remote(&it.id)
                                .and_then(|(k, id)| self.metas.get(&format!("{k}:{id}")))
                            {
                                addons::fill(&mut it, m);
                            }
                            let id = it.id.clone();
                            self.remote.entry(id.clone()).or_insert(it);
                            id
                        }
                    })
                    .collect();
                Shown {
                    key: r.key.clone(),
                    title: r.title.clone(),
                    source: r.source.clone(),
                    service: r.service,
                    kind: r.kind,
                    ids,
                }
            })
            .collect();
    }

    /// Take finished loads; fills `lib` when new details came in.
    pub fn poll(&mut self, ctx: &egui::Context, client: &Client, lib: &mut Option<Arc<Library>>) {
        if let Some(r) = self.load.as_mut().and_then(|t| t.poll()) {
            self.load = None;
            self.loaded = true;
            let c = r.unwrap_or_default();
            self.addons = c.addons;
            self.rows = c.rows;
            self.searchable = c.search;
            self.results = None;
            if let Some(l) = lib.as_ref() {
                let l = l.clone();
                self.queue_gaps(&l);
                self.rebuild(&l);
            }
        }
        if let Some(found) = self.fetching.as_mut().and_then(|t| t.poll()) {
            self.fetching = None;
            for (key, m) in found {
                if let Some(it) = &m {
                    // a remote title's page shows the full details
                    if let Some(r) = self.remote.get_mut(&format!("addon:{key}")) {
                        addons::fill(r, it);
                    }
                }
                self.metas.insert(key, m);
            }
            if let Some(l) = lib.as_mut() {
                self.apply(Arc::make_mut(l));
            }
        }
        if self.fetching.is_none() && self.loaded && !self.want.is_empty() {
            let n = self.want.len().min(BATCH);
            let batch: Vec<(String, String)> = self.want.drain(..n).collect();
            let a = self.addons.clone();
            self.fetching = Some(Task::spawn(ctx, move || addons::metas(&a, &batch)));
        }
        if let Some(mut sr) = self.searching.take() {
            let mut new = false;
            let finished = loop {
                match sr.rx.try_recv() {
                    Ok((n, items)) => {
                        sr.parts[n] = items;
                        sr.left = sr.left.saturating_sub(1);
                        new = true;
                    }
                    // every catalog answered: done, without waiting for the channel to close
                    Err(TryRecvError::Empty) => break sr.left == 0,
                    Err(TryRecvError::Disconnected) => break true,
                }
            };
            if new || finished {
                let ours = lib.as_ref().map(|l| l.clone());
                let ours = ours.as_deref().map(Ours::new);
                let mut ids = Vec::new();
                for mut it in addons::merge(&sr.parts) {
                    if ours.as_ref().is_some_and(|o| o.find(&it).is_some()) {
                        continue; // the library's own results show it
                    }
                    if let Some(Some(m)) = split_remote(&it.id)
                        .and_then(|(k, id)| self.metas.get(&format!("{k}:{id}")))
                    {
                        addons::fill(&mut it, m);
                    }
                    ids.push(it.id.clone());
                    self.remote.entry(it.id.clone()).or_insert(it);
                }
                self.results = Some((sr.q.clone(), ids));
            }
            if !finished {
                self.searching = Some(sr);
            }
        }
        if let Some((asked, r)) = self.photos_task.as_mut().and_then(|t| t.poll()) {
            self.photos_task = None;
            let found = r.unwrap_or_default();
            for n in asked {
                let p = found.get(&n).cloned();
                self.photos.insert(n, p);
            }
            self.apply_photos(lib);
        }
        if self.photos_task.is_none() && !self.photos_want.is_empty() {
            let names = std::mem::take(&mut self.photos_want);
            self.photos_task = Some(Task::spawn(ctx, move || {
                let r = filmography::photos(&names);
                (names, r)
            }));
        }
        if let Some(r) = self.requests_task.as_mut().and_then(|t| t.poll()) {
            self.requests_task = None;
            self.requests = Some(r.map_err(|e| {
                if e.is::<NoAdminApi>() {
                    "This server doesn't take requests.".to_owned()
                } else {
                    format!("{e:#}")
                }
            }));
        }
        if let Some(r) = self.sending.as_mut().and_then(|t| t.poll()) {
            self.sending = None;
            match r {
                Ok(req) => {
                    if let Some(Ok(l)) = self.requests.as_mut() {
                        l.push(req);
                    }
                    let c = client.clone();
                    self.requests_task = Some(Task::spawn(ctx, move || c.my_requests()));
                }
                Err(e) if e.is::<NoAdminApi>() => {
                    self.send_error = Some("This server doesn't take requests.".into())
                }
                Err(e) => self.send_error = Some(format!("{e:#}")),
            }
        }
    }

    /// The search box says `q`: search the addons once typing pauses.
    /// The addon titles found for it so far, and whether more may come.
    pub fn search(&mut self, ctx: &egui::Context, q: &str) -> (Vec<String>, bool) {
        let q = q.trim();
        if self.searchable.is_empty() || q.chars().count() < 2 {
            return (Vec::new(), self.load.is_some() && q.chars().count() >= 2);
        }
        if self.typed.0 != q {
            self.typed = (q.to_owned(), Some(Instant::now()));
        }
        let have = self.results.as_ref().is_some_and(|(r, _)| r == q);
        let running = self.searching.as_ref().is_some_and(|s| s.q == q);
        if !have && !running {
            match self.typed.1 {
                Some(t) if t.elapsed() >= TYPING => {
                    let c = ctx.clone();
                    let rx = addons::search(
                        self.addons.clone(),
                        self.searchable.clone(),
                        q.to_owned(),
                        move || c.request_repaint(),
                    );
                    self.searching = Some(Search {
                        q: q.to_owned(),
                        rx,
                        parts: vec![Vec::new(); self.searchable.len()],
                        left: self.searchable.len(),
                    });
                }
                Some(t) => ctx.request_repaint_after(TYPING.saturating_sub(t.elapsed())),
                None => {}
            }
        }
        let ids = match &self.results {
            Some((r, ids)) if r == q => ids.clone(),
            _ => Vec::new(),
        };
        // still waiting for typing to pause, or for some addons to answer
        let more = !have || self.searching.as_ref().is_some_and(|s| s.q == q);
        (ids, more)
    }

    /// The user's request for this title, if they made one.
    fn request_for(&self, item: &Item) -> Option<&MediaRequest> {
        let Some(Ok(list)) = &self.requests else {
            return None;
        };
        let title = item.title.to_lowercase();
        list.iter()
            .rev()
            .find(|r| match (&r.imdb_id, &item.imdb_id) {
                (Some(a), Some(b)) => a == b,
                _ => r.title.to_lowercase() == title && (r.year.is_none() || r.year == item.year),
            })
    }
}

/// A title's year and our id.
type YearId<'a> = (Option<u32>, &'a str);

/// A catalog, its titles as ids.
pub(super) struct Shown {
    /// [`addons::catalog_key`]
    pub key: String,
    /// "Netflix · Movies"
    pub title: String,
    /// "Netflix"
    pub source: String,
    /// A streaming service's list (see [`Row::service`]).
    pub service: bool,
    pub kind: Kind,
    pub ids: Vec<String>,
}

/// The names asked about, and the photos found for them.
type PhotoAnswer = (Vec<String>, Result<HashMap<String, String>>);

/// A person's filmography from Wikidata.
enum Films {
    Loading(Task<Result<Filmography>>),
    Done(std::result::Result<Filmography, String>),
}

/// Which of a person's titles to show.
#[derive(Clone, Copy, PartialEq, Default)]
pub(super) enum FilmFilter {
    #[default]
    All,
    OnServer,
    NotOnServer,
}

/// Titles per catalog row on the home page.
const HOME_ROW: usize = 40;

/// A search on its way: each catalog's titles as they come in.
struct Search {
    q: String,
    rx: Receiver<(usize, Vec<Item>)>,
    parts: Vec<Vec<Item>>,
    /// Catalogs that haven't answered yet.
    left: usize,
}

/// Our titles, to find an addon's title among them.
struct Ours<'a> {
    by_imdb: HashMap<(Kind, &'a str), &'a str>,
    /// By [`title_key`].
    by_title: HashMap<(Kind, String), Vec<YearId<'a>>>,
}

impl<'a> Ours<'a> {
    fn new(lib: &'a Library) -> Self {
        let mut o = Self {
            by_imdb: HashMap::new(),
            by_title: HashMap::new(),
        };
        for i in lib.all() {
            if let Some(imdb) = i.imdb_id.as_deref() {
                o.by_imdb.insert((i.kind, imdb.trim()), i.id.as_str());
            }
            o.by_title
                .entry((i.kind, title_key(&i.title)))
                .or_default()
                .push((i.year, i.id.as_str()));
        }
        o
    }

    /// By IMDb id; else by title and year (a year apart is fine: release
    /// dates differ by country), for titles the server has no IMDb id for.
    fn find(&self, it: &Item) -> Option<&'a str> {
        if let Some(id) = it
            .imdb_id
            .as_deref()
            .and_then(|i| self.by_imdb.get(&(it.kind, i)))
        {
            return Some(id);
        }
        let same = self.by_title.get(&(it.kind, title_key(&it.title)))?;
        let near = |y: Option<u32>| match (y, it.year) {
            (Some(a), Some(b)) => a.abs_diff(b) <= 1,
            _ => true,
        };
        // the closest year wins, for remakes with the same name
        same.iter()
            .filter(|(y, _)| near(*y))
            .min_by_key(|(y, _)| y.zip(it.year).map_or(2, |(a, b)| a.abs_diff(b)))
            .map(|(_, id)| *id)
    }
}

impl State {
    /// The catalogs' names ("Netflix", "HBO Max", …) in the admin's order:
    /// the streaming services, or every catalog when none is one.
    pub fn sources(&self) -> Vec<&str> {
        let services = self.shown.iter().any(|s| s.service);
        let mut out: Vec<&str> = Vec::new();
        for s in self.shown.iter().filter(|s| s.service || !services) {
            if !out.contains(&s.source.as_str()) {
                out.push(&s.source);
            }
        }
        out
    }

    /// "Netflix, Prime Video": the streaming services listing this title.
    pub fn streaming_on(&self, id: &str) -> Option<String> {
        let mut out: Vec<&str> = Vec::new();
        for s in self
            .shown
            .iter()
            .filter(|s| s.service && s.ids.iter().any(|i| i == id))
        {
            if !out.contains(&s.source.as_str()) {
                out.push(&s.source);
            }
        }
        (!out.is_empty()).then(|| out.join(", "))
    }

    /// A person's filmography: loads it the first time it's asked for.
    fn films(
        &mut self,
        ctx: &egui::Context,
        name: &str,
    ) -> Option<&std::result::Result<Filmography, String>> {
        let key = name.to_lowercase();
        let entry = self.films.entry(key).or_insert_with(|| {
            let n = name.to_owned();
            Films::Loading(Task::spawn(ctx, move || filmography::lookup(&n)))
        });
        if let Films::Loading(t) = entry {
            if let Some(r) = t.poll() {
                *entry = Films::Done(r.map_err(|e| format!("{e:#}")));
            }
        }
        match entry {
            Films::Done(r) => Some(r),
            Films::Loading(_) => None,
        }
    }

    /// Their photo from Wikidata, once loaded.
    pub fn person_photo(&self, name: &str) -> Option<String> {
        match self.films.get(&name.to_lowercase()) {
            Some(Films::Done(Ok(f))) => f.photo.clone(),
            _ => None,
        }
    }
}

impl Browse {
    /// Everything a person has been in, and which of it is on the server.
    pub(super) fn filmography_section(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        name: &str,
        images: &mut Images,
        progress: &Progress,
    ) -> Option<Click> {
        let ctx = ui.ctx().clone();
        let films = self.discover.films(&ctx, name).cloned();
        ui.add_space(12.0);
        let heading = |ui: &mut Ui, n: Option<usize>| {
            ui.horizontal(|ui| {
                ui.add_space(SIDE);
                ui.vertical(|ui| {
                    caps(ui, "Everything they've been in", TEAL());
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(bold("Filmography", 24.0));
                        if let Some(n) = n {
                            ui.label(mono(n.to_string(), 13.0).color(MUTED()));
                        }
                    });
                });
            });
        };
        let f = match films {
            None => {
                heading(ui, None);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(SIDE);
                    ui.add(egui::Spinner::new().size(14.0).color(MUTED()));
                    ui.label(mono("Looking them up…", 11.0).color(MUTED()));
                });
                return None;
            }
            Some(Err(e)) => {
                heading(ui, None);
                ui.horizontal(|ui| {
                    ui.add_space(SIDE);
                    ui.label(RichText::new(format!("Couldn't look them up: {e}")).color(MUTED()));
                });
                return None;
            }
            Some(Ok(f)) if f.credits.is_empty() => return None,
            Some(Ok(f)) => f,
        };
        // each credit as ours (on the server) or an addon title to request
        let ours = Ours::new(lib);
        let ids: Vec<(String, bool)> = f
            .credits
            .iter()
            .map(|c| {
                let it = Item {
                    id: addons::remote_id(kind_name(c.kind), &c.imdb),
                    kind: c.kind,
                    title: c.title.clone(),
                    year: c.year,
                    imdb_id: Some(c.imdb.clone()),
                    poster: Some(format!(
                        "https://images.metahub.space/poster/small/{}/img",
                        c.imdb
                    )),
                    backdrop: Some(format!(
                        "https://images.metahub.space/background/medium/{}/img",
                        c.imdb
                    )),
                    ..Default::default()
                };
                match ours.find(&it) {
                    Some(id) => (id.to_owned(), true),
                    None => {
                        let id = it.id.clone();
                        self.discover.remote.entry(id.clone()).or_insert(it);
                        (id, false)
                    }
                }
            })
            .collect();
        let on = ids.iter().filter(|(_, o)| *o).count();
        heading(ui, Some(ids.len()));
        ui.add_space(12.0);
        let mut filter = self.discover.film_filter;
        ui.horizontal(|ui| {
            ui.add_space(SIDE - 22.0);
            segmented(
                ui,
                &mut filter,
                &[
                    (FilmFilter::All, "All", Some(ids.len())),
                    (FilmFilter::OnServer, "On the server", Some(on)),
                    (
                        FilmFilter::NotOnServer,
                        "Not on the server",
                        Some(ids.len() - on),
                    ),
                ],
            );
        });
        self.discover.film_filter = filter;
        ui.add_space(12.0);
        let shown: Vec<String> = ids
            .into_iter()
            .filter(|(_, o)| match filter {
                FilmFilter::All => true,
                FilmFilter::OnServer => *o,
                FilmFilter::NotOnServer => !*o,
            })
            .map(|(id, _)| id)
            .collect();
        if shown.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(SIDE);
                ui.label(mono("Nothing here", 12.0).color(MUTED()));
            });
            return None;
        }
        let cards = self.catalog_cards(lib, shown.iter(), progress);
        let marked = |id: &str| self.watchlist.contains(&self.key(id));
        grid(ui, &cards, &marked, images)
    }

    /// Cards for catalog titles: ours as usual, the addon's marked as not
    /// on the server.
    pub(super) fn catalog_cards<'a>(
        &self,
        lib: &'a Library,
        ids: impl Iterator<Item = &'a String>,
        progress: &Progress,
    ) -> Vec<Card> {
        let items = ids.filter_map(|id| lib.find(id).or_else(|| self.discover.remote.get(id)));
        let mut cards = self.poster_cards(items, progress);
        for c in &mut cards {
            if self.discover.remote.contains_key(&c.item) {
                c.sub = Some(match c.sub.take() {
                    Some(y) => format!("{y} · not on server"),
                    None => "Not on server".into(),
                });
            }
        }
        cards
    }

    /// The catalogs as home page rows: (key, title, cards).
    pub(super) fn catalog_rows(
        &self,
        lib: &Library,
        progress: &Progress,
    ) -> Vec<(String, String, Vec<Card>)> {
        self.discover
            .shown
            .iter()
            .filter(|s| self.layout.shows(&s.key))
            .map(|s| {
                let cards = self.catalog_cards(lib, s.ids.iter().take(HOME_ROW), progress);
                (s.key.clone(), s.title.clone(), cards)
            })
            .collect()
    }

    /// Everything on one streaming service (or other catalog), as a grid.
    pub(super) fn streaming_page(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        images: &mut Images,
        progress: &Progress,
    ) -> Option<Click> {
        let sources: Vec<String> = self
            .discover
            .sources()
            .into_iter()
            .map(str::to_owned)
            .collect();
        ui.add_space(28.0);
        if sources.is_empty() {
            ui.add_space(80.0);
            ui.vertical_centered(|ui| {
                if self.discover.busy() {
                    ui.add(egui::Spinner::new().size(22.0).color(MUTED()));
                    ui.label(mono("Loading the catalogs", 12.0).color(MUTED()));
                } else {
                    ui.label(mono("No catalogs yet", 13.0));
                    ui.label(
                        RichText::new("An admin can add them under Admin › Addons, for example Streaming Catalogs.")
                            .color(MUTED()),
                    );
                }
            });
            return None;
        }
        let st = &mut self.discover;
        let pick = match &st.service {
            Some(s) if sources.contains(s) => s.clone(),
            _ => sources[0].clone(),
        };
        st.service = Some(pick.clone());
        let mine: Vec<(Kind, Vec<String>)> = st
            .shown
            .iter()
            .filter(|s| s.source == pick)
            .map(|s| (s.kind, s.ids.clone()))
            .collect();
        let count = |k: Kind| -> usize {
            mine.iter()
                .filter(|(sk, _)| *sk == k)
                .map(|(_, ids)| ids.len())
                .sum()
        };
        let (movies, series) = (count(Kind::Movie), count(Kind::Series));
        if st.service_kind.is_some_and(|k| count(k) == 0) {
            st.service_kind = None;
        }

        let mut picked = pick.clone();
        let mut kind = st.service_kind;
        ui.horizontal(|ui| {
            ui.add_space(SIDE);
            ui.vertical(|ui| {
                caps(ui, "Streaming", TEAL());
                ui.add_space(4.0);
                ui.label(bold(&pick, 34.0));
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "What's on it now. Titles on the server play; the rest you can request.",
                    )
                    .color(MUTED()),
                );
            });
        });
        ui.add_space(16.0);
        let chips: Vec<(String, &str, Option<usize>)> = sources
            .iter()
            .map(|s| (s.clone(), s.as_str(), None))
            .collect();
        ui.horizontal(|ui| {
            ui.add_space(SIDE - 22.0);
            egui::ScrollArea::horizontal()
                .id_salt("streaming_sources")
                .show(ui, |ui| segmented(ui, &mut picked, &chips));
        });
        if movies > 0 && series > 0 {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(SIDE - 22.0);
                segmented(
                    ui,
                    &mut kind,
                    &[
                        (None, "All", Some(movies + series)),
                        (Some(Kind::Movie), "Movies", Some(movies)),
                        (Some(Kind::Series), "TV shows", Some(series)),
                    ],
                );
            });
        }
        let st = &mut self.discover;
        if picked != pick {
            st.service = Some(picked);
            self.to_top = true;
        }
        st.service_kind = kind;

        let groups: Vec<(&str, Vec<String>)> =
            [(Kind::Movie, "Movies"), (Kind::Series, "TV shows")]
                .into_iter()
                .filter(|(k, _)| kind.is_none_or(|w| w == *k))
                .map(|(k, label)| {
                    let mut ids: Vec<String> = Vec::new();
                    for (_, list) in mine.iter().filter(|(sk, _)| *sk == k) {
                        for id in list {
                            if !ids.contains(id) {
                                ids.push(id.clone());
                            }
                        }
                    }
                    (label, ids)
                })
                .filter(|(_, ids)| !ids.is_empty())
                .collect();
        let mut click = None;
        let both = groups.len() > 1;
        for (label, ids) in groups {
            ui.add_space(22.0);
            if both {
                ui.horizontal(|ui| {
                    ui.add_space(SIDE);
                    ui.label(crate::ui::heading(label));
                    ui.label(mono(ids.len().to_string(), 13.0).color(MUTED()));
                });
                ui.add_space(8.0);
            }
            let cards = self.catalog_cards(lib, ids.iter(), progress);
            let marked = |id: &str| self.watchlist.contains(&self.key(id));
            click = grid(ui, &cards, &marked, images).or(click);
        }
        click
    }

    /// A title from an addon that isn't on the server: its details and a
    /// Request button.
    pub(super) fn remote_page(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        item: &Item,
        images: &mut Images,
    ) -> Option<Click> {
        let mut click = None;
        let ctx = ui.ctx().clone();
        let st = &mut self.discover;
        if let Some((k, id)) = split_remote(&item.id) {
            st.want_now(k, id);
        }
        st.want_photos(item);
        if st.requests.is_none() && st.requests_task.is_none() && self.client.token.is_some() {
            let c = self.client.clone();
            st.requests_task = Some(Task::spawn(&ctx, move || c.my_requests()));
        }

        let w = ui.available_width();
        let h = (w * 0.42).clamp(440.0, 620.0);
        let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        let art = item.backdrop.as_ref().or(item.poster.as_ref());
        let tex = art.and_then(|u| images.get(u));
        backdrop(&ui.painter_at(rect), rect, tex.as_ref(), &item.title, 140);
        let back = Rect::from_min_size(rect.min + vec2(28.0, 22.0), vec2(300.0, 24.0));
        placed(ui, back, "remote_back", 0.0, |ui| {
            let r = super::details::link(
                ui,
                "Back",
                13.0,
                TEXT().gamma_multiply(0.8),
                Some((Icon::ChevronLeft, false)),
            );
            if r.clicked() {
                click = Some(Click::Back);
            }
        });

        let (m, inner) = column(ui);
        let left = rect.left() + m;
        let show_poster = inner >= 860.0;
        let poster_w = if show_poster {
            (inner * 0.2).clamp(200.0, 280.0)
        } else {
            0.0
        };
        if show_poster {
            let ph = poster_w * 1.5;
            let pr =
                Rect::from_min_size(pos2(left, rect.center().y - ph / 2.0), vec2(poster_w, ph));
            let p = ui.painter_at(rect);
            match item.poster.as_ref().and_then(|u| images.get(u)) {
                Some(t) => image_cover(&p, pr, &t, 8, egui::Color32::WHITE),
                None => placeholder(&p, pr, &item.title, 8),
            }
        }
        let tx = left + if show_poster { poster_w + 40.0 } else { 0.0 };
        let text_rect = Rect::from_min_max(
            pos2(tx, rect.top() + 60.0),
            pos2(left + inner, rect.bottom() - 30.0),
        );
        let st = &mut self.discover;
        let client = self.client.clone();
        placed(ui, text_rect, "remote_hero", 0.5, |ui| {
            let mut kinds = vec![match item.kind {
                Kind::Movie => "Movie".to_owned(),
                Kind::Series => "Series".to_owned(),
            }];
            if !item.genres.is_empty() {
                kinds.push(item.genres.join(" · "));
            }
            caps(ui, &kinds.join("   ·   "), TEXT().gamma_multiply(0.65));
            ui.add_space(8.0);
            title_art(ui, item, images, vec2(560.0, 96.0), 50.0);
            if let Some(s) = st.streaming_on(&item.id) {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("Streaming on {s}"))
                        .size(15.0)
                        .color(TEAL()),
                );
            }
            ui.add_space(14.0);
            if !item.overview.is_empty() {
                ui.add(
                    egui::Label::new(
                        RichText::new(&item.overview)
                            .size(16.0)
                            .color(TEXT().gamma_multiply(0.75)),
                    )
                    .wrap(),
                );
                ui.add_space(14.0);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let mut first = true;
                let mut sep = |ui: &mut Ui| {
                    if !std::mem::take(&mut first) {
                        dot(ui);
                    }
                };
                if let Some(s) = item.score {
                    sep(ui);
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    chrome::draw_icon(ui.painter(), r.center(), Icon::Star, GOLD());
                    ui.label(RichText::new(format!("{s:.1}")).size(15.0).color(TEXT()));
                    if let Some(src) = &item.score_source {
                        ui.label(RichText::new(src).size(15.0).color(MUTED()));
                    }
                }
                if let Some(y) = item.year {
                    sep(ui);
                    let years = match item.end_year {
                        Some(e) if e != y => format!("{y}–{e}"),
                        _ => y.to_string(),
                    };
                    ui.label(RichText::new(years).size(15.0).color(GOLD()));
                }
                if let Some(d) = item.duration {
                    sep(ui);
                    ui.label(RichText::new(fmt_runtime(d)).size(15.0).color(TEXT()));
                }
                if let Some(d) = &item.director {
                    sep(ui);
                    tag(ui, d);
                }
            });
            ui.add_space(22.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                match st.request_for(item) {
                    Some(req) => {
                        let (col, text) = match req.status.as_str() {
                            "approved" => (TEAL(), "Requested · approved, on its way"),
                            "available" => (OK(), "Requested · now available"),
                            "denied" => (ERROR(), "Requested · denied"),
                            _ => (WARN(), "Requested · waiting for the admin"),
                        };
                        ui.painter().circle_filled(r.center(), 3.5, col);
                        ui.label(RichText::new(text).size(13.0).color(col));
                        if let Some(n) = req.note.as_ref().filter(|n| !n.trim().is_empty()) {
                            ui.label(RichText::new(format!("“{n}”")).italics().color(MUTED()));
                        }
                    }
                    None => {
                        ui.painter().circle_filled(r.center(), 3.5, MUTED());
                        ui.label(RichText::new("Not on the server").size(13.0).color(MUTED()));
                        ui.add_space(8.0);
                        match &st.requests {
                            Some(Err(e)) => {
                                ui.label(RichText::new(e).size(13.0).color(MUTED()));
                            }
                            _ if client.token.is_none() => {}
                            _ => {
                                let sending = st.sending.is_some();
                                let label = if sending { "Requesting…" } else { "Request" };
                                let b = fill_button(ui, label, None, play_colors(), 42.0, 0.0, 0.0);
                                if b.clicked() && !sending {
                                    st.send_error = None;
                                    let (c, it) = (client.clone(), item.clone());
                                    st.sending =
                                        Some(Task::spawn(&ctx, move || c.request_title(&it)));
                                }
                            }
                        }
                    }
                }
            });
            if let Some(e) = &st.send_error {
                ui.add_space(8.0);
                ui.label(RichText::new(format!("Couldn't send the request: {e}")).color(ERROR()));
            }
        });

        // below: cast, episodes and similar titles; the details beside them
        ui.add_space(28.0);
        let narrow = inner < NARROW;
        let gap = 48.0;
        let lw = if narrow { inner } else { inner * 0.64 };
        let rw = if narrow { inner } else { inner - lw - gap };
        let sp = ui.spacing().item_spacing.x;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(m);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.x = sp;
                ui.set_width(lw);
                if let Some(c) = self.remote_main(ui, lib, item, images, lw) {
                    click = Some(c);
                }
                if narrow {
                    ui.add_space(12.0);
                    remote_side(ui, item, self.discover.busy());
                }
            });
            if !narrow {
                ui.add_space(gap);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.x = sp;
                    ui.set_width(rw);
                    remote_side(ui, item, self.discover.busy());
                });
            }
        });
        click
    }

    /// Cast, a series' episodes, and titles like it that are on the server.
    fn remote_main(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        item: &Item,
        images: &mut Images,
        w: f32,
    ) -> Option<Click> {
        let mut click = None;
        if !item.cast.is_empty() {
            caps(ui, "Cast", TEAL());
            ui.add_space(4.0);
            ui.label(bold("Top billed", 24.0));
            ui.add_space(16.0);
            let n = item.cast.len().min(15);
            if let Some(c) = cast_grid(ui, &item.cast[..n], images, 150.0, 50.0) {
                click = Some(c);
            }
            ui.add_space(24.0);
        }
        if item.kind == Kind::Series && !item.seasons.is_empty() {
            let seasons = &item.seasons;
            let st = &mut self.discover;
            if st.season.0 != item.id {
                // the first real season, not the specials
                let first = seasons.iter().position(|s| s.number > 0).unwrap_or(0);
                st.season = (item.id.clone(), first);
            }
            let eps: usize = seasons.iter().map(|s| s.episodes.len()).sum();
            caps(ui, "Episodes", TEAL());
            ui.add_space(4.0);
            ui.label(bold(
                format!(
                    "{} season{} · {eps} episodes",
                    seasons.iter().filter(|s| s.number > 0).count(),
                    if seasons.iter().filter(|s| s.number > 0).count() == 1 {
                        ""
                    } else {
                        "s"
                    }
                ),
                24.0,
            ));
            ui.add_space(12.0);
            let labels: Vec<String> = seasons.iter().map(|s| s.label()).collect();
            let tabs: Vec<(usize, &str, Option<usize>)> = labels
                .iter()
                .enumerate()
                .map(|(i, l)| (i, l.as_str(), Some(seasons[i].episodes.len())))
                .collect();
            egui::ScrollArea::horizontal()
                .id_salt("remote_seasons")
                .show(ui, |ui| segmented(ui, &mut st.season.1, &tabs));
            ui.add_space(12.0);
            let season = &seasons[st.season.1.min(seasons.len() - 1)];
            let mut eps: Vec<&Episode> = season.episodes.iter().collect();
            eps.sort_by_key(|e| e.number);
            for e in eps {
                episode_row(ui, season.number, e, images, w);
                ui.add_space(8.0);
            }
            ui.add_space(16.0);
        }
        let more = similar(lib, item, 10, &self.shuffle);
        if !more.is_empty() {
            caps(ui, "On the server", TEAL());
            ui.add_space(4.0);
            ui.label(bold("Like this, ready to watch", 24.0));
            ui.add_space(16.0);
            let cols = ((w + GAP) / (170.0 + GAP)).floor().clamp(1.0, 5.0);
            let tw = (w - GAP * (cols - 1.0)) / cols;
            for row in more.chunks(cols as usize) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = GAP;
                    for i in row {
                        let year = i.year.map(|y| y.to_string());
                        if poster_tile(ui, i, year.as_deref(), tw, self.watched(i), images) {
                            click = Some(Click::Open(i.id.clone()));
                        }
                    }
                });
                ui.add_space(12.0);
            }
        }
        click
    }
}

/// One episode of a series that isn't on the server: still, title, air date, overview.
fn episode_row(ui: &mut Ui, season: u32, e: &Episode, images: &mut Images, w: f32) {
    egui::Frame::new()
        .stroke(Stroke::new(1.0f32, LINE()))
        .corner_radius(8)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(w - 22.0);
            ui.horizontal_top(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(160.0, 90.0), Sense::hover());
                match e.still.as_ref().and_then(|u| images.get(u)) {
                    Some(t) => image_cover(ui.painter(), r, &t, 4, egui::Color32::WHITE),
                    None => placeholder(ui.painter(), r, &e.title, 4),
                }
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    let title = if e.title.is_empty() {
                        format!("Episode {}", e.number)
                    } else {
                        e.title.clone()
                    };
                    ui.label(bold(title, 15.0));
                    let mut meta = vec![format!("S{season:02}E{:02}", e.number)];
                    if let Some(a) = &e.aired {
                        meta.push(date_long(a));
                    }
                    ui.label(mono(meta.join("   ·   "), 10.5).color(MUTED()));
                    if !e.overview.is_empty() {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&e.overview)
                                    .size(13.0)
                                    .color(TEXT().gamma_multiply(0.8)),
                            )
                            .wrap(),
                        );
                    }
                });
            });
        });
}

/// The details panel: who made it, when, where, and links out.
fn remote_side(ui: &mut Ui, item: &Item, loading: bool) {
    let extra = item.addon.clone().unwrap_or_default();
    let people = |v: &[String]| (!v.is_empty()).then(|| v.join(", "));
    let candidates = [
        (
            if item.kind == Kind::Series {
                "Created by"
            } else {
                "Director"
            },
            if item.kind == Kind::Series {
                people(&item.creators).or(item.director.clone())
            } else {
                item.director.clone()
            },
        ),
        (
            "Writers",
            (item.kind == Kind::Movie)
                .then(|| people(&extra.writers))
                .flatten(),
        ),
        ("Released", item.released.as_deref().map(date_long)),
        (
            "Years",
            match (item.kind, item.year, item.end_year) {
                (Kind::Series, Some(a), Some(b)) if a != b => Some(format!("{a}–{b}")),
                (Kind::Series, Some(a), _) => Some(format!("{a}–")),
                _ => None,
            },
        ),
        (
            if item.kind == Kind::Series {
                "Episode length"
            } else {
                "Runtime"
            },
            item.duration.map(fmt_runtime),
        ),
        ("Country", extra.country.clone()),
        ("Genres", people(&item.genres)),
        (
            "Rating",
            item.score.map(|s| {
                format!(
                    "{s:.1} {}",
                    item.score_source.as_deref().unwrap_or_default()
                )
                .trim()
                .to_owned()
            }),
        ),
        ("Awards", extra.awards.clone()),
    ];
    let rows: Vec<(&str, String)> = candidates
        .into_iter()
        .filter_map(|(k, v)| Some((k, v?)))
        .collect();
    panel(ui, |ui| {
        caps(ui, "Details", TEAL());
        ui.add_space(14.0);
        if rows.is_empty() {
            ui.label(RichText::new("No details for this title.").color(MUTED()));
        }
        info_rows(ui, &rows, 110.0);
        ui.horizontal_wrapped(|ui| {
            if let Some(t) = &extra.trailer {
                if crate::ui::ext_link(ui, "Watch the trailer") {
                    chrome::open_url(&format!("https://www.youtube.com/watch?v={t}"));
                }
                ui.add_space(12.0);
            }
            if let Some(id) = &item.imdb_id {
                if crate::ui::ext_link(ui, "IMDb") {
                    chrome::open_url(&format!("https://www.imdb.com/title/{id}/"));
                }
            }
        });
        ui.add_space(8.0);
        let from = if loading {
            "Loading more details…".to_owned()
        } else if extra.source.is_empty() {
            "From a Stremio addon".to_owned()
        } else {
            format!("Details from {}", extra.source)
        };
        ui.label(mono(from, 10.0).color(MUTED()));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, kind: Kind, title: &str, year: Option<u32>, imdb: Option<&str>) -> Item {
        Item {
            id: id.into(),
            kind,
            title: title.into(),
            year,
            imdb_id: imdb.map(Into::into),
            ..Default::default()
        }
    }

    #[test]
    fn finds_ours_by_imdb_then_title() {
        let lib = Library {
            movies: vec![
                item("1", Kind::Movie, "Buddy", Some(2026), None),
                item("2", Kind::Movie, "Obsession", Some(2025), Some("tt111")),
                item("3", Kind::Movie, "Obsession", Some(1976), None),
            ],
            series: vec![item("4", Kind::Series, "The Backrooms", None, None)],
        };
        let ours = Ours::new(&lib);
        let find = |k, t, y, imdb| ours.find(&item("x", k, t, y, imdb));
        assert_eq!(
            find(Kind::Movie, "Obsession", Some(2026), Some("tt111")),
            Some("2")
        );
        assert_eq!(
            find(Kind::Movie, "Buddy", Some(2026), Some("tt999")),
            Some("1")
        );
        assert_eq!(find(Kind::Movie, "buddy!", Some(2025), None), Some("1"));
        assert_eq!(find(Kind::Movie, "Obsession", Some(1977), None), Some("3"));
        assert_eq!(find(Kind::Movie, "Buddy", Some(2010), None), None); // another film
        assert_eq!(find(Kind::Series, "Backrooms", Some(2026), None), Some("4"));
        assert_eq!(find(Kind::Movie, "Backrooms", Some(2026), None), None); // not a movie
    }
}
