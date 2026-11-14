//! Where an episode's intro, recap and credits are, from the file's chapter
//! names ("Opening", "Previously on…", "End Credits"). The player offers to
//! skip the intro and recap, and to go on to the next episode at the credits.

/// What a stretch of an episode is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Intro,
    Recap,
    Credits,
}

/// A stretch of an episode, in seconds.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Marker {
    pub kind: Kind,
    pub start: f64,
    pub end: f64,
}

/// What a chapter's name says it is. Names are matched by whole words, so
/// "Operation" isn't an opening and "Bedtime" isn't an ending.
fn kind_of(title: &str) -> Option<Kind> {
    let title = title.to_lowercase();
    let words: Vec<&str> = title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let has = |w: &str| words.contains(&w);
    // "Opening Credits" is the intro, so the intro words come first
    if has("intro") || has("opening") || has("op") || has("theme") {
        Some(Kind::Intro)
    } else if has("recap") || has("previously") || has("summary") {
        Some(Kind::Recap)
    } else if has("credits") || has("ending") || has("ed") || has("outro") || has("closing") {
        Some(Kind::Credits)
    } else {
        None
    }
}

/// The intro, recap and credits among `chapters` (start, name), in order.
/// A chapter ends where the next begins; the last at `duration`. Neighbouring
/// chapters of the same kind ("Opening Part 1", "Opening Part 2") are one.
pub fn from_chapters(chapters: &[(f64, Option<String>)], duration: f64) -> Vec<Marker> {
    let mut out: Vec<Marker> = Vec::new();
    for (i, (start, title)) in chapters.iter().enumerate() {
        let end = chapters.get(i + 1).map_or(duration, |c| c.0);
        let Some(kind) = title.as_deref().and_then(kind_of) else {
            continue;
        };
        if end - start < 1.0 {
            continue; // unknown length, or a marker rather than a stretch
        }
        match out.last_mut() {
            Some(m) if m.kind == kind && (m.end - start).abs() < 0.5 => m.end = end,
            _ => out.push(Marker {
                kind,
                start: *start,
                end,
            }),
        }
    }
    out
}

/// The server's times for an episode.
pub fn from_server(m: &crate::api::Markers) -> Vec<Marker> {
    [
        (Kind::Recap, m.recap),
        (Kind::Intro, m.intro),
        (Kind::Credits, m.credits),
    ]
    .into_iter()
    .filter_map(|(kind, t)| {
        let [start, end] = t?;
        (end > start).then_some(Marker { kind, start, end })
    })
    .collect()
}

/// The server's times, with the chapters' for whatever the server doesn't
/// know (an admin may have fixed the server's by hand), in order.
pub fn merge(server: Vec<Marker>, chapters: Vec<Marker>) -> Vec<Marker> {
    let mut out = server;
    for m in chapters {
        if !out.iter().any(|s| s.kind == m.kind) {
            out.push(m);
        }
    }
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    out
}

/// The marker playing at `t`.
pub fn at(markers: &[Marker], t: f64) -> Option<Marker> {
    markers.iter().find(|m| m.start <= t && t < m.end).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(list: &[(f64, &str)]) -> Vec<(f64, Option<String>)> {
        list.iter()
            .map(|(t, n)| (*t, (!n.is_empty()).then(|| n.to_string())))
            .collect()
    }

    #[test]
    fn names() {
        assert_eq!(kind_of("Opening"), Some(Kind::Intro));
        assert_eq!(kind_of("Opening Credits"), Some(Kind::Intro));
        assert_eq!(kind_of("OP"), Some(Kind::Intro));
        assert_eq!(kind_of("Previously On"), Some(Kind::Recap));
        assert_eq!(kind_of("Recap"), Some(Kind::Recap));
        assert_eq!(kind_of("End Credits"), Some(Kind::Credits));
        assert_eq!(kind_of("ED"), Some(Kind::Credits));
        assert_eq!(kind_of("Operation"), None);
        assert_eq!(kind_of("Bedtime"), None);
        assert_eq!(kind_of("Chapter 2"), None);
    }

    #[test]
    fn stretches_from_chapters() {
        let c = ch(&[
            (0.0, "Previously on"),
            (45.0, "Opening"),
            (130.0, "Chapter 3"),
            (1300.0, "Credits"),
            (1380.0, "Preview"),
        ]);
        let m = from_chapters(&c, 1400.0);
        assert_eq!(
            m,
            vec![
                Marker {
                    kind: Kind::Recap,
                    start: 0.0,
                    end: 45.0
                },
                Marker {
                    kind: Kind::Intro,
                    start: 45.0,
                    end: 130.0
                },
                Marker {
                    kind: Kind::Credits,
                    start: 1300.0,
                    end: 1380.0
                },
            ]
        );
        assert_eq!(at(&m, 50.0).map(|m| m.kind), Some(Kind::Intro));
        assert_eq!(at(&m, 130.0), None);
        assert_eq!(at(&m, 1379.0).map(|m| m.kind), Some(Kind::Credits));
    }

    #[test]
    fn last_chapter_runs_to_the_end_and_parts_join() {
        let c = ch(&[
            (0.0, ""),
            (60.0, "Opening Part 1"),
            (90.0, "Opening Part 2"),
            (150.0, ""),
            (1200.0, "Ending"),
        ]);
        let m = from_chapters(&c, 1290.0);
        assert_eq!(
            m[0],
            Marker {
                kind: Kind::Intro,
                start: 60.0,
                end: 150.0
            }
        );
        assert_eq!(
            m[1],
            Marker {
                kind: Kind::Credits,
                start: 1200.0,
                end: 1290.0
            }
        );
    }

    #[test]
    fn reads_the_servers_times_from_the_library() {
        let e: crate::api::Episode = serde_json::from_str(
            r#"{"id": "812", "number": 1, "title": "Pilot", "stream": "/s",
                "markers": {"intro": [19.07, 49.15], "credits": [319.65, 360.08]}}"#,
        )
        .unwrap();
        let m = from_server(e.markers.as_ref().unwrap());
        assert_eq!(
            m[0],
            Marker {
                kind: Kind::Intro,
                start: 19.07,
                end: 49.15
            }
        );
        assert_eq!(m[1].kind, Kind::Credits);
        // no times on the server: nothing
        let e: crate::api::Episode = serde_json::from_str(
            r#"{"id": 1, "number": 1, "title": "", "stream": "", "markers": null}"#,
        )
        .unwrap();
        assert!(e.markers.is_none());
    }

    #[test]
    fn server_times_win_over_chapters() {
        let server = from_server(&crate::api::Markers {
            intro: Some([40.0, 100.0]),
            credits: Some([1300.0, 1290.0]), // ends before it starts: ignored
            ..Default::default()
        });
        let chapters = from_chapters(
            &ch(&[(0.0, "Recap"), (30.0, "Opening"), (95.0, "")]),
            1400.0,
        );
        let m = merge(server, chapters);
        assert_eq!(
            m,
            vec![
                Marker {
                    kind: Kind::Recap,
                    start: 0.0,
                    end: 30.0
                },
                Marker {
                    kind: Kind::Intro,
                    start: 40.0,
                    end: 100.0
                },
            ]
        );
    }

    #[test]
    fn unnamed_chapters_give_nothing() {
        assert!(from_chapters(&ch(&[(0.0, ""), (300.0, "")]), 600.0).is_empty());
        // the length isn't known: no end for the last chapter
        assert!(from_chapters(&ch(&[(0.0, ""), (1200.0, "Credits")]), 0.0).is_empty());
    }
}
