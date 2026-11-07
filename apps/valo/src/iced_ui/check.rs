//! Shown after the user enters a new server's address: checks every API the
//! app uses, one by one, and how complete the metadata is. Errors block the
//! server; warnings can be accepted. Sign-in happens halfway through.
//! The Iced port of `ui::check`.
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Instant;

use iced::alignment::Vertical;
use iced::widget::{column, container, progress_bar, responsive, row, space, text};
use iced::{Background, Border, Color, Element, Length};

use super::icon;
use super::widgets::{card, link, primary, section, small, spinner_text};
use super::{style, Out};
use crate::api::check::{self, Event, Group, Job, Level};
use crate::api::{normalize, Client, API_DOCS};
use crate::ui::theme;
use crate::ui::Action;

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
    copied: bool,
    started: Instant,
}

#[derive(Clone, Debug)]
pub enum Msg {
    Cancel,
    Retry,
    Proceed,
    Copy,
    Docs,
    ChangeAddress,
}

fn color(l: Level) -> Color {
    let p = theme::pal();
    style::c(match l {
        Level::Pass => p.ok,
        Level::Later => p.muted,
        Level::Warn => p.warn,
        Level::Fail => p.error,
    })
}

fn tag(l: Level) -> &'static str {
    match l {
        Level::Pass => "PASS",
        Level::Later => "LATER",
        Level::Warn => "WARN",
        Level::Fail => "FAIL",
    }
}

impl Check {
    pub fn new(address: String) -> Self {
        let rx = check::start(|| {}, Job::Address(address.clone()));
        Self {
            address,
            rows: Vec::new(),
            groups: Vec::new(),
            rx: Some(rx),
            client: None,
            user: None,
            signed_in_at: None,
            copied: false,
            started: Instant::now(),
        }
    }

    /// Run the rest of the checks with the signed-in client.
    pub fn signed_in(&mut self, client: Client, user: Option<String>) {
        self.user = user;
        self.client = None;
        self.signed_in_at = Some(self.rows.len());
        self.rx = Some(check::start(|| {}, Job::SignedIn(client)));
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }

    fn count(&self, l: Level) -> usize {
        self.rows.iter().filter(|r| r.level == Some(l)).count()
    }

    /// Take in what the check has found since the last tick.
    pub fn tick(&mut self) -> Out {
        let Some(rx) = &self.rx else {
            return Out::None;
        };
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
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.rx = None;
                    break;
                }
            }
        }
        Out::None
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

    fn proceed(&self) -> Out {
        let Some(client) = self.client.clone() else {
            return Out::None;
        };
        Out::Act(if client.info.auth && client.token.is_none() {
            Action::Connected(client)
        } else {
            Action::Ready {
                client,
                user: self.user.clone(),
            }
        })
    }

    pub fn update(&mut self, msg: Msg) -> Out {
        match msg {
            Msg::Cancel | Msg::ChangeAddress => Out::Act(Action::ChangeServer),
            Msg::Retry => Out::Act(Action::CheckServer(self.address.clone())),
            Msg::Proceed => self.proceed(),
            Msg::Copy => {
                self.copied = true;
                Out::Copy(self.report())
            }
            Msg::Docs => {
                crate::ui::chrome::open_url(API_DOCS);
                Out::None
            }
        }
    }

    fn badge(&self, level: Option<Level>) -> Element<'_, Msg> {
        let Some(l) = level else {
            let t = self.started.elapsed().as_secs_f32();
            return container(icon::spinner(t, style::TEXT(), 14.0))
                .center_x(52)
                .center_y(20)
                .into();
        };
        let c = color(l);
        container(text(tag(l)).font(style::MONO).size(10.5).color(c))
            .center_x(52)
            .center_y(20)
            .style(move |_| container::Style {
                border: Border {
                    color: c,
                    width: 1.0,
                    radius: 2.0.into(),
                },
                ..container::Style::default()
            })
            .into()
    }

    /// "Poster ▓▓▓▓░░ 48/52"
    fn bar(f: &check::Field) -> Element<'_, Msg> {
        let c = color(f.level());
        let frac = f.have as f32 / f.total.max(1) as f32;
        let line = style::LINE();
        row![
            text(f.name).size(13).width(110),
            progress_bar(0.0..=1.0, frac)
                .girth(6)
                .length(Length::Fill)
                .style(move |_| progress_bar::Style {
                    background: Background::Color(line),
                    bar: Background::Color(c),
                    border: iced::border::rounded(1),
                }),
            container(
                text(format!("{}/{}", f.have, f.total))
                    .font(style::MONO)
                    .size(11.5)
                    .color(if f.have == f.total { style::MUTED() } else { c }),
            )
            .width(72)
            .align_x(iced::alignment::Horizontal::Right),
        ]
        .align_y(Vertical::Center)
        .height(20)
        .into()
    }

    pub fn view(&self) -> Element<'_, Msg> {
        responsive(move |size| card(size.height, self.body())).into()
    }

    fn body(&self) -> Element<'_, Msg> {
        let running = self.busy();
        let (fails, warns) = (self.count(Level::Fail), self.count(Level::Warn));
        let mut col = column![
            text("Checking the server").font(style::BOLD).size(20),
            text(normalize(&self.address)).color(style::MUTED()),
            space().height(12),
        ]
        .spacing(2);

        for (i, r) in self.rows.iter().enumerate() {
            if self.signed_in_at == Some(i) {
                col = col.push(section("After signing in"));
            }
            let mut what = column![text(r.title).font(style::BOLD).size(14.5)];
            if !r.detail.is_empty() {
                what = what.push(text(r.detail.clone()).size(12).color(style::MUTED()));
            }
            col = col.push(
                row![self.badge(r.level), what]
                    .spacing(10)
                    .padding(iced::Padding::default().bottom(2.0)),
            );
        }

        if !self.groups.is_empty() {
            col = col.push(section("Metadata"));
            for g in &self.groups {
                col = col.push(space().height(4)).push(
                    text(format!("{} · {} {}", g.title, g.total, g.noun))
                        .size(12)
                        .color(style::MUTED()),
                );
                for f in &g.fields {
                    col = col.push(Self::bar(f));
                }
            }
        }

        col = col.push(space().height(14));
        let next = match &self.client {
            Some(c) if c.info.auth && c.token.is_none() => "sign in",
            _ => "open the library",
        };
        let pal = theme::pal();
        if running {
            let t = self.started.elapsed().as_secs_f32();
            col = col.push(
                row![spinner_text("Checking…", t), link("Cancel", Msg::Cancel)]
                    .spacing(14)
                    .align_y(Vertical::Center),
            );
        } else if fails > 0 || self.client.is_none() {
            let n = fails.max(1);
            let msg = format!(
                "{n} {} must be fixed on the server before Valo can use it. \
                 Only the server's admin can do that: send them the report.",
                if n == 1 { "problem" } else { "problems" }
            );
            col = col
                .push(text(msg).color(style::c(pal.error)))
                .push(space().height(8))
                .push(primary("Try again", Some(Msg::Retry)));
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
            col = col
                .push(text(msg).color(style::c(pal.warn)))
                .push(space().height(8))
                .push(primary(
                    "I know this is missing, continue",
                    Some(Msg::Proceed),
                ));
        } else {
            col = col
                .push(text("Everything checks out.").color(style::c(pal.ok)))
                .push(space().height(8))
                .push(primary(&format!("Continue to {next}"), Some(Msg::Proceed)));
        }

        if !running {
            let mut actions = row![small(
                if self.copied { "Copied" } else { "Copy report" },
                Msg::Copy
            )]
            .spacing(12)
            .align_y(Vertical::Center);
            if fails == 0 && self.client.is_some() {
                actions = actions.push(small("Re-check", Msg::Retry));
            }
            actions = actions
                .push(link("API guide", Msg::Docs))
                .push(link("Change address", Msg::ChangeAddress));
            col = col.push(space().height(6)).push(actions);
        }
        col.into()
    }
}
