//! The Admin page, as egui draws it (`browse::admin`): a sidebar of
//! sections for managing the server, each talking to its own part of the
//! optional admin API; when the server doesn't have that part, the section
//! links to its setup guide instead.
mod dashboard;
mod moving;
mod requests;
mod users;

use anyhow::Result;
use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, space, text, Column};
use iced::{Background, Border, Color, Element, Length, Padding};

use super::details::{caps, column_of, gap, link, panel};
use super::profile::Remote;
use super::{spin_secs, Library as Lib, Msg as LMsg};
use crate::api::{self, AdminUser, Client, Item, Kind, Library};
use crate::iced_ui::icon::{self, Icon};
use crate::iced_ui::spaced::mono;
use crate::iced_ui::style::{self, c, BOLD};
use crate::iced_ui::widgets::{self, Kind as Btn};
use crate::iced_ui::Job;
use crate::ui::browse::details::TEAL;
use crate::ui::theme::{ERROR, INK, LINE, OK, PRIMARY, SURFACE, SURFACE_HI, WARN};

/// A change for the server, run off the UI thread.
pub type JobFn = Box<dyn FnOnce(&Client) -> Result<Done> + Send>;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Section {
    #[default]
    Dashboard,
    Analytics,
    Requests,
    Library,
    Collections,
    Moving,
    Addons,
    Layout,
    Users,
}

impl Section {
    const ALL: [Section; 9] = [
        Section::Dashboard,
        Section::Analytics,
        Section::Requests,
        Section::Library,
        Section::Collections,
        Section::Moving,
        Section::Addons,
        Section::Layout,
        Section::Users,
    ];

    fn label(self) -> &'static str {
        match self {
            Section::Dashboard => "Dashboard",
            Section::Analytics => "Analytics",
            Section::Requests => "Requests",
            Section::Library => "Library",
            Section::Collections => "Collections",
            Section::Moving => "Moving",
            Section::Addons => "Addons",
            Section::Layout => "Layout",
            Section::Users => "Users",
        }
    }

    fn about(self) -> &'static str {
        match self {
            Section::Dashboard => {
                "What's in the library, how the server is doing, syncs and the live log."
            }
            Section::Analytics => "What gets watched, when, and by whom.",
            Section::Requests => {
                "Titles people asked for through the Discord bot. Answers go back to them there."
            }
            Section::Library => {
                "Fix a title's details and artwork by hand, or fetch them from IMDb."
            }
            Section::Collections => {
                "Group titles like the Batman films, so A to Z keeps them together, oldest first."
            }
            Section::Moving => {
                "Mark titles whose files you're moving between disks. Nobody can play them \
                 until you untick them, and their posters say MOVING."
            }
            Section::Addons => {
                "Turn Stremio addons on for everyone: catalogs on the home page, missing \
                 details filled in, and subtitles in the player."
            }
            Section::Layout => {
                "How the app looks for everyone: which rows the home page shows, in what order."
            }
            Section::Users => "Who can sign in, and who can manage the server.",
        }
    }

    /// The setup guide on GitHub.
    fn docs(self) -> &'static str {
        match self {
            Section::Dashboard => api::ADMIN_DOCS_DASHBOARD,
            Section::Analytics => api::ADMIN_DOCS_ANALYTICS,
            Section::Requests => api::ADMIN_DOCS_REQUESTS,
            Section::Library => api::ADMIN_DOCS_LIBRARY,
            Section::Collections => api::ADMIN_DOCS_COLLECTIONS,
            Section::Moving => api::ADMIN_DOCS_MOVING,
            Section::Addons => api::ADMIN_DOCS_ADDONS,
            Section::Layout => api::ADMIN_DOCS_LAYOUT,
            Section::Users => api::ADMIN_API_DOCS,
        }
    }

    /// What the server is missing, for the "not set up" panel.
    fn needs(self) -> &'static str {
        match self {
            Section::Dashboard => {
                "the status, sync and log endpoints (/api/admin/status, /api/admin/sync/…, \
                 /api/admin/logs)"
            }
            Section::Analytics => "the analytics endpoint (/api/admin/analytics)",
            Section::Requests => {
                "a request queue (/api/admin/requests), usually filled by a Discord bot. \
                 The guide shows how to create the bot and connect it"
            }
            Section::Library => {
                "the endpoints for editing titles (/api/admin/movies/…, /api/admin/series/…)"
            }
            Section::Collections => "the collection endpoints (/api/admin/collections)",
            Section::Moving => "the moving endpoints (GET and PUT /api/admin/moving)",
            Section::Addons => "the addon endpoints (/api/addons, /api/admin/addons)",
            Section::Layout => "the layout endpoints (/api/layout, /api/admin/layout)",
            Section::Users => "Valo's admin API (/api/admin/users)",
        }
    }

    /// Ported to this build; the rest come in later stages.
    fn ported(self) -> bool {
        matches!(
            self,
            Section::Dashboard | Section::Requests | Section::Moving | Section::Users
        )
    }
}

/// A finished change.
#[derive(Default)]
pub struct Done {
    /// What to tell the admin.
    pub msg: String,
    /// Reload the library afterwards.
    pub library: bool,
    /// Load the home page layout again afterwards.
    pub layout: bool,
}

impl Done {
    pub fn msg(msg: impl Into<String>) -> Self {
        Self {
            msg: msg.into(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug)]
pub enum Msg {
    Section(Section),
    Refresh,
    /// Open or close a row's "⋮" menu (by key).
    Menu(Option<String>),
    /// Look for the section's endpoints again.
    CheckAgain(Section),
    Dash(dashboard::Msg),
    Requests(requests::Msg),
    Moving(moving::Msg),
    Users(users::Msg),
}

/// What the admin page asks the library to do.
#[derive(Default)]
pub struct Asks {
    pub login: bool,
    pub reload_library: bool,
    pub reload_layout: bool,
    /// Open this user's page under Analytics.
    pub profile: Option<String>,
}

#[derive(Default)]
pub struct State {
    section: Section,
    op: Option<Job<Result<Done>>>,
    /// The last change's outcome, and whether it worked.
    notice: Option<(String, bool)>,
    /// The last change failed because the server doesn't have it: link the guide.
    notice_docs: bool,
    /// Whether the server has its own /admin web page.
    web: Option<bool>,
    web_check: Option<Job<bool>>,
    /// The open "⋮" menu.
    menu: Option<String>,
    dashboard: dashboard::State,
    requests: requests::State,
    moving: moving::State,
    users: users::State,
    /// The server's own title lists (`GET /api/admin/movies`, `/series`).
    catalog: [Remote<Vec<Item>>; 2],
}

/// What a section's view gets to work with.
pub struct Cx<'a> {
    pub client: &'a Client,
    pub lib: &'a Library,
    pub images: &'a crate::iced_ui::images::Images,
    pub busy: bool,
    /// The signed-in user, lowercased.
    pub me: String,
    pub users: Option<usize>,
    /// Every title as the server's admin sees it, or the library's.
    pub movies: &'a [Item],
    pub series: &'a [Item],
    pub menu: Option<&'a str>,
}

impl Cx<'_> {
    pub fn titles(&self, kind: Kind) -> &[Item] {
        if kind == Kind::Movie {
            self.movies
        } else {
            self.series
        }
    }
}

impl State {
    /// Reload the open section's data (after a change, or Refresh).
    fn refresh(&mut self) {
        match self.section {
            Section::Dashboard => self.dashboard.refresh(),
            Section::Requests => self.requests.refresh(),
            Section::Moving => self.moving.refresh(),
            _ => {}
        }
        self.users.refresh();
        for c in &mut self.catalog {
            c.retry();
        }
    }

    fn missing(&self, s: Section) -> bool {
        match s {
            Section::Dashboard => self.dashboard.missing(),
            Section::Requests => self.requests.missing(),
            Section::Moving => self.moving.missing(),
            Section::Users => self.users.missing(),
            _ => false,
        }
    }

    pub fn busy(&self) -> bool {
        self.op.is_some()
            || self.web_check.is_some()
            || self.dashboard.busy()
            || self.users.busy()
            || self.requests.busy()
            || self.moving.busy()
    }

    /// Run `f` in the background; its outcome becomes the notice.
    fn run(&mut self, client: &Client, f: impl FnOnce(&Client) -> Result<Done> + Send + 'static) {
        if self.op.is_some() {
            return;
        }
        self.notice = None;
        self.notice_docs = false;
        let c = client.clone();
        self.op = Some(Job::spawn(move || f(&c)));
    }

    /// Poll and start what the open page needs.
    pub fn work(&mut self, client: &Client, open: bool) -> Asks {
        let mut asks = Asks::default();
        // the request queue is counted in the sidebar, so it's kept fresh
        self.requests.background(client, &mut asks.login);
        if !open {
            return asks;
        }
        if self.web.is_none() && self.web_check.is_none() {
            let c = client.clone();
            self.web_check = Some(Job::spawn(move || c.has_admin_page()));
        }
        if let Some(w) = self.web_check.as_mut().and_then(Job::poll) {
            self.web_check = None;
            self.web = Some(w);
        }
        if let Some(r) = self.op.as_mut().and_then(Job::poll) {
            self.op = None;
            match r {
                Ok(done) => {
                    self.notice_docs = false;
                    self.notice = Some((done.msg, true));
                    self.refresh();
                    asks.reload_library |= done.library;
                    asks.reload_layout |= done.layout;
                }
                Err(e) if e.is::<api::Unauthorized>() => asks.login = true,
                Err(e) if e.is::<api::NoAdminApi>() => {
                    self.notice = Some(("This server doesn't support that yet.".into(), false));
                    self.notice_docs = true;
                }
                Err(e) => self.notice = Some((format!("{e:#}"), false)),
            }
        }
        self.users.work(client, &mut asks.login);
        for (c, kind) in self.catalog.iter_mut().zip([Kind::Movie, Kind::Series]) {
            c.poll(&mut asks.login);
            if c.idle() || (c.data.is_some() && c.stale(u64::MAX)) {
                let cl = client.clone();
                c.load(move || cl.admin_items(kind));
            }
        }
        match self.section {
            Section::Dashboard => self.dashboard.work(client, &mut asks),
            Section::Moving => self.moving.work(client, &mut asks.login),
            _ => {}
        }
        asks
    }

    pub fn update(&mut self, msg: Msg, client: &Client, lib: Option<&Library>) -> Asks {
        let mut asks = Asks::default();
        match msg {
            Msg::Section(s) => {
                if s != self.section {
                    self.section = s;
                    self.notice = None;
                    self.notice_docs = false;
                    self.menu = None;
                }
            }
            Msg::Refresh => {
                self.notice = None;
                self.notice_docs = false;
                self.refresh();
                asks.reload_library = true;
            }
            Msg::Menu(m) => self.menu = m,
            Msg::CheckAgain(s) => match s {
                Section::Dashboard => self.dashboard.refresh(),
                Section::Requests => self.requests.refresh(),
                Section::Moving => self.moving.refresh(),
                Section::Users => self.users.refresh(),
                _ => {}
            },
            Msg::Dash(m) => {
                if let Some(job) = self.dashboard.update(m, client) {
                    self.run(client, job);
                }
            }
            Msg::Requests(m) => {
                self.menu = None;
                if let Some(job) = self.requests.update(m) {
                    self.run(client, job);
                }
            }
            Msg::Moving(m) => {
                let items: Vec<Item> = self
                    .catalog
                    .iter()
                    .filter_map(|c| c.data.as_ref())
                    .flatten()
                    .cloned()
                    .chain(lib.into_iter().flat_map(|l| l.all().cloned()))
                    .collect();
                if let Some(job) = self.moving.update(m, &items) {
                    self.run(client, job);
                }
            }
            Msg::Users(m) => {
                self.menu = None;
                match self.users.update(m) {
                    users::Out::None => {}
                    users::Out::Run(job) => self.run(client, job),
                    users::Out::Profile(name) => asks.profile = Some(name),
                }
            }
        }
        asks
    }

    /// Requests waiting for an answer, for the sidebar.
    fn pending(&self) -> usize {
        self.requests.pending()
    }

    /// The sections, on the left (egui's `admin_sidebar`).
    pub fn sidebar<'a>(&self, server: &str, base: &str) -> Element<'a, LMsg> {
        let pending = self.pending();
        let mut col: Column<'a, LMsg> = column![
            caps("Admin", c(TEAL())),
            space().height(12),
            text(server.to_owned())
                .font(BOLD)
                .size(16)
                .wrapping(text::Wrapping::None),
            space().height(26),
        ];
        for s in Section::ALL {
            let badge = match s {
                Section::Requests if pending > 0 => Some(pending.to_string()),
                _ if !s.ported() => Some("later".to_owned()),
                _ if self.missing(s) => Some("not set up".into()),
                _ => None,
            };
            col = col
                .push(nav_item(
                    s.label(),
                    badge,
                    self.section == s,
                    LMsg::Admin(Msg::Section(s)),
                ))
                .push(space().height(10));
        }
        col = col.push(space().height(18));
        if self.web == Some(true) {
            col = col
                .push(link(
                    "Open web admin",
                    14.5,
                    style::TEXT(),
                    Some((Icon::External, true)),
                    LMsg::OpenUrl(format!("{base}/admin")),
                ))
                .push(space().height(8));
        }
        col = col.push(link(
            "Setup guides",
            14.5,
            style::TEXT(),
            Some((Icon::External, true)),
            LMsg::OpenUrl(api::API_DOCS.to_owned()),
        ));
        container(col)
            .width(220)
            .height(Length::Fill)
            .padding([24, 14])
            .clip(true)
            .style(|_| container::Style {
                background: Some(Background::Color(c(SURFACE()))),
                border: Border {
                    color: c(LINE()),
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            })
            .into()
    }

    /// The page: the section's title, Refresh, the last change's outcome,
    /// then the section.
    #[allow(clippy::too_many_arguments)]
    pub fn view<'a>(
        &'a self,
        client: &'a Client,
        lib: &'a Library,
        images: &'a crate::iced_ui::images::Images,
        me: &str,
        w: f32,
    ) -> Element<'a, LMsg> {
        let (m, inner) = column_of(w);
        let section = self.section;
        let busy = self.op.is_some();
        let mut title = row![
            text(section.label()).font(BOLD).size(34),
            space::horizontal()
        ]
        .spacing(10)
        .align_y(Vertical::Center);
        if busy {
            title = title.push(crate::iced_ui::icon::spinner(
                spin_secs(),
                style::MUTED(),
                16.0,
            ));
        }
        title = title.push(widgets::button_with(
            Btn::Small,
            "Refresh",
            None,
            (!busy).then_some(LMsg::Admin(Msg::Refresh)),
        ));
        let mut head: Column<'a, LMsg> = column![
            caps(&format!("Admin   ·   {}", client.info.name), c(TEAL())),
            gap(4.0),
            title,
            gap(4.0),
            text(section.about()).size(14.5).color(style::MUTED()),
            gap(14.0),
        ]
        .width(inner);
        if let Some(n) = notice(&self.notice) {
            head = head.push(n).push(gap(10.0));
        }
        if self.notice_docs && self.notice.is_some() {
            head = head
                .push(needs_note(
                    "What the server needs is in the guide.",
                    section,
                ))
                .push(gap(10.0));
        }
        let (movies, series) = match (&self.catalog[0].data, &self.catalog[1].data) {
            (Some(mv), Some(sr)) => (mv.as_slice(), sr.as_slice()),
            _ => (lib.movies.as_slice(), lib.series.as_slice()),
        };
        let cx = Cx {
            client,
            lib,
            images,
            busy,
            me: me.to_lowercase(),
            users: self.users.count(),
            movies,
            series,
            menu: self.menu.as_deref(),
        };
        let body: Element<'a, LMsg> = match section {
            Section::Dashboard => self.dashboard.view(&cx, w),
            Section::Requests => self.requests.view(&cx, w),
            Section::Moving => self.moving.view(&cx),
            Section::Users => self.users.view(&cx, w),
            _ => panel(column![
                caps("Coming soon", c(WARN())),
                gap(6.0),
                text(format!("{} isn't in this build yet", section.label())).font(BOLD).size(20),
                gap(6.0),
                text("It's being moved to the new interface; use a release build to manage it for now.")
                    .size(14.5)
                    .color(style::MUTED()),
            ]),
        };
        let body: Element<'a, LMsg> = match section {
            // the dashboard and users lay out their own columns
            Section::Dashboard | Section::Requests | Section::Users => body,
            _ => container(container(body).width(inner))
                .padding(Padding::default().left(m))
                .into(),
        };
        column![
            container(head).padding(Padding::default().left(m).top(40.0)),
            gap(10.0),
            body,
        ]
        .into()
    }
}

/// "This isn't set up on the server", with a link to the setup guide and Check again.
pub fn not_set_up<'a>(s: Section, server: &str) -> Element<'a, LMsg> {
    panel(column![
        caps("Not set up", c(WARN())),
        gap(6.0),
        text(format!("{} isn't set up on {server}", s.label()))
            .font(BOLD)
            .size(20),
        gap(6.0),
        text(format!(
            "To use this page, the server needs {}. The setup guide has \
             everything to add, with examples.",
            s.needs()
        ))
        .size(14.5)
        .color(style::MUTED()),
        gap(12.0),
        row![
            widgets::button_with(
                Btn::Small,
                "Setup guide",
                Some(Icon::External),
                Some(LMsg::OpenUrl(s.docs().to_owned()))
            ),
            widgets::button_with(
                Btn::Small,
                "Check again",
                None,
                Some(LMsg::Admin(Msg::CheckAgain(s)))
            ),
        ]
        .spacing(10),
    ])
}

/// A small note under a feature that needs more from the server.
pub fn needs_note<'a>(t: &str, s: Section) -> Element<'a, LMsg> {
    row![
        text(t.to_owned()).size(12).color(style::MUTED()),
        link(
            "Setup guide",
            12.0,
            style::TEXT(),
            Some((Icon::External, true)),
            LMsg::OpenUrl(s.docs().to_owned())
        ),
    ]
    .spacing(8)
    .wrap()
    .into()
}

/// A colored dot, for status rows.
pub fn status_dot<'a>(col: Color) -> Element<'a, LMsg> {
    container(space())
        .width(8)
        .height(8)
        .style(move |_| container::Style {
            background: Some(Background::Color(col)),
            border: iced::border::rounded(4),
            ..container::Style::default()
        })
        .into()
}

/// The last change's outcome, green or red.
fn notice<'a>(n: &Option<(String, bool)>) -> Option<Element<'a, LMsg>> {
    let (msg, ok) = n.as_ref()?;
    let col = c(if *ok { OK() } else { ERROR() });
    Some(text(msg.clone()).size(14.5).color(col).into())
}

/// A red-outlined button for what can't be undone.
pub fn danger_button<'a>(t: &str, msg: Option<LMsg>) -> Element<'a, LMsg> {
    let err = c(ERROR());
    button(container(mono(t, 11.5).color(err)).center_y(30))
        .height(30)
        .padding([0, 12])
        .style(move |_, s| button::Style {
            background: matches!(s, button::Status::Hovered)
                .then(|| Background::Color(err.scale_alpha(0.12))),
            border: Border {
                color: err.scale_alpha(0.6),
                width: 1.0,
                radius: 2.0.into(),
            },
            ..button::Style::default()
        })
        .on_press_maybe(msg)
        .into()
}

/// A bordered row (users, requests).
pub fn row_frame<'a>(content: impl Into<Element<'a, LMsg>>) -> Element<'a, LMsg> {
    container(content)
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

/// The "⋮" button and, when open, its menu under it.
pub fn more_menu<'a>(
    key: String,
    open: bool,
    items: Vec<(&str, LMsg)>,
    busy: bool,
) -> Element<'a, LMsg> {
    let toggle = LMsg::Admin(Msg::Menu((!open).then(|| key.clone())));
    let b = crate::iced_ui::chrome::tip_below(
        widgets::small_icon_button_maybe(Icon::More, (!busy).then_some(toggle)),
        "More",
    );
    if !open {
        return b;
    }
    let mut menu = column![].width(220);
    for (label, msg) in items {
        menu = menu.push(widgets::menu_item(label, msg));
    }
    let panel = container(menu).padding(6).style(super::popup_style);
    iced::widget::stack![
        b,
        iced::widget::float(container(panel).padding(Padding::default().top(34.0)))
            .translate(|_, _| iced::Vector::new(-190.0, 0.0)),
    ]
    .into()
}

/// A sidebar row, filled when it's the open section.
fn nav_item<'a>(
    label: &str,
    badge: Option<String>,
    selected: bool,
    msg: LMsg,
) -> Element<'a, LMsg> {
    let mut r = row![mono(label, 12.0)].align_y(Vertical::Center);
    if let Some(b) = badge {
        let warn = b == "not set up" || b == "later";
        let col = if selected {
            Color::from_rgb8(90, 90, 90)
        } else if warn {
            style::MUTED()
        } else {
            c(TEAL())
        };
        r = r.push(space::horizontal()).push(mono(&b, 10.0).color(col));
    }
    button(container(r).center_y(36))
        .width(Length::Fill)
        .height(36)
        .padding([0, 12])
        .style(move |_, s| {
            let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: if selected {
                    Some(Background::Color(c(PRIMARY())))
                } else if hot {
                    Some(Background::Color(c(SURFACE_HI())))
                } else {
                    None
                },
                text_color: if selected { c(INK()) } else { style::TEXT() },
                border: iced::border::rounded(4),
                ..button::Style::default()
            }
        })
        .on_press(msg)
        .into()
}

/// A collapsible list's heading: "Movies · 120", with a chevron.
pub fn fold<'a>(label: String, open: bool, msg: LMsg) -> Element<'a, LMsg> {
    button(
        row![
            icon::icon(
                if open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                style::MUTED(),
                14.0
            ),
            text(label).font(BOLD).size(15),
        ]
        .spacing(6)
        .align_y(Vertical::Center),
    )
    .padding([4, 0])
    .style(|_, _| button::Style {
        text_color: style::TEXT(),
        ..button::Style::default()
    })
    .on_press(msg)
    .into()
}

impl Lib {
    /// Whether the open page is Admin.
    pub(super) fn admin_open(&self) -> bool {
        self.view == super::View::Admin && self.search.trim().is_empty()
    }
}

/// Everyone who can sign in (for other sections).
#[allow(dead_code)]
pub fn accounts(st: &State) -> &[AdminUser] {
    st.users.list()
}
