//! Sign-in, shown when the server says it needs one. The Iced port of `ui::login`.
use std::time::Instant;

use anyhow::Result;
use iced::widget::{checkbox, column, container, responsive, space, text};
use iced::{Element, Length};

use super::widgets::{admin_help, card, field, label, link, primary, spinner_text};
use super::{style, Job, Out};
use crate::api::{Client, Missing, Session, API_DOCS};
use crate::ui::Action;

pub const USER: &str = "login-user";
pub const PASS: &str = "login-pass";

pub struct Login {
    client: Client,
    username: String,
    password: String,
    remember: bool,
    job: Option<Job<Result<Session>>>,
    error: Option<String>,
    missing: Option<Missing>,
    copied: bool,
    started: Instant,
}

#[derive(Clone, Debug)]
pub enum Msg {
    User(String),
    Pass(String),
    Remember(bool),
    /// Enter in the username: on to the password, or sign in when it's filled in.
    UserEnter,
    Submit,
    ChangeServer,
    CopyNote,
    Docs,
}

impl Login {
    pub fn new(client: Client, username: String, error: Option<String>) -> Self {
        Self {
            client,
            username,
            password: String::new(),
            remember: true,
            job: None,
            error,
            missing: None,
            copied: false,
            started: Instant::now(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// The field to type in first: the username, or the password once it's known.
    pub fn first_field(&self) -> &'static str {
        if self.username.is_empty() {
            USER
        } else {
            PASS
        }
    }

    fn submit(&mut self) {
        if self.busy() || self.username.trim().is_empty() {
            return;
        }
        let (c, u, p) = (
            self.client.clone(),
            self.username.trim().to_owned(),
            self.password.clone(),
        );
        self.job = Some(Job::spawn(move || c.login(&u, &p)));
        self.started = Instant::now();
        self.error = None;
        self.missing = None;
    }

    pub fn tick(&mut self) -> Out {
        let Some(r) = self.job.as_mut().and_then(Job::poll) else {
            return Out::None;
        };
        self.job = None;
        match r {
            Ok(s) => {
                let mut client = self.client.clone();
                client.token = Some(s.token);
                client.admin = s.admin;
                let user = s.name.unwrap_or_else(|| self.username.trim().to_owned());
                Out::Act(Action::SignedIn {
                    client,
                    user,
                    remember: self.remember,
                })
            }
            Err(e) => {
                self.missing = e.downcast_ref::<Missing>().copied();
                self.error = Some(format!("{e:#}"));
                self.copied = false;
                self.password.clear();
                Out::Focus(PASS)
            }
        }
    }

    pub fn update(&mut self, msg: Msg) -> Out {
        match msg {
            Msg::User(u) => self.username = u,
            Msg::Pass(p) => self.password = p,
            Msg::Remember(r) => self.remember = r,
            Msg::UserEnter if self.password.is_empty() => return Out::Focus(PASS),
            Msg::UserEnter | Msg::Submit => self.submit(),
            Msg::ChangeServer => return Out::Act(Action::ChangeServer),
            Msg::CopyNote => {
                if let Some(m) = self.missing {
                    self.copied = true;
                    return Out::Copy(m.admin_note(&self.client.base));
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
        let ok = !self.username.trim().is_empty();
        let input = |f: fn(String) -> Msg| (!busy).then_some(f);
        let mut col = column![
            text("Sign in").font(style::BOLD).size(22),
            text(format!("to {}", self.client.info.name)).color(style::MUTED()),
            space().height(14),
            label("Username", 11.0),
            field(
                USER,
                &self.username,
                "",
                false,
                input(Msg::User),
                (!busy).then_some(Msg::UserEnter),
            ),
            space().height(4),
            label("Password", 11.0),
            field(
                PASS,
                &self.password,
                "",
                true,
                input(Msg::Pass),
                (!busy && ok).then_some(Msg::Submit),
            ),
            space().height(4),
            checkbox(self.remember)
                .label("Keep me signed in")
                .on_toggle(Msg::Remember),
            space().height(10),
        ]
        .spacing(4);
        col = col.push(if busy {
            spinner_text("Signing in…", self.started.elapsed().as_secs_f32())
        } else {
            primary("Sign in", ok.then_some(Msg::Submit)).into()
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
        col = col.push(space().height(14)).push(
            container(link(
                &format!("Not {}? Use a different server", self.client.base),
                Msg::ChangeServer,
            ))
            .center_x(Length::Fill),
        );
        col.into()
    }
}
