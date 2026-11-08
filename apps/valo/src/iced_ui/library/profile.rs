//! The profile page (watching stats, history, "Your devices"), "Viewed by
//! other users", the "Change your password" prompt and the new-arrivals
//! dialog, laid out like egui's `browse::people`, `devices`, `viewed`,
//! `password` and `arrivals`.
use std::collections::HashMap;
use std::time::Instant;

use anyhow::Result;
use iced::alignment::Vertical;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::widget::{column, container, row, space, stack, text, text_input, Column};
use iced::{
    mouse, Background, Border, Color, Element, Length, Padding, Rectangle, Renderer, Size, Theme,
};

use super::cards::{self, GAP, SIDE};
use super::details::{caps, column_of, gap, hover_area, kv_rows, link, panel, NARROW};
use super::{fill_gradient, muted_center, Env, Filter, Library, Msg, View};
use crate::api::{self, featurette, DeviceSession, Item, Viewed, Watcher};
use crate::config::Entry;
use crate::iced_ui::dialogs;
use crate::iced_ui::spaced::mono;
use crate::iced_ui::style::{self, black, c, BLACK, BOLD};
use crate::iced_ui::widgets::{self, Kind};
use crate::iced_ui::Job;
use crate::ui::browse::arrivals::SHOWN;
use crate::ui::browse::details::{GOLD, TEAL};
use crate::ui::browse::people::ago;
use crate::ui::theme::{CARD, LINE};
use crate::util::{color_for, fmt_runtime};

/// Something loaded from the server, again when asked (egui's `admin::Remote`).
pub(super) struct Remote<T> {
    pub data: Option<T>,
    job: Option<Job<Result<T>>>,
    pub error: Option<String>,
    at: Option<Instant>,
}

impl<T> Default for Remote<T> {
    fn default() -> Self {
        Self {
            data: None,
            job: None,
            error: None,
            at: None,
        }
    }
}

impl<T: Send + 'static> Remote<T> {
    pub fn load(&mut self, f: impl FnOnce() -> Result<T> + Send + 'static) {
        self.job = Some(Job::spawn(f));
    }

    pub fn loading(&self) -> bool {
        self.job.is_some()
    }

    /// Nothing loaded or loading, and no error to wait out.
    pub fn idle(&self) -> bool {
        self.data.is_none() && self.job.is_none() && self.error.is_none()
    }

    /// Older than `secs` (and not loading).
    pub fn stale(&self, secs: u64) -> bool {
        self.job.is_none() && self.at.is_none_or(|t| t.elapsed().as_secs() >= secs)
    }

    /// Load again next time, keeping what's shown meanwhile.
    pub fn retry(&mut self) {
        self.error = None;
        self.at = None;
    }

    /// Take a finished load. A 401 sets `login`.
    pub fn poll(&mut self, login: &mut bool) {
        let Some(r) = self.job.as_mut().and_then(Job::poll) else {
            return;
        };
        self.job = None;
        self.at = Some(Instant::now());
        match r {
            Ok(d) => {
                self.data = Some(d);
                self.error = None;
            }
            Err(e) if e.is::<api::Unauthorized>() => *login = true,
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }
}

/// "Your devices".
#[derive(Default)]
pub(super) struct Devices {
    /// `None` inside: the server doesn't list sessions.
    list: Remote<Option<Vec<DeviceSession>>>,
    op: Option<Job<Result<String>>>,
    notice: Option<(String, bool)>,
}

/// What others watched lately.
#[derive(Default)]
pub(super) struct ViewedList {
    list: Option<Vec<Viewed>>,
    job: Option<Job<Result<Option<Vec<Viewed>>>>>,
    at: Option<Instant>,
}

/// "Change your password".
#[derive(Default)]
pub(super) struct Prompt {
    pub open: bool,
    current: String,
    new: String,
    again: String,
    job: Option<Job<Result<()>>>,
    error: Option<String>,
    /// Saved: say so, then close.
    changed: bool,
}

#[derive(Clone, Debug)]
pub enum PwMsg {
    Current(String),
    New(String),
    Again(String),
    Submit,
    Close,
}

const PW_FIELDS: [&str; 3] = ["pw-current", "pw-new", "pw-again"];

/// "Jo", "Jo +2": who played it, for under the poster.
fn names(w: &[Watcher]) -> Option<String> {
    let first = w.first()?.shown();
    Some(match w.len() {
        1 => first.to_owned(),
        n => format!("{first} +{}", n - 1),
    })
}

/// "Viewed by other users"
pub(super) const VIEWED: &str = "Viewed by other users";

/// A section heading: a small kicker, the title and a count (egui's `section_title`).
fn section_title<'a>(kicker: &str, title: &str, n: usize) -> Element<'a, Msg> {
    column![
        caps(kicker, c(TEAL())),
        gap(4.0),
        row![
            text(title.to_owned()).font(BOLD).size(24),
            mono(&n.to_string(), 13.0).color(style::MUTED()),
        ]
        .spacing(10)
        .align_y(Vertical::Center),
        gap(16.0),
    ]
    .into()
}

/// One big number with a label under it, 92 high.
fn stat<'a>(w: f32, value: String, label: &str) -> Element<'a, Msg> {
    container(
        column![
            text(value).font(BOLD).size(28),
            space::vertical(),
            mono(label, 11.0).color(style::MUTED()),
        ]
        .height(Length::Fill),
    )
    .width(w)
    .height(92)
    .padding(Padding {
        top: 16.0,
        right: 18.0,
        bottom: 14.0,
        left: 18.0,
    })
    .style(|_| container::Style {
        background: Some(Background::Color(c(CARD()))),
        border: Border {
            color: c(LINE()),
            width: 1.0,
            radius: 10.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

/// A row of [`stat`] tiles that wraps on narrow windows.
fn stats<'a>(tiles: Vec<(String, &str)>, w: f32) -> Element<'a, Msg> {
    let g = 14.0;
    let cols = ((w + g) / (170.0 + g))
        .floor()
        .clamp(1.0, tiles.len() as f32) as usize;
    let tw = (w - g * (cols as f32 - 1.0)) / cols as f32;
    let mut out = column![].spacing(g + 8.0);
    let mut it = tiles.into_iter().peekable();
    while it.peek().is_some() {
        let mut r = row![].spacing(g);
        for (v, l) in it.by_ref().take(cols) {
            r = r.push(stat(tw, v, l));
        }
        out = out.push(r);
    }
    out.into()
}

/// A labelled bar ("Favorite genres"), `w` wide.
fn bar<'a>(label: &str, value: String, f: f32, w: f32) -> Element<'a, Msg> {
    let gold = c(GOLD());
    column![
        row![
            text(label.to_owned()).size(14),
            space::horizontal(),
            mono(&value, 11.0).color(style::MUTED()),
        ],
        space().height(6),
        container(
            container(space())
                .width(w * f.clamp(0.0, 1.0))
                .height(5)
                .style(move |_| container::Style {
                    background: Some(Background::Color(gold)),
                    border: iced::border::rounded(2),
                    ..container::Style::default()
                }),
        )
        .width(w)
        .style(|_| container::Style {
            background: Some(Background::Color(style::white(20))),
            border: iced::border::rounded(2),
            ..container::Style::default()
        }),
    ]
    .width(w)
    .height(36)
    .into()
}

/// Main and side columns inside the content column; stacked when narrow.
fn columns<'a>(
    w: f32,
    main: impl FnOnce(f32) -> Element<'a, Msg>,
    side: impl FnOnce(f32) -> Element<'a, Msg>,
) -> Element<'a, Msg> {
    let (m, inner) = column_of(w);
    let narrow = inner < NARROW;
    let g = 48.0;
    let lw = if narrow { inner } else { inner * 0.66 };
    let rw = if narrow { inner } else { inner - lw - g };
    let body: Element<'a, Msg> = if narrow {
        column![main(lw), gap(24.0), side(rw)].into()
    } else {
        row![
            container(main(lw)).width(lw),
            space().width(g),
            container(side(rw)).width(rw),
        ]
        .into()
    };
    container(body).padding(Padding::default().left(m)).into()
}

impl Library {
    /// Poll and start the profile and viewed loads, the devices' sign-outs
    /// and the password change. A 401 asks to sign in again.
    pub(super) fn people_work(&mut self) -> bool {
        let mut login = false;
        // what others watched: loaded once, then every minute while it shows
        let v = &mut self.viewed;
        if let Some(r) = v.job.as_mut().and_then(Job::poll) {
            v.job = None;
            v.at = Some(Instant::now());
            if let Ok(list) = r {
                v.list = list;
            }
        }
        let shows_viewed =
            self.view == View::Viewed || (self.view == View::Home && self.layout.shows("others"));
        if shows_viewed && v.job.is_none() && v.at.is_none_or(|t| t.elapsed().as_secs() >= 60) {
            let cl = self.client.clone();
            v.job = Some(Job::spawn(move || cl.viewed()));
        }
        // devices, on the profile page
        let d = &mut self.devices;
        d.list.poll(&mut login);
        if let Some(r) = d.op.as_mut().and_then(Job::poll) {
            d.op = None;
            match r {
                Ok(msg) => d.notice = Some((msg, true)),
                Err(e) if e.is::<api::Unauthorized>() => login = true,
                Err(e) => d.notice = Some((format!("{e:#}"), false)),
            }
            d.list.retry();
            d.list.data = d.list.data.take(); // keep showing the old list
        }
        if self.view == View::Profile
            && self.client.token.is_some()
            && (d.list.idle() || (d.list.data.is_some() && d.list.stale(60)))
        {
            let cl = self.client.clone();
            d.list.load(move || cl.sessions());
        }
        // the password change
        let p = &mut self.password;
        if let Some(r) = p.job.as_mut().and_then(Job::poll) {
            p.job = None;
            match r {
                Ok(()) => {
                    p.changed = true;
                    p.error = None;
                    p.current.clear();
                    p.new.clear();
                    p.again.clear();
                }
                Err(e) if e.is::<api::Unauthorized>() => {
                    p.open = false;
                    login = true;
                }
                Err(e) => p.error = Some(format!("{e:#}")),
            }
        }
        login
    }

    pub(super) fn people_busy(&self) -> bool {
        self.viewed.job.is_some()
            || self.devices.list.loading()
            || self.devices.op.is_some()
            || self.password.job.is_some()
    }

    /// Sign a device out, or all the others.
    pub(super) fn end_sessions(&mut self, which: Option<String>) -> crate::iced_ui::Out {
        let list = self.devices.list.data.clone().flatten().unwrap_or_default();
        let c = self.client.clone();
        self.devices.notice = None;
        match which {
            Some(id) => {
                let Some(s) = list.into_iter().find(|s| s.id == id) else {
                    return crate::iced_ui::Out::None;
                };
                if s.current {
                    return crate::iced_ui::Out::Act(crate::ui::Action::SignOut(
                        self.client.clone(),
                    ));
                }
                let name = s.device.clone().unwrap_or_else(|| "that device".into());
                self.devices.op = Some(Job::spawn(move || {
                    c.end_session(&s.id)?;
                    Ok(format!("Signed out {name}."))
                }));
            }
            None => {
                let others: Vec<String> = list
                    .into_iter()
                    .filter(|s| !s.current)
                    .map(|s| s.id)
                    .collect();
                self.devices.op = Some(Job::spawn(move || {
                    for id in &others {
                        c.end_session(id)?;
                    }
                    Ok(format!("Signed out {} other devices.", others.len()))
                }));
            }
        }
        crate::iced_ui::Out::None
    }

    pub(super) fn update_password(&mut self, m: PwMsg) -> crate::iced_ui::Out {
        let p = &mut self.password;
        match m {
            PwMsg::Current(s) => p.current = s,
            PwMsg::New(s) => p.new = s,
            PwMsg::Again(s) => p.again = s,
            PwMsg::Submit => {
                let ok = p.job.is_none()
                    && !p.current.is_empty()
                    && !p.new.is_empty()
                    && p.new == p.again;
                if ok {
                    p.error = None;
                    let (c, cur, new) = (self.client.clone(), p.current.clone(), p.new.clone());
                    p.job = Some(Job::spawn(move || c.change_password(&cur, &new)));
                }
            }
            PwMsg::Close => {
                if p.job.is_none() {
                    p.open = false;
                    p.changed = false;
                    p.error = None;
                }
            }
        }
        crate::iced_ui::Out::None
    }

    // ---------------------------------------------------------- viewed

    /// Posters of what others played, latest first, with who under each.
    pub(super) fn viewed_cards(&self, lib: &api::Library, env: Env<'_>) -> Vec<cards::Card> {
        let Some(list) = self.viewed.list.as_ref() else {
            return Vec::new();
        };
        let found: Vec<_> = list
            .iter()
            .filter_map(|v| Some((lib.find(&v.key()?)?, &v.watchers)))
            .collect();
        let mut out = self.poster_cards(found.iter().map(|(i, _)| *i), env.progress);
        for (c, (_, w)) in out.iter_mut().zip(&found) {
            if let Some(n) = names(w) {
                c.sub = Some(n);
            }
        }
        out
    }

    /// The page the row's See all opens.
    pub(super) fn viewed_page<'a>(
        &'a self,
        lib: &'a api::Library,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let cards = self.viewed_cards(lib, env);
        let head = column![
            caps("From other users", c(TEAL())),
            gap(4.0),
            text(VIEWED).font(BOLD).size(34),
            gap(4.0),
            text("What everyone else on this server watched lately, latest first.")
                .size(14.5)
                .color(style::MUTED()),
        ]
        .padding(Padding::default().left(SIDE).top(40.0).bottom(32.0));
        if cards.is_empty() {
            let wait = self.viewed.job.is_some() && self.viewed.list.is_none();
            let body: Element<'a, Msg> = if wait {
                container(crate::iced_ui::icon::spinner(
                    super::spin_secs(),
                    style::MUTED(),
                    24.0,
                ))
                .center_x(Length::Fill)
                .padding(Padding::default().top(40.0))
                .into()
            } else {
                muted_center("Nobody else has watched anything yet")
            };
            return column![head, body].into();
        }
        column![
            head,
            cards::grid(env.images, &cards, w, self.hot(), "viewed")
        ]
        .into()
    }

    // --------------------------------------------------------- profile

    pub(super) fn profile_page<'a>(
        &'a self,
        lib: &'a api::Library,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let name = self.user.clone().unwrap_or_else(|| "Guest".to_owned());
        let prefix = self.prefix();
        let entries = env.progress.recent(&prefix);
        let watched_secs = |e: &Entry| if e.dur > 0.0 { e.pos.min(e.dur) } else { e.pos };
        let (mut movies_done, mut eps_done, mut in_progress) = (0, 0, 0);
        let (mut movie_secs, mut ep_secs) = (0.0, 0.0);
        let mut genre_secs: HashMap<&str, f64> = HashMap::new();
        let mut per_show: HashMap<&str, (usize, f64)> = HashMap::new();
        let mut recent: Vec<(&Item, Option<String>, Option<String>, &Entry)> = Vec::new();
        for (id, e) in &entries {
            let secs = watched_secs(e);
            let item = if let Some(m) = lib.movies.iter().find(|m| m.id == *id) {
                movie_secs += secs;
                movies_done += usize::from(e.watched());
                recent.push((m, None, None, e));
                m
            } else if let Some((s, si, ei)) = lib.find_episode(id) {
                ep_secs += secs;
                eps_done += usize::from(e.watched());
                let show = per_show.entry(s.id.as_str()).or_default();
                show.0 += usize::from(e.watched());
                show.1 += secs;
                let season = &s.seasons[si];
                let ep = &season.episodes[ei];
                let code = featurette(season.number, ep.number)
                    .unwrap_or_else(|| format!("S{:02}E{:02}", season.number, ep.number));
                recent.push((
                    s,
                    Some(ep.id.clone()),
                    Some(format!("{code} · {}", ep.title)),
                    e,
                ));
                s
            } else {
                continue;
            };
            in_progress += usize::from(e.in_progress());
            for g in &item.genres {
                *genre_secs.entry(g.as_str()).or_default() += secs;
            }
        }
        let shows_done = lib
            .series
            .iter()
            .filter(|s| self.status_of(s, env.progress) == Filter::Watched)
            .count();
        let on_list = self
            .watchlist
            .ids(&prefix)
            .filter(|id| lib.find(id).is_some())
            .count();
        let total = movie_secs + ep_secs;
        let time = |s: f64| {
            if s > 0.0 {
                fmt_runtime(s)
            } else {
                "—".into()
            }
        };

        // the header: the name's color fading into the page, the avatar, who and where
        let h = 260.0;
        let (m, inner) = column_of(w);
        let tint = c(color_for(&name));
        let top = Color { a: 0.35, ..tint };
        let bg = style::BG();
        let r = 62.0;
        let how = if self.client.token.is_some() {
            "Signed in to"
        } else {
            "Connected to"
        };
        let mut line = format!("{how} {}   ·   {}", self.client.info.name, self.client.base);
        if self.client.admin {
            line.push_str("   ·   Admin");
        }
        let mut buttons = row![widgets::button_with(
            Kind::Small,
            "Settings",
            None,
            Some(Msg::Settings)
        )]
        .spacing(10);
        if self.client.token.is_some() {
            buttons = buttons.push(widgets::button_with(
                Kind::Small,
                "Sign out",
                None,
                Some(Msg::SignOut),
            ));
        }
        let words = column![
            caps("Profile", c(TEAL())),
            gap(4.0),
            text(name.clone()).font(BLACK).size(44),
            gap(6.0),
            mono(&line, 11.5).color(style::MUTED()),
            gap(14.0),
            buttons,
        ];
        let ring = container(super::avatar(&name, r))
            .padding(4)
            .style(move |_| container::Style {
                background: Some(Background::Color(black(110))),
                border: iced::border::rounded(r + 4.0),
                ..container::Style::default()
            });
        let header = stack![
            container(space())
                .width(w)
                .height(h)
                .style(move |_| fill_gradient(std::f32::consts::PI, top, bg)),
            container(row![ring, space().width(28), words].align_y(Vertical::Center))
                .width(w)
                .height(h)
                .padding(Padding::default().left(m - 4.0).top(20.0))
                .align_y(Vertical::Center),
        ];

        let tiles = stats(
            vec![
                (time(total), "Time watched"),
                (movies_done.to_string(), "Movies finished"),
                (eps_done.to_string(), "Episodes finished"),
                (shows_done.to_string(), "Shows completed"),
                (in_progress.to_string(), "In progress"),
                (on_list.to_string(), "On My list"),
            ],
            inner,
        );

        let mut genres: Vec<(&str, f64)> = genre_secs.into_iter().collect();
        genres.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
        genres.truncate(6);
        let mut shows: Vec<(&Item, usize, f64)> = per_show
            .into_iter()
            .filter_map(|(id, (n, s))| Some((lib.find(id)?, n, s)))
            .collect();
        shows.sort_by(|a, b| b.2.total_cmp(&a.2));
        shows.truncate(5);

        let main = |lw: f32| -> Element<'a, Msg> {
            let mut col: Column<'a, Msg> = column![section_title(
                "History",
                "Recently watched",
                recent.len().min(15)
            )];
            if recent.is_empty() {
                col = col.push(
                    text("Nothing watched yet.")
                        .size(14.5)
                        .color(style::MUTED()),
                );
            }
            for (item, ep, sub, e) in recent.iter().take(15) {
                col = col
                    .push(self.history_row(lib, item, ep.as_deref(), sub.clone(), e, lw, env))
                    .push(space().height(12));
            }
            col.into()
        };
        let side = |rw: f32| -> Element<'a, Msg> {
            let mut col: Column<'a, Msg> = column![];
            if !genres.is_empty() {
                let max = genres[0].1.max(1.0);
                let mut p: Column<'a, Msg> = column![caps("Favorite genres", c(TEAL())), gap(12.0)];
                for (g, s) in &genres {
                    p = p
                        .push(bar(g, fmt_runtime(*s), (*s / max) as f32, rw - 48.0))
                        .push(gap(6.0));
                }
                col = col.push(panel(p)).push(gap(20.0));
            }
            if !shows.is_empty() {
                let mut p: Column<'a, Msg> =
                    column![caps("Most watched shows", c(TEAL())), gap(10.0)];
                for (s, done, secs) in &shows {
                    let eps: usize = s.seasons.iter().map(|x| x.episodes.len()).sum();
                    p = p
                        .push(link(
                            &s.title,
                            14.5,
                            style::TEXT(),
                            None,
                            Msg::Open(s.id.clone()),
                        ))
                        .push(
                            mono(
                                &format!("{done} of {eps} episodes · {}", fmt_runtime(*secs)),
                                11.0,
                            )
                            .color(style::MUTED()),
                        )
                        .push(gap(10.0));
                }
                col = col.push(panel(p)).push(gap(20.0));
            }
            col.push(panel(column![
                caps("Watching", c(TEAL())),
                space().height(6),
                kv_rows(vec![
                    ("Movies", time(movie_secs), style::TEXT(), 14.0),
                    ("TV shows", time(ep_secs), style::TEXT(), 14.0),
                    (
                        "Movies & episodes started",
                        entries.len().to_string(),
                        style::TEXT(),
                        14.0
                    ),
                ]),
                gap(8.0),
                text("Counted from your watch history on this server.")
                    .size(12)
                    .color(style::MUTED()),
            ]))
            .into()
        };
        let mut page = column![
            header,
            gap(20.0),
            container(tiles).padding(Padding::default().left(m)),
            gap(20.0),
            columns(w, main, side),
        ];
        if self.client.token.is_some() {
            page = page
                .push(gap(30.0))
                .push(container(self.devices_section(inner)).padding(Padding::default().left(m)));
        }
        page.into()
    }

    /// One history entry: its picture with how far, the title and episode,
    /// when, and how much.
    #[allow(clippy::too_many_arguments)]
    fn history_row<'a>(
        &self,
        lib: &api::Library,
        item: &Item,
        ep: Option<&str>,
        sub: Option<String>,
        e: &Entry,
        w: f32,
        env: Env<'a>,
    ) -> Element<'a, Msg> {
        let key = format!("hist\u{1f}{}\u{1f}{}", item.id, ep.unwrap_or(""));
        let lit = self.hovered.as_deref() == Some(key.as_str());
        let still = ep
            .and_then(|id| lib.find_episode(id))
            .and_then(|(s, si, ei)| s.seasons[si].episodes[ei].still.clone());
        let art = still.or(item.backdrop.clone()).or(item.poster.clone());
        let f = if e.watched() { 1.0 } else { e.fraction() };
        let gold = c(GOLD());
        let thumb = stack![
            cards::art(env.images, art.as_deref(), &item.title, 107.0, 60.0, 4.0),
            container(
                container(
                    container(space())
                        .width(95.0 * f)
                        .height(3)
                        .style(move |_| container::Style {
                            background: Some(Background::Color(gold)),
                            border: iced::border::rounded(1.5),
                            ..container::Style::default()
                        }),
                )
                .width(95)
                .style(|_| container::Style {
                    background: Some(Background::Color(style::white(70))),
                    border: iced::border::rounded(1.5),
                    ..container::Style::default()
                }),
            )
            .height(60)
            .padding(Padding::default().left(6.0).bottom(4.0))
            .align_y(Vertical::Bottom),
        ];
        let sub =
            sub.unwrap_or_else(|| item.year.map_or("Movie".into(), |y| format!("Movie · {y}")));
        let state = if e.watched() {
            "Watched".to_owned()
        } else {
            format!("{:.0}%", f * 100.0)
        };
        let state_col = c(if e.watched() { TEAL() } else { GOLD() });
        let body = row![
            thumb,
            column![
                text(item.title.clone())
                    .font(BOLD)
                    .size(15)
                    .wrapping(text::Wrapping::None),
                text(sub)
                    .size(12.5)
                    .color(style::MUTED())
                    .wrapping(text::Wrapping::None),
            ]
            .spacing(6)
            .width(Length::Fill),
            column![
                mono(&ago(e.updated), 11.0).color(style::MUTED()),
                mono(&state, 11.0).color(state_col),
            ]
            .spacing(8)
            .align_x(iced::alignment::Horizontal::Right),
        ]
        .spacing(16)
        .align_y(Vertical::Center);
        let open = match ep {
            Some(ep) => Msg::Nav(View::Episode(item.id.clone(), ep.to_owned())),
            None => Msg::Open(item.id.clone()),
        };
        hover_area(
            container(body)
                .width(w)
                .height(72)
                .padding(Padding {
                    top: 6.0,
                    right: 10.0,
                    bottom: 6.0,
                    left: 6.0,
                })
                .clip(true)
                .style(move |_| container::Style {
                    background: lit.then(|| Background::Color(style::white(8))),
                    border: iced::border::rounded(8),
                    ..container::Style::default()
                }),
            key,
            open,
        )
    }

    /// "Your devices": each session, signing any of them out.
    fn devices_section<'a>(&self, w: f32) -> Element<'a, Msg> {
        let d = &self.devices;
        let busy = d.op.is_some();
        let list = d.list.data.clone().flatten();
        let mut col: Column<'a, Msg> = column![section_title(
            "Security",
            "Your devices",
            list.as_ref().map_or(0, Vec::len)
        )]
        .width(w);
        if let Some((msg, ok)) = &d.notice {
            let color = if *ok {
                c(crate::ui::theme::OK())
            } else {
                style::ERROR()
            };
            col = col
                .push(text(msg.clone()).size(14.5).color(color))
                .push(gap(8.0));
        }
        if let Some(e) = &d.list.error {
            col = col.push(
                text(format!("Couldn't load your devices: {e}"))
                    .size(14.5)
                    .color(style::ERROR()),
            );
        }
        match (&d.list.data, list) {
            (Some(None), _) => {
                col = col
                    .push(
                        text(format!(
                            "{} doesn't list where you're signed in. Its admin can add it; \
                             until then you can only sign out here.",
                            self.client.info.name
                        ))
                        .size(14.5)
                        .color(style::MUTED()),
                    )
                    .push(gap(6.0))
                    .push(link(
                        "Setup guide for server admins",
                        14.5,
                        style::TEXT(),
                        Some((crate::iced_ui::icon::Icon::External, true)),
                        Msg::OpenUrl(api::SESSIONS_DOCS.to_owned()),
                    ));
            }
            (_, Some(mut list)) => {
                // this device first, then the most recently used
                list.sort_by_key(|s| (!s.current, std::cmp::Reverse(s.last_used.or(s.created))));
                for s in &list {
                    col = col.push(device_row(s, busy)).push(gap(8.0));
                }
                if list.iter().filter(|s| !s.current).count() > 1 {
                    col = col.push(gap(4.0)).push(widgets::button_with(
                        Kind::Small,
                        "Sign out all other devices",
                        None,
                        (!busy).then_some(Msg::EndSession(None)),
                    ));
                }
            }
            _ => {
                if d.list.loading() {
                    col = col.push(crate::iced_ui::icon::spinner(
                        super::spin_secs(),
                        style::MUTED(),
                        18.0,
                    ));
                }
            }
        }
        col.into()
    }

    // ---------------------------------------------------------- popups

    /// The password prompt or the new-arrivals dialog when one is up and
    /// nothing else is: its content, width, and what closing it sends.
    pub fn popup<'a>(&'a self, env: Env<'a>) -> Option<(Element<'a, Msg>, f32, Msg)> {
        if !self.quiet {
            return None;
        }
        if self.password.open {
            return Some((self.password_view(), 420.0, Msg::Password(PwMsg::Close)));
        }
        if self.arrivals.open {
            let lib = self.lib.as_ref()?;
            return Some((self.arrivals_view(lib, env), 640.0, Msg::CloseArrivals));
        }
        None
    }

    /// What Escape sends while a popup is up.
    pub fn popup_close(&self) -> Option<Msg> {
        if !self.quiet {
            return None;
        }
        if self.password.open {
            Some(Msg::Password(PwMsg::Close))
        } else if self.arrivals.open {
            Some(Msg::CloseArrivals)
        } else {
            None
        }
    }

    fn password_view<'a>(&self) -> Element<'a, Msg> {
        let p = &self.password;
        let pw = |m: PwMsg| Msg::Password(m);
        if p.changed {
            return column![
                caps("Done", c(crate::ui::theme::OK())),
                gap(4.0),
                text("Password changed").font(BOLD).size(24),
                gap(6.0),
                text("Use it the next time you sign in. Your other devices were signed out.")
                    .size(14.5)
                    .color(style::MUTED()),
                gap(16.0),
                widgets::button_with(Kind::Primary, "Close", None, Some(pw(PwMsg::Close))),
            ]
            .into();
        }
        let field = |id: &'static str, v: &str, on: fn(String) -> PwMsg, submit: Option<PwMsg>| {
            let mut f = text_input("", v)
                .id(id)
                .secure(true)
                .size(15)
                .padding([9, 10])
                .on_input(move |s| Msg::Password(on(s)))
                .style(|_, s| {
                    let p = crate::ui::theme::pal();
                    text_input::Style {
                        background: Background::Color(c(p.field)),
                        border: Border {
                            color: match s {
                                text_input::Status::Focused { .. } => c(p.text),
                                text_input::Status::Hovered => c(p.line_hi),
                                _ => c(p.line),
                            },
                            width: 1.0,
                            radius: 3.0.into(),
                        },
                        icon: c(p.muted),
                        placeholder: c(p.muted),
                        value: c(p.text),
                        selection: c(p.accent).scale_alpha(0.4),
                    }
                });
            if let Some(m) = submit {
                f = f.on_submit(Msg::Password(m));
            }
            f
        };
        let mut col: Column<'a, Msg> = column![
            caps("Your account", c(TEAL())),
            gap(4.0),
            text("Change your password").font(BOLD).size(24),
            gap(6.0),
            text(format!(
                "An admin of {} asked you to pick a new password. \
                 Until you do, this shows each time you open Valo.",
                self.client.info.name
            ))
            .size(14.5)
            .color(style::MUTED()),
            gap(14.0),
            widgets::label("Current password", 11.0),
            space().height(8),
            field(PW_FIELDS[0], &p.current, PwMsg::Current, None),
            gap(4.0),
            widgets::label("New password", 11.0),
            space().height(8),
            field(PW_FIELDS[1], &p.new, PwMsg::New, None),
            gap(4.0),
            widgets::label("New password again", 11.0),
            space().height(8),
            field(PW_FIELDS[2], &p.again, PwMsg::Again, Some(PwMsg::Submit)),
        ];
        if !p.again.is_empty() && p.new != p.again {
            col = col.push(space().height(8)).push(
                text("The new passwords don't match.")
                    .size(12)
                    .color(style::ERROR()),
            );
        }
        if let Some(e) = &p.error {
            col = col
                .push(gap(4.0))
                .push(text(e.clone()).size(14.5).color(style::ERROR()));
        }
        let busy = p.job.is_some();
        let ok = !busy && !p.current.is_empty() && !p.new.is_empty() && p.new == p.again;
        let mut buttons = row![
            widgets::button_with(
                Kind::Primary,
                "Change password",
                None,
                ok.then(|| pw(PwMsg::Submit))
            ),
            widgets::button_with(Kind::Small, "Later", None, Some(pw(PwMsg::Close))),
        ]
        .spacing(10)
        .align_y(Vertical::Center);
        if busy {
            buttons = buttons.push(crate::iced_ui::icon::spinner(
                super::spin_secs(),
                style::MUTED(),
                16.0,
            ));
        }
        col.push(gap(14.0)).push(buttons).into()
    }

    fn arrivals_view<'a>(&self, lib: &'a api::Library, env: Env<'a>) -> Element<'a, Msg> {
        let shown = self.arrivals.shown(lib, &self.client.info.name);
        let mut col: Column<'a, Msg> = column![
            dialogs::header(&shown.title, &shown.summary),
            space().height(8)
        ];
        let w = 640.0;
        let cols = ((w + GAP) / (140.0 + GAP)).floor().clamp(1.0, 4.0) as usize;
        let tw = (w - GAP * (cols as f32 - 1.0)) / cols as f32;
        let items: Vec<_> = shown.items.iter().take(SHOWN).collect();
        for chunk in items.chunks(cols) {
            let mut r = row![].spacing(GAP);
            for (item, sub) in chunk {
                r = r.push(self.poster_tile(item, Some(sub.clone()), tw, env));
            }
            col = col.push(r).push(gap(12.0));
        }
        if shown.items.len() > SHOWN {
            col = col
                .push(
                    text(format!("And {} more.", shown.items.len() - SHOWN))
                        .size(14.5)
                        .color(style::MUTED()),
                )
                .push(gap(12.0));
        }
        if !shown.gone.is_empty() {
            if !shown.items.is_empty() {
                col = col
                    .push(text("No longer on the server").font(BOLD).size(14.5))
                    .push(gap(4.0));
            }
            for n in shown.gone.iter().take(SHOWN) {
                col = col.push(text((*n).to_owned()).size(14.5).color(style::MUTED()));
            }
            if shown.gone.len() > SHOWN {
                col = col.push(
                    text(format!("And {} more.", shown.gone.len() - SHOWN))
                        .size(14.5)
                        .color(style::MUTED()),
                );
            }
            col = col.push(gap(12.0));
        }
        col.push(widgets::button_wide(
            Kind::Primary,
            "Close",
            40.0,
            Some(Msg::CloseArrivals),
        ))
        .into()
    }
}

/// One session: a monitor, the device and when, and Sign out.
fn device_row<'a>(s: &DeviceSession, busy: bool) -> Element<'a, Msg> {
    let col = c(if s.current {
        TEAL()
    } else {
        crate::ui::theme::MUTED()
    });
    let name = s.device.clone().unwrap_or_else(|| "Unknown device".into());
    let mut title = row![text(name).size(15)]
        .spacing(10)
        .align_y(Vertical::Center);
    if s.current {
        title = title.push(mono("This device", 10.0).color(c(TEAL())));
    }
    let mut parts = Vec::new();
    if let Some(t) = s.last_used {
        parts.push(format!("active {}", ago(t)));
    }
    if let Some(t) = s.created {
        parts.push(format!("signed in {}", ago(t)));
    }
    if let Some(ip) = &s.ip {
        parts.push(ip.clone());
    }
    let label = if s.current {
        "Sign out"
    } else {
        "Sign out device"
    };
    container(
        row![
            canvas::Canvas::new(Monitor { color: col })
                .width(28)
                .height(28),
            column![
                title,
                mono(&parts.join("   ·   "), 10.0).color(style::MUTED())
            ]
            .spacing(2),
            space::horizontal(),
            widgets::button_with(
                Kind::Small,
                label,
                None,
                (!busy).then(|| Msg::EndSession(Some(s.id.clone())))
            ),
        ]
        .spacing(16)
        .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .padding([12, 14])
    .style(|_| container::Style {
        border: Border {
            color: c(LINE()),
            width: 1.0,
            radius: 8.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

/// A small monitor outline (egui's `devices::screen`).
struct Monitor {
    color: Color,
}

impl<M> canvas::Program<M> for Monitor {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        let stroke = Stroke::default().with_width(1.5).with_color(self.color);
        let cx = bounds.width / 2.0;
        let cy = bounds.height / 2.0 - 3.0;
        let body = Path::rounded_rectangle(
            iced::Point::new(cx - 12.0 + 0.75, cy - 7.5 + 0.75),
            Size::new(22.5, 13.5),
            2.0.into(),
        );
        f.stroke(&body, stroke);
        let bottom = cy + 7.5;
        f.stroke(
            &Path::line(
                iced::Point::new(cx - 6.0, bottom + 5.0),
                iced::Point::new(cx + 6.0, bottom + 5.0),
            ),
            stroke,
        );
        f.stroke(
            &Path::line(
                iced::Point::new(cx, bottom),
                iced::Point::new(cx, bottom + 5.0),
            ),
            stroke,
        );
        vec![f.into_geometry()]
    }
}
