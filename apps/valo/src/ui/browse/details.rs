//! The details pages: movie, series (Episodes / Related / Details tabs) and
//! a single episode. Sections the server sends no data for are left out.
use crate::api::featurette;
use crate::api::FileInfo;
use crate::api::Item;
use crate::api::Library;
use crate::api::Watcher;
use crate::ui::theme::INK;
use crate::ui::theme::PRIMARY;
use crate::util::percent_decode;
use ecolor::Color32;
use std::hash::BuildHasher;
use std::hash::RandomState;

/// Ratings, quality and the main button on Valo's details pages.
#[allow(non_snake_case)]
pub(crate) fn GOLD() -> Color32 {
    if crate::ui::theme::pal().light {
        Color32::from_rgb(168, 118, 10)
    } else {
        Color32::from_rgb(232, 196, 106)
    }
}

/// Small captions ("Cast", "File · Metadata").
#[allow(non_snake_case)]
pub(crate) fn TEAL() -> Color32 {
    if crate::ui::theme::pal().light {
        Color32::from_rgb(0, 128, 112)
    } else {
        Color32::from_rgb(110, 205, 190)
    }
}

pub(crate) fn play_colors() -> (Color32, Color32) {
    if crate::ui::theme::skin() == crate::ui::theme::Skin::Valo {
        (GOLD(), INK())
    } else {
        (PRIMARY(), INK())
    }
}

/// "31 Oct 2002"
pub(crate) fn date_long(s: &str) -> String {
    ymd(s).map_or_else(
        || s.to_owned(),
        |(y, m, d)| format!("{d} {} {y}", MONTHS[m - 1]),
    )
}

/// Unix seconds -> "31 Oct 2002" (UTC).
pub(crate) fn date_unix(t: i64) -> String {
    // days to a civil date: Howard Hinnant's days_from_civil, backwards
    let z = t.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{d} {} {y}", MONTHS[m as usize - 1])
}

/// "Oct 31, 2002"
pub(crate) fn date_short(s: &str) -> String {
    ymd(s).map_or_else(
        || s.to_owned(),
        |(y, m, d)| format!("{} {d}, {y}", MONTHS[m - 1]),
    )
}

/// 8000000 -> "$8,000,000"
pub(crate) fn money(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    format!("${out}")
}

/// 2090000000 -> "2.09 GB"
pub(crate) fn bytes(n: u64) -> String {
    let n = n as f64;
    match n {
        n if n >= 1e9 => format!("{:.2} GB", n / 1e9),
        n if n >= 1e6 => format!("{:.0} MB", n / 1e6),
        n => format!("{:.0} KB", n / 1e3),
    }
}

/// 2600000 -> "2.6 Mb/s"
pub(crate) fn bitrate(n: u64) -> String {
    format!("{:.1} Mb/s", n as f64 / 1e6)
}

pub(crate) fn minutes(secs: f64) -> String {
    format!("{} min", (secs / 60.0).round())
}

/// "2021–2024", or just the start year.
pub(crate) fn years(item: &Item) -> Option<String> {
    let y = item.year?;
    Some(match item.end_year.filter(|e| *e != y) {
        Some(e) => format!("{y}–{e}"),
        None => y.to_string(),
    })
}

/// "Arcane.S01E01.1080p.mp4": the server's file name, else the stream URL's
/// when it looks like one (not "/api/stream/movie/42").
pub(crate) fn file_name(file: Option<&FileInfo>, stream: &str) -> Option<String> {
    file.and_then(|f| f.name.clone()).or_else(|| {
        let last = stream.split(['?', '#']).next().unwrap_or(stream);
        let name = percent_decode(last.rsplit('/').next().unwrap_or(last));
        let ext = name.rsplit_once('.')?.1.to_lowercase();
        crate::api::VIDEO_EXTS
            .contains(&ext.as_str())
            .then_some(name)
    })
}

/// The server's file info with gaps filled from probing the stream.
pub(crate) fn merge(server: Option<&FileInfo>, probed: Option<&FileInfo>) -> Option<FileInfo> {
    let (s, p) = match (server, probed) {
        (None, None) => return None,
        (Some(s), None) => return Some(s.clone()),
        (None, Some(p)) => return Some(p.clone()),
        (Some(s), Some(p)) => (s, p),
    };
    Some(FileInfo {
        quality: s.quality.clone().or_else(|| p.quality.clone()),
        container: s.container.clone().or_else(|| p.container.clone()),
        video_codec: s.video_codec.clone().or_else(|| p.video_codec.clone()),
        audio_codec: s.audio_codec.clone().or_else(|| p.audio_codec.clone()),
        audio_channels: s.audio_channels.or(p.audio_channels),
        bitrate: s.bitrate.or(p.bitrate),
        size: s.size.or(p.size),
        name: s.name.clone().or_else(|| p.name.clone()),
        path: s.path.clone().or_else(|| p.path.clone()),
    })
}

pub(crate) fn sxe(season: u32, ep: u32) -> String {
    featurette(season, ep).unwrap_or_else(|| format!("S{season:02}E{ep:02}"))
}

/// "A, B" as names, for a server that sends several directors in one string.
pub(crate) fn split_names(s: &str) -> Vec<&str> {
    s.split(',')
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .collect()
}

/// `n` random items sharing a genre with `item`, the same ones each time
/// for one `shuffle`.
pub(crate) fn similar<'a>(
    lib: &'a Library,
    item: &'a Item,
    n: usize,
    shuffle: &RandomState,
) -> Vec<&'a Item> {
    let mut v: Vec<&Item> = lib
        .all()
        .filter(|i| i.id != item.id && i.genres.iter().any(|g| item.genres.contains(g)))
        .collect();
    v.sort_by_cached_key(|i| shuffle.hash_one((&item.id, &i.id)));
    v.truncate(n);
    v
}

/// (season index, episode index) of every episode, in order.
pub(crate) fn all_episodes(s: &Item) -> Vec<(usize, usize)> {
    s.seasons
        .iter()
        .enumerate()
        .flat_map(|(si, season)| (0..season.episodes.len()).map(move |ei| (si, ei)))
        .collect()
}

/// "Jo watched this", "Jo and Sam …", "Jo, Sam and 3 others …": the two
/// latest by name.
pub(crate) fn watchers_text(list: &[Watcher]) -> String {
    if list.is_empty() {
        return String::new();
    }
    format!("{} watched this", names(list.iter()))
}

/// "Jo Ann", "Jo Ann and sam", "Jo Ann, sam and 2 others".
fn names<'a>(list: impl Iterator<Item = &'a Watcher>) -> String {
    let list: Vec<&Watcher> = list.collect();
    match list.as_slice() {
        [] => String::new(),
        [a] => a.shown().to_owned(),
        [a, b] => format!("{} and {}", a.shown(), b.shown()),
        [a, b, rest @ ..] => format!(
            "{}, {} and {} other{}",
            a.shown(),
            b.shown(),
            rest.len(),
            if rest.len() == 1 { "" } else { "s" }
        ),
    }
}

/// "has" for one person, "have" for several.
fn have(n: usize) -> &'static str {
    if n == 1 {
        "has"
    } else {
        "have"
    }
}

/// Episode `e` of season number `season` is in `list`. Servers name it by
/// the library's id (which the app keeps as "episode:<id>") or as "S01E02".
fn listed(list: &[String], season: u32, e: &crate::api::Episode) -> bool {
    let id = e.id.strip_prefix("episode:").unwrap_or(&e.id);
    let code = format!("S{season:02}E{:02}", e.number);
    list.iter()
        .any(|x| x == id || x == &e.id || x.eq_ignore_ascii_case(&code))
}

/// `w` watched episode `e` to the end.
fn saw(w: &Watcher, season: u32, e: &crate::api::Episode) -> bool {
    listed(&w.episodes, season, e)
}

/// `w` played episode `e` at all, finished or not.
fn viewed(w: &Watcher, season: u32, e: &crate::api::Episode) -> bool {
    saw(w, season, e) || listed(&w.viewed, season, e)
}

/// Every episode of the season watched by `w`.
fn saw_season(w: &Watcher, x: &crate::api::Season) -> bool {
    !x.episodes.is_empty() && x.episodes.iter().all(|e| saw(w, x.number, e))
}

/// Every episode of the series (featurettes aside) watched by `w`.
fn watched_series(w: &Watcher, s: &Item) -> bool {
    if w.episodes.is_empty() {
        return w.completed; // a server that doesn't list episodes
    }
    let mut story = s
        .seasons
        .iter()
        .filter(|x| x.number != crate::api::FEATURETTES && !x.episodes.is_empty())
        .peekable();
    story.peek().is_some() && story.all(|x| saw_season(w, x))
}

/// What the series page says above a season's episodes about who else
/// watched it: "Artur has watched the whole series", or, for the others who
/// played any of this season's episodes, "Artur has watched this season".
pub(crate) fn watchers_season_notes(list: &[Watcher], s: &Item, si: usize) -> Vec<String> {
    let mut out = Vec::new();
    let whole: Vec<&Watcher> = list.iter().filter(|w| watched_series(w, s)).collect();
    if !whole.is_empty() {
        out.push(format!(
            "{} {} watched the whole series",
            names(whole.iter().copied()),
            have(whole.len())
        ));
    }
    let season: Vec<&Watcher> = match s.seasons.get(si) {
        Some(x) => list
            .iter()
            .filter(|w| !watched_series(w, s) && x.episodes.iter().any(|e| viewed(w, x.number, e)))
            .collect(),
        None => Vec::new(),
    };
    if !season.is_empty() {
        out.push(format!(
            "{} {} watched this season",
            names(season.iter().copied()),
            have(season.len())
        ));
    }
    out
}

/// Under an episode: "Artur has watched this episode", for whoever else
/// played it at all.
pub(crate) fn watchers_episode_text(list: &[Watcher], s: &Item, ep: &str) -> Option<String> {
    let (season, e) = s.seasons.iter().find_map(|x| {
        x.episodes
            .iter()
            .find(|e| e.id == ep)
            .map(|e| (x.number, e))
    })?;
    let who: Vec<&Watcher> = list
        .iter()
        .filter(|w| viewed(w, season, e) || (w.episodes.is_empty() && watched_series(w, s)))
        .collect();
    (!who.is_empty()).then(|| {
        format!(
            "{} {} watched this episode",
            names(who.iter().copied()),
            have(who.len())
        )
    })
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// "2002-10-31" -> (2002, 10, 31)
fn ymd(s: &str) -> Option<(u32, usize, u32)> {
    let mut it = s.get(..10)?.split('-');
    let y = it.next()?.parse().ok()?;
    let m: usize = it.next()?.parse().ok()?;
    let d = it.next()?.parse().ok()?;
    (1..=12).contains(&m).then_some((y, m, d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn started_episodes_count_for_the_episode_and_its_season() {
        let ep = |id: &str, number| crate::api::Episode {
            id: id.into(),
            number,
            ..Default::default()
        };
        let s = Item {
            seasons: vec![
                crate::api::Season {
                    number: 1,
                    title: None,
                    episodes: vec![ep("episode:1", 1), ep("episode:2", 2)],
                },
                crate::api::Season {
                    number: 2,
                    title: None,
                    episodes: vec![ep("episode:3", 1)],
                },
            ],
            ..Default::default()
        };
        // finished nothing, started S01E02 (by code) and episode 3 (by id)
        let list = [Watcher {
            name: "havard".into(),
            viewed: vec!["S01E02".into(), "3".into()],
            ..Default::default()
        }];
        assert_eq!(
            watchers_season_notes(&list, &s, 0),
            ["havard has watched this season"]
        );
        assert_eq!(
            watchers_season_notes(&list, &s, 1),
            ["havard has watched this season"]
        );
        assert_eq!(watchers_episode_text(&list, &s, "episode:1"), None);
        assert_eq!(
            watchers_episode_text(&list, &s, "episode:2").as_deref(),
            Some("havard has watched this episode")
        );
        assert_eq!(
            watchers_episode_text(&list, &s, "episode:3").as_deref(),
            Some("havard has watched this episode")
        );
        // the server's field may also be called "started" or "watching"
        let w: Watcher =
            serde_json::from_str(r#"{"name":"jo","episodes":[],"started":["S01E01"]}"#).unwrap();
        assert_eq!(w.viewed, ["S01E01"]);
    }

    #[test]
    fn who_watched_seasons_and_episodes() {
        let ep = |id: &str| crate::api::Episode {
            id: id.into(),
            ..Default::default()
        };
        let season = |number, ids: &[&str]| crate::api::Season {
            number,
            title: None,
            episodes: ids.iter().map(|i| ep(i)).collect(),
        };
        let s = Item {
            seasons: vec![season(1, &["a", "b"]), season(2, &["c"])],
            ..Default::default()
        };
        let w = |n: &str, eps: &[&str]| Watcher {
            name: n.into(),
            episodes: eps.iter().map(|e| e.to_string()).collect(),
            ..Default::default()
        };
        let list = [
            w("artur", &["a", "b"]),
            w("sam", &["a"]),
            w("kim", &["a", "b", "c"]),
        ];
        assert_eq!(
            watchers_season_notes(&list, &s, 0),
            [
                "kim has watched the whole series",
                // sam watched one of its two episodes: that's enough
                "artur and sam have watched this season"
            ]
        );
        assert_eq!(
            watchers_season_notes(&list, &s, 1),
            ["kim has watched the whole series"]
        );
        assert_eq!(
            watchers_episode_text(&list, &s, "a").as_deref(),
            Some("artur, sam and 1 other have watched this episode")
        );
        assert_eq!(
            watchers_episode_text(&list, &s, "c").as_deref(),
            Some("kim has watched this episode")
        );
        // named "S01E02" instead of by id, and ids as the app keeps them
        let mut s2 = s.clone();
        for (i, x) in s2.seasons.iter_mut().enumerate() {
            for (j, e) in x.episodes.iter_mut().enumerate() {
                e.id = format!("episode:{i}{j}");
                e.number = j as u32 + 1;
            }
        }
        let coded = [w("havard", &["S01E01", "s01e02"]), w("ruben", &["01"])];
        assert_eq!(
            watchers_season_notes(&coded, &s2, 0),
            ["havard and ruben have watched this season"]
        );
        assert_eq!(
            watchers_episode_text(&coded, &s2, "episode:01").as_deref(),
            Some("havard and ruben have watched this episode")
        );
        // an older server: only "completed"
        let old = [Watcher {
            name: "jo".into(),
            completed: true,
            ..Default::default()
        }];
        assert_eq!(
            watchers_season_notes(&old, &s, 0),
            ["jo has watched the whole series"]
        );
        assert!(watchers_episode_text(&old, &s, "b").is_some());
        // one episode of a season is enough for it, and only for that season
        assert_eq!(
            watchers_season_notes(&list[1..2], &s, 0),
            ["sam has watched this season"]
        );
        assert!(watchers_season_notes(&list[1..2], &s, 1).is_empty());
    }

    #[test]
    fn names_who_watched() {
        let w = |n: &str| Watcher {
            name: n.into(),
            ..Default::default()
        };
        let mut list = vec![w("jo")];
        assert_eq!(watchers_text(&list), "jo watched this");
        list[0].display_name = Some("Jo Ann".into());
        list.push(w("sam"));
        assert_eq!(watchers_text(&list), "Jo Ann and sam watched this");
        list.push(w("kim"));
        assert_eq!(watchers_text(&list), "Jo Ann, sam and 1 other watched this");
        list.push(w("lee"));
        assert_eq!(
            watchers_text(&list),
            "Jo Ann, sam and 2 others watched this"
        );
    }

    #[test]
    fn formats() {
        assert_eq!(date_long("2002-10-31"), "31 Oct 2002");
        assert_eq!(date_short("2021-11-06"), "Nov 6, 2021");
        assert_eq!(date_long("soon"), "soon");
        assert_eq!(date_unix(0), "1 Jan 1970");
        assert_eq!(date_unix(1_792_101_466), "15 Oct 2026");
        assert_eq!(date_unix(951_782_400), "29 Feb 2000");
        assert_eq!(money(8_000_000), "$8,000,000");
        assert_eq!(money(82_784_517), "$82,784,517");
        assert_eq!(money(999), "$999");
        assert_eq!(bytes(2_090_000_000), "2.09 GB");
        assert_eq!(bitrate(2_600_000), "2.6 Mb/s");
        assert_eq!(
            file_name(
                None,
                "http://h/media/Arcane/Arcane.S01E01.1080p%20x.mp4?token=1"
            )
            .as_deref(),
            Some("Arcane.S01E01.1080p x.mp4")
        );
        assert_eq!(file_name(None, "http://h/api/stream/movie/42"), None);
    }
}
