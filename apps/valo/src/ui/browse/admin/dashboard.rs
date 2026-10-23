//! Admin › Dashboard: library numbers, API status, library health, syncs
//! and the server's live log.
use std::time::{Duration, Instant};

use anyhow::Result;
use eframe::egui::{self, RichText, ScrollArea, Ui};

use super::super::details::{bytes, caps, padded, panel, TEAL};
use super::super::people::{columns_with, stats};
use super::{needs_note, status_dot, Cx, Remote, Section};
use crate::api::{AdminStatus, Item, Kind, LogLine, SyncReport};
use crate::ui::{bold, mono, small_button, ERROR, MUTED, OK, TEXT, WARN};
use crate::util::{now_unix, Task};

/// How often the live log asks for new lines.
const LOG_EVERY: Duration = Duration::from_secs(3);
const LOG_KEEP: usize = 500;

const SYNCS: [(&str, &str, &str); 3] = [
    (
        "folders",
        "Sync folders",
        "Scans /media directories and compares against the database. Lists new and \
         removed files — no changes written.",
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

#[derive(Default)]
pub(in super::super) struct State {
    /// The status and how long it took to answer, in ms.
    status: Remote<(AdminStatus, u128)>,
    logs: Vec<LogLine>,
    /// The newest line's id: the next request asks for what came after it.
    log_after: u64,
    log_task: Option<Task<Result<Vec<LogLine>>>>,
    log_at: Option<Instant>,
    log_missing: bool,
    log_error: Option<String>,
    paused: bool,
    /// Which sync is running.
    syncing: Option<(&'static str, Task<Result<SyncReport>>)>,
    /// The last sync's name and report.
    report: Option<(&'static str, SyncReport)>,
    sync_error: Option<String>,
    sync_missing: bool,
    /// Asking "write the folder sync's changes?".
    confirm_apply: bool,
    /// The health row that's open.
    open: Option<String>,
}

impl State {
    pub fn refresh(&mut self) {
        self.status.retry();
        self.log_missing = false;
        self.log_error = None;
        self.sync_missing = false;
    }

    /// Nothing here works without the server's help, except the numbers
    /// and health checks, so it's only "not set up" when all of it is missing.
    pub fn missing(&self) -> bool {
        self.status.missing && self.log_missing
    }

    fn poll(&mut self, cx: &mut Cx) {
        let ctx = cx.ctx.clone();
        self.status.poll(&mut cx.login);
        if self.status.idle() || (self.status.data.is_some() && self.status.stale(30)) {
            let c = cx.client.clone();
            self.status.load(&ctx, move || {
                let t = Instant::now();
                let s = c.admin_status()?;
                Ok((s, t.elapsed().as_millis()))
            });
        }

        if let Some(r) = self.log_task.as_mut().and_then(|t| t.poll()) {
            self.log_task = None;
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
                Err(e) if e.is::<crate::api::Unauthorized>() => cx.login = true,
                Err(e) if e.is::<crate::api::NoAdminApi>() => self.log_missing = true,
                Err(e) => self.log_error = Some(format!("{e:#}")),
            }
        }
        let due = self.log_at.is_none_or(|t| t.elapsed() >= LOG_EVERY);
        if !self.paused && !self.log_missing && self.log_task.is_none() && due {
            let c = cx.client.clone();
            let after = self.log_after;
            self.log_task = Some(Task::spawn(&ctx, move || c.admin_logs(after)));
        }
        if !self.paused && !self.log_missing {
            ctx.request_repaint_after(LOG_EVERY);
        }

        if let Some(r) = self.syncing.as_mut().and_then(|(_, t)| t.poll()) {
            let (what, _) = self.syncing.take().unwrap();
            match r {
                Ok(rep) => {
                    // even a dry run: the server may have rescanned on its own,
                    // so titles it reports removed shouldn't linger here
                    cx.reload_library = true;
                    self.sync_error = None;
                    self.report = Some((what, rep));
                }
                Err(e) if e.is::<crate::api::Unauthorized>() => cx.login = true,
                Err(e) if e.is::<crate::api::NoAdminApi>() => self.sync_missing = true,
                Err(e) => self.sync_error = Some(format!("{e:#}")),
            }
            self.log_at = None; // show what it logged
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        self.poll(cx);
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
        // any other numbers the server counts, as more tiles
        let extra: Vec<(String, String)> = srv
            .extra
            .iter()
            .filter_map(|(k, v)| Some((v.as_f64()?, k)))
            .map(|(v, k)| (format!("{v}"), tile_label(k)))
            .collect();
        padded(ui, |ui| {
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
            stats(ui, &tiles);
        });
        ui.add_space(10.0);

        columns_with(
            ui,
            &mut (self, cx),
            |ui, (st, cx)| {
                st.health(ui, cx);
                ui.add_space(20.0);
                st.log(ui);
            },
            |ui, (st, cx)| {
                st.api_status(ui);
                ui.add_space(20.0);
                st.syncs(ui, cx);
            },
        );
    }

    fn api_status(&mut self, ui: &mut Ui) {
        panel(ui, |ui| {
            caps(ui, "API status", TEAL());
            ui.add_space(10.0);
            let row = |ui: &mut Ui, ok: bool, name: &str, detail: &str| {
                ui.horizontal(|ui| {
                    status_dot(ui, if ok { OK() } else { ERROR() });
                    ui.label(RichText::new(name).color(TEXT()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(mono(detail, 10.5).color(MUTED())).truncate());
                    });
                });
                ui.add_space(4.0);
            };
            match &self.status.data {
                Some((s, ms)) => {
                    let mut d = format!("online, {ms} ms");
                    if let Some(v) = &s.version {
                        // "1.4.0" reads as v1.4.0; "laravel 13.8.0" stays as it is
                        let v_ = if v.starts_with(|c: char| c.is_ascii_digit()) {
                            "v"
                        } else {
                            ""
                        };
                        d.push_str(&format!(", {v_}{v}"));
                    }
                    row(ui, true, "Valo API", &d);
                    if let Some(up) = s.uptime {
                        row(ui, true, "Uptime", &uptime(up));
                    }
                    for sv in &s.services {
                        let detail = sv
                            .detail
                            .clone()
                            .unwrap_or_else(|| if sv.ok { "ok" } else { "down" }.into());
                        row(ui, sv.ok, &sv.name, &detail);
                    }
                    if s.services.is_empty() {
                        needs_note(
                            ui,
                            "The server doesn't report its services (metadata source, \
                             Discord bot, database).",
                            Section::Dashboard,
                        );
                    }
                }
                None => {
                    // the library loaded, so the API answers
                    row(ui, true, "Valo API", "online");
                    if self.status.missing {
                        needs_note(
                            ui,
                            "For response time, uptime and the state of the metadata \
                             source and Discord bot, add /api/admin/status.",
                            Section::Dashboard,
                        );
                    } else if let Some(e) = &self.status.error {
                        ui.label(RichText::new(e).small().color(ERROR()));
                    } else if self.status.loading() {
                        ui.add(egui::Spinner::new().size(14.0).color(MUTED()));
                    }
                }
            }
        });
    }

    fn health(&mut self, ui: &mut Ui, cx: &mut Cx) {
        caps(ui, "Library health", TEAL());
        ui.add_space(4.0);
        let server_checks = self
            .status
            .data
            .as_ref()
            .map(|(s, _)| s.health.clone())
            .unwrap_or_default();
        if !server_checks.is_empty() {
            // the server's checks, the same ones its website shows
            let flagged: usize = server_checks.iter().map(|h| h.count).sum();
            ui.horizontal(|ui| {
                let col = if flagged == 0 { OK() } else { WARN() };
                ui.label(bold(flagged.to_string(), 24.0).color(col));
                ui.label(RichText::new("issues found by the server").color(MUTED()));
            });
            ui.add_space(12.0);
            for h in &server_checks {
                self.health_row(ui, cx, &h.label, h.count, |ui, cx| {
                    for t in h.titles.iter().take(60) {
                        let label = match (&t.title, t.year) {
                            (Some(n), Some(y)) => format!("{n} ({y})"),
                            (Some(n), None) => n.clone(),
                            _ => t.id.clone(),
                        };
                        if ui.link(label).clicked() {
                            cx.goto = Some((Section::Library, Some(t.key())));
                        }
                    }
                    for it in h.items.iter().take(60) {
                        ui.label(mono(it, 10.5).color(TEXT()));
                    }
                    let shown = h.titles.len().min(60) + h.items.len().min(60);
                    if h.count > shown && shown > 0 {
                        ui.label(
                            RichText::new(format!("and {} more", h.count - shown)).color(MUTED()),
                        );
                    }
                });
            }
            return;
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
        ui.horizontal(|ui| {
            let col = match score {
                90.. => OK(),
                60.. => WARN(),
                _ => ERROR(),
            };
            ui.label(bold(format!("{score}%"), 24.0).color(col));
            ui.label(RichText::new(format!("of {total} titles have everything")).color(MUTED()));
        });
        ui.add_space(12.0);
        for (label, f) in &checks {
            let hits: Vec<&Item> = all.iter().copied().filter(|i| f(i)).collect();
            self.health_row(ui, cx, label, hits.len(), |ui, cx| {
                for i in hits.iter().take(60) {
                    let kind = if i.kind == Kind::Movie {
                        "movie"
                    } else {
                        "series"
                    };
                    let year = i.year.map(|y| format!(" ({y})")).unwrap_or_default();
                    ui.horizontal(|ui| {
                        if ui.link(format!("{}{year}", i.title)).clicked() {
                            cx.goto = Some((Section::Library, Some(i.id.clone())));
                        }
                        ui.label(mono(kind, 9.5).color(MUTED()));
                    });
                }
                if hits.len() > 60 {
                    ui.label(RichText::new(format!("and {} more", hits.len() - 60)).color(MUTED()));
                }
            });
        }
    }

    fn health_row(
        &mut self,
        ui: &mut Ui,
        cx: &mut Cx,
        label: &str,
        n: usize,
        list: impl FnOnce(&mut Ui, &mut Cx),
    ) {
        let open = self.open.as_deref() == Some(label);
        let resp = ui
            .horizontal(|ui| {
                status_dot(ui, if n == 0 { OK() } else { WARN() });
                let r = ui.add(
                    egui::Label::new(RichText::new(label).color(TEXT()))
                        .sense(egui::Sense::click()),
                );
                if n > 0 {
                    let (cr, _) =
                        ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    let icon = if open {
                        crate::ui::chrome::Icon::ChevronDown
                    } else {
                        crate::ui::chrome::Icon::ChevronRight
                    };
                    crate::ui::chrome::draw_icon(ui.painter(), cr.center(), icon, MUTED());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(mono(n.to_string(), 11.0).color(if n == 0 {
                        MUTED()
                    } else {
                        WARN()
                    }));
                });
                r
            })
            .inner;
        if n > 0
            && resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
        {
            self.open = if open { None } else { Some(label.to_owned()) };
        }
        if open && n > 0 {
            ui.indent(label, |ui| list(ui, cx));
        }
        ui.add_space(6.0);
    }

    fn syncs(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let mut apply = false;
        panel(ui, |ui| {
            caps(ui, "Sync operations", TEAL());
            ui.add_space(10.0);
            if self.sync_missing {
                needs_note(
                    ui,
                    "This server can't sync from the app yet (/api/admin/sync/…).",
                    Section::Dashboard,
                );
                ui.add_space(10.0);
            }
            let running = self.syncing.as_ref().map(|(w, _)| *w);
            for (what, title, about) in SYNCS {
                ui.horizontal(|ui| {
                    ui.label(bold(title, 15.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if running == Some(what) {
                            ui.add(egui::Spinner::new().size(16.0).color(MUTED()));
                        } else {
                            let label = if what == "folders" { "Compare" } else { "Run" };
                            let b = ui
                                .add_enabled_ui(running.is_none() && !self.sync_missing, |ui| {
                                    small_button(ui, label)
                                })
                                .inner;
                            if b.clicked() {
                                let c = cx.client.clone();
                                self.sync_error = None;
                                self.syncing = Some((
                                    what,
                                    Task::spawn(&cx.ctx.clone(), move || c.admin_sync(what)),
                                ));
                            }
                        }
                    });
                });
                ui.label(RichText::new(about).small().color(MUTED()));
                ui.add_space(12.0);
            }
            // the original admin API's one button, which every Valo admin server has
            ui.horizontal(|ui| {
                ui.label(bold("Rescan library", 15.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled_ui(!cx.busy, |ui| small_button(ui, "Run")).inner.clicked() {
                        cx.run(|c| {
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
                            Ok(super::Done { msg, library: true, ..Default::default() })
                        });
                    }
                });
            });
            ui.label(
                RichText::new("Reads the media folders again and reloads the library here.")
                    .small()
                    .color(MUTED()),
            );
            ui.add_space(12.0);
            if let Some(e) = &self.sync_error {
                ui.label(RichText::new(e).color(ERROR()));
            }
            if let Some((what, rep)) = &self.report {
                ui.separator();
                ui.add_space(6.0);
                let title = SYNCS.iter().find(|s| s.0 == *what).map_or("", |s| s.1);
                let verb = if rep.applied { "done" } else { "dry run" };
                caps(ui, &format!("{title}: {verb}"), MUTED());
                if let Some(m) = &rep.message {
                    ui.label(RichText::new(m).color(TEXT()));
                }
                if rep.running {
                    ui.label(
                        RichText::new("Still running on the server; follow the live log.")
                            .color(MUTED()),
                    );
                }
                for (label, list, col) in [
                    ("New", &rep.added, OK()),
                    ("Removed", &rep.removed, ERROR()),
                    ("Updated", &rep.updated, TEXT()),
                ] {
                    if list.is_empty() {
                        continue;
                    }
                    egui::CollapsingHeader::new(format!("{label}: {}", list.len()))
                        .id_salt(("sync", label))
                        .default_open(list.len() <= 10)
                        .show(ui, |ui| {
                            for f in list.iter().take(200) {
                                ui.label(mono(f, 10.0).color(col));
                            }
                        });
                }
                // a dry run with changes: offer to write them
                let changes = rep.added.len() + rep.removed.len();
                if *what == "folders" && !rep.applied && changes > 0 {
                    ui.add_space(8.0);
                    if self.confirm_apply {
                        ui.label(
                            RichText::new(format!(
                                "Write these {changes} changes to the database? Removed files \
                                 lose their entries, watch progress included."
                            ))
                            .color(TEXT()),
                        );
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled_ui(self.syncing.is_none(), |ui| {
                                    super::users::danger_button(ui, "Apply changes")
                                })
                                .inner
                                .clicked()
                            {
                                apply = true;
                            }
                            if small_button(ui, "Cancel").clicked() {
                                self.confirm_apply = false;
                            }
                        });
                    } else if small_button(ui, "Apply these changes…").clicked() {
                        self.confirm_apply = true;
                    }
                }
                if rep.added.is_empty()
                    && rep.removed.is_empty()
                    && rep.updated.is_empty()
                    && !rep.running
                {
                    ui.label(
                        RichText::new("Nothing changed: the database matches the folders.")
                            .color(MUTED()),
                    );
                }
            }
        });
        if apply {
            self.confirm_apply = false;
            let c = cx.client.clone();
            self.sync_error = None;
            self.syncing = Some((
                "folders",
                Task::spawn(&cx.ctx, move || c.admin_sync("folders?apply=1")),
            ));
        }
    }

    fn log(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            caps(ui, "Live log", TEAL());
            if !self.paused && !self.log_missing {
                status_dot(ui, OK());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !self.log_missing {
                    if small_button(ui, if self.paused { "Resume" } else { "Pause" }).clicked() {
                        self.paused = !self.paused;
                    }
                    if small_button(ui, "Clear").clicked() {
                        self.logs.clear();
                    }
                }
            });
        });
        ui.label(
            RichText::new(
                "The server's log as it happens: scans, syncs, requests and errors. \
                 Updates every few seconds.",
            )
            .small()
            .color(MUTED()),
        );
        ui.add_space(8.0);
        if self.log_missing {
            needs_note(
                ui,
                "This server doesn't share its log yet (/api/admin/logs).",
                Section::Dashboard,
            );
            return;
        }
        if let Some(e) = &self.log_error {
            ui.label(RichText::new(e).small().color(ERROR()));
        }
        egui::Frame::new()
            .fill(crate::ui::FIELD())
            .stroke(egui::Stroke::new(1.0f32, crate::ui::LINE()))
            .corner_radius(8)
            .inner_margin(12)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ScrollArea::vertical()
                    .id_salt("admin-log")
                    .max_height(320.0)
                    .min_scrolled_height(320.0)
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.logs.is_empty() {
                            ui.label(mono("Waiting for log lines…", 10.5).color(MUTED()));
                        }
                        let now = now_unix();
                        for l in &self.logs {
                            let col = match l.level.to_lowercase().as_str() {
                                "error" | "critical" => ERROR(),
                                "warning" | "warn" => WARN(),
                                "debug" => MUTED(),
                                _ => TEXT(),
                            };
                            ui.horizontal_top(|ui| {
                                ui.label(
                                    // no time: the server puts its own in the message
                                    RichText::new(if l.time > 0 {
                                        format!("{:>4}", short_ago(now - l.time))
                                    } else {
                                        String::new()
                                    })
                                    .monospace()
                                    .size(11.0)
                                    .color(MUTED()),
                                );
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&l.message).monospace().size(11.0).color(col),
                                    )
                                    .wrap(),
                                );
                            });
                        }
                    });
            });
    }
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
    let mut c = words.chars();
    c.next()
        .map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
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
