//! "New on the server": movies, series and seasons added since the user
//! last saw the library, and titles taken off it, announced on the browse
//! pages (never over the player).
use std::collections::HashSet;

use eframe::egui::{self, RichText, Ui};

use super::details::poster_tile;
use super::{Browse, Click, View, GAP};
use crate::api::{Item, Kind, Library};
use crate::images::Images;
use crate::ui::settings::{dialog, header};
use crate::ui::{primary_button, MUTED};

/// At most this many posters; the rest are counted.
const SHOWN: usize = 12;

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

/// "Season 2", "Seasons 1–3", "Seasons 1, 3 and 5", "Season 1 and Specials"
fn seasons_label(nums: &[u32]) -> String {
    let mut n: Vec<u32> = nums.iter().copied().filter(|&n| n > 0).collect();
    n.sort_unstable();
    n.dedup();
    let specials = nums.contains(&0);
    let numbered = match n.as_slice() {
        [] => None,
        [one] => Some(format!("Season {one}")),
        [a, .., b] if (b - a) as usize == n.len() - 1 => Some(format!("Seasons {a}–{b}")),
        _ => Some(format!(
            "Seasons {}",
            and_list(&n.iter().map(u32::to_string).collect::<Vec<_>>())
        )),
    };
    match (numbered, specials) {
        (Some(s), true) => format!("{s} and Specials"),
        (Some(s), false) => s,
        (None, _) => "Specials".into(),
    }
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

impl Browse {
    /// A library (re)loaded: queue what's new and what's gone since last
    /// time. The first load of a library only remembers it.
    pub(super) fn note_arrivals(&mut self, lib: &Library) {
        let now = ids(lib);
        let prefix = self.prefix();
        match self.known.get(&prefix) {
            None => self.known.set(&prefix, now),
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
                for id in old
                    .iter()
                    .filter(|k| !k.contains("#S") && !now.contains(*k))
                {
                    if self.departures.iter().any(|(d, _)| d == id) {
                        continue;
                    }
                    let gone = self
                        .known
                        .name(&prefix, id)
                        .map(str::to_owned)
                        .or_else(|| self.lib.as_ref()?.find(id).map(name));
                    if let Some(n) = gone {
                        self.departures.push((id.clone(), n));
                    }
                }
                self.departures.retain(|(id, _)| !now.contains(id));
            }
        }
        self.known
            .name_all(&prefix, lib.all().map(|i| (i.id.clone(), name(i))));
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
        if (self.arrivals.is_empty() && self.departures.is_empty()) || !allowed || !browsing {
            return None;
        }
        if !self.arrivals_open {
            // seen now, even if the app closes with the dialog up
            self.arrivals_open = true;
            let prefix = self.prefix();
            self.known.set(&prefix, ids(lib));
        }
        let items: Vec<_> = self.arrivals.iter().filter_map(|id| lib.find(id)).collect();
        let movies = items.iter().filter(|i| i.kind == Kind::Movie).count();
        let new_series = items
            .iter()
            .filter(|i| i.kind == Kind::Series && !self.new_seasons.contains_key(&i.id))
            .count();
        let seasons: usize = self.new_seasons.values().map(Vec::len).sum();
        let mut gone: Vec<&str> = self.departures.iter().map(|(_, n)| n.as_str()).collect();
        gone.sort_unstable_by_key(|n| n.to_lowercase());
        let server = &self.client.info.name;
        let title = if items.is_empty() {
            format!("Removed from {server}")
        } else {
            format!("New on {server}")
        };
        let mut open = None;
        let (done, close) = dialog(ctx, "new-arrivals", 640.0, |ui: &mut Ui| {
            header(
                ui,
                &title,
                &summary(movies, new_series, seasons, gone.len()),
            );
            ui.add_space(8.0);
            let w = ui.available_width();
            let cols = ((w + GAP) / (140.0 + GAP)).floor().clamp(1.0, 4.0);
            let tw = (w - GAP * (cols - 1.0)) / cols;
            for row in items
                .iter()
                .take(SHOWN)
                .collect::<Vec<_>>()
                .chunks(cols as usize)
            {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = GAP;
                    for item in row {
                        let sub = subtitle(item, self.new_seasons.get(&item.id));
                        if poster_tile(ui, item, Some(&sub), tw, images) {
                            open = Some(item.id.clone());
                        }
                    }
                });
                ui.add_space(12.0);
            }
            if items.len() > SHOWN {
                let more = items.len() - SHOWN;
                ui.label(RichText::new(format!("And {more} more.")).color(MUTED()));
                ui.add_space(12.0);
            }
            if !gone.is_empty() {
                if !items.is_empty() {
                    ui.label(RichText::new("No longer on the server").strong());
                    ui.add_space(4.0);
                }
                for n in gone.iter().take(SHOWN) {
                    ui.label(RichText::new(*n).color(MUTED()));
                }
                if gone.len() > SHOWN {
                    let more = gone.len() - SHOWN;
                    ui.label(RichText::new(format!("And {more} more.")).color(MUTED()));
                }
                ui.add_space(12.0);
            }
            let b = ui.add_sized([ui.available_width(), 40.0], |ui: &mut Ui| {
                primary_button(ui, "Close", true)
            });
            b.clicked()
        });
        if done || close || open.is_some() || (items.is_empty() && gone.is_empty()) {
            self.arrivals.clear();
            self.new_seasons.clear();
            self.departures.clear();
            self.arrivals_open = false;
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
    }
}
