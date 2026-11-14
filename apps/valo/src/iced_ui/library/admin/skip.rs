//! Admin › Skip markers: where episodes' recap, intro and credits are, for the
//! player's Skip Recap / Skip Intro / Next Episode buttons. Series without
//! times are picked and sent off to be detected (the server reads chapter
//! names and matches the theme song and credits music between episodes); the
//! ones with times are listed apart, where an episode's times can be fixed.
use std::collections::HashSet;

use iced::alignment::Vertical;
use iced::widget::{button, checkbox, column, container, row, space, text, text_input, Column};
use iced::{Background, Border, Element, Length, Padding};
use serde::Deserialize;
use serde_json::{json, Value};

use super::super::details::{caps, gap, panel};
use super::super::profile::Remote;
use super::super::{spin_secs, Msg as LMsg};
use super::{danger_button, not_set_up, Asks, Cx, Done, JobFn, Msg as AMsg, Section};
use crate::api::{Client, Item, Kind};
use crate::iced_ui::icon;
use crate::iced_ui::spaced::mono;
use crate::iced_ui::style::{self, c, BOLD};
use crate::iced_ui::widgets::{self, Kind as Btn};
use crate::ui::browse::details::TEAL;
use crate::ui::theme::{LINE, LINE_HI, OK, SURFACE, SURFACE_HI, WARN};
use crate::util::fmt_time;

/// How many series to list before asking for a search.
const SHOW: usize = 300;
/// How often to look again while the server is detecting, in seconds.
const POLL: u64 = 2;

/// `GET /api/admin/skip-markers`
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct List {
    pub running: bool,
    pub series: Vec<Row>,
}

/// A series and how many of its episodes have times.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Row {
    pub id: String,
    pub title: String,
    pub year: Option<u32>,
    pub episodes: usize,
    pub intro: usize,
    pub recap: usize,
    pub credits: usize,
    /// Episodes with any time.
    pub any: usize,
    /// Episodes detection has looked at.
    pub checked: usize,
    pub job: Option<JobState>,
}

/// A series' latest detection run.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct JobState {
    /// queued, running, done or failed
    pub state: String,
    pub message: Option<String>,
}

impl JobState {
    fn busy(&self) -> bool {
        matches!(self.state.as_str(), "queued" | "running")
    }
}

/// `GET /api/admin/skip-markers/{id}`
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Detail {
    pub id: String,
    pub title: String,
    pub job: Option<JobState>,
    pub episodes: Vec<Episode>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Episode {
    pub id: String,
    pub season: u32,
    pub number: u32,
    pub title: String,
    pub recap: Option<[f64; 2]>,
    pub intro: Option<[f64; 2]>,
    pub credits: Option<[f64; 2]>,
    /// chapters, audio or manual
    pub source: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Tab {
    #[default]
    Missing,
    Found,
}

/// An episode's times being edited: the text of each field.
struct Form {
    episode: String,
    /// recap, intro, credits
    fields: [String; 3],
    /// The fields as first shown: unchanged ones aren't sent, so times
    /// with fractions of a second aren't rounded by saving.
    shown: [String; 3],
    error: Option<String>,
}

const KINDS: [&str; 3] = ["recap", "intro", "credits"];

#[derive(Clone, Debug)]
pub enum Msg {
    Tab(Tab),
    Search(String),
    Pick(String, bool),
    /// Tick every series shown, or none.
    PickAll(bool),
    /// Find times for the ticked series.
    Detect,
    /// Open a series' episodes (None: close).
    Open(Option<String>),
    /// Find the series' missing times (false), or every detected one again (true).
    Again(String, bool),
    ConfirmClear(bool),
    Clear(String),
    /// Edit an episode's times (None: stop).
    Edit(Option<String>),
    Field(usize, String),
    Save,
}

#[derive(Default)]
pub struct State {
    list: Remote<List>,
    tab: Tab,
    search: String,
    /// Server ids of the ticked series.
    picked: HashSet<String>,
    open: Option<String>,
    detail: Remote<Detail>,
    confirm_clear: bool,
    form: Option<Form>,
}

fn send(m: Msg) -> LMsg {
    LMsg::Admin(AMsg::Skip(m))
}

impl State {
    pub fn refresh(&mut self) {
        self.list.retry();
        self.detail.retry();
    }

    pub fn missing(&self) -> bool {
        self.list.missing
    }

    pub fn busy(&self) -> bool {
        self.list.loading() || self.detail.loading()
    }

    /// Something is being detected: keep looking.
    fn detecting(&self) -> bool {
        self.list.data.as_ref().is_some_and(|l| {
            l.running
                || l.series
                    .iter()
                    .any(|s| s.job.as_ref().is_some_and(JobState::busy))
        })
    }

    /// How many times the list counts, all series together: a change means
    /// new times for the player.
    fn coverage_total(&self) -> Option<usize> {
        let l = self.list.data.as_ref()?;
        Some(l.series.iter().map(|r| r.intro + r.recap + r.credits).sum())
    }

    pub fn work(&mut self, client: &Client, asks: &mut Asks) {
        let (was, before) = (self.detecting(), self.coverage_total());
        // a run finished, seen running or not (a quick one can end between
        // polls): the library has new times for the player
        if self.list.poll(&mut asks.login)
            && !self.detecting()
            && (was || (before.is_some() && before != self.coverage_total()))
        {
            asks.reload_library = true;
        }
        let login = &mut asks.login;
        if self.list.due() || (self.detecting() && self.list.stale(POLL)) {
            let c = client.clone();
            self.list.load(move || c.admin_get("skip-markers"));
        }
        self.detail.poll(login);
        if let Some(id) = self.open.clone() {
            // another series' episodes are shown: load this one's, but not again
            // and again after a failed try (Open and Refresh retry)
            let wrong = self.detail.data.as_ref().is_some_and(|d| d.id != id)
                && self.detail.error.is_none()
                && !self.detail.missing;
            let running = self
                .detail
                .data
                .as_ref()
                .and_then(|d| d.job.as_ref())
                .is_some_and(JobState::busy);
            if (self.detail.due() || wrong || (running && self.detail.stale(POLL)))
                && !self.detail.loading()
            {
                let c = client.clone();
                self.detail
                    .load(move || c.admin_get(&format!("skip-markers/{id}")));
            }
        }
    }

    pub fn update(&mut self, m: Msg, cx_rows: &[Row]) -> Option<JobFn> {
        match m {
            Msg::Tab(t) => {
                self.tab = t;
                self.open = None;
                self.form = None;
            }
            Msg::Search(s) => self.search = s,
            Msg::Pick(id, on) => {
                if on {
                    self.picked.insert(id);
                } else {
                    self.picked.remove(&id);
                }
            }
            Msg::PickAll(on) => {
                if on {
                    self.picked = self
                        .shown(cx_rows)
                        .iter()
                        .take(SHOW)
                        .map(|r| r.id.clone())
                        .collect();
                } else {
                    self.picked.clear();
                }
            }
            Msg::Detect => {
                let ids: Vec<String> = std::mem::take(&mut self.picked).into_iter().collect();
                if ids.is_empty() {
                    return None;
                }
                return Some(detect(ids, false));
            }
            Msg::Open(id) => {
                self.open = id;
                self.form = None;
                self.confirm_clear = false;
                self.detail.retry();
            }
            Msg::Again(id, redo) => return Some(detect(vec![id], redo)),
            Msg::ConfirmClear(on) => self.confirm_clear = on,
            Msg::Clear(id) => {
                self.confirm_clear = false;
                self.open = None;
                let title = title_of(cx_rows, &id);
                return Some(Box::new(move |c: &Client| {
                    c.admin_call("DELETE", &format!("skip-markers/{id}"), None)?;
                    Ok(Done {
                        msg: format!("Cleared the skip times of {title}"),
                        library: true,
                        ..Default::default()
                    })
                }));
            }
            Msg::Edit(None) => self.form = None,
            Msg::Edit(Some(id)) => {
                let ep = self
                    .detail
                    .data
                    .as_ref()?
                    .episodes
                    .iter()
                    .find(|e| e.id == id)?;
                let fields =
                    [ep.recap, ep.intro, ep.credits].map(|t| t.map(range_text).unwrap_or_default());
                self.form = Some(Form {
                    episode: id,
                    shown: fields.clone(),
                    fields,
                    error: None,
                });
            }
            Msg::Field(i, v) => {
                if let Some(f) = self.form.as_mut() {
                    f.fields[i] = v;
                    f.error = None;
                }
            }
            Msg::Save => {
                let f = self.form.as_mut()?;
                let mut body = serde_json::Map::new();
                for (i, kind) in KINDS.iter().enumerate() {
                    if f.fields[i].trim() == f.shown[i].trim() {
                        continue;
                    }
                    match parse_range(&f.fields[i]) {
                        Ok(t) => {
                            body.insert(
                                (*kind).into(),
                                t.map_or(Value::Null, |[a, b]| json!([a, b])),
                            );
                        }
                        Err(e) => {
                            f.error = Some(format!("{}: {e}", capital(kind)));
                            return None;
                        }
                    }
                }
                let id = f.episode.clone();
                self.form = None;
                if body.is_empty() {
                    return None; // nothing changed
                }
                return Some(Box::new(move |c: &Client| {
                    c.admin_call(
                        "PATCH",
                        &format!("skip-markers/episodes/{id}"),
                        Some(&Value::Object(body)),
                    )?;
                    Ok(Done {
                        msg: "Saved the episode's skip times".into(),
                        library: true,
                        ..Default::default()
                    })
                }));
            }
        }
        None
    }

    /// The series of the open tab that the search finds.
    fn shown<'r>(&self, rows: &'r [Row]) -> Vec<&'r Row> {
        let q = self.search.trim().to_lowercase();
        rows.iter()
            .filter(|r| (r.any > 0) == (self.tab == Tab::Found))
            .filter(|r| q.is_empty() || r.title.to_lowercase().contains(&q))
            .collect()
    }

    /// The rows, for update().
    pub fn rows(&self) -> Vec<Row> {
        self.list
            .data
            .as_ref()
            .map(|l| l.series.clone())
            .unwrap_or_default()
    }

    pub fn view<'a>(&'a self, cx: &Cx<'a>) -> Element<'a, LMsg> {
        if self.list.missing {
            return not_set_up(Section::SkipMarkers, &cx.client.info.name);
        }
        let mut col: Column<'a, LMsg> = column![];
        if let Some(e) = &self.list.error {
            col = col
                .push(
                    text(format!("Couldn't load the list: {e}"))
                        .size(14.5)
                        .color(style::ERROR()),
                )
                .push(gap(8.0));
        }
        let Some(list) = self.list.data.as_ref() else {
            return col
                .push(icon::spinner(spin_secs(), style::MUTED(), 18.0))
                .into();
        };
        let found = list.series.iter().filter(|r| r.any > 0).count();
        col = col
            .push(widgets::segmented(vec![
                (
                    "Not detected".into(),
                    Some(list.series.len() - found),
                    self.tab == Tab::Missing,
                    send(Msg::Tab(Tab::Missing)),
                ),
                (
                    "Detected".into(),
                    Some(found),
                    self.tab == Tab::Found,
                    send(Msg::Tab(Tab::Found)),
                ),
            ]))
            .push(gap(16.0))
            .push(container(field(&self.search)).max_width(420))
            .push(gap(12.0));

        let shown = self.shown(&list.series);
        col = match self.tab {
            Tab::Missing => col.push(self.missing_view(&shown, cx)),
            Tab::Found => col.push(self.found_view(&shown, cx)),
        };
        col.into()
    }

    fn missing_view<'a>(&'a self, shown: &[&'a Row], cx: &Cx<'a>) -> Element<'a, LMsg> {
        let n = self.picked.len();
        let label = match n {
            0 => "Find times".to_owned(),
            1 => "Find times for 1 series".to_owned(),
            n => format!("Find times for {n} series"),
        };
        let all_on =
            !shown.is_empty() && shown.iter().take(SHOW).all(|r| self.picked.contains(&r.id));
        let mut col: Column<'a, LMsg> =
            column![
                panel(column![
                caps("Find the times", c(TEAL())),
                gap(6.0),
                text(
                    "Tick the series to look at. The server reads each episode's chapter names, \
                     then matches the theme song and the credits music between episodes. It \
                     takes about a minute a season, in the background; times already there stay \
                     as they are."
                )
                .size(14.5)
                .color(style::MUTED()),
                gap(12.0),
                row![
                    widgets::button_with(
                        Btn::Primary,
                        &label,
                        None,
                        (n > 0 && !cx.busy).then(|| send(Msg::Detect))
                    ),
                    widgets::button_with(
                        Btn::Small,
                        if all_on { "Untick all" } else { "Tick all shown" },
                        None,
                        (!shown.is_empty()).then(|| send(Msg::PickAll(!all_on)))
                    ),
                ]
                .spacing(10)
                .align_y(Vertical::Center),
            ]),
                gap(12.0),
            ];
        if shown.is_empty() {
            col = col.push(
                text(if self.search.trim().is_empty() {
                    "Every series has times."
                } else {
                    "No series by that name here."
                })
                .size(14.5)
                .color(style::MUTED()),
            );
        }
        for r in shown.iter().take(SHOW) {
            let id = r.id.clone();
            let mut b = checkbox(self.picked.contains(&r.id))
                .size(16)
                .style(check_style);
            if !r.job.as_ref().is_some_and(JobState::busy) {
                b = b.on_toggle(move |v| send(Msg::Pick(id.clone(), v)));
            }
            let mut line = row![
                b,
                art(cx, &r.id, &r.title),
                text(r.title.clone()).size(14.5)
            ]
            .spacing(10)
            .align_y(Vertical::Center);
            if let Some(y) = r.year {
                line = line.push(mono(&y.to_string(), 10.5).color(style::MUTED()));
            }
            line = line
                .push(space::horizontal())
                .push(text(episodes(r.episodes)).size(13).color(style::MUTED()))
                .push(job_view(r));
            col = col.push(container(line).padding([4, 0]));
        }
        if shown.len() > SHOW {
            col = col.push(more(shown.len() - SHOW));
        }
        col.into()
    }

    fn found_view<'a>(&'a self, shown: &[&'a Row], cx: &Cx<'a>) -> Element<'a, LMsg> {
        let mut col: Column<'a, LMsg> = column![];
        if shown.is_empty() {
            col = col.push(
                text(if self.search.trim().is_empty() {
                    "No series has times yet. Find them under Not detected."
                } else {
                    "No series by that name here."
                })
                .size(14.5)
                .color(style::MUTED()),
            );
        }
        for r in shown.iter().take(SHOW) {
            let open = self.open.as_deref() == Some(r.id.as_str());
            let head = button(
                row![
                    icon::icon(
                        if open {
                            icon::Icon::ChevronDown
                        } else {
                            icon::Icon::ChevronRight
                        },
                        style::MUTED(),
                        14.0
                    ),
                    art(cx, &r.id, &r.title),
                    column![
                        text(r.title.clone()).font(BOLD).size(15),
                        text(coverage(r)).size(13).color(style::MUTED()),
                    ]
                    .spacing(2),
                    space::horizontal(),
                    job_view(r),
                ]
                .spacing(10)
                .align_y(Vertical::Center),
            )
            .width(Length::Fill)
            .padding([6, 0])
            .style(|_, _| button::Style {
                text_color: style::TEXT(),
                ..button::Style::default()
            })
            .on_press(send(Msg::Open((!open).then(|| r.id.clone()))));
            col = col.push(head);
            if open {
                col = col
                    .push(container(self.detail_view(r, cx)).padding(Padding::default().left(24.0)))
                    .push(gap(10.0));
            }
        }
        if shown.len() > SHOW {
            col = col.push(more(shown.len() - SHOW));
        }
        col.into()
    }

    /// An open series: what to do with it, then its episodes.
    fn detail_view<'a>(&'a self, r: &'a Row, cx: &Cx<'a>) -> Element<'a, LMsg> {
        let busy = cx.busy || r.job.as_ref().is_some_and(JobState::busy);
        let id = r.id.clone();
        let clear: Element<'a, LMsg> = if self.confirm_clear {
            row![
                danger_button(
                    &format!("Clear {}", r.title),
                    (!busy).then(|| send(Msg::Clear(id.clone())))
                ),
                widgets::button_with(
                    Btn::Small,
                    "Cancel",
                    None,
                    Some(send(Msg::ConfirmClear(false)))
                ),
            ]
            .spacing(8)
            .into()
        } else {
            danger_button("Clear…", (!busy).then(|| send(Msg::ConfirmClear(true))))
        };
        let col: Column<'a, LMsg> = column![
            row![
                widgets::button_with(
                    Btn::Small,
                    "Find missing times",
                    None,
                    (!busy && (r.intro < r.episodes || r.credits < r.episodes))
                        .then(|| send(Msg::Again(id.clone(), false)))
                ),
                widgets::button_with(
                    Btn::Small,
                    "Find all again",
                    None,
                    (!busy).then(|| send(Msg::Again(id.clone(), true)))
                ),
                space::horizontal(),
                clear,
            ]
            .spacing(8)
            .align_y(Vertical::Center),
            text(
                "Find all again redoes the times found from chapters and sound. Times you set \
                 by hand stay; Clear removes those too."
            )
            .size(12)
            .color(style::MUTED()),
            gap(10.0),
        ]
        .spacing(6);

        let Some(d) = self.detail.data.as_ref().filter(|d| d.id == r.id) else {
            if let Some(e) = &self.detail.error {
                return col
                    .push(
                        text(format!("Couldn't load the episodes: {e}"))
                            .size(14.5)
                            .color(style::ERROR()),
                    )
                    .into();
            }
            return col
                .push(icon::spinner(spin_secs(), style::MUTED(), 16.0))
                .into();
        };

        let mut season = None;
        let mut table: Column<'a, LMsg> = column![];
        for e in &d.episodes {
            if season != Some(e.season) {
                season = Some(e.season);
                table = table
                    .push(gap(8.0))
                    .push(caps(&format!("Season {}", e.season), c(TEAL())));
                table = table.push(
                    row![
                        space().width(250),
                        head_cell("Recap"),
                        head_cell("Intro"),
                        head_cell("Credits"),
                    ]
                    .spacing(8),
                );
            }
            table = table.push(self.episode_row(e, busy));
        }
        col.push(panel(table)).into()
    }

    fn episode_row<'a>(&'a self, e: &'a Episode, busy: bool) -> Element<'a, LMsg> {
        // a long name wraps inside its column instead of running into Recap
        let name = text(format!("E{:02}  {}", e.number, e.title))
            .size(14)
            .width(250)
            .wrapping(text::Wrapping::WordOrGlyph);
        if let Some(f) = self.form.as_ref().filter(|f| f.episode == e.id) {
            let mut r = row![name].spacing(8).align_y(Vertical::Center);
            for i in 0..3 {
                r = r.push(
                    text_input("0:00 – 0:00", &f.fields[i])
                        .on_input(move |v| send(Msg::Field(i, v)))
                        .on_submit_maybe((!busy).then(|| send(Msg::Save)))
                        .size(13.5)
                        .padding([5, 8])
                        .width(130)
                        .style(super::super::profile::field_style),
                );
            }
            r = r
                .push(widgets::button_with(
                    Btn::Small,
                    "Save",
                    None,
                    (!busy).then(|| send(Msg::Save)),
                ))
                .push(widgets::button_with(
                    Btn::Small,
                    "Cancel",
                    None,
                    Some(send(Msg::Edit(None))),
                ));
            let mut col = column![r].spacing(4);
            col = col.push(
                text(match &f.error {
                    Some(err) => err.clone(),
                    None => "Like 1:20 – 2:05. Leave one empty to remove it.".into(),
                })
                .size(12)
                .color(if f.error.is_some() {
                    style::ERROR()
                } else {
                    style::MUTED()
                }),
            );
            return container(col).padding([4, 0]).into();
        }
        let mut r = row![name, cell(e.recap), cell(e.intro), cell(e.credits)]
            .spacing(8)
            .align_y(Vertical::Center);
        r = r.push(
            container(mono(&source_label(e.source.as_deref()), 10.0).color(style::MUTED()))
                .width(140),
        );
        r = r.push(widgets::button_with(
            Btn::Small,
            "Edit",
            None,
            (!busy).then(|| send(Msg::Edit(Some(e.id.clone())))),
        ));
        container(r).padding([3, 0]).into()
    }
}

/// Queue series for detection.
fn detect(ids: Vec<String>, redo: bool) -> JobFn {
    Box::new(move |c: &Client| {
        let nums: Vec<Value> = ids
            .iter()
            .map(|i| {
                i.parse::<u64>()
                    .map_or_else(|_| Value::from(i.as_str()), Value::from)
            })
            .collect();
        c.admin_call(
            "POST",
            "skip-markers/detect",
            Some(&json!({ "ids": nums, "redo": redo })),
        )?;
        let what = if ids.len() == 1 {
            "1 series".to_owned()
        } else {
            format!("{} series", ids.len())
        };
        Ok(Done::msg(format!(
            "Finding times for {what} in the background. This page updates as they're done."
        )))
    })
}

fn title_of(rows: &[Row], id: &str) -> String {
    rows.iter()
        .find(|r| r.id == id)
        .map_or_else(|| "the series".into(), |r| r.title.clone())
}

/// The series' poster from the library, small; a plain tile without one
/// (a lettered placeholder is unreadable this small).
fn art<'a>(cx: &Cx<'a>, id: &str, title: &str) -> Element<'a, LMsg> {
    let poster = cx
        .series
        .iter()
        .find(|i| i.kind == Kind::Series && i.server_id() == id)
        .and_then(|i: &Item| i.poster.as_deref());
    match poster {
        Some(url) => super::super::cards::art(cx.images, Some(url), title, 24.0, 36.0, 3.0),
        None => container(space())
            .width(24)
            .height(36)
            .style(|_| container::Style {
                background: Some(Background::Color(c(SURFACE_HI()))),
                border: Border {
                    color: c(LINE()),
                    width: 1.0,
                    radius: 3.0.into(),
                },
                ..container::Style::default()
            })
            .into(),
    }
}

/// What came of the series' latest run.
fn job_view<'a>(r: &Row) -> Element<'a, LMsg> {
    let Some(j) = &r.job else {
        return space().width(0).into();
    };
    let (t, col) = match j.state.as_str() {
        "queued" => ("Queued".to_owned(), style::MUTED()),
        "running" => {
            return row![
                icon::spinner(spin_secs(), style::MUTED(), 14.0),
                text("Finding times…").size(13).color(style::MUTED())
            ]
            .spacing(6)
            .align_y(Vertical::Center)
            .into();
        }
        "failed" => (
            format!("Failed: {}", j.message.clone().unwrap_or_default()),
            style::ERROR(),
        ),
        _ if r.any == 0 && r.checked > 0 => ("Nothing found".to_owned(), c(WARN())),
        _ => (j.message.clone().unwrap_or_else(|| "Done".into()), c(OK())),
    };
    text(t).size(13).color(col).into()
}

/// "Intro 10 of 10 · Credits 9 · Recap 2"
fn coverage(r: &Row) -> String {
    let mut parts = vec![format!("{} of {}", r.any, episodes(r.episodes))];
    for (n, label) in [
        (r.intro, "intro"),
        (r.credits, "credits"),
        (r.recap, "recap"),
    ] {
        if n > 0 {
            parts.push(format!("{label} {n}"));
        }
    }
    parts.join("  ·  ")
}

fn episodes(n: usize) -> String {
    if n == 1 {
        "1 episode".into()
    } else {
        format!("{n} episodes")
    }
}

fn more<'a>(n: usize) -> Element<'a, LMsg> {
    text(format!("and {n} more; search to find them"))
        .size(14.5)
        .color(style::MUTED())
        .into()
}

fn head_cell<'a>(t: &str) -> Element<'a, LMsg> {
    container(mono(t, 10.0).color(style::MUTED()))
        .width(130)
        .into()
}

/// A time range, or a dash.
fn cell<'a>(t: Option<[f64; 2]>) -> Element<'a, LMsg> {
    match t {
        Some(r) => text(range_text(r)).size(13.5).width(130).into(),
        None => text("—").size(13.5).color(style::MUTED()).width(130).into(),
    }
}

fn source_label(s: Option<&str>) -> String {
    match s {
        Some("manual") => "by hand".into(),
        Some(s) => s.replace("audio", "sound").replace(',', " + "),
        None => String::new(),
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// "1:20 – 2:05"
fn range_text([a, b]: [f64; 2]) -> String {
    format!("{} – {}", clock(a), clock(b))
}

/// m:ss, or h:mm:ss
fn clock(s: f64) -> String {
    let t = fmt_time(s.round());
    t.strip_prefix('0')
        .filter(|r| r.len() == 4)
        .map_or(t.clone(), str::to_owned)
}

/// "1:20 - 2:05" (or "80-125", "1:20–2:05") as seconds; empty is none.
fn parse_range(s: &str) -> Result<Option<[f64; 2]>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let parts: Vec<&str> = s.split(['-', '–', '—']).map(str::trim).collect();
    let [a, b] = parts.as_slice() else {
        return Err("write it as start – end, like 1:20 – 2:05".into());
    };
    let (a, b) = (parse_clock(a)?, parse_clock(b)?);
    if b <= a {
        return Err("it has to end after it starts".into());
    }
    Ok(Some([a, b]))
}

/// "1:20", "0:01:20" or "80" (seconds).
fn parse_clock(s: &str) -> Result<f64, String> {
    let mut total = 0.0;
    for (i, p) in s.split(':').enumerate() {
        if i > 2 {
            return Err(format!("{s} isn't a time"));
        }
        let v: f64 = p.trim().parse().map_err(|_| format!("{s} isn't a time"))?;
        if v < 0.0 {
            return Err(format!("{s} isn't a time"));
        }
        total = total * 60.0 + v;
    }
    Ok(total)
}

fn check_style(_: &iced::Theme, s: checkbox::Status) -> checkbox::Style {
    let hot = matches!(s, checkbox::Status::Hovered { .. });
    checkbox::Style {
        background: Background::Color(c(if hot { SURFACE_HI() } else { SURFACE() })),
        icon_color: style::TEXT(),
        border: Border {
            color: c(if hot { LINE_HI() } else { LINE() }),
            width: 1.0,
            radius: 3.0.into(),
        },
        text_color: Some(style::TEXT()),
    }
}

fn field<'a>(v: &str) -> Element<'a, LMsg> {
    text_input("Search series", v)
        .on_input(|s| send(Msg::Search(s)))
        .size(15)
        .padding([9, 10])
        .style(super::super::profile::field_style)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_untouched_times_sends_nothing() {
        let mut st = State::default();
        st.detail.data = Some(Detail {
            id: "7".into(),
            episodes: vec![Episode {
                id: "812".into(),
                intro: Some([19.07, 49.15]), // shown rounded: 0:19 – 0:49
                ..Default::default()
            }],
            ..Default::default()
        });
        st.update(Msg::Edit(Some("812".into())), &[]);
        assert!(
            st.update(Msg::Save, &[]).is_none(),
            "nothing changed, nothing sent"
        );
        st.update(Msg::Edit(Some("812".into())), &[]);
        st.update(Msg::Field(0, "0:00 - 0:10".into()), &[]);
        assert!(st.update(Msg::Save, &[]).is_some(), "the recap changed");
    }

    #[test]
    fn a_quick_run_still_counts_as_new_times() {
        let row = |intro| Row {
            id: "7".into(),
            episodes: 4,
            intro,
            ..Default::default()
        };
        let mut st = State::default();
        st.list.data = Some(List {
            running: false,
            series: vec![row(0)],
        });
        let before = st.coverage_total();
        // the run started and ended between two polls: never seen running
        st.list.data = Some(List {
            running: false,
            series: vec![row(4)],
        });
        assert!(!st.detecting());
        assert_ne!(before, st.coverage_total());
    }

    #[test]
    fn times_typed_by_hand() {
        assert_eq!(parse_range("1:20 - 2:05"), Ok(Some([80.0, 125.0])));
        assert_eq!(parse_range("80–125"), Ok(Some([80.0, 125.0])));
        assert_eq!(parse_range("0:01:20 — 0:02:00"), Ok(Some([80.0, 120.0])));
        assert_eq!(parse_range("  "), Ok(None));
        assert!(parse_range("2:00 - 1:00").is_err());
        assert!(parse_range("1:20").is_err());
        assert!(parse_range("a - b").is_err());
        assert_eq!(range_text([80.0, 125.4]), "1:20 – 2:05");
        assert_eq!(range_text([3725.0, 3800.0]), "1:02:05 – 1:03:20");
    }
}
