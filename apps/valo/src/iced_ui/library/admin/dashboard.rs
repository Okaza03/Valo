//! Admin › Dashboard: library numbers, API status, library health, syncs
//! and the server's live log.
use std::collections::HashSet;
use std::time::{Duration, Instant};

use anyhow::Result;
use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, scrollable, space, text, Column};
use iced::{Background, Border, Color, Element, Length, Padding};

use super::super::details::{caps, column_of, gap, panel, NARROW};
use super::super::profile::{stats, Remote};
use super::super::{spin_secs, Msg as LMsg};
use super::{
    danger_button, fold, needs_note, status_dot, Asks, Cx, Done, JobFn, Msg as AMsg, Section,
};
use crate::api::{AdminStatus, Client, Item, Kind, LogLine, SyncReport};
use crate::iced_ui::spaced::mono;
use crate::iced_ui::style::{self, c, BOLD, MONO};
use crate::iced_ui::widgets::{self, Kind as Btn};
use crate::iced_ui::Job;
use crate::ui::browse::details::{bytes, TEAL};
use crate::ui::theme::{ERROR, FIELD, LINE, OK, WARN};
use crate::util::now_unix;

/// How often the live log asks for new lines.
const LOG_EVERY: Duration = Duration::from_secs(3);
const LOG_KEEP: usize = 500;
/// The log's scrollable, kept at the bottom.
const LOG: &str = "admin-log";

const SYNCS: [(&str, &str, &str); 3] = [
    (
        "folders",
        "Sync folders",
        "Scans the media folders on every disk and compares them against the database. \
         Lists new and removed files and missing images; writes nothing.",
    ),
    (
        "movies",
        "Sync movies",
        "Adds new movie files to the database, removes movies whose files are gone, and \
         fetches details for the new ones.",
    ),
    (
        "series",
        "Sync series",
        "Adds new shows, seasons and episodes, removes the ones whose files are gone, and \
         fetches details for the new ones.",
    ),
];

/// Why Sync movies or Sync series waits after a scan.
const NOTHING_NEW: &str =
    "The last scan found nothing new for this. Run Sync folders again after changing files";

#[derive(Clone, Debug)]
pub enum Msg {
    Sync(&'static str),
    Rescan,
    /// Ask "write the folder sync's changes?", or stop.
    ConfirmApply(bool),
    Apply,
    Pause,
    Clear,
    /// Open or close a health row (by its label).
    Health(String),
    /// Open or shut one of the sync report's lists.
    Fold(String),
    /// A title a health check flagged: open it in the Library section.
    /// Open this title in the Library section.
    Goto(String),
}

#[derive(Default)]
pub struct State {
    /// The status and how long it took to answer, in ms.
    status: Remote<(AdminStatus, u128)>,
    logs: Vec<LogLine>,
    log_after: u64,
    log_job: Option<Job<Result<Vec<LogLine>>>>,
    log_at: Option<Instant>,
    log_missing: bool,
    log_error: Option<String>,
    paused: bool,
    syncing: Option<(&'static str, Job<Result<SyncReport>>)>,
    report: Option<(&'static str, SyncReport)>,
    sync_error: Option<String>,
    sync_missing: bool,
    /// What the last folder scan found to do: (movies, series).
    work: Option<(bool, bool)>,
    clear_job: Option<Job<Result<()>>>,
    clear_missing: bool,
    confirm_apply: bool,
    /// The health row that's open.
    open: Option<String>,
    /// The report's lists the admin opened or shut, against their default.
    flipped: HashSet<String>,
}

impl State {
    pub fn refresh(&mut self) {
        self.status.retry();
        self.log_missing = false;
        self.log_error = None;
        self.sync_missing = false;
    }

    /// Only "not set up" when the status and the log are both missing.
    pub fn missing(&self) -> bool {
        self.status.missing && self.log_missing
    }

    pub fn busy(&self) -> bool {
        self.status.loading()
            || self.log_job.is_some()
            || self.syncing.is_some()
            || self.clear_job.is_some()
    }

    pub fn work(&mut self, client: &Client, asks: &mut Asks) {
        self.status.poll(&mut asks.login);
        if self.status.idle() || (self.status.data.is_some() && self.status.stale(30)) {
            let c = client.clone();
            self.status.load(move || {
                let t = Instant::now();
                let s = c.admin_status()?;
                Ok((s, t.elapsed().as_millis()))
            });
        }
        if let Some(r) = self.log_job.as_mut().and_then(Job::poll) {
            self.log_job = None;
            self.log_at = Some(Instant::now());
            match r {
                Ok(lines) => {
                    self.log_error = None;
                    if let Some(l) = lines.last() {
                        self.log_after = l.id;
                    }
                    self.logs.extend(lines);
                    let n = self.logs.len();
                    if n > LOG_KEEP {
                        self.logs.drain(..n - LOG_KEEP);
                    }
                }
                Err(e) if e.is::<crate::api::Unauthorized>() => asks.login = true,
                Err(e) if e.is::<crate::api::NoAdminApi>() => self.log_missing = true,
                Err(e) => self.log_error = Some(format!("{e:#}")),
            }
        }
        let due = self.log_at.is_none_or(|t| t.elapsed() >= LOG_EVERY);
        if !self.paused && !self.log_missing && self.log_job.is_none() && due {
            let c = client.clone();
            let after = self.log_after;
            self.log_job = Some(Job::spawn(move || c.admin_logs(after)));
        }
        if let Some(r) = self.clear_job.as_mut().and_then(Job::poll) {
            self.clear_job = None;
            match r {
                Ok(()) => {
                    // the server may number its lines from 1 again
                    self.logs.clear();
                    self.log_after = 0;
                    self.log_job = None;
                    self.log_at = None;
                }
                Err(e) if e.is::<crate::api::Unauthorized>() => asks.login = true,
                Err(e) if e.is::<crate::api::NoAdminApi>() => self.clear_missing = true,
                Err(e) => self.log_error = Some(format!("Couldn't clear the log: {e:#}")),
            }
        }
        if let Some(r) = self.syncing.as_mut().and_then(|(_, t)| t.poll()) {
            let (what, _) = self.syncing.take().expect("polled it");
            match r {
                Ok(rep) => {
                    // even a dry run: the server may have rescanned on its own
                    asks.reload_library = true;
                    self.sync_error = None;
                    if what == "folders" {
                        self.work = Some((rep.has_work(Kind::Movie), rep.has_work(Kind::Series)));
                    }
                    self.flipped.clear();
                    self.report = Some((what, rep));
                }
                Err(e) if e.is::<crate::api::Unauthorized>() => asks.login = true,
                Err(e) if e.is::<crate::api::NoAdminApi>() => self.sync_missing = true,
                Err(e) => self.sync_error = Some(format!("{e:#}")),
            }
            self.log_at = None; // show what it logged
        }
    }

    pub fn update(&mut self, m: Msg, client: &Client) -> Option<JobFn> {
        match m {
            Msg::Sync(what) => {
                if self.syncing.is_none() {
                    // until the next scan says there's more to do
                    match (what, self.work.as_mut()) {
                        ("movies", Some(w)) => w.0 = false,
                        ("series", Some(w)) => w.1 = false,
                        _ => {}
                    }
                    let c = client.clone();
                    self.sync_error = None;
                    self.syncing = Some((what, Job::spawn(move || c.admin_sync(what))));
                }
            }
            Msg::Apply => {
                self.confirm_apply = false;
                if self.syncing.is_none() {
                    let c = client.clone();
                    self.sync_error = None;
                    self.syncing = Some((
                        "folders",
                        Job::spawn(move || c.admin_sync("folders?apply=1")),
                    ));
                }
            }
            Msg::ConfirmApply(on) => self.confirm_apply = on,
            Msg::Rescan => {
                return Some(Box::new(|c: &Client| {
                    let r = c.admin_rescan()?;
                    let found = match (r.movies, r.series) {
                        (Some(m), Some(s)) => format!(": {m} movies, {s} series"),
                        _ => String::new(),
                    };
                    let msg = if r.running {
                        format!("Rescan started{found} so far. New titles show up once the server finishes; press Refresh then.")
                    } else {
                        format!("Library rescanned{found}.")
                    };
                    Ok(Done {
                        msg,
                        library: true,
                        ..Default::default()
                    })
                }));
            }
            Msg::Pause => self.paused = !self.paused,
            Msg::Clear => {
                self.logs.clear();
                if !self.clear_missing && self.clear_job.is_none() {
                    let c = client.clone();
                    self.clear_job = Some(Job::spawn(move || c.admin_clear_logs()));
                }
            }
            Msg::Health(label) => {
                self.open = if self.open.as_deref() == Some(label.as_str()) {
                    None
                } else {
                    Some(label)
                };
            }
            Msg::Fold(k) => {
                if !self.flipped.remove(&k) {
                    self.flipped.insert(k);
                }
            }
            // the admin page opens the title in Library
            Msg::Goto(_) => {}
        }
        None
    }

    pub fn view<'a>(&'a self, cx: &Cx<'a>, w: f32) -> Element<'a, LMsg> {
        let (m, inner) = column_of(w);
        let lib = cx.lib;
        // numbers: the server's own when it sends them, else counted here
        let srv = self
            .status
            .data
            .as_ref()
            .and_then(|(s, _)| s.library.clone())
            .unwrap_or_default();
        let num =
            |server: Option<u64>, here: usize| server.map_or(here.to_string(), |n| n.to_string());
        let episodes: usize = lib
            .series
            .iter()
            .flat_map(|s| &s.seasons)
            .map(|s| s.episodes.len())
            .sum();
        let size: u64 = lib
            .all()
            .flat_map(|i| {
                let eps = i.seasons.iter().flat_map(|s| &s.episodes);
                i.file
                    .iter()
                    .chain(eps.filter_map(|e| e.file.as_ref()))
                    .filter_map(|f| f.size)
            })
            .sum();
        let extra: Vec<(String, String)> = srv
            .extra
            .iter()
            .filter_map(|(k, v)| Some((v.as_f64()?, k)))
            .map(|(v, k)| (format!("{v}"), tile_label(k)))
            .collect();
        let mut tiles: Vec<(String, &str)> = vec![
            (num(srv.movies, cx.movies.len()), "Movies"),
            (num(srv.series, cx.series.len()), "Series"),
            (num(srv.episodes, episodes), "Episodes"),
            (
                match srv.size.unwrap_or(size) {
                    0 => "—".into(),
                    n => bytes(n),
                },
                "Library size",
            ),
            (
                srv.users
                    .map(|n| n.to_string())
                    .or(cx.users.map(|n| n.to_string()))
                    .unwrap_or_else(|| "—".into()),
                "Users",
            ),
        ];
        tiles.extend(extra.iter().map(|(v, l)| (v.clone(), l.as_str())));
        let tiles = stats(tiles, inner);

        let narrow = inner < NARROW;
        let g = 48.0;
        let lw = if narrow { inner } else { inner * 0.66 };
        let rw = if narrow { inner } else { inner - lw - g };
        let main = column![self.health(cx), gap(20.0), self.log()].width(lw);
        let side = column![self.api_status(), gap(20.0), self.syncs(cx)].width(rw);
        let body: Element<'a, LMsg> = if narrow {
            column![main, gap(24.0), side].into()
        } else {
            row![main, space().width(g), side].into()
        };
        container(column![tiles, gap(10.0), body])
            .padding(Padding::default().left(m))
            .into()
    }

    fn api_status<'a>(&'a self) -> Element<'a, LMsg> {
        let row_of = |ok: bool, name: &str, detail: &str| -> Element<'a, LMsg> {
            column![
                row![
                    status_dot(c(if ok { OK() } else { ERROR() })),
                    text(name.to_owned()).size(14.5),
                    space::horizontal(),
                    mono(detail, 10.5).color(style::MUTED()),
                ]
                .spacing(10)
                .align_y(Vertical::Center),
                space().height(12),
            ]
            .into()
        };
        let mut col: Column<'a, LMsg> = column![caps("API status", c(TEAL())), gap(10.0)];
        match &self.status.data {
            Some((s, ms)) => {
                let mut d = format!("online, {ms} ms");
                if let Some(v) = &s.version {
                    // "1.4.0" reads as v1.4.0; "laravel 13.8.0" stays as it is
                    let v_ = if v.starts_with(|ch: char| ch.is_ascii_digit()) {
                        "v"
                    } else {
                        ""
                    };
                    d.push_str(&format!(", {v_}{v}"));
                }
                col = col.push(row_of(true, "Valo API", &d));
                if let Some(up) = s.uptime {
                    col = col.push(row_of(true, "Uptime", &uptime(up)));
                }
                for sv in &s.services {
                    let detail = sv
                        .detail
                        .clone()
                        .unwrap_or_else(|| if sv.ok { "ok" } else { "down" }.into());
                    col = col.push(row_of(sv.ok, &sv.name, &detail));
                }
                if s.services.is_empty() {
                    col = col.push(needs_note(
                        "The server doesn't report its services (metadata source, Discord bot, database).",
                        Section::Dashboard,
                    ));
                }
            }
            None => {
                // the library loaded, so the API answers
                col = col.push(row_of(true, "Valo API", "online"));
                if self.status.missing {
                    col = col.push(needs_note(
                        "For response time, uptime and the state of the metadata \
                         source and Discord bot, add /api/admin/status.",
                        Section::Dashboard,
                    ));
                } else if let Some(e) = &self.status.error {
                    col = col.push(text(e.clone()).size(12).color(style::ERROR()));
                } else if self.status.loading() {
                    col = col.push(crate::iced_ui::icon::spinner(
                        spin_secs(),
                        style::MUTED(),
                        14.0,
                    ));
                }
            }
        }
        panel(col)
    }

    fn health<'a>(&'a self, cx: &Cx<'a>) -> Element<'a, LMsg> {
        let mut col: Column<'a, LMsg> = column![caps("Library health", c(TEAL())), gap(4.0)];
        let server_checks = self
            .status
            .data
            .as_ref()
            .map(|(s, _)| s.health.clone())
            .unwrap_or_default();
        if !server_checks.is_empty() {
            // the server's checks, the same ones its website shows
            let flagged: usize = server_checks.iter().map(|h| h.count).sum();
            let colr = c(if flagged == 0 { OK() } else { WARN() });
            col = col
                .push(
                    row![
                        text(flagged.to_string()).font(BOLD).size(24).color(colr),
                        text("issues found by the server")
                            .size(14.5)
                            .color(style::MUTED()),
                    ]
                    .spacing(10)
                    .align_y(Vertical::Center),
                )
                .push(gap(12.0));
            for h in server_checks {
                let mut list: Column<'a, LMsg> = column![];
                for t in h.titles.iter().take(60) {
                    let label = match (&t.title, t.year) {
                        (Some(n), Some(y)) => format!("{n} ({y})"),
                        (Some(n), None) => n.clone(),
                        _ => t.id.clone(),
                    };
                    list = list.push(text_link(
                        label,
                        LMsg::Admin(AMsg::Dash(Msg::Goto(t.key()))),
                    ));
                }
                for it in h.items.iter().take(60) {
                    list = list.push(mono(it, 10.5));
                }
                let shown = h.titles.len().min(60) + h.items.len().min(60);
                if h.count > shown && shown > 0 {
                    list = list.push(
                        text(format!("and {} more", h.count - shown))
                            .size(14.5)
                            .color(style::MUTED()),
                    );
                }
                col = col.push(self.health_row(&h.label, h.count, list));
            }
            return col.into();
        }
        // the app's own checks, over every title the server lists
        let all: Vec<&Item> = cx.movies.iter().chain(cx.series).collect();
        let checks = checks();
        let total = all.len();
        let flawed = all
            .iter()
            .filter(|i| checks.iter().any(|(_, f)| f(i)))
            .count();
        let score = (100 * (total - flawed)).checked_div(total).unwrap_or(100);
        let colr = c(match score {
            90.. => OK(),
            60.. => WARN(),
            _ => ERROR(),
        });
        col = col
            .push(
                row![
                    text(format!("{score}%")).font(BOLD).size(24).color(colr),
                    text(format!("of {total} titles have everything"))
                        .size(14.5)
                        .color(style::MUTED()),
                ]
                .spacing(10)
                .align_y(Vertical::Center),
            )
            .push(gap(12.0));
        for (label, f) in &checks {
            let hits: Vec<&Item> = all.iter().copied().filter(|i| f(i)).collect();
            let mut list: Column<'a, LMsg> = column![];
            for i in hits.iter().take(60) {
                let kind = if i.kind == Kind::Movie {
                    "movie"
                } else {
                    "series"
                };
                let year = i.year.map(|y| format!(" ({y})")).unwrap_or_default();
                list = list.push(
                    row![
                        text_link(
                            format!("{}{year}", i.title),
                            LMsg::Admin(AMsg::Dash(Msg::Goto(i.id.clone())))
                        ),
                        mono(kind, 9.5).color(style::MUTED()),
                    ]
                    .spacing(10)
                    .align_y(Vertical::Center),
                );
            }
            if hits.len() > 60 {
                list = list.push(
                    text(format!("and {} more", hits.len() - 60))
                        .size(14.5)
                        .color(style::MUTED()),
                );
            }
            col = col.push(self.health_row(label, hits.len(), list));
        }
        col.into()
    }

    /// A check: a dot, its label, how many it flagged; click to list them.
    fn health_row<'a>(&self, label: &str, n: usize, list: Column<'a, LMsg>) -> Element<'a, LMsg> {
        let open = self.open.as_deref() == Some(label);
        let warn = c(WARN());
        let mut head = row![
            status_dot(if n == 0 { c(OK()) } else { warn }),
            text(label.to_owned()).size(14.5)
        ]
        .spacing(10)
        .align_y(Vertical::Center);
        if n > 0 {
            head = head.push(crate::iced_ui::icon::icon(
                if open {
                    crate::iced_ui::icon::Icon::ChevronDown
                } else {
                    crate::iced_ui::icon::Icon::ChevronRight
                },
                style::MUTED(),
                12.0,
            ));
        }
        head = head
            .push(space::horizontal())
            .push(mono(&n.to_string(), 11.0).color(if n == 0 { style::MUTED() } else { warn }));
        let head: Element<'a, LMsg> = if n > 0 {
            button(head)
                .padding(0)
                .style(|_, _| button::Style {
                    text_color: style::TEXT(),
                    ..button::Style::default()
                })
                .on_press(LMsg::Admin(AMsg::Dash(Msg::Health(label.to_owned()))))
                .into()
        } else {
            head.into()
        };
        let mut col = column![head];
        if open && n > 0 {
            col = col.push(list.padding(Padding::default().left(22.0).top(6.0)));
        }
        col.push(space().height(14)).into()
    }

    fn syncs<'a>(&'a self, cx: &Cx<'a>) -> Element<'a, LMsg> {
        let send = |m: Msg| LMsg::Admin(AMsg::Dash(m));
        let mut col: Column<'a, LMsg> = column![caps("Sync operations", c(TEAL())), gap(10.0)];
        if self.sync_missing {
            col = col
                .push(needs_note(
                    "This server can't sync from the app yet (/api/admin/sync/…).",
                    Section::Dashboard,
                ))
                .push(gap(10.0));
        }
        let running = self.syncing.as_ref().map(|(w, _)| *w);
        for (what, title, about) in SYNCS {
            let control: Element<'a, LMsg> = if running == Some(what) {
                crate::iced_ui::icon::spinner(spin_secs(), style::MUTED(), 16.0).into()
            } else {
                let label = if what == "folders" { "Compare" } else { "Run" };
                // movies and series only after a scan that found them work
                let (ready, why) = match (what, self.work) {
                    ("folders", _) => (true, ""),
                    (_, None) => (false, "Run Sync folders first"),
                    ("movies", Some((m, _))) => (m, NOTHING_NEW),
                    (_, Some((_, s))) => (s, NOTHING_NEW),
                };
                let free = running.is_none() && !self.sync_missing;
                let b = widgets::button_with(
                    Btn::Small,
                    label,
                    None,
                    (free && ready).then(|| send(Msg::Sync(what))),
                );
                if free && !ready {
                    crate::iced_ui::chrome::tip_below(b, why)
                } else {
                    b
                }
            };
            col = col
                .push(
                    row![
                        text(title).font(BOLD).size(15),
                        space::horizontal(),
                        control
                    ]
                    .align_y(Vertical::Center),
                )
                .push(space().height(4))
                .push(text(about).size(12).color(style::MUTED()))
                .push(gap(12.0));
        }
        // the original admin API's one button, which every Valo admin server has
        col = col
            .push(
                row![
                    text("Rescan library").font(BOLD).size(15),
                    space::horizontal(),
                    widgets::button_with(
                        Btn::Small,
                        "Run",
                        None,
                        (!cx.busy).then(|| send(Msg::Rescan))
                    ),
                ]
                .align_y(Vertical::Center),
            )
            .push(space().height(4))
            .push(
                text("Reads the media folders again and reloads the library here.")
                    .size(12)
                    .color(style::MUTED()),
            )
            .push(gap(12.0));
        if let Some(e) = &self.sync_error {
            col = col.push(text(e.clone()).size(14.5).color(style::ERROR()));
        }
        if let Some((what, rep)) = &self.report {
            col = col.push(super::super::details::hline()).push(gap(6.0));
            let title = SYNCS.iter().find(|s| s.0 == *what).map_or("", |s| s.1);
            let verb = if rep.applied { "done" } else { "dry run" };
            col = col.push(caps(&format!("{title}: {verb}"), style::MUTED()));
            if let Some(m) = &rep.message {
                col = col.push(text(m.clone()).size(14.5));
            }
            if rep.running {
                col = col.push(
                    text("Still running on the server; follow the live log.")
                        .size(14.5)
                        .color(style::MUTED()),
                );
            }
            for (label, list, colr) in [
                ("New", &rep.added, c(OK())),
                ("Removed", &rep.removed, c(ERROR())),
                ("Updated", &rep.updated, style::TEXT()),
            ] {
                if list.is_empty() {
                    continue;
                }
                col = col.push(self.report_list(
                    &format!("{label}: {}", list.len()),
                    list,
                    colr,
                    None,
                ));
            }
            if !rep.unnamed.is_empty() {
                col = col.push(self.report_list(
                    &format!("Not renamed: {}", rep.unnamed.len()),
                    &rep.unnamed,
                    c(WARN()),
                    Some(
                        "No SxxEyy in the name: numbered from the season folder \
                         where possible. Rename them to be sure.",
                    ),
                ));
            }
            if !rep.new.is_empty() {
                col = col.push(self.report_list(
                    &format!("New on disk: {}", rep.new.len()),
                    &rep.new.lines(),
                    c(OK()),
                    Some("Not in the library yet: Sync movies or Sync series adds them."),
                ));
            }
            if !rep.to_rename.is_empty() {
                col = col.push(self.report_list(
                    &format!("To rename: {}", rep.to_rename.len()),
                    &rep.to_rename,
                    c(WARN()),
                    Some("Names that don't follow the library's naming, with the server's suggestion after the arrow."),
                ));
            }
            if !rep.unedited_list.is_empty() {
                col = col.push(self.report_list(
                    &format!("Not edited: {}", rep.unedited_list.len()),
                    &rep.unedited_list,
                    c(WARN()),
                    Some("No details fetched yet: the next movie or series sync fills them in."),
                ));
            }
            let missing = &rep.missing_images;
            if !missing.is_empty() {
                let key = format!("Missing images: {}", missing.len());
                let open = (missing.len() <= 10) != self.flipped.contains(&key);
                col = col.push(fold(key.clone(), open, send(Msg::Fold(key))));
                if open {
                    let mut inner: Column<'a, LMsg> =
                        column![].padding(Padding::default().left(20.0));
                    for (label, list) in [
                        ("Movies", &missing.movies),
                        ("Series", &missing.series),
                        ("Seasons", &missing.seasons),
                        ("Episodes", &missing.episodes),
                    ] {
                        if !list.is_empty() {
                            inner = inner.push(self.report_list(
                                &format!("{label}: {}", list.len()),
                                list,
                                c(WARN()),
                                None,
                            ));
                        }
                    }
                    inner = inner.push(
                        text("Sync movies or Sync series downloads these again.")
                            .size(12)
                            .color(style::MUTED()),
                    );
                    col = col.push(inner);
                }
            }
            // a dry run with changes: offer to write them
            let changes = rep.added.len() + rep.removed.len();
            if *what == "folders" && !rep.applied && changes > 0 {
                col = col.push(gap(8.0));
                if self.confirm_apply {
                    col = col
                        .push(
                            text(format!(
                                "Write these {changes} changes to the database? Removed files \
                                 lose their entries, watch progress included."
                            ))
                            .size(14.5),
                        )
                        .push(
                            row![
                                danger_button(
                                    "Apply changes",
                                    self.syncing.is_none().then(|| send(Msg::Apply))
                                ),
                                widgets::button_with(
                                    Btn::Small,
                                    "Cancel",
                                    None,
                                    Some(send(Msg::ConfirmApply(false)))
                                ),
                            ]
                            .spacing(10),
                        );
                } else {
                    col = col.push(widgets::button_with(
                        Btn::Small,
                        "Apply these changes…",
                        None,
                        Some(send(Msg::ConfirmApply(true))),
                    ));
                }
            }
            if rep.added.is_empty()
                && rep.removed.is_empty()
                && rep.updated.is_empty()
                && missing.is_empty()
                && rep.new.is_empty()
                && !rep.running
            {
                col = col.push(
                    text("Nothing changed: the database matches the folders.")
                        .size(14.5)
                        .color(style::MUTED()),
                );
            }
        }
        panel(col)
    }

    /// One of the sync report's lists, open by default when it's short.
    fn report_list<'a>(
        &self,
        key: &str,
        list: &[String],
        colr: Color,
        note: Option<&'static str>,
    ) -> Element<'a, LMsg> {
        let open = (list.len() <= 10) != self.flipped.contains(key);
        let mut col = column![fold(
            key.to_owned(),
            open,
            LMsg::Admin(AMsg::Dash(Msg::Fold(key.to_owned())))
        )];
        if open {
            let mut items: Column<'a, LMsg> = column![].padding(Padding::default().left(20.0));
            for f in list.iter().take(200) {
                items = items.push(mono(f, 10.0).color(colr));
            }
            if list.len() > 200 {
                items = items.push(
                    text(format!("and {} more", list.len() - 200))
                        .size(14.5)
                        .color(style::MUTED()),
                );
            }
            if let Some(n) = note {
                items = items.push(text(n).size(12).color(style::MUTED()));
            }
            col = col.push(items);
        }
        col.into()
    }

    fn log<'a>(&'a self) -> Element<'a, LMsg> {
        let send = |m: Msg| LMsg::Admin(AMsg::Dash(m));
        let mut head = row![caps("Live log", c(TEAL()))]
            .spacing(10)
            .align_y(Vertical::Center);
        if !self.paused && !self.log_missing {
            head = head.push(status_dot(c(OK())));
        }
        head = head.push(space::horizontal());
        if !self.log_missing {
            let tip = if self.clear_missing {
                "Clears it here only: this server can't empty its log (DELETE /api/admin/logs)"
            } else {
                "Empties the server's log, for every admin"
            };
            head = head
                .push(crate::iced_ui::chrome::tip_below(
                    widgets::button_with(
                        Btn::Small,
                        "Clear",
                        None,
                        self.clear_job.is_none().then(|| send(Msg::Clear)),
                    ),
                    tip,
                ))
                .push(widgets::button_with(
                    Btn::Small,
                    if self.paused { "Resume" } else { "Pause" },
                    None,
                    Some(send(Msg::Pause)),
                ));
        }
        let mut col: Column<'a, LMsg> = column![
            head,
            space().height(4),
            text(
                "The server's log as it happens: scans, syncs, requests and errors. \
                 Updates every few seconds.",
            )
            .size(12)
            .color(style::MUTED()),
            gap(8.0),
        ];
        if self.log_missing {
            return col
                .push(needs_note(
                    "This server doesn't share its log yet (/api/admin/logs).",
                    Section::Dashboard,
                ))
                .into();
        }
        if let Some(e) = &self.log_error {
            col = col.push(text(e.clone()).size(12).color(style::ERROR()));
        }
        let now = now_unix();
        let mut lines: Column<'a, LMsg> = column![].spacing(2);
        if self.logs.is_empty() {
            lines = lines.push(mono("Waiting for log lines…", 10.5).color(style::MUTED()));
        }
        for l in &self.logs {
            let colr = match l.level.to_lowercase().as_str() {
                "error" | "critical" => c(ERROR()),
                "warning" | "warn" => c(WARN()),
                "debug" => style::MUTED(),
                _ => style::TEXT(),
            };
            // no time: the server puts its own in the message
            let when = if l.time > 0 {
                format!("{:>4}", short_ago(now - l.time))
            } else {
                String::new()
            };
            lines = lines.push(
                row![
                    text(when)
                        .font(MONO)
                        .size(11)
                        .color(style::MUTED())
                        .width(34),
                    text(l.message.clone()).font(MONO).size(11).color(colr),
                ]
                .spacing(10),
            );
        }
        col.push(
            container(
                scrollable(lines)
                    .id(LOG)
                    .anchor_bottom()
                    .height(320)
                    .width(Length::Fill),
            )
            .padding(12)
            .style(|_| container::Style {
                background: Some(Background::Color(c(FIELD()))),
                border: Border {
                    color: c(LINE()),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..container::Style::default()
            }),
        )
        .into()
    }
}

/// Text that acts as a link to a title.
fn text_link<'a>(t: String, msg: LMsg) -> Element<'a, LMsg> {
    button(text(t).size(14.5))
        .padding(0)
        .style(|_, s| button::Style {
            text_color: if matches!(s, button::Status::Hovered) {
                style::TEXT()
            } else {
                c(crate::ui::theme::ACCENT())
            },
            ..button::Style::default()
        })
        .on_press(msg)
        .into()
}

type Check = (&'static str, fn(&Item) -> bool);

/// The app's own checks: what a title is missing.
fn checks() -> [Check; 5] {
    [
        ("Missing poster", |i| i.poster.is_none()),
        ("Missing backdrop", |i| i.backdrop.is_none()),
        ("Missing overview", |i| i.overview.trim().is_empty()),
        ("Missing year", |i| i.year.is_none()),
        ("Missing genres", |i| i.genres.is_empty()),
    ]
}

/// "mp4" -> "MP4", "series_complete" -> "Series complete".
fn tile_label(key: &str) -> String {
    if key.len() <= 4 && !key.contains('_') {
        return key.to_uppercase();
    }
    let words = key.replace('_', " ");
    let mut ch = words.chars();
    ch.next()
        .map_or(String::new(), |f| f.to_uppercase().chain(ch).collect())
}

/// "3d 4h", "5h 12m", "8m"
fn uptime(s: u64) -> String {
    let (d, h, m) = (s / 86_400, s % 86_400 / 3600, s % 3600 / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

/// "now", "45s", "3m", "2h", "4d"
fn short_ago(s: i64) -> String {
    match s.max(0) {
        s if s < 5 => "now".into(),
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m", s / 60),
        s if s < 86_400 => format!("{}h", s / 3600),
        s => format!("{}d", s / 86_400),
    }
}
