//! The Admin page: a sidebar of sections for managing the server. Each
//! section talks to its own part of the optional admin API (docs/api/admin*.md);
//! when the server doesn't have that part, the section links to its setup
//! guide instead.
mod addons;
mod analytics;
mod collections;
mod dashboard;
mod layout;
mod library;
mod requests;
mod users;

use std::time::Instant;

use anyhow::Result;
use eframe::egui::{self, vec2, Align2, Color32, FontId, RichText, Sense, Ui};

use super::details::{caps, padded, panel, TEAL};
use super::{Browse, Click};
use crate::api::{self, Client, Item, Kind, Library, NoAdminApi, Unauthorized};
use crate::images::Images;
use crate::ui::chrome;
use crate::ui::{
    bold, small_button, Action, ERROR, LINE, MUTED, OK, SURFACE, SURFACE_HI, TEXT, WARN,
};
use crate::util::Task;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Section {
    #[default]
    Dashboard,
    Analytics,
    Requests,
    Library,
    Collections,
    Addons,
    Layout,
    Users,
}

impl Section {
    const ALL: [Section; 8] = [
        Section::Dashboard,
        Section::Analytics,
        Section::Requests,
        Section::Library,
        Section::Collections,
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
            Section::Addons => {
                "Stremio addons for everyone: catalogs on the home page, missing details \
                 filled in, and subtitles in the player."
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
            Section::Addons => "the addon endpoints (/api/addons, /api/admin/addons)",
            Section::Layout => "the layout endpoints (/api/layout, /api/admin/layout)",
            Section::Users => "Valo's admin API (/api/admin/users)",
        }
    }
}

/// One section's data from the server.
pub(super) struct Remote<T> {
    pub data: Option<T>,
    task: Option<Task<Result<T>>>,
    pub error: Option<String>,
    /// The server doesn't have this part of the admin API.
    pub missing: bool,
    /// When the last load finished.
    pub at: Option<Instant>,
}

impl<T> Default for Remote<T> {
    fn default() -> Self {
        Self {
            data: None,
            task: None,
            error: None,
            missing: false,
            at: None,
        }
    }
}

impl<T: Send + 'static> Remote<T> {
    pub fn load(&mut self, ctx: &egui::Context, f: impl FnOnce() -> Result<T> + Send + 'static) {
        self.task = Some(Task::spawn(ctx, f));
    }

    pub fn loading(&self) -> bool {
        self.task.is_some()
    }

    /// Nothing loaded or loading, and nothing to wait out.
    pub fn idle(&self) -> bool {
        self.data.is_none() && self.task.is_none() && self.error.is_none() && !self.missing
    }

    /// Time to load: never loaded, or asked to load again with [`retry`](Self::retry).
    pub fn due(&self) -> bool {
        self.task.is_none()
            && self.error.is_none()
            && !self.missing
            && (self.data.is_none() || self.at.is_none())
    }

    /// Older than `secs` (and not loading): time to load again.
    pub fn stale(&self, secs: u64) -> bool {
        self.task.is_none() && self.at.is_none_or(|t| t.elapsed().as_secs() >= secs)
    }

    /// Load again on the next frame, keeping what's shown meanwhile.
    pub fn retry(&mut self) {
        self.error = None;
        self.missing = false;
        self.at = None;
        if self.data.is_none() {
            self.task = None;
        }
    }

    /// Take a finished load: true when new data came in. A 401 sets `login`.
    pub fn poll(&mut self, login: &mut bool) -> bool {
        let Some(r) = self.task.as_mut().and_then(|t| t.poll()) else {
            return false;
        };
        self.task = None;
        self.at = Some(Instant::now());
        match r {
            Ok(d) => {
                self.data = Some(d);
                self.error = None;
                self.missing = false;
                true
            }
            Err(e) if e.is::<Unauthorized>() => {
                *login = true;
                false
            }
            Err(e) if e.is::<NoAdminApi>() => {
                self.missing = true;
                false
            }
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                false
            }
        }
    }
}

/// A finished change.
#[derive(Default)]
pub(super) struct Done {
    /// What to tell the admin.
    pub msg: String,
    /// Reload the library afterwards.
    pub library: bool,
    /// Images that changed on the server.
    pub forget: Vec<String>,
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

/// A change to make in the background.
pub(super) type Job = Box<dyn FnOnce(&Client) -> Result<Done> + Send>;

/// What a section gets to work with, and what it can ask for.
pub(super) struct Cx<'a> {
    pub ctx: egui::Context,
    pub client: &'a Client,
    pub lib: &'a Library,
    pub images: &'a mut Images,
    /// A change is running.
    pub busy: bool,
    /// The signed-in user, lowercased.
    pub me: String,
    /// How many users the server has, once known.
    pub users: Option<usize>,
    /// Every title as the server's admin sees it, or the library's when
    /// the server can't list them (`catalog` false).
    pub movies: &'a [Item],
    pub series: &'a [Item],
    pub catalog: bool,
    /// Sign in again (a 401).
    pub login: bool,
    pub job: Option<Job>,
    /// Open another section; with an item id, open it in the Library.
    pub goto: Option<(Section, Option<String>)>,
    pub reload_library: bool,
    pub click: Option<Click>,
    /// Scroll the page back to the top.
    pub to_top: bool,
}

impl<'a> Cx<'a> {
    /// A title by our id: the server's list first, then the library.
    pub fn find(&self, id: &str) -> Option<&'a Item> {
        let (movies, series) = (self.movies, self.series);
        movies
            .iter()
            .chain(series)
            .find(|i| i.id == id)
            .or_else(|| self.lib.find(id))
    }

    pub fn titles(&self, kind: Kind) -> &'a [Item] {
        if kind == Kind::Movie {
            self.movies
        } else {
            self.series
        }
    }

    pub fn run(&mut self, f: impl FnOnce(&Client) -> Result<Done> + Send + 'static) {
        self.job = Some(Box::new(f));
    }
}

#[derive(Default)]
pub(super) struct AdminState {
    section: Section,
    op: Option<Task<Result<Done>>>,
    /// The last change's outcome, and whether it worked.
    notice: Option<(String, bool)>,
    /// The last change failed because the server doesn't have it: link the guide.
    notice_docs: bool,
    /// Whether the server has its own /admin web page.
    web: Option<bool>,
    web_check: Option<Task<bool>>,
    dashboard: dashboard::State,
    analytics: analytics::State,
    requests: requests::State,
    library: library::State,
    collections: collections::State,
    addons: addons::State,
    layout: layout::State,
    users: users::State,
    /// The server's own title lists (`GET /api/admin/movies`, `/series`).
    catalog: [Remote<Vec<Item>>; 2],
}

impl AdminState {
    /// Reload the open section's data (after a change, or Refresh).
    fn refresh(&mut self) {
        match self.section {
            Section::Dashboard => self.dashboard.refresh(),
            Section::Analytics => self.analytics.refresh(),
            Section::Requests => self.requests.refresh(),
            Section::Library => self.library.refresh(),
            Section::Collections => self.collections.refresh(),
            Section::Addons => self.addons.refresh(),
            Section::Layout => self.layout.refresh(),
            Section::Users => {}
        }
        self.users.refresh();
        for c in &mut self.catalog {
            c.retry();
        }
    }

    /// Keep the server's title lists loaded, whichever section is open.
    fn load_catalog(&mut self, ctx: &egui::Context, client: &Client, login: &mut bool) {
        for (c, kind) in self.catalog.iter_mut().zip([Kind::Movie, Kind::Series]) {
            c.poll(login);
            if c.idle() || (c.data.is_some() && c.stale(u64::MAX)) {
                let cl = client.clone();
                c.load(ctx, move || cl.admin_items(kind));
            }
        }
    }

    /// The server lists its titles for the admin.
    fn has_catalog(&self) -> bool {
        self.catalog.iter().all(|c| c.data.is_some())
    }

    fn missing(&self, s: Section) -> bool {
        match s {
            Section::Dashboard => self.dashboard.missing(),
            Section::Analytics => self.analytics.missing(),
            Section::Requests => self.requests.missing(),
            Section::Library => self.library.missing,
            Section::Collections => self.collections.missing(),
            Section::Addons => self.addons.missing(),
            Section::Layout => self.layout.missing(),
            Section::Users => self.users.missing(),
        }
    }
}

/// "This isn't set up on the server" with a link to the setup guide.
/// True when the admin asks to check again.
pub(super) fn not_set_up(ui: &mut Ui, s: Section, server: &str) -> bool {
    let mut again = false;
    panel(ui, |ui| {
        caps(ui, "Not set up", WARN());
        ui.add_space(6.0);
        ui.label(bold(
            format!("{} isn't set up on {server}", s.label()),
            20.0,
        ));
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!(
                "To use this page, the server needs {}. The setup guide has \
                 everything to add, with examples.",
                s.needs()
            ))
            .color(MUTED()),
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if crate::ui::small_button_icon(ui, crate::ui::chrome::Icon::External, "Setup guide")
                .clicked()
            {
                chrome::open_url(s.docs());
            }
            if small_button(ui, "Check again").clicked() {
                again = true;
            }
        });
    });
    again
}

/// A small note under a feature that needs more from the server.
pub(super) fn needs_note(ui: &mut Ui, text: &str, s: Section) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(text).small().color(MUTED()));
        if crate::ui::ext_link(ui, RichText::new("Setup guide").small()) {
            chrome::open_url(s.docs());
        }
    });
}

/// A colored dot and a line of text, for status rows.
pub(super) fn status_dot(ui: &mut Ui, col: Color32) {
    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 4.0, col);
}

pub(super) fn notice(ui: &mut Ui, n: &Option<(String, bool)>) {
    if let Some((msg, ok)) = n {
        ui.label(RichText::new(msg).color(if *ok { OK() } else { ERROR() }));
        ui.add_space(10.0);
    }
}

impl Browse {
    /// The admin sections, on the left.
    pub(super) fn admin_sidebar(&mut self, ctx: &egui::Context) {
        let st = &mut self.admin;
        let pending = st.requests.pending();
        let mut pick = None;
        egui::SidePanel::left("admin-sidebar")
            .resizable(false)
            .exact_width(220.0)
            .frame(
                egui::Frame::new()
                    .fill(SURFACE())
                    .stroke(egui::Stroke::new(1.0f32, LINE()))
                    .inner_margin(egui::Margin::symmetric(14, 24)),
            )
            .show(ctx, |ui| {
                caps(ui, "Admin", TEAL());
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(bold(&self.client.info.name, 16.0).color(TEXT())).truncate(),
                );
                ui.add_space(18.0);
                for s in Section::ALL {
                    let badge = match s {
                        Section::Requests if pending > 0 => Some(pending.to_string()),
                        _ if st.missing(s) => Some("not set up".into()),
                        _ => None,
                    };
                    if nav_item(ui, s.label(), badge.as_deref(), st.section == s) {
                        pick = Some(s);
                    }
                    ui.add_space(2.0);
                }
                ui.add_space(18.0);
                if st.web == Some(true) && crate::ui::ext_link(ui, "Open web admin") {
                    chrome::open_url(&format!("{}/admin", self.client.base));
                }
                if crate::ui::ext_link(ui, "Setup guides") {
                    chrome::open_url(api::API_DOCS);
                }
            });
        if let Some(s) = pick {
            if s != st.section {
                st.section = s;
                st.notice = None;
                st.notice_docs = false;
                self.to_top = true;
            }
        }
    }

    pub(super) fn admin_page(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        images: &mut Images,
        action: &mut Action,
    ) -> Option<Click> {
        let ctx = ui.ctx().clone();
        let st = &mut self.admin;
        if st.web.is_none() && st.web_check.is_none() {
            let c = self.client.clone();
            st.web_check = Some(Task::spawn(&ctx, move || c.has_admin_page()));
        }
        if let Some(w) = st.web_check.as_mut().and_then(|t| t.poll()) {
            st.web_check = None;
            st.web = Some(w);
        }
        let mut login = false;
        if let Some(r) = st.op.as_mut().and_then(|t| t.poll()) {
            st.op = None;
            match r {
                Ok(done) => {
                    st.notice_docs = false;
                    st.notice = Some((done.msg, true));
                    for u in &done.forget {
                        images.forget(u);
                    }
                    st.refresh();
                    if done.library {
                        self.reload(&ctx);
                    } else if done.layout {
                        self.reload_layout(&ctx);
                    }
                }
                Err(e) if e.is::<Unauthorized>() => login = true,
                Err(e) if e.is::<NoAdminApi>() => {
                    self.admin.notice =
                        Some(("This server doesn't support that yet.".into(), false));
                    self.admin.notice_docs = true;
                }
                Err(e) => self.admin.notice = Some((format!("{e:#}"), false)),
            }
        }
        let st = &mut self.admin;
        // the users list feeds the dashboard's numbers too
        st.users.poll_and_load(&ctx, &self.client, &mut login);
        st.load_catalog(&ctx, &self.client, &mut login);
        st.requests.background(&ctx, &self.client, &mut login);

        let section = st.section;
        let mut refresh = false;
        let busy = st.op.is_some();

        // header
        ui.add_space(32.0);
        padded(ui, |ui| {
            caps(
                ui,
                &format!("Admin   ·   {}", self.client.info.name),
                TEAL(),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(bold(section.label(), 34.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled_ui(!busy, |ui| small_button(ui, "Refresh"))
                        .inner
                        .clicked()
                    {
                        refresh = true;
                    }
                    if busy {
                        ui.add(egui::Spinner::new().size(16.0).color(MUTED()));
                    }
                });
            });
            ui.add_space(4.0);
            ui.label(RichText::new(section.about()).color(MUTED()));
            ui.add_space(14.0);
            notice(ui, &st.notice);
            if st.notice_docs && st.notice.is_some() {
                needs_note(ui, "What the server needs is in the guide.", section);
                ui.add_space(10.0);
            }
        });
        ui.add_space(10.0);

        let users = st.users.count();
        let catalog = st.has_catalog();
        let (movies, series) = match (&st.catalog[0].data, &st.catalog[1].data) {
            (Some(m), Some(s)) => (m.as_slice(), s.as_slice()),
            _ => (lib.movies.as_slice(), lib.series.as_slice()),
        };
        let mut cx = Cx {
            ctx: ctx.clone(),
            client: &self.client,
            lib,
            images,
            busy,
            me: self.user.clone().unwrap_or_default().to_lowercase(),
            users,
            movies,
            series,
            catalog,
            login: false,
            job: None,
            goto: None,
            reload_library: false,
            click: None,
            to_top: false,
        };
        match section {
            Section::Dashboard => st.dashboard.ui(ui, &mut cx),
            Section::Analytics => st.analytics.ui(ui, &mut cx),
            Section::Requests => st.requests.ui(ui, &mut cx),
            Section::Library => st.library.ui(ui, &mut cx),
            Section::Collections => st.collections.ui(ui, &mut cx),
            Section::Addons => st.addons.ui(ui, &mut cx),
            Section::Layout => st.layout.ui(ui, &mut cx),
            Section::Users => st.users.ui(ui, &mut cx),
        }
        let Cx {
            login: l2,
            job,
            goto,
            reload_library,
            click,
            to_top,
            ..
        } = cx;
        self.to_top |= to_top;

        if login || l2 {
            *action = Action::NeedLogin(self.client.clone());
        }
        if let Some(job) = job {
            st.notice = None;
            st.notice_docs = false;
            let c = self.client.clone();
            st.op = Some(Task::spawn(&ctx, move || job(&c)));
        }
        if let Some((s, item)) = goto {
            st.section = s;
            st.notice = None;
            st.notice_docs = false;
            self.to_top = true;
            if let Some(id) = item {
                let found = st.catalog.iter().filter_map(|c| c.data.as_ref()).flatten();
                if let Some(i) = found
                    .into_iter()
                    .find(|i| i.id == id)
                    .or_else(|| lib.find(&id))
                {
                    st.library.select(i);
                }
            }
        }
        if refresh {
            st.notice = None;
            st.notice_docs = false;
            st.refresh();
            st.library.missing = false;
            self.reload(&ctx);
        } else if reload_library {
            self.reload(&ctx);
        }
        click
    }
}

/// A sidebar row; true when clicked.
fn nav_item(ui: &mut Ui, label: &str, badge: Option<&str>, selected: bool) -> bool {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 36.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(r, 4.0, crate::ui::PRIMARY());
    } else if resp.hovered() {
        p.rect_filled(r, 4.0, SURFACE_HI());
    }
    let col = if selected { crate::ui::INK() } else { TEXT() };
    p.text(
        r.left_center() + vec2(12.0, 0.0),
        Align2::LEFT_CENTER,
        label.to_uppercase(),
        FontId::monospace(12.0),
        col,
    );
    if let Some(b) = badge {
        let warn = b == "not set up";
        p.text(
            r.right_center() - vec2(12.0, 0.0),
            Align2::RIGHT_CENTER,
            b,
            FontId::monospace(10.0),
            if selected {
                Color32::from_gray(90)
            } else if warn {
                MUTED()
            } else {
                TEAL()
            },
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

#[cfg(test)]
mod tests {
    use super::Remote;

    #[test]
    fn reloads_after_retry_even_with_data() {
        let mut r: Remote<u32> = Remote::default();
        assert!(r.due()); // never loaded
        r.data = Some(1);
        r.at = Some(std::time::Instant::now());
        assert!(!r.due()); // loaded, fresh
        r.retry(); // e.g. after a save
        assert!(r.due() && r.data == Some(1)); // reloads, keeps showing the old data meanwhile
        r.missing = true;
        assert!(!r.due()); // the server doesn't have it: don't ask again
    }
}
