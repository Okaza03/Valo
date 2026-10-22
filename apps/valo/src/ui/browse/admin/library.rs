//! Admin › Library: edit a title's details and artwork by hand, or fetch
//! them from IMDb and check them before saving.
mod extras;

use std::path::Path;

use anyhow::{Context, Result};
use eframe::egui::{self, vec2, Color32, RichText, ScrollArea, Sense, Stroke, Ui};
use serde_json::{json, Map, Value};

use super::super::details::{caps, column, panel, TEAL};
use super::{needs_note, not_set_up, Cx, Done, Remote, Section};
use crate::api::{Item, Kind, NoAdminApi, Unauthorized};
use crate::ui::{
    bold, image_cover, mono, placeholder, primary_button, segmented, small_button, text_field,
};
use crate::ui::{ERROR, LINE, MUTED, OK, SURFACE_HI, TEXT, WARN};
use crate::util::Task;

/// The editor's fields, as text.
#[derive(Clone, Default, PartialEq)]
struct Form {
    title: String,
    year: String,
    overview: String,
    /// Comma-separated.
    genres: String,
    rating: String,
    /// Director (movies) or creators, comma-separated (series).
    maker: String,
    released: String,
    end_year: String,
    network: String,
    status: String,
    score: String,
    imdb_id: String,
    poster: String,
    backdrop: String,
    thumb: String,
    logo: String,
}

impl Form {
    fn from_item(i: &Item) -> Self {
        let opt = |o: &Option<String>| o.clone().unwrap_or_default();
        let num = |o: Option<u32>| o.map(|n| n.to_string()).unwrap_or_default();
        Self {
            title: i.title.clone(),
            year: num(i.year),
            overview: i.overview.clone(),
            genres: i.genres.join(", "),
            rating: opt(&i.rating),
            maker: match i.kind {
                Kind::Movie => opt(&i.director),
                Kind::Series => i.creators.join(", "),
            },
            released: opt(&i.released),
            end_year: num(i.end_year),
            network: opt(&i.network),
            status: opt(&i.status),
            score: i.score.map(|s| format!("{s:.1}")).unwrap_or_default(),
            imdb_id: opt(&i.imdb_id),
            poster: opt(&i.poster),
            backdrop: opt(&i.backdrop),
            thumb: opt(&i.thumb),
            logo: opt(&i.logo),
        }
    }

    /// Fill in what a lookup found; empty answers leave a field alone.
    fn merge(&mut self, found: &Form) {
        for (to, from) in self.fields_mut().into_iter().zip(found.fields()) {
            if !from.trim().is_empty() {
                *to = from.to_owned();
            }
        }
    }

    fn fields(&self) -> [&String; 16] {
        [
            &self.title,
            &self.year,
            &self.overview,
            &self.genres,
            &self.rating,
            &self.maker,
            &self.released,
            &self.end_year,
            &self.network,
            &self.status,
            &self.score,
            &self.imdb_id,
            &self.poster,
            &self.backdrop,
            &self.thumb,
            &self.logo,
        ]
    }

    fn fields_mut(&mut self) -> [&mut String; 16] {
        [
            &mut self.title,
            &mut self.year,
            &mut self.overview,
            &mut self.genres,
            &mut self.rating,
            &mut self.maker,
            &mut self.released,
            &mut self.end_year,
            &mut self.network,
            &mut self.status,
            &mut self.score,
            &mut self.imdb_id,
            &mut self.poster,
            &mut self.backdrop,
            &mut self.thumb,
            &mut self.logo,
        ]
    }

    /// The fields that differ from `base`, in the library's names.
    /// A cleared field is sent as `null`.
    fn changes(&self, base: &Form, kind: Kind) -> Result<Map<String, Value>, String> {
        let mut out = Map::new();
        let text = |s: &str| {
            let s = s.trim();
            if s.is_empty() {
                Value::Null
            } else {
                json!(s)
            }
        };
        let list = |s: &str| {
            json!(s
                .split(',')
                .map(str::trim)
                .filter(|g| !g.is_empty())
                .collect::<Vec<_>>())
        };
        let int = |s: &str, name: &str| -> Result<Value, String> {
            let s = s.trim();
            if s.is_empty() {
                return Ok(Value::Null);
            }
            s.parse::<u32>()
                .map(|n| json!(n))
                .map_err(|_| format!("{name} must be a number"))
        };
        if self.title != base.title {
            if self.title.trim().is_empty() {
                return Err("The title can't be empty".into());
            }
            out.insert("title".into(), text(&self.title));
        }
        if self.year != base.year {
            out.insert("year".into(), int(&self.year, "Year")?);
        }
        if self.overview != base.overview {
            out.insert("overview".into(), json!(self.overview.trim()));
        }
        if self.genres != base.genres {
            out.insert("genres".into(), list(&self.genres));
        }
        if self.rating != base.rating {
            out.insert("rating".into(), text(&self.rating));
        }
        if self.maker != base.maker {
            match kind {
                Kind::Movie => out.insert("director".into(), text(&self.maker)),
                Kind::Series => out.insert("creators".into(), list(&self.maker)),
            };
        }
        if self.released != base.released {
            out.insert("released".into(), text(&self.released));
        }
        if self.end_year != base.end_year {
            out.insert("end_year".into(), int(&self.end_year, "End year")?);
        }
        for (name, a, b) in [
            ("network", &self.network, &base.network),
            ("status", &self.status, &base.status),
            ("imdb_id", &self.imdb_id, &base.imdb_id),
            ("poster", &self.poster, &base.poster),
            ("backdrop", &self.backdrop, &base.backdrop),
            ("thumb", &self.thumb, &base.thumb),
            ("logo", &self.logo, &base.logo),
        ] {
            if a != b {
                out.insert(name.into(), text(a));
            }
        }
        if self.score != base.score {
            let s = self.score.trim().replace(',', ".");
            let v = if s.is_empty() {
                Value::Null
            } else {
                let f: f32 = s
                    .parse()
                    .map_err(|_| "Score must be a number, like 7.4".to_owned())?;
                json!(f)
            };
            out.insert("score".into(), v);
        }
        Ok(out)
    }
}

/// The editor's tabs.
#[derive(Clone, Copy, PartialEq, Default)]
enum Tab {
    #[default]
    Details,
    Images,
    Files,
    Subtitles,
    Episodes,
}

#[derive(Default)]
pub(in super::super) struct State {
    tab: Tab,
    /// Subtitles, TMDB matches and episode edits for the open title.
    extras: extras::Extras,
    /// Only titles missing subtitles.
    subs_only: bool,
    /// Language for the bulk subtitle download.
    bulk_lang: String,
    /// Saving or fetching said the server doesn't have this part.
    pub missing: bool,
    kind: Kind,
    search: String,
    /// Only titles missing something.
    attention: bool,
    /// The item being edited, by our id.
    selected: Option<String>,
    form: Form,
    /// The item as the library has it; `form` starts as a copy.
    base: Form,
    fetching: Option<Task<Result<Item>>>,
    /// The open title as `GET /api/admin/<kind>/<id>` has it.
    detail: Remote<Item>,
    fetch_note: Option<(String, bool)>,
    fetch_missing: bool,
    /// A form error ("Year must be a number").
    error: Option<String>,
}

impl State {
    /// Open `id` in the editor.
    pub fn select(&mut self, i: &Item) {
        self.kind = i.kind;
        self.selected = Some(i.id.clone());
        self.base = Form::from_item(i);
        self.form = self.base.clone();
        self.fetch_note = None;
        self.error = None;
        self.search.clear();
        self.detail = Remote::default(); // load the server's full record
        self.extras.reset();
        if i.kind == Kind::Movie && self.tab == Tab::Episodes {
            self.tab = Tab::Details;
        }
    }

    /// Load the open title again (after a save, or Refresh).
    pub fn refresh(&mut self) {
        self.missing = false;
        self.detail.retry();
        self.extras.refresh();
    }

    /// The open title: the server's full record once it's here, else the
    /// list's (or the library's) copy.
    fn current<'a>(&'a self, cx: &Cx<'a>) -> Option<Item> {
        let id = self.selected.as_deref()?;
        match &self.detail.data {
            Some(d) if d.id == id => Some(d.clone()),
            _ => cx.find(id).cloned(),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        if let Some(r) = self.fetching.as_mut().and_then(|t| t.poll()) {
            self.fetching = None;
            match r {
                Ok(found) => {
                    self.form.merge(&Form::from_item(&found));
                    self.fetch_note = Some((
                        "Filled in from IMDb. Check the details, then Save.".into(),
                        true,
                    ));
                }
                Err(e) if e.is::<Unauthorized>() => cx.login = true,
                Err(e) if e.is::<NoAdminApi>() => self.fetch_missing = true,
                Err(e) => self.fetch_note = Some((format!("{e:#}"), false)),
            }
        }
        // the server's full record; without the endpoint, the list's copy will do
        self.detail.poll(&mut cx.login);
        if let Some(item) = self.current(cx).filter(|_| self.detail.due()) {
            let c = cx.client.clone();
            self.detail.load(&cx.ctx, move || c.admin_item(&item));
        }
        // new data came in: follow it, unless there are unsaved edits
        if let Some(item) = self.current(cx) {
            let fresh = Form::from_item(&item);
            if fresh != self.base {
                if self.form == self.base {
                    self.form = fresh.clone();
                }
                self.base = fresh;
            }
        }

        let (m, inner) = column(ui);
        let narrow = inner < 900.0;
        let list_w = if narrow { inner } else { 340.0 };
        let gap = 32.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(m);
            if narrow {
                ui.vertical(|ui| {
                    ui.set_width(inner);
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if self.missing {
                        if not_set_up(ui, Section::Library, &cx.client.info.name) {
                            self.missing = false;
                        }
                        ui.add_space(16.0);
                    }
                    let top = ui.cursor().top();
                    self.list(ui, cx, top + 460.0);
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
                self.list(ui, cx, top + h);
            });
            ui.add_space(gap);
            ui.vertical(|ui| {
                let w = inner - list_w - gap;
                ui.set_width(w);
                ui.spacing_mut().item_spacing.x = 8.0;
                ScrollArea::vertical()
                    .id_salt(("admin-library-editor", self.selected.clone()))
                    .max_height(h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(w - 14.0); // room for the scroll bar
                        if self.missing {
                            if not_set_up(ui, Section::Library, &cx.client.info.name) {
                                self.missing = false;
                            }
                            ui.add_space(16.0);
                        }
                        self.editor(ui, cx);
                        ui.add_space(24.0);
                    });
            });
        });
    }

    /// The title list; its bottom edge ends at `bottom` (screen y).
    fn list(&mut self, ui: &mut Ui, cx: &mut Cx, bottom: f32) {
        segmented(
            ui,
            &mut self.kind,
            &[
                (Kind::Movie, "Movies", Some(cx.movies.len())),
                (Kind::Series, "Series", Some(cx.series.len())),
            ],
        );
        ui.add_space(10.0);
        text_field(ui, &mut self.search, "Search titles", false);
        ui.add_space(6.0);
        ui.checkbox(&mut self.attention, "Only titles missing something");
        let knows_subs = cx
            .titles(self.kind)
            .iter()
            .any(|i| i.has_subtitles.is_some() || i.missing_subtitles.is_some());
        if knows_subs {
            ui.checkbox(&mut self.subs_only, "Only titles missing subtitles");
        }
        ui.add_space(8.0);
        if !cx.catalog {
            needs_note(
                ui,
                "Showing the app's library: the server doesn't list its titles for admins, \
                 so titles without a file are missing.",
                Section::Library,
            );
            ui.add_space(6.0);
        }
        let items = cx.titles(self.kind);
        let q = self.search.trim().to_lowercase();
        let shown: Vec<&Item> = items
            .iter()
            .filter(|i| q.is_empty() || i.title.to_lowercase().contains(&q))
            .filter(|i| !self.attention || !gaps(i).is_empty())
            .filter(|i| !self.subs_only || !knows_subs || no_subs(i))
            .collect();
        if self.subs_only && knows_subs && !shown.is_empty() {
            if self.bulk_lang.is_empty() {
                self.bulk_lang = "en".into();
            }
            ui.horizontal(|ui| {
                ui.add_sized(
                    [56.0, 30.0],
                    egui::TextEdit::singleline(&mut self.bulk_lang)
                        .margin(egui::Margin::symmetric(8, 6)),
                );
                let label = format!("Download subtitles for these {}", shown.len());
                if ui
                    .add_enabled_ui(!cx.busy, |ui| small_button(ui, &label))
                    .inner
                    .clicked()
                {
                    let kind = if self.kind == Kind::Movie {
                        "movies"
                    } else {
                        "series"
                    };
                    let ids: Vec<String> = shown.iter().map(|i| i.server_id().to_owned()).collect();
                    let lang = self.bulk_lang.trim().to_owned();
                    cx.run(move |c| {
                        let body =
                            serde_json::json!({ "type": kind, "ids": ids, "language": lang });
                        c.admin_call("POST", "subtitles/bulk", Some(&body))?;
                        Ok(Done {
                            msg: "Downloading subtitles; follow the live log on the Dashboard."
                                .into(),
                            library: true,
                            ..Default::default()
                        })
                    });
                }
            });
            ui.add_space(6.0);
        }
        egui::Frame::new()
            .stroke(Stroke::new(1.0f32, LINE()))
            .corner_radius(8)
            .inner_margin(4)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let height = (bottom - ui.cursor().top() - 8.0).max(200.0);
                ScrollArea::vertical()
                    .id_salt("admin-library-list")
                    .max_height(height)
                    .min_scrolled_height(height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if shown.is_empty() {
                            ui.label(mono("Nothing here", 11.0).color(MUTED()));
                        }
                        for i in shown {
                            let sel = self.selected.as_deref() == Some(i.id.as_str());
                            if list_row(ui, i, sel, cx) && !sel {
                                if self.form != self.base {
                                    // switching drops unsaved edits; say so
                                    self.fetch_note = None;
                                }
                                self.select(i);
                            }
                        }
                    });
            });
    }

    fn editor(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let Some(item) = self.current(cx) else {
            panel(ui, |ui| {
                caps(ui, "Edit a title", TEAL());
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Pick a movie or series on the left to change its details or artwork, \
                         or to fetch them from IMDb.",
                    )
                    .color(MUTED()),
                );
            });
            return;
        };
        let item = &item;
        let busy = cx.busy || self.fetching.is_some();
        let dirty = self.form != self.base;

        // header: poster, title, actions
        ui.horizontal_top(|ui| {
            let (art, _) = ui.allocate_exact_size(vec2(96.0, 144.0), Sense::hover());
            let url =
                (!self.form.poster.trim().is_empty()).then(|| self.form.poster.trim().to_owned());
            match url.as_deref().and_then(|u| cx.images.get(u)) {
                Some(t) => image_cover(ui.painter(), art, &t, 6, Color32::WHITE),
                None => placeholder(ui.painter(), art, &self.form.title, 6),
            }
            ui.add_space(16.0);
            ui.vertical(|ui| {
                caps(
                    ui,
                    if item.kind == Kind::Movie {
                        "Movie"
                    } else {
                        "Series"
                    },
                    TEAL(),
                );
                ui.label(bold(&item.title, 26.0));
                if let Some(f) = item
                    .file
                    .as_ref()
                    .and_then(|f| f.path.clone().zip(f.name.clone()))
                {
                    ui.label(mono(format!("{}/{}", f.0, f.1), 10.0).color(MUTED()));
                }
                let g = gaps(item);
                if !g.is_empty() {
                    ui.label(
                        RichText::new(format!("Missing: {}", g.join(", ")))
                            .small()
                            .color(WARN()),
                    );
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let b = ui
                        .add_enabled_ui(!busy && !self.fetch_missing, |ui| {
                            primary_button(ui, "Fetch from IMDb", true)
                        })
                        .inner;
                    if b.clicked() {
                        let (c, it, imdb) =
                            (cx.client.clone(), item.clone(), self.form.imdb_id.clone());
                        self.fetch_note = None;
                        self.fetching = Some(Task::spawn(&cx.ctx, move || {
                            c.admin_fetch_item(&it, Some(&imdb))
                        }));
                    }
                    if self.fetching.is_some() {
                        ui.add(egui::Spinner::new().size(16.0).color(MUTED()));
                    }
                });
                if self.fetch_missing {
                    needs_note(
                        ui,
                        "This server can't look titles up on IMDb yet.",
                        Section::Library,
                    );
                } else {
                    ui.label(
                        RichText::new("Uses the IMDb ID below when set, else the title and year.")
                            .small()
                            .color(MUTED()),
                    );
                }
            });
        });
        ui.add_space(12.0);
        if let Some((msg, ok)) = &self.fetch_note {
            ui.label(RichText::new(msg).color(if *ok { OK() } else { ERROR() }));
            ui.add_space(8.0);
        }

        let series = item.kind == Kind::Series;
        let mut tabs = vec![
            (Tab::Details, "Details", None),
            (Tab::Images, "Images", None),
            (Tab::Files, "Files", None),
            (Tab::Subtitles, "Subtitles", None),
        ];
        if series {
            tabs.push((Tab::Episodes, "Episodes", None));
        }
        segmented(ui, &mut self.tab, &tabs);
        ui.add_space(14.0);
        match self.tab {
            Tab::Files => return self.files_tab(ui, cx, item),
            Tab::Subtitles => return self.subtitles_tab(ui, cx, item),
            Tab::Episodes => return self.episodes_tab(ui, cx, item),
            Tab::Details | Tab::Images => {}
        }
        let details = self.tab == Tab::Details;
        let f = &mut self.form;
        if details {
            panel(ui, |ui| {
                caps(ui, "Details", TEAL());
                ui.add_space(8.0);
                field(ui, "Title", &mut f.title, "");
                pair(
                    ui,
                    ("Year", &mut f.year, "2010"),
                    ("IMDb ID", &mut f.imdb_id, "tt1375666"),
                );
                ui.label(mono("Overview", 10.5).color(MUTED()));
                ui.add(
                    egui::TextEdit::multiline(&mut f.overview)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY)
                        .margin(egui::Margin::symmetric(10, 8)),
                );
                ui.add_space(6.0);
                field(
                    ui,
                    "Genres (comma-separated)",
                    &mut f.genres,
                    "Action, Sci-Fi",
                );
                if series {
                    field(ui, "Creators (comma-separated)", &mut f.maker, "");
                    pair(
                        ui,
                        ("Network", &mut f.network, "HBO"),
                        ("Status", &mut f.status, "Ended"),
                    );
                    pair(
                        ui,
                        ("First aired", &mut f.released, "YYYY-MM-DD"),
                        ("Last year", &mut f.end_year, "2024"),
                    );
                } else {
                    field(ui, "Director", &mut f.maker, "");
                    pair(
                        ui,
                        ("Released", &mut f.released, "YYYY-MM-DD"),
                        ("Age rating", &mut f.rating, "PG-13"),
                    );
                }
                pair(
                    ui,
                    ("Score (out of 10)", &mut f.score, "7.4"),
                    ("", &mut String::new(), ""),
                );
                if series {
                    field(ui, "Age rating", &mut f.rating, "TV-MA");
                }
                if !item.locked.is_empty() {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!(
                            "Kept as edited, so syncs leave them alone: {}",
                            item.locked.join(", ")
                        ))
                        .small()
                        .color(MUTED()),
                    );
                }
            });
            ui.add_space(16.0);
        }

        let mut upload = None;
        let mut refetch = false;
        if !details {
            panel(ui, |ui| {
                caps(ui, "Artwork", TEAL());
                ui.add_space(4.0);
                if ui
                    .add_enabled_ui(!busy, |ui| small_button(ui, "Refetch missing images"))
                    .inner
                    .on_hover_text(
                        "Downloads only the ones this title doesn't have; existing images are kept",
                    )
                    .clicked()
                {
                    refetch = true;
                }
                ui.add_space(4.0);
                ui.label(
                RichText::new("Paste an image address (the server downloads it when you save), or upload a file.")
                    .small()
                    .color(MUTED()),
            );
                ui.add_space(8.0);
                for (which, label, hint, url) in [
                    ("poster", "Poster", "2:3, shown on tiles", &mut f.poster),
                    (
                        "backdrop",
                        "Backdrop",
                        "16:9, behind the details page",
                        &mut f.backdrop,
                    ),
                    (
                        "thumb",
                        "Thumbnail",
                        "16:9 with the title, for wide cards like Continue watching",
                        &mut f.thumb,
                    ),
                    (
                        "logo",
                        "Logo",
                        "transparent PNG, replaces the title text",
                        &mut f.logo,
                    ),
                ] {
                    ui.horizontal_top(|ui| {
                        let size = if which == "poster" {
                            vec2(80.0, 120.0)
                        } else {
                            vec2(160.0, 90.0)
                        };
                        art_preview(ui, cx, url.trim(), size, which == "logo");
                        ui.add_space(12.0);
                        ui.vertical(|ui| {
                            ui.label(mono(label, 10.5).color(MUTED()));
                            ui.label(RichText::new(hint).small().color(MUTED()));
                            ui.add_space(4.0);
                            ui.add(
                                egui::TextEdit::singleline(url)
                                    .hint_text("https://…")
                                    .desired_width(f32::INFINITY)
                                    .margin(egui::Margin::symmetric(10, 8)),
                            );
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled_ui(!busy, |ui| small_button(ui, "Upload…"))
                                    .inner
                                    .clicked()
                                {
                                    upload = Some(which);
                                }
                                if !url.trim().is_empty() && small_button(ui, "Remove").clicked() {
                                    url.clear();
                                }
                            });
                        });
                    });
                    ui.add_space(14.0);
                }
            });
            ui.add_space(16.0);
        }

        if let Some(e) = &self.error {
            ui.label(RichText::new(e).color(ERROR()));
            ui.add_space(6.0);
        }
        ui.horizontal(|ui| {
            let b = ui
                .add_enabled_ui(!busy && dirty, |ui| primary_button(ui, "Save", true))
                .inner;
            if b.clicked() {
                match self.form.changes(&self.base, item.kind) {
                    Ok(fields) => {
                        self.error = None;
                        self.fetch_note = None;
                        let it = item.clone();
                        let title = self.form.title.trim().to_owned();
                        let art: Vec<String> =
                            [&item.poster, &item.backdrop, &item.thumb, &item.logo]
                                .into_iter()
                                .flatten()
                                .cloned()
                                .collect();
                        cx.run(move |c| {
                            c.admin_update_item(&it, &Value::Object(fields))?;
                            Ok(Done {
                                msg: format!("Saved {title}."),
                                library: true,
                                forget: art,
                                ..Default::default()
                            })
                        });
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            if dirty && small_button(ui, "Undo changes").clicked() {
                self.form = self.base.clone();
                self.fetch_note = None;
                self.error = None;
            }
            if !dirty {
                ui.label(mono("No unsaved changes", 10.5).color(MUTED()));
            }
        });

        if refetch {
            let it = item.clone();
            let old: Vec<String> = [&item.poster, &item.backdrop, &item.thumb, &item.logo]
                .into_iter()
                .flatten()
                .cloned()
                .collect();
            cx.run(move |c| {
                let path = crate::api::Client::item_path(&it);
                let r = c.admin_call("POST", &format!("{path}/art/refetch"), None)?;
                let n = |k: &str| r.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
                Ok(Done {
                    msg: format!("Downloaded {} images, kept {}.", n("downloaded"), n("kept")),
                    library: true,
                    forget: old,
                    ..Default::default()
                })
            });
        }
        if details {
            self.match_and_delete(ui, cx, item);
        }

        if let Some(which) = upload {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Image", &["jpg", "jpeg", "png", "webp"])
                .pick_file()
            {
                let it = item.clone();
                let old: Vec<String> = match which {
                    "poster" => item.poster.clone(),
                    "backdrop" => item.backdrop.clone(),
                    "thumb" => item.thumb.clone(),
                    _ => item.logo.clone(),
                }
                .into_iter()
                .collect();
                cx.run(move |c| {
                    let (data, mime) = read_image(&path)?;
                    c.admin_upload_art(&it, which, &data, mime)?;
                    Ok(Done {
                        msg: format!("Uploaded a new {which} for {}.", it.title),
                        library: true,
                        forget: old,
                        ..Default::default()
                    })
                });
            }
        }
    }
}

/// The server says the title (or some of its episodes) has no subtitles.
fn no_subs(i: &Item) -> bool {
    i.has_subtitles == Some(false) || i.missing_subtitles.is_some_and(|n| n > 0)
}

/// What a title is missing, for the list and the editor.
fn gaps(i: &Item) -> Vec<&'static str> {
    let mut v = Vec::new();
    if i.poster.is_none() {
        v.push("poster");
    }
    if i.backdrop.is_none() {
        v.push("backdrop");
    }
    if i.overview.trim().is_empty() {
        v.push("overview");
    }
    if i.year.is_none() {
        v.push("year");
    }
    if i.genres.is_empty() {
        v.push("genres");
    }
    v
}

fn list_row(ui: &mut Ui, i: &Item, selected: bool, cx: &mut Cx) -> bool {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 52.0), Sense::click());
    if !ui.is_rect_visible(r) {
        return false;
    }
    let p = ui.painter();
    if selected {
        p.rect_filled(r, 4.0, SURFACE_HI());
        p.rect_stroke(
            r,
            4.0,
            Stroke::new(1.0f32, TEAL()),
            egui::StrokeKind::Inside,
        );
    } else if resp.hovered() {
        p.rect_filled(r, 4.0, SURFACE_HI());
    }
    let art = egui::Rect::from_min_size(r.min + vec2(6.0, 4.0), vec2(29.0, 44.0));
    match i.poster.as_ref().and_then(|u| cx.images.get(u)) {
        Some(t) => image_cover(p, art, &t, 3, Color32::WHITE),
        None => placeholder(p, art, &i.title, 3),
    }
    let clip = p.with_clip_rect(r.shrink(2.0));
    clip.text(
        art.right_top() + vec2(10.0, 4.0),
        egui::Align2::LEFT_TOP,
        &i.title,
        egui::FontId::proportional(14.0),
        TEXT(),
    );
    let g = gaps(i);
    let mut sub = i.year.map(|y| y.to_string()).unwrap_or_default();
    if let Some(c) = &i.collection {
        sub.push_str(&format!("   {c}"));
    }
    clip.text(
        art.right_bottom() + vec2(10.0, -4.0),
        egui::Align2::LEFT_BOTTOM,
        sub,
        egui::FontId::monospace(10.0),
        MUTED(),
    );
    if !g.is_empty() {
        clip.circle_filled(egui::pos2(r.right() - 12.0, r.center().y), 4.0, WARN());
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// The image at `url`, fitted into `size`; a dashed box when there's none.
fn art_preview(ui: &mut Ui, cx: &mut Cx, url: &str, size: egui::Vec2, logo: bool) {
    let (r, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    // a logo is see-through: show it on a dark card, like the details page
    p.rect_filled(
        r,
        4.0,
        if logo {
            Color32::from_rgb(10, 11, 14)
        } else {
            SURFACE_HI()
        },
    );
    let tex = (!url.is_empty()).then(|| cx.images.get(url)).flatten();
    match tex {
        Some(t) if logo => {
            let s = t.size_vec2();
            let k = ((r.width() - 16.0) / s.x).min((r.height() - 16.0) / s.y);
            let fit = egui::Rect::from_center_size(r.center(), s * k);
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            p.image(t.id(), fit, uv, Color32::WHITE);
        }
        Some(t) => image_cover(p, r, &t, 4, Color32::WHITE),
        None => {
            let text = if url.is_empty() {
                "none"
            } else if cx.images.failed(url) {
                "can't load"
            } else {
                "loading…"
            };
            p.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::monospace(10.0),
                MUTED(),
            );
        }
    }
    p.rect_stroke(
        r,
        4.0,
        Stroke::new(1.0f32, LINE()),
        egui::StrokeKind::Inside,
    );
}

fn field(ui: &mut Ui, label: &str, value: &mut String, hint: &str) {
    ui.label(mono(label, 10.5).color(MUTED()));
    text_field(ui, value, hint, false);
    ui.add_space(6.0);
}

/// Two fields side by side; an empty label leaves the second slot blank.
fn pair(ui: &mut Ui, a: (&str, &mut String, &str), b: (&str, &mut String, &str)) {
    let w = (ui.available_width() - 12.0) / 2.0;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        for (label, value, hint) in [a, b] {
            ui.vertical(|ui| {
                ui.set_width(w);
                if !label.is_empty() {
                    field(ui, label, value, hint);
                }
            });
        }
    });
}

fn read_image(path: &Path) -> Result<(Vec<u8>, &'static str)> {
    let data = std::fs::read(path).with_context(|| format!("can't read {}", path.display()))?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    };
    Ok((data, mime))
}
