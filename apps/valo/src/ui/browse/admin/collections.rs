//! Admin › Collections: hand-made groups of titles ("Batman"). In A to Z
//! the library puts a collection's titles together under its name, oldest
//! first, so "Batman" and "The Batman" end up side by side.
use std::collections::{HashMap, HashSet};

use eframe::egui::{self, pos2, vec2, Color32, RichText, ScrollArea, Sense, Stroke, Ui};
use serde_json::json;

use super::super::details::{caps, column, padded, panel, TEAL};
use super::users::danger_button;
use super::{not_set_up, Cx, Done, Remote, Section};
use crate::api::{join, Collection, CollectionItem, Item, Kind};
use crate::ui::{bold, image_cover, mono, placeholder, primary_button, small_button, text_field};
use crate::ui::{ERROR, LINE, MUTED, SURFACE_HI, TEXT};

#[derive(Default)]
pub(in super::super) struct State {
    pub list: Remote<Vec<Collection>>,
    /// The open collection's id; `Some("")` while making a new one.
    selected: Option<String>,
    /// The open collection being edited: name and members (our item
    /// ids), in the order the app shows them.
    name: String,
    members: Vec<String>,
    /// What the server has, to tell edits apart.
    saved: (String, Vec<String>),
    /// The member being dragged to a new place, by index.
    drag: Option<usize>,
    search: String,
    /// Filters the list of collections, by name or by a title in one.
    find: String,
    confirm_delete: bool,
    /// Showing the movies without a collection instead of one collection.
    loose: bool,
    /// The ticked ones, by our id.
    picked: Vec<String>,
    /// Where they go: a collection's id, `""` for a new one.
    target: Option<String>,
    /// Asking "sort every collection by release date?".
    confirm_sort: bool,
}

/// What the server takes to save a collection.
fn save_body(name: &str, members: &[String]) -> serde_json::Value {
    let items: Vec<_> = members
        .iter()
        .filter_map(|m| {
            let (kind, id) = m.split_once(':')?;
            Some(json!({ "type": kind, "id": id }))
        })
        .collect();
    json!({ "name": name, "items": items })
}

/// A collection's members in release order, oldest first: by release date
/// where there is one, else by year; titles with neither keep their place
/// at the end.
fn chronological(members: &[String], items: &[CollectionItem], cx: &Cx) -> Vec<String> {
    let mut out = members.to_vec();
    out.sort_by_cached_key(|m| {
        let item = cx.find(m);
        let released = item.and_then(|i| i.released.clone()).unwrap_or_default();
        let year = released
            .get(..4)
            .and_then(|y| y.parse::<u32>().ok())
            .or_else(|| item.and_then(|i| i.year))
            .or_else(|| {
                items
                    .iter()
                    .find(|ci| ci.key() == *m)
                    .and_then(|ci| ci.year)
            });
        (year.is_none(), year, released)
    });
    out
}

impl State {
    pub fn refresh(&mut self) {
        self.list.retry();
    }

    pub fn missing(&self) -> bool {
        self.list.missing
    }

    fn open(&mut self, c: Option<&Collection>) {
        self.confirm_delete = false;
        self.loose = false;
        self.drag = None;
        self.search.clear();
        match c {
            Some(c) => {
                self.selected = Some(c.id.clone());
                self.name = c.name.clone();
                self.members = c.members();
            }
            None => {
                self.selected = Some(String::new());
                self.name.clear();
                self.members.clear();
            }
        }
        self.saved = (self.name.clone(), self.members.clone());
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        if self.list.poll(&mut cx.login) {
            if let Some(l) = self.list.data.as_mut() {
                l.sort_by_key(|c| c.name.to_lowercase());
            }
            // follow the server after a save, unless there are unsaved edits
            let open = self.selected.clone().filter(|s| !s.is_empty());
            let clean = (self.name.clone(), self.members.clone()) == self.saved;
            match open.and_then(|id| {
                self.list
                    .data
                    .as_ref()?
                    .iter()
                    .find(|c| c.id == id)
                    .cloned()
            }) {
                Some(c) if clean => self.open(Some(&c)),
                Some(_) => {}
                None if self.selected.as_deref() != Some("") => self.selected = None,
                None => {
                    // a new one was saved: open it by name
                    let name = self.name.trim().to_lowercase();
                    let made =
                        self.list.data.as_ref().and_then(|l| {
                            l.iter().find(|c| c.name.to_lowercase() == name).cloned()
                        });
                    if let Some(c) = made.filter(|_| clean) {
                        self.open(Some(&c));
                    }
                }
            }
        }
        if self.list.due() {
            let c = cx.client.clone();
            self.list.load(&cx.ctx, move || c.admin_collections());
        }
        if self.list.missing {
            padded(ui, |ui| {
                if not_set_up(ui, Section::Collections, &cx.client.info.name) {
                    self.list.retry();
                }
            });
            return;
        }

        let list = self.list.data.clone().unwrap_or_default();
        let mut open = None;
        let mut new = false;
        // the list on the left, the open collection beside it
        let (m, inner) = column(ui);
        let narrow = inner < 900.0;
        let list_w = if narrow { inner } else { 320.0 };
        let gap = 32.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(m);
            if narrow {
                ui.vertical(|ui| {
                    ui.set_width(inner);
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let top = ui.cursor().top();
                    self.list_panel(ui, cx, &list, &mut open, &mut new, top + 460.0);
                    ui.add_space(20.0);
                    self.editor(ui, cx);
                });
                return;
            }
            // both sides fill the window: the list stays put while the editor scrolls
            let top = ui.cursor().top();
            let h = (ui.clip_rect().bottom() - top - 48.0).max(360.0);
            ui.vertical(|ui| {
                ui.set_width(list_w);
                ui.spacing_mut().item_spacing.x = 8.0;
                self.list_panel(ui, cx, &list, &mut open, &mut new, top + h);
            });
            ui.add_space(gap);
            ui.vertical(|ui| {
                let w = inner - list_w - gap;
                ui.set_width(w);
                ui.spacing_mut().item_spacing.x = 8.0;
                ScrollArea::vertical()
                    .id_salt(("admin-collection-editor", self.selected.clone()))
                    .max_height(h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(w - 14.0); // room for the scroll bar
                        self.editor(ui, cx);
                        ui.add_space(24.0);
                    });
            });
        });
        if let Some(c) = open {
            self.open(Some(&c));
        } else if new {
            self.open(None);
        }
    }

    /// Whether `c` is called something with `q` in it, or has a title that is.
    fn matches(c: &Collection, q: &str, cx: &Cx) -> bool {
        q.is_empty()
            || c.name.to_lowercase().contains(q)
            || c.items.iter().any(|ci| {
                let title = ci
                    .title
                    .clone()
                    .or_else(|| cx.find(&ci.key()).map(|i| i.title.clone()));
                title.is_some_and(|t| t.to_lowercase().contains(q))
            })
    }

    /// Every collection, with how many titles each has; the list scrolls on
    /// its own down to `bottom`.
    fn list_panel(
        &mut self,
        ui: &mut Ui,
        cx: &mut Cx,
        list: &[Collection],
        open: &mut Option<Collection>,
        new: &mut bool,
        bottom: f32,
    ) {
        panel(ui, |ui| {
            caps(ui, "Collections", TEAL());
            ui.add_space(8.0);
            if let Some(e) = &self.list.error {
                ui.label(RichText::new(e).color(ERROR()));
            }
            ui.horizontal_wrapped(|ui| {
                if small_button(ui, "New collection").clicked() {
                    *new = true;
                }
                if small_button(ui, "Movies without a collection").clicked() {
                    self.selected = None;
                    self.loose = true;
                    self.picked.clear();
                    self.search.clear();
                }
                let b = ui
                    .add_enabled_ui(!cx.busy && !list.is_empty(), |ui| {
                        small_button(ui, "Sort all by release date")
                    })
                    .inner
                    .on_hover_text("Put every collection's titles in release order, oldest first");
                if b.clicked() {
                    self.confirm_sort = true;
                }
                let b = ui
                    .add_enabled_ui(!cx.busy, |ui| small_button(ui, "Detect from TMDB"))
                    .inner
                    .on_hover_text(
                        "Make collections from TMDB's (Batman, Harry Potter, …) for every title",
                    );
                if b.clicked() {
                    cx.run(|c| {
                        let r = c.admin_call("POST", "collections/detect", None)?;
                        let n = |k: &str| r.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
                        let running = r.get("running").and_then(|v| v.as_bool()) == Some(true);
                        Ok(Done {
                            msg: if running {
                                "Detecting collections on the server; follow the live log, \
                                 then press Refresh."
                                    .into()
                            } else {
                                format!(
                                    "Detected collections: {} new, {} updated.",
                                    n("created"),
                                    n("updated")
                                )
                            },
                            library: true,
                            ..Default::default()
                        })
                    });
                }
            });
            if self.confirm_sort {
                self.sort_all_confirm(ui, cx, list);
            }
            ui.add_space(10.0);
            text_field(ui, &mut self.find, "Search collections", false);
            ui.add_space(8.0);
            let q = self.find.trim().to_lowercase();
            let shown: Vec<&Collection> =
                list.iter().filter(|c| Self::matches(c, &q, cx)).collect();
            let height = (bottom - ui.cursor().top() - 16.0).max(200.0);
            ScrollArea::vertical()
                .id_salt("admin-collection-list")
                .max_height(height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if list.is_empty() && self.list.data.is_some() {
                        ui.label(RichText::new("None yet.").color(MUTED()));
                    } else if shown.is_empty() && !list.is_empty() {
                        ui.label(RichText::new("No collections match.").color(MUTED()));
                    }
                    for c in shown {
                        let n = c.members().len();
                        let sel = self.selected.as_deref() == Some(c.id.as_str());
                        if nav_row(ui, &c.name, n, sel) {
                            *open = Some(c.clone());
                        }
                    }
                });
        });
    }

    /// "Sort every collection?", and saving the ones whose order changes.
    fn sort_all_confirm(&mut self, ui: &mut Ui, cx: &mut Cx, list: &[Collection]) {
        ui.add_space(10.0);
        ui.label(
            RichText::new(
                "Put the titles of every collection in release order, oldest first? \
                 Orders you set by hand are replaced.",
            )
            .color(TEXT()),
        );
        ui.horizontal(|ui| {
            if ui
                .add_enabled_ui(!cx.busy, |ui| primary_button(ui, "Sort all", true))
                .inner
                .clicked()
            {
                let changes: Vec<(String, serde_json::Value)> = list
                    .iter()
                    .filter_map(|c| {
                        let members = c.members();
                        let sorted = chronological(&members, &c.items, cx);
                        (sorted != members).then(|| (c.id.clone(), save_body(&c.name, &sorted)))
                    })
                    .collect();
                cx.run(move |c| {
                    for (id, body) in &changes {
                        c.admin_save_collection(Some(id), body)?;
                    }
                    Ok(Done {
                        msg: match changes.len() {
                            0 => "Every collection was already in release order.".into(),
                            1 => "Sorted 1 collection by release date.".into(),
                            n => format!("Sorted {n} collections by release date."),
                        },
                        library: !changes.is_empty(),
                        ..Default::default()
                    })
                });
                self.confirm_sort = false;
            }
            if small_button(ui, "Cancel").clicked() {
                self.confirm_sort = false;
            }
        });
    }

    /// Movies in no collection: tick some and add them to one.
    fn loose_movies(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let list = self.list.data.clone().unwrap_or_default();
        let taken: HashSet<String> = list.iter().flat_map(Collection::members).collect();
        let mut loose: Vec<&Item> = cx
            .movies
            .iter()
            .filter(|i| i.collection.is_none() && !taken.contains(&i.id))
            .collect();
        loose.sort_by_key(|i| i.title.to_lowercase());
        self.picked.retain(|id| loose.iter().any(|i| i.id == *id));

        caps(ui, "Movies without a collection", TEAL());
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!(
                "{} movies aren't in a collection. Tick the ones that belong together and \
                 add them to a collection.",
                loose.len()
            ))
            .color(MUTED()),
        );
        ui.add_space(10.0);
        text_field(ui, &mut self.search, "Search movies", false);
        ui.add_space(8.0);
        let q = self.search.trim().to_lowercase();
        let shown: Vec<&Item> = loose
            .into_iter()
            .filter(|i| q.is_empty() || i.title.to_lowercase().contains(&q))
            .collect();
        ui.horizontal(|ui| {
            if small_button(ui, "Tick all shown").clicked() {
                for i in &shown {
                    if !self.picked.contains(&i.id) {
                        self.picked.push(i.id.clone());
                    }
                }
            }
            if !self.picked.is_empty() && small_button(ui, "Untick all").clicked() {
                self.picked.clear();
            }
        });
        ui.add_space(6.0);
        egui::Frame::new()
            .stroke(Stroke::new(1.0f32, LINE()))
            .corner_radius(8)
            .inner_margin(6)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ScrollArea::vertical()
                    .id_salt("collection-loose")
                    .max_height(420.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if shown.is_empty() {
                            ui.label(RichText::new("None.").color(MUTED()));
                        }
                        for i in &shown {
                            let mut on = self.picked.contains(&i.id);
                            let year = i.year.map(|y| format!(" ({y})")).unwrap_or_default();
                            if ui.checkbox(&mut on, format!("{}{year}", i.title)).changed() {
                                if on {
                                    self.picked.push(i.id.clone());
                                } else {
                                    self.picked.retain(|p| *p != i.id);
                                }
                            }
                        }
                    });
            });
        ui.add_space(10.0);

        // where they go
        let target = self
            .target
            .clone()
            .filter(|t| t.is_empty() || list.iter().any(|c| c.id == *t));
        let target_name = match target.as_deref() {
            None => "Choose a collection…".to_owned(),
            Some("") => "New collection".to_owned(),
            Some(t) => list
                .iter()
                .find(|c| c.id == t)
                .map_or(String::new(), |c| c.name.clone()),
        };
        let mut add = false;
        ui.horizontal(|ui| {
            ui.label(mono("Add to", 10.5).color(MUTED()));
            egui::ComboBox::from_id_salt("collection-target")
                .width(220.0)
                .selected_text(target_name)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.target, Some(String::new()), "New collection");
                    for c in &list {
                        ui.selectable_value(&mut self.target, Some(c.id.clone()), &c.name);
                    }
                });
            let n = self.picked.len();
            let label = match n {
                1 => "Add 1 movie".to_owned(),
                n => format!("Add {n} movies"),
            };
            add = ui
                .add_enabled_ui(n > 0 && target.is_some(), |ui| {
                    primary_button(ui, &label, true)
                })
                .inner
                .clicked();
        });
        ui.label(
            RichText::new("The collection opens with them added; press Save to keep it.")
                .small()
                .color(MUTED()),
        );
        if add {
            let picked = std::mem::take(&mut self.picked);
            match target
                .as_deref()
                .and_then(|t| list.iter().find(|c| c.id == t))
            {
                Some(c) => self.open(Some(c)),
                None => self.open(None),
            }
            for id in picked {
                if !self.members.contains(&id) {
                    self.members.push(id);
                }
            }
        }
    }

    fn editor(&mut self, ui: &mut Ui, cx: &mut Cx) {
        if self.loose {
            return self.loose_movies(ui, cx);
        }
        let Some(id) = self.selected.clone() else {
            panel(ui, |ui| {
                caps(ui, "How collections work", TEAL());
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Titles in a collection are sorted under the collection's name in \
                         A to Z, oldest first. So Batman (1989), Batman Returns and The \
                         Batman sit next to each other under B, instead of spread across \
                         the alphabet.",
                    )
                    .color(MUTED()),
                );
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Pick one from the list, or make a new one.").color(MUTED()),
                );
            });
            return;
        };
        let is_new = id.is_empty();
        let busy = cx.busy;
        let dirty = (self.name.clone(), self.members.clone()) != self.saved;

        caps(
            ui,
            if is_new {
                "New collection"
            } else {
                "Collection"
            },
            TEAL(),
        );
        ui.add_space(6.0);
        ui.label(mono("Name", 10.5).color(MUTED()));
        text_field(ui, &mut self.name, "Batman", false);
        ui.add_space(12.0);

        // members, in the order the app shows them; drag to reorder
        ui.horizontal(|ui| {
            ui.label(bold("Titles", 20.0));
            ui.label(mono(self.members.len().to_string(), 12.0).color(MUTED()));
        });
        ui.label(
            RichText::new("Shown in this order in the app, top first. Drag a title to move it.")
                .small()
                .color(MUTED()),
        );
        ui.add_space(8.0);
        if self.members.is_empty() {
            ui.label(RichText::new("Add titles from the list below.").color(MUTED()));
        }
        self.member_rows(ui, cx);
        ui.add_space(10.0);

        // actions
        ui.horizontal(|ui| {
            let ok = !busy && dirty && !self.name.trim().is_empty();
            if ui
                .add_enabled_ui(ok, |ui| {
                    primary_button(
                        ui,
                        if is_new {
                            "Create collection"
                        } else {
                            "Save changes"
                        },
                        true,
                    )
                })
                .inner
                .clicked()
            {
                let name = self.name.trim().to_owned();
                let body = save_body(&name, &self.members);
                let id = (!is_new).then(|| id.clone());
                cx.run(move |c| {
                    c.admin_save_collection(id.as_deref(), &body)?;
                    let verb = if id.is_some() { "Saved" } else { "Created" };
                    Ok(Done {
                        msg: format!("{verb} {name}."),
                        library: true,
                        ..Default::default()
                    })
                });
                self.saved = (self.name.clone(), self.members.clone());
            }
            if dirty && !is_new && small_button(ui, "Undo changes").clicked() {
                (self.name, self.members) = self.saved.clone();
            }
            if !is_new && small_button(ui, "Delete…").clicked() {
                self.confirm_delete = true;
            }
            if is_new && small_button(ui, "Cancel").clicked() {
                self.selected = None;
            }
        });
        if self.confirm_delete {
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!(
                    "Delete {}? Its titles stay in the library; they just go back to their \
                     own place in A to Z.",
                    self.saved.0
                ))
                .color(TEXT()),
            );
            ui.horizontal(|ui| {
                if ui
                    .add_enabled_ui(!busy, |ui| danger_button(ui, "Delete"))
                    .inner
                    .clicked()
                {
                    let name = self.saved.0.clone();
                    let id = id.clone();
                    cx.run(move |c| {
                        c.admin_delete_collection(&id)?;
                        Ok(Done {
                            msg: format!("Deleted {name}."),
                            library: true,
                            ..Default::default()
                        })
                    });
                    self.confirm_delete = false;
                    self.selected = None;
                }
                if small_button(ui, "Cancel").clicked() {
                    self.confirm_delete = false;
                }
            });
        }
        ui.add_space(24.0);

        // adding titles
        caps(ui, "Add titles", TEAL());
        ui.add_space(6.0);
        text_field(ui, &mut self.search, "Search movies and series", false);
        ui.add_space(8.0);
        let q = self.search.trim().to_lowercase();
        let mut found: Vec<&Item> = cx
            .movies
            .iter()
            .chain(cx.series)
            .filter(|i| q.is_empty() || i.title.to_lowercase().contains(&q))
            .collect();
        found.sort_by_key(|i| i.title.to_lowercase());
        let mut toggle = None;
        egui::Frame::new()
            .stroke(Stroke::new(1.0f32, LINE()))
            .corner_radius(8)
            .inner_margin(6)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ScrollArea::vertical()
                    .id_salt("collection-add")
                    .max_height(320.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for i in found {
                            let mut on = self.members.contains(&i.id);
                            let year = i.year.map(|y| format!(" ({y})")).unwrap_or_default();
                            let mut label = format!("{}{year}", i.title);
                            if let Some(c) = i.collection.as_ref().filter(|c| **c != self.saved.0) {
                                label.push_str(&format!("   · in {c}"));
                            }
                            if ui.checkbox(&mut on, label).changed() {
                                toggle = Some((i.id.clone(), on));
                            }
                        }
                    });
            });
        if let Some((id, on)) = toggle {
            if on {
                self.members.push(id);
            } else {
                self.members.retain(|m| *m != id);
            }
        }
    }
}

fn nav_row(ui: &mut Ui, name: &str, n: usize, selected: bool) -> bool {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(r, 4.0, crate::ui::PRIMARY());
    } else if resp.hovered() {
        p.rect_filled(r, 4.0, SURFACE_HI());
    }
    let col = if selected { crate::ui::INK() } else { TEXT() };
    p.text(
        r.left_center() + vec2(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(14.0),
        col,
    );
    p.text(
        r.right_center() - vec2(10.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        n.to_string(),
        egui::FontId::monospace(11.0),
        if selected {
            crate::ui::INK().gamma_multiply(0.6)
        } else {
            MUTED()
        },
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

impl State {
    /// The members as rows: drag one to a new place, or remove it.
    fn member_rows(&mut self, ui: &mut Ui, cx: &mut Cx) {
        // what the server says about titles neither list has (no file yet)
        let extra: HashMap<String, Item> = self
            .list
            .data
            .iter()
            .flatten()
            .flat_map(|c| &c.items)
            .filter(|ci| ci.title.is_some())
            .map(|ci| {
                let item = Item {
                    id: ci.key(),
                    title: ci.title.clone().unwrap_or_default(),
                    year: ci.year,
                    poster: ci.poster.as_ref().map(|p| join(&cx.client.base, p)),
                    kind: if ci.kind == "series" {
                        Kind::Series
                    } else {
                        Kind::Movie
                    },
                    ..Default::default()
                };
                (ci.key(), item)
            })
            .collect();
        let w = ui.available_width();
        let mut rects = Vec::new();
        let mut remove = None;
        let mut shift: Option<(usize, usize)> = None;
        let n = self.members.len();
        for (idx, id) in self.members.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(vec2(w, 58.0), Sense::click_and_drag());
            rects.push(r);
            if resp.drag_started() {
                self.drag = Some(idx);
            }
            let dragged = self.drag == Some(idx);
            let p = ui.painter();
            p.rect_filled(
                r,
                6.0,
                if dragged || resp.hovered() {
                    SURFACE_HI()
                } else {
                    Color32::TRANSPARENT
                },
            );
            p.rect_stroke(
                r,
                6.0,
                Stroke::new(1.0f32, LINE()),
                egui::StrokeKind::Inside,
            );
            // grip
            for dy in [-5.0, 0.0, 5.0] {
                let y = r.center().y + dy;
                p.hline(
                    r.left() + 10.0..=r.left() + 22.0,
                    y,
                    Stroke::new(1.5f32, MUTED()),
                );
            }
            p.text(
                pos2(r.left() + 34.0, r.center().y),
                egui::Align2::LEFT_CENTER,
                format!("{}", idx + 1),
                egui::FontId::monospace(11.0),
                MUTED(),
            );
            let art =
                egui::Rect::from_min_size(pos2(r.left() + 56.0, r.top() + 6.0), vec2(31.0, 46.0));
            let item: Option<&Item> = cx.find(id).or_else(|| extra.get(id));
            let title = item.map_or("(not in the library)", |i| i.title.as_str());
            match item
                .and_then(|i| i.poster.as_ref())
                .and_then(|u| cx.images.get(u))
            {
                Some(t) => image_cover(p, art, &t, 3, Color32::WHITE),
                None => placeholder(p, art, title, 3),
            }
            let clip = p.with_clip_rect(r.shrink(2.0));
            clip.text(
                pos2(art.right() + 12.0, r.center().y - 9.0),
                egui::Align2::LEFT_CENTER,
                title,
                egui::FontId::proportional(14.5),
                TEXT(),
            );
            let sub = item.map(|i| {
                let kind = if i.kind == Kind::Movie {
                    "Movie"
                } else {
                    "Series"
                };
                i.year.map_or(kind.to_owned(), |y| format!("{kind} · {y}"))
            });
            if let Some(sub) = sub {
                clip.text(
                    pos2(art.right() + 12.0, r.center().y + 10.0),
                    egui::Align2::LEFT_CENTER,
                    sub,
                    egui::FontId::monospace(10.0),
                    MUTED(),
                );
            }
            // buttons on the right: remove, down, up
            let mut x = r.right() - 8.0;
            for (label, act) in [("Remove", 0), ("down", 1), ("up", 2)] {
                let bw = if act == 0 { 70.0 } else { 30.0 };
                let br = egui::Rect::from_min_max(
                    pos2(x - bw, r.center().y - 14.0),
                    pos2(x, r.center().y + 14.0),
                );
                x -= bw + 6.0;
                let enabled = match act {
                    1 => idx + 1 < n,
                    2 => idx > 0,
                    _ => true,
                };
                if !enabled {
                    continue;
                }
                let b = ui.interact(br, ui.id().with(("member-btn", idx, act)), Sense::click());
                let p = ui.painter();
                if b.hovered() {
                    p.rect_filled(br, 2.0, Color32::from_white_alpha(14));
                }
                p.rect_stroke(
                    br,
                    2.0,
                    Stroke::new(1.0f32, crate::ui::LINE_HI()),
                    egui::StrokeKind::Inside,
                );
                match act {
                    0 => {
                        p.text(
                            br.center(),
                            egui::Align2::CENTER_CENTER,
                            label.to_uppercase(),
                            egui::FontId::monospace(10.5),
                            ERROR(),
                        );
                    }
                    1 => crate::ui::chrome::draw_icon(
                        p,
                        br.center(),
                        crate::ui::chrome::Icon::ChevronDown,
                        TEXT(),
                    ),
                    _ => crate::ui::chrome::draw_icon(
                        p,
                        br.center(),
                        crate::ui::chrome::Icon::ChevronUp,
                        TEXT(),
                    ),
                }
                if b.clicked() {
                    match act {
                        0 => remove = Some(idx),
                        1 => shift = Some((idx, idx + 2)),
                        _ => shift = Some((idx, idx - 1)),
                    }
                }
            }
            if !dragged {
                resp.on_hover_cursor(egui::CursorIcon::Grab);
            }
            ui.add_space(4.0);
        }

        // dropping: the gap nearest the pointer
        if let Some(from) = self.drag {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if let (Some(pos), false) = (ui.ctx().pointer_interact_pos(), rects.is_empty()) {
                let to = rects
                    .iter()
                    .position(|r| pos.y < r.center().y)
                    .unwrap_or(rects.len());
                let y = match rects.get(to) {
                    Some(r) => r.top() - 2.0,
                    None => rects[rects.len() - 1].bottom() + 2.0,
                };
                ui.painter()
                    .hline(rects[0].x_range(), y, Stroke::new(2.0f32, TEAL()));
                if ui.input(|i| i.pointer.any_released()) {
                    shift = Some((from, to));
                    self.drag = None;
                }
            }
            if !ui.input(|i| i.pointer.any_down() || i.pointer.any_released()) {
                self.drag = None;
            }
        }
        if let Some((from, to)) = shift {
            if from < self.members.len() {
                let m = self.members.remove(from);
                let to = if to > from { to - 1 } else { to };
                self.members.insert(to.min(self.members.len()), m);
            }
        }
        if let Some(i) = remove {
            self.members.remove(i);
        }
    }
}
