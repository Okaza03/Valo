//! Shown after the user enters a new server's address: checks every API the
//! app uses, one by one, and how complete the metadata is. Errors block the
//! server; warnings can be accepted. Sign-in happens halfway through.
use std::sync::mpsc::{Receiver, TryRecvError};

use eframe::egui::{self, pos2, vec2, Align2, FontId, RichText, Sense, Stroke, StrokeKind, Ui};

use super::{
    card, primary_button, small_button, spinner_text, Action, ERROR, LINE, MUTED, OK, TEXT, WARN,
};
use crate::api::check::{self, Event, Group, Job, Level};
use crate::api::{normalize, Client, API_DOCS};

struct Row {
    title: &'static str,
    /// None while running.
    level: Option<Level>,
    detail: String,
}

pub struct Check {
    address: String,
    rows: Vec<Row>,
    groups: Vec<Group>,
    rx: Option<Receiver<Event>>,
    /// Set when the check finished without errors.
    client: Option<Client>,
    user: Option<String>,
    /// Rows from here on ran after signing in.
    signed_in_at: Option<usize>,
}

/// Redraw when the check has news.
fn repaint(ctx: &egui::Context) -> impl Fn() + Send + 'static {
    let ctx = ctx.clone();
    move || ctx.request_repaint()
}

fn color(l: Level) -> egui::Color32 {
    match l {
        Level::Pass => OK(),
        Level::Later => MUTED(),
        Level::Warn => WARN(),
        Level::Fail => ERROR(),
    }
}

fn tag(l: Level) -> &'static str {
    match l {
        Level::Pass => "PASS",
        Level::Later => "LATER",
        Level::Warn => "WARN",
        Level::Fail => "FAIL",
    }
}

fn badge(ui: &mut Ui, level: Option<Level>) {
    let (r, _) = ui.allocate_exact_size(vec2(52.0, 20.0), Sense::hover());
    let Some(l) = level else {
        let s = 14.0;
        let at = egui::Rect::from_center_size(r.center(), vec2(s, s));
        ui.put(at, egui::Spinner::new().size(s).color(TEXT()));
        return;
    };
    let c = color(l);
    let p = ui.painter();
    p.rect_stroke(r, 2.0, Stroke::new(1.0f32, c), StrokeKind::Inside);
    p.text(
        r.center(),
        Align2::CENTER_CENTER,
        tag(l),
        FontId::monospace(10.5),
        c,
    );
}

/// "Poster ▓▓▓▓░░ 48/52"
fn bar(ui: &mut Ui, f: &check::Field) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 20.0), Sense::hover());
    let p = ui.painter();
    let c = color(f.level());
    p.text(
        pos2(r.left(), r.center().y),
        Align2::LEFT_CENTER,
        f.name,
        FontId::proportional(13.0),
        TEXT(),
    );
    p.text(
        pos2(r.right(), r.center().y),
        Align2::RIGHT_CENTER,
        format!("{}/{}", f.have, f.total),
        FontId::monospace(11.5),
        if f.have == f.total { MUTED() } else { c },
    );
    let track = egui::Rect::from_min_max(
        pos2(r.left() + 110.0, r.center().y - 3.0),
        pos2(r.right() - 72.0, r.center().y + 3.0),
    );
    if track.width() > 10.0 {
        p.rect_filled(track, 1.0, LINE());
        let frac = f.have as f32 / f.total.max(1) as f32;
        let mut fill = track;
        fill.set_width(track.width() * frac);
        p.rect_filled(fill, 1.0, c);
    }
}

impl Check {
    pub fn new(ctx: &egui::Context, address: String) -> Self {
        let rx = check::start(repaint(ctx), Job::Address(address.clone()));
        Self {
            address,
            rows: Vec::new(),
            groups: Vec::new(),
            rx: Some(rx),
            client: None,
            user: None,
            signed_in_at: None,
        }
    }

    /// Run the rest of the checks with the signed-in client.
    pub fn signed_in(&mut self, ctx: &egui::Context, client: Client, user: Option<String>) {
        self.user = user;
        self.client = None;
        self.signed_in_at = Some(self.rows.len());
        self.rx = Some(check::start(repaint(ctx), Job::SignedIn(client)));
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    fn count(&self, l: Level) -> usize {
        self.rows.iter().filter(|r| r.level == Some(l)).count()
    }

    fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        loop {
            match rx.try_recv() {
                Ok(Event::Begin(title)) => self.rows.push(Row {
                    title,
                    level: None,
                    detail: String::new(),
                }),
                Ok(Event::End(level, detail)) => {
                    if let Some(r) = self.rows.last_mut() {
                        r.level = Some(level);
                        r.detail = detail;
                    }
                }
                Ok(Event::Metadata(g)) => self.groups = g,
                Ok(Event::Done(c)) => {
                    self.client = c;
                    self.rx = None;
                    return;
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.rx = None;
                    return;
                }
            }
        }
    }

    /// Plain-text results for the server's admin.
    fn report(&self) -> String {
        let mut s = format!(
            "Valo {} server check for {}\n\n",
            env!("CARGO_PKG_VERSION"),
            normalize(&self.address)
        );
        for r in &self.rows {
            let t = r.level.map_or("....", tag);
            s += &format!("[{t:<5}] {}: {}\n", r.title, r.detail);
        }
        if !self.groups.is_empty() {
            s += "\nMetadata (titles that have each field):\n";
            for g in &self.groups {
                s += &format!("  {} ({} {})\n", g.title, g.total, g.noun);
                for f in g.fields.iter().filter(|f| f.have < f.total) {
                    s += &format!("    {:<14} {}/{}\n", f.name, f.have, f.total);
                }
            }
        }
        s += &format!("\nHow to add what's missing: {API_DOCS}\n");
        s
    }

    pub fn ui(&mut self, ctx: &egui::Context) -> Action {
        self.poll();
        let running = self.rx.is_some();
        let (fails, warns) = (self.count(Level::Fail), self.count(Level::Warn));
        card(ctx, |ui| {
            let mut action = Action::None;
            ui.label(super::bold("Checking the server", 20.0));
            ui.label(RichText::new(normalize(&self.address)).color(MUTED()));
            ui.add_space(12.0);

            for (i, r) in self.rows.iter().enumerate() {
                if self.signed_in_at == Some(i) {
                    super::section(ui, "After signing in");
                }
                ui.horizontal_top(|ui| {
                    badge(ui, r.level);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(r.title).strong());
                        if !r.detail.is_empty() {
                            ui.label(RichText::new(&r.detail).small().color(MUTED()));
                        }
                    });
                });
                ui.add_space(2.0);
            }

            if !self.groups.is_empty() {
                super::section(ui, "Metadata");
                for g in &self.groups {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("{} · {} {}", g.title, g.total, g.noun))
                            .small()
                            .color(MUTED()),
                    );
                    for f in &g.fields {
                        bar(ui, f);
                    }
                }
            }

            ui.add_space(14.0);
            let wide = |ui: &mut Ui, text: &str| {
                ui.add_sized([ui.available_width(), 42.0], |ui: &mut Ui| {
                    primary_button(ui, text, true)
                })
                .clicked()
            };
            let next = match &self.client {
                Some(c) if c.info.auth && c.token.is_none() => "sign in",
                _ => "open the library",
            };
            if running {
                ui.horizontal(|ui| {
                    spinner_text(ui, "Checking…");
                    if ui.link("Cancel").clicked() {
                        action = Action::ChangeServer;
                    }
                });
            } else if fails > 0 || self.client.is_none() {
                let n = fails.max(1);
                let msg = format!(
                    "{n} {} must be fixed on the server before Valo can use it. \
                     Only the server's admin can do that: send them the report.",
                    if n == 1 { "problem" } else { "problems" }
                );
                ui.label(RichText::new(msg).color(ERROR()));
                ui.add_space(8.0);
                if wide(ui, "Try again") {
                    action = Action::CheckServer(self.address.clone());
                }
            } else if warns > 0 {
                let msg = format!(
                    "Valo works with this server, but {} above {} missing or incomplete.",
                    if warns == 1 {
                        "one thing"
                    } else {
                        "some things"
                    },
                    if warns == 1 { "is" } else { "are" },
                );
                ui.label(RichText::new(msg).color(WARN()));
                ui.add_space(8.0);
                if wide(ui, "I know this is missing, continue") {
                    action = self.proceed();
                }
            } else {
                ui.label(RichText::new("Everything checks out.").color(OK()));
                ui.add_space(8.0);
                if wide(ui, &format!("Continue to {next}")) {
                    action = self.proceed();
                }
            }

            if !running {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let id = egui::Id::new(("check-report-copied", &self.address));
                    let copied = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
                    let label = if copied { "Copied" } else { "Copy report" };
                    if small_button(ui, label).clicked() {
                        ui.ctx().copy_text(self.report());
                        ui.data_mut(|d| d.insert_temp(id, true));
                    }
                    if fails == 0 && self.client.is_some() && small_button(ui, "Re-check").clicked()
                    {
                        action = Action::CheckServer(self.address.clone());
                    }
                    if ui.link("API guide").clicked() {
                        super::chrome::open_url(API_DOCS);
                    }
                    if ui.link("Change address").clicked() {
                        action = Action::ChangeServer;
                    }
                });
            }
            action
        })
    }

    fn proceed(&mut self) -> Action {
        let Some(client) = self.client.clone() else {
            return Action::None;
        };
        if client.info.auth && client.token.is_none() {
            Action::Connected(client)
        } else {
            Action::Ready {
                client,
                user: self.user.clone(),
            }
        }
    }
}
