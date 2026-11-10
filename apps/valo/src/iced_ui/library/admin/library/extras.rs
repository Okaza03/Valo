//! The editor's other tabs (files, subtitles, episodes) and its extra
//! actions (TMDB matching, deleting a title), each on its own endpoint.
//! Release matching and episode codes are in `crate::ui::browse::admin::library::extras`.
use std::collections::HashSet;
use std::fmt;

use iced::alignment::Vertical;
use iced::widget::{
    column, container, pick_list, row, space, text, text_editor, text_input, Column,
};
use iced::{Element, Length};
use serde_json::{json, Value};

use super::super::super::details::{caps, gap, kv_rows, panel};
use super::super::super::profile::{field_style, Remote};
use super::super::super::{cards, picker, spin_secs, Msg as LMsg};
use super::super::{danger_button, fold, needs_note, Asks, Cx, Done, JobFn, Section};
use super::{send as lsend, Msg as LibMsg, Tab};
use crate::api::{
    encode_segment, Client, Episode, Item, Kind, Season, Subtitle, SubtitleResult, TmdbMatch,
};
use crate::iced_ui::spaced::mono;
use crate::iced_ui::style::{self, c};
use crate::iced_ui::widgets::{self, Kind as Btn};
use crate::ui::browse::admin::library::extras::{
    episode_code, episode_id, parse_code, release_score, video_name,
};
use crate::ui::browse::details::{bytes, TEAL};
use crate::ui::theme::ERROR;
use crate::util::fmt_runtime;

#[derive(Clone, Debug)]
pub enum Msg {
    Lang(String),
    /// The subtitle's episode, as "S01E02".
    Episode(String),
    RemoveSub(String),
    UploadSub,
    SearchSubs,
    /// Download a found subtitle (its id), for an episode.
    DownloadSub(String, Option<String>),
    TmdbQuery(String),
    TmdbSearch,
    /// Use this TMDB entry: (id, title).
    UseTmdb(String, String),
    ConfirmDelete(bool),
    Delete,
    /// Open or close a season, under Files (false) or Episodes (true).
    Fold(bool, u32),
    RefreshSeason(u32),
    FetchStill(String),
    /// Start editing an episode (by the server's id).
    Edit(String),
    EditTitle(String),
    EditAired(String),
    EditRuntime(String),
    EditOverview(text_editor::Action),
    SaveEpisode,
    CancelEdit,
}

/// One episode being edited.
struct EpisodeForm {
    id: String,
    title: String,
    aired: String,
    /// Minutes.
    runtime: String,
    overview: text_editor::Content,
}

/// What the subtitle and TMDB lookups found, for the open title.
#[derive(Default)]
pub struct Extras {
    subs: Remote<Vec<Subtitle>>,
    found: Remote<Vec<SubtitleResult>>,
    lang: String,
    episode: String,
    tmdb_q: String,
    matches: Remote<Vec<TmdbMatch>>,
    confirm_delete: bool,
    edit: Option<EpisodeForm>,
    /// Seasons opened (or, with one season, closed) under Files and Episodes.
    folded: [HashSet<u32>; 2],
}

/// A season to pick, by label.
#[derive(Clone, Debug, PartialEq)]
struct Pick {
    label: String,
    code: String,
}

impl fmt::Display for Pick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

fn send(m: Msg) -> LMsg {
    lsend(LibMsg::X(m))
}

/// The series' seasons: from the server's record, else the library's.
fn seasons_of(item: &Item, cx: &Cx) -> Vec<Season> {
    if !item.seasons.is_empty() {
        return item.seasons.clone();
    }
    cx.lib
        .find(&item.id)
        .map(|i| i.seasons.clone())
        .unwrap_or_default()
}

impl Extras {
    /// Start over for another title.
    pub fn reset(&mut self) {
        *self = Self {
            lang: "en".into(),
            ..Default::default()
        };
    }

    /// Load the subtitle list again (after a change).
    pub fn refresh(&mut self) {
        self.subs.retry();
    }

    pub fn busy(&self) -> bool {
        self.subs.loading() || self.found.loading() || self.matches.loading()
    }

    pub fn work(&mut self, client: &Client, asks: &mut Asks, item: &Item, tab: Tab) {
        self.subs.poll(&mut asks.login);
        self.found.poll(&mut asks.login);
        self.matches.poll(&mut asks.login);
        if tab == Tab::Subtitles && self.subs.due() {
            let (c, p) = (client.clone(), Client::item_path(item));
            self.subs
                .load(move || c.admin_get(&format!("{p}/subtitles")));
        }
    }

    /// The subtitle query: language, and the episode for a series.
    fn query(&self, item: &Item) -> String {
        let mut q = format!("language={}", encode_segment(self.lang.trim()));
        if item.kind == Kind::Series && !self.episode.trim().is_empty() {
            q.push_str(&format!("&episode={}", encode_segment(self.episode.trim())));
        }
        q
    }

    pub fn update(&mut self, m: Msg, client: &Client, item: &Item) -> Option<JobFn> {
        let path = Client::item_path(item);
        match m {
            Msg::Lang(l) => self.lang = l,
            Msg::Episode(e) => self.episode = e,
            Msg::RemoveSub(id) => {
                return Some(Box::new(move |c: &Client| {
                    c.admin_call(
                        "DELETE",
                        &format!("{path}/subtitles/{}", encode_segment(&id)),
                        None,
                    )?;
                    Ok(Done::msg("Removed the subtitle."))
                }));
            }
            Msg::UploadSub => {
                let file = rfd::FileDialog::new()
                    .add_filter("Subtitles", &["srt", "vtt", "ass", "ssa"])
                    .pick_file()?;
                let q = self.query(item);
                return Some(Box::new(move |c: &Client| {
                    let data = std::fs::read(&file)?;
                    c.admin_upload(&format!("{path}/subtitles?{q}"), &data, "text/plain")?;
                    Ok(Done::msg("Uploaded the subtitle."))
                }));
            }
            Msg::SearchSubs => {
                let (c, q) = (client.clone(), self.query(item));
                self.found = Remote::default();
                self.found
                    .load(move || c.admin_get(&format!("{path}/subtitles/search?{q}")));
            }
            Msg::DownloadSub(id, episode) => {
                let q = episode
                    .as_ref()
                    .map_or(String::new(), |e| format!("?episode={}", encode_segment(e)));
                let body = match &episode {
                    Some(e) => json!({ "id": id, "episode": e }),
                    None => json!({ "id": id }),
                };
                return Some(Box::new(move |c: &Client| {
                    c.admin_call(
                        "POST",
                        &format!("{path}/subtitles/download{q}"),
                        Some(&body),
                    )?;
                    Ok(Done::msg("Downloaded the subtitle."))
                }));
            }
            Msg::TmdbQuery(q) => self.tmdb_q = q,
            Msg::TmdbSearch => {
                let (c, q) = (client.clone(), encode_segment(self.tmdb_q.trim()));
                self.matches = Remote::default();
                self.matches
                    .load(move || c.admin_get(&format!("{path}/tmdb-search?q={q}")));
            }
            Msg::UseTmdb(id, title) => {
                return Some(Box::new(move |c: &Client| {
                    c.admin_call(
                        "PUT",
                        &format!("{path}/tmdb"),
                        Some(&json!({ "tmdb_id": id, "refetch": true })),
                    )?;
                    Ok(Done {
                        msg: format!("Matched to {title} and refreshed its details."),
                        library: true,
                        ..Default::default()
                    })
                }));
            }
            Msg::ConfirmDelete(on) => self.confirm_delete = on,
            Msg::Delete => {
                self.confirm_delete = false;
                let t = item.title.clone();
                return Some(Box::new(move |c: &Client| {
                    c.admin_call("DELETE", &path, None)?;
                    Ok(Done {
                        msg: format!("Deleted {t}."),
                        library: true,
                        ..Default::default()
                    })
                }));
            }
            Msg::Fold(eps, n) => {
                let set = &mut self.folded[usize::from(eps)];
                if !set.remove(&n) {
                    set.insert(n);
                }
            }
            Msg::RefreshSeason(n) => {
                return Some(Box::new(move |c: &Client| {
                    c.admin_call("POST", &format!("{path}/seasons/{n}/refresh"), None)?;
                    Ok(Done {
                        msg: format!("Refreshed season {n}."),
                        library: true,
                        ..Default::default()
                    })
                }));
            }
            Msg::FetchStill(id) => {
                return Some(Box::new(move |c: &Client| {
                    c.admin_call(
                        "POST",
                        &format!("episodes/{}/fetch-image", encode_segment(&id)),
                        None,
                    )?;
                    Ok(Done {
                        msg: "Fetched the episode image.".into(),
                        library: true,
                        ..Default::default()
                    })
                }));
            }
            Msg::Edit(id) => {
                let e = item
                    .seasons
                    .iter()
                    .flat_map(|s| &s.episodes)
                    .find(|e| episode_id(e) == id)?;
                self.edit = Some(EpisodeForm {
                    id,
                    title: e.title.clone(),
                    aired: e.aired.clone().unwrap_or_default(),
                    runtime: e
                        .duration
                        .map(|d| format!("{:.0}", d / 60.0))
                        .unwrap_or_default(),
                    overview: text_editor::Content::with_text(&e.overview),
                });
            }
            Msg::EditTitle(s) => self.edit.as_mut()?.title = s,
            Msg::EditAired(s) => self.edit.as_mut()?.aired = s,
            Msg::EditRuntime(s) => self.edit.as_mut()?.runtime = s,
            Msg::EditOverview(a) => self.edit.as_mut()?.overview.perform(a),
            Msg::CancelEdit => self.edit = None,
            Msg::SaveEpisode => {
                let f = self.edit.take()?;
                let minutes = f.runtime.trim().parse::<f64>().ok();
                let aired = f.aired.trim();
                let body = json!({
                    "title": f.title.trim(),
                    "aired": if aired.is_empty() { Value::Null } else { json!(aired) },
                    "duration": minutes.map(|m| m * 60.0),
                    "overview": f.overview.text().trim(),
                });
                let id = f.id;
                return Some(Box::new(move |c: &Client| {
                    c.admin_call(
                        "PATCH",
                        &format!("episodes/{}", encode_segment(&id)),
                        Some(&body),
                    )?;
                    Ok(Done {
                        msg: "Saved the episode.".into(),
                        library: true,
                        ..Default::default()
                    })
                }));
            }
        }
        None
    }

    /// Whether a season's list shows: opened, or the only one and not closed.
    fn open(&self, eps: bool, n: u32, seasons: usize) -> bool {
        self.folded[usize::from(eps)].contains(&n) != (seasons == 1)
    }

    pub fn files_tab<'a>(&'a self, cx: &Cx<'a>, item: &Item) -> Element<'a, LMsg> {
        if item.kind == Kind::Movie {
            let body: Element<'a, LMsg> = match &item.file {
                Some(f) => file_rows(f),
                None => needs_note(
                    "The server doesn't send file details for this title.",
                    Section::Library,
                ),
            };
            return panel(column![caps("Video file", c(TEAL())), gap(6.0), body]);
        }
        let seasons = seasons_of(item, cx);
        let mut col: Column<'a, LMsg> = column![].spacing(4);
        if seasons.is_empty() {
            col = col.push(text("No episodes.").size(14.5).color(style::MUTED()));
        }
        for s in &seasons {
            let open = self.open(false, s.number, seasons.len());
            col = col.push(fold(s.label(), open, send(Msg::Fold(false, s.number))));
            if !open {
                continue;
            }
            for e in &s.episodes {
                let f = e.file.clone().unwrap_or_default();
                let mut bits = vec![format!("E{:02} {}", e.number, e.title)];
                bits.extend(f.quality.clone());
                bits.extend(f.container.clone());
                bits.extend(f.video_codec.clone());
                bits.extend(f.size.map(bytes));
                col = col.push(crate::iced_ui::chrome::tip_below(
                    mono(&bits.join("   ·   "), 10.5),
                    &f.name.clone().unwrap_or_default(),
                ));
            }
        }
        col.into()
    }

    pub fn subtitles_tab<'a>(&'a self, cx: &Cx<'a>, item: &Item) -> Element<'a, LMsg> {
        if self.subs.missing {
            return needs_note("This server can't manage subtitles yet.", Section::Library);
        }
        let busy = cx.busy;
        let series = item.kind == Kind::Series;
        let mut list: Column<'a, LMsg> = column![caps("Subtitles", c(TEAL())), gap(8.0)];
        match &self.subs.data {
            None if self.subs.loading() => {
                list = list.push(crate::iced_ui::icon::spinner(
                    spin_secs(),
                    style::MUTED(),
                    16.0,
                ));
            }
            Some(l) if l.is_empty() => {
                list = list.push(text("None yet.").size(14.5).color(style::MUTED()));
            }
            Some(l) => {
                for s in l {
                    let mut label = s.label.clone().unwrap_or_else(|| s.language.clone());
                    if let Some(e) = &s.episode {
                        label = format!("{e}   {label}");
                    }
                    let mut r = row![text(label).size(14.5)]
                        .spacing(10)
                        .align_y(Vertical::Center);
                    if let Some(src) = &s.source {
                        r = r.push(mono(src, 9.5).color(style::MUTED()));
                    }
                    r = r.push(space::horizontal()).push(widgets::button_with(
                        Btn::Small,
                        "Remove",
                        None,
                        (!busy).then(|| send(Msg::RemoveSub(s.id.clone()))),
                    ));
                    list = list.push(r);
                }
            }
            None => {
                if let Some(e) = &self.subs.error {
                    list = list.push(text(e.clone()).size(14.5).color(style::ERROR()));
                }
            }
        }

        let mut add: Column<'a, LMsg> = column![caps("Add a subtitle", c(TEAL())), gap(8.0)];
        let mut pickers = row![column![
            mono("Language", 10.5).color(style::MUTED()),
            text_input("en", &self.lang)
                .on_input(|s| send(Msg::Lang(s)))
                .size(15)
                .padding([9, 10])
                .width(120)
                .style(field_style),
        ]]
        .spacing(12);
        if series {
            pickers = pickers.push(episode_picker(&seasons_of(item, cx), &self.episode));
        }
        add = add.push(pickers).push(gap(8.0));
        let ok = !busy && !self.lang.trim().is_empty();
        let mut buttons = row![
            widgets::button_with(
                Btn::Small,
                "Upload .srt / .vtt…",
                None,
                ok.then(|| send(Msg::UploadSub))
            ),
            widgets::button_with(
                Btn::Small,
                "Search online",
                None,
                (ok && !self.found.loading()).then(|| send(Msg::SearchSubs))
            ),
        ]
        .spacing(8)
        .align_y(Vertical::Center);
        if self.found.loading() {
            buttons = buttons.push(crate::iced_ui::icon::spinner(
                spin_secs(),
                style::MUTED(),
                14.0,
            ));
        }
        add = add.push(buttons);
        if self.found.missing {
            add = add.push(needs_note(
                "This server can't search for subtitles online.",
                Section::Library,
            ));
        } else if let Some(e) = &self.found.error {
            add = add.push(text(e.clone()).size(12).color(c(ERROR())));
        }
        if let Some(results) = &self.found.data {
            add = add.push(gap(8.0));
            if results.is_empty() {
                add = add.push(text("Nothing found.").size(14.5).color(style::MUTED()));
            }
            // subtitles are timed to one release: ones made for this file
            // first, then the server's order (most downloaded)
            let video = video_name(item, &self.episode);
            let mut ranked: Vec<(i32, &SubtitleResult)> = results
                .iter()
                .map(|r| {
                    let rel = r.release.as_deref().unwrap_or(&r.name);
                    let s = if r.moviehash_match {
                        100
                    } else {
                        release_score(&video, rel)
                    };
                    (s, r)
                })
                .collect();
            ranked.sort_by_key(|(s, _)| -s);
            if !video.trim().is_empty() && !results.is_empty() {
                add = add
                    .push(
                        text(format!("Your file: {}", video.trim()))
                            .size(12)
                            .color(style::MUTED()),
                    )
                    .push(gap(4.0));
            }
            for (score, r) in ranked.into_iter().take(30) {
                let mut line = r.release.clone().unwrap_or_else(|| r.name.clone());
                if let Some(e) = &r.episode {
                    line = format!("{e}   {line}");
                }
                let mut rw = row![].spacing(10).align_y(Vertical::Center);
                if score >= 4 {
                    rw = rw.push(mono("Matches your file", 9.5).color(c(TEAL())));
                }
                rw = rw.push(
                    container(text(line).size(12).wrapping(text::Wrapping::None))
                        .width(Length::Fill)
                        .clip(true),
                );
                if let Some(d) = r.downloads {
                    rw = rw.push(mono(&format!("{d} downloads"), 9.5).color(style::MUTED()));
                }
                // which episode it's for: the one picked, else the result's
                let episode = Some(self.episode.trim())
                    .filter(|e| !e.is_empty())
                    .map(str::to_owned)
                    .or_else(|| r.episode.clone());
                rw = rw.push(widgets::button_with(
                    Btn::Small,
                    "Download",
                    None,
                    (!busy).then(|| send(Msg::DownloadSub(r.id.clone(), episode))),
                ));
                add = add.push(rw);
            }
        }
        column![panel(list), gap(16.0), panel(add)].into()
    }

    pub fn episodes_tab<'a>(&'a self, cx: &Cx<'a>, item: &Item) -> Element<'a, LMsg> {
        let seasons = seasons_of(item, cx);
        if seasons.is_empty() {
            return text("No episodes.").size(14.5).color(style::MUTED()).into();
        }
        let mut col: Column<'a, LMsg> = column![].spacing(4);
        for s in &seasons {
            let open = self.open(true, s.number, seasons.len());
            col = col.push(fold(
                format!("{}   ·   {} episodes", s.label(), s.episodes.len()),
                open,
                send(Msg::Fold(true, s.number)),
            ));
            if !open {
                continue;
            }
            col = col
                .push(widgets::button_with(
                    Btn::Small,
                    "Refresh season from TMDB",
                    None,
                    (!cx.busy).then(|| send(Msg::RefreshSeason(s.number))),
                ))
                .push(gap(6.0));
            for e in &s.episodes {
                col = col.push(self.episode_row(cx, e)).push(gap(4.0));
            }
        }
        col.into()
    }

    fn episode_row<'a>(&'a self, cx: &Cx<'a>, e: &Episode) -> Element<'a, LMsg> {
        let id = episode_id(e).to_owned();
        let mut bits = Vec::new();
        bits.extend(e.aired.clone());
        bits.extend(e.duration.map(fmt_runtime));
        let line = row![
            cards::art(cx.images, e.still.as_deref(), &e.title, 96.0, 54.0, 3.0),
            column![
                text(format!("{}. {}", e.number, e.title)).size(14.5),
                mono(&bits.join("   ·   "), 10.0).color(style::MUTED()),
            ]
            .spacing(2)
            .width(Length::Fill),
            widgets::button_with(Btn::Small, "Edit", None, Some(send(Msg::Edit(id.clone())))),
            widgets::button_with(
                Btn::Small,
                "Fetch still",
                None,
                (!cx.busy).then(|| send(Msg::FetchStill(id.clone())))
            ),
        ]
        .spacing(10)
        .align_y(Vertical::Center);
        let Some(f) = self.edit.as_ref().filter(|f| f.id == id) else {
            return line.into();
        };
        let labelled = |l: &str, v: &str, hint: &str, w: f32, on: fn(String) -> Msg| {
            column![
                mono(l, 10.5).color(style::MUTED()),
                text_input(hint, v)
                    .on_input(move |s| send(on(s)))
                    .size(15)
                    .padding([9, 10])
                    .style(field_style),
            ]
            .width(w)
        };
        column![
            line,
            container(
                column![
                    labelled("Title", &f.title, "", 9999.0, Msg::EditTitle),
                    gap(6.0),
                    row![
                        labelled("Aired", &f.aired, "YYYY-MM-DD", 160.0, Msg::EditAired),
                        labelled("Runtime (min)", &f.runtime, "42", 120.0, Msg::EditRuntime),
                    ]
                    .spacing(12),
                    gap(6.0),
                    mono("Overview", 10.5).color(style::MUTED()),
                    text_editor(&f.overview)
                        .on_action(|a| send(Msg::EditOverview(a)))
                        .height(80)
                        .padding([8, 10])
                        .size(15),
                    gap(6.0),
                    row![
                        widgets::primary(
                            "Save episode",
                            (!cx.busy).then(|| send(Msg::SaveEpisode))
                        ),
                        widgets::button_with(
                            Btn::Small,
                            "Cancel",
                            None,
                            Some(send(Msg::CancelEdit))
                        ),
                    ]
                    .spacing(8),
                ]
                .width(Length::Fill)
            )
            .padding(8),
        ]
        .into()
    }

    /// Details tab, under the fields: pick the right TMDB entry, or delete the title.
    pub fn match_and_delete<'a>(&'a self, cx: &Cx<'a>, item: &Item) -> Element<'a, LMsg> {
        let busy = cx.busy;
        let now = item
            .tmdb_id
            .as_ref()
            .map(|v| v.to_string().trim_matches('"').to_owned());
        let q = if self.tmdb_q.is_empty() {
            item.title.clone()
        } else {
            self.tmdb_q.clone()
        };
        let mut tmdb: Column<'a, LMsg> = column![
            caps("TMDB match", c(TEAL())),
            gap(4.0),
            text(match &now {
                Some(id) => format!(
                    "Matched to TMDB {id}. Wrong film or show? Search and pick the right one."
                ),
                None => "Not matched to TMDB. Search and pick the right entry.".into(),
            })
            .size(12)
            .color(style::MUTED()),
            gap(6.0),
            row![
                text_input("", &q)
                    .on_input(|s| send(Msg::TmdbQuery(s)))
                    .on_submit(send(Msg::TmdbSearch))
                    .size(15)
                    .padding([8, 10])
                    .style(field_style),
                widgets::button_with(
                    Btn::Small,
                    "Search",
                    None,
                    (!self.matches.loading()).then(|| send(Msg::TmdbSearch))
                ),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
        ];
        if self.matches.missing {
            tmdb = tmdb.push(needs_note(
                "This server can't search TMDB from the app yet.",
                Section::Library,
            ));
        } else if let Some(e) = &self.matches.error {
            tmdb = tmdb.push(text(e.clone()).size(12).color(c(ERROR())));
        }
        if let Some(list) = &self.matches.data {
            tmdb = tmdb.push(gap(6.0));
            if list.is_empty() {
                tmdb = tmdb.push(text("Nothing found.").size(14.5).color(style::MUTED()));
            }
            for m in list.iter().take(10) {
                let year = m.year.map(|y| format!(" ({y})")).unwrap_or_default();
                let mut info: Column<'a, LMsg> =
                    column![text(format!("{}{year}", m.title)).size(14.5)];
                if let Some(o) = &m.overview {
                    info = info.push(
                        container(
                            text(o.clone())
                                .size(12)
                                .color(style::MUTED())
                                .wrapping(text::Wrapping::None),
                        )
                        .clip(true),
                    );
                }
                tmdb = tmdb.push(
                    row![
                        cards::art(cx.images, m.poster.as_deref(), &m.title, 34.0, 51.0, 3.0),
                        info.width(Length::Fill),
                        widgets::button_with(
                            Btn::Small,
                            "Use this",
                            None,
                            (!busy).then(|| send(Msg::UseTmdb(m.tmdb_id.clone(), m.title.clone())))
                        ),
                    ]
                    .spacing(10)
                    .align_y(Vertical::Center),
                );
            }
        }

        let mut delete: Column<'a, LMsg> = column![
            caps("Delete", c(ERROR())),
            gap(4.0),
            text(
                "Removes the title from the library and its watch progress. The video files \
                 stay on disk unless the server says otherwise."
            )
            .size(12)
            .color(style::MUTED()),
            gap(6.0),
        ];
        delete = delete.push(if self.confirm_delete {
            Element::from(
                row![
                    danger_button(
                        &format!("Delete {}", item.title),
                        (!busy).then(|| send(Msg::Delete))
                    ),
                    widgets::button_with(
                        Btn::Small,
                        "Cancel",
                        None,
                        Some(send(Msg::ConfirmDelete(false)))
                    ),
                ]
                .spacing(8),
            )
        } else {
            danger_button("Delete title…", Some(send(Msg::ConfirmDelete(true))))
        });
        column![gap(20.0), panel(tmdb), gap(20.0), panel(delete)].into()
    }
}

/// What the server says about a video file.
fn file_rows<'a>(f: &crate::api::FileInfo) -> Element<'a, LMsg> {
    let mut rows: Vec<(&str, String, iced::Color, f32)> = Vec::new();
    let mut add = |k: &'static str, v: Option<String>| {
        if let Some(v) = v {
            rows.push((k, v, style::TEXT(), 14.0));
        }
    };
    add("File", f.name.clone());
    add("Folder", f.path.clone());
    add("Quality", f.quality.clone());
    add("Container", f.container.clone());
    add("Video", f.video_codec.clone());
    let audio = match (&f.audio_codec, f.audio_channels) {
        (Some(a), Some(c)) => Some(format!("{a}, {c} channels")),
        (a, _) => a.clone(),
    };
    add("Audio", audio);
    add(
        "Bitrate",
        f.bitrate.map(|b| format!("{:.1} Mbit/s", b as f64 / 1e6)),
    );
    add("Size", f.size.map(bytes));
    if rows.is_empty() {
        text("The server doesn't describe this file.")
            .size(14.5)
            .color(style::MUTED())
            .into()
    } else {
        kv_rows(rows)
    }
}

/// Season and episode dropdowns for the subtitle's episode, kept as `code`
/// ("S01E02"; empty: none picked).
fn episode_picker<'a>(seasons: &[Season], code: &str) -> Element<'a, LMsg> {
    let picked = parse_code(code);
    let season = picked.and_then(|(n, _)| seasons.iter().find(|s| s.number == n));
    // a season picks its first episode, so a choice is always made
    let season_opts: Vec<Pick> = seasons
        .iter()
        .map(|s| Pick {
            label: s.label(),
            code: s
                .episodes
                .first()
                .map(|e| episode_code(s.number, e.number))
                .unwrap_or_default(),
        })
        .collect();
    let chosen = season.and_then(|s| season_opts.iter().find(|p| p.label == s.label()).cloned());
    let mut r = row![column![
        mono("Season", 10.5).color(style::MUTED()),
        picker(
            pick_list(season_opts, chosen, |p| send(Msg::Episode(p.code)))
                .placeholder("Choose…")
                .width(140)
        ),
    ]]
    .spacing(12);
    let Some(season) = season else {
        return r.into();
    };
    let name = |e: &Episode| match e.title.trim() {
        "" => format!("Episode {}", e.number),
        t => format!("{}. {t}", e.number),
    };
    let ep_opts: Vec<Pick> = season
        .episodes
        .iter()
        .map(|e| Pick {
            label: name(e),
            code: episode_code(season.number, e.number),
        })
        .collect();
    let chosen = picked.and_then(|(_, n)| {
        let code = episode_code(season.number, n);
        ep_opts.iter().find(|p| p.code == code).cloned()
    });
    r = r.push(column![
        mono("Episode", 10.5).color(style::MUTED()),
        picker(
            pick_list(ep_opts, chosen, |p| send(Msg::Episode(p.code)))
                .placeholder("Choose…")
                .width(260)
        ),
    ]);
    r.into()
}
