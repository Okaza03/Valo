//! Person pages (a cast member, director or creator, built from what the
//! library knows about them) and the signed-in user's profile page.
use crate::api::Item;
use crate::api::Kind;
use crate::api::Library;
use crate::util::now_unix;
use std::collections::HashMap;

pub(crate) fn same(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// Newest first, titles without a year last.
fn by_year(v: &mut [Credit]) {
    v.sort_by_key(|c| (c.item.year.is_none(), std::cmp::Reverse(c.item.year)));
}

pub(crate) struct Credit<'a> {
    pub item: &'a Item,
    /// The character played, for acting credits.
    pub character: Option<&'a str>,
}

/// Everything one person did in the library.
pub(crate) struct Person<'a> {
    pub movies: Vec<Credit<'a>>,
    pub shows: Vec<Credit<'a>>,
    pub directed: Vec<Credit<'a>>,
    pub created: Vec<Credit<'a>>,
    pub photo: Option<&'a str>,
}

impl<'a> Person<'a> {
    pub fn find(lib: &'a Library, name: &str) -> Self {
        let mut p = Person {
            movies: Vec::new(),
            shows: Vec::new(),
            directed: Vec::new(),
            created: Vec::new(),
            photo: None,
        };
        for item in lib.all() {
            if let Some(m) = item.cast.iter().find(|c| same(&c.name, name)) {
                p.photo = p.photo.or(m.photo.as_deref());
                let c = Credit {
                    item,
                    character: m.character.as_deref().filter(|s| !s.trim().is_empty()),
                };
                match item.kind {
                    Kind::Movie => p.movies.push(c),
                    Kind::Series => p.shows.push(c),
                }
            }
            let directed = item
                .director
                .iter()
                .flat_map(|d| d.split(','))
                .any(|d| same(d, name));
            if directed {
                p.directed.push(Credit {
                    item,
                    character: None,
                });
            }
            if item.creators.iter().any(|c| same(c, name)) {
                p.created.push(Credit {
                    item,
                    character: None,
                });
            }
        }
        for v in [&mut p.movies, &mut p.shows, &mut p.directed, &mut p.created] {
            by_year(v);
        }
        p
    }

    /// Every distinct title, each once.
    pub fn titles(&self) -> Vec<&'a Item> {
        let mut v: Vec<&Item> = Vec::new();
        for c in [&self.movies, &self.shows, &self.directed, &self.created]
            .into_iter()
            .flatten()
        {
            if !v.iter().any(|i| i.id == c.item.id) {
                v.push(c.item);
            }
        }
        v
    }

    pub fn roles(&self) -> Vec<&'static str> {
        let mut r = Vec::new();
        if !self.movies.is_empty() || !self.shows.is_empty() {
            r.push("Actor");
        }
        if !self.directed.is_empty() {
            r.push("Director");
        }
        if !self.created.is_empty() {
            r.push("Creator");
        }
        r
    }
}

/// "1999–2021", or one year.
pub(crate) fn span(items: &[&Item]) -> Option<String> {
    let lo = items.iter().filter_map(|i| i.year).min()?;
    let hi = items
        .iter()
        .filter_map(|i| i.end_year.or(i.year))
        .max()
        .unwrap_or(lo);
    Some(if hi > lo {
        format!("{lo}–{hi}")
    } else {
        lo.to_string()
    })
}

/// The `n` most common of `names`, most common first, ties A to Z.
pub(crate) fn top<'a>(names: impl Iterator<Item = &'a str>, n: usize) -> Vec<(&'a str, usize)> {
    let mut count: HashMap<&str, usize> = HashMap::new();
    for g in names {
        *count.entry(g).or_default() += 1;
    }
    let mut v: Vec<(&str, usize)> = count.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    v.truncate(n);
    v
}

/// "just now", "3 hours ago", "yesterday", "12 days ago", …
pub(crate) fn ago(t: i64) -> String {
    let s = (now_unix() - t).max(0);
    let plural = |n: i64, unit: &str| format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
    match s {
        s if s < 60 => "just now".into(),
        s if s < 3600 => plural(s / 60, "minute"),
        s if s < 86_400 => plural(s / 3600, "hour"),
        s if s < 2 * 86_400 => "yesterday".into(),
        s if s < 60 * 86_400 => plural(s / 86_400, "day"),
        s if s < 365 * 86_400 => plural(s / (30 * 86_400), "month"),
        s => plural(s / (365 * 86_400), "year"),
    }
}
