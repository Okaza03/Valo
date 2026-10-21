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
    encode_segment, Addon, CastMember, Client, Episode, Item, Kind, Season, SubtitleTrack,
};

/// Titles per catalog row.
const ROW_MAX: usize = 40;

/// What to ask the addons for, in Stremio's terms.
#[derive(Clone)]
pub struct Lookup {
    pub client: Client,
    /// "movie" or "series".
    pub kind: &'static str,
    /// "tt1375666", or "tt0903747:1:2" for an episode.
    pub id: String,
}

impl Lookup {
    pub fn movie(client: &Client, imdb: Option<&str>) -> Option<Self> {
        Some(Self {
            client: client.clone(),
            kind: "movie",
            id: imdb_id(imdb?)?,
        })
    }

    pub fn episode(client: &Client, imdb: Option<&str>, season: u32, episode: u32) -> Option<Self> {
        Some(Self {
            client: client.clone(),
            kind: "series",
            id: format!("{}:{season}:{episode}", imdb_id(imdb?)?),
        })
    }
}

/// "tt1375666", and nothing else.
pub fn imdb_id(s: &str) -> Option<String> {
    let s = s.trim();
    let ok = s.len() > 2 && s.starts_with("tt") && s[2..].bytes().all(|b| b.is_ascii_digit());
    ok.then(|| s.to_owned())
}

pub fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Movie => "movie",
        Kind::Series => "series",
    }
}

/// `GET <addon>/<resource>/<type>/<id>.json`
fn call<T: DeserializeOwned>(a: &Addon, resource: &str, kind: &str, id: &str) -> Result<T> {
    let base = a
        .url
        .trim_end_matches("/manifest.json")
        .trim_end_matches('/');
    let url = format!("{base}/{resource}/{kind}/{}.json", encode_segment(id));
    Ok(ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("Valo/", env!("CARGO_PKG_VERSION")))
        .build()
        .get(&url)
        .set("Accept", "application/json")
        .call()
        .map_err(|e| anyhow!("{}: {e}", a.name))?
        .into_json()?)
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
/// fails or is slow is left out; the others still count.
pub fn subtitles(l: &Lookup) -> Vec<SubtitleTrack> {
    let Ok(addons) = l.client.addons() else {
        return Vec::new();
    };
    let wanted: Vec<&Addon> = addons
        .iter()
        .filter(|a| a.serves("subtitles", l.kind, &l.id))
        .collect();
    each(&wanted, |a| fetch_subs(a, l).unwrap_or_default())
        .into_iter()
        .flatten()
        .collect()
}

fn fetch_subs(a: &Addon, l: &Lookup) -> Result<Vec<SubtitleTrack>> {
    let ans: SubsAnswer = call(a, "subtitles", l.kind, &l.id)?;
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
    /// "Netflix · Movies"
    pub title: String,
    pub items: Vec<Item>,
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
pub type Catalogs = (Vec<Addon>, Vec<Row>);
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
            .filter_map(|m| to_item(m, &c.kind))
            .take(ROW_MAX)
            .collect();
        let what = if c.kind == "movie" {
            "Movies"
        } else {
            "TV shows"
        };
        let name = if c.name.is_empty() { &a.name } else { &c.name };
        (!items.is_empty()).then(|| Row {
            title: format!("{name} · {what}"),
            items,
        })
    });
    Ok((addons, rows.into_iter().flatten().collect()))
}

/// Full details from the first metadata addon that has this title.
pub fn meta(addons: &[Addon], kind: &str, id: &str) -> Option<Item> {
    addons
        .iter()
        .filter(|a| a.serves("meta", kind, id))
        .find_map(|a| {
            let ans: MetaAnswer = call(a, "meta", kind, id).ok()?;
            to_item(ans.meta.as_ref()?, kind)
        })
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
fn to_item(m: &Value, kind: &str) -> Option<Item> {
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
        genres: or_links("genres", "Genres"),
        imdb_id: s("imdb_id").or_else(|| imdb_id(&id)),
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
    it.cast = or_links("cast", "Cast")
        .into_iter()
        .take(20)
        .map(|name| CastMember {
            name,
            ..Default::default()
        })
        .collect();
    let directors = or_links("director", "Directors");
    if kind == "series" {
        it.creators = or_links("writer", "Writers");
    } else {
        it.director = directors.first().cloned();
    }
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
    if dst.score.is_none() {
        dst.score = src.score;
        dst.score_source.clone_from(&src.score_source);
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
        let it = to_item(&m, "series").unwrap();
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
    fn imdb_ids() {
        assert_eq!(imdb_id(" tt0903747 ").as_deref(), Some("tt0903747"));
        assert_eq!(imdb_id("12345"), None);
        assert_eq!(imdb_id("tt"), None);
    }
}
