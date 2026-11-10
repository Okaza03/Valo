//! What the admin's Stremio addons add to browsing: catalog rows on the home
//! page, details for titles that aren't on the server (with a Request
//! button), and details our own titles are missing (filled in, never
//! replaced).
use crate::addons;
use crate::addons::imdb_id;
use crate::addons::kind_name;
use crate::addons::split_remote;
use crate::addons::{title_key, Row};
use crate::api::Addon;
use crate::api::Client;
use crate::api::Item;
use crate::api::Kind;
use crate::api::Library;
use crate::api::MediaRequest;
use crate::api::NoAdminApi;
use crate::filmography;
use crate::filmography::Filmography;
use crate::util::Task;
use crate::util::Waker;
use anyhow::Result;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::TryRecvError;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

/// Metadata lookups per batch.
const BATCH: usize = 24;

/// Search the addons once typing pauses for this long.
const TYPING: Duration = Duration::from_millis(300);

#[derive(Default)]
pub(crate) struct State {
    load: Option<Task<Result<addons::Catalogs>>>,
    /// The addon list has come in (possibly empty).
    loaded: bool,
    addons: Vec<Addon>,
    rows: Vec<Row>,
    /// Each row as item ids: ours where the IMDb id matches, else the addon's.
    pub shown: Vec<Shown>,
    /// The Streaming page: which catalog, and movies, series or both.
    pub(crate) service: Option<String>,
    pub(crate) service_kind: Option<Kind>,
    /// Addon titles, by [`addons::remote_id`].
    pub remote: HashMap<String, Item>,
    /// From metadata addons, by "movie:tt1375666" (None: none of them has it).
    metas: HashMap<String, Option<Item>>,
    want: Vec<(String, String)>,
    asked: HashSet<String>,
    fetching: Option<Task<addons::Metas>>,
    /// The user's own requests; Err: the server doesn't take requests.
    pub(crate) requests: Option<std::result::Result<Vec<MediaRequest>, String>>,
    requests_task: Option<Task<Result<Vec<MediaRequest>>>>,
    pub(crate) sending: Option<Task<Result<MediaRequest>>>,
    pub(crate) send_error: Option<String>,
    /// Filmographies by person name.
    films: HashMap<String, Films>,
    /// Cast photos from Wikidata, by name (None: it has none).
    photos: HashMap<String, Option<String>>,
    photos_task: Option<Task<PhotoAnswer>>,
    photos_want: Vec<String>,
    pub(crate) film_filter: FilmFilter,
    /// The season shown on an addon series' page: (item id, season index).
    pub(crate) season: (String, usize),
    searchable: Vec<addons::Searchable>,
    /// What's typed in the search box, and since when.
    typed: (String, Option<Instant>),
    searching: Option<Search>,
    /// Addon titles for a search, as ids; ours left out.
    results: Option<(String, Vec<String>)>,
}

impl State {
    pub fn reload(&mut self, wake: impl Into<Waker>, client: &Client) {
        let c = client.clone();
        self.load = Some(Task::spawn(wake, move || addons::catalogs(&c)));
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
    pub fn poll(
        &mut self,
        wake: impl Into<Waker>,
        client: &Client,
        lib: &mut Option<Arc<Library>>,
    ) {
        let wake: Waker = wake.into();
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
            self.fetching = Some(Task::spawn(wake.clone(), move || addons::metas(&a, &batch)));
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
            self.photos_task = Some(Task::spawn(wake.clone(), move || {
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
                    self.requests_task = Some(Task::spawn(wake.clone(), move || c.my_requests()));
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
    pub fn search(&mut self, wake: impl Into<Waker>, q: &str) -> (Vec<String>, bool) {
        let wake: Waker = wake.into();
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
                    let w = wake.clone();
                    let rx = addons::search(
                        self.addons.clone(),
                        self.searchable.clone(),
                        q.to_owned(),
                        move || w.wake(),
                    );
                    self.searching = Some(Search {
                        q: q.to_owned(),
                        rx,
                        parts: vec![Vec::new(); self.searchable.len()],
                        left: self.searchable.len(),
                    });
                }
                Some(t) => wake.wake_after(TYPING.saturating_sub(t.elapsed())),
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

    /// The request that stands for this title: anyone's that isn't denied
    /// (nobody asks twice), else the user's own denied one. Someone else's
    /// denied request doesn't count: the user can ask again.
    pub(crate) fn request_for(&self, item: &Item, me: Option<&str>) -> Option<&MediaRequest> {
        let Some(Ok(list)) = &self.requests else {
            return None;
        };
        let mine = |r: &MediaRequest| {
            r.requested_by
                .as_deref()
                .is_some_and(|by| me.is_some_and(|me| by.eq_ignore_ascii_case(me)))
        };
        let matches = list.iter().rev().filter(|r| asks_for(r, item));
        let mut denied = None;
        for r in matches {
            if r.status != "denied" {
                return Some(r);
            }
            if denied.is_none() && mine(r) {
                denied = Some(r);
            }
        }
        denied
    }
}

/// This request is for this title. Without IMDb ids the years have to
/// agree, or a request for one film would stand for its remake too.
fn asks_for(r: &MediaRequest, item: &Item) -> bool {
    match (&r.imdb_id, &item.imdb_id) {
        (Some(a), Some(b)) => a == b,
        _ => r.title.to_lowercase() == item.title.to_lowercase() && r.year == item.year,
    }
}

/// A title's year and our id.
type YearId<'a> = (Option<u32>, &'a str);

/// A catalog, its titles as ids.
pub(crate) struct Shown {
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
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub(crate) enum FilmFilter {
    #[default]
    All,
    OnServer,
    NotOnServer,
}

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
    by_imdb: HashMap<(Kind, &'a str), YearId<'a>>,
    /// By [`title_key`], with whether we know the title's IMDb id.
    by_title: HashMap<(Kind, String), Vec<(YearId<'a>, bool)>>,
}

impl<'a> Ours<'a> {
    fn new(lib: &'a Library) -> Self {
        let mut o = Self {
            by_imdb: HashMap::new(),
            by_title: HashMap::new(),
        };
        for i in lib.all() {
            if let Some(imdb) = i.imdb_id.as_deref() {
                o.by_imdb
                    .insert((i.kind, imdb.trim()), (i.year, i.id.as_str()));
            }
            o.by_title
                .entry((i.kind, title_key(&i.title)))
                .or_default()
                .push(((i.year, i.id.as_str()), i.imdb_id.is_some()));
        }
        o
    }

    /// By IMDb id; else by title and year (a year apart is fine: release
    /// dates differ by country), for titles the server has no IMDb id for.
    /// Years further apart are another film, even with the same IMDb id:
    /// the server may have looked up the wrong one of a remake.
    fn find(&self, it: &Item) -> Option<&'a str> {
        let gap = |y: Option<u32>| y.zip(it.year).map(|(a, b)| a.abs_diff(b));
        let imdb = it.imdb_id.as_deref();
        if let Some((y, id)) = imdb.and_then(|i| self.by_imdb.get(&(it.kind, i))) {
            if gap(*y).is_none_or(|g| g <= 1) {
                return Some(id);
            }
        }
        let same = self.by_title.get(&(it.kind, title_key(&it.title)))?;
        // the closest year wins, for remakes with the same name
        same.iter()
            .filter(|((y, _), known)| match gap(*y) {
                Some(g) => g <= 1,
                // no year to go by: different IMDb ids are different films
                None => !(*known && imdb.is_some()),
            })
            .min_by_key(|((y, _), _)| gap(*y).unwrap_or(2))
            .map(|((_, id), _)| *id)
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

    /// The streaming services listing this title.
    pub fn services(&self, id: &str) -> Vec<&str> {
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
        out
    }

    /// A person's filmography: loads it the first time it's asked for.
    pub(crate) fn films(
        &mut self,
        wake: impl Into<Waker>,
        name: &str,
    ) -> Option<&std::result::Result<Filmography, String>> {
        let key = name.to_lowercase();
        let wake: Waker = wake.into();
        let entry = self.films.entry(key).or_insert_with(|| {
            let n = name.to_owned();
            Films::Loading(Task::spawn(wake, move || filmography::lookup(&n)))
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

    /// The filmography's credits as ids, and whether each is on the
    /// server; titles that aren't become addon titles to request.
    pub(crate) fn credits(&mut self, lib: &Library, f: &Filmography) -> Vec<(String, bool)> {
        let ours = Ours::new(lib);
        f.credits
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
                        self.remote.entry(id.clone()).or_insert(it);
                        (id, false)
                    }
                }
            })
            .collect()
    }

    /// An addon title's page opened: ask for its details, its cast's
    /// photos, and (signed in) the user's requests.
    pub(crate) fn want_remote(&mut self, wake: impl Into<Waker>, client: &Client, item: &Item) {
        if let Some((k, id)) = split_remote(&item.id) {
            self.want_now(k, id);
        }
        self.want_photos(item);
        if self.requests.is_none() && self.requests_task.is_none() && client.token.is_some() {
            let c = client.clone();
            self.requests_task = Some(Task::spawn(wake, move || c.my_requests()));
        }
    }

    /// Ask the server for `item`.
    pub(crate) fn send_request(&mut self, wake: impl Into<Waker>, client: &Client, item: &Item) {
        if self.sending.is_none() {
            self.send_error = None;
            let (c, it) = (client.clone(), item.clone());
            self.sending = Some(Task::spawn(wake, move || c.request_title(&it)));
        }
    }

    /// A person's filmography if it's been loaded (see [`films`](Self::films)).
    pub(crate) fn films_loaded(
        &self,
        name: &str,
    ) -> Option<&std::result::Result<Filmography, String>> {
        match self.films.get(&name.to_lowercase()) {
            Some(Films::Done(r)) => Some(r),
            _ => None,
        }
    }

    /// Loading something the pages wait on.
    pub(crate) fn working(&self) -> bool {
        self.busy()
            || self.searching.is_some()
            || self.photos_task.is_some()
            || self.requests_task.is_some()
            || self.sending.is_some()
            || self.films.values().any(|f| matches!(f, Films::Loading(_)))
            || self.typed.1.is_some_and(|t| t.elapsed() < TYPING * 2)
    }

    /// The streaming page's catalogs for `source`: (kind, ids) per catalog.
    pub(crate) fn of_source(&self, source: &str) -> Vec<(Kind, Vec<String>)> {
        self.shown
            .iter()
            .filter(|s| s.source == source)
            .map(|s| (s.kind, s.ids.clone()))
            .collect()
    }

    /// Their photo from Wikidata, once loaded.
    pub fn person_photo(&self, name: &str) -> Option<String> {
        match self.films.get(&name.to_lowercase()) {
            Some(Films::Done(Ok(f))) => f.photo.clone(),
            _ => None,
        }
    }
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

    #[test]
    fn remakes_are_other_films() {
        let lib = Library {
            movies: vec![
                // the server looked up the 1987 film for its 2025 remake
                item(
                    "1",
                    Kind::Movie,
                    "The Running Man",
                    Some(2025),
                    Some("tt0093894"),
                ),
                item("2", Kind::Movie, "Total Recall", None, Some("tt0100802")),
            ],
            series: Vec::new(),
        };
        let ours = Ours::new(&lib);
        let find = |t, y, imdb| ours.find(&item("x", Kind::Movie, t, y, imdb));
        assert_eq!(find("The Running Man", Some(1987), Some("tt0093894")), None);
        assert_eq!(find("The Running Man", Some(1963), Some("tt0057463")), None);
        assert_eq!(
            find("The Running Man", Some(2025), Some("tt14107334")),
            Some("1")
        );
        assert_eq!(find("Total Recall", Some(2012), Some("tt1386703")), None);
        assert_eq!(
            find("Total Recall", Some(1990), Some("tt0100802")),
            Some("2")
        );
    }

    #[test]
    fn requests_are_for_one_film() {
        let film = |y, imdb| item("x", Kind::Movie, "The Running Man", y, imdb);
        let asked = |year, imdb_id: Option<&str>| MediaRequest {
            title: "The running man".into(),
            year,
            imdb_id: imdb_id.map(Into::into),
            ..Default::default()
        };
        assert!(asks_for(&asked(Some(2025), None), &film(Some(2025), None)));
        assert!(asks_for(
            &asked(None, Some("tt14107334")),
            &film(Some(2025), Some("tt14107334"))
        ));
        assert!(!asks_for(
            &asked(Some(2025), Some("tt14107334")),
            &film(Some(1987), Some("tt0093894"))
        ));
        assert!(!asks_for(&asked(Some(2025), None), &film(Some(1987), None)));
        assert!(!asks_for(
            &asked(None, None),
            &film(Some(1987), Some("tt0093894"))
        ));
    }
}
