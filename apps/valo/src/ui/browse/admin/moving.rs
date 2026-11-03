//! Admin › Moving: mark titles whose files are being moved between disks.
//! While marked, nobody can play them (the server turns their streams down)
//! and their posters carry a red MOVING band; unmark them once they play
//! from their new place.
use std::collections::HashSet;

use eframe::egui::{self, RichText, Ui};

use super::super::details::{caps, padded, panel, TEAL};
use super::{not_set_up, Cx, Done, Remote, Section};
use crate::api::{Item, Kind};
use crate::ui::{bold, mono, text_field, ERROR, MUTED, TEXT};

/// How many titles of a kind to list before asking for a search.
const SHOW: usize = 300;

#[derive(Default)]
pub(in super::super) struct State {
    /// Our ids of the marked titles.
    marked: Remote<HashSet<String>>,
    search: String,
}

impl State {
    pub fn refresh(&mut self) {
        self.marked.retry();
    }

    pub fn missing(&self) -> bool {
        self.marked.missing
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        self.marked.poll(&mut cx.login);
        if self.marked.due() {
            let c = cx.client.clone();
            self.marked.load(&cx.ctx, move || c.admin_moving());
        }
        padded(ui, |ui| {
            if self.marked.missing {
                if not_set_up(ui, Section::Moving, &cx.client.info.name) {
                    self.marked.retry();
                }
                return;
            }
            if let Some(e) = &self.marked.error {
                ui.label(RichText::new(format!("Couldn't load the list: {e}")).color(ERROR()));
                ui.add_space(8.0);
            }
            let Some(marked) = self.marked.data.clone() else {
                ui.add(egui::Spinner::new().size(18.0).color(MUTED()));
                return;
            };
            let mut flip: Option<(&Item, bool)> = None;

            // what's unplayable now, on top
            let now: Vec<&Item> = cx
                .series
                .iter()
                .chain(cx.movies)
                .filter(|i| marked.contains(&i.id))
                .collect();
            panel(ui, |ui| {
                caps(ui, "Unavailable now", TEAL());
                ui.add_space(8.0);
                if now.is_empty() {
                    ui.label(
                        RichText::new(
                            "Nothing. Tick a title below before you move its files; \
                             untick it once it plays from its new place.",
                        )
                        .color(MUTED()),
                    );
                }
                for i in &now {
                    if let Some(on) = row(ui, i, true, cx.busy) {
                        flip = Some((i, on));
                    }
                }
            });
            ui.add_space(16.0);

            ui.horizontal(|ui| {
                ui.set_max_width(420.0);
                text_field(ui, &mut self.search, "Search titles", false);
            });
            ui.add_space(12.0);
            let q = self.search.trim().to_lowercase();
            for (kind, label) in [(Kind::Series, "Series"), (Kind::Movie, "Movies")] {
                let hits: Vec<&Item> = cx
                    .titles(kind)
                    .iter()
                    .filter(|i| q.is_empty() || i.title.to_lowercase().contains(&q))
                    .collect();
                egui::CollapsingHeader::new(bold(format!("{label}  ·  {}", hits.len()), 15.0))
                    .id_salt(("moving", label))
                    .default_open(true)
                    .show(ui, |ui| {
                        for i in hits.iter().take(SHOW) {
                            if let Some(on) = row(ui, i, marked.contains(&i.id), cx.busy) {
                                flip = Some((i, on));
                            }
                        }
                        if hits.len() > SHOW {
                            ui.label(
                                RichText::new(format!(
                                    "and {} more; search to find them",
                                    hits.len() - SHOW
                                ))
                                .color(MUTED()),
                            );
                        }
                    });
                ui.add_space(8.0);
            }

            if let Some((item, on)) = flip {
                // show it at once; the reload after the change confirms it
                if let Some(m) = self.marked.data.as_mut() {
                    if on {
                        m.insert(item.id.clone());
                    } else {
                        m.remove(&item.id);
                    }
                }
                let item = item.clone();
                cx.run(move |c| {
                    c.admin_set_moving(&item, on)?;
                    let msg = if on {
                        format!("Marked as moving (unavailable): {}", item.title)
                    } else {
                        format!("Available again: {}", item.title)
                    };
                    Ok(Done {
                        msg,
                        library: true,
                        ..Default::default()
                    })
                });
            }
        });
    }
}

/// A title with its box; `Some(ticked)` when the box changed.
fn row(ui: &mut Ui, i: &Item, on: bool, busy: bool) -> Option<bool> {
    let mut ticked = on;
    let changed = ui
        .horizontal(|ui| {
            let r = ui
                .add_enabled_ui(!busy, |ui| ui.checkbox(&mut ticked, ""))
                .inner;
            ui.label(RichText::new(&i.title).color(TEXT()));
            if let Some(y) = i.year {
                ui.label(mono(y.to_string(), 10.5).color(MUTED()));
            }
            r.changed()
        })
        .inner;
    changed.then_some(ticked)
}
