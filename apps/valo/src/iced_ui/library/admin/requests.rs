//! Admin › Requests: titles people asked for, usually through a Discord
//! bot. Answering here changes the request's status; the server (or its
//! bot) tells the requester.
use iced::alignment::Vertical;
use iced::widget::{column, container, row, space, text, text_input, Column};
use iced::{Element, Padding};

use super::super::details::{caps, column_of, gap, link, panel, NARROW};
use super::super::profile::Remote;
use super::super::{cards, spin_secs, Msg as LMsg};
use super::{
    danger_button, more_menu, not_set_up, row_frame, Cx, Done, JobFn, Msg as AMsg, Section,
};
use crate::api::{Client, Library, MediaRequest};
use crate::iced_ui::icon::Icon;
use crate::iced_ui::spaced::mono;
use crate::iced_ui::style::{self, c, BOLD};
use crate::iced_ui::widgets::{self, Kind as Btn};
use crate::ui::browse::details::TEAL;
use crate::ui::browse::people::ago;
use crate::ui::theme::{ERROR, OK, WARN};

/// How often the queue is checked for new requests, in seconds.
const EVERY: u64 = 30;

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum Filter {
    #[default]
    Pending,
    Approved,
    Available,
    Denied,
    All,
}

impl Filter {
    fn matches(self, status: &str) -> bool {
        match self {
            Filter::All => true,
            Filter::Pending => status == "pending" || status.is_empty(),
            Filter::Approved => status == "approved",
            Filter::Available => status == "available",
            Filter::Denied => status == "denied",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Msg {
    Filter(Filter),
    /// Set a request's status (with a note, for a denial).
    Status(String, &'static str, Option<String>),
    /// Start denying, or stop.
    Deny(Option<String>),
    Reason(String),
    /// Ask "remove this request?", or stop asking.
    ConfirmDelete(Option<String>),
    Delete(String),
}

#[derive(Default)]
pub struct State {
    list: Remote<Vec<MediaRequest>>,
    filter: Filter,
    /// Denying this request, with this reason.
    deny: Option<(String, String)>,
    confirm_delete: Option<String>,
}

impl State {
    pub fn refresh(&mut self) {
        self.list.retry();
    }

    pub fn missing(&self) -> bool {
        self.list.missing
    }

    pub fn busy(&self) -> bool {
        self.list.loading()
    }

    /// Requests waiting for an answer, for the sidebar.
    pub fn pending(&self) -> usize {
        self.list.data.as_ref().map_or(0, |l| {
            l.iter()
                .filter(|r| Filter::Pending.matches(&r.status))
                .count()
        })
    }

    /// Keep the queue fresh, whichever section is open.
    pub fn background(&mut self, client: &Client, login: &mut bool) {
        self.list.poll(login);
        if self.list.idle()
            || (!self.list.missing && self.list.error.is_none() && self.list.stale(EVERY))
        {
            let c = client.clone();
            self.list.load(move || c.admin_requests());
        }
    }

    fn title_of(&self, id: &str) -> String {
        self.list
            .data
            .iter()
            .flatten()
            .find(|r| r.id == id)
            .map(|r| r.title.clone())
            .unwrap_or_default()
    }

    pub fn update(&mut self, m: Msg) -> Option<JobFn> {
        match m {
            Msg::Filter(f) => self.filter = f,
            Msg::Deny(d) => self.deny = d.map(|id| (id, String::new())),
            Msg::Reason(r) => {
                if let Some((_, reason)) = self.deny.as_mut() {
                    *reason = r;
                }
            }
            Msg::ConfirmDelete(d) => self.confirm_delete = d,
            Msg::Status(id, status, note) => {
                self.deny = None;
                let title = self.title_of(&id);
                return Some(Box::new(move |c: &Client| {
                    c.admin_update_request(&id, status, note.as_deref())?;
                    let what = match status {
                        "approved" => "Approved",
                        "available" => "Marked available:",
                        "denied" => "Denied",
                        _ => "Updated",
                    };
                    Ok(Done::msg(format!("{what} {title}.")))
                }));
            }
            Msg::Delete(id) => {
                self.confirm_delete = None;
                let title = self.title_of(&id);
                return Some(Box::new(move |c: &Client| {
                    c.admin_delete_request(&id)?;
                    Ok(Done::msg(format!("Removed the request for {title}.")))
                }));
            }
        }
        None
    }

    pub fn view<'a>(&'a self, cx: &Cx<'a>, w: f32) -> Element<'a, LMsg> {
        let (m, inner) = column_of(w);
        if self.list.missing {
            return container(not_set_up(Section::Requests, &cx.client.info.name))
                .padding(Padding::default().left(m))
                .width(m + inner)
                .into();
        }
        let all = self.list.data.clone().unwrap_or_default();
        let count = |f: Filter| Some(all.iter().filter(|r| f.matches(&r.status)).count());
        let msg = |f: Filter| LMsg::Admin(AMsg::Requests(Msg::Filter(f)));
        let mut head: Column<'a, LMsg> = column![widgets::segmented(
            [
                (Filter::Pending, "Pending"),
                (Filter::Approved, "Approved"),
                (Filter::Available, "Available"),
                (Filter::Denied, "Denied"),
                (Filter::All, "All"),
            ]
            .into_iter()
            .map(|(f, l)| (l.to_owned(), count(f), self.filter == f, msg(f)))
            .collect(),
        )];
        head = head.push(gap(18.0));
        if let Some(e) = &self.list.error {
            head = head.push(
                text(format!("Couldn't load the requests: {e}"))
                    .size(14.5)
                    .color(style::ERROR()),
            );
        }

        let mut shown: Vec<&MediaRequest> = all
            .iter()
            .filter(|r| self.filter.matches(&r.status))
            .collect();
        shown.sort_by_key(|r| std::cmp::Reverse(r.created));
        let narrow = inner < NARROW;
        let g = 48.0;
        let lw = if narrow { inner } else { inner * 0.66 };
        let rw = if narrow { inner } else { inner - lw - g };
        let mut main: Column<'a, LMsg> = column![].width(lw);
        if shown.is_empty() && self.list.data.is_some() {
            main = main
                .push(gap(20.0))
                .push(mono("No requests here", 12.0).color(style::MUTED()));
        }
        if self.list.data.is_none() && self.list.loading() {
            main = main.push(crate::iced_ui::icon::spinner(
                spin_secs(),
                style::MUTED(),
                20.0,
            ));
        }
        for r in shown {
            main = main.push(self.row(r, cx)).push(gap(8.0));
        }
        let mut how: Column<'a, LMsg> = column![caps("How requests work", c(TEAL())), gap(8.0)];
        for line in [
            "People ask for a movie or series in Discord, through the server's bot.",
            "Approve it when you'll get it, then mark it available once it's in \
             the library. Deny it with a reason if you won't.",
            "Each answer goes back to the requester on Discord.",
        ] {
            how = how
                .push(text(line).size(14.5).color(style::MUTED()))
                .push(gap(6.0));
        }
        how = how.push(link(
            "Bot setup guide",
            14.5,
            style::TEXT(),
            Some((Icon::External, true)),
            LMsg::OpenUrl(Section::Requests.docs().to_owned()),
        ));
        let side = container(panel(how)).width(rw);
        let body: Element<'a, LMsg> = if narrow {
            column![main, gap(24.0), side].into()
        } else {
            row![main, space().width(g), side].into()
        };
        container(column![head, body])
            .padding(Padding::default().left(m))
            .into()
    }

    /// One request: its poster, who asked and how it stands, and the answers.
    fn row<'a>(&'a self, r: &MediaRequest, cx: &Cx<'a>) -> Element<'a, LMsg> {
        let busy = cx.busy;
        let have = in_library(cx.lib, r);
        let send = |m: Msg| LMsg::Admin(AMsg::Requests(m));
        let year = r.year.map(|y| format!(" ({y})")).unwrap_or_default();
        let mut title = row![text(format!("{}{year}", r.title)).font(BOLD).size(16)]
            .spacing(10)
            .align_y(Vertical::Center);
        if let Some(k) = &r.kind {
            title = title.push(mono(k, 9.5).color(style::MUTED()));
        }
        let mut info: Column<'a, LMsg> = column![title].spacing(3);
        let mut who = Vec::new();
        if let Some(b) = &r.requested_by {
            who.push(format!("by {b}"));
        }
        if let Some(s) = &r.source {
            who.push(format!("via {}", capitalize(s)));
        }
        if let Some(t) = r.created {
            who.push(ago(t));
        }
        if !who.is_empty() {
            info = info.push(mono(&who.join("   ·   "), 10.0).color(style::MUTED()));
        }
        let (col, label) = match r.status.as_str() {
            "approved" => (TEAL(), "Approved"),
            "available" => (OK(), "Available"),
            "denied" => (ERROR(), "Denied"),
            _ => (WARN(), "Pending"),
        };
        let mut state = row![mono(label, 10.0).color(c(col))].spacing(10);
        if have && r.status != "available" {
            state = state.push(mono("already in the library", 10.0).color(c(OK())));
        }
        if r.notified == Some(false) {
            state = state.push(crate::iced_ui::chrome::tip_below(
                mono("requester wasn't told", 10.0).color(c(ERROR())),
                "The bot couldn't message them, for example because their Discord DMs are closed.",
            ));
        }
        info = info.push(state);
        if let Some(n) = r.note.as_ref().filter(|n| !n.trim().is_empty()) {
            info = info.push(text(format!("“{n}”")).size(14.5));
        }
        // the answers: Approve, Mark available, Deny…, ⋮
        let mut answers = row![].spacing(8).align_y(Vertical::Center);
        let id = r.id.clone();
        let ok = |m: Msg| (!busy).then(|| send(m));
        if Filter::Pending.matches(&r.status) {
            answers = answers.push(widgets::button_with(
                Btn::Small,
                "Approve",
                None,
                ok(Msg::Status(id.clone(), "approved", None)),
            ));
        }
        if r.status != "available" && (have || r.status == "approved") {
            answers = answers.push(widgets::button_with(
                Btn::Small,
                "Mark available",
                None,
                ok(Msg::Status(id.clone(), "available", None)),
            ));
        }
        if r.status != "denied" && r.status != "available" {
            answers = answers.push(widgets::button_with(
                Btn::Small,
                "Deny…",
                None,
                ok(Msg::Deny(Some(id.clone()))),
            ));
        }
        let key = format!("req\u{1f}{}", r.id);
        let mut items = Vec::new();
        if r.status != "pending" {
            items.push((
                "Back to pending",
                send(Msg::Status(id.clone(), "pending", None)),
            ));
        }
        items.push((
            "Remove request…",
            send(Msg::ConfirmDelete(Some(id.clone()))),
        ));
        answers = answers.push(more_menu(
            key.clone(),
            cx.menu == Some(key.as_str()),
            items,
            busy,
        ));

        let mut body: Column<'a, LMsg> = column![row![
            cards::art(cx.images, r.poster.as_deref(), &r.title, 54.0, 81.0, 4.0),
            info.width(iced::Length::Fill),
            answers,
        ]
        .spacing(16)
        .align_y(Vertical::Top)];
        if let Some((_, reason)) = self.deny.as_ref().filter(|(d, _)| *d == r.id) {
            body = body
                .push(gap(10.0))
                .push(crate::iced_ui::widgets::label(
                    "Reason (sent to the requester)",
                    11.0,
                ))
                .push(space().height(8))
                .push(
                    text_input("e.g. not available in good quality yet", reason)
                        .on_input(|s| LMsg::Admin(AMsg::Requests(Msg::Reason(s))))
                        .size(15)
                        .padding([9, 10])
                        .style(super::super::profile::field_style),
                )
                .push(gap(6.0))
                .push(
                    row![
                        danger_button(
                            "Deny request",
                            (!busy).then(|| send(Msg::Status(
                                id.clone(),
                                "denied",
                                Some(reason.clone())
                            ))),
                        ),
                        widgets::button_with(
                            Btn::Small,
                            "Cancel",
                            None,
                            Some(send(Msg::Deny(None)))
                        ),
                    ]
                    .spacing(10),
                );
        }
        if self.confirm_delete.as_deref() == Some(r.id.as_str()) {
            body = body
                .push(gap(10.0))
                .push(
                    text(format!(
                        "Remove the request for {}? The requester isn't told.",
                        r.title
                    ))
                    .size(14.5),
                )
                .push(gap(6.0))
                .push(
                    row![
                        danger_button("Remove", (!busy).then(|| send(Msg::Delete(id.clone())))),
                        widgets::button_with(
                            Btn::Small,
                            "Cancel",
                            None,
                            Some(send(Msg::ConfirmDelete(None)))
                        ),
                    ]
                    .spacing(10),
                );
        }
        row_frame(body)
    }
}

/// The library already has it (same IMDb id, or same title and year).
fn in_library(lib: &Library, r: &MediaRequest) -> bool {
    let title = r.title.to_lowercase();
    lib.all().any(|i| match (&i.imdb_id, &r.imdb_id) {
        (Some(a), Some(b)) => a == b,
        _ => i.title.to_lowercase() == title && (r.year.is_none() || i.year == r.year),
    })
}

fn capitalize(s: &str) -> String {
    let mut ch = s.chars();
    ch.next()
        .map_or(String::new(), |f| f.to_uppercase().chain(ch).collect())
}
