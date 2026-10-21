//! Talking to the media server.
//!
//! Two kinds of servers are supported:
//! * **API servers** answer `GET /api/info` with JSON and serve the catalog at
//!   `GET /api/library`, optionally behind `POST /api/login` (see docs/api/).
//! * **Plain file servers** (nginx/Apache/Caddy directory listings): the
//!   listing is crawled and movies/series are guessed from folder and file names.
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Read;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;

use crate::util::percent_decode;

pub mod check;

pub const VIDEO_EXTS: &[&str] = &[
    "mp4", "mkv", "webm", "avi", "mov", "m4v", "flv", "wmv", "mpg", "mpeg", "ogv", "ts", "3gp",
];
const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp"];

// ------------------------------------------------------------------ model

/// Lists may come as `null` from servers; treat that like a missing list.
fn null_as_empty<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(d)?.unwrap_or_default())
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub enum Kind {
    #[default]
    Movie,
    Series,
}

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Item {
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    pub title: String,
    pub year: Option<u32>,
    pub overview: String,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    /// Landscape artwork (16:9, usually with the title on it) for wide
    /// cards such as Continue watching; the backdrop stands in without it.
    pub thumb: Option<String>,
    #[serde(deserialize_with = "null_as_empty")]
    pub genres: Vec<String>,
    /// Age rating such as "PG-13".
    pub rating: Option<String>,
    /// Seconds.
    pub duration: Option<f64>,
    /// Unix time the item was added to the library.
    pub added: Option<i64>,
    /// Movies only.
    pub stream: Option<String>,
    /// Movies only: subtitle files to pick from in the player.
    #[serde(deserialize_with = "null_as_empty")]
    pub subtitles: Vec<SubtitleTrack>,
    /// Series only.
    #[serde(deserialize_with = "null_as_empty")]
    pub seasons: Vec<Season>,
    #[serde(skip)]
    pub kind: Kind,

    // Optional extras for the details page; sections without data are hidden.
    /// Title artwork (transparent PNG) shown instead of the title text.
    pub logo: Option<String>,
    /// Movies.
    pub director: Option<String>,
    /// Series.
    #[serde(deserialize_with = "null_as_empty")]
    pub creators: Vec<String>,
    #[serde(deserialize_with = "null_as_empty")]
    pub cast: Vec<CastMember>,
    /// Release date, "YYYY-MM-DD".
    pub released: Option<String>,
    /// Series: last year it aired ("2021–2024").
    pub end_year: Option<u32>,
    /// Series: "Netflix".
    pub network: Option<String>,
    /// Series: "Ended", "Returning Series", …
    pub status: Option<String>,
    /// US dollars.
    pub budget: Option<u64>,
    /// Box office, US dollars.
    pub revenue: Option<u64>,
    /// Audience score out of 10, e.g. 7.2 …
    pub score: Option<f32>,
    /// … and where it's from, e.g. "TMDB".
    pub score_source: Option<String>,
    /// Movies: about the video file.
    pub file: Option<FileInfo>,
    /// Critics' quotes.
    #[serde(deserialize_with = "null_as_empty")]
    pub reviews: Vec<Review>,
    /// "tt1375666"
    pub imdb_id: Option<String>,
    /// The collection it belongs to ("Batman"): A to Z, its titles sit
    /// together under that name, in `collection_index` order (else oldest first).
    #[serde(deserialize_with = "name_or_object")]
    pub collection: Option<String>,
    /// Its place in the collection, 0 first.
    pub collection_index: Option<u32>,

    // Admin lists and records only.
    /// Movies: it has at least one subtitle.
    pub has_subtitles: Option<bool>,
    /// Series: episodes without a subtitle.
    #[serde(alias = "missing_subtitle_count")]
    pub missing_subtitles: Option<u32>,
    /// Fields an admin edited, which syncs leave alone.
    #[serde(deserialize_with = "null_as_empty")]
    pub locked: Vec<String>,
    pub tmdb_id: Option<Value>,
}

/// "2021-11-06 00:00:00" (a database timestamp) -> "2021-11-06".
fn trim_date(d: &mut Option<String>) {
    if let Some(s) = d.as_mut() {
        let b = s.as_bytes();
        if s.len() > 10 && b[4] == b'-' && b[7] == b'-' && s.is_char_boundary(10) {
            s.truncate(10);
        }
    }
}

/// `"Batman"` or `{"name": "Batman", …}`; anything else counts as missing.
fn name_or_object<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let name = match Value::deserialize(d)? {
        Value::String(s) => s,
        Value::Object(o) => match o.get("name") {
            Some(Value::String(s)) => s.clone(),
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    let name = name.trim();
    Ok((!name.is_empty()).then(|| name.to_owned()))
}

/// An id that may come as a number or a string, as a string.
fn id_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => s,
        Value::Null => String::new(),
        v => v.to_string(),
    })
}

fn id_strings<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let v = Option::<Vec<Value>>::deserialize(d)?.unwrap_or_default();
    Ok(v.into_iter()
        .map(|v| match v {
            Value::String(s) => s,
            v => v.to_string(),
        })
        .collect())
}

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct CastMember {
    pub name: String,
    pub character: Option<String>,
    pub photo: Option<String>,
}

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct FileInfo {
    /// "1080p"
    pub quality: Option<String>,
    /// "mkv"
    pub container: Option<String>,
    /// "h264"
    pub video_codec: Option<String>,
    /// "aac"
    pub audio_codec: Option<String>,
    pub audio_channels: Option<u32>,
    /// Bits per second.
    pub bitrate: Option<u64>,
    /// Bytes.
    pub size: Option<u64>,
    /// File name.
    pub name: Option<String>,
    /// Where it lives on the server, e.g. "/movies/28 Days Later".
    pub path: Option<String>,
}

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Review {
    pub quote: String,
    /// "Reverse Shot"
    pub source: Option<String>,
    /// "4.5 / 5"
    pub score: Option<String>,
}

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Season {
    pub number: u32,
    pub title: Option<String>,
    #[serde(deserialize_with = "null_as_empty")]
    pub episodes: Vec<Episode>,
}

impl Season {
    pub fn label(&self) -> String {
        self.title.clone().unwrap_or_else(|| {
            if self.number == 0 {
                "Specials".into()
            } else {
                format!("Season {}", self.number)
            }
        })
    }
}

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Episode {
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    pub number: u32,
    pub title: String,
    pub overview: String,
    pub still: Option<String>,
    pub duration: Option<f64>,
    pub stream: String,
    /// Air date, "YYYY-MM-DD".
    pub aired: Option<String>,
    /// Audience score out of 10.
    pub score: Option<f32>,
    pub file: Option<FileInfo>,
    #[serde(deserialize_with = "null_as_empty")]
    pub subtitles: Vec<SubtitleTrack>,
}

/// A subtitle file for a movie or episode (`"subtitles"` in the library).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SubtitleTrack {
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    /// "en", "nb", …; "und" when unknown.
    pub language: String,
    /// "English (SDH)"
    pub label: Option<String>,
    /// "srt", "vtt", "ass" or "ssa"; taken from the URL when missing.
    pub format: Option<String>,
    /// Only the parts in another language (signs, foreign dialogue).
    pub forced: bool,
    /// With sound descriptions, for the deaf and hard of hearing.
    pub sdh: bool,
    pub url: String,
    /// From a Stremio addon, not our server: fetched without our credentials.
    #[serde(skip)]
    pub external: bool,
}

impl SubtitleTrack {
    pub fn label(&self) -> String {
        match &self.label {
            Some(l) if !l.trim().is_empty() => l.trim().to_owned(),
            _ if self.language.is_empty() || self.language == "und" => "Unknown".into(),
            _ => self.language.clone(),
        }
    }
}

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Library {
    #[serde(deserialize_with = "null_as_empty")]
    pub movies: Vec<Item>,
    #[serde(deserialize_with = "null_as_empty")]
    pub series: Vec<Item>,
}

/// Where an item goes A to Z: under its collection's name, in the
/// collection's order (else oldest first), otherwise under its title.
pub fn title_key(i: &Item) -> (String, u32, u32, String) {
    let title = i.title.to_lowercase();
    match &i.collection {
        Some(c) => (
            c.to_lowercase(),
            i.collection_index.unwrap_or(u32::MAX),
            i.year.unwrap_or(u32::MAX),
            title,
        ),
        None => (title, 0, 0, String::new()),
    }
}

impl Item {
    /// Absolute URLs and tagged ids, as the rest of the app expects them.
    fn finish(&mut self, base: &str, kind: Kind) {
        let fix = |u: &mut Option<String>| {
            if let Some(s) = u.as_mut() {
                *s = join(base, s);
            }
        };
        self.kind = kind;
        trim_date(&mut self.released);
        fix(&mut self.poster);
        fix(&mut self.backdrop);
        fix(&mut self.thumb);
        fix(&mut self.logo);
        for c in &mut self.cast {
            fix(&mut c.photo);
        }
        fix(&mut self.stream);
        self.subtitles.retain(|t| !t.url.is_empty());
        for t in &mut self.subtitles {
            t.url = join(base, &t.url);
        }
        if self.id.is_empty() {
            self.id = self.stream.clone().unwrap_or_else(|| self.title.clone());
        }
        // servers often number movies, series and episodes separately
        let tag = if kind == Kind::Movie {
            "movie"
        } else {
            "series"
        };
        self.id = format!("{tag}:{}", self.id);
        self.seasons.sort_by_key(|s| s.number);
        for s in &mut self.seasons {
            s.episodes.sort_by_key(|e| e.number);
            for e in &mut s.episodes {
                trim_date(&mut e.aired);
                fix(&mut e.still);
                e.stream = join(base, &e.stream);
                e.subtitles.retain(|t| !t.url.is_empty());
                for t in &mut e.subtitles {
                    t.url = join(base, &t.url);
                }
                if e.id.is_empty() {
                    e.id = e.stream.clone();
                }
                e.id = format!("episode:{}", e.id);
            }
        }
    }

    /// The id the server knows it by (ours carries a "movie:"/"series:" tag).
    pub fn server_id(&self) -> &str {
        self.id.split_once(':').map_or(&self.id, |(_, id)| id)
    }
}

impl Library {
    pub fn all(&self) -> impl Iterator<Item = &Item> {
        self.movies.iter().chain(&self.series)
    }

    pub fn find(&self, id: &str) -> Option<&Item> {
        self.all().find(|i| i.id == id)
    }

    /// Series, season index and episode index for an episode id.
    pub fn find_episode(&self, ep_id: &str) -> Option<(&Item, usize, usize)> {
        self.series.iter().find_map(|s| {
            s.seasons.iter().enumerate().find_map(|(si, season)| {
                let ei = season.episodes.iter().position(|e| e.id == ep_id)?;
                Some((s, si, ei))
            })
        })
    }

    /// Genres ordered by how many titles have them.
    pub fn genres(&self) -> Vec<String> {
        let mut count: HashMap<&str, usize> = HashMap::new();
        for g in self.all().flat_map(|i| &i.genres) {
            *count.entry(g.as_str()).or_default() += 1;
        }
        let mut v: Vec<_> = count.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        v.into_iter().map(|(g, _)| g.to_owned()).collect()
    }

    fn finish(&mut self, base: &str) {
        for it in &mut self.movies {
            it.finish(base, Kind::Movie);
        }
        for it in &mut self.series {
            it.finish(base, Kind::Series);
        }
        self.movies.retain(|m| m.stream.is_some());
        self.series
            .retain(|s| s.seasons.iter().any(|x| !x.episodes.is_empty()));
        self.movies.sort_by_cached_key(title_key);
        self.series.sort_by_cached_key(title_key);
    }
}

/// A `null` where a number or flag belongs counts as missing ("duration": null
/// when the server doesn't know it yet).
fn null_as_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// One title's watch state as `/api/progress` sends and takes it.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Watched {
    /// "movie" or "episode"
    #[serde(rename = "type")]
    pub kind: String,
    /// The library's id for it; a number or a string.
    pub id: Value,
    #[serde(deserialize_with = "null_as_default")]
    pub position: f64,
    #[serde(deserialize_with = "null_as_default")]
    pub duration: f64,
    /// Watched to the end (the server decides where that is).
    #[serde(deserialize_with = "null_as_default")]
    pub completed: bool,
    /// Unix time of the last change.
    #[serde(deserialize_with = "null_as_default")]
    pub updated: i64,
}

impl Watched {
    /// The id the rest of the app uses ("movie:42"; see [`Library::finish`]).
    pub fn key(&self) -> Option<String> {
        let id = match &self.id {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => return None,
        };
        matches!(self.kind.as_str(), "movie" | "episode").then(|| format!("{}:{id}", self.kind))
    }

    /// The reverse of [`Watched::key`]. Ids that look like numbers go out as numbers.
    pub fn from_key(key: &str, position: f64, duration: f64) -> Option<Self> {
        let (kind, id) = key.split_once(':')?;
        if !matches!(kind, "movie" | "episode") {
            return None;
        }
        let id = match id.parse::<u64>() {
            Ok(n) if n.to_string() == id => Value::from(n),
            _ => Value::from(id),
        };
        Some(Self {
            kind: kind.to_owned(),
            id,
            position,
            duration,
            ..Default::default()
        })
    }
}

// ----------------------------------------------------------------- client

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Api,
    Directory,
}

#[derive(Clone, Debug)]
pub struct ServerInfo {
    pub name: String,
    pub auth: bool,
    pub mode: Mode,
}

#[derive(Deserialize)]
struct InfoJson {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    auth: bool,
}

#[derive(Deserialize)]
pub struct Session {
    pub token: String,
    #[serde(default)]
    pub name: Option<String>,
    /// The server lets this user open its /admin page.
    #[serde(default)]
    pub admin: bool,
}

/// The server rejected our token; the user has to sign in again.
#[derive(Debug)]
pub struct Unauthorized;

impl std::fmt::Display for Unauthorized {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("session expired, please sign in again")
    }
}
impl std::error::Error for Unauthorized {}

/// `GET /api/me`.
#[derive(Deserialize)]
pub struct Me {
    #[serde(default)]
    pub admin: bool,
    /// An admin asked them to pick a new password.
    #[serde(default)]
    pub must_change_password: bool,
}

/// A user as the admin API lists them.
#[derive(Clone, Debug, Deserialize)]
pub struct AdminUser {
    pub name: String,
    /// Shown instead of the username, when it differs.
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub admin: bool,
    /// Asked to pick a new password the next time they open the app.
    #[serde(default)]
    pub must_change_password: bool,
    /// How many devices are signed in.
    #[serde(default)]
    pub sessions: Option<u32>,
    /// Unix seconds of their last saved watch progress.
    #[serde(default)]
    pub last_active: Option<i64>,
}

/// What `POST /api/admin/rescan` found.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Rescan {
    pub movies: Option<usize>,
    pub series: Option<usize>,
    pub episodes: Option<usize>,
    /// The scan carries on in the background, so new titles show up later.
    pub running: bool,
}

/// `GET /api/admin/status`: how the server and what it talks to are doing.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct AdminStatus {
    /// The server's own version, e.g. "1.4.0".
    pub version: Option<String>,
    /// Seconds since the server started.
    pub uptime: Option<u64>,
    /// Outside services: metadata source, Discord bot, database, …
    #[serde(deserialize_with = "null_as_empty")]
    pub services: Vec<Service>,
    /// The server's own library checks. When it sends any, they replace
    /// the app's, so the page shows what the website shows.
    #[serde(deserialize_with = "null_as_empty")]
    pub health: Vec<HealthCheck>,
    /// The server's own numbers; they win over counting the library.
    pub library: Option<LibraryStats>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct LibraryStats {
    pub movies: Option<u64>,
    pub series: Option<u64>,
    pub episodes: Option<u64>,
    /// Bytes.
    pub size: Option<u64>,
    pub users: Option<u64>,
    /// Any other numbers the server counts ("mp4", "series_complete"),
    /// shown as more tiles.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Service {
    pub name: String,
    pub ok: bool,
    /// "online, 3 guilds", "no API key"
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct HealthCheck {
    /// "Files without a database entry"
    pub label: String,
    pub count: usize,
    /// Examples (file paths, titles), shown when the row is opened.
    #[serde(deserialize_with = "null_as_empty")]
    pub items: Vec<String>,
    /// The titles it's about, so the app can open them in the editor.
    #[serde(deserialize_with = "null_as_empty")]
    pub titles: Vec<CollectionItem>,
}

/// What a sync found (`POST /api/admin/sync/<what>`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SyncReport {
    #[serde(deserialize_with = "null_as_empty")]
    pub added: Vec<String>,
    #[serde(deserialize_with = "null_as_empty")]
    pub removed: Vec<String>,
    #[serde(deserialize_with = "null_as_empty")]
    pub updated: Vec<String>,
    /// The changes were written (false: a dry run).
    pub applied: bool,
    /// It carries on in the background; follow the log.
    pub running: bool,
    pub message: Option<String>,

    // Other shapes servers answer with, folded into the above by `tidy`.
    #[serde(deserialize_with = "null_as_empty")]
    added_movies: Vec<String>,
    #[serde(deserialize_with = "null_as_empty")]
    added_series: Vec<String>,
    #[serde(deserialize_with = "null_as_empty")]
    removed_movies: Vec<String>,
    #[serde(deserialize_with = "null_as_empty")]
    removed_series: Vec<String>,
    dry_run: Option<bool>,
    started: bool,
}

impl SyncReport {
    /// Fold per-kind lists, `dry_run` and `started` into the documented
    /// fields; `accepted` is a `202` answer (started, not done).
    fn tidy(mut self, accepted: bool) -> Self {
        self.added.append(&mut self.added_movies);
        self.added.append(&mut self.added_series);
        self.removed.append(&mut self.removed_movies);
        self.removed.append(&mut self.removed_series);
        if let Some(dry) = self.dry_run {
            self.applied = !dry;
        }
        if self.started || accepted {
            self.running = true;
            self.applied = true;
        }
        self
    }
}

/// One line of the server's log (`GET /api/admin/logs`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct LogLine {
    pub id: u64,
    /// Unix seconds.
    pub time: i64,
    /// "info", "warning", "error"
    pub level: String,
    #[serde(alias = "text", alias = "line")]
    pub message: String,
}

/// `GET /api/admin/analytics?days=N`
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Analytics {
    pub plays: u64,
    pub hours: f64,
    pub active_users: u32,
    /// Oldest first, one per day of the range.
    #[serde(deserialize_with = "null_as_empty")]
    pub days: Vec<DayStat>,
    /// Most played first.
    #[serde(deserialize_with = "null_as_empty")]
    pub top: Vec<TitleStat>,
    #[serde(deserialize_with = "null_as_empty")]
    pub users: Vec<UserStat>,
    /// Best rated by users, best first.
    #[serde(deserialize_with = "null_as_empty")]
    pub top_rated: Vec<RatedStat>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RatedStat {
    pub title: String,
    /// "movie" or "series"
    pub kind: Option<String>,
    /// Out of 10.
    pub avg_rating: f32,
    pub rating_count: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DayStat {
    /// "2026-10-14"
    pub day: String,
    pub plays: u32,
    pub hours: f64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct TitleStat {
    /// Optional; lets the app open the title.
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    pub title: String,
    /// "movie" or "series"
    pub kind: Option<String>,
    pub plays: u32,
    pub hours: f64,
    /// How many different users played it.
    pub users: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct UserStat {
    pub name: String,
    pub plays: u32,
    pub hours: f64,
    pub movies: Option<u32>,
    pub episodes: Option<u32>,
    pub last_active: Option<i64>,
}

/// `GET /api/admin/analytics/users/<name>?days=N`: one user's watching.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct UserAnalytics {
    pub name: String,
    pub display_name: Option<String>,
    pub admin: bool,
    pub plays: u64,
    pub hours: f64,
    pub movies: Option<u32>,
    pub episodes: Option<u32>,
    pub finished: Option<u32>,
    pub in_progress: Option<u32>,
    pub last_active: Option<i64>,
    /// Oldest first, one per day of the range.
    #[serde(deserialize_with = "null_as_empty")]
    pub days: Vec<DayStat>,
    /// Plays started in each hour of the day, 0 to 23 (server time).
    #[serde(deserialize_with = "null_as_empty")]
    pub hours_of_day: Vec<u32>,
    /// Plays on each weekday, Monday first.
    #[serde(deserialize_with = "null_as_empty")]
    pub weekdays: Vec<u32>,
    /// Most watched first.
    #[serde(deserialize_with = "null_as_empty")]
    pub top: Vec<TitleStat>,
    #[serde(deserialize_with = "null_as_empty")]
    pub genres: Vec<GenreStat>,
    /// Newest first.
    #[serde(deserialize_with = "null_as_empty")]
    pub recent: Vec<RecentPlay>,
    #[serde(deserialize_with = "null_as_empty")]
    pub sessions: Vec<UserSession>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct GenreStat {
    pub name: String,
    pub hours: f64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RecentPlay {
    /// "movie" or "episode"
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    pub title: String,
    /// The episode's series, for episodes; empty otherwise.
    #[serde(deserialize_with = "id_string")]
    pub series_id: String,
    /// "S01E02 · Pilot"
    pub episode: Option<String>,
    pub position: f64,
    pub duration: f64,
    pub completed: bool,
    pub updated: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct UserSession {
    pub device: Option<String>,
    pub ip: Option<String>,
    pub created: Option<i64>,
    pub last_used: Option<i64>,
}

/// A title someone asked for, e.g. through the Discord bot
/// (`GET /api/admin/requests`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MediaRequest {
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    pub title: String,
    pub year: Option<u32>,
    /// "movie" or "series"
    pub kind: Option<String>,
    pub imdb_id: Option<String>,
    pub poster: Option<String>,
    /// Their Discord (or Valo) name.
    pub requested_by: Option<String>,
    /// "discord", "app", …
    pub source: Option<String>,
    /// "pending", "approved", "available" or "denied"
    pub status: String,
    /// Unix seconds.
    pub created: Option<i64>,
    /// What the requester wrote, or the admin's answer.
    pub note: Option<String>,
    /// `false`: the requester couldn't be told about the last answer
    /// (their Discord DMs are closed, …).
    pub notified: Option<bool>,
}

/// A Stremio addon the admin added (`GET /api/addons`): catalogs,
/// metadata and/or subtitles. Never streams.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Addon {
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    /// Its manifest: `https://…/manifest.json`.
    pub url: String,
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    /// "movie", "series"; empty: whatever it says in its manifest.
    #[serde(deserialize_with = "null_as_empty")]
    pub types: Vec<String>,
    /// What it offers. Empty: subtitles (older servers).
    #[serde(deserialize_with = "null_as_empty")]
    pub resources: Vec<AddonResource>,
    /// Lists of titles, shown as rows on the home page.
    #[serde(deserialize_with = "null_as_empty")]
    pub catalogs: Vec<AddonCatalog>,
    /// Unix seconds.
    pub added: Option<i64>,
    pub added_by: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AddonResource {
    /// "catalog", "meta" or "subtitles".
    pub name: String,
    /// Empty: any.
    #[serde(deserialize_with = "null_as_empty")]
    pub types: Vec<String>,
    /// Ids it knows ("tt", "tmdb:"); None: any.
    pub id_prefixes: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AddonCatalog {
    /// "movie" or "series".
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub name: String,
}

impl Addon {
    /// It answers `resource` requests for this type and id.
    pub fn serves(&self, resource: &str, kind: &str, id: &str) -> bool {
        if self.resources.is_empty() {
            return resource == "subtitles"
                && (self.types.is_empty() || self.types.iter().any(|t| t == kind));
        }
        self.resources.iter().any(|r| {
            r.name == resource
                && (r.types.is_empty() || r.types.iter().any(|t| t == kind))
                && r.id_prefixes
                    .as_ref()
                    .is_none_or(|p| p.iter().any(|p| id.starts_with(p.as_str())))
        })
    }

    /// "Catalogs · Metadata · Subtitles"
    pub fn offers(&self) -> String {
        let has = |n: &str| self.resources.iter().any(|r| r.name == n);
        let mut out = Vec::new();
        if has("catalog") || !self.catalogs.is_empty() {
            out.push("Catalogs");
        }
        if has("meta") {
            out.push("Metadata");
        }
        if has("subtitles") || self.resources.is_empty() {
            out.push("Subtitles");
        }
        out.join(" · ")
    }
}

/// A TMDB search result, to match a title by hand.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct TmdbMatch {
    #[serde(deserialize_with = "id_string")]
    pub tmdb_id: String,
    pub title: String,
    pub year: Option<u32>,
    pub poster: Option<String>,
    pub overview: Option<String>,
}

/// A subtitle file the server has for a title or episode.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Subtitle {
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    /// "en", "nb", …
    pub language: String,
    /// "English (SDH)"
    pub label: Option<String>,
    /// "upload", "opensubtitles", "embedded"
    pub source: Option<String>,
    /// For a series: which episode ("S01E02").
    pub episode: Option<String>,
}

/// A subtitle the server can download (`GET …/subtitles/search`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SubtitleResult {
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    pub name: String,
    /// The video release it was timed for ("Parasite.2019.1080p.BluRay.x264-REGRET").
    pub release: Option<String>,
    pub language: Option<String>,
    pub downloads: Option<u32>,
    pub episode: Option<String>,
    /// Made for this exact file (OpenSubtitles' movie hash matched).
    pub moviehash_match: bool,
}

/// A hand-made collection (`GET /api/admin/collections`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Collection {
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    pub name: String,
    /// Its titles in the order the app shows them.
    #[serde(deserialize_with = "null_as_empty")]
    pub items: Vec<CollectionItem>,
    /// Servers without an order may list library ids by kind instead.
    #[serde(deserialize_with = "id_strings")]
    pub movies: Vec<String>,
    #[serde(deserialize_with = "id_strings")]
    pub series: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct CollectionItem {
    /// "movie" or "series"
    #[serde(rename = "type")]
    pub kind: String,
    /// The library id, as the server knows it.
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    /// What to show for it, for titles the app's library doesn't have
    /// (no file yet, hidden from users).
    pub title: Option<String>,
    pub year: Option<u32>,
    pub poster: Option<String>,
}

impl CollectionItem {
    /// Our id for it ("movie:12").
    pub fn key(&self) -> String {
        let tag = if self.kind == "series" {
            "series"
        } else {
            "movie"
        };
        format!("{tag}:{}", self.id)
    }
}

impl Collection {
    /// Our item ids ("movie:12"), in order.
    pub fn members(&self) -> Vec<String> {
        if !self.items.is_empty() {
            return self.items.iter().map(CollectionItem::key).collect();
        }
        let movies = self.movies.iter().map(|id| format!("movie:{id}"));
        movies
            .chain(self.series.iter().map(|id| format!("series:{id}")))
            .collect()
    }
}

/// A device signed in as the current user (`GET /api/sessions`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DeviceSession {
    /// Not the token: an id the server gives the session.
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    /// What the app said it is at sign-in: "Valo on user-pc (Linux)".
    pub device: Option<String>,
    pub ip: Option<String>,
    /// Unix seconds.
    pub created: Option<i64>,
    pub last_used: Option<i64>,
    /// The session this request came from: this device.
    pub current: bool,
}

/// What this app calls itself at sign-in, so the user can tell their
/// devices apart: "Valo on user-pc (Linux)".
pub fn device_name() -> String {
    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty());
    let os = match std::env::consts::OS {
        "linux" => "Linux",
        "windows" => "Windows",
        "macos" => "macOS",
        o => o,
    };
    match host {
        Some(h) => format!("Valo on {h} ({os})"),
        None => format!("Valo ({os})"),
    }
}

/// The server has no admin API (`/api/admin/...`).
#[derive(Debug)]
pub struct NoAdminApi;

impl std::fmt::Display for NoAdminApi {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("this server doesn't have the Valo admin API (/api/admin/users)")
    }
}
impl std::error::Error for NoAdminApi {}

/// An admin request failed. The server's own message (`error` for the
/// Python server, `message` for Laravel) says why, e.g. "user exists".
fn admin_err(e: ureq::Error) -> anyhow::Error {
    match e {
        ureq::Error::Status(401, _) => anyhow!(Unauthorized),
        ureq::Error::Status(403, _) => anyhow!("the server says this account isn't an admin"),
        ureq::Error::Status(code, r) => {
            let text = r.status_text().to_owned();
            let msg = r.into_json::<Value>().ok().and_then(|v| {
                let m = v.get("error").or_else(|| v.get("message"))?;
                m.as_str().map(str::to_owned)
            });
            match msg {
                Some(m) => anyhow!("{m}"),
                None if matches!(code, 404 | 405 | 501) => anyhow!(NoAdminApi),
                None => anyhow!("server answered {code} {text}"),
            }
        }
        ureq::Error::Transport(t) => anyhow!("can't reach the server: {t}"),
    }
}

/// Like [`admin_err`], but a `404`/`405`/`501` always means the server
/// doesn't have this part of the admin API (even with a message).
fn admin_missing(e: ureq::Error) -> anyhow::Error {
    match e {
        ureq::Error::Status(404 | 405 | 501, _) => anyhow!(NoAdminApi),
        e => admin_err(e),
    }
}

/// Percent-encode one URL path segment (a user name).
pub fn encode_segment(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Where the Valo API is documented, for server admins.
pub const API_DOCS: &str = "https://github.com/Okaza03/Valo/blob/main/docs/api/README.md";
/// Signed-in devices, in the sign-in docs.
pub const SESSIONS_DOCS: &str =
    "https://github.com/Okaza03/Valo/blob/main/docs/api/auth.md#get-apisessions-optional";
/// The admin API part of those docs.
pub const ADMIN_API_DOCS: &str = "https://github.com/Okaza03/Valo/blob/main/docs/api/admin.md";
/// Each Admin page section's setup guide.
pub const ADMIN_DOCS_DASHBOARD: &str =
    "https://github.com/Okaza03/Valo/blob/main/docs/api/admin-dashboard.md";
pub const ADMIN_DOCS_ANALYTICS: &str =
    "https://github.com/Okaza03/Valo/blob/main/docs/api/admin-analytics.md";
pub const ADMIN_DOCS_REQUESTS: &str =
    "https://github.com/Okaza03/Valo/blob/main/docs/api/admin-requests.md";
pub const ADMIN_DOCS_LIBRARY: &str =
    "https://github.com/Okaza03/Valo/blob/main/docs/api/admin-library.md";
pub const ADMIN_DOCS_COLLECTIONS: &str =
    "https://github.com/Okaza03/Valo/blob/main/docs/api/admin-collections.md";
pub const ADMIN_DOCS_ADDONS: &str =
    "https://github.com/Okaza03/Valo/blob/main/docs/api/admin-addons.md";

/// The server is reachable but lacks a part of the Valo API, so only
/// its admin can fix it. The UI offers a message the user can send them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    /// No `/api/info` and no usable file listing.
    Api,
    /// `/api/info` says sign-in is required but `/api/login` isn't there.
    Login,
    /// `/api/info` exists but `/api/library` doesn't.
    Library,
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self {
            Missing::Api => {
                "This server doesn't support Valo: it has no Valo API \
                 (/api/info) and doesn't share its files as a folder listing either."
            }
            Missing::Login => {
                "This server says you have to sign in, but it has no sign-in API \
                 (/api/login), so Valo can't sign you in."
            }
            Missing::Library => {
                "This server has a Valo API but no library endpoint \
                 (/api/library), so there's nothing to show."
            }
        })
    }
}
impl std::error::Error for Missing {}

impl Missing {
    /// What the user can do about it.
    pub fn hint(self) -> &'static str {
        match self {
            Missing::Api | Missing::Library => {
                "Only the server's admin can add it. Send them the message below; \
                 it explains exactly what to set up."
            }
            Missing::Login => {
                "Only the server's admin can fix this: they can add the sign-in API, \
                 or turn sign-in off. Send them the message below."
            }
        }
    }

    /// A ready-to-send request for the server's admin.
    pub fn admin_note(self, base: &str) -> String {
        let (problem, wanted) = match self {
            Missing::Api => (
                "it doesn't have the Valo API",
                "  GET  /api/info     -> {\"name\": \"My server\", \"auth\": false, \"version\": 1}\n\
                 \x20 GET  /api/library  -> the list of movies and series, with stream URLs\n\
                 \x20 POST /api/login    -> only needed if you set \"auth\": true\n\n\
                 If you'd rather not write code, either of these works too:\n\
                 \x20 - run the ready-made server: python3 valo_server.py /path/to/videos\n\
                 \x20 - share the video folder as a plain directory listing\n\
                 \x20   (nginx \"autoindex on;\", Caddy \"file_server browse\", or Apache)",
            ),
            Missing::Login => (
                "it asks me to sign in but has no sign-in endpoint",
                "  POST /api/login  with {\"username\": \"...\", \"password\": \"...\"}\n\
                 \x20   -> {\"token\": \"...\", \"name\": \"...\"}, or 401 for a wrong password\n\
                 \x20 After that every request carries \"Authorization: Bearer <token>\".\n\n\
                 Or, if the library doesn't need to be private, return \"auth\": false from /api/info.",
            ),
            Missing::Library => (
                "it has /api/info but no /api/library",
                "  GET  /api/library  -> {\"movies\": [...], \"series\": [...]}\n\
                 \x20   (each movie needs a title and a stream URL that supports Range requests)",
            ),
        };
        format!(
            "Hi! I'd like to watch the videos on your server ({base}) with the Valo app, \
             but {problem}.\n\n\
             Could you add this?\n\n\
             {wanted}\n\n\
             The full spec, with examples (including a Laravel snippet), is here:\n\
             {API_DOCS}\n\n\
             Thanks!"
        )
    }
}

#[derive(Clone)]
pub struct Client {
    pub base: String,
    pub info: ServerInfo,
    pub token: Option<String>,
    /// Signed in as an admin: show the Admin buttons. Only hides them;
    /// the server's /admin page must check permissions itself.
    pub admin: bool,
    agent: ureq::Agent,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(30))
        .user_agent(concat!("Valo/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// "192.168.1.5:8080/" -> "http://192.168.1.5:8080"
pub fn normalize(input: &str) -> String {
    let s = input.trim().trim_end_matches('/');
    if s.contains("://") {
        s.to_owned()
    } else {
        format!("http://{s}")
    }
}

fn origin(url: &str) -> &str {
    let start = url.find("://").map_or(0, |i| i + 3);
    url[start..].find('/').map_or(url, |i| &url[..start + i])
}

/// Resolve a possibly-relative URL against the server base.
pub fn join(base: &str, rel: &str) -> String {
    if rel.contains("://") {
        rel.to_owned()
    } else if let Some(r) = rel.strip_prefix('/') {
        format!("{}/{r}", origin(base))
    } else {
        format!("{}/{rel}", base.trim_end_matches('/'))
    }
}

fn status_err(e: ureq::Error, what: &str) -> anyhow::Error {
    match e {
        ureq::Error::Status(401 | 403, _) => anyhow!(Unauthorized),
        ureq::Error::Status(code, r) => {
            anyhow!("{what}: server answered {code} {}", r.status_text())
        }
        ureq::Error::Transport(t) => anyhow!("{what}: {t}"),
    }
}

impl Client {
    /// Find out what kind of server lives at `address`.
    pub fn connect(address: &str) -> Result<Self> {
        let base = normalize(address);
        if base.len() <= "http://".len() {
            bail!("enter the server's address, e.g. 192.168.1.20:8080");
        }
        let agent = agent();
        let api = agent
            .get(&format!("{base}/api/info"))
            .set("Accept", "application/json")
            .call()
            .ok()
            .and_then(|r| r.into_json::<InfoJson>().ok());
        let host = base.split("://").nth(1).unwrap_or(&base).to_owned();
        let info = match api {
            Some(i) => ServerInfo {
                name: i.name.unwrap_or(host),
                auth: i.auth,
                mode: Mode::Api,
            },
            None => {
                // not an API server: is it at least a file listing?
                match agent.get(&format!("{base}/")).call() {
                    Ok(_) => {}
                    // it answered, so it's up, but there's nothing for us here
                    Err(ureq::Error::Status(..)) => bail!(Missing::Api),
                    Err(e) => return Err(status_err(e, "can't reach the server")),
                }
                ServerInfo {
                    name: host,
                    auth: false,
                    mode: Mode::Directory,
                }
            }
        };
        Ok(Self {
            base,
            info,
            token: None,
            admin: false,
            agent,
        })
    }

    fn new(base: String, info: ServerInfo) -> Self {
        Self {
            base,
            info,
            token: None,
            admin: false,
            agent: agent(),
        }
    }

    pub fn auth_header(&self) -> Option<String> {
        self.token.as_ref().map(|t| format!("Bearer {t}"))
    }

    fn get(&self, url: &str) -> ureq::Request {
        let r = self.agent.get(url);
        match self.auth_header() {
            Some(h) => r.set("Authorization", &h),
            None => r,
        }
    }

    pub fn login(&self, username: &str, password: &str) -> Result<Session> {
        let r = self
            .agent
            .post(&format!("{}/api/login", self.base))
            .set("Accept", "application/json")
            .send_json(serde_json::json!({
                "username": username, "password": password, "device": device_name(),
            }));
        match r {
            // e.g. a web app answering every path with its HTML page
            Ok(r) => r.into_json().map_err(|_| anyhow!(Missing::Login)),
            Err(ureq::Error::Status(400 | 401 | 403 | 422, _)) => {
                bail!("Wrong username or password")
            }
            Err(ureq::Error::Status(404 | 405 | 501, _)) => bail!(Missing::Login),
            Err(e) => Err(status_err(e, "sign-in failed")),
        }
    }

    /// Best-effort; the session is forgotten locally either way.
    pub fn logout(&self) {
        if self.info.mode == Mode::Api && self.token.is_some() {
            let url = format!("{}/api/logout", self.base);
            let mut r = self
                .agent
                .post(&url)
                .set("Accept", "application/json")
                .timeout(Duration::from_secs(3));
            if let Some(h) = self.auth_header() {
                r = r.set("Authorization", &h);
            }
            let _ = r.call();
        }
    }

    pub fn library(&self) -> Result<Library> {
        let mut lib = self.raw_library()?;
        lib.finish(&self.base);
        Ok(lib)
    }

    /// The library as the server sent it, before unplayable entries are dropped.
    fn raw_library(&self) -> Result<Library> {
        Ok(match self.info.mode {
            Mode::Api => match self
                .get(&format!("{}/api/library", self.base))
                .set("Accept", "application/json")
                .call()
            {
                Err(ureq::Error::Status(404 | 405 | 501, _)) => bail!(Missing::Library),
                r => r.map_err(|e| status_err(e, "can't load the library"))?,
            }
            .into_json::<Library>()
            .context("the server sent a library this app doesn't understand")?,
            Mode::Directory => self.crawl()?,
        })
    }

    /// Everyone's watch state for the signed-in user (`GET /api/progress`).
    /// `Ok(None)`: this server doesn't keep progress.
    pub fn progress(&self) -> Result<Option<Vec<Watched>>> {
        if self.info.mode != Mode::Api {
            return Ok(None);
        }
        match self
            .get(&format!("{}/api/progress", self.base))
            .set("Accept", "application/json")
            .call()
        {
            Ok(r) => Ok(Some(
                r.into_json()
                    .context("the server sent progress this app doesn't understand")?,
            )),
            // 401 also covers servers without sign-in, which have no user to keep it for
            Err(ureq::Error::Status(401 | 404 | 405 | 501, _)) => Ok(None),
            Err(e) => Err(status_err(e, "can't load watch progress")),
        }
    }

    /// Save one entry (`POST /api/progress`). The server answers with what it
    /// stored, or `Ok(None)` when it doesn't know the item or rejects it.
    pub fn save_progress(&self, w: &Watched) -> Result<Option<Watched>> {
        let mut r = self
            .agent
            .post(&format!("{}/api/progress", self.base))
            .set("Accept", "application/json");
        if let Some(h) = self.auth_header() {
            r = r.set("Authorization", &h);
        }
        let body = serde_json::json!({
            "type": w.kind, "id": w.id, "position": w.position, "duration": w.duration,
        });
        match r.send_json(body) {
            Ok(r) => Ok(r.into_json().ok()),
            Err(ureq::Error::Status(404 | 422, _)) => Ok(None),
            Err(e) => Err(status_err(e, "can't save watch progress")),
        }
    }

    /// Who the saved token belongs to (`GET /api/me`), so a remembered
    /// session picks up admin rights given or taken since it signed in.
    /// `None`: no session, or the server doesn't have the endpoint.
    pub fn me(&self) -> Option<Me> {
        if self.info.mode != Mode::Api || self.token.is_none() {
            return None;
        }
        self.get(&format!("{}/api/me", self.base))
            .set("Accept", "application/json")
            .call()
            .ok()?
            .into_json()
            .ok()
    }

    /// Change the signed-in user's own password (`POST /api/password`).
    /// This session stays signed in.
    pub fn change_password(&self, current: &str, new: &str) -> Result<()> {
        let url = format!("{}/api/password", self.base);
        let mut r = self.agent.post(&url).set("Accept", "application/json");
        if let Some(h) = self.auth_header() {
            r = r.set("Authorization", &h);
        }
        match r.send_json(serde_json::json!({ "current_password": current, "password": new })) {
            Ok(_) => Ok(()),
            Err(ureq::Error::Status(401, _)) => Err(anyhow!(Unauthorized)),
            Err(ureq::Error::Status(403, _)) => bail!("Your current password isn't right"),
            Err(ureq::Error::Status(404 | 405 | 501, _)) => {
                bail!("This server can't change passwords from the app")
            }
            Err(ureq::Error::Status(code, r)) => {
                let msg = r
                    .into_json::<Value>()
                    .ok()
                    .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_owned));
                bail!(msg.unwrap_or_else(|| format!("The server answered {code}")))
            }
            Err(e) => Err(status_err(e, "can't change the password")),
        }
    }

    /// Where the signed-in user is signed in (`GET /api/sessions`).
    /// `Ok(None)`: the server doesn't list sessions.
    pub fn sessions(&self) -> Result<Option<Vec<DeviceSession>>> {
        match self
            .get(&format!("{}/api/sessions", self.base))
            .set("Accept", "application/json")
            .call()
        {
            Ok(r) => Ok(r.into_json().ok()),
            Err(ureq::Error::Status(404 | 405 | 501, _)) => Ok(None),
            Err(e) => Err(admin_err(e)),
        }
    }

    /// Sign one of the user's devices out (`DELETE /api/sessions/<id>`).
    pub fn end_session(&self, id: &str) -> Result<()> {
        let mut r = self
            .agent
            .delete(&format!(
                "{}/api/sessions/{}",
                self.base,
                encode_segment(id)
            ))
            .set("Accept", "application/json");
        if let Some(h) = self.auth_header() {
            r = r.set("Authorization", &h);
        }
        r.call().map_err(admin_err)?;
        Ok(())
    }

    // ------------------------------------------------------------ admin API

    /// The server has its own admin web page at `/admin` (any answer but
    /// "not here": a login page or a redirect to one counts).
    pub fn has_admin_page(&self) -> bool {
        let url = format!("{}/admin", self.base);
        !matches!(
            self.agent.get(&url).call(),
            Err(ureq::Error::Status(404 | 405 | 501, _) | ureq::Error::Transport(_))
        )
    }

    fn admin(&self, method: &str, path: &str) -> ureq::Request {
        let r = self
            .agent
            .request(method, &format!("{}/api/admin/{path}", self.base))
            .set("Accept", "application/json");
        match self.auth_header() {
            Some(h) => r.set("Authorization", &h),
            None => r,
        }
    }

    /// Everyone who can sign in (`GET /api/admin/users`).
    pub fn admin_users(&self) -> Result<Vec<AdminUser>> {
        match self.admin("GET", "users").call() {
            Ok(r) => r.into_json().map_err(|_| anyhow!(NoAdminApi)), // e.g. a web app's HTML page
            Err(ureq::Error::Status(404 | 405 | 501, _)) => bail!(NoAdminApi),
            Err(e) => Err(admin_err(e)),
        }
    }

    pub fn admin_add_user(
        &self,
        name: &str,
        password: &str,
        admin: bool,
        display_name: Option<&str>,
    ) -> Result<()> {
        let mut body =
            serde_json::json!({ "username": name, "password": password, "admin": admin });
        if let Some(d) = display_name.map(str::trim).filter(|d| !d.is_empty()) {
            body["display_name"] = d.into();
        }
        self.admin("POST", "users")
            .send_json(body)
            .map_err(admin_err)?;
        Ok(())
    }

    /// Change a user's password and/or admin rights (`PATCH /api/admin/users/<name>`).
    pub fn admin_update_user(
        &self,
        name: &str,
        password: Option<&str>,
        admin: Option<bool>,
    ) -> Result<()> {
        let mut body = serde_json::Map::new();
        if let Some(p) = password {
            body.insert("password".into(), p.into());
        }
        if let Some(a) = admin {
            body.insert("admin".into(), a.into());
        }
        self.admin("PATCH", &format!("users/{}", encode_segment(name)))
            .send_json(body)
            .map_err(admin_err)?;
        Ok(())
    }

    pub fn admin_delete_user(&self, name: &str) -> Result<()> {
        self.admin("DELETE", &format!("users/{}", encode_segment(name)))
            .call()
            .map_err(admin_err)?;
        Ok(())
    }

    /// Ask a user to pick a new password, or stop asking
    /// (`PATCH /api/admin/users/<name>` with `must_change_password`).
    pub fn admin_require_password_change(&self, name: &str, on: bool) -> Result<()> {
        self.admin("PATCH", &format!("users/{}", encode_segment(name)))
            .send_json(serde_json::json!({ "must_change_password": on }))
            .map_err(admin_err)?;
        Ok(())
    }

    /// End all of a user's sessions (`POST /api/admin/users/<name>/logout`).
    pub fn admin_logout_user(&self, name: &str) -> Result<()> {
        self.admin("POST", &format!("users/{}/logout", encode_segment(name)))
            .call()
            .map_err(admin_err)?;
        Ok(())
    }

    /// Re-read the media folders now (`POST /api/admin/rescan`).
    pub fn admin_rescan(&self) -> Result<Rescan> {
        let r = self.admin("POST", "rescan").call().map_err(admin_err)?;
        Ok(r.into_json().unwrap_or_default())
    }

    /// GET an optional admin endpoint; `NoAdminApi` when the server doesn't have it.
    fn admin_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let r = self.admin("GET", path).call().map_err(admin_missing)?;
        r.into_json().map_err(|_| anyhow!(NoAdminApi))
    }

    pub fn admin_status(&self) -> Result<AdminStatus> {
        self.admin_json("status")
    }

    /// `what`: "folders" (dry run), "movies" or "series".
    pub fn admin_sync(&self, what: &str) -> Result<SyncReport> {
        let r = self
            .admin("POST", &format!("sync/{what}"))
            .timeout(Duration::from_secs(600))
            .call()
            .map_err(admin_missing)?;
        let accepted = r.status() == 202;
        let rep: SyncReport = r.into_json().unwrap_or_default();
        Ok(rep.tidy(accepted))
    }

    /// Log lines after `after` (0: the latest ones).
    pub fn admin_logs(&self, after: u64) -> Result<Vec<LogLine>> {
        self.admin_json(&format!("logs?after={after}"))
    }

    pub fn admin_analytics(&self, days: u32) -> Result<Analytics> {
        self.admin_json(&format!("analytics?days={days}"))
    }

    pub fn admin_user_analytics(&self, name: &str, days: u32) -> Result<UserAnalytics> {
        self.admin_json(&format!(
            "analytics/users/{}?days={days}",
            encode_segment(name)
        ))
    }

    pub fn admin_requests(&self) -> Result<Vec<MediaRequest>> {
        self.admin_json("requests")
    }

    /// Answer a request: a new `status`, with a note for the requester.
    pub fn admin_update_request(&self, id: &str, status: &str, note: Option<&str>) -> Result<()> {
        let mut body = serde_json::json!({ "status": status });
        if let Some(n) = note.filter(|n| !n.trim().is_empty()) {
            body["note"] = n.trim().into();
        }
        self.admin("PATCH", &format!("requests/{}", encode_segment(id)))
            .send_json(body)
            .map_err(admin_err)?;
        Ok(())
    }

    pub fn admin_delete_request(&self, id: &str) -> Result<()> {
        self.admin("DELETE", &format!("requests/{}", encode_segment(id)))
            .call()
            .map_err(admin_err)?;
        Ok(())
    }

    /// Any admin request with an optional JSON body; the answer as JSON
    /// (`null` for an empty one). `404`/`405`/`501`: not on this server.
    pub fn admin_call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value> {
        let r = self.admin(method, path).timeout(Duration::from_secs(120));
        let r = match body {
            Some(b) => r.send_json(b),
            None => r.call(),
        }
        .map_err(admin_missing)?;
        Ok(r.into_json().unwrap_or(Value::Null))
    }

    /// GET an admin endpoint as `T`.
    pub fn admin_get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.admin_json(path)
    }

    /// PUT a file (subtitle, image) as the request body.
    pub fn admin_upload(&self, path: &str, data: &[u8], mime: &str) -> Result<()> {
        self.admin("PUT", path)
            .set("Content-Type", mime)
            .timeout(Duration::from_secs(120))
            .send_bytes(data)
            .map_err(admin_missing)?;
        Ok(())
    }

    /// "movies/<id>" or "series/<id>" for an item of the library.
    pub fn item_path(item: &Item) -> String {
        let kind = match item.kind {
            Kind::Movie => "movies",
            Kind::Series => "series",
        };
        format!("{kind}/{}", encode_segment(item.server_id()))
    }

    /// Every movie or series the server has, as its admin sees them
    /// (`GET /api/admin/movies`, `/api/admin/series`): titles without a
    /// file included.
    pub fn admin_items(&self, kind: Kind) -> Result<Vec<Item>> {
        let path = if kind == Kind::Movie {
            "movies"
        } else {
            "series"
        };
        let mut items: Vec<Item> = self.admin_json(path)?;
        for it in &mut items {
            it.finish(&self.base, kind);
        }
        items.sort_by_cached_key(title_key);
        Ok(items)
    }

    /// One title with every field the server keeps (`GET /api/admin/<kind>/<id>`).
    pub fn admin_item(&self, item: &Item) -> Result<Item> {
        let mut it: Item = self.admin_json(&Self::item_path(item))?;
        it.finish(&self.base, item.kind);
        it.id = item.id.clone(); // keep ours, whatever shape the answer had
        Ok(it)
    }

    /// Change an item's details. `fields` uses the library's names;
    /// `null` clears one.
    pub fn admin_update_item(&self, item: &Item, fields: &Value) -> Result<()> {
        self.admin("PATCH", &Self::item_path(item))
            .send_json(fields)
            .map_err(admin_missing)?;
        Ok(())
    }

    /// Look the item up on IMDb (by `imdb_id`, else by title and year).
    /// Nothing is saved; the answer is library-shaped, for the editor.
    pub fn admin_fetch_item(&self, item: &Item, imdb_id: Option<&str>) -> Result<Item> {
        let mut body = serde_json::json!({ "title": item.title, "year": item.year });
        if let Some(id) = imdb_id.filter(|i| !i.trim().is_empty()) {
            body["imdb_id"] = id.trim().into();
        }
        let r = self
            .admin("POST", &format!("{}/fetch", Self::item_path(item)))
            .timeout(Duration::from_secs(60))
            .send_json(body)
            .map_err(admin_missing)?;
        let mut found: Item = r
            .into_json()
            .context("the server sent details this app doesn't understand")?;
        for u in [
            &mut found.poster,
            &mut found.backdrop,
            &mut found.thumb,
            &mut found.logo,
        ] {
            if let Some(s) = u.as_mut() {
                *s = join(&self.base, s);
            }
        }
        Ok(found)
    }

    /// Upload artwork: `which` is "poster", "backdrop", "thumb" or "logo".
    pub fn admin_upload_art(
        &self,
        item: &Item,
        which: &str,
        data: &[u8],
        mime: &str,
    ) -> Result<()> {
        self.admin("PUT", &format!("{}/art/{which}", Self::item_path(item)))
            .set("Content-Type", mime)
            .timeout(Duration::from_secs(120))
            .send_bytes(data)
            .map_err(admin_missing)?;
        Ok(())
    }

    pub fn admin_collections(&self) -> Result<Vec<Collection>> {
        self.admin_json("collections")
    }

    /// Create (`id` None) or change a collection; lists replace the old ones.
    pub fn admin_save_collection(&self, id: Option<&str>, body: &Value) -> Result<()> {
        let r = match id {
            Some(id) => self.admin("PATCH", &format!("collections/{}", encode_segment(id))),
            None => self.admin("POST", "collections"),
        };
        r.send_json(body).map_err(admin_missing)?;
        Ok(())
    }

    pub fn admin_delete_collection(&self, id: &str) -> Result<()> {
        self.admin("DELETE", &format!("collections/{}", encode_segment(id)))
            .call()
            .map_err(admin_err)?;
        Ok(())
    }

    /// The admin's Stremio subtitle addons, for everyone (`GET /api/addons`).
    /// A server without them has none.
    pub fn addons(&self) -> Result<Vec<Addon>> {
        if self.info.mode != Mode::Api {
            return Ok(Vec::new());
        }
        match self
            .get(&format!("{}/api/addons", self.base))
            .set("Accept", "application/json")
            .call()
        {
            Ok(r) => Ok(r.into_json().unwrap_or_default()),
            Err(ureq::Error::Status(404 | 405 | 501, _)) => Ok(Vec::new()),
            Err(e) => Err(status_err(e, "addons")),
        }
    }

    /// The signed-in user's own requests (`GET /api/requests`).
    pub fn my_requests(&self) -> Result<Vec<MediaRequest>> {
        let r = self
            .get(&format!("{}/api/requests", self.base))
            .set("Accept", "application/json")
            .call()
            .map_err(admin_missing)?;
        r.into_json().map_err(|_| anyhow!(NoAdminApi))
    }

    /// Ask the admin for a title that isn't on the server (`POST /api/requests`).
    pub fn request_title(&self, item: &Item) -> Result<MediaRequest> {
        let kind = match item.kind {
            Kind::Movie => "movie",
            Kind::Series => "series",
        };
        let mut r = self
            .agent
            .post(&format!("{}/api/requests", self.base))
            .set("Accept", "application/json");
        if let Some(h) = self.auth_header() {
            r = r.set("Authorization", &h);
        }
        let r = r
            .send_json(serde_json::json!({
                "title": item.title,
                "year": item.year,
                "kind": kind,
                "imdb_id": item.imdb_id,
                "poster": item.poster,
                "source": "app",
            }))
            .map_err(admin_missing)?;
        Ok(r.into_json().unwrap_or_default())
    }

    /// The addon list for the Admin page; a server without it is "not set up".
    pub fn admin_addons(&self) -> Result<Vec<Addon>> {
        let r = self
            .get(&format!("{}/api/addons", self.base))
            .set("Accept", "application/json")
            .call()
            .map_err(admin_missing)?;
        r.into_json().map_err(|_| anyhow!(NoAdminApi))
    }

    /// Add an addon by its manifest URL; the server refuses stream addons.
    pub fn admin_add_addon(&self, url: &str) -> Result<Addon> {
        let r = self
            .admin("POST", "addons")
            .send_json(serde_json::json!({ "url": url.trim() }))
            .map_err(admin_err)?;
        Ok(r.into_json().unwrap_or_default())
    }

    pub fn admin_delete_addon(&self, id: &str) -> Result<()> {
        self.admin("DELETE", &format!("addons/{}", encode_segment(id)))
            .call()
            .map_err(admin_err)?;
        Ok(())
    }

    /// Download an image (poster, backdrop) with our credentials.
    pub fn fetch(&self, url: &str) -> Result<Vec<u8>> {
        let mut r = self.agent.get(url);
        if url.starts_with(&self.base) {
            if let Some(h) = self.auth_header() {
                r = r.set("Authorization", &h);
            }
        }
        let r = r.call().map_err(|e| status_err(e, "image"))?;
        let mut buf = Vec::new();
        r.into_reader().take(25 << 20).read_to_end(&mut buf)?;
        Ok(buf)
    }

    // ------------------------------------------------------- directory mode

    fn crawl(&self) -> Result<Library> {
        let root = format!("{}/", self.base);
        let mut queue = VecDeque::from([(root.clone(), 0usize)]);
        let mut seen = HashSet::new();
        let mut videos = Vec::new();
        let mut images: HashMap<String, Vec<(String, String)>> = HashMap::new();
        while let Some((dir, depth)) = queue.pop_front() {
            if seen.len() > 5000 || !seen.insert(dir.clone()) {
                continue;
            }
            let html = match self.get(&dir).call() {
                Ok(r) => r.into_string().unwrap_or_default(),
                Err(e) if depth == 0 => return Err(status_err(e, "can't list the server")),
                Err(_) => continue,
            };
            for href in hrefs(&html) {
                let url = resolve_href(&dir, &href);
                if url.len() <= dir.len() || !url.starts_with(&dir) {
                    continue; // parent, sorting links, other hosts
                }
                if url.ends_with('/') {
                    if depth < 6 {
                        queue.push_back((url, depth + 1));
                    }
                    continue;
                }
                let name = percent_decode(url.rsplit('/').next().unwrap_or_default());
                let (stem, ext) = split_ext(&name);
                if VIDEO_EXTS.contains(&ext.as_str()) {
                    let rel = percent_decode(&dir[root.len()..]);
                    let dirs = rel
                        .split('/')
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect();
                    videos.push(Found {
                        url,
                        dir: dir.clone(),
                        dirs,
                        stem: stem.to_owned(),
                    });
                } else if IMAGE_EXTS.contains(&ext.as_str()) {
                    images
                        .entry(dir.clone())
                        .or_default()
                        .push((stem.to_lowercase(), url));
                }
            }
        }
        if videos.is_empty() {
            bail!(Missing::Api);
        }
        Ok(classify(&root, videos, &images))
    }
}

struct Found {
    url: String,
    dir: String,
    /// decoded folder names below the server root
    dirs: Vec<String>,
    stem: String,
}

fn hrefs(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(i) = rest.find("href=") {
        rest = &rest[i + 5..];
        let Some(q) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let Some(end) = rest[1..].find(q) else { break };
        out.push(rest[1..1 + end].replace("&amp;", "&"));
        rest = &rest[1 + end..];
    }
    out
}

fn resolve_href(dir: &str, href: &str) -> String {
    let href = href.split(['?', '#']).next().unwrap_or_default();
    if href.contains("://") {
        href.to_owned()
    } else if let Some(r) = href.strip_prefix("//") {
        format!("{}://{r}", dir.split("://").next().unwrap_or("http"))
    } else if href.starts_with('/') {
        format!("{}{href}", origin(dir))
    } else {
        format!("{dir}{}", href.trim_start_matches("./"))
    }
}

fn split_ext(name: &str) -> (&str, String) {
    match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, e.to_lowercase()),
        _ => (name, String::new()),
    }
}

const SHOW_DIRS: &[&str] = &["tv", "tv shows", "tvshows", "shows", "series", "tv series"];
const MOVIE_DIRS: &[&str] = &["movies", "movie", "films", "film"];
const JUNK: &[&str] = &[
    "2160p", "1080p", "1080i", "720p", "576p", "480p", "4k", "uhd", "bluray", "blu-ray", "bdrip",
    "brrip", "webrip", "web-dl", "webdl", "web", "hdtv", "dvdrip", "hdrip", "remux", "x264",
    "x265", "h264", "h265", "hevc", "avc", "xvid", "hdr", "hdr10", "dv", "10bit", "proper",
    "repack", "extended", "unrated", "aac", "ac3", "dts", "ddp5", "atmos", "imax",
];

fn is_season_dir(name: &str) -> Option<u32> {
    let l = name.to_lowercase();
    if l == "specials" {
        return Some(0);
    }
    let n = l.strip_prefix("season").or_else(|| l.strip_prefix('s'))?;
    n.trim_matches(|c: char| c == ' ' || c == '_' || c == '.')
        .parse()
        .ok()
}

/// "Show.Name.S02E05.Title.1080p" -> (2, 5, "Show.Name.", ".Title.1080p")
fn parse_episode(stem: &str) -> Option<(u32, u32, &str, &str)> {
    let b = stem.as_bytes();
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let num = |from: usize, n: usize| stem[from..from + n].parse::<u32>().ok();
    for i in 0..b.len() {
        let boundary = i == 0 || !b[i - 1].is_ascii_alphanumeric();
        if !boundary {
            continue;
        }
        // S01E02 / s1e2 / S01.E02
        if b[i].eq_ignore_ascii_case(&b's') {
            let sn = digits(i + 1);
            if (1..=2).contains(&sn) {
                let mut j = i + 1 + sn;
                if j < b.len() && matches!(b[j], b'.' | b' ' | b'_' | b'-') {
                    j += 1;
                }
                if j < b.len() && b[j].eq_ignore_ascii_case(&b'e') {
                    let en = digits(j + 1);
                    if (1..=3).contains(&en) {
                        let end = j + 1 + en;
                        return Some((num(i + 1, sn)?, num(j + 1, en)?, &stem[..i], &stem[end..]));
                    }
                }
            }
        }
        // 1x02
        let sn = digits(i);
        if (1..=2).contains(&sn) && b.get(i + sn).is_some_and(|c| c.eq_ignore_ascii_case(&b'x')) {
            let en = digits(i + sn + 1);
            if (2..=3).contains(&en) {
                let end = i + sn + 1 + en;
                return Some((num(i, sn)?, num(i + sn + 1, en)?, &stem[..i], &stem[end..]));
            }
        }
    }
    None
}

/// "2.Fast.2.Furious.2003.2160p" -> ("2 Fast 2 Furious", Some(2003))
pub fn clean_title(raw: &str) -> (String, Option<u32>) {
    let mut s = String::with_capacity(raw.len());
    let mut depth = 0;
    for c in raw.chars() {
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' => depth = (depth - 1).max(0),
            '.' | '_' | '(' | ')' if depth == 0 => s.push(' '),
            c if depth == 0 => s.push(c),
            _ => {}
        }
    }
    let mut words = Vec::new();
    let mut year = None;
    for (i, w) in s.split_whitespace().enumerate() {
        let lw = w.to_lowercase();
        if i > 0 && w.len() == 4 {
            if let Ok(y @ 1900..=2099) = w.parse::<u32>() {
                year = Some(y);
                break;
            }
        }
        if i > 0 && JUNK.contains(&lw.as_str()) {
            break;
        }
        words.push(w);
    }
    while words.last().is_some_and(|w| *w == "-") {
        words.pop();
    }
    while words.first().is_some_and(|w| *w == "-") {
        words.remove(0);
    }
    (words.join(" "), year)
}

fn pick_image(images: Option<&Vec<(String, String)>>, names: &[&str]) -> Option<String> {
    let imgs = images?;
    names
        .iter()
        .find_map(|n| imgs.iter().find(|(s, _)| s == n).map(|(_, u)| u.clone()))
}

fn classify(
    root: &str,
    videos: Vec<Found>,
    images: &HashMap<String, Vec<(String, String)>>,
) -> Library {
    let mut lib = Library::default();
    let mut shows: HashMap<String, Item> = HashMap::new();
    for v in videos {
        let category = v.dirs.first().map(|d| d.to_lowercase()).unwrap_or_default();
        let in_movies = MOVIE_DIRS.contains(&category.as_str());
        let in_shows = SHOW_DIRS.contains(&category.as_str());
        let season_dir = v.dirs.iter().rposition(|d| is_season_dir(d).is_some());
        let ep = parse_episode(&v.stem);
        let dir_url = |upto: usize| {
            // URL of v.dirs[..upto]; v.dir is the URL of all of them
            let mut u = v.dir.clone();
            for _ in upto..v.dirs.len() {
                u.truncate(u[..u.len() - 1].rfind('/').map_or(u.len(), |i| i + 1));
            }
            u
        };

        if in_movies || !(ep.is_some() || in_shows || season_dir.is_some()) {
            let (mut title, mut year) = clean_title(&v.stem);
            let own_folder = !v.dirs.is_empty() && v.dirs.len() > usize::from(in_movies);
            if own_folder {
                let (t, y) = clean_title(v.dirs.last().unwrap());
                year = year.or(y);
                if title.is_empty() {
                    title = t;
                }
            }
            if title.is_empty() {
                title = v.stem.clone();
            }
            let stem = v.stem.to_lowercase();
            let imgs = images.get(&v.dir);
            let (poster_names, backdrop_names): (Vec<&str>, Vec<&str>) = if own_folder {
                (
                    vec![&stem, "poster", "folder", "cover"],
                    vec!["fanart", "backdrop", "background"],
                )
            } else {
                (vec![&stem], vec![])
            };
            let fanart = format!("{stem}-fanart");
            let poster2 = format!("{stem}-poster");
            lib.movies.push(Item {
                id: v.url.clone(),
                title,
                year,
                poster: pick_image(imgs, &poster_names).or_else(|| pick_image(imgs, &[&poster2])),
                backdrop: pick_image(imgs, &backdrop_names)
                    .or_else(|| pick_image(imgs, &[&fanart])),
                stream: Some(v.url),
                ..Default::default()
            });
            continue;
        }

        let (ep_s, ep_e, before, after) = match ep {
            Some((s, e, b, a)) => (Some(s), Some(e), b, a),
            None => (None, None, v.stem.as_str(), ""),
        };
        let show_idx = if in_shows && v.dirs.len() >= 2 {
            Some(1)
        } else if let Some(i) = season_dir.filter(|i| *i > 0) {
            Some(i - 1)
        } else if !v.dirs.is_empty() && !in_shows {
            Some(v.dirs.len() - 1)
        } else {
            None
        };
        let (show_name, show_url) = match show_idx {
            Some(i) => (clean_title(&v.dirs[i]).0, Some(dir_url(i + 1))),
            None => (clean_title(before).0, None),
        };
        let show_name = if show_name.is_empty() {
            "Unknown show".to_owned()
        } else {
            show_name
        };
        let season_no = ep_s
            .or_else(|| season_dir.and_then(|i| is_season_dir(&v.dirs[i])))
            .unwrap_or(1);
        let ep_title = clean_title(after).0;

        let key = show_name.to_lowercase();
        let show = shows.entry(key.clone()).or_insert_with(|| {
            let imgs = show_url.as_ref().and_then(|u| images.get(u));
            Item {
                id: format!("{root}#show:{key}"),
                title: show_name.clone(),
                kind: Kind::Series,
                poster: pick_image(imgs, &["poster", "folder", "cover", "show"]),
                backdrop: pick_image(imgs, &["fanart", "backdrop", "background"]),
                ..Default::default()
            }
        });
        if show.poster.is_none() {
            show.poster = pick_image(images.get(&v.dir), &["poster", "folder", "cover"]);
        }
        let season = match show.seasons.iter_mut().position(|s| s.number == season_no) {
            Some(i) => &mut show.seasons[i],
            None => {
                show.seasons.push(Season {
                    number: season_no,
                    ..Default::default()
                });
                show.seasons.last_mut().unwrap()
            }
        };
        let still_name = v.stem.to_lowercase();
        season.episodes.push(Episode {
            id: v.url.clone(),
            number: ep_e.unwrap_or(0),
            title: ep_title,
            still: pick_image(
                images.get(&v.dir),
                &[&still_name, &format!("{still_name}-thumb")],
            ),
            stream: v.url,
            ..Default::default()
        });
    }
    for mut show in shows.into_values() {
        for s in &mut show.seasons {
            s.episodes
                .sort_by(|a, b| a.number.cmp(&b.number).then(a.stream.cmp(&b.stream)));
            if s.episodes.iter().any(|e| e.number == 0) {
                for (i, e) in s.episodes.iter_mut().enumerate() {
                    e.number = i as u32 + 1;
                }
            }
            for e in &mut s.episodes {
                if e.title.is_empty() {
                    e.title = format!("Episode {}", e.number);
                }
            }
        }
        lib.series.push(show);
    }
    lib
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles() {
        assert_eq!(
            clean_title("2.Fast.2.Furious.2003.2160p"),
            ("2 Fast 2 Furious".into(), Some(2003))
        );
        assert_eq!(
            clean_title("1917 (2019) [1080p]"),
            ("1917".into(), Some(2019))
        );
        assert_eq!(
            clean_title("The_Matrix_1080p_BluRay"),
            ("The Matrix".into(), None)
        );
    }

    #[test]
    fn episodes() {
        let (s, e, before, after) = parse_episode("Breaking.Bad.S02E05.Breakage.720p").unwrap();
        assert_eq!((s, e), (2, 5));
        assert_eq!(clean_title(before).0, "Breaking Bad");
        assert_eq!(clean_title(after).0, "Breakage");
        assert_eq!(
            parse_episode("show 3x07 - Name").map(|x| (x.0, x.1)),
            Some((3, 7))
        );
        assert!(parse_episode("2.Fast.2.Furious.2003.2160p").is_none());
        assert!(parse_episode("Inception.2010.1080p.x264").is_none());
    }

    #[test]
    fn urls() {
        assert_eq!(normalize(" 10.0.0.2:8080/ "), "http://10.0.0.2:8080");
        assert_eq!(join("http://h:1/media", "/a/b.jpg"), "http://h:1/a/b.jpg");
        assert_eq!(join("http://h:1", "a/b.jpg"), "http://h:1/a/b.jpg");
        assert_eq!(resolve_href("http://h/Movies/", "/x/"), "http://h/x/");
        assert_eq!(
            resolve_href("http://h/Movies/", "A%20B.mkv?x"),
            "http://h/Movies/A%20B.mkv"
        );
    }

    #[test]
    fn null_lists() {
        let lib: Library = serde_json::from_str(
            r#"{"movies":[{"title":"M","stream":"/s","cast":null,"genres":null,"reviews":null}],"series":null}"#,
        )
        .unwrap();
        assert!(lib.movies[0].cast.is_empty() && lib.series.is_empty());
    }

    #[test]
    fn session_admin_flag() {
        let s: Session = serde_json::from_str(r#"{"token":"t","name":"N"}"#).unwrap();
        assert!(!s.admin);
        let s: Session = serde_json::from_str(r#"{"token":"t","admin":true}"#).unwrap();
        assert!(s.admin);
    }

    #[test]
    fn collections_sort_together_in_their_order() {
        let mut lib: Library = serde_json::from_str(
            r#"{"movies": [
                {"id": "1", "title": "The Batman", "year": 2022, "stream": "a", "collection": "Batman", "collection_index": 0},
                {"id": "2", "title": "Avatar", "year": 2009, "stream": "b"},
                {"id": "3", "title": "Batman Returns", "year": 1992, "stream": "c", "collection": {"name": "Batman"}, "collection_index": 2},
                {"id": "4", "title": "Batman", "year": 1989, "stream": "d", "collection": "Batman", "collection_index": 1},
                {"id": "5", "title": "Casino", "year": 1995, "stream": "e", "collection": 7}
            ]}"#,
        )
        .unwrap();
        lib.finish("http://h");
        let titles: Vec<&str> = lib.movies.iter().map(|m| m.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Avatar", "The Batman", "Batman", "Batman Returns", "Casino"]
        );
        assert_eq!(lib.movies[1].server_id(), "1");
    }

    #[test]
    fn collection_members() {
        let c: Collection = serde_json::from_str(
            r#"{"id": 3, "name": "B", "items": [{"type": "series", "id": "x"}, {"type": "movie", "id": 9}]}"#,
        )
        .unwrap();
        assert_eq!(
            (c.id.as_str(), c.members()),
            ("3", vec!["series:x".into(), "movie:9".into()])
        );
        let old: Collection =
            serde_json::from_str(r#"{"id": "a", "name": "B", "movies": [1, 2], "series": null}"#)
                .unwrap();
        assert_eq!(old.members(), ["movie:1", "movie:2"]);
    }

    #[test]
    fn other_sync_and_log_shapes() {
        let r: SyncReport = serde_json::from_str(
            r#"{"dry_run":true,"added_movies":["a"],"removed_movies":[],"added_series":["b"],"removed_series":["c"]}"#,
        )
        .unwrap();
        let r = r.tidy(false);
        assert_eq!(
            (r.added, r.removed, r.applied, r.running),
            (vec!["a".into(), "b".into()], vec!["c".into()], false, false)
        );
        let r: SyncReport = serde_json::from_str(r#"{"started":true,"target":"movies"}"#).unwrap();
        assert!(r.tidy(false).running);
        assert!(SyncReport::default().tidy(true).running);
        let l: Vec<LogLine> =
            serde_json::from_str(r#"[{"id":1,"text":"[00:39:39] folders"}]"#).unwrap();
        assert_eq!(
            (l[0].message.as_str(), l[0].time),
            ("[00:39:39] folders", 0)
        );
    }

    #[test]
    fn number_ids_and_server_health() {
        let mut lib: Library =
            serde_json::from_str(r#"{"movies": [{"id": 12, "title": "Heat", "stream": "/m/12"}]}"#)
                .unwrap();
        lib.finish("http://h");
        assert_eq!(lib.movies[0].id, "movie:12");
        let s: AdminStatus = serde_json::from_str(
            r#"{"library": {"movies": 3, "size": 9}, "health": [{"label": "No poster", "count": 1,
                "titles": [{"type": "movie", "id": 12, "title": "Heat", "year": 1995}]}]}"#,
        )
        .unwrap();
        assert_eq!(s.library.unwrap().movies, Some(3));
        assert_eq!(s.health[0].titles[0].key(), "movie:12");
    }

    #[test]
    fn dates_lose_their_time() {
        let mut lib: Library = serde_json::from_str(
            r#"{"series": [{"id": 1, "title": "Arcane", "released": "2021-11-06 00:00:00",
                "seasons": [{"number": 1, "episodes": [{"id": 1, "number": 1, "stream": "/e/1", "aired": "2021-11-06 00:00:00"}]}]}]}"#,
        )
        .unwrap();
        lib.finish("http://h");
        let s = &lib.series[0];
        assert_eq!(s.released.as_deref(), Some("2021-11-06"));
        assert_eq!(
            s.seasons[0].episodes[0].aired.as_deref(),
            Some("2021-11-06")
        );
    }

    #[test]
    fn website_parity_shapes() {
        let s: LibraryStats =
            serde_json::from_str(r#"{"movies": 468, "mp4": 271, "series_complete": 17}"#).unwrap();
        assert_eq!((s.movies, s.extra.len()), (Some(468), 2));
        let i: Item = serde_json::from_str(
            r#"{"id": 1, "title": "Arcane", "missing_subtitle_count": 3, "locked": ["genres"], "tmdb_id": 94605}"#,
        )
        .unwrap();
        assert_eq!((i.missing_subtitles, i.locked.len()), (Some(3), 1));
        let a: Analytics = serde_json::from_str(
            r#"{"top": [{"title": "Heat", "kind": "movie", "plays": 3, "users": 2}],
                "top_rated": [{"title": "Heat", "avg_rating": 8.5, "rating_count": 4}]}"#,
        )
        .unwrap();
        assert_eq!((a.top[0].users, a.top_rated[0].rating_count), (Some(2), 4));
        let ua: UserAnalytics = serde_json::from_str(
            r#"{"name": "jo", "hours_of_day": null, "recent": [{"type": "episode", "id": 7, "title": "Arcane", "updated": 1}]}"#,
        )
        .unwrap();
        assert_eq!((ua.recent[0].id.as_str(), ua.hours_of_day.len()), ("7", 0));
        let u: AdminUser =
            serde_json::from_str(r#"{"name": "jo", "display_name": "Jo Ann"}"#).unwrap();
        assert_eq!(u.display_name.as_deref(), Some("Jo Ann"));
    }

    #[test]
    fn admin_api_json() {
        let me: Me = serde_json::from_str(r#"{"name":"N","admin":true}"#).unwrap();
        assert!(me.admin && !me.must_change_password);
        let me: Me = serde_json::from_str(r#"{"must_change_password":true}"#).unwrap();
        assert!(me.must_change_password);
        let u: Vec<AdminUser> = serde_json::from_str(
            r#"[{"name":"a","admin":true,"sessions":2,"last_active":1760400000},{"name":"b"}]"#,
        )
        .unwrap();
        assert_eq!(
            (u[0].sessions, u[1].admin, u[1].last_active),
            (Some(2), false, None)
        );
        let r: Rescan = serde_json::from_str(r#"{"movies":3}"#).unwrap();
        assert_eq!((r.movies, r.series, r.running), (Some(3), None, false));
        let r: Rescan = serde_json::from_str(r#"{"running":true}"#).unwrap();
        assert!(r.running);
        assert_eq!(encode_segment("Jo Ann/ä"), "Jo%20Ann%2F%C3%A4");
    }

    #[test]
    fn admin_notes() {
        for m in [Missing::Api, Missing::Login, Missing::Library] {
            let n = m.admin_note("http://h:8080");
            assert!(n.contains("http://h:8080") && n.contains(API_DOCS), "{n}");
        }
        assert!(Missing::Login.admin_note("x").contains("/api/login"));
        let e = anyhow!(Missing::Login);
        assert_eq!(e.downcast_ref::<Missing>(), Some(&Missing::Login));
    }

    #[test]
    fn ids_unique_across_kinds() {
        let mut lib: Library = serde_json::from_str(
            r#"{"movies":[{"id":"1","title":"M","stream":"/s/m/1"}],
                "series":[{"id":"1","title":"S","seasons":[{"number":1,"episodes":[{"id":"1","number":1,"title":"E","stream":"/s/e/1"}]}]}]}"#,
        )
        .unwrap();
        lib.finish("http://h");
        assert_eq!(lib.find("movie:1").unwrap().title, "M");
        assert_eq!(lib.find("series:1").unwrap().title, "S");
        assert_eq!(lib.find_episode("episode:1").unwrap().0.title, "S");
        assert_eq!(lib.movies[0].stream.as_deref(), Some("http://h/s/m/1"));
    }

    #[test]
    fn classify_listing() {
        let root = "http://h/";
        let f = |path: &str| {
            let (d, file) = path.rsplit_once('/').unwrap_or(("", path));
            let dirs: Vec<String> = d
                .split('/')
                .filter(|s| !s.is_empty())
                .map(Into::into)
                .collect();
            let dir = if dirs.is_empty() {
                root.to_owned()
            } else {
                format!("{root}{d}/")
            };
            Found {
                url: format!("{root}{path}"),
                dir,
                dirs,
                stem: split_ext(file).0.to_owned(),
            }
        };
        let lib = classify(
            root,
            vec![
                f("Movies/Inception (2010)/Inception.mkv"),
                f("2.Fast.2.Furious.2003.2160p.mkv"),
                f("TV/The Office/Season 2/The.Office.S02E01.The.Dundies.mkv"),
                f("TV/The Office/Season 1/The.Office.S01E01.Pilot.mkv"),
                f("Cartoons/Bluey/Bluey.S01E02.mkv"),
            ],
            &HashMap::from([(
                "http://h/TV/The Office/".to_owned(),
                vec![("poster".into(), "p.jpg".into())],
            )]),
        );
        let mut movies: Vec<_> = lib
            .movies
            .iter()
            .map(|m| (m.title.as_str(), m.year))
            .collect();
        movies.sort();
        assert_eq!(
            movies,
            [("2 Fast 2 Furious", Some(2003)), ("Inception", Some(2010))]
        );
        let office = lib.series.iter().find(|s| s.title == "The Office").unwrap();
        assert_eq!(office.poster.as_deref(), Some("p.jpg"));
        assert_eq!(office.seasons.len(), 2);
        assert!(lib.series.iter().any(|s| s.title == "Bluey"));
    }
}
