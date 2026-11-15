//! The Admin page (`browse::admin`): a sidebar of
//! sections for managing the server, each talking to its own part of the
//! optional admin API; when the server doesn't have that part, the section
//! links to its setup guide instead.
mod addons;
mod analytics;
mod collections;
mod dashboard;
mod layout;
mod library;
mod moving;
mod requests;
mod skip;
mod ui;
mod users;

use anyhow::Result;
use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, space, text, Column};
use iced::{Background, Border, Color, Element, Length, Padding};

use super::details::{column_of, gap, link};
use super::profile::Remote;
use super::{spin_secs, Library as Lib, Msg as LMsg};
use crate::api::{self, AdminUser, Client, Item, Kind, Library};
use crate::iced_ui::icon::{self, Icon};
use crate::iced_ui::style::{self, c, BOLD};
use crate::iced_ui::widgets::{self, Kind as Btn};
use crate::iced_ui::Job;
use crate::ui::browse::details::TEAL;
use crate::ui::theme::{ERROR, INK, LINE, OK, PRIMARY, SURFACE, SURFACE_HI, WARN};
use ui::{caps, panel};

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
    SkipMarkers,
    Addons,
    Layout,
    Users,
}

impl Section {
    const ALL: [Section; 10] = [
        Section::Dashboard,
        Section::Analytics,
        Section::Requests,
        Section::Library,
        Section::Collections,
        Section::Moving,
        Section::SkipMarkers,
        Section::Addons,
        Section::Layout,
        Section::Users,
    ];

    /// The section's icon in the sidebar.
    fn icon(self) -> Icon {
        match self {
            Section::Dashboard => Icon::Home,
            Section::Analytics => Icon::Star,
            Section::Requests => Icon::Bookmark,
            Section::Library => Icon::Film,
            Section::Collections => Icon::List,
            Section::Moving => Icon::ArrowRight,
            Section::SkipMarkers => Icon::SkipForward,
            Section::Addons => Icon::More,
            Section::Layout => Icon::Tv,
            Section::Users => Icon::Settings,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Section::Dashboard => "Dashboard",
            Section::Analytics => "Analytics",
            Section::Requests => "Requests",
            Section::Library => "Library",
            Section::Collections => "Collections",
            Section::Moving => "Moving",
            Section::SkipMarkers => "Skip markers",
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
            Section::SkipMarkers => {
                "Where episodes' recap, intro and credits are, for Skip Intro, Skip Recap and \
                 Next Episode in the player."
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
            Section::SkipMarkers => api::ADMIN_DOCS_SKIP,
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
            Section::SkipMarkers => "the skip marker endpoints (/api/admin/skip-markers)",
            Section::Addons => "the addon endpoints (/api/addons, /api/admin/addons)",
            Section::Layout => "the layout endpoints (/api/layout, /api/admin/layout)",
            Section::Users => "Valo's admin API (/api/admin/users)",
        }
    }
}

/// Where the admin page is, for the mouse's back and forward buttons: a
/// section, and in Analytics the user whose page is open.
#[derive(Clone, PartialEq, Debug)]
struct Place {
    section: Section,
    user: Option<String>,
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
    /// Images that changed on the server: download them again.
    pub forget: Vec<String>,
    /// Only part of it worked: `msg` says what's left, shown in red.
    pub partial: bool,
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
    Skip(skip::Msg),
    Users(users::Msg),
    Library(library::Msg),
    Analytics(analytics::Msg),
    Collections(collections::Msg),
    Layout(layout::Msg),
    Addons(addons::Msg),
}

/// What the admin page asks the library to do.
#[derive(Default)]
pub struct Asks {
    pub login: bool,
    pub reload_library: bool,
    pub reload_layout: bool,
    /// Images that changed on the server.
    pub forget: Vec<String>,
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
    skip: skip::State,
    users: users::State,
    library: library::State,
    analytics: analytics::State,
    collections: collections::State,
    layout: layout::State,
    addons: addons::State,
    /// The server's own title lists (`GET /api/admin/movies`, `/series`).
    catalog: [Remote<Vec<Item>>; 2],
    /// Places left with Back, and the ones Back came from (for Forward).
    back: Vec<Place>,
    ahead: Vec<Place>,
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
    /// Everyone who can sign in.
    pub accounts: &'a [AdminUser],
    /// The server lists its titles for admins (else the library stands in).
    pub catalog: bool,
}

impl<'a> Cx<'a> {
    /// A title by our id: the server's admin lists first, then the library.
    pub fn find(&self, id: &str) -> Option<&'a Item> {
        let (movies, series) = (self.movies, self.series);
        movies
            .iter()
            .chain(series)
            .find(|i| i.id == id)
            .or_else(|| self.lib.find(id))
    }

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
            Section::SkipMarkers => self.skip.refresh(),
            Section::Library => self.library.refresh(),
            Section::Analytics => self.analytics.refresh(),
            Section::Collections => self.collections.refresh(),
            Section::Layout => self.layout.refresh(),
            Section::Addons => self.addons.refresh(),
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
            Section::SkipMarkers => self.skip.missing(),
            Section::Users => self.users.missing(),
            Section::Library => self.library.missing(),
            Section::Analytics => self.analytics.missing(),
            Section::Collections => self.collections.missing(),
            Section::Layout => self.layout.missing(),
            Section::Addons => self.addons.missing(),
        }
    }

    pub fn busy(&self) -> bool {
        self.op.is_some()
            || self.web_check.is_some()
            || self.dashboard.busy()
            || self.users.busy()
            || self.requests.busy()
            || self.moving.busy()
            || self.skip.busy()
            || self.library.busy()
            || self.analytics.busy()
            || self.collections.busy()
            || self.layout.busy()
            || self.addons.busy()
    }

    /// Every title we know: the server's admin lists, then the library.
    fn all_items(&self, lib: Option<&Library>) -> Vec<Item> {
        self.catalog
            .iter()
            .filter_map(|c| c.data.as_ref())
            .flatten()
            .cloned()
            .chain(lib.into_iter().flat_map(|l| l.all().cloned()))
            .collect()
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
            // a new visit starts a new history
            self.back.clear();
            self.ahead.clear();
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
                    self.notice = Some((done.msg, !done.partial));
                    self.refresh();
                    asks.reload_library |= done.library;
                    asks.reload_layout |= done.layout;
                    asks.forget = done.forget;
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
            Section::SkipMarkers => self.skip.work(client, &mut asks),
            Section::Library => self.library.work(client, &mut asks),
            Section::Analytics => self.analytics.work(client, &mut asks),
            Section::Collections => self.collections.work(client, &mut asks),
            Section::Layout => self.layout.work(client, &mut asks),
            Section::Addons => self.addons.work(client, &mut asks),
            _ => {}
        }
        asks
    }

    fn place(&self) -> Place {
        Place {
            section: self.section,
            user: (self.section == Section::Analytics)
                .then(|| self.analytics.user())
                .flatten(),
        }
    }

    fn go(&mut self, p: Place) {
        self.section = p.section;
        if p.section == Section::Analytics {
            self.analytics.set_user(p.user);
        }
        self.notice = None;
        self.notice_docs = false;
        self.menu = None;
    }

    /// The mouse's back button: the admin place before this one; false when
    /// there's none, so the library goes back instead.
    pub fn back(&mut self) -> bool {
        let Some(p) = self.back.pop() else {
            return false;
        };
        self.ahead.push(self.place());
        self.go(p);
        true
    }

    /// The mouse's forward button: the place last left with Back.
    pub fn forward(&mut self) -> bool {
        let Some(p) = self.ahead.pop() else {
            return false;
        };
        self.back.push(self.place());
        self.go(p);
        true
    }

    pub fn update(&mut self, msg: Msg, client: &Client, lib: Option<&Library>) -> Asks {
        let before = self.place();
        let asks = self.update_inner(msg, client, lib);
        self.remember(before);
        asks
    }

    /// Somewhere else than `before` now: Back returns there.
    fn remember(&mut self, before: Place) {
        if self.place() != before {
            self.back.push(before);
            self.ahead.clear();
        }
    }

    fn update_inner(&mut self, msg: Msg, client: &Client, lib: Option<&Library>) -> Asks {
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
                Section::SkipMarkers => self.skip.refresh(),
                Section::Users => self.users.refresh(),
                Section::Library => self.library.refresh(),
                Section::Analytics => self.analytics.refresh(),
                Section::Collections => self.collections.refresh(),
                Section::Layout => self.layout.refresh(),
                Section::Addons => self.addons.refresh(),
            },
            Msg::Dash(dashboard::Msg::Goto(id)) => {
                // a title on the dashboard: open it in Library
                if let Some(i) = self.all_items(lib).iter().find(|i| i.id == id) {
                    self.library.select(i);
                }
                self.section = Section::Library;
                self.notice = None;
            }
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
                let items = self.all_items(lib);
                if let Some(job) = self.moving.update(m, &items) {
                    self.run(client, job);
                }
            }
            Msg::Skip(m) => {
                let rows = self.skip.rows();
                if let Some(job) = self.skip.update(m, &rows) {
                    self.run(client, job);
                }
            }
            Msg::Addons(m) => {
                if let Some(job) = self.addons.update(m) {
                    self.run(client, job);
                }
            }
            Msg::Layout(m) => {
                if let Some(job) = self.layout.update(m) {
                    self.run(client, job);
                }
            }
            Msg::Collections(m) => {
                let items = self.all_items(lib);
                if let Some(job) = self.collections.update(m, &items) {
                    self.run(client, job);
                }
            }
            Msg::Analytics(m) => {
                if let analytics::Out::Goto(s) = self.analytics.update(m) {
                    self.section = s;
                }
            }
            Msg::Library(m) => {
                let items = self.all_items(lib);
                if let Some(job) = self.library.update(m, client, &items) {
                    self.run(client, job);
                }
            }
            Msg::Users(m) => {
                self.menu = None;
                match self.users.update(m) {
                    users::Out::None => {}
                    users::Out::Run(job) => self.run(client, job),
                    users::Out::Profile(name) => {
                        self.analytics.open(name, Section::Users);
                        self.section = Section::Analytics;
                        self.notice = None;
                    }
                }
            }
        }
        asks
    }

    /// Requests waiting for an answer, for the sidebar.
    fn pending(&self) -> usize {
        self.requests.pending()
    }

    /// The sections, on the left.
    pub fn sidebar<'a>(&self, server: &str, base: &str) -> Element<'a, LMsg> {
        let pending = self.pending();
        // the server, like shadcn's team switcher: a lettered square and its name
        let letter = server
            .chars()
            .next()
            .unwrap_or('V')
            .to_uppercase()
            .to_string();
        let head = row![
            container(text(letter).font(BOLD).size(14).color(c(INK())))
                .center(30)
                .style(|_| container::Style {
                    background: Some(Background::Color(c(PRIMARY()))),
                    border: iced::border::rounded(8),
                    ..container::Style::default()
                }),
            column![
                text(server.to_owned())
                    .font(BOLD)
                    .size(14)
                    .wrapping(text::Wrapping::None),
                text("Admin").size(12).color(style::MUTED()),
            ],
        ]
        .spacing(10)
        .align_y(Vertical::Center);
        let mut col: Column<'a, LMsg> = column![
            container(head).padding([0, 6]),
            space().height(22),
            container(text("Manage").size(12).color(style::MUTED())).padding([0, 10]),
            space().height(6),
        ]
        .spacing(2);
        for s in Section::ALL {
            let badge = match s {
                Section::Requests if pending > 0 => Some(pending.to_string()),
                _ if self.missing(s) => Some("not set up".into()),
                _ => None,
            };
            col = col.push(nav_item(
                s.icon(),
                s.label(),
                badge,
                self.section == s,
                LMsg::Admin(Msg::Section(s)),
            ));
        }
        col = col
            .push(space().height(18))
            .push(container(text("Help").size(12).color(style::MUTED())).padding([0, 10]))
            .push(space().height(6));
        if self.web == Some(true) {
            col = col.push(nav_link(
                "Open web admin",
                LMsg::OpenUrl(format!("{base}/admin")),
            ));
        }
        col = col.push(nav_link(
            "Setup guides",
            LMsg::OpenUrl(api::API_DOCS.to_owned()),
        ));
        container(col)
            .width(232)
            .height(Length::Fill)
            .padding([20, 12])
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
            text(section.label()).font(BOLD).size(28),
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
        title = title.push(ui::button_with(
            Btn::Small,
            "Refresh",
            Some(Icon::Refresh),
            (!busy).then_some(LMsg::Admin(Msg::Refresh)),
        ));
        // a breadcrumb over the title, as in shadcn's dashboard
        let crumbs = row![
            text(client.info.name.clone())
                .size(13)
                .color(style::MUTED()),
            text("/").size(13).color(style::MUTED()),
            text("Admin").size(13).color(style::MUTED()),
            text("/").size(13).color(style::MUTED()),
            text(section.label()).size(13),
        ]
        .spacing(8);
        let mut head: Column<'a, LMsg> = column![
            crumbs,
            gap(6.0),
            title,
            space().height(2),
            ui::description(section.about()),
            gap(16.0),
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
            accounts: self.users.list(),
            catalog: self.catalog[0].data.is_some(),
        };
        let body: Element<'a, LMsg> = match section {
            Section::Dashboard => self.dashboard.view(&cx, w),
            Section::Requests => self.requests.view(&cx, w),
            Section::Moving => self.moving.view(&cx),
            Section::SkipMarkers => self.skip.view(&cx),
            Section::Users => self.users.view(&cx, w),
            Section::Library => self.library.view(&cx, w),
            Section::Analytics => self.analytics.view(&cx, w),
            Section::Collections => self.collections.view(&cx, w),
            Section::Layout => self.layout.view(&cx, w),
            Section::Addons => self.addons.view(&cx, w),
        };
        let body: Element<'a, LMsg> = match section {
            // these lay out their own columns (and margin): wrapping them
            // again put a second margin on the left
            Section::Dashboard
            | Section::Analytics
            | Section::Requests
            | Section::Library
            | Section::Collections
            | Section::Users
            | Section::Layout
            | Section::Addons => body,
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
            ui::button_with(
                Btn::Small,
                "Setup guide",
                Some(Icon::External),
                Some(LMsg::OpenUrl(s.docs().to_owned()))
            ),
            ui::button_with(
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
    // an alert: a tinted, bordered strip with a dot
    Some(
        container(
            row![
                status_dot(col),
                text(msg.clone()).size(14).color(style::TEXT())
            ]
            .spacing(10)
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .padding([10, 14])
        .style(move |_| container::Style {
            background: Some(Background::Color(col.scale_alpha(0.08))),
            border: Border {
                color: col.scale_alpha(0.4),
                width: 1.0,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        })
        .into(),
    )
}

/// A red-outlined button for what can't be undone.
pub fn danger_button<'a>(t: &str, msg: Option<LMsg>) -> Element<'a, LMsg> {
    ui::danger(t, msg)
}

/// A bordered row (users, requests).
pub fn row_frame<'a>(content: impl Into<Element<'a, LMsg>>) -> Element<'a, LMsg> {
    ui::row_frame(content)
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
        iced::widget::float(crate::iced_ui::loose::loose(
            container(panel).padding(Padding::default().top(34.0)),
        ))
        // under the button; over it when the bottom of the view would cut it
        // off (`bounds` starts at the button and holds the 34px gap)
        .translate(|bounds, viewport| {
            let up = bounds.y + bounds.height > viewport.y + viewport.height;
            iced::Vector::new(-190.0, if up { -(bounds.height + 4.0) } else { 0.0 })
        }),
    ]
    .into()
}

/// A sidebar row, filled when it's the open section.
fn nav_item<'a>(
    ic: Icon,
    label: &str,
    badge: Option<String>,
    selected: bool,
    msg: LMsg,
) -> Element<'a, LMsg> {
    let fg = if selected {
        style::TEXT()
    } else {
        style::MUTED()
    };
    let mut r = row![
        icon::icon(ic, fg, 16.0),
        text(label.to_owned()).size(14).color(fg)
    ]
    .spacing(10)
    .align_y(Vertical::Center);
    if let Some(b) = badge {
        let warn = b == "not set up" || b == "later";
        r = r
            .push(space::horizontal())
            .push(ui::badge(&b, if warn { style::MUTED() } else { c(TEAL()) }));
    }
    button(container(r).center_y(34))
        .width(Length::Fill)
        .height(34)
        .padding([0, 10])
        .style(move |_, s| {
            let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: (selected || hot).then(|| Background::Color(c(SURFACE_HI()))),
                text_color: style::TEXT(),
                border: iced::border::rounded(8),
                ..button::Style::default()
            }
        })
        .on_press(msg)
        .into()
}

/// A link in the sidebar's Help group, opening in the browser.
fn nav_link<'a>(label: &str, msg: LMsg) -> Element<'a, LMsg> {
    button(
        container(
            row![
                text(label.to_owned()).size(14).color(style::MUTED()),
                space::horizontal(),
                icon::icon(Icon::External, style::MUTED(), 14.0),
            ]
            .align_y(Vertical::Center),
        )
        .center_y(32),
    )
    .width(Length::Fill)
    .height(32)
    .padding([0, 10])
    .style(|_, s| button::Style {
        background: matches!(s, button::Status::Hovered | button::Status::Pressed)
            .then(|| Background::Color(c(SURFACE_HI()))),
        border: iced::border::rounded(8),
        ..button::Style::default()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_buttons_walk_the_admin_sections() {
        let mut st = State::default();
        for s in [Section::Users, Section::Analytics] {
            let before = st.place();
            st.section = s;
            st.remember(before);
        }
        // a user's page in Analytics is a place of its own
        let before = st.place();
        st.analytics.set_user(Some("ann".into()));
        st.remember(before);

        assert!(st.back());
        assert_eq!(
            st.place(),
            Place {
                section: Section::Analytics,
                user: None
            }
        );
        assert!(st.back());
        assert_eq!(st.section, Section::Users);
        assert!(st.back());
        assert_eq!(st.section, Section::Dashboard);
        assert!(!st.back(), "out of the admin page next");
        assert!(st.forward());
        assert!(st.forward());
        assert!(st.forward());
        assert_eq!(
            st.place(),
            Place {
                section: Section::Analytics,
                user: Some("ann".into())
            }
        );
        assert!(!st.forward());
    }
}
