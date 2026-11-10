//! The media library: home page, movie/series grids, search and details.
use crate::addons::Lookup;
use crate::api::featurette;
use crate::api::Client;
use crate::api::Item;
use crate::api::Kind;
use crate::api::Library;
use crate::config::Entry;
use crate::config::Progress;
use crate::player::Source;
use crate::ui::watch::PlayItem;
use crate::util::fmt_runtime;
use crate::util::fmt_time;
use std::hash::BuildHasher;
use std::hash::RandomState;

pub(crate) mod admin;

pub(crate) mod arrivals;

pub(crate) mod details;

pub(crate) mod discover;

pub(crate) mod people;

/// The home page's own rows in their usual order (their layout keys). The
/// addon catalogs go between the first five and the rest.
pub(crate) const HOME_ROWS: [&str; 8] = [
    "continue", "mylist", "recent", "others", "top", "movies", "series", "genres",
];

pub(crate) struct NextUp<'a> {
    pub si: usize,
    pub ei: usize,
    pub resume: Option<&'a Entry>,
    pub started: bool,
    /// The series is watched (see [`series_done`]): nothing to continue,
    /// so it's off Continue watching and plays again from the start.
    pub done: bool,
}

/// The play button's label while an admin moves a title's files.
const MOVING_LABEL: &str = "Unavailable — being moved";

/// The play button's label for `item`, and whether "Start over" goes with it.
/// `key`: an item id's progress key.
pub(crate) fn play_label(
    item: &Item,
    progress: &Progress,
    key: &dyn Fn(&str) -> String,
) -> (String, bool) {
    if item.unavailable {
        return (MOVING_LABEL.into(), false);
    }
    match item.kind {
        Kind::Movie => match progress.get(&key(&item.id)) {
            Some(e) if e.in_progress() => (format!("Resume from {}", fmt_time(e.pos)), true),
            Some(e) if e.watched() => ("Watch Again".into(), false),
            _ => ("Play".into(), false),
        },
        Kind::Series => match next_up(item, progress, key) {
            Some(n) if n.done => ("Watch Again".into(), false),
            Some(n) => {
                let s = &item.seasons[n.si];
                let ep = &s.episodes[n.ei];
                let verb = match (n.resume, n.started) {
                    (Some(_), _) => "Resume",
                    (None, true) => "Continue",
                    (None, false) => "Play",
                };
                (
                    match featurette(s.number, ep.number) {
                        Some(f) => format!("{verb} {f}"),
                        None => format!("{verb} S{:02} E{:02}", s.number, ep.number),
                    },
                    n.started,
                )
            }
            None => ("Play".into(), false),
        },
    }
}

/// The episode to continue with: the one being watched, or the one after the last finished.
pub(crate) fn next_up<'a>(
    s: &Item,
    progress: &'a Progress,
    key: &dyn Fn(&str) -> String,
) -> Option<NextUp<'a>> {
    let eps: Vec<(usize, usize, &str)> = s
        .seasons
        .iter()
        .enumerate()
        .flat_map(|(si, season)| {
            season
                .episodes
                .iter()
                .enumerate()
                .map(move |(ei, e)| (si, ei, e.id.as_str()))
        })
        .collect();
    let at = |i: usize, resume, started| NextUp {
        si: eps[i].0,
        ei: eps[i].1,
        resume,
        started,
        done: false,
    };
    if eps.is_empty() {
        return None;
    }
    let last_seen = eps
        .iter()
        .enumerate()
        .filter_map(|(i, (_, _, id))| progress.get(&key(id)).map(|e| (i, e)))
        .max_by_key(|(_, e)| e.updated);
    Some(match last_seen {
        Some((i, e)) if e.in_progress() => at(i, Some(e), true),
        // opened and left within its first seconds: that one again, not the next
        Some((i, e)) if !e.watched() => at(i, None, true),
        Some(_) if series_done(s, progress, key) => NextUp {
            done: true,
            ..at(0, None, true)
        },
        Some((i, _)) if i + 1 < eps.len() => at(i + 1, None, true),
        Some(_) => at(0, None, true),
        None => at(0, None, false),
    })
}

/// The episodes that make up the series, without its featurettes.
fn story(s: &Item) -> impl Iterator<Item = &crate::api::Season> {
    s.seasons
        .iter()
        .filter(|x| x.number != crate::api::FEATURETTES)
}

/// Whether this episode has been watched: seen to 90%, or left with Next
/// Episode (the player saves that as watched to the end).
pub(crate) fn episode_watched(progress: &Progress, key: &dyn Fn(&str) -> String, id: &str) -> bool {
    progress.get(&key(id)).is_some_and(Entry::watched)
}

/// Every episode of this season watched.
pub(crate) fn season_watched(
    season: &crate::api::Season,
    progress: &Progress,
    key: &dyn Fn(&str) -> String,
) -> bool {
    !season.episodes.is_empty()
        && season
            .episodes
            .iter()
            .all(|e| episode_watched(progress, key, &e.id))
}

/// Every episode of every season watched (featurettes don't count).
pub(crate) fn whole_series_watched(
    s: &Item,
    progress: &Progress,
    key: &dyn Fn(&str) -> String,
) -> bool {
    story(s).any(|x| !x.episodes.is_empty())
        && story(s).all(|x| x.episodes.is_empty() || season_watched(x, progress, key))
}

/// The series counts as watched: every episode, or the last one was the
/// last thing watched of it.
pub(crate) fn series_done(s: &Item, progress: &Progress, key: &dyn Fn(&str) -> String) -> bool {
    if whole_series_watched(s, progress, key) {
        return true;
    }
    let Some(last) = story(s).flat_map(|x| x.episodes.last()).last() else {
        return false;
    };
    let latest = story(s)
        .flat_map(|x| &x.episodes)
        .filter_map(|e| progress.get(&key(&e.id)).map(|p| (e, p)))
        .max_by_key(|(_, p)| p.updated);
    latest.is_some_and(|(e, p)| e.id == last.id && p.watched())
}

/// What the series page says above a season's episodes, once it's all watched.
pub(crate) fn seen_note(
    s: &Item,
    si: usize,
    progress: &Progress,
    key: &dyn Fn(&str) -> String,
) -> Option<&'static str> {
    if whole_series_watched(s, progress, key) {
        Some("You've watched the whole series")
    } else if s
        .seasons
        .get(si)
        .is_some_and(|x| season_watched(x, progress, key))
    {
        Some("You've watched this season")
    } else {
        None
    }
}

/// What to play for a click on `id` (and episode `ep`): a movie, or a
/// series from that episode on. `restart`: from the beginning.
pub(crate) fn play_items(
    client: &Client,
    lib: &Library,
    id: &str,
    ep: Option<&str>,
    restart: bool,
    progress: &Progress,
    key: &dyn Fn(&str) -> String,
) -> Option<Vec<PlayItem>> {
    let item = lib.find(id)?;
    let auth = client.auth_header();
    let resume = |key: &str| {
        if restart {
            return 0.0;
        }
        progress
            .get(key)
            .filter(|e| e.in_progress())
            .map_or(0.0, |e| (e.pos - 3.0).max(0.0))
    };
    match item.kind {
        Kind::Movie => {
            let key = key(&item.id);
            Some(vec![PlayItem {
                title: item.title.clone(),
                subtitle: [
                    item.year.map(|y| y.to_string()),
                    item.duration.filter(|d| *d > 0.0).map(fmt_runtime),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · "),
                episode: false,
                source: Source::Url {
                    url: item.stream.clone()?,
                    auth,
                },
                file_name: item.file.as_ref().and_then(|f| f.name.clone()),
                start: resume(&key),
                key: Some(key),
                subtitles: item.subtitles.clone(),
                addons: Some(Lookup::movie(client, item)),
            }])
        }
        Kind::Series => {
            let (si, ei) = match ep {
                Some(ep) => lib.find_episode(ep).map(|(_, si, ei)| (si, ei))?,
                None if restart => (0, 0),
                None => next_up(item, progress, key).map(|n| (n.si, n.ei))?,
            };
            // queue this episode and everything after it
            let mut queue = Vec::new();
            for (sj, season) in item.seasons.iter().enumerate().skip(si) {
                for (ej, e) in season.episodes.iter().enumerate() {
                    if sj == si && ej < ei {
                        continue;
                    }
                    let key = key(&e.id);
                    queue.push(PlayItem {
                        title: item.title.clone(),
                        subtitle: format!(
                            "{} · {}",
                            featurette(season.number, e.number).unwrap_or_else(|| {
                                format!("S{:02}E{:02}", season.number, e.number)
                            }),
                            e.title
                        ),
                        episode: true,
                        source: Source::Url {
                            url: e.stream.clone(),
                            auth: auth.clone(),
                        },
                        file_name: e.file.as_ref().and_then(|f| f.name.clone()),
                        start: if queue.is_empty() { resume(&key) } else { 0.0 },
                        key: Some(key),
                        subtitles: e.subtitles.clone(),
                        addons: Some(Lookup::episode(client, item, season.number, e.number)),
                    });
                }
            }
            Some(queue)
        }
    }
}

/// `items` in a random order that stays the same all session; `row` gives
/// each row its own order.
pub(crate) fn shuffled<'a>(
    items: impl Iterator<Item = &'a Item>,
    row: &str,
    shuffle: &RandomState,
) -> Vec<&'a Item> {
    let mut v: Vec<&Item> = items.collect();
    v.sort_by_cached_key(|i| shuffle.hash_one((row, &i.id)));
    v
}

/// Sort `items` by `key`, ascending or descending; ties keep their order and
/// items without a key go last either way.
pub(crate) fn sort_by<K: Ord>(items: &mut [&Item], desc: bool, key: impl Fn(&Item) -> Option<K>) {
    let mut keyed: Vec<(Option<K>, &Item)> = items.iter().map(|i| (key(i), *i)).collect();
    keyed.sort_by(|(a, _), (b, _)| match (a, b) {
        (Some(a), Some(b)) if desc => b.cmp(a),
        (Some(a), Some(b)) => a.cmp(b),
        _ => a.is_none().cmp(&b.is_none()),
    });
    for (slot, (_, i)) in items.iter_mut().zip(keyed) {
        *slot = i;
    }
}

/// The movie's director, else the series' first creator.
pub(crate) fn maker(i: &Item) -> Option<&str> {
    i.director
        .as_deref()
        .or(i.creators.first().map(String::as_str))
}

/// Everyone who directed or created `i` ("A, B" directors are split).
pub(crate) fn makers(i: &Item) -> impl Iterator<Item = &str> {
    i.director
        .iter()
        .flat_map(|d| d.split(','))
        .chain(i.creators.iter().map(String::as_str))
        .map(str::trim)
        .filter(|d| !d.is_empty())
}

/// Items for the home banner, in random order: ones with a backdrop first.
pub(crate) fn featured<'a>(lib: &'a Library, shuffle: &RandomState) -> Vec<&'a Item> {
    let mut v: Vec<&Item> = lib.all().filter(|i| i.backdrop.is_some()).collect();
    if v.is_empty() {
        v = lib.all().filter(|i| i.poster.is_some()).collect();
    }
    if v.is_empty() {
        v = lib.all().collect();
    }
    v.sort_by_cached_key(|i| shuffle.hash_one(&i.id));
    v.truncate(6);
    v
}

pub(crate) fn meta_line(item: &Item) -> String {
    let mut parts = Vec::new();
    if let Some(y) = item.year {
        parts.push(y.to_string());
    }
    match item.kind {
        Kind::Movie => {
            if let Some(d) = item.duration.filter(|d| *d > 0.0) {
                parts.push(fmt_runtime(d));
            }
        }
        Kind::Series => {
            let n = item.season_count();
            let eps: usize = item.seasons.iter().map(|s| s.episodes.len()).sum();
            parts.push(format!("{n} season{}", if n == 1 { "" } else { "s" }));
            parts.push(format!("{eps} episode{}", if eps == 1 { "" } else { "s" }));
        }
    }
    if let Some(r) = &item.rating {
        parts.push(r.clone());
    }
    if !item.genres.is_empty() {
        parts.push(
            item.genres
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    parts.join("  ·  ")
}

/// "Movie · 2014 · 1h 26m", "Series · 2019 · 3 seasons".
pub(crate) fn short_meta(i: &Item) -> String {
    let mut parts = vec![if i.kind == Kind::Movie {
        "Movie"
    } else {
        "Series"
    }
    .to_owned()];
    parts.extend(i.year.map(|y| y.to_string()));
    match i.kind {
        Kind::Movie => parts.extend(i.duration.filter(|d| *d > 0.0).map(fmt_runtime)),
        Kind::Series => {
            let n = i.season_count();
            parts.push(format!("{n} season{}", if n == 1 { "" } else { "s" }));
        }
    }
    parts.join(" · ")
}

pub(crate) fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{}…", cut.rsplit_once(' ').map_or(cut.as_str(), |(a, _)| a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffle_is_stable_and_keeps_everything() {
        let items: Vec<Item> = (0..20)
            .map(|n| Item {
                id: format!("movie:{n}"),
                ..Default::default()
            })
            .collect();
        let st = RandomState::new();
        let a = shuffled(items.iter(), "movies", &st);
        let b = shuffled(items.iter(), "movies", &st);
        let ids = |v: &[&Item]| v.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&a), ids(&b)); // same order every frame
        let mut sorted = ids(&a);
        sorted.sort();
        let mut all = ids(&items.iter().collect::<Vec<_>>());
        all.sort();
        assert_eq!(sorted, all); // nothing lost or doubled
    }

    #[test]
    fn sorts_both_ways_with_blanks_last() {
        let item = |id: &str, year: Option<u32>| Item {
            id: id.into(),
            year,
            ..Default::default()
        };
        let all = [
            item("a", Some(2001)),
            item("b", None),
            item("c", Some(1999)),
        ];
        let ids = |v: &[&Item]| v.iter().map(|i| i.id.as_str()).collect::<String>();
        let mut v: Vec<&Item> = all.iter().collect();
        sort_by(&mut v, false, |i| i.year);
        assert_eq!(ids(&v), "cab");
        sort_by(&mut v, true, |i| i.year);
        assert_eq!(ids(&v), "acb");
    }

    /// Two seasons of two episodes, and progress at the given (id, pos, updated).
    fn show(watched: &[(&str, f64, i64)]) -> (Item, Progress) {
        let ep = |id: &str, number| crate::api::Episode {
            id: id.into(),
            number,
            ..Default::default()
        };
        let s = Item {
            id: "show".into(),
            kind: Kind::Series,
            seasons: vec![
                crate::api::Season {
                    number: 1,
                    title: None,
                    episodes: vec![ep("a", 1), ep("b", 2)],
                },
                crate::api::Season {
                    number: 2,
                    title: None,
                    episodes: vec![ep("c", 1), ep("d", 2)],
                },
            ],
            ..Default::default()
        };
        let mut p = Progress::default();
        p.merge(
            "",
            watched
                .iter()
                .map(|(id, pos, updated)| {
                    let e = Entry {
                        pos: *pos,
                        dur: 100.0,
                        updated: *updated,
                    };
                    (id.to_string(), e)
                })
                .collect(),
        );
        (s, p)
    }

    fn key(id: &str) -> String {
        id.to_owned()
    }

    #[test]
    fn watched_at_90_percent() {
        let (s, p) = show(&[("a", 91.0, 1), ("b", 89.0, 2)]);
        assert!(episode_watched(&p, &key, "a"));
        assert!(!episode_watched(&p, &key, "b"));
        assert_eq!(seen_note(&s, 0, &p, &key), None);
        let n = next_up(&s, &p, &key).unwrap();
        assert_eq!((n.si, n.ei, n.resume.is_some()), (0, 1, true), "resumes b");

        let (s, p) = show(&[("a", 90.0, 1), ("b", 95.0, 2)]);
        assert_eq!(
            seen_note(&s, 0, &p, &key),
            Some("You've watched this season")
        );
    }

    #[test]
    fn last_episode_watched_finishes_the_series() {
        let (s, p) = show(&[("a", 100.0, 1), ("d", 100.0, 2)]);
        let n = next_up(&s, &p, &key).unwrap();
        assert!(
            n.done,
            "off Continue watching, not back to the first episode"
        );
        assert!(series_done(&s, &p, &key));
        assert!(!whole_series_watched(&s, &p, &key));
        assert_eq!(seen_note(&s, 0, &p, &key), None);
    }

    #[test]
    fn next_episode_follows_a_watched_one() {
        let (s, p) = show(&[("a", 100.0, 1), ("b", 96.0, 2)]);
        let n = next_up(&s, &p, &key).unwrap();
        assert!(!n.done);
        assert_eq!((n.si, n.ei), (1, 0));
        assert_eq!(
            seen_note(&s, 0, &p, &key),
            Some("You've watched this season")
        );
        assert_eq!(seen_note(&s, 1, &p, &key), None);
    }

    #[test]
    fn every_episode_is_the_whole_series() {
        let all = [
            ("a", 100.0, 1),
            ("b", 100.0, 2),
            ("c", 100.0, 3),
            ("d", 100.0, 4),
        ];
        let (s, p) = show(&all);
        assert_eq!(
            seen_note(&s, 1, &p, &key),
            Some("You've watched the whole series")
        );
        // rewatching the first episode: back on Continue watching
        let mut again = all.to_vec();
        again[0] = ("a", 40.0, 5);
        let (s, p) = show(&again);
        let n = next_up(&s, &p, &key).unwrap();
        assert!(!n.done);
        assert_eq!((n.si, n.ei), (0, 0));
    }
}
