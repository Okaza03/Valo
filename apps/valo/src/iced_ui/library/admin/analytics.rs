//! Admin › Analytics: plays per day, the most watched titles and each
//! user's watching, from `GET /api/admin/analytics`. Clicking a user (here
//! or under Users) opens their page, from `GET /api/admin/analytics/users/<name>`.
use iced::alignment::{Horizontal, Vertical};
use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::widget::{column, container, mouse_area, row, space, stack, text, Column};
use iced::{Element, Length, Padding, Point, Rectangle, Renderer, Size, Theme};

use super::super::details::{caps, column_of, gap, kv_rows, link, panel};
use super::super::profile::{bar, columns, section_title, stats, Remote};
use super::super::{avatar, cards, fill_gradient, spin_secs, Msg as LMsg, View};
use super::{needs_note, not_set_up, Asks, Cx, Msg as AMsg, Section};
use crate::api::{Analytics, Client, DayStat, RecentPlay, UserAnalytics, UserStat};
use crate::iced_ui::spaced::mono;
use crate::iced_ui::style::{self, c, BOLD};
use crate::iced_ui::widgets;
use crate::ui::browse::details::{GOLD, TEAL};
use crate::ui::browse::people::ago;
use crate::ui::theme::{BG, LINE};
use crate::util::{color_for, fmt_runtime};

#[derive(Clone, Debug)]
pub enum Msg {
    Days(u32),
    /// Which kind the top lists show: None for both.
    Kind(Option<&'static str>),
    /// Open a user's page.
    Open(String),
    /// Back from a user's page.
    Back,
}

pub struct State {
    days: u32,
    data: Remote<Analytics>,
    /// The range `data` is for.
    loaded: u32,
    kind: Option<&'static str>,
    /// The user whose page is open.
    user: Option<String>,
    person: Remote<UserAnalytics>,
    /// The user and range `person` is for.
    person_for: Option<(String, u32)>,
    /// The open user's page came from this section (Users), so Back goes there.
    from: Option<Section>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            days: 30,
            data: Remote::default(),
            loaded: 30,
            kind: None,
            user: None,
            person: Remote::default(),
            person_for: None,
            from: None,
        }
    }
}

/// What a click on the page asks of the admin page.
pub enum Out {
    None,
    /// Go to this section.
    Goto(Section),
}

impl State {
    pub fn refresh(&mut self) {
        self.data.retry();
        self.person.retry();
    }

    pub fn missing(&self) -> bool {
        self.data.missing
    }

    pub fn busy(&self) -> bool {
        self.data.loading() || self.person.loading()
    }

    /// Open a user's page, from section `from`.
    pub fn open(&mut self, name: String, from: Section) {
        self.user = Some(name);
        self.from = (from != Section::Analytics).then_some(from);
    }

    pub fn work(&mut self, client: &Client, asks: &mut Asks) {
        self.data.poll(&mut asks.login);
        self.person.poll(&mut asks.login);
        if self.days != self.loaded && !self.data.loading() {
            self.data.retry();
            self.data.data = None;
        }
        if let Some(name) = self.user.clone() {
            let want = (name, self.days);
            if self.person_for.as_ref() != Some(&want) && !self.person.loading() {
                self.person.retry();
                self.person.data = None;
            }
            if self.person.idle() || (self.person.data.is_some() && self.person.stale(300)) {
                let c = client.clone();
                let (n, days) = want.clone();
                self.person_for = Some(want);
                self.person.load(move || c.admin_user_analytics(&n, days));
            }
            return;
        }
        if self.data.idle() || (self.data.data.is_some() && self.data.stale(300)) {
            let (c, days) = (client.clone(), self.days);
            self.loaded = days;
            self.data.load(move || c.admin_analytics(days));
        }
    }

    pub fn update(&mut self, m: Msg) -> Out {
        match m {
            Msg::Days(d) => self.days = d,
            Msg::Kind(k) => self.kind = k,
            Msg::Open(name) => self.open(name, Section::Analytics),
            Msg::Back => {
                self.user = None;
                if let Some(s) = self.from.take() {
                    return Out::Goto(s);
                }
            }
        }
        Out::None
    }

    pub fn view<'a>(&'a self, cx: &Cx<'a>, w: f32) -> Element<'a, LMsg> {
        if let Some(name) = &self.user {
            return self.user_view(cx, name, w);
        }
        let (m, inner) = column_of(w);
        let pad = |e: Element<'a, LMsg>| -> Element<'a, LMsg> {
            container(e).padding(Padding::default().left(m)).into()
        };
        if self.data.missing {
            return pad(not_set_up(Section::Analytics, &cx.client.info.name));
        }
        let mut top: Column<'a, LMsg> = column![range(self.days, self.data.loading()), gap(18.0)];
        if let Some(e) = &self.data.error {
            top = top.push(
                text(format!("Couldn't load analytics: {e}"))
                    .size(14.5)
                    .color(style::ERROR()),
            );
        }
        let Some(a) = &self.data.data else {
            return pad(top.into());
        };
        top = top
            .push(stats(
                vec![
                    (a.plays.to_string(), "Plays"),
                    (format!("{:.0}", a.hours), "Hours watched"),
                    (a.active_users.to_string(), "Active users"),
                    (
                        if a.plays > 0 {
                            format!("{:.1}", a.hours / a.plays as f64)
                        } else {
                            "—".into()
                        },
                        "Hours per play",
                    ),
                ],
                inner,
            ))
            .push(gap(6.0))
            .push(panel(column![
                caps("Plays per day", c(TEAL())),
                gap(12.0),
                day_chart(&a.days),
            ]))
            .push(gap(24.0))
            .width(inner);

        let kind = self.kind;
        let fits =
            |k: &Option<String>, want: Option<&str>| want.is_none_or(|w| k.as_deref() == Some(w));
        let everyone = with_accounts(a, cx.accounts);
        let body = columns(
            w,
            |lw| {
                let send = |k| LMsg::Admin(AMsg::Analytics(Msg::Kind(k)));
                let mut col: Column<'a, LMsg> = column![
                    caps("Most watched", c(TEAL())),
                    gap(4.0),
                    row![
                        text("Top titles").font(BOLD).size(22),
                        space::horizontal(),
                        widgets::segmented(
                            [
                                (None, "Both"),
                                (Some("movie"), "Movies"),
                                (Some("series"), "Series"),
                            ]
                            .into_iter()
                            .map(|(k, l)| (l.to_owned(), None, kind == k, send(k)))
                            .collect()
                        ),
                    ]
                    .align_y(Vertical::Center),
                    gap(12.0),
                ];
                let top: Vec<_> = a.top.iter().filter(|t| fits(&t.kind, kind)).collect();
                if top.is_empty() {
                    col = col.push(
                        text("Nothing watched in this range.")
                            .size(14.5)
                            .color(style::MUTED()),
                    );
                }
                let max = top.iter().map(|t| t.plays).max().unwrap_or(1).max(1);
                for t in top.iter().take(15) {
                    let mut value = format!(
                        "{} play{}   ·   {:.1} h",
                        t.plays,
                        if t.plays == 1 { "" } else { "s" },
                        t.hours
                    );
                    if let Some(u) = t.users {
                        value.push_str(&format!(
                            "   ·   {u} viewer{}",
                            if u == 1 { "" } else { "s" }
                        ));
                    }
                    col = col
                        .push(bar(&t.title, value, t.plays as f32 / max as f32, lw))
                        .push(gap(4.0));
                }
                let rated: Vec<_> = a.top_rated.iter().filter(|t| fits(&t.kind, kind)).collect();
                if !rated.is_empty() {
                    col = col
                        .push(gap(20.0))
                        .push(caps("Users' ratings", c(TEAL())))
                        .push(gap(4.0))
                        .push(text("Top rated").font(BOLD).size(22))
                        .push(gap(12.0));
                    for t in rated.iter().take(15) {
                        let value = format!(
                            "{:.1} / 10   ·   {} rating{}",
                            t.avg_rating,
                            t.rating_count,
                            if t.rating_count == 1 { "" } else { "s" }
                        );
                        col = col
                            .push(bar(&t.title, value, t.avg_rating / 10.0, lw))
                            .push(gap(4.0));
                    }
                }
                col.into()
            },
            |_| {
                let mut col: Column<'a, LMsg> = column![caps("By user", c(TEAL())), gap(10.0)];
                if everyone.is_empty() {
                    col = col.push(
                        text("No one watched in this range.")
                            .size(14.5)
                            .color(style::MUTED()),
                    );
                }
                for (u, watched) in &everyone {
                    let mut line = format!("{} plays · {:.1} h", u.plays, u.hours);
                    if let (Some(m), Some(e)) = (u.movies, u.episodes) {
                        line = format!("{m} movies · {e} episodes · {:.1} h", u.hours);
                    }
                    match watched {
                        Some(true) => {}
                        Some(false) => line = "Nothing watched in this range".into(),
                        // the server listed only some: don't claim they watched nothing
                        None => line = "Not in the server's list".into(),
                    }
                    if let Some(t) = u.last_active {
                        line.push_str(&format!(" · {}", ago(t)));
                    }
                    let who = row![
                        avatar(&u.name, 13.0),
                        column![
                            text(u.name.clone()).size(14.5),
                            mono(&line, 10.0).color(style::MUTED()),
                        ]
                        .spacing(1),
                    ]
                    .spacing(10)
                    .align_y(Vertical::Center);
                    col = col
                        .push(crate::iced_ui::chrome::tip_below(
                            mouse_area(who)
                                .on_press(LMsg::Admin(AMsg::Analytics(Msg::Open(u.name.clone()))))
                                .interaction(mouse::Interaction::Pointer),
                            &format!("Open {}'s analytics", u.name),
                        ))
                        .push(gap(8.0));
                }
                panel(col)
            },
        );
        column![pad(top.into()), body].into()
    }

    /// One user's page: who they are, how much, when and what they watch.
    fn user_view<'a>(&'a self, cx: &Cx<'a>, name: &str, w: f32) -> Element<'a, LMsg> {
        let (m, inner) = column_of(w);
        let pad = |e: Element<'a, LMsg>| -> Element<'a, LMsg> {
            container(e).padding(Padding::default().left(m)).into()
        };
        let back_to = if self.from == Some(Section::Users) {
            "‹  Users"
        } else {
            "‹  All users"
        };
        let a = self.person.data.as_ref();
        let mut out: Column<'a, LMsg> = column![
            pad(link(
                back_to,
                14.0,
                style::MUTED(),
                None,
                LMsg::Admin(AMsg::Analytics(Msg::Back))
            )),
            gap(8.0),
            hero(name, a, w),
            gap(20.0),
        ];
        let mut top: Column<'a, LMsg> =
            column![range(self.days, self.person.loading()), gap(18.0)].width(inner);
        if self.person.missing {
            top = top.push(panel(column![
                caps("Not set up", c(TEAL())),
                gap(6.0),
                text(format!(
                    "{} can't show one user's analytics yet.",
                    cx.client.info.name
                ))
                .size(14.5)
                .color(style::MUTED()),
                gap(6.0),
                needs_note(
                    "It needs GET /api/admin/analytics/users/<name>.",
                    Section::Analytics
                ),
            ]));
        }
        if let Some(e) = &self.person.error {
            top = top.push(
                text(format!("Couldn't load {name}'s analytics: {e}"))
                    .size(14.5)
                    .color(style::ERROR()),
            );
        }
        let Some(a) = a else {
            return out.push(pad(top.into())).into();
        };
        let secs = a.hours * 3600.0;
        let n = |v: Option<u32>| v.map_or("—".into(), |v| v.to_string());
        top = top
            .push(stats(
                vec![
                    (
                        if secs > 0.0 {
                            fmt_runtime(secs)
                        } else {
                            "—".into()
                        },
                        "Time watched",
                    ),
                    (a.plays.to_string(), "Plays"),
                    (n(a.movies), "Movies"),
                    (n(a.episodes), "Episodes"),
                    (n(a.finished), "Finished"),
                    (n(a.in_progress), "Unfinished"),
                ],
                inner,
            ))
            .push(gap(6.0))
            .push(panel(column![
                caps("Plays per day", c(TEAL())),
                gap(12.0),
                day_chart(&a.days),
            ]))
            .push(gap(24.0));
        out = out.push(pad(top.into()));

        let body = columns(
            w,
            |lw| {
                let mut col: Column<'a, LMsg> =
                    column![section_title("History", "Recently watched", a.recent.len())];
                if a.recent.is_empty() {
                    col = col.push(
                        text("Nothing watched in this range.")
                            .size(14.5)
                            .color(style::MUTED()),
                    );
                }
                for r in &a.recent {
                    col = col.push(recent_row(cx, r, lw)).push(gap(4.0));
                }
                if !a.top.is_empty() {
                    col = col.push(gap(24.0)).push(section_title(
                        "Most watched",
                        "Top titles",
                        a.top.len(),
                    ));
                    let max = a.top.iter().map(|t| t.hours).fold(0.0, f64::max).max(0.1);
                    for t in &a.top {
                        let value = format!(
                            "{} play{}   ·   {:.1} h",
                            t.plays,
                            if t.plays == 1 { "" } else { "s" },
                            t.hours
                        );
                        let b = bar(&t.title, value, (t.hours / max) as f32, lw);
                        col = col.push(if t.id.is_empty() {
                            b
                        } else {
                            mouse_area(b)
                                .on_press(LMsg::Nav(View::Details(t.id.clone())))
                                .interaction(mouse::Interaction::Pointer)
                                .into()
                        });
                        col = col.push(gap(4.0));
                    }
                }
                col.into()
            },
            |rw| {
                let mut col: Column<'a, LMsg> = column![];
                let hours: u32 = a.hours_of_day.iter().sum();
                let week: u32 = a.weekdays.iter().sum();
                let inner_w = rw - 36.0;
                if hours > 0 || week > 0 {
                    let mut p: Column<'a, LMsg> = column![caps("When they watch", c(TEAL()))];
                    if hours > 0 {
                        p = p.push(gap(12.0)).push(hour_chart(&a.hours_of_day));
                    }
                    if week > 0 {
                        p = p.push(gap(14.0));
                        let max = a.weekdays.iter().copied().max().unwrap_or(1).max(1);
                        for (d, v) in [
                            "Monday",
                            "Tuesday",
                            "Wednesday",
                            "Thursday",
                            "Friday",
                            "Saturday",
                            "Sunday",
                        ]
                        .iter()
                        .zip(&a.weekdays)
                        {
                            p = p
                                .push(bar(
                                    d,
                                    format!("{v} play{}", if *v == 1 { "" } else { "s" }),
                                    *v as f32 / max as f32,
                                    inner_w,
                                ))
                                .push(gap(2.0));
                        }
                    }
                    col = col.push(panel(p)).push(gap(20.0));
                }
                if !a.genres.is_empty() {
                    let mut p: Column<'a, LMsg> =
                        column![caps("Favorite genres", c(TEAL())), gap(12.0)];
                    let max = a.genres[0].hours.max(0.1);
                    for g in a.genres.iter().take(8) {
                        p = p
                            .push(bar(
                                &g.name,
                                fmt_runtime(g.hours * 3600.0),
                                (g.hours / max) as f32,
                                inner_w,
                            ))
                            .push(gap(6.0));
                    }
                    col = col.push(panel(p)).push(gap(20.0));
                }
                let mut rows = vec![
                    ("Username", a.name.clone(), style::TEXT(), 14.0),
                    (
                        "Role",
                        if a.admin { "Admin" } else { "User" }.to_owned(),
                        style::TEXT(),
                        14.0,
                    ),
                    (
                        "Signed in on",
                        a.sessions.len().to_string(),
                        style::TEXT(),
                        14.0,
                    ),
                ];
                if let Some(t) = a.last_active {
                    rows.push(("Last watched", ago(t), style::TEXT(), 14.0));
                }
                if a.plays > 0 {
                    rows.push((
                        "Average play",
                        fmt_runtime(secs / a.plays as f64),
                        style::TEXT(),
                        14.0,
                    ));
                }
                col = col.push(panel(column![
                    caps("Account", c(TEAL())),
                    gap(6.0),
                    kv_rows(rows),
                    gap(8.0),
                    link(
                        "Manage in Users",
                        13.0,
                        c(GOLD()),
                        None,
                        LMsg::Admin(AMsg::Section(Section::Users))
                    ),
                ]));
                if !a.sessions.is_empty() {
                    let mut p: Column<'a, LMsg> = column![caps("Devices", c(TEAL())), gap(10.0)];
                    for s in &a.sessions {
                        let mut line = Vec::new();
                        if let Some(ip) = &s.ip {
                            line.push(ip.clone());
                        }
                        if let Some(t) = s.last_used {
                            line.push(format!("used {}", ago(t)));
                        }
                        if let Some(t) = s.created {
                            line.push(format!("signed in {}", ago(t)));
                        }
                        p = p
                            .push(
                                text(s.device.clone().unwrap_or_else(|| "Unknown device".into()))
                                    .size(14.5),
                            )
                            .push(mono(&line.join(" · "), 10.0).color(style::MUTED()))
                            .push(gap(8.0));
                    }
                    col = col.push(gap(20.0)).push(panel(p));
                }
                col.into()
            },
        );
        out.push(body).into()
    }
}

/// Everyone under By user: who watched, most hours first, then the accounts
/// that aren't in the analytics, A to Z. With each, whether they watched in
/// the range: `None` when the server's list looks cut short (fewer users than
/// `active_users`), so it can't be told.
fn with_accounts(
    a: &Analytics,
    accounts: &[crate::api::AdminUser],
) -> Vec<(UserStat, Option<bool>)> {
    let mut out: Vec<_> = a.users.iter().map(|u| (u.clone(), Some(true))).collect();
    out.sort_by(|x, y| y.0.hours.total_cmp(&x.0.hours));
    let complete = a.users.len() as u32 >= a.active_users;
    let mut rest: Vec<_> = accounts
        .iter()
        .filter(|acc| {
            !a.users
                .iter()
                .any(|u| u.name.eq_ignore_ascii_case(&acc.name))
        })
        .map(|acc| {
            let u = UserStat {
                name: acc.name.clone(),
                last_active: acc.last_active,
                ..Default::default()
            };
            (u, complete.then_some(false))
        })
        .collect();
    rest.sort_by_key(|(u, _)| u.name.to_lowercase());
    out.extend(rest);
    out
}

/// The 7 days … Year picker, with a spinner while loading.
fn range<'a>(days: u32, loading: bool) -> Element<'a, LMsg> {
    let send = |d| LMsg::Admin(AMsg::Analytics(Msg::Days(d)));
    let mut r = row![widgets::segmented(
        [
            (7, "7 days"),
            (30, "30 days"),
            (90, "90 days"),
            (365, "Year")
        ]
        .into_iter()
        .map(|(d, l)| (l.to_owned(), None, days == d, send(d)))
        .collect()
    )]
    .spacing(10)
    .align_y(Vertical::Center);
    if loading {
        r = r.push(crate::iced_ui::icon::spinner(
            spin_secs(),
            style::MUTED(),
            16.0,
        ));
    }
    r.into()
}

/// The user's header, like their own profile page.
fn hero<'a>(name: &str, a: Option<&UserAnalytics>, w: f32) -> Element<'a, LMsg> {
    let (m, _) = column_of(w);
    let col = c(color_for(name));
    let shown = a
        .and_then(|a| a.display_name.as_deref())
        .filter(|d| !d.trim().is_empty())
        .unwrap_or(name);
    let mut line = vec![name.to_owned()];
    if a.is_some_and(|a| a.admin) {
        line.push("Admin".into());
    }
    if let Some(t) = a.and_then(|a| a.last_active) {
        line.push(format!("last watched {}", ago(t)));
    }
    let content = row![
        avatar(name, 50.0),
        column![
            caps("User analytics", c(TEAL())),
            gap(4.0),
            text(shown.to_owned())
                .font(style::BLACK)
                .size(40)
                .color(style::TEXT()),
            gap(6.0),
            mono(&line.join("   ·   "), 11.5).color(style::MUTED()),
        ],
    ]
    .spacing(28)
    .align_y(Vertical::Center);
    stack![
        container(space())
            .width(Length::Fill)
            .height(200)
            .style(move |_| fill_gradient(0.0, col.scale_alpha(0.35), c(BG()))),
        container(content)
            .height(200)
            .center_y(200)
            .padding(Padding::default().left(m)),
    ]
    .into()
}

/// One play in the history list; opens the title when clicked.
fn recent_row<'a>(cx: &Cx<'a>, e: &RecentPlay, w: f32) -> Element<'a, LMsg> {
    let owner = if e.series_id.is_empty() {
        &e.id
    } else {
        &e.series_id
    };
    let item = cx.find(owner);
    let still = cx
        .lib
        .find_episode(&e.id)
        .and_then(|(s, si, ei)| s.seasons[si].episodes[ei].still.as_deref());
    let art = still
        .or(item.and_then(|i| i.backdrop.as_deref()))
        .or(item.and_then(|i| i.poster.as_deref()));
    let f = if e.completed || e.duration <= 0.0 {
        1.0
    } else {
        (e.position / e.duration).clamp(0.0, 1.0) as f32
    };
    let (state, col) = if e.completed {
        ("Watched".to_owned(), TEAL())
    } else {
        (
            format!("{:.0}%  ·  {}", f * 100.0, fmt_runtime(e.position)),
            GOLD(),
        )
    };
    let thumb = stack![
        cards::art(cx.images, art, &e.title, 93.0, 52.0, 4.0),
        container(progress(f, 81.0))
            .width(93)
            .height(52)
            .align_y(Vertical::Bottom)
            .padding(Padding::default().bottom(4.0).left(6.0)),
    ];
    let line = row![
        thumb,
        column![
            text(e.title.clone())
                .font(BOLD)
                .size(15)
                .wrapping(text::Wrapping::None),
            text(e.episode.clone().unwrap_or_else(|| "Movie".into()))
                .size(12.5)
                .color(style::MUTED()),
        ]
        .spacing(4)
        .width(Length::Fill),
        column![
            mono(&ago(e.updated), 11.0).color(style::MUTED()),
            mono(&state, 11.0).color(c(col)),
        ]
        .spacing(8)
        .align_x(Horizontal::Right),
    ]
    .spacing(16)
    .align_y(Vertical::Center)
    .padding(6)
    .width(w);
    if item.is_none() {
        return line.into();
    }
    let to = if e.series_id.is_empty() {
        View::Details(e.id.clone())
    } else {
        View::Episode(e.series_id.clone(), e.id.clone())
    };
    mouse_area(line)
        .on_press(LMsg::Nav(to))
        .interaction(mouse::Interaction::Pointer)
        .into()
}

/// A thin gold progress line over a white track.
fn progress<'a>(f: f32, w: f32) -> Element<'a, LMsg> {
    let gold = c(GOLD());
    container(
        container(space())
            .width(w * f)
            .height(3)
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(gold)),
                border: iced::border::rounded(1.5),
                ..container::Style::default()
            }),
    )
    .width(w)
    .height(3)
    .style(|_| container::Style {
        background: Some(iced::Background::Color(style::white(70))),
        border: iced::border::rounded(1.5),
        ..container::Style::default()
    })
    .into()
}

/// Plays per day: one bar per day, the first and last day under the axis.
fn day_chart<'a>(days: &[DayStat]) -> Element<'a, LMsg> {
    let max = days.iter().map(|d| d.plays).max().unwrap_or(0).max(1);
    let chart = Bars {
        values: days.iter().map(|d| d.plays as f32).collect(),
        tips: days
            .iter()
            .map(|d| format!("{}\n{} plays, {:.1} hours", d.day, d.plays, d.hours))
            .collect(),
        labels: [days.first(), days.last()]
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(i, d)| (if i == 0 { 0.0 } else { 1.0 }, d.day.clone()))
            .collect(),
        top_label: Some(max.to_string()),
        max_bar: 24.0,
        grid: true,
        empty: "No plays yet",
    };
    canvas::Canvas::new(chart)
        .width(Length::Fill)
        .height(160.0 + 22.0)
        .into()
}

/// Plays per hour of the day, 24 bars.
fn hour_chart<'a>(hours: &[u32]) -> Element<'a, LMsg> {
    let hours: Vec<u32> = hours.iter().copied().take(24).collect();
    let chart = Bars {
        values: hours.iter().map(|v| *v as f32).collect(),
        tips: hours
            .iter()
            .enumerate()
            .map(|(i, v)| {
                format!(
                    "{i:02}:00–{:02}:00\n{v} play{}",
                    (i + 1) % 24,
                    if *v == 1 { "" } else { "s" }
                )
            })
            .collect(),
        labels: [0, 6, 12, 18]
            .iter()
            .map(|i| ((*i as f32 + 0.5) / 24.0, format!("{i:02}")))
            .collect(),
        top_label: None,
        max_bar: 14.0,
        grid: false,
        empty: "",
    };
    canvas::Canvas::new(chart)
        .width(Length::Fill)
        .height(70.0 + 18.0)
        .into()
}

/// A bar chart: gold bars, the hovered one white with its numbers above.
struct Bars {
    values: Vec<f32>,
    tips: Vec<String>,
    /// Under the axis: (where across, 0 to 1; text).
    labels: Vec<(f32, String)>,
    /// The biggest value, over the top left corner.
    top_label: Option<String>,
    max_bar: f32,
    /// Lines at half and full height.
    grid: bool,
    empty: &'static str,
}

impl<M> canvas::Program<M> for Bars {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        let w = bounds.width;
        let label_h = if self.labels.is_empty() { 0.0 } else { 20.0 };
        let h = bounds.height - label_h;
        let muted = style::MUTED();
        let line = Stroke::default().with_width(1.0).with_color(c(LINE()));
        let small = |s: &str, at: Point, ax: Horizontal, ay: Vertical| Text {
            content: s.to_owned(),
            position: at,
            color: muted,
            size: 9.5.into(),
            font: iced::Font::MONOSPACE,
            align_x: ax.into(),
            align_y: ay,
            ..Text::default()
        };
        if self.values.is_empty() {
            f.fill_text(small(
                self.empty,
                Point::new(w / 2.0, h / 2.0),
                Horizontal::Center,
                Vertical::Center,
            ));
            return vec![f.into_geometry()];
        }
        let max = self.values.iter().copied().fold(0.0, f32::max).max(1.0);
        if self.grid {
            for k in [0.5, 1.0] {
                let y = h - h * k;
                f.stroke(&Path::line(Point::new(0.0, y), Point::new(w, y)), line);
            }
        }
        f.stroke(&Path::line(Point::new(0.0, h), Point::new(w, h)), line);
        if let Some(t) = &self.top_label {
            f.fill_text(small(
                t,
                Point::new(0.0, 0.0),
                Horizontal::Left,
                Vertical::Bottom,
            ));
        }
        let slot = w / self.values.len() as f32;
        let bw = (slot * 0.7).clamp(1.0, self.max_bar);
        let hover = cursor.position_in(bounds);
        let mut tip = None;
        for (i, v) in self.values.iter().enumerate() {
            let cx = slot * (i as f32 + 0.5);
            let hot = hover.is_some_and(|p| (p.x - cx).abs() <= slot / 2.0 && p.y <= h);
            if hot {
                f.fill_rectangle(
                    Point::new(cx - slot / 2.0, 0.0),
                    Size::new(slot, h),
                    style::white(8),
                );
                tip = self.tips.get(i).map(|t| (cx, t));
            }
            if *v > 0.0 {
                let bh = h * v / max;
                f.fill(
                    &Path::rounded_rectangle(
                        Point::new(cx - bw / 2.0, h - bh),
                        Size::new(bw, bh),
                        2.0.into(),
                    ),
                    if hot { style::TEXT() } else { c(GOLD()) },
                );
            }
        }
        for (x, l) in &self.labels {
            let ax = if *x <= 0.0 {
                Horizontal::Left
            } else if *x >= 1.0 {
                Horizontal::Right
            } else {
                Horizontal::Center
            };
            f.fill_text(small(l, Point::new(w * x, h + 6.0), ax, Vertical::Top));
        }
        // the hovered bar's numbers, in a box near the top
        if let Some((cx, t)) = tip {
            let lines: Vec<&str> = t.lines().collect();
            let tw = lines.iter().map(|l| l.len()).max().unwrap_or(0) as f32 * 6.6 + 16.0;
            let th = lines.len() as f32 * 15.0 + 10.0;
            let x = (cx - tw / 2.0).clamp(0.0, (w - tw).max(0.0));
            f.fill(
                &Path::rounded_rectangle(Point::new(x, 2.0), Size::new(tw, th), 4.0.into()),
                c(crate::ui::theme::SURFACE()),
            );
            f.stroke(
                &Path::rounded_rectangle(Point::new(x, 2.0), Size::new(tw, th), 4.0.into()),
                Stroke::default()
                    .with_width(1.0)
                    .with_color(c(crate::ui::theme::LINE_HI())),
            );
            for (k, l) in lines.iter().enumerate() {
                f.fill_text(Text {
                    content: (*l).to_owned(),
                    position: Point::new(x + 8.0, 7.0 + k as f32 * 15.0),
                    color: style::TEXT(),
                    size: 11.5.into(),
                    ..Text::default()
                });
            }
        }
        vec![f.into_geometry()]
    }

    fn mouse_interaction(&self, _: &(), _: Rectangle, _: mouse::Cursor) -> mouse::Interaction {
        mouse::Interaction::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::AdminUser;

    fn account(name: &str) -> AdminUser {
        serde_json::from_value(serde_json::json!({ "name": name })).unwrap()
    }

    fn stat(name: &str, hours: f64) -> UserStat {
        UserStat {
            name: name.into(),
            hours,
            ..Default::default()
        }
    }

    #[test]
    fn lists_every_account() {
        let mut a = Analytics {
            active_users: 2,
            users: vec![stat("ann", 1.0), stat("Bob", 5.0)],
            ..Default::default()
        };
        let accounts = [account("zoe"), account("bob"), account("cat")];
        let names = |a: &Analytics| -> Vec<(String, Option<bool>)> {
            with_accounts(a, &accounts)
                .into_iter()
                .map(|(u, w)| (u.name, w))
                .collect()
        };
        assert_eq!(
            names(&a),
            [
                ("Bob".to_owned(), Some(true)),
                ("ann".to_owned(), Some(true)),
                ("cat".to_owned(), Some(false)),
                ("zoe".to_owned(), Some(false)),
            ]
        );
        a.active_users = 4;
        assert_eq!(names(&a)[2].1, None);
    }
}
