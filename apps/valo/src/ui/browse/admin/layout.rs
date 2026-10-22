//! Admin › Layout: how the app looks for everyone on the server. For now,
//! which rows the home page shows and in what order: the built-in ones,
//! each addon catalog and each genre.
use eframe::egui::{self, RichText, Ui};

use super::super::details::{caps, panel, TEAL};
use super::super::people::columns_with;
use super::{not_set_up, Cx, Done, Remote, Section};
use crate::addons::catalog_key;
use crate::api::{Addon, ServerLayout};
use crate::ui::chrome::Icon;
use crate::ui::{bold, mono, primary_button, small_button, small_icon_button};
use crate::ui::{ERROR, MUTED};

/// The home page's own rows: (key, name, what it is). The first two always
/// sit at the top; the rest go in the admin's order.
const HOME_ROWS: [(&str, &str, &str); 9] = [
    (
        "featured",
        "Featured banner",
        "The big artwork at the top, in the styles that have one",
    ),
    (
        "genre_tiles",
        "Genre tiles",
        "Big genre buttons under the banner, in the styles that have them",
    ),
    (
        "continue",
        "Continue watching",
        "Each user's unfinished titles",
    ),
    ("mylist", "My list", "What each user saved"),
    (
        "recent",
        "Recently added",
        "The newest titles on the server",
    ),
    (
        "top",
        "Top rated",
        "Or the Top 10, in the styles that have one",
    ),
    ("movies", "Movies", "A few movies, in a random order"),
    ("series", "TV shows", "A few series, in a random order"),
    (
        "genres",
        "Genre rows",
        "A row for each genre, up to 8, the biggest first. Users can also turn them off in Settings",
    ),
];

#[derive(Default)]
pub(in super::super) struct State {
    saved: Remote<ServerLayout>,
    /// The admin's changes, not saved yet.
    draft: Option<ServerLayout>,
    /// For the catalog rows.
    addons: Remote<Vec<Addon>>,
}

impl State {
    pub fn refresh(&mut self) {
        self.saved.retry();
        self.addons.retry();
    }

    pub fn missing(&self) -> bool {
        self.saved.missing
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        if self.saved.poll(&mut cx.login) {
            self.draft = None; // saved, or reloaded
        }
        self.addons.poll(&mut cx.login);
        if self.saved.due() {
            let c = cx.client.clone();
            self.saved.load(&cx.ctx, move || c.admin_layout());
        }
        if self.addons.due() {
            let c = cx.client.clone();
            self.addons.load(&cx.ctx, move || c.admin_addons());
        }
        if self.saved.missing {
            super::super::details::padded(ui, |ui| {
                if not_set_up(ui, Section::Layout, &cx.client.info.name) {
                    self.saved.retry();
                }
            });
            return;
        }
        let Some(saved) = self.saved.data.clone() else {
            super::super::details::padded(ui, |ui| match &self.saved.error {
                Some(e) => {
                    ui.label(
                        RichText::new(format!("Couldn't load the layout: {e}")).color(ERROR()),
                    );
                }
                None => {
                    ui.add(egui::Spinner::new().size(20.0).color(MUTED()));
                }
            });
            return;
        };
        let mut l = self.draft.clone().unwrap_or_else(|| saved.clone());
        let catalogs: Vec<(String, String)> = self
            .addons
            .data
            .iter()
            .flatten()
            .flat_map(|a| {
                a.catalogs
                    .iter()
                    .filter(|c| c.kind == "movie" || c.kind == "series")
                    .map(move |c| {
                        let what = if c.kind == "movie" {
                            "Movies"
                        } else {
                            "TV shows"
                        };
                        let name = if c.name.trim().is_empty() {
                            &a.name
                        } else {
                            &c.name
                        };
                        (
                            catalog_key(&a.id, &c.kind, &c.id),
                            format!("{} · {what}", name.trim()),
                        )
                    })
            })
            .collect();
        let genres = cx.lib.genres();
        let addons_missing = self.addons.missing;
        let busy = cx.busy;
        let mut save = false;
        let mut undo = false;
        columns_with(
            ui,
            &mut (&mut l, &mut save, &mut undo),
            |ui, (l, _, _)| {
                caps(ui, "Home page", TEAL());
                ui.add_space(4.0);
                ui.label(bold("Rows", 24.0));
                ui.add_space(14.0);
                for (key, name, about) in &HOME_ROWS[..2] {
                    toggle(ui, l, key, name, Some(about));
                }

                ui.add_space(18.0);
                caps(ui, "Rows, top to bottom", MUTED());
                ui.add_space(8.0);
                // the usual order, as the home page has it
                let mut keys: Vec<String> = super::super::HOME_ROWS[..4]
                    .iter()
                    .map(|k| k.to_string())
                    .collect();
                keys.extend(catalogs.iter().map(|c| c.0.clone()));
                keys.extend(super::super::HOME_ROWS[4..].iter().map(|k| k.to_string()));
                let order = l.ordered(&keys);
                let mut shift = None;
                for (i, key) in order.iter().enumerate() {
                    let (name, about) = match HOME_ROWS.iter().find(|r| r.0 == key) {
                        Some(r) => (r.1, Some(r.2)),
                        None => match catalogs.iter().find(|c| &c.0 == key) {
                            Some(c) => (c.1.as_str(), Some("Addon catalog")),
                            None => continue,
                        },
                    };
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.add_enabled_ui(i + 1 < order.len(), |ui| {
                                if small_icon_button(ui, Icon::ChevronDown, "Move down").clicked() {
                                    shift = Some((i, i + 1));
                                }
                            });
                            ui.add_enabled_ui(i > 0, |ui| {
                                if small_icon_button(ui, Icon::ChevronUp, "Move up").clicked() {
                                    shift = Some((i, i - 1));
                                }
                            });
                            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                toggle(ui, l, key, name, about);
                            });
                        });
                    });
                }
                if let Some((a, b)) = shift {
                    let mut order = order;
                    order.swap(a, b);
                    // keep the places of rows we can't see now, like an addon that didn't load
                    order.extend(l.home_order.iter().filter(|k| !keys.contains(k)).cloned());
                    l.home_order = order;
                }
                if catalogs.is_empty() {
                    let why = if addons_missing {
                        "This server doesn't have addons."
                    } else if self.addons.loading() {
                        "Loading the addons…"
                    } else {
                        "No catalog addons yet. Add them under Addons."
                    };
                    ui.label(mono(why, 11.0).color(MUTED()));
                }

                ui.add_space(18.0);
                caps(ui, "Genres", MUTED());
                ui.add_space(8.0);
                if genres.is_empty() {
                    ui.label(mono("The library has no genres", 11.0).color(MUTED()));
                }
                ui.add_enabled_ui(l.shows("genres") || l.shows("genre_tiles"), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(14.0, 6.0);
                        for g in &genres {
                            let key = format!("genre:{g}");
                            let mut on = l.shows(&key);
                            if ui.checkbox(&mut on, RichText::new(g).size(14.0)).changed() {
                                set(l, &key, on);
                            }
                        }
                    });
                });
            },
            |ui, (l, save, undo)| {
                panel(ui, |ui| {
                    caps(ui, "For everyone", TEAL());
                    ui.add_space(10.0);
                    let changed = **l != saved;
                    let hidden = l.home_hidden.len();
                    let moved = !l.home_order.is_empty();
                    ui.label(
                        RichText::new(if changed {
                            "You have changes that aren't saved yet.".to_owned()
                        } else if hidden == 0 && !moved {
                            "Everything shows on the home page, in the usual order.".to_owned()
                        } else if hidden == 0 {
                            "Everything shows on the home page, in your order.".to_owned()
                        } else {
                            format!(
                                "{hidden} {} hidden from the home page.",
                                if hidden == 1 { "row is" } else { "rows are" }
                            )
                        })
                        .color(crate::ui::TEXT()),
                    );
                    ui.add_space(10.0);
                    let b = ui.add_sized([ui.available_width(), 38.0], |ui: &mut Ui| {
                        primary_button(ui, "Save", changed && !busy)
                    });
                    if b.clicked() && changed && !busy {
                        **save = true;
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if changed && small_button(ui, "Undo changes").clicked() {
                            **undo = true;
                        }
                        if hidden > 0 && small_button(ui, "Show everything").clicked() {
                            l.home_hidden.clear();
                        }
                        if moved && small_button(ui, "Usual order").clicked() {
                            l.home_order.clear();
                        }
                    });
                    ui.add_space(14.0);
                    for line in [
                        "Unticked rows don't show on anyone's home page. The pages for \
                         Movies, TV shows and Streaming still list everything.",
                        "The arrows move a row up or down. The banner and genre tiles \
                         always stay at the top.",
                        "Rows with nothing in them never show, like Continue watching for \
                         someone who hasn't started anything.",
                        "Everyone gets the new home page the next time their library loads.",
                    ] {
                        ui.label(RichText::new(line).color(MUTED()));
                        ui.add_space(6.0);
                    }
                });
            },
        );
        if undo {
            self.draft = None;
        } else if l != saved {
            self.draft = Some(l.clone());
        } else {
            self.draft = None;
        }
        if save {
            cx.run(move |c| {
                c.admin_set_layout(&l)?;
                Ok(Done {
                    msg: "Saved. Everyone sees the new home page the next time it loads.".into(),
                    layout: true,
                    ..Default::default()
                })
            });
        }
    }
}

/// Show or hide a row.
fn set(l: &mut ServerLayout, key: &str, on: bool) {
    l.home_hidden.retain(|k| k != key);
    if !on {
        l.home_hidden.push(key.to_owned());
    }
}

/// A row's checkbox, with a line about it.
fn toggle(ui: &mut Ui, l: &mut ServerLayout, key: &str, name: &str, about: Option<&str>) {
    let mut on = l.shows(key);
    if ui
        .checkbox(&mut on, RichText::new(name).size(15.0))
        .changed()
    {
        set(l, key, on);
    }
    if let Some(a) = about {
        ui.horizontal(|ui| {
            ui.add_space(26.0);
            ui.label(RichText::new(a).small().color(MUTED()));
        });
    }
    ui.add_space(4.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_and_shows_rows() {
        let mut l = ServerLayout::default();
        assert!(l.shows("recent"));
        set(&mut l, "recent", false);
        set(&mut l, "recent", false);
        assert!(!l.shows("recent"));
        assert_eq!(l.home_hidden, ["recent"]);
        set(&mut l, "recent", true);
        assert!(l.shows("recent") && l.home_hidden.is_empty());
    }

    #[test]
    fn orders_rows() {
        let keys: Vec<String> = ["continue", "catalog:a", "movies", "genres"]
            .map(String::from)
            .to_vec();
        let mut l = ServerLayout::default();
        assert_eq!(l.ordered(&keys), keys);
        l.home_order = ["genres", "gone", "continue", "genres"]
            .map(String::from)
            .to_vec();
        // the rest follow the row they came after
        assert_eq!(
            l.ordered(&keys),
            ["genres", "continue", "catalog:a", "movies"]
        );
        l.home_order = ["movies", "continue"].map(String::from).to_vec();
        assert_eq!(
            l.ordered(&keys),
            ["movies", "genres", "continue", "catalog:a"]
        );
    }
}
