//! "Viewed by other users": what everyone else played lately, from `GET
//! /api/watchers`. A row on the home page, and a page of its own that only
//! the row's See all opens.
use std::time::Instant;

use anyhow::Result;
use eframe::egui::{self, RichText, Ui};

use super::details::{caps, TEAL};
use super::{grid, Browse, Card, Click, SIDE};
use crate::api::{Library, Viewed, Watcher};
use crate::config::Progress;
use crate::images::Images;
use crate::ui::{bold, mono, MUTED};
use crate::util::Task;

pub(super) const TITLE: &str = "Viewed by other users";

#[derive(Default)]
pub(super) struct State {
    /// None until loaded, or when the server doesn't say.
    list: Option<Vec<Viewed>>,
    task: Option<Task<Result<Option<Vec<Viewed>>>>>,
    at: Option<Instant>,
}

impl State {
    fn loading(&self) -> bool {
        self.task.is_some() && self.list.is_none()
    }
}

/// "Jo", "Jo +2": who played it, for under the poster.
fn names(w: &[Watcher]) -> Option<String> {
    let first = w.first()?.shown();
    Some(match w.len() {
        1 => first.to_owned(),
        n => format!("{first} +{}", n - 1),
    })
}

impl Browse {
    /// Load the list, and again every minute while it's on screen.
    fn poll_viewed(&mut self, ctx: &egui::Context) {
        let v = &mut self.viewed;
        if let Some(r) = v.task.as_mut().and_then(|t| t.poll()) {
            v.task = None;
            v.at = Some(Instant::now());
            if let Ok(list) = r {
                v.list = list;
            }
        }
        if v.task.is_none() && v.at.is_none_or(|t| t.elapsed().as_secs() >= 60) {
            let c = self.client.clone();
            v.task = Some(Task::spawn(ctx, move || c.viewed()));
        }
    }

    /// Posters of what others played, latest first, with who under each.
    pub(super) fn viewed_cards(
        &mut self,
        ctx: &egui::Context,
        lib: &Library,
        progress: &Progress,
    ) -> Vec<Card> {
        self.poll_viewed(ctx);
        let Some(list) = self.viewed.list.as_ref() else {
            return Vec::new();
        };
        let found: Vec<_> = list
            .iter()
            .filter_map(|v| Some((lib.find(&v.key()?)?, &v.watchers)))
            .collect();
        let mut cards = self.poster_cards(found.iter().map(|(i, _)| *i), progress);
        for (c, (_, w)) in cards.iter_mut().zip(&found) {
            if let Some(n) = names(w) {
                c.sub = Some(n);
            }
        }
        cards
    }

    /// The page See all opens: everything in the row.
    pub(super) fn viewed_page(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        images: &mut Images,
        progress: &Progress,
    ) -> Option<Click> {
        let cards = self.viewed_cards(ui.ctx(), lib, progress);
        ui.add_space(32.0);
        ui.horizontal(|ui| {
            ui.add_space(SIDE);
            ui.vertical(|ui| {
                caps(ui, "From other users", TEAL());
                ui.add_space(4.0);
                ui.label(bold(TITLE, 34.0));
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "What everyone else on this server watched lately, latest first.",
                    )
                    .color(MUTED()),
                );
            });
        });
        ui.add_space(24.0);
        if cards.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                if self.viewed.loading() {
                    ui.add(egui::Spinner::new().size(24.0).color(MUTED()));
                } else {
                    ui.label(mono("Nobody else has watched anything yet", 12.0).color(MUTED()));
                }
            });
            return None;
        }
        let marked = |id: &str| self.watchlist.contains(&self.key(id));
        grid(ui, &cards, &marked, images)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_under_the_poster() {
        let w = |n: &str| Watcher {
            name: n.into(),
            ..Default::default()
        };
        assert_eq!(names(&[]), None);
        assert_eq!(names(&[w("jo")]).as_deref(), Some("jo"));
        assert_eq!(
            names(&[w("jo"), w("sam"), w("kim")]).as_deref(),
            Some("jo +2")
        );
    }
}
