//! Admin › Library: edit a title's details and artwork by hand, or fetch
//! them from IMDb and check them before saving. The form, its checks and
//! what's sent are egui's (`browse::admin::library`), shared.
mod extras;

use anyhow::Result;
use iced::alignment::Vertical;
use iced::widget::{
    button, checkbox, column, container, image, row, scrollable, space, text, text_editor,
    text_input, Column,
};
use iced::{Background, Border, ContentFit, Element, Length, Padding};
use serde_json::Value;

use super::super::details::{caps, column_of, gap, panel};
use super::super::profile::{field_style, Remote};
use super::super::{cards, scroll_style, spin_secs, Msg as LMsg};
use super::{needs_note, not_set_up, Asks, Cx, Done, JobFn, Msg as AMsg, Section};
use crate::api::{Client, Item, Kind, NoAdminApi, Unauthorized};
use crate::iced_ui::images::Variant;
use crate::iced_ui::spaced::mono;
use crate::iced_ui::style::{self, c, BOLD};
use crate::iced_ui::widgets::{self, Kind as Btn};
use crate::iced_ui::Job;
use crate::ui::browse::admin::library::{gaps, no_subs, read_image, Form};
use crate::ui::browse::details::TEAL;
use crate::ui::theme::{ERROR, LINE, OK, SURFACE_HI, WARN};

/// The editor's tabs.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Tab {
    #[default]
    Details,
    Images,
    Files,
    Subtitles,
    Episodes,
}

/// Where each form field sits in `Form::fields_mut`.
mod f {
    pub const TITLE: usize = 0;
    pub const YEAR: usize = 1;
    pub const GENRES: usize = 3;
    pub const RATING: usize = 4;
    pub const MAKER: usize = 5;
    pub const RELEASED: usize = 6;
    pub const END_YEAR: usize = 7;
    pub const NETWORK: usize = 8;
    pub const STATUS: usize = 9;
    pub const SCORE: usize = 10;
    pub const IMDB: usize = 11;
    pub const POSTER: usize = 12;
    pub const BACKDROP: usize = 13;
    pub const THUMB: usize = 14;
    pub const LOGO: usize = 15;
}

#[derive(Clone, Debug)]
pub enum Msg {
    Kind(Kind),
    Search(String),
    Attention(bool),
    SubsOnly(bool),
    BulkLang(String),
    /// Download subtitles for these titles (server ids).
    BulkSubs(Vec<String>),
    Select(String),
    Tab(Tab),
    /// A form field changed (its place in the form).
    Field(usize, String),
    Overview(text_editor::Action),
    Fetch,
    Save,
    Undo,
    Refetch,
    /// Upload a file as this artwork ("poster", …).
    Upload(&'static str),
    X(extras::Msg),
}

#[derive(Default)]
pub struct State {
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
    /// The item being edited: the list's copy.
    item: Option<Item>,
    form: Form,
    /// The item as the library has it; `form` starts as a copy.
    base: Form,
    /// The overview's multi-line field, kept in step with `form`.
    overview: text_editor::Content,
    fetching: Option<Job<Result<Item>>>,
    /// The open title as `GET /api/admin/<kind>/<id>` has it.
    detail: Remote<Item>,
    fetch_note: Option<(String, bool)>,
    fetch_missing: bool,
    /// A form error ("Year must be a number").
    error: Option<String>,
}

fn send(m: Msg) -> LMsg {
    LMsg::Admin(AMsg::Library(m))
}

impl State {
    pub fn refresh(&mut self) {
        self.missing = false;
        self.detail.retry();
        self.extras.refresh();
    }

    pub fn missing(&self) -> bool {
        self.missing
    }

    pub fn busy(&self) -> bool {
        self.fetching.is_some() || self.detail.loading() || self.extras.busy()
    }

    /// The open title: the server's full record once it's here, else the list's copy.
    fn current(&self) -> Option<Item> {
        let id = &self.item.as_ref()?.id;
        match &self.detail.data {
            Some(d) if d.id == *id => Some(d.clone()),
            _ => self.item.clone(),
        }
    }

    fn set_form(&mut self, form: Form) {
        self.overview = text_editor::Content::with_text(&form.overview);
        self.form = form;
    }

    /// Open `i` in the editor.
    pub fn select(&mut self, i: &Item) {
        self.kind = i.kind;
        self.item = Some(i.clone());
        self.base = Form::from_item(i);
        self.set_form(self.base.clone());
        self.fetch_note = None;
        self.error = None;
        self.search.clear();
        self.detail = Remote::default(); // load the server's full record
        self.extras.reset();
        if i.kind == Kind::Movie && self.tab == Tab::Episodes {
            self.tab = Tab::Details;
        }
    }

    pub fn work(&mut self, client: &Client, asks: &mut Asks) {
        if let Some(r) = self.fetching.as_mut().and_then(Job::poll) {
            self.fetching = None;
            match r {
                Ok(found) => {
                    let mut form = self.form.clone();
                    form.merge(&Form::from_item(&found));
                    self.set_form(form);
                    self.fetch_note = Some((
                        "Filled in from IMDb. Check the details, then Save.".into(),
                        true,
                    ));
                }
                Err(e) if e.is::<Unauthorized>() => asks.login = true,
                Err(e) if e.is::<NoAdminApi>() => self.fetch_missing = true,
                Err(e) => self.fetch_note = Some((format!("{e:#}"), false)),
            }
        }
        // the server's full record; without the endpoint, the list's copy will do
        self.detail.poll(&mut asks.login);
        if let Some(item) = self.current().filter(|_| self.detail.due()) {
            let c = client.clone();
            self.detail.load(move || c.admin_item(&item));
        }
        // new data came in: follow it, unless there are unsaved edits
        if let Some(item) = self.current() {
            let fresh = Form::from_item(&item);
            if fresh != self.base {
                if self.form == self.base {
                    self.set_form(fresh.clone());
                }
                self.base = fresh;
            }
        }
        if let Some(item) = self.current() {
            self.extras.work(client, asks, &item, self.tab);
        }
    }

    /// `all`: every title we know, to open one by id.
    pub fn update(&mut self, m: Msg, client: &Client, all: &[Item]) -> Option<JobFn> {
        match m {
            Msg::Kind(k) => self.kind = k,
            Msg::Search(s) => self.search = s,
            Msg::Attention(on) => self.attention = on,
            Msg::SubsOnly(on) => self.subs_only = on,
            Msg::BulkLang(l) => self.bulk_lang = l,
            Msg::BulkSubs(ids) => {
                let kind = if self.kind == Kind::Movie {
                    "movies"
                } else {
                    "series"
                };
                let lang = self.bulk_lang.trim().to_owned();
                return Some(Box::new(move |c: &Client| {
                    let body = serde_json::json!({ "type": kind, "ids": ids, "language": lang });
                    c.admin_call("POST", "subtitles/bulk", Some(&body))?;
                    Ok(Done {
                        msg: "Downloading subtitles; follow the live log on the Dashboard.".into(),
                        library: true,
                        ..Default::default()
                    })
                }));
            }
            Msg::Select(id) => {
                if let Some(i) = all.iter().find(|i| i.id == id) {
                    if self.form != self.base {
                        // switching drops unsaved edits
                        self.fetch_note = None;
                    }
                    self.select(i);
                }
            }
            Msg::Tab(t) => self.tab = t,
            Msg::Field(i, v) => {
                if let Some(slot) = self.form.fields_mut().into_iter().nth(i) {
                    *slot = v;
                }
            }
            Msg::Overview(a) => {
                self.overview.perform(a);
                self.form.overview = self.overview.text();
                // the editor keeps a final newline
                if self.form.overview.ends_with('\n') {
                    self.form.overview.pop();
                }
            }
            Msg::Fetch => {
                let item = self.current()?;
                let (c, imdb) = (client.clone(), self.form.imdb_id.clone());
                self.fetch_note = None;
                self.fetching = Some(Job::spawn(move || c.admin_fetch_item(&item, Some(&imdb))));
            }
            Msg::Undo => {
                self.set_form(self.base.clone());
                self.fetch_note = None;
                self.error = None;
            }
            Msg::Save => {
                let item = self.current()?;
                match self.form.changes(&self.base, item.kind) {
                    Ok(fields) => {
                        self.error = None;
                        self.fetch_note = None;
                        let title = self.form.title.trim().to_owned();
                        let art = artwork(&item);
                        return Some(Box::new(move |c: &Client| {
                            c.admin_update_item(&item, &Value::Object(fields))?;
                            Ok(Done {
                                msg: format!("Saved {title}."),
                                library: true,
                                forget: art,
                                ..Default::default()
                            })
                        }));
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            Msg::Refetch => {
                let item = self.current()?;
                let old = artwork(&item);
                return Some(Box::new(move |c: &Client| {
                    let path = Client::item_path(&item);
                    let r = c.admin_call("POST", &format!("{path}/art/refetch"), None)?;
                    let n = |k: &str| r.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
                    Ok(Done {
                        msg: format!("Downloaded {} images, kept {}.", n("downloaded"), n("kept")),
                        library: true,
                        forget: old,
                        ..Default::default()
                    })
                }));
            }
            Msg::Upload(which) => {
                let item = self.current()?;
                let path = rfd::FileDialog::new()
                    .add_filter("Image", &["jpg", "jpeg", "png", "webp"])
                    .pick_file()?;
                let old: Vec<String> = match which {
                    "poster" => item.poster.clone(),
                    "backdrop" => item.backdrop.clone(),
                    "thumb" => item.thumb.clone(),
                    _ => item.logo.clone(),
                }
                .into_iter()
                .collect();
                return Some(Box::new(move |c: &Client| {
                    let (data, mime) = read_image(&path)?;
                    c.admin_upload_art(&item, which, &data, mime)?;
                    Ok(Done {
                        msg: format!("Uploaded a new {which} for {}.", item.title),
                        library: true,
                        forget: old,
                        ..Default::default()
                    })
                }));
            }
            Msg::X(m) => {
                let item = self.current()?;
                return self.extras.update(m, client, &item);
            }
        }
        None
    }

    /// The list on the left, the editor beside it.
    pub fn view<'a>(&'a self, cx: &Cx<'a>, w: f32) -> Element<'a, LMsg> {
        let (m, inner) = column_of(w);
        let narrow = inner < 900.0;
        let list_w = if narrow { inner } else { 340.0 };
        let g = 32.0;
        let editor_w = if narrow { inner } else { inner - list_w - g };
        let mut editor: Column<'a, LMsg> = column![].width(editor_w);
        if self.missing {
            editor = editor
                .push(not_set_up(Section::Library, &cx.client.info.name))
                .push(gap(16.0));
        }
        editor = editor.push(self.editor(cx, editor_w));
        let list = container(self.list(cx)).width(list_w);
        let body: Element<'a, LMsg> = if narrow {
            column![list, gap(20.0), editor].into()
        } else {
            row![list, space().width(g), editor]
                .align_y(Vertical::Top)
                .into()
        };
        container(body).padding(Padding::default().left(m)).into()
    }

    /// The title list.
    fn list<'a>(&'a self, cx: &Cx<'a>) -> Element<'a, LMsg> {
        let mut col: Column<'a, LMsg> = column![
            widgets::segmented(vec![
                (
                    "Movies".to_owned(),
                    Some(cx.movies.len()),
                    self.kind == Kind::Movie,
                    send(Msg::Kind(Kind::Movie))
                ),
                (
                    "Series".to_owned(),
                    Some(cx.series.len()),
                    self.kind == Kind::Series,
                    send(Msg::Kind(Kind::Series))
                ),
            ]),
            gap(10.0),
            text_input("Search titles", &self.search)
                .on_input(|s| send(Msg::Search(s)))
                .size(15)
                .padding([9, 10])
                .style(field_style),
            gap(6.0),
            checkbox(self.attention)
                .label("Only titles missing something")
                .on_toggle(|v| send(Msg::Attention(v)))
                .size(15)
                .text_size(14),
        ];
        let items = cx.titles(self.kind);
        let knows_subs = items
            .iter()
            .any(|i| i.has_subtitles.is_some() || i.missing_subtitles.is_some());
        if knows_subs {
            col = col.push(gap(4.0)).push(
                checkbox(self.subs_only)
                    .label("Only titles missing subtitles")
                    .on_toggle(|v| send(Msg::SubsOnly(v)))
                    .size(15)
                    .text_size(14),
            );
        }
        col = col.push(gap(8.0));
        if !cx.catalog {
            col = col
                .push(needs_note(
                    "Showing the app's library: the server doesn't list its titles for admins, \
                     so titles without a file are missing.",
                    Section::Library,
                ))
                .push(gap(6.0));
        }
        let q = self.search.trim().to_lowercase();
        let shown: Vec<&Item> = items
            .iter()
            .filter(|i| q.is_empty() || i.title.to_lowercase().contains(&q))
            .filter(|i| !self.attention || !gaps(i).is_empty())
            .filter(|i| !self.subs_only || !knows_subs || no_subs(i))
            .collect();
        if self.subs_only && knows_subs && !shown.is_empty() {
            let lang = if self.bulk_lang.is_empty() {
                "en"
            } else {
                &self.bulk_lang
            };
            let ids: Vec<String> = shown.iter().map(|i| i.server_id().to_owned()).collect();
            col = col
                .push(
                    row![
                        text_input("en", lang)
                            .on_input(|s| send(Msg::BulkLang(s)))
                            .size(14)
                            .padding([6, 8])
                            .width(56)
                            .style(field_style),
                        widgets::button_with(
                            Btn::Small,
                            &format!("Download subtitles for these {}", shown.len()),
                            None,
                            (!cx.busy).then(|| send(Msg::BulkSubs(ids)))
                        ),
                    ]
                    .spacing(8)
                    .align_y(Vertical::Center),
                )
                .push(gap(6.0));
        }
        let selected = self.item.as_ref().map(|i| i.id.as_str());
        let mut rows: Column<'a, LMsg> = column![].spacing(2);
        if shown.is_empty() {
            rows = rows.push(mono("Nothing here", 11.0).color(style::MUTED()));
        }
        for i in shown {
            rows = rows.push(list_row(cx, i, selected == Some(i.id.as_str())));
        }
        col.push(
            container(
                scrollable(rows.width(Length::Fill))
                    .height(Length::Fixed(520.0))
                    .style(scroll_style),
            )
            .padding(4)
            .style(|_| framed()),
        )
        .into()
    }

    fn editor<'a>(&'a self, cx: &Cx<'a>, w: f32) -> Element<'a, LMsg> {
        let Some(item) = self.current() else {
            return panel(column![
                caps("Edit a title", c(TEAL())),
                gap(6.0),
                text(
                    "Pick a movie or series on the left to change its details or artwork, \
                     or to fetch them from IMDb."
                )
                .size(14.5)
                .color(style::MUTED()),
            ]);
        };
        let busy = cx.busy || self.fetching.is_some();
        let dirty = self.form != self.base;
        let series = item.kind == Kind::Series;

        // header: poster, title, actions
        let poster = self.form.poster.trim();
        let mut head: Column<'a, LMsg> = column![
            caps(if series { "Series" } else { "Movie" }, c(TEAL())),
            text(item.title.clone()).font(BOLD).size(26),
        ];
        if let Some(f) = item
            .file
            .as_ref()
            .and_then(|f| f.path.clone().zip(f.name.clone()))
        {
            head = head.push(mono(&format!("{}/{}", f.0, f.1), 10.0).color(style::MUTED()));
        }
        let g = gaps(&item);
        if !g.is_empty() {
            head = head.push(
                text(format!("Missing: {}", g.join(", ")))
                    .size(12)
                    .color(c(WARN())),
            );
        }
        let mut fetch = row![widgets::primary(
            "Fetch from IMDb",
            (!busy && !self.fetch_missing).then(|| send(Msg::Fetch))
        )]
        .spacing(10)
        .align_y(Vertical::Center);
        if self.fetching.is_some() {
            fetch = fetch.push(crate::iced_ui::icon::spinner(
                spin_secs(),
                style::MUTED(),
                16.0,
            ));
        }
        head = head.push(gap(10.0)).push(fetch);
        head = head.push(if self.fetch_missing {
            needs_note(
                "This server can't look titles up on IMDb yet.",
                Section::Library,
            )
        } else {
            text("Uses the IMDb ID below when set, else the title and year.")
                .size(12)
                .color(style::MUTED())
                .into()
        });
        let mut col: Column<'a, LMsg> = column![
            row![
                cards::art(
                    cx.images,
                    (!poster.is_empty()).then_some(poster),
                    &self.form.title,
                    96.0,
                    144.0,
                    6.0
                ),
                head.spacing(2),
            ]
            .spacing(16),
            gap(12.0),
        ]
        .width(w);
        if let Some((msg, ok)) = &self.fetch_note {
            col = col
                .push(
                    text(msg.clone())
                        .size(14.5)
                        .color(c(if *ok { OK() } else { ERROR() })),
                )
                .push(gap(8.0));
        }

        let mut tabs = vec![
            (Tab::Details, "Details"),
            (Tab::Images, "Images"),
            (Tab::Files, "Files"),
            (Tab::Subtitles, "Subtitles"),
        ];
        if series {
            tabs.push((Tab::Episodes, "Episodes"));
        }
        col = col
            .push(widgets::segmented(
                tabs.into_iter()
                    .map(|(t, l)| (l.to_owned(), None, self.tab == t, send(Msg::Tab(t))))
                    .collect(),
            ))
            .push(gap(14.0));
        match self.tab {
            Tab::Files => return col.push(self.extras.files_tab(cx, &item)).into(),
            Tab::Subtitles => return col.push(self.extras.subtitles_tab(cx, &item)).into(),
            Tab::Episodes => return col.push(self.extras.episodes_tab(cx, &item)).into(),
            Tab::Details | Tab::Images => {}
        }
        let details = self.tab == Tab::Details;
        if details {
            col = col.push(self.details_panel(&item, w)).push(gap(16.0));
        } else {
            col = col.push(self.artwork_panel(cx, busy, w)).push(gap(16.0));
        }

        if let Some(e) = &self.error {
            col = col
                .push(text(e.clone()).size(14.5).color(style::ERROR()))
                .push(gap(6.0));
        }
        let mut actions = row![widgets::primary(
            "Save",
            (!busy && dirty).then(|| send(Msg::Save))
        )]
        .spacing(10)
        .align_y(Vertical::Center);
        if dirty {
            actions = actions.push(widgets::button_with(
                Btn::Small,
                "Undo changes",
                None,
                Some(send(Msg::Undo)),
            ));
        } else {
            actions = actions.push(mono("No unsaved changes", 10.5).color(style::MUTED()));
        }
        col = col.push(actions);
        if details {
            col = col.push(self.extras.match_and_delete(cx, &item));
        }
        col.into()
    }

    fn details_panel<'a>(&'a self, item: &Item, w: f32) -> Element<'a, LMsg> {
        let series = item.kind == Kind::Series;
        let fm = &self.form;
        let half = (w - 36.0 - 12.0) / 2.0;
        let mut col: Column<'a, LMsg> = column![
            caps("Details", c(TEAL())),
            gap(8.0),
            field("Title", f::TITLE, &fm.title, ""),
            pair(
                half,
                ("Year", f::YEAR, &fm.year, "2010"),
                Some(("IMDb ID", f::IMDB, &fm.imdb_id, "tt1375666"))
            ),
            mono("Overview", 10.5).color(style::MUTED()),
            text_editor(&self.overview)
                .on_action(|a| send(Msg::Overview(a)))
                .height(100)
                .padding([8, 10])
                .size(15),
            gap(6.0),
            field(
                "Genres (comma-separated)",
                f::GENRES,
                &fm.genres,
                "Action, Sci-Fi"
            ),
        ];
        if series {
            col = col
                .push(field("Creators (comma-separated)", f::MAKER, &fm.maker, ""))
                .push(pair(
                    half,
                    ("Network", f::NETWORK, &fm.network, "HBO"),
                    Some(("Status", f::STATUS, &fm.status, "Ended")),
                ))
                .push(pair(
                    half,
                    ("First aired", f::RELEASED, &fm.released, "YYYY-MM-DD"),
                    Some(("Last year", f::END_YEAR, &fm.end_year, "2024")),
                ));
        } else {
            col = col
                .push(field("Director", f::MAKER, &fm.maker, ""))
                .push(pair(
                    half,
                    ("Released", f::RELEASED, &fm.released, "YYYY-MM-DD"),
                    Some(("Age rating", f::RATING, &fm.rating, "PG-13")),
                ));
        }
        col = col.push(pair(
            half,
            ("Score (out of 10)", f::SCORE, &fm.score, "7.4"),
            None,
        ));
        if series {
            col = col.push(field("Age rating", f::RATING, &fm.rating, "TV-MA"));
        }
        if !item.locked.is_empty() {
            col = col.push(gap(6.0)).push(
                text(format!(
                    "Kept as edited, so syncs leave them alone: {}",
                    item.locked.join(", ")
                ))
                .size(12)
                .color(style::MUTED()),
            );
        }
        panel(col)
    }

    fn artwork_panel<'a>(&'a self, cx: &Cx<'a>, busy: bool, w: f32) -> Element<'a, LMsg> {
        let fm = &self.form;
        let mut col: Column<'a, LMsg> = column![
            caps("Artwork", c(TEAL())),
            gap(4.0),
            crate::iced_ui::chrome::tip_below(
                widgets::button_with(
                    Btn::Small,
                    "Refetch missing images",
                    None,
                    (!busy).then(|| send(Msg::Refetch))
                ),
                "Downloads only the ones this title doesn't have; existing images are kept"
            ),
            gap(4.0),
            text(
                "Paste an image address (the server downloads it when you save), or upload a file."
            )
            .size(12)
            .color(style::MUTED()),
            gap(8.0),
        ];
        for (which, idx, label, hint, url) in [
            (
                "poster",
                f::POSTER,
                "Poster",
                "2:3, shown on tiles",
                &fm.poster,
            ),
            (
                "backdrop",
                f::BACKDROP,
                "Backdrop",
                "16:9, behind the details page",
                &fm.backdrop,
            ),
            (
                "thumb",
                f::THUMB,
                "Thumbnail",
                "16:9 with the title, for wide cards like Continue watching",
                &fm.thumb,
            ),
            (
                "logo",
                f::LOGO,
                "Logo",
                "transparent PNG, replaces the title text",
                &fm.logo,
            ),
        ] {
            let size = if which == "poster" {
                (80.0, 120.0)
            } else {
                (160.0, 90.0)
            };
            let mut buttons = row![widgets::button_with(
                Btn::Small,
                "Upload…",
                None,
                (!busy).then(|| send(Msg::Upload(which)))
            )]
            .spacing(8);
            if !url.trim().is_empty() {
                buttons = buttons.push(widgets::button_with(
                    Btn::Small,
                    "Remove",
                    None,
                    Some(send(Msg::Field(idx, String::new()))),
                ));
            }
            let fields = column![
                mono(label, 10.5).color(style::MUTED()),
                text(hint).size(12).color(style::MUTED()),
                gap(4.0),
                text_input("https://…", url)
                    .on_input(move |s| send(Msg::Field(idx, s)))
                    .size(14)
                    .padding([8, 10])
                    .style(field_style),
                gap(4.0),
                buttons,
            ]
            .width(Length::Fill);
            col = col
                .push(
                    row![art_preview(cx, url.trim(), size, which == "logo"), fields]
                        .spacing(12)
                        .width(w - 36.0),
                )
                .push(gap(14.0));
        }
        panel(col)
    }
}

/// Every artwork address the title has, to download again after a change.
fn artwork(item: &Item) -> Vec<String> {
    [&item.poster, &item.backdrop, &item.thumb, &item.logo]
        .into_iter()
        .flatten()
        .cloned()
        .collect()
}

/// A labelled text field for one form field.
fn field<'a>(label: &str, idx: usize, value: &str, hint: &str) -> Element<'a, LMsg> {
    column![
        mono(label, 10.5).color(style::MUTED()),
        text_input(hint, value)
            .on_input(move |s| send(Msg::Field(idx, s)))
            .size(15)
            .padding([9, 10])
            .style(field_style),
        gap(6.0),
    ]
    .into()
}

/// Two fields side by side; `None` leaves the second slot blank.
fn pair<'a>(
    w: f32,
    a: (&str, usize, &str, &str),
    b: Option<(&str, usize, &str, &str)>,
) -> Element<'a, LMsg> {
    let second: Element<'a, LMsg> = match b {
        Some((l, i, v, h)) => field(l, i, v, h),
        None => space().into(),
    };
    row![
        container(field(a.0, a.1, a.2, a.3)).width(w),
        container(second).width(w),
    ]
    .spacing(12)
    .into()
}

/// A bordered box around a list.
fn framed() -> container::Style {
    container::Style {
        border: Border {
            color: c(LINE()),
            width: 1.0,
            radius: 8.0.into(),
        },
        ..container::Style::default()
    }
}

/// A title in the list: poster, title, year and collection, a dot when it's missing something.
fn list_row<'a>(cx: &Cx<'a>, i: &Item, selected: bool) -> Element<'a, LMsg> {
    let mut sub = i.year.map(|y| y.to_string()).unwrap_or_default();
    if let Some(c) = &i.collection {
        sub.push_str(&format!("   {c}"));
    }
    let mut line = row![
        cards::art(cx.images, i.poster.as_deref(), &i.title, 29.0, 44.0, 3.0),
        column![
            text(i.title.clone())
                .size(14)
                .wrapping(text::Wrapping::None),
            mono(&sub, 10.0).color(style::MUTED()),
        ]
        .spacing(4)
        .width(Length::Fill),
    ]
    .spacing(10)
    .align_y(Vertical::Center);
    if !gaps(i).is_empty() {
        line = line.push(super::status_dot(c(WARN())));
    }
    let teal = c(TEAL());
    button(container(line).clip(true))
        .width(Length::Fill)
        .height(52)
        .padding([4, 8])
        .style(move |_, s| {
            let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: (selected || hot).then(|| Background::Color(c(SURFACE_HI()))),
                border: Border {
                    color: if selected {
                        teal
                    } else {
                        iced::Color::TRANSPARENT
                    },
                    width: 1.0,
                    radius: 4.0.into(),
                },
                text_color: style::TEXT(),
                ..button::Style::default()
            }
        })
        .on_press(send(Msg::Select(i.id.clone())))
        .into()
}

/// The image at `url`, fitted into `size`; "none" or "loading…" when there's
/// none yet. A logo is see-through: it sits on a dark card, like the details page.
fn art_preview<'a>(cx: &Cx<'a>, url: &str, size: (f32, f32), logo: bool) -> Element<'a, LMsg> {
    let (w, h) = size;
    let bg = if logo {
        iced::Color::from_rgb8(10, 11, 14)
    } else {
        c(SURFACE_HI())
    };
    let v = Variant::for_size(w, h);
    let inner: Element<'a, LMsg> = if url.is_empty() {
        mono("none", 10.0).color(style::MUTED()).into()
    } else if logo {
        match cx.images.get(url, v) {
            Some((handle, _)) => image(handle)
                .width(w - 16.0)
                .height(h - 16.0)
                .content_fit(ContentFit::Contain)
                .into(),
            None if cx.images.failed(url, v) => {
                mono("can't load", 10.0).color(style::MUTED()).into()
            }
            None => mono("loading…", 10.0).color(style::MUTED()).into(),
        }
    } else if cx.images.failed(url, v) {
        mono("can't load", 10.0).color(style::MUTED()).into()
    } else {
        return container(cards::art(cx.images, Some(url), "", w, h, 4.0))
            .style(|_| container::Style {
                border: Border {
                    color: c(LINE()),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..container::Style::default()
            })
            .into();
    };
    container(inner)
        .width(w)
        .height(h)
        .center_x(w)
        .center_y(h)
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: c(LINE()),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}
