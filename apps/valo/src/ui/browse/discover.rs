//! What the admin's Stremio addons add to browsing: catalog rows on the home
//! page, details for titles that aren't on the server (with a Request
//! button), and details our own titles are missing (filled in, never
//! replaced).
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use eframe::egui::{self, pos2, vec2, Rect, RichText, Sense, Ui};

use super::details::{
    backdrop, caps, cast_grid, column, dot, fill_button, placed, play_colors, tag, title_art, GOLD,
    TEAL,
};
use super::{Browse, Click};
use crate::addons::{self, imdb_id, kind_name, split_remote, Row};
use crate::api::{Addon, Client, Item, Kind, Library, MediaRequest, NoAdminApi};
use crate::images::Images;
use crate::ui::chrome::{self, Icon};
use crate::ui::{bold, image_cover, mono, placeholder};
use crate::ui::{ERROR, MUTED, OK, TEXT, WARN};
use crate::util::{fmt_runtime, Task};

/// Metadata lookups per batch.
const BATCH: usize = 24;

#[derive(Default)]
pub(super) struct State {
    load: Option<Task<Result<addons::Catalogs>>>,
    /// The addon list has come in (possibly empty).
    loaded: bool,
    addons: Vec<Addon>,
    rows: Vec<Row>,
    /// Each row as item ids: ours where the IMDb id matches, else the addon's.
    pub shown: Vec<(String, Vec<String>)>,
    /// Addon titles, by [`addons::remote_id`].
    pub remote: HashMap<String, Item>,
    /// From metadata addons, by "movie:tt1375666" (None: none of them has it).
    metas: HashMap<String, Option<Item>>,
    want: Vec<(String, String)>,
    asked: HashSet<String>,
    fetching: Option<Task<addons::Metas>>,
    /// The user's own requests; Err: the server doesn't take requests.
    requests: Option<std::result::Result<Vec<MediaRequest>, String>>,
    requests_task: Option<Task<Result<Vec<MediaRequest>>>>,
    sending: Option<Task<Result<MediaRequest>>>,
    send_error: Option<String>,
}

impl State {
    pub fn reload(&mut self, ctx: &egui::Context, client: &Client) {
        let c = client.clone();
        self.load = Some(Task::spawn(ctx, move || addons::catalogs(&c)));
        self.requests = None;
    }

    fn has_meta(&self) -> bool {
        self.addons
            .iter()
            .any(|a| a.resources.iter().any(|r| r.name == "meta"))
    }

    /// Fill a freshly loaded library from what's known, and ask for the
    /// rest of what its cards are missing.
    pub fn on_library(&mut self, lib: &mut Library) {
        self.apply(lib);
        self.queue_gaps(lib);
        self.rebuild(lib);
    }

    fn apply(&self, lib: &mut Library) {
        for it in lib.movies.iter_mut().chain(lib.series.iter_mut()) {
            let key = it
                .imdb_id
                .as_deref()
                .and_then(imdb_id)
                .map(|id| format!("{}:{id}", kind_name(it.kind)));
            if let Some(Some(m)) = key.and_then(|k| self.metas.get(&k)) {
                addons::fill(it, m);
            }
        }
    }

    /// Titles without a poster, backdrop or overview.
    fn queue_gaps(&mut self, lib: &Library) {
        if !self.has_meta() {
            return;
        }
        let gaps: Vec<(Kind, String)> = lib
            .all()
            .filter(|i| i.poster.is_none() || i.backdrop.is_none() || i.overview.is_empty())
            .filter_map(|i| Some((i.kind, imdb_id(i.imdb_id.as_deref()?)?)))
            .collect();
        for (k, id) in gaps {
            self.want(kind_name(k), &id);
        }
    }

    /// Ask the metadata addons about this title (once).
    pub fn want(&mut self, kind: &str, id: &str) {
        let key = format!("{kind}:{id}");
        if self.loaded && !self.has_meta() {
            return;
        }
        if !self.metas.contains_key(&key) && self.asked.insert(key) {
            self.want.push((kind.to_owned(), id.to_owned()));
        }
    }

    /// Our title's full details, the first time its page opens.
    pub fn want_item(&mut self, item: &Item) {
        if let Some(id) = item.imdb_id.as_deref().and_then(imdb_id) {
            self.want(kind_name(item.kind), &id);
        }
    }

    /// The rows as ids, matching addon titles to ours by IMDb id.
    fn rebuild(&mut self, lib: &Library) {
        let ours: HashMap<(Kind, &str), &str> = lib
            .all()
            .filter_map(|i| Some(((i.kind, i.imdb_id.as_deref()?), i.id.as_str())))
            .collect();
        self.shown = self
            .rows
            .iter()
            .map(|r| {
                let ids = r
                    .items
                    .iter()
                    .map(|it| {
                        let mine = it
                            .imdb_id
                            .as_deref()
                            .and_then(|id| ours.get(&(it.kind, id)));
                        match mine {
                            Some(id) => (*id).to_owned(),
                            None => {
                                let mut it = it.clone();
                                if let Some(Some(m)) = split_remote(&it.id)
                                    .and_then(|(k, id)| self.metas.get(&format!("{k}:{id}")))
                                {
                                    addons::fill(&mut it, m);
                                }
                                let id = it.id.clone();
                                self.remote.entry(id.clone()).or_insert(it);
                                id
                            }
                        }
                    })
                    .collect();
                (r.title.clone(), ids)
            })
            .collect();
    }

    /// Take finished loads; fills `lib` when new details came in.
    pub fn poll(&mut self, ctx: &egui::Context, client: &Client, lib: &mut Option<Arc<Library>>) {
        if let Some(r) = self.load.as_mut().and_then(|t| t.poll()) {
            self.load = None;
            self.loaded = true;
            let (addons, rows) = r.unwrap_or_default();
            self.addons = addons;
            self.rows = rows;
            self.remote.clear();
            if let Some(l) = lib.as_ref() {
                let l = l.clone();
                self.queue_gaps(&l);
                self.rebuild(&l);
            }
        }
        if let Some(found) = self.fetching.as_mut().and_then(|t| t.poll()) {
            self.fetching = None;
            for (key, m) in found {
                if let Some(it) = &m {
                    // a remote title's page shows the full details
                    if let Some(r) = self.remote.get_mut(&format!("addon:{key}")) {
                        addons::fill(r, it);
                    }
                }
                self.metas.insert(key, m);
            }
            if let Some(l) = lib.as_mut() {
                self.apply(Arc::make_mut(l));
            }
        }
        if self.fetching.is_none() && self.loaded && !self.want.is_empty() {
            let n = self.want.len().min(BATCH);
            let batch: Vec<(String, String)> = self.want.drain(..n).collect();
            let a = self.addons.clone();
            self.fetching = Some(Task::spawn(ctx, move || addons::metas(&a, &batch)));
        }
        if let Some(r) = self.requests_task.as_mut().and_then(|t| t.poll()) {
            self.requests_task = None;
            self.requests = Some(r.map_err(|e| {
                if e.is::<NoAdminApi>() {
                    "This server doesn't take requests.".to_owned()
                } else {
                    format!("{e:#}")
                }
            }));
        }
        if let Some(r) = self.sending.as_mut().and_then(|t| t.poll()) {
            self.sending = None;
            match r {
                Ok(req) => {
                    if let Some(Ok(l)) = self.requests.as_mut() {
                        l.push(req);
                    }
                    let c = client.clone();
                    self.requests_task = Some(Task::spawn(ctx, move || c.my_requests()));
                }
                Err(e) if e.is::<NoAdminApi>() => {
                    self.send_error = Some("This server doesn't take requests.".into())
                }
                Err(e) => self.send_error = Some(format!("{e:#}")),
            }
        }
    }

    /// The user's request for this title, if they made one.
    fn request_for(&self, item: &Item) -> Option<&MediaRequest> {
        let Some(Ok(list)) = &self.requests else {
            return None;
        };
        let title = item.title.to_lowercase();
        list.iter()
            .rev()
            .find(|r| match (&r.imdb_id, &item.imdb_id) {
                (Some(a), Some(b)) => a == b,
                _ => r.title.to_lowercase() == title && (r.year.is_none() || r.year == item.year),
            })
    }
}

impl Browse {
    /// A title from an addon that isn't on the server: its details and a
    /// Request button.
    pub(super) fn remote_page(
        &mut self,
        ui: &mut Ui,
        item: &Item,
        images: &mut Images,
    ) -> Option<Click> {
        let mut click = None;
        let ctx = ui.ctx().clone();
        let st = &mut self.discover;
        if let Some((k, id)) = split_remote(&item.id) {
            st.want(k, id);
        }
        if st.requests.is_none() && st.requests_task.is_none() && self.client.token.is_some() {
            let c = self.client.clone();
            st.requests_task = Some(Task::spawn(&ctx, move || c.my_requests()));
        }

        let w = ui.available_width();
        let h = (w * 0.42).clamp(440.0, 620.0);
        let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        let art = item.backdrop.as_ref().or(item.poster.as_ref());
        let tex = art.and_then(|u| images.get(u));
        backdrop(&ui.painter_at(rect), rect, tex.as_ref(), &item.title, 140);
        let back = Rect::from_min_size(rect.min + vec2(28.0, 22.0), vec2(300.0, 24.0));
        placed(ui, back, "remote_back", 0.0, |ui| {
            let r = super::details::link(
                ui,
                "Back",
                13.0,
                TEXT().gamma_multiply(0.8),
                Some((Icon::ChevronLeft, false)),
            );
            if r.clicked() {
                click = Some(Click::Back);
            }
        });

        let (m, inner) = column(ui);
        let left = rect.left() + m;
        let show_poster = inner >= 860.0;
        let poster_w = if show_poster {
            (inner * 0.2).clamp(200.0, 280.0)
        } else {
            0.0
        };
        if show_poster {
            let ph = poster_w * 1.5;
            let pr =
                Rect::from_min_size(pos2(left, rect.center().y - ph / 2.0), vec2(poster_w, ph));
            let p = ui.painter_at(rect);
            match item.poster.as_ref().and_then(|u| images.get(u)) {
                Some(t) => image_cover(&p, pr, &t, 8, egui::Color32::WHITE),
                None => placeholder(&p, pr, &item.title, 8),
            }
        }
        let tx = left + if show_poster { poster_w + 40.0 } else { 0.0 };
        let text_rect = Rect::from_min_max(
            pos2(tx, rect.top() + 60.0),
            pos2(left + inner, rect.bottom() - 30.0),
        );
        let st = &mut self.discover;
        let client = self.client.clone();
        placed(ui, text_rect, "remote_hero", 0.5, |ui| {
            let mut kinds = vec![match item.kind {
                Kind::Movie => "Movie".to_owned(),
                Kind::Series => "Series".to_owned(),
            }];
            if !item.genres.is_empty() {
                kinds.push(item.genres.join(" · "));
            }
            caps(ui, &kinds.join("   ·   "), TEXT().gamma_multiply(0.65));
            ui.add_space(8.0);
            title_art(ui, item, images, vec2(560.0, 96.0), 50.0);
            ui.add_space(14.0);
            if !item.overview.is_empty() {
                ui.add(
                    egui::Label::new(
                        RichText::new(&item.overview)
                            .size(16.0)
                            .color(TEXT().gamma_multiply(0.75)),
                    )
                    .wrap(),
                );
                ui.add_space(14.0);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let mut first = true;
                let mut sep = |ui: &mut Ui| {
                    if !std::mem::take(&mut first) {
                        dot(ui);
                    }
                };
                if let Some(s) = item.score {
                    sep(ui);
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    chrome::draw_icon(ui.painter(), r.center(), Icon::Star, GOLD());
                    ui.label(RichText::new(format!("{s:.1}")).size(15.0).color(TEXT()));
                    if let Some(src) = &item.score_source {
                        ui.label(RichText::new(src).size(15.0).color(MUTED()));
                    }
                }
                if let Some(y) = item.year {
                    sep(ui);
                    let years = match item.end_year {
                        Some(e) if e != y => format!("{y}–{e}"),
                        _ => y.to_string(),
                    };
                    ui.label(RichText::new(years).size(15.0).color(GOLD()));
                }
                if let Some(d) = item.duration {
                    sep(ui);
                    ui.label(RichText::new(fmt_runtime(d)).size(15.0).color(TEXT()));
                }
                if let Some(d) = &item.director {
                    sep(ui);
                    tag(ui, d);
                }
            });
            ui.add_space(22.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                match st.request_for(item) {
                    Some(req) => {
                        let (col, text) = match req.status.as_str() {
                            "approved" => (TEAL(), "Requested · approved, on its way"),
                            "available" => (OK(), "Requested · now available"),
                            "denied" => (ERROR(), "Requested · denied"),
                            _ => (WARN(), "Requested · waiting for the admin"),
                        };
                        ui.painter().circle_filled(r.center(), 3.5, col);
                        ui.label(RichText::new(text).size(13.0).color(col));
                        if let Some(n) = req.note.as_ref().filter(|n| !n.trim().is_empty()) {
                            ui.label(RichText::new(format!("“{n}”")).italics().color(MUTED()));
                        }
                    }
                    None => {
                        ui.painter().circle_filled(r.center(), 3.5, MUTED());
                        ui.label(RichText::new("Not on the server").size(13.0).color(MUTED()));
                        ui.add_space(8.0);
                        match &st.requests {
                            Some(Err(e)) => {
                                ui.label(RichText::new(e).size(13.0).color(MUTED()));
                            }
                            _ if client.token.is_none() => {}
                            _ => {
                                let sending = st.sending.is_some();
                                let label = if sending { "Requesting…" } else { "Request" };
                                let b = fill_button(ui, label, None, play_colors(), 42.0, 0.0, 0.0);
                                if b.clicked() && !sending {
                                    st.send_error = None;
                                    let (c, it) = (client.clone(), item.clone());
                                    st.sending =
                                        Some(Task::spawn(&ctx, move || c.request_title(&it)));
                                }
                            }
                        }
                    }
                }
            });
            if let Some(e) = &st.send_error {
                ui.add_space(8.0);
                ui.label(RichText::new(format!("Couldn't send the request: {e}")).color(ERROR()));
            }
        });

        if !item.cast.is_empty() {
            ui.add_space(28.0);
            ui.horizontal_top(|ui| {
                ui.add_space(m);
                ui.vertical(|ui| {
                    ui.set_width(inner);
                    caps(ui, "Cast", TEAL());
                    ui.add_space(4.0);
                    ui.label(bold("Top billed", 24.0));
                    ui.add_space(16.0);
                    let n = item.cast.len().min(10);
                    if let Some(c) = cast_grid(ui, &item.cast[..n], images, 150.0, 50.0) {
                        click = Some(c);
                    }
                });
            });
        }
        if self.discover.load.is_some() || self.discover.fetching.is_some() {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(m);
                ui.label(mono("Loading details…", 11.0).color(MUTED()));
            });
        }
        click
    }
}
