//! "New on the server": movies, series and seasons added since the user
//! last saw the library, and titles taken off it, announced on the browse
//! pages (never over the player).
use std::collections::{HashMap, HashSet};

use eframe::egui::{self, RichText, Ui};

use super::details::poster_tile;
use super::{Browse, Click, View, GAP};
use crate::api::{Item, Kind, Library, FEATURETTES};
use crate::config::Known;
use crate::images::Images;
use crate::ui::settings::{dialog, header};
use crate::ui::{primary_button, MUTED};

/// At most this many posters; the rest are counted.
pub(crate) const SHOWN: usize = 12;

/// A series' season as remembered next to the title ids: "<id>#S<n>".
fn season_key(id: &str, n: u32) -> String {
    format!("{id}#S{n}")
}

/// Every title's id, and every season's key.
fn ids(lib: &Library) -> HashSet<String> {
    lib.all()
        .flat_map(|i| {
            let seasons = i.seasons.iter().map(|s| season_key(&i.id, s.number));
            std::iter::once(i.id.clone()).chain(seasons)
        })
        .collect()
}

/// "Inception (2010)": how a title is named once it's gone.
fn name(item: &Item) -> String {
    match item.year {
        Some(y) => format!("{} ({y})", item.title),
        None => item.title.clone(),
    }
}

/// "1, 2 and 3"
fn and_list(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// "Season 2", "Seasons 1–3", "Seasons 1, 3 and 5", "Season 1 and Specials",
/// "Season 1, Specials and Featurettes"
fn seasons_label(nums: &[u32]) -> String {
    let mut n: Vec<u32> = nums
        .iter()
        .copied()
        .filter(|&n| n > 0 && n != FEATURETTES)
        .collect();
    n.sort_unstable();
    n.dedup();
    let numbered = match n.as_slice() {
        [] => None,
        [one] => Some(format!("Season {one}")),
        [a, .., b] if (b - a) as usize == n.len() - 1 => Some(format!("Seasons {a}–{b}")),
        _ => Some(format!(
            "Seasons {}",
            and_list(&n.iter().map(u32::to_string).collect::<Vec<_>>())
        )),
    };
    let parts: Vec<String> = numbered
        .into_iter()
        .chain(nums.contains(&0).then(|| "Specials".into()))
        .chain(nums.contains(&FEATURETTES).then(|| "Featurettes".into()))
        .collect();
    and_list(&parts)
}

/// "2 new movies, 1 new series, 3 new seasons and 1 title removed"
fn summary(movies: usize, series: usize, seasons: usize, removed: usize) -> String {
    let part = |n: usize, one: &str, many: &str| match n {
        0 => None,
        1 => Some(format!("1 new {one}")),
        n => Some(format!("{n} new {many}")),
    };
    let parts: Vec<String> = [
        part(movies, "movie", "movies"),
        part(series, "series", "series"),
        part(seasons, "season", "seasons"),
        match removed {
            0 => None,
            1 => Some("1 title removed".into()),
            n => Some(format!("{n} titles removed")),
        },
    ]
    .into_iter()
    .flatten()
    .collect();
    and_list(&parts)
}

/// The line under an arrival's poster: the year for a movie; for a series,
/// the seasons it came with, or the ones added since.
fn subtitle(item: &Item, new_seasons: Option<&Vec<u32>>) -> String {
    match (item.kind, new_seasons) {
        (Kind::Series, Some(new)) => format!("New: {}", seasons_label(new)),
        (Kind::Series, None) => {
            let nums: Vec<u32> = item.seasons.iter().map(|s| s.number).collect();
            let label = if nums.is_empty() {
                "Series".into()
            } else {
                seasons_label(&nums)
            };
            match item.year {
                Some(y) => format!("{label} · {y}"),
                None => label,
            }
        }
        _ => item.year.map(|y| y.to_string()).unwrap_or_default(),
    }
}

/// What's waiting to be announced: titles and seasons added, titles gone.
#[derive(Default)]
pub(crate) struct Queue {
    /// Newly added titles, by id.
    pub arrivals: Vec<String>,
    /// Seasons added to series the user already knew, by series id.
    pub new_seasons: HashMap<String, Vec<u32>>,
    /// Titles taken off the server since, as (id, name).
    pub departures: Vec<(String, String)>,
    /// The dialog is up.
    pub open: bool,
}

/// The dialog's contents, worked out from a [`Queue`].
pub(crate) struct Shown<'a> {
    pub title: String,
    pub summary: String,
    /// (title, the line under its poster)
    pub items: Vec<(&'a Item, String)>,
    /// Names of the titles taken off, A to Z.
    pub gone: Vec<&'a str>,
}

impl Queue {
    pub fn is_empty(&self) -> bool {
        self.arrivals.is_empty() && self.departures.is_empty()
    }

    pub fn clear(&mut self) {
        self.arrivals.clear();
        self.new_seasons.clear();
        self.departures.clear();
        self.open = false;
    }

    /// A library (re)loaded: queue what's new and what's gone since last
    /// time. The first load of a library only remembers it. `before`: the
    /// library as it was, to name a title that's gone.
    pub fn note(
        &mut self,
        known: &mut Known,
        prefix: &str,
        lib: &Library,
        before: Option<&Library>,
    ) {
        let now = ids(lib);
        match known.get(prefix) {
            None => known.set(prefix, now),
            Some(old) => {
                // series remembered before seasons were: their seasons all count as known
                let with_seasons: HashSet<&str> = old
                    .iter()
                    .filter_map(|k| k.rsplit_once("#S").map(|(id, _)| id))
                    .collect();
                for i in lib.all() {
                    if !old.contains(&i.id) {
                        if !self.arrivals.contains(&i.id) {
                            self.arrivals.push(i.id.clone());
                        }
                        continue;
                    }
                    if i.kind != Kind::Series || !with_seasons.contains(i.id.as_str()) {
                        continue;
                    }
                    let added: Vec<u32> = i
                        .seasons
                        .iter()
                        .map(|s| s.number)
                        .filter(|&n| !old.contains(&season_key(&i.id, n)))
                        .collect();
                    if added.is_empty() {
                        continue;
                    }
                    if !self.arrivals.contains(&i.id) {
                        self.arrivals.push(i.id.clone());
                    }
                    let queued = self.new_seasons.entry(i.id.clone()).or_default();
                    queued.extend(added);
                    queued.sort_unstable();
                    queued.dedup();
                }
                self.arrivals.retain(|id| now.contains(id));
                let arrivals = &self.arrivals;
                self.new_seasons.retain(|id, _| arrivals.contains(id));
                // titles (not seasons) remembered but no longer on the server
                let gone_ids: Vec<String> = old
                    .iter()
                    .filter(|k| !k.contains("#S") && !now.contains(*k))
                    .cloned()
                    .collect();
                for id in gone_ids {
                    if self.departures.iter().any(|(d, _)| *d == id) {
                        continue;
                    }
                    let gone = known
                        .name(prefix, &id)
                        .map(str::to_owned)
                        .or_else(|| before?.find(&id).map(name));
                    if let Some(n) = gone {
                        self.departures.push((id, n));
                    }
                }
                self.departures.retain(|(id, _)| !now.contains(id));
            }
        }
        known.name_all(prefix, lib.all().map(|i| (i.id.clone(), name(i))));
    }

    /// The dialog is opening: everything in it counts as seen, even if the
    /// app closes with it up.
    pub fn opening(&mut self, known: &mut Known, prefix: &str, lib: &Library) {
        if !self.open {
            self.open = true;
            known.set(prefix, ids(lib));
        }
    }

    /// What the dialog shows for `server`.
    pub fn shown<'a>(&'a self, lib: &'a Library, server: &str) -> Shown<'a> {
        let items: Vec<&Item> = self.arrivals.iter().filter_map(|id| lib.find(id)).collect();
        let movies = items.iter().filter(|i| i.kind == Kind::Movie).count();
        let new_series = items
            .iter()
            .filter(|i| i.kind == Kind::Series && !self.new_seasons.contains_key(&i.id))
            .count();
        let seasons: usize = self.new_seasons.values().map(Vec::len).sum();
        let mut gone: Vec<&str> = self.departures.iter().map(|(_, n)| n.as_str()).collect();
        gone.sort_unstable_by_key(|n| n.to_lowercase());
        Shown {
            title: if items.is_empty() {
                format!("Removed from {server}")
            } else {
                format!("New on {server}")
            },
            summary: summary(movies, new_series, seasons, gone.len()),
            items: items
                .into_iter()
                .map(|i| (i, subtitle(i, self.new_seasons.get(&i.id))))
                .collect(),
            gone,
        }
    }
}

impl Browse {
    /// A library (re)loaded: queue what's new and what's gone since last time.
    pub(super) fn note_arrivals(&mut self, lib: &Library) {
        let prefix = self.prefix();
        let before = self.lib.clone();
        self.arrivals
            .note(&mut self.known, &prefix, lib, before.as_deref());
    }

    /// Show the queued arrivals once the user is on Home, Movies or TV Shows.
    pub(super) fn arrivals_ui(
        &mut self,
        ctx: &egui::Context,
        lib: &Library,
        images: &mut Images,
        allowed: bool,
    ) -> Option<Click> {
        let browsing =
            matches!(self.view, View::Home | View::Movies | View::Series) && self.search.is_empty();
        if self.arrivals.is_empty() || !allowed || !browsing {
            return None;
        }
        let prefix = self.prefix();
        self.arrivals.opening(&mut self.known, &prefix, lib);
        let shown = self.arrivals.shown(lib, &self.client.info.name);
        let mut open = None;
        let (done, close) = dialog(ctx, "new-arrivals", 640.0, |ui: &mut Ui| {
            header(ui, &shown.title, &shown.summary);
            ui.add_space(8.0);
            let w = ui.available_width();
            let cols = ((w + GAP) / (140.0 + GAP)).floor().clamp(1.0, 4.0);
            let tw = (w - GAP * (cols - 1.0)) / cols;
            for row in shown
                .items
                .iter()
                .take(SHOWN)
                .collect::<Vec<_>>()
                .chunks(cols as usize)
            {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = GAP;
                    for (item, sub) in row {
                        if poster_tile(ui, item, Some(sub), tw, self.watched(item), images) {
                            open = Some(item.id.clone());
                        }
                    }
                });
                ui.add_space(12.0);
            }
            if shown.items.len() > SHOWN {
                let more = shown.items.len() - SHOWN;
                ui.label(RichText::new(format!("And {more} more.")).color(MUTED()));
                ui.add_space(12.0);
            }
            if !shown.gone.is_empty() {
                if !shown.items.is_empty() {
                    ui.label(RichText::new("No longer on the server").strong());
                    ui.add_space(4.0);
                }
                for n in shown.gone.iter().take(SHOWN) {
                    ui.label(RichText::new(*n).color(MUTED()));
                }
                if shown.gone.len() > SHOWN {
                    let more = shown.gone.len() - SHOWN;
                    ui.label(RichText::new(format!("And {more} more.")).color(MUTED()));
                }
                ui.add_space(12.0);
            }
            let b = ui.add_sized([ui.available_width(), 40.0], |ui: &mut Ui| {
                primary_button(ui, "Close", true)
            });
            b.clicked()
        });
        let empty = shown.items.is_empty() && shown.gone.is_empty();
        if done || close || open.is_some() || empty {
            self.arrivals.clear();
        }
        open.map(Click::Open)
    }
}

#[cfg(test)]
mod tests {
    use super::{seasons_label, summary};

    #[test]
    fn summaries() {
        assert_eq!(summary(2, 1, 0, 0), "2 new movies and 1 new series");
        assert_eq!(summary(1, 0, 0, 0), "1 new movie");
        assert_eq!(summary(0, 3, 0, 0), "3 new series");
        assert_eq!(
            summary(2, 1, 3, 0),
            "2 new movies, 1 new series and 3 new seasons"
        );
        assert_eq!(summary(0, 0, 1, 0), "1 new season");
        assert_eq!(summary(1, 0, 0, 1), "1 new movie and 1 title removed");
        assert_eq!(summary(0, 0, 0, 2), "2 titles removed");
    }

    #[test]
    fn season_labels() {
        assert_eq!(seasons_label(&[2]), "Season 2");
        assert_eq!(seasons_label(&[3, 1, 2]), "Seasons 1–3");
        assert_eq!(seasons_label(&[1, 3, 5]), "Seasons 1, 3 and 5");
        assert_eq!(seasons_label(&[0, 1]), "Season 1 and Specials");
        assert_eq!(seasons_label(&[0]), "Specials");
        assert_eq!(seasons_label(&[999]), "Featurettes");
        assert_eq!(
            seasons_label(&[0, 1, 2, 999]),
            "Seasons 1–2, Specials and Featurettes"
        );
    }
}
