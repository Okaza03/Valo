//! First screen: where is the media server? The Iced port of `ui::setup`.
use std::time::Instant;

use anyhow::Result;
use iced::alignment::Vertical;
use iced::widget::{column, container, responsive, row, rule, space, text};
use iced::Element;

use super::widgets::{admin_help, card, field, label, link, primary, small, spinner_text};
use super::{style, Job, Out};
use crate::api::{normalize, Client, Missing, API_DOCS};
use crate::ui::Action;

/// The address field, to focus it.
pub const FIELD: &str = "setup-address";

pub struct Setup {
    address: String,
    job: Option<Job<Result<Client>>>,
    error: Option<String>,
    missing: Option<Missing>,
    /// The admin message was copied.
    copied: bool,
    started: Instant,
}

#[derive(Clone, Debug)]
pub enum Msg {
    Address(String),
    Connect,
    Cancel,
    OpenFiles,
    CopyNote,
    Docs,
}

impl Setup {
    /// With `auto`, connects to `address` right away (the saved server).
    pub fn new(address: String, auto: bool) -> Self {
        let mut s = Self {
            address,
            job: None,
            error: None,
            missing: None,
            copied: false,
            started: Instant::now(),
        };
        if auto && !s.address.trim().is_empty() {
            let a = s.address.clone();
            s.job = Some(Job::spawn(move || Client::connect(&a)));
        }
        s
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// The field wants the keyboard when it's empty.
    pub fn wants_focus(&self) -> bool {
        self.address.is_empty() && !self.busy()
    }

    pub fn tick(&mut self) -> Out {
        let Some(r) = self.job.as_mut().and_then(Job::poll) else {
            return Out::None;
        };
        self.job = None;
        match r {
            Ok(c) => Out::Act(Action::Connected(c)),
            Err(e) => {
                self.missing = e.downcast_ref::<Missing>().copied();
                self.error = Some(format!("{e:#}"));
                self.copied = false;
                Out::None
            }
        }
    }

    pub fn update(&mut self, msg: Msg) -> Out {
        match msg {
            Msg::Address(a) => self.address = a,
            Msg::Connect => {
                if !self.busy() && !self.address.trim().is_empty() {
                    // a server the user enters gets the full check first
                    return Out::Act(Action::CheckServer(self.address.trim().to_owned()));
                }
            }
            Msg::Cancel => self.job = None,
            Msg::OpenFiles => return Out::Act(Action::OpenFiles),
            Msg::CopyNote => {
                if let Some(m) = self.missing {
                    self.copied = true;
                    return Out::Copy(m.admin_note(&normalize(&self.address)));
                }
            }
            Msg::Docs => crate::ui::chrome::open_url(API_DOCS),
        }
        Out::None
    }

    pub fn view(&self) -> Element<'_, Msg> {
        responsive(move |size| card(size.height, self.body())).into()
    }

    fn body(&self) -> Element<'_, Msg> {
        let busy = self.busy();
        let ok = !self.address.trim().is_empty();
        let mut col = column![
            text("Connect to your media library")
                .font(style::BOLD)
                .size(20),
            text(
                "Enter the IP address (and port) of the computer on your network \
                 that serves your movies and series."
            )
            .color(style::MUTED()),
            space().height(14),
            label("Server address", 11.0),
            field(
                FIELD,
                &self.address,
                "192.168.1.20:8080",
                false,
                (!busy).then_some(Msg::Address as fn(String) -> Msg),
                (!busy && ok).then_some(Msg::Connect),
            ),
            text("http:// is assumed. Include a path if the library lives in a subfolder.")
                .size(12)
                .color(style::MUTED()),
            space().height(12),
        ]
        .spacing(4);
        col = col.push(if busy {
            let t = self.started.elapsed().as_secs_f32();
            Element::from(
                row![
                    spinner_text(&format!("Connecting to {}…", self.address.trim()), t),
                    link("Cancel", Msg::Cancel),
                ]
                .spacing(14)
                .align_y(Vertical::Center),
            )
        } else {
            primary("Connect", ok.then_some(Msg::Connect)).into()
        });
        if let Some(e) = &self.error {
            col = col.push(
                container(text(e.clone()).color(style::ERROR()))
                    .padding(iced::Padding::default().top(6.0)),
            );
            if let Some(m) = self.missing {
                col = col.push(admin_help(m, self.copied, Msg::CopyNote, Msg::Docs));
            }
        }
        col = col.push(space().height(16)).push(rule::horizontal(1)).push(
            row![
                text("Just want to watch a file?").color(style::MUTED()),
                small("Open file", Msg::OpenFiles),
            ]
            .spacing(10)
            .align_y(Vertical::Center),
        );
        col.into()
    }
}
