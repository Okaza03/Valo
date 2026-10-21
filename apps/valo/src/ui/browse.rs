//! The media library: home page, movie/series grids, search and details.
mod admin;
mod arrivals;
mod details;
mod devices;
mod discover;
mod layouts;
mod password;
mod people;

use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CursorIcon, FontId, Layout, Rect, RichText,
    ScrollArea, Sense, Stroke, Ui, UiBuilder,
};

use super::chrome::{self, Icon};
use super::{
    bold, gradient, image_cover, menu_item, mono, placeholder, primary_button, secondary_button,
    segmented, tab,
};
use super::{ACCENT, BG, ERROR, MUTED, SURFACE, SURFACE_HI, TEXT};
use crate::addons::Lookup;
use crate::api::{Client, Item, Kind, Library, Me, Missing, Mode, Unauthorized};
use crate::config::{Config, Entry, Hidden, Known, Progress, Watchlist};
use crate::images::Images;
use crate::player::Source;
use crate::ui::theme::{self, TileText, Tiles};
use crate::ui::watch::PlayItem;
use crate::ui::Action;
use crate::update::{self, Updates};
use crate::util::{fmt_runtime, fmt_time, Task};

const POSTER: egui::Vec2 = vec2(150.0, 225.0);
const WIDE: egui::Vec2 = vec2(288.0, 162.0);
const GAP: f32 = 14.0;
const SIDE: f32 = 40.0;
const HERO_ROTATE: Duration = Duration::from_secs(9);

#[derive(Clone, PartialEq)]
enum View {
    Home,
    Movies,
    Series,
    Details(String),
    /// series id, episode id
    Episode(String, String),
    /// A cast member, director or creator, by name.
    Person(String),
    /// The signed-in user's watching stats.
    Profile,
    /// Server management, for admins.
    Admin,
}

/// Background probe of a stream's file info (see `player::probe`).
enum Probe {
    Running(Task<Result<(crate::api::FileInfo, Option<f64>)>>),
    Done(crate::api::FileInfo, Option<f64>),
    Failed,
}

/// The series page's sections.
#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Episodes,
    Related,
    Details,
}

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    InProgress,
    Unwatched,
    Watched,
}

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Title,
    Year,
    Added,
    Director,
}

/// What a card or button asked for.
#[derive(Clone)]
enum Click {
    Open(String),
    /// item id, episode id (series), start from the beginning
    Play(String, Option<String>, bool),
    Nav(View),
    Back,
    /// Remove an item from Continue watching.
    Hide(String),
    /// Add an item to My list, or take it off.
    Bookmark(String),
}

struct NextUp<'a> {
    si: usize,
    ei: usize,
    resume: Option<&'a Entry>,
    started: bool,
}

struct Card {
    item: String,
    episode: Option<String>,
    title: String,
    sub: Option<String>,
    art: Option<String>,
    /// 16:9 artwork, for the styles with landscape cards.
    wide_art: Option<String>,
    /// "Movie · 2014 · 1h 26m", under the title on YouTube-style cards.
    meta: Option<String>,
    progress: Option<f32>,
    /// Has a "⋮" menu to remove it from Continue watching.
    removable: bool,
}

pub struct Browse {
    client: Client,
    user: Option<String>,
    lib: Option<Arc<Library>>,
    loading: Option<Task<Result<Library>>>,
    error: Option<String>,
    missing: Option<Missing>,
    view: View,
    history: Vec<View>,
    /// A new page is opening: show it from the top.
    to_top: bool,
    search: String,
    sort: Sort,
    /// Sort direction.
    desc: bool,
    genre: Option<String>,
    season: usize,
    hero_idx: usize,
    hero_since: Instant,
    /// The selected filter chip on the YouTube-style home page.
    chip: usize,
    row_jump: HashMap<String, f32>,
    filter: Filter,
    loaded_at: Option<Instant>,
    tab: Tab,
    watchlist: Watchlist,
    hidden: Hidden,
    /// Orders Featured and "You might also like": random, but the same all session.
    shuffle: RandomState,
    /// By stream URL.
    probes: HashMap<String, Probe>,
    /// Re-checking a remembered session's admin rights (`GET /api/me`).
    me: Option<Task<Option<Me>>>,
    /// Asking for a new password, when an admin wants one.
    password: password::Prompt,
    admin: admin::AdminState,
    devices: devices::State,
    /// Catalogs and details from the admin's Stremio addons.
    discover: discover::State,
    /// Titles each library had when the user last saw it.
    known: Known,
    /// Newly added titles, by id, waiting to be announced.
    arrivals: Vec<String>,
    /// Seasons added to series the user already knew, by series id.
    new_seasons: HashMap<String, Vec<u32>>,
    arrivals_open: bool,
}

impl Browse {
    pub fn new(ctx: &egui::Context, client: Client, user: Option<String>) -> Self {
        let mut b = Self {
            client,
            user,
            lib: None,
            loading: None,
            error: None,
            missing: None,
            view: View::Home,
            history: Vec::new(),
            to_top: false,
            search: String::new(),
            sort: Sort::Title,
            desc: false,
            genre: None,
            season: 0,
            hero_idx: 0,
            hero_since: Instant::now(),
            chip: 0,
            row_jump: HashMap::new(),
            filter: Filter::All,
            loaded_at: None,
            tab: Tab::Episodes,
            watchlist: Watchlist::load(),
            hidden: Hidden::load(),
            shuffle: RandomState::new(),
            probes: HashMap::new(),
            me: None,
            password: password::Prompt::default(),
            admin: admin::AdminState::default(),
            devices: devices::State::default(),
            discover: discover::State::default(),
            known: Known::load(),
            arrivals: Vec::new(),
            new_seasons: HashMap::new(),
            arrivals_open: false,
        };
        if b.client.token.is_some() {
            let c = b.client.clone();
            b.me = Some(Task::spawn(ctx, move || c.me()));
        }
        b.reload(ctx);
        b
    }

    pub fn reload(&mut self, ctx: &egui::Context) {
        let c = self.client.clone();
        self.loading = Some(Task::spawn(ctx, move || c.library()));
        self.discover.reload(ctx, &self.client);
        self.error = None;
        self.missing = None;
    }

    fn prefix(&self) -> String {
        crate::config::user_prefix(&self.client.base, self.user.as_deref())
    }

    fn key(&self, id: &str) -> String {
        format!("{}{id}", self.prefix())
    }

    fn go(&mut self, v: View) {
        if v != self.view {
            if matches!(
                v,
                View::Details(_)
                    | View::Episode(..)
                    | View::Person(_)
                    | View::Profile
                    | View::Admin
            ) {
                self.history.push(self.view.clone());
                if let View::Details(_) = v {
                    self.season = usize::MAX; // pick the one with the next episode
                    self.tab = Tab::Episodes;
                }
            } else {
                self.history.clear();
            }
            self.view = v;
            self.search.clear();
            self.to_top = true;
        }
    }

    fn back(&mut self) {
        self.view = self.history.pop().unwrap_or(View::Home);
        self.to_top = true;
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    pub fn has_library(&self) -> bool {
        self.lib.is_some()
    }

    /// The library has loaded, or failed to.
    pub fn settled(&self) -> bool {
        self.lib.is_some() || self.error.is_some()
    }

    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        images: &mut Images,
        progress: &Progress,
        cfg: &Config,
        updates: &Updates,
        popups: bool,
    ) -> Action {
        let mut action = Action::None;
        if let Some(r) = self.me.as_mut().and_then(|t| t.poll()) {
            self.me = None;
            if let Some(me) = r {
                self.password.open |= me.must_change_password;
                if me.admin != self.client.admin {
                    self.client.admin = me.admin;
                    action = Action::AdminChanged(me.admin);
                }
            }
        }
        if self.view == View::Admin && !self.client.admin {
            self.view = View::Home;
            self.to_top = true;
        }
        if let Some(r) = self.loading.as_mut().and_then(|t| t.poll()) {
            self.loading = None;
            match r {
                Ok(mut lib) => {
                    self.discover.on_library(&mut lib);
                    let gone = match &self.view {
                        View::Details(id) => {
                            lib.find(id).is_none() && !self.discover.remote.contains_key(id)
                        }
                        View::Episode(_, ep) => lib.find_episode(ep).is_none(),
                        _ => false,
                    };
                    if gone {
                        self.view = View::Home;
                        self.to_top = true;
                    }
                    self.note_arrivals(&lib);
                    self.lib = Some(Arc::new(lib));
                    self.loaded_at = Some(Instant::now());
                }
                Err(e) if e.downcast_ref::<Unauthorized>().is_some() => {
                    return Action::NeedLogin(self.client.clone());
                }
                Err(e) => {
                    self.missing = e.downcast_ref::<Missing>().copied();
                    self.error = Some(format!("{e:#}"));
                    self.loaded_at = Some(Instant::now()); // don't retry in a tight loop
                }
            }
        }
        self.discover.poll(ctx, &self.client, &mut self.lib);
        let every = Duration::from_secs(u64::from(cfg.refresh_minutes) * 60);
        if cfg.refresh_minutes > 0
            && self.loading.is_none()
            && self.loaded_at.is_some_and(|t| t.elapsed() > every)
        {
            self.reload(ctx);
        }

        let typing = ctx.memory(|m| m.focused().is_some());
        if !typing
            && ctx.input(|i| {
                i.key_pressed(egui::Key::Backspace)
                    || i.pointer.button_pressed(egui::PointerButton::Extra1)
            })
        {
            self.back();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("search")));
        }

        self.top_bar(ctx, &mut action, updates);
        if self.status_bar(ctx) {
            action = Action::Refresh;
        }

        if theme::spec().layout.nav == theme::Nav::Side {
            self.nav_sidebar(ctx, images);
        }
        if self.view == View::Admin && self.search.trim().is_empty() && self.lib.is_some() {
            self.admin_sidebar(ctx);
        }
        let mut click = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG()))
            .show(ctx, |ui| {
                let Some(lib) = self.lib.clone() else {
                    ui.add_space(ui.available_height() / 2.0 - 40.0);
                    ui.vertical_centered(|ui| {
                        if let Some(e) = &self.error {
                            ui.label(mono("Couldn't load your library", 13.0));
                            ui.add_space(4.0);
                            ui.label(RichText::new(e).color(ERROR()));
                            if let Some(m) = self.missing {
                                ui.scope(|ui| {
                                    ui.set_max_width(460.0);
                                    super::admin_help(ui, m, &self.client.base);
                                });
                            }
                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                let w = 300.0;
                                ui.add_space((ui.available_width() - w) / 2.0);
                                if primary_button(ui, "Try again", true).clicked() {
                                    self.reload(ctx);
                                }
                                if secondary_button(ui, "Switch server").clicked() {
                                    action = Action::ChangeServer;
                                }
                            });
                        } else {
                            ui.add(egui::Spinner::new().size(28.0).color(TEXT()));
                            ui.add_space(6.0);
                            ui.label(mono("Loading your library", 12.0).color(MUTED()));
                        }
                    });
                    return;
                };
                let scroll_id = match &self.view {
                    View::Details(id) => format!("details{id}"),
                    View::Episode(_, ep) => format!("episode{ep}"),
                    View::Person(name) => format!("person{name}"),
                    _ if !self.search.is_empty() => "search".to_owned(),
                    v => format!("{}", view_index(v)),
                };
                let mut area = ScrollArea::vertical().id_salt(scroll_id).auto_shrink(false);
                if std::mem::take(&mut self.to_top) {
                    area = area.vertical_scroll_offset(0.0);
                }
                area.show(ui, |ui| {
                    click = if !self.search.trim().is_empty() {
                        self.search_page(ui, &lib, images, progress)
                    } else {
                        match self.view.clone() {
                            View::Home => self.home(ui, &lib, images, progress, cfg),
                            View::Movies => self.grid_page(ui, &lib, Kind::Movie, images, progress),
                            View::Series => {
                                self.grid_page(ui, &lib, Kind::Series, images, progress)
                            }
                            View::Details(id) => match lib.find(&id) {
                                Some(item) if item.kind == Kind::Movie => {
                                    self.discover.want_item(item);
                                    self.movie_page(ui, &lib, item, images, progress)
                                }
                                Some(item) => {
                                    self.discover.want_item(item);
                                    self.series_page(ui, &lib, item, images, progress)
                                }
                                None => match self.discover.remote.get(&id).cloned() {
                                    Some(item) => self.remote_page(ui, &item, images),
                                    None => Some(Click::Nav(View::Home)),
                                },
                            },
                            View::Episode(_, ep) => match lib.find_episode(&ep) {
                                Some((s, si, ei)) => {
                                    self.episode_page(ui, s, si, ei, images, progress)
                                }
                                None => Some(Click::Nav(View::Home)),
                            },
                            View::Person(name) => {
                                self.person_page(ui, &lib, &name, images, progress)
                            }
                            View::Profile => {
                                self.profile_page(ui, &lib, images, progress, &mut action)
                            }
                            View::Admin => self.admin_page(ui, &lib, images, &mut action),
                        }
                    };
                    ui.add_space(40.0);
                });
            });

        self.password_ui(ctx, popups, &mut action);
        let popups = popups && !self.password.open;
        if let Some(lib) = self.lib.clone() {
            if let Some(c) = self.arrivals_ui(ctx, &lib, images, popups) {
                click = Some(c);
            }
        }

        match click {
            Some(Click::Open(id)) => self.go(View::Details(id)),
            Some(Click::Nav(v)) => self.go(v),
            Some(Click::Back) => self.back(),
            Some(Click::Hide(id)) => {
                let key = self.key(&id);
                self.hidden.hide(&key);
            }
            Some(Click::Bookmark(id)) => {
                let key = self.key(&id);
                self.watchlist.toggle(&key);
            }
            Some(Click::Play(id, ep, restart)) => {
                if let Some(lib) = self.lib.clone() {
                    if let Some(a) =
                        self.play(&lib, &id, ep.as_deref(), restart || !cfg.resume, progress)
                    {
                        action = a;
                    }
                }
            }
            None => {}
        }
        if self.loading.is_none() {
            ctx.request_repaint_after(Duration::from_secs(1)); // hero rotation
        }
        action
    }

    // ------------------------------------------------------------ chrome

    fn top_bar(&mut self, ctx: &egui::Context, action: &mut Action, updates: &Updates) {
        let mut nav = None;
        let mut back = false;
        let in_details = matches!(
            self.view,
            View::Details(_) | View::Episode(..) | View::Person(_) | View::Profile | View::Admin
        );
        let (current, searching) = (self.view.clone(), !self.search.is_empty());
        let name = self.client.info.name.clone();
        let mut open: Option<View> = None;
        chrome::title_bar(
            ctx,
            &name,
            |ui| {
                if in_details
                    && tab(ui, "‹ Back", false)
                        .on_hover_text("Backspace")
                        .clicked()
                {
                    back = true;
                }
                let tabs: &[(&str, View)] = if theme::spec().layout.nav == theme::Nav::Top {
                    &[
                        ("Home", View::Home),
                        ("Movies", View::Movies),
                        ("TV Shows", View::Series),
                    ]
                } else {
                    &[] // they're in the sidebar
                };
                for (label, v) in tabs.iter().cloned() {
                    let selected = !searching && current == v;
                    if tab(ui, label, selected).clicked() {
                        nav = Some(v);
                    }
                }
            },
            |ui| {
                self.profile_menu(ui, action, &mut open);
                chrome::vsep(ui);
                if chrome::icon_button(ui, Icon::Settings, "Settings").clicked() {
                    *action = Action::OpenSettings;
                }
                chrome::vsep(ui);
                match chrome::update_button(ui, updates) {
                    Some(update::Click::Check) => *action = Action::CheckUpdates,
                    Some(update::Click::ShowUpdate) => *action = Action::ShowUpdate,
                    None => {}
                }
                chrome::vsep(ui);
                ui.add_space(10.0);
                let w = (ui.available_width() - 40.0).clamp(90.0, 240.0);
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .id(egui::Id::new("search"))
                        .hint_text("Search movies & shows")
                        .desired_width(w)
                        .margin(egui::Margin::symmetric(10, 6)),
                );
                if r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.search.clear();
                    r.surrender_focus();
                }
                ui.add_space(10.0);
            },
        );
        if back {
            self.back();
        }
        if open.is_some() {
            nav = open;
        }
        if let Some(v) = nav {
            self.go(v);
            self.search.clear();
        }
    }

    fn profile_menu(&mut self, ui: &mut Ui, action: &mut Action, go: &mut Option<View>) {
        let name = self.user.clone().unwrap_or_else(|| "Guest".to_owned());
        let label =
            ui.fonts(|f| f.layout_no_wrap(name.to_uppercase(), FontId::monospace(11.5), TEXT()));
        let (r, resp) =
            ui.allocate_exact_size(vec2(label.size().x + 72.0, chrome::BAR_H), Sense::click());
        let id = ui.make_persistent_id("profile_menu");
        let open = ui.memory(|m| m.is_popup_open(id));
        let p = ui.painter();
        if resp.hovered() || open {
            p.rect_filled(r, 0.0, SURFACE_HI());
        }
        chrome::avatar(p, pos2(r.left() + 26.0, r.center().y), 12.0, &name);
        p.galley(
            pos2(r.left() + 46.0, r.center().y - label.size().y / 2.0),
            label,
            TEXT(),
        );
        let c = pos2(r.right() - 15.0, r.center().y);
        p.add(egui::Shape::convex_polygon(
            vec![
                c + vec2(-4.0, -2.0),
                c + vec2(4.0, -2.0),
                c + vec2(0.0, 3.0),
            ],
            MUTED(),
            Stroke::NONE,
        ));
        let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
        if resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(id));
        }
        egui::popup::popup_below_widget(
            ui,
            id,
            &resp,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(250.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
                    chrome::avatar(ui.painter(), r.center(), 17.0, &name);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(bold(&name, 15.0));
                        ui.label(mono(&self.client.info.name, 10.5).color(MUTED()));
                    });
                });
                ui.separator();
                let mut pick = |ui: &mut Ui, text: &str, a: Action| {
                    if menu_item(ui, text) {
                        *action = a;
                        ui.memory_mut(|m| m.close_popup());
                    }
                };
                if menu_item(ui, "Profile") {
                    *go = Some(View::Profile);
                    ui.memory_mut(|m| m.close_popup());
                }
                if self.client.admin && menu_item(ui, "Admin") {
                    *go = Some(View::Admin);
                    ui.memory_mut(|m| m.close_popup());
                }
                ui.separator();
                if self.client.token.is_some() {
                    pick(ui, "Sign out", Action::SignOut(self.client.clone()));
                }
                pick(ui, "Quit", Action::Quit);
            },
        );
    }

    /// True when "Library updated …" was clicked.
    fn status_bar(&self, ctx: &egui::Context) -> bool {
        let host = self
            .client
            .base
            .split("://")
            .nth(1)
            .unwrap_or(&self.client.base);
        let when = if self.loading.is_some() {
            "Loading library…".to_owned()
        } else {
            match self.loaded_at.map(|t| t.elapsed().as_secs() / 60) {
                Some(0) => "Library updated just now".to_owned(),
                Some(1) => "Library updated 1 min ago".to_owned(),
                Some(m) => format!("Library updated {m} min ago"),
                None => "Library not loaded".to_owned(),
            }
        };
        let left = format!("{when}  ·  {host}");
        let right = match &self.lib {
            Some(l) => {
                let eps: usize = l
                    .series
                    .iter()
                    .flat_map(|s| &s.seasons)
                    .map(|s| s.episodes.len())
                    .sum();
                let kind = if self.client.info.mode == Mode::Directory {
                    "  ·  file server"
                } else {
                    ""
                };
                format!(
                    "{} movies  ·  {} series  ·  {eps} episodes{kind}  ·  ",
                    l.movies.len(),
                    l.series.len()
                )
            }
            None => String::new(),
        };
        let right = format!("{right}Valo {}", update::VERSION);
        chrome::status_bar(ctx, &left, &right, self.loading.is_none())
    }

    // ---------------------------------------------------------------- home

    fn home(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        images: &mut Images,
        progress: &Progress,
        cfg: &Config,
    ) -> Option<Click> {
        let mut click = None;
        if lib.movies.is_empty() && lib.series.is_empty() {
            ui.add_space(120.0);
            ui.vertical_centered(|ui| {
                ui.label(mono("Your library is empty", 13.0));
                ui.label(
                    RichText::new("No movies or series were found on this server.").color(MUTED()),
                );
            });
            return None;
        }

        let lay = &theme::spec().layout;
        if lay.home_grid {
            return self.home_grid(ui, lib, images, progress);
        }
        let featured = featured(lib, &self.shuffle);
        if !featured.is_empty() && lay.hero != theme::Hero::None {
            if self.hero_since.elapsed() > HERO_ROTATE {
                self.hero_idx = (self.hero_idx + 1) % featured.len();
                self.hero_since = Instant::now();
            }
            self.hero_idx %= featured.len();
            let item = featured[self.hero_idx];
            let at = (self.hero_idx, featured.len());
            let (c, nav) = match lay.hero {
                theme::Hero::FullBleed => self.hero_full(ui, item, images, progress, at),
                theme::Hero::Carousel => {
                    self.hero_carousel(ui, &featured, self.hero_idx, images, progress)
                }
                _ => self.hero(ui, item, images, progress, Some(at)),
            };
            click = click.or(c);
            if let Some(d) = nav {
                self.hero_idx =
                    (self.hero_idx as isize + d).rem_euclid(featured.len() as isize) as usize;
                self.hero_since = Instant::now();
            }
        }

        if lay.genre_tiles {
            click = click.or(self.genre_tiles(ui, lib));
        }
        let cont = self.continue_cards(lib, progress);
        if !cont.is_empty() {
            click = click.or(self.row(ui, "Continue watching", &cont, true, None, images));
        }
        let prefix = self.prefix();
        let mine: Vec<&Item> = self
            .watchlist
            .ids(&prefix)
            .filter_map(|id| lib.find(id))
            .collect();
        if !mine.is_empty() {
            let cards = self.poster_cards(mine.into_iter(), progress);
            click = click.or(self.row(ui, "My list", &cards, false, None, images));
        }
        if lib.all().any(|i| i.added.is_some()) {
            let mut recent: Vec<&Item> = lib.all().filter(|i| i.added.is_some()).collect();
            recent.sort_by_key(|i| std::cmp::Reverse(i.added));
            let cards = self.poster_cards(recent.into_iter().take(24), progress);
            click = click.or(self.row(ui, "Recently added", &cards, false, None, images));
        }
        // best rated first, from the server's scores
        let mut rated: Vec<&Item> = lib.all().filter(|i| i.score.is_some()).collect();
        if rated.len() >= 5 {
            rated = shuffled(rated.into_iter(), "top", &self.shuffle); // ties in a random order
            rated.sort_by(|a, b| b.score.unwrap_or(0.0).total_cmp(&a.score.unwrap_or(0.0)));
            if lay.top10 {
                click = click.or(self.top10_row(ui, &rated, images));
            } else {
                let cards = self.poster_cards(rated.into_iter().take(24), progress);
                click = click.or(self.row(ui, "Top rated", &cards, false, None, images));
            }
        }
        // the admin's addon catalogs; titles we have play, the rest can be requested
        let rows: Vec<(String, Vec<Card>)> = self
            .discover
            .shown
            .iter()
            .map(|(title, ids)| {
                let items = ids
                    .iter()
                    .filter_map(|id| lib.find(id).or_else(|| self.discover.remote.get(id)));
                let mut cards = self.poster_cards(items, progress);
                for c in &mut cards {
                    if self.discover.remote.contains_key(&c.item) {
                        c.sub = Some(match c.sub.take() {
                            Some(y) => format!("{y} · not on server"),
                            None => "Not on server".into(),
                        });
                    }
                }
                (title.clone(), cards)
            })
            .collect();
        for (title, cards) in rows {
            click = click.or(self.row(ui, &title, &cards, false, None, images));
        }
        // the rest in a random order (the same all session), so the home page isn't A to Z
        if !lib.movies.is_empty() {
            let movies = shuffled(lib.movies.iter(), "movies", &self.shuffle);
            let cards = self.poster_cards(movies.into_iter().take(30), progress);
            click = click.or(self.row(ui, "Movies", &cards, false, Some(View::Movies), images));
        }
        if !lib.series.is_empty() {
            let series = shuffled(lib.series.iter(), "series", &self.shuffle);
            let cards = self.poster_cards(series.into_iter().take(30), progress);
            click = click.or(self.row(ui, "TV shows", &cards, false, Some(View::Series), images));
        }
        let genres = if cfg.genre_rows {
            lib.genres()
        } else {
            Vec::new()
        };
        for g in genres.into_iter().take(8) {
            let items = shuffled(
                lib.all().filter(|i| i.genres.contains(&g)),
                &g,
                &self.shuffle,
            );
            let cards = self.poster_cards(items.into_iter().take(30), progress);
            if cards.len() >= 2 {
                click = click.or(self.row(ui, &g, &cards, false, None, images));
            }
        }
        click
    }

    /// Big artwork banner. Returns a click and, for the carousel, a step (-1/+1).
    fn hero(
        &mut self,
        ui: &mut Ui,
        item: &Item,
        images: &mut Images,
        progress: &Progress,
        carousel: Option<(usize, usize)>,
    ) -> (Option<Click>, Option<isize>) {
        let w = ui.available_width();
        let h = (w * 0.40).clamp(300.0, 560.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        // a featured title opens from anywhere on the banner, like More info
        let hit = carousel.map(|_| ui.interact(rect, ui.id().with("banner"), Sense::click()));
        let p = ui.painter_at(rect);
        let art = item.backdrop.as_ref().or(item.poster.as_ref());
        match art.and_then(|u| images.get(u)) {
            Some(t) => {
                // artwork sits on the right like a TV banner; text on the left
                let ar = Rect::from_min_max(pos2(rect.left() + w * 0.25, rect.top()), rect.max);
                p.rect_filled(rect, 0.0, Color32::BLACK);
                image_cover(&p, ar, &t, 0, Color32::WHITE);
                let fade =
                    Rect::from_min_max(ar.min, pos2(ar.left() + ar.width() * 0.45, ar.bottom()));
                gradient(
                    &p,
                    fade,
                    [BG(), Color32::TRANSPARENT, Color32::TRANSPARENT, BG()],
                );
                p.rect_filled(
                    Rect::from_min_max(rect.min, pos2(ar.left() + 1.0, rect.bottom())),
                    0.0,
                    BG(),
                );
            }
            None => {
                let c = crate::util::color_for(&item.title);
                gradient(&p, rect, [BG(), c.gamma_multiply(0.5), c, BG()]);
            }
        }
        let bottom = Rect::from_min_max(pos2(rect.left(), rect.bottom() - h * 0.35), rect.max);
        gradient(
            &p,
            bottom,
            [Color32::TRANSPARENT, Color32::TRANSPARENT, BG(), BG()],
        );

        let mut click = None;
        let text_w = (w * 0.45).clamp(320.0, 640.0).min(w - 2.0 * SIDE);
        let inner = Rect::from_min_size(
            pos2(rect.left() + SIDE, rect.top() + 24.0),
            vec2(text_w, h - 56.0),
        );
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(inner)
                .layout(Layout::bottom_up(Align::Min)),
        );
        let ui = &mut child;
        ui.horizontal(|ui| {
            let (label, restart) = self.play_label(item, progress);
            if crate::ui::primary_button_icon(ui, Icon::Play, &label, true).clicked() {
                click = Some(Click::Play(item.id.clone(), None, false));
            }
            if restart && secondary_button(ui, "Start over").clicked() {
                click = Some(Click::Play(item.id.clone(), None, true));
            }
            if carousel.is_some() && secondary_button(ui, "More info").clicked() {
                click = Some(Click::Open(item.id.clone()));
            }
        });
        ui.add_space(8.0);
        if !item.overview.is_empty() {
            let max = if carousel.is_some() { 260 } else { 900 };
            ui.add(
                egui::Label::new(
                    RichText::new(shorten(&item.overview, max))
                        .size(15.0)
                        .color(TEXT()),
                )
                .wrap(),
            );
            ui.add_space(4.0);
        }
        ui.label(mono(meta_line(item), 11.5).color(MUTED()));
        ui.add_space(2.0);
        ui.add(egui::Label::new(bold(&item.title, 40.0)).wrap());
        if carousel.is_some() {
            ui.label(
                mono(
                    if item.kind == Kind::Movie {
                        "Featured movie"
                    } else {
                        "Featured series"
                    },
                    11.0,
                )
                .color(ACCENT()),
            );
        }

        let mut step = None;
        if let Some((cur, n)) = carousel.filter(|(_, n)| *n > 1) {
            let pr = ui.painter();
            let y = rect.bottom() - 22.0;
            let x0 = rect.right() - SIDE - n as f32 * 16.0;
            for i in 0..n {
                let c = pos2(x0 + i as f32 * 16.0, y);
                let r = Rect::from_center_size(c, vec2(if i == cur { 14.0 } else { 6.0 }, 3.0));
                pr.rect_filled(
                    r,
                    0.0,
                    if i == cur {
                        TEXT()
                    } else {
                        MUTED().gamma_multiply(0.6)
                    },
                );
            }
            if resp.hovered() || resp.contains_pointer() {
                for (dir, x) in [(-1isize, rect.left() + 18.0), (1, rect.right() - 18.0)] {
                    let r = Rect::from_center_size(pos2(x, rect.center().y), vec2(32.0, 64.0));
                    let a = ui.interact(r, ui.id().with(("hero_arrow", dir)), Sense::click());
                    let col = Color32::WHITE.gamma_multiply(if a.hovered() { 1.0 } else { 0.7 });
                    pr.rect_filled(
                        r,
                        2.0,
                        Color32::from_black_alpha(if a.hovered() { 160 } else { 80 }),
                    );
                    pr.text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        if dir < 0 { "‹" } else { "›" },
                        FontId::proportional(30.0),
                        col,
                    );
                    if a.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        step = Some(dir);
                    }
                }
            }
        }
        if let Some(hit) = hit {
            if click.is_none() && hit.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                click = Some(Click::Open(item.id.clone()));
            }
        }
        (click, step)
    }

    fn play_label(&self, item: &Item, progress: &Progress) -> (String, bool) {
        match item.kind {
            Kind::Movie => match progress.get(&self.key(&item.id)) {
                Some(e) if e.in_progress() => (format!("Resume from {}", fmt_time(e.pos)), true),
                Some(e) if e.watched() => ("Watch Again".into(), false),
                _ => ("Play".into(), false),
            },
            Kind::Series => match self.next_up(item, progress) {
                Some(n) => {
                    let s = &item.seasons[n.si];
                    let ep = &s.episodes[n.ei];
                    let verb = match (n.resume, n.started) {
                        (Some(_), _) => "Resume",
                        (None, true) => "Continue",
                        (None, false) => "Play",
                    };
                    (
                        format!("{verb} S{:02} E{:02}", s.number, ep.number),
                        n.started,
                    )
                }
                None => ("Play".into(), false),
            },
        }
    }

    /// The episode to continue with: the one being watched, or the one after the last finished.
    fn next_up<'a>(&self, s: &Item, progress: &'a Progress) -> Option<NextUp<'a>> {
        let eps: Vec<(usize, usize, &str)> = s
            .seasons
            .iter()
            .enumerate()
            .flat_map(|(si, season)| {
                season
                    .episodes
                    .iter()
                    .enumerate()
                    .map(move |(ei, e)| (si, ei, e.id.as_str()))
            })
            .collect();
        let at = |i: usize, resume, started| NextUp {
            si: eps[i].0,
            ei: eps[i].1,
            resume,
            started,
        };
        if eps.is_empty() {
            return None;
        }
        let last_seen = eps
            .iter()
            .enumerate()
            .filter_map(|(i, (_, _, id))| progress.get(&self.key(id)).map(|e| (i, e)))
            .max_by_key(|(_, e)| e.updated);
        Some(match last_seen {
            Some((i, e)) if e.in_progress() => at(i, Some(e), true),
            Some((i, _)) if i + 1 < eps.len() => at(i + 1, None, true),
            Some(_) => at(0, None, true),
            None => at(0, None, false),
        })
    }

    fn continue_cards(&self, lib: &Library, progress: &Progress) -> Vec<Card> {
        let prefix = self.prefix();
        let mut seen_series = Vec::new();
        let mut out = Vec::new();
        for (id, e) in progress.recent(&prefix) {
            if out.len() >= 20 {
                break;
            }
            if let Some(m) = lib.movies.iter().find(|m| m.id == id) {
                if e.in_progress() && !self.hidden.hides(&self.key(&m.id), e.updated) {
                    out.push(Card {
                        item: m.id.clone(),
                        episode: None,
                        title: m.title.clone(),
                        sub: Some(format!("{} left", fmt_runtime(e.dur - e.pos))),
                        art: m.thumb.clone().or(m.backdrop.clone()).or(m.poster.clone()),
                        wide_art: None,
                        meta: Some(short_meta(m)),
                        progress: Some(e.fraction()),
                        removable: true,
                    });
                }
            } else if let Some((s, _, _)) = lib.find_episode(id) {
                if seen_series.contains(&s.id) {
                    continue;
                }
                seen_series.push(s.id.clone());
                // newest first, so `e` is the series' latest watch
                if self.hidden.hides(&self.key(&s.id), e.updated) {
                    continue;
                }
                if let Some(n) = self.next_up(s, progress) {
                    let season = &s.seasons[n.si];
                    let ep = &season.episodes[n.ei];
                    out.push(Card {
                        item: s.id.clone(),
                        episode: Some(ep.id.clone()),
                        title: s.title.clone(),
                        sub: Some(format!("S{} E{} · {}", season.number, ep.number, ep.title)),
                        art: ep
                            .still
                            .clone()
                            .or(s.thumb.clone())
                            .or(s.backdrop.clone())
                            .or(s.poster.clone()),
                        wide_art: None,
                        meta: Some(format!("{} · S{} E{}", s.title, season.number, ep.number)),
                        progress: Some(n.resume.map_or(0.0, Entry::fraction)),
                        removable: true,
                    });
                }
            }
        }
        out
    }

    fn poster_cards<'a>(
        &self,
        items: impl Iterator<Item = &'a Item>,
        progress: &Progress,
    ) -> Vec<Card> {
        items
            .map(|i| Card {
                item: i.id.clone(),
                episode: None,
                title: i.title.clone(),
                sub: i.year.map(|y| y.to_string()),
                art: i.poster.clone(),
                wide_art: i.thumb.clone().or(i.backdrop.clone()),
                meta: Some(short_meta(i)),
                progress: (i.kind == Kind::Movie)
                    .then(|| {
                        progress
                            .get(&self.key(&i.id))
                            .filter(|e| e.in_progress())
                            .map(Entry::fraction)
                    })
                    .flatten(),
                removable: false,
            })
            .collect()
    }

    /// A titled, horizontally scrolling row of cards.
    fn row(
        &mut self,
        ui: &mut Ui,
        title: &str,
        cards: &[Card],
        wide: bool,
        all: Option<View>,
        images: &mut Images,
    ) -> Option<Click> {
        let mut click = None;
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(SIDE);
            ui.label(crate::ui::heading(title));
            if let Some(v) = all {
                ui.add_space(10.0);
                let r = ui.add(
                    egui::Label::new(mono("See all", 11.0).color(MUTED()))
                        .sense(Sense::click())
                        .selectable(false),
                );
                if r.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    click = Some(Click::Nav(v));
                }
            }
        });
        ui.add_space(6.0);
        let mut area = ScrollArea::horizontal()
            .id_salt(("row", title))
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
        if let Some(x) = self.row_jump.remove(title) {
            area = area.horizontal_scroll_offset(x);
        }
        let out = area.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                ui.add_space(SIDE);
                for c in cards {
                    let marked = self.watchlist.contains(&self.key(&c.item));
                    if let Some(k) = card(ui, c, wide, marked, images) {
                        click = Some(k);
                    }
                }
                ui.add_space(SIDE);
            });
        });
        // paging arrows
        let r = out.inner_rect;
        let max = (out.content_size.x - r.width()).max(0.0);
        if ui.rect_contains_pointer(r) && max > 0.0 {
            let x = out.state.offset.x;
            for (dir, show, ax) in [
                (-1.0, x > 1.0, r.left() + 16.0),
                (1.0, x < max - 1.0, r.right() - 16.0),
            ] {
                if !show {
                    continue;
                }
                let ar = Rect::from_center_size(
                    pos2(ax, r.center().y - 10.0),
                    vec2(30.0, r.height() - 40.0),
                );
                let a = ui.interact(
                    ar,
                    ui.id().with(("row_arrow", title, dir as i32)),
                    Sense::click(),
                );
                let p = ui.painter();
                p.rect_filled(
                    ar,
                    2.0,
                    Color32::from_black_alpha(if a.hovered() { 190 } else { 130 }),
                );
                p.text(
                    ar.center(),
                    Align2::CENTER_CENTER,
                    if dir < 0.0 { "‹" } else { "›" },
                    FontId::proportional(30.0),
                    Color32::WHITE,
                );
                if a.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    self.row_jump.insert(
                        title.to_owned(),
                        (x + dir * (r.width() - 2.0 * SIDE)).clamp(0.0, max),
                    );
                }
            }
        }
        click
    }

    // --------------------------------------------------------- grid pages

    fn grid_page(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        kind: Kind,
        images: &mut Images,
        progress: &Progress,
    ) -> Option<Click> {
        let all = if kind == Kind::Movie {
            &lib.movies
        } else {
            &lib.series
        };
        let mut genres: Vec<&String> = all.iter().flat_map(|i| &i.genres).collect();
        genres.sort();
        genres.dedup();
        if self.genre.as_ref().is_some_and(|g| !genres.contains(&g)) {
            self.genre = None;
        }
        let in_genre: Vec<&Item> = all
            .iter()
            .filter(|i| self.genre.as_ref().is_none_or(|g| i.genres.contains(g)))
            .collect();
        let status: Vec<Filter> = in_genre.iter().map(|i| self.status(i, progress)).collect();
        let count = |f: Filter| status.iter().filter(|s| **s == f).count();
        let counts = [
            count(Filter::InProgress),
            count(Filter::Unwatched),
            count(Filter::Watched),
        ];
        let mut items: Vec<&Item> = in_genre
            .iter()
            .zip(&status)
            .filter(|(_, s)| self.filter == Filter::All || **s == self.filter)
            .map(|(i, _)| *i)
            .collect();
        let desc = self.desc;
        match self.sort {
            // the library comes sorted by title
            Sort::Title if desc => items.reverse(),
            Sort::Title => {}
            Sort::Year => sort_by(&mut items, desc, |i| i.year),
            Sort::Added => sort_by(&mut items, desc, |i| i.added),
            Sort::Director => sort_by(&mut items, desc, |i| maker(i).map(str::to_lowercase)),
        }
        let maker_label = if kind == Kind::Movie {
            "Director"
        } else {
            "Creator"
        };

        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(SIDE - 22.0);
            segmented(
                ui,
                &mut self.filter,
                &[
                    (Filter::All, "All", Some(in_genre.len())),
                    (Filter::InProgress, "In progress", Some(counts[0])),
                    (Filter::Unwatched, "Unwatched", Some(counts[1])),
                    (Filter::Watched, "Watched", Some(counts[2])),
                ],
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(SIDE - 22.0);
                if !genres.is_empty() {
                    egui::ComboBox::from_id_salt("genre")
                        .width(170.0)
                        .selected_text(self.genre.clone().unwrap_or_else(|| "All genres".into()))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.genre, None, "All genres");
                            for g in genres {
                                ui.selectable_value(&mut self.genre, Some(g.clone()), g);
                            }
                        });
                    ui.label(mono("Genre", 11.0).color(MUTED()));
                    ui.add_space(14.0);
                }
                let (icon, arrow) = if self.desc {
                    (Icon::ArrowDown, "Descending")
                } else {
                    (Icon::ArrowUp, "Ascending")
                };
                if crate::ui::small_button_icon(ui, icon, arrow)
                    .on_hover_text("Sort direction")
                    .clicked()
                {
                    self.desc = !self.desc;
                }
                ui.add_space(6.0);
                let before = self.sort;
                egui::ComboBox::from_id_salt("sort")
                    .width(170.0)
                    .selected_text(match self.sort {
                        Sort::Title => "A – Z",
                        Sort::Year => "Release year",
                        Sort::Added => "Recently added",
                        Sort::Director => maker_label,
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.sort, Sort::Title, "A – Z");
                        ui.selectable_value(&mut self.sort, Sort::Year, "Release year");
                        if all.iter().any(|i| i.added.is_some()) {
                            ui.selectable_value(&mut self.sort, Sort::Added, "Recently added");
                        }
                        if all.iter().any(|i| maker(i).is_some()) {
                            ui.selectable_value(&mut self.sort, Sort::Director, maker_label);
                        }
                    });
                if self.sort != before {
                    // newest first, names A to Z
                    self.desc = matches!(self.sort, Sort::Year | Sort::Added);
                }
                ui.label(mono("Sort", 11.0).color(MUTED()));
            });
        });
        ui.add_space(20.0);
        if items.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| ui.label(mono("Nothing here", 12.0).color(MUTED())));
            return None;
        }
        ui.add_space(16.0);
        let mut cards = self.poster_cards(items.iter().copied(), progress);
        if self.sort == Sort::Director {
            for (c, i) in cards.iter_mut().zip(&items) {
                c.sub = maker(i).map(str::to_owned);
            }
        }
        let marked = |id: &str| self.watchlist.contains(&self.key(id));
        grid(ui, &cards, &marked, images)
    }

    fn status(&self, i: &Item, progress: &Progress) -> Filter {
        match i.kind {
            Kind::Movie => match progress.get(&self.key(&i.id)) {
                Some(e) if e.watched() => Filter::Watched,
                Some(e) if e.in_progress() => Filter::InProgress,
                _ => Filter::Unwatched,
            },
            Kind::Series => {
                let eps: Vec<Option<&Entry>> = i
                    .seasons
                    .iter()
                    .flat_map(|s| &s.episodes)
                    .map(|e| progress.get(&self.key(&e.id)))
                    .collect();
                if !eps.is_empty() && eps.iter().all(|e| e.is_some_and(Entry::watched)) {
                    Filter::Watched
                } else if eps.iter().any(|e| e.is_some_and(|e| e.pos > 30.0)) {
                    Filter::InProgress
                } else {
                    Filter::Unwatched
                }
            }
        }
    }

    fn search_page(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        images: &mut Images,
        progress: &Progress,
    ) -> Option<Click> {
        let q = self.search.trim().to_lowercase();
        let hits: Vec<&Item> = lib
            .all()
            .filter(|i| {
                i.title.to_lowercase().contains(&q)
                    || i.genres.iter().any(|g| g.to_lowercase() == q)
                    || i.year.is_some_and(|y| y.to_string() == q)
                    || makers(i).any(|d| d.to_lowercase().contains(&q))
            })
            .collect();
        let mut people: Vec<&str> = hits
            .iter()
            .flat_map(|i| makers(i))
            .filter(|d| d.to_lowercase().contains(&q))
            .collect();
        people.sort_by_key(|d| d.to_lowercase());
        people.dedup();
        ui.add_space(24.0);
        ui.horizontal(|ui| {
            ui.add_space(SIDE);
            ui.label(mono(format!("Results for “{}”", self.search.trim()), 13.0));
            ui.label(mono(hits.len().to_string(), 13.0).color(MUTED()));
        });
        let mut click = None;
        if !people.is_empty() {
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.add_space(SIDE);
                ui.label(mono("Directors & creators", 11.0).color(MUTED()));
                ui.add_space(6.0);
                for d in people {
                    if secondary_button(ui, d).clicked() {
                        click = Some(Click::Nav(View::Person(d.to_owned())));
                    }
                }
            });
        }
        ui.add_space(16.0);
        if hits.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| ui.label(mono("Nothing matches", 12.0).color(MUTED())));
            return None;
        }
        let cards = self.poster_cards(hits.into_iter(), progress);
        let marked = |id: &str| self.watchlist.contains(&self.key(id));
        grid(ui, &cards, &marked, images).or(click)
    }

    // -------------------------------------------------------------- details

    // ----------------------------------------------------------- playback

    fn play(
        &self,
        lib: &Library,
        id: &str,
        ep: Option<&str>,
        restart: bool,
        progress: &Progress,
    ) -> Option<Action> {
        let item = lib.find(id)?;
        let auth = self.client.auth_header();
        let resume = |key: &str| {
            if restart {
                return 0.0;
            }
            progress
                .get(key)
                .filter(|e| e.in_progress())
                .map_or(0.0, |e| (e.pos - 3.0).max(0.0))
        };
        match item.kind {
            Kind::Movie => {
                let key = self.key(&item.id);
                Some(Action::Play(vec![PlayItem {
                    title: item.title.clone(),
                    subtitle: [
                        item.year.map(|y| y.to_string()),
                        item.duration.filter(|d| *d > 0.0).map(fmt_runtime),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · "),
                    episode: false,
                    source: Source::Url {
                        url: item.stream.clone()?,
                        auth,
                    },
                    start: resume(&key),
                    key: Some(key),
                    subtitles: item.subtitles.clone(),
                    addons: Lookup::movie(&self.client, item.imdb_id.as_deref()),
                }]))
            }
            Kind::Series => {
                let (si, ei) = match ep {
                    Some(ep) => lib.find_episode(ep).map(|(_, si, ei)| (si, ei))?,
                    None if restart => (0, 0),
                    None => self.next_up(item, progress).map(|n| (n.si, n.ei))?,
                };
                // queue this episode and everything after it
                let mut queue = Vec::new();
                for (sj, season) in item.seasons.iter().enumerate().skip(si) {
                    for (ej, e) in season.episodes.iter().enumerate() {
                        if sj == si && ej < ei {
                            continue;
                        }
                        let key = self.key(&e.id);
                        queue.push(PlayItem {
                            title: item.title.clone(),
                            subtitle: format!(
                                "S{:02}E{:02} · {}",
                                season.number, e.number, e.title
                            ),
                            episode: true,
                            source: Source::Url {
                                url: e.stream.clone(),
                                auth: auth.clone(),
                            },
                            start: if queue.is_empty() { resume(&key) } else { 0.0 },
                            key: Some(key),
                            subtitles: e.subtitles.clone(),
                            addons: Lookup::episode(
                                &self.client,
                                item.imdb_id.as_deref(),
                                season.number,
                                e.number,
                            ),
                        });
                    }
                }
                Some(Action::Play(queue))
            }
        }
    }
}

fn view_index(v: &View) -> usize {
    match v {
        View::Home => 0,
        View::Movies => 1,
        View::Series => 2,
        View::Details(_) | View::Episode(..) | View::Person(_) => 3,
        View::Profile => 4,
        View::Admin => 5,
    }
}

/// `items` in a random order that stays the same all session; `row` gives
/// each row its own order.
fn shuffled<'a>(
    items: impl Iterator<Item = &'a Item>,
    row: &str,
    shuffle: &RandomState,
) -> Vec<&'a Item> {
    let mut v: Vec<&Item> = items.collect();
    v.sort_by_cached_key(|i| shuffle.hash_one((row, &i.id)));
    v
}

/// Sort `items` by `key`, ascending or descending; ties keep their order and
/// items without a key go last either way.
fn sort_by<K: Ord>(items: &mut [&Item], desc: bool, key: impl Fn(&Item) -> Option<K>) {
    let mut keyed: Vec<(Option<K>, &Item)> = items.iter().map(|i| (key(i), *i)).collect();
    keyed.sort_by(|(a, _), (b, _)| match (a, b) {
        (Some(a), Some(b)) if desc => b.cmp(a),
        (Some(a), Some(b)) => a.cmp(b),
        _ => a.is_none().cmp(&b.is_none()),
    });
    for (slot, (_, i)) in items.iter_mut().zip(keyed) {
        *slot = i;
    }
}

/// The movie's director, else the series' first creator.
fn maker(i: &Item) -> Option<&str> {
    i.director
        .as_deref()
        .or(i.creators.first().map(String::as_str))
}

/// Everyone who directed or created `i` ("A, B" directors are split).
fn makers(i: &Item) -> impl Iterator<Item = &str> {
    i.director
        .iter()
        .flat_map(|d| d.split(','))
        .chain(i.creators.iter().map(String::as_str))
        .map(str::trim)
        .filter(|d| !d.is_empty())
}

/// Items for the home banner, in random order: ones with a backdrop first.
fn featured<'a>(lib: &'a Library, shuffle: &RandomState) -> Vec<&'a Item> {
    let mut v: Vec<&Item> = lib.all().filter(|i| i.backdrop.is_some()).collect();
    if v.is_empty() {
        v = lib.all().filter(|i| i.poster.is_some()).collect();
    }
    if v.is_empty() {
        v = lib.all().collect();
    }
    v.sort_by_cached_key(|i| shuffle.hash_one(&i.id));
    v.truncate(6);
    v
}

fn meta_line(item: &Item) -> String {
    let mut parts = Vec::new();
    if let Some(y) = item.year {
        parts.push(y.to_string());
    }
    match item.kind {
        Kind::Movie => {
            if let Some(d) = item.duration.filter(|d| *d > 0.0) {
                parts.push(fmt_runtime(d));
            }
        }
        Kind::Series => {
            let n = item.seasons.len();
            let eps: usize = item.seasons.iter().map(|s| s.episodes.len()).sum();
            parts.push(format!("{n} season{}", if n == 1 { "" } else { "s" }));
            parts.push(format!("{eps} episode{}", if eps == 1 { "" } else { "s" }));
        }
    }
    if let Some(r) = &item.rating {
        parts.push(r.clone());
    }
    if !item.genres.is_empty() {
        parts.push(
            item.genres
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    parts.join("  ·  ")
}

/// "Movie · 2014 · 1h 26m", "Series · 2019 · 3 seasons".
fn short_meta(i: &Item) -> String {
    let mut parts = vec![if i.kind == Kind::Movie {
        "Movie"
    } else {
        "Series"
    }
    .to_owned()];
    parts.extend(i.year.map(|y| y.to_string()));
    match i.kind {
        Kind::Movie => parts.extend(i.duration.filter(|d| *d > 0.0).map(fmt_runtime)),
        Kind::Series => {
            let n = i.seasons.len();
            parts.push(format!("{n} season{}", if n == 1 { "" } else { "s" }));
        }
    }
    parts.join(" · ")
}

fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{}…", cut.rsplit_once(' ').map_or(cut.as_str(), |(a, _)| a))
}

/// One card in a row, shaped by the style; returns what to do when clicked.
/// `wide`: a Continue watching card (16:9, plays when clicked). `marked`:
/// it's on My list.
fn card(ui: &mut Ui, c: &Card, wide: bool, marked: bool, images: &mut Images) -> Option<Click> {
    let landscape = wide || theme::spec().layout.tiles == Tiles::Landscape;
    let size = if landscape { WIDE } else { POSTER };
    card_at(ui, c, size, landscape, wide, marked, images)
}

/// The room a card's text takes under its artwork.
fn text_room(text: TileText) -> f32 {
    match text {
        TileText::Below => 40.0,
        TileText::Rich => 78.0,
        TileText::Over | TileText::Hover => 0.0,
    }
}

/// [`card`] at any size (the YouTube-style grid sizes its cards to the width).
fn card_at(
    ui: &mut Ui,
    c: &Card,
    size: egui::Vec2,
    landscape: bool,
    wide: bool,
    marked: bool,
    images: &mut Images,
) -> Option<Click> {
    let spec = theme::spec();
    let text = spec.layout.text;
    let (rect, resp) = ui.allocate_exact_size(size + vec2(0.0, text_room(text)), Sense::click());
    if !ui.is_rect_visible(rect) {
        return None;
    }
    // the buttons on it cover part of the card, so count them as hovering it
    let hot = resp.hovered() || resp.contains_pointer();
    let t = ui.ctx().animate_bool_with_time(resp.id, hot, 0.12);
    let rad = spec.card_radius;
    let art = Rect::from_min_size(rect.min, size).expand(t * spec.hover_grow);
    let p = ui.painter();
    let url = if landscape {
        c.wide_art.as_ref().or(c.art.as_ref())
    } else {
        c.art.as_ref()
    };
    match url.and_then(|u| images.get(u)) {
        Some(tex) => image_cover(p, art, &tex, rad, Color32::WHITE),
        None if url.is_some_and(|u| !images.failed(u)) => {
            p.rect_filled(art, rad, SURFACE());
        }
        // the title goes over the art anyway: don't print it twice
        None if text == TileText::Over => {
            p.rect_filled(art, rad, crate::util::color_for(&c.title));
        }
        None => placeholder(p, art, &c.title, rad),
    }
    let shade = |p: &egui::Painter, frac: f32, a: f32| {
        let r = Rect::from_min_max(
            pos2(art.left(), art.bottom() - art.height() * frac),
            art.max,
        );
        let dark = Color32::from_black_alpha((a * 255.0) as u8);
        crate::ui::gradient(
            p,
            r,
            [Color32::TRANSPARENT, Color32::TRANSPARENT, dark, dark],
        );
    };
    let lift = if c.progress.is_some() { 10.0 } else { 0.0 };
    match text {
        TileText::Over => {
            shade(p, 0.6, 0.75);
            let wrap = art.width() - 24.0;
            let title = p.layout(
                c.title.clone(),
                FontId::new(
                    if landscape { 17.0 } else { 15.0 },
                    egui::FontFamily::Name("black".into()),
                ),
                Color32::WHITE,
                wrap,
            );
            let sub_h = if wide && c.sub.is_some() { 17.0 } else { 0.0 };
            let y = art.bottom() - 12.0 - lift - sub_h;
            p.galley(
                pos2(art.left() + 12.0, y - title.size().y),
                title,
                Color32::WHITE,
            );
            if let (true, Some(s)) = (sub_h > 0.0, &c.sub) {
                p.with_clip_rect(art).text(
                    pos2(art.left() + 12.0, y + 2.0),
                    Align2::LEFT_TOP,
                    s,
                    FontId::proportional(12.5),
                    Color32::from_white_alpha(200),
                );
            }
        }
        TileText::Hover if t > 0.0 => {
            shade(p, 0.7, 0.85 * t);
            let cy = art.bottom() - 28.0 - lift;
            let disc = pos2(art.left() + 26.0, cy);
            p.circle_filled(disc, 15.0, ACCENT().gamma_multiply(t));
            chrome::draw_icon(
                p,
                disc + vec2(1.5, 0.0),
                Icon::Play,
                Color32::WHITE.gamma_multiply(t),
            );
            let clip = p.with_clip_rect(art.shrink(4.0));
            clip.text(
                pos2(art.left() + 50.0, cy - 2.0),
                Align2::LEFT_BOTTOM,
                &c.title,
                FontId::new(15.0, egui::FontFamily::Name("bold".into())),
                Color32::WHITE.gamma_multiply(t),
            );
            if let Some(m) = c.sub.as_ref().filter(|_| wide).or(c.meta.as_ref()) {
                clip.text(
                    pos2(art.left() + 50.0, cy + 2.0),
                    Align2::LEFT_TOP,
                    m,
                    FontId::proportional(12.5),
                    Color32::from_white_alpha((200.0 * t) as u8),
                );
            }
        }
        _ => {}
    }
    if let Some(f) = c.progress {
        let bar = if text == TileText::Rich {
            // YouTube: a red line along the bottom edge
            Rect::from_min_max(pos2(art.left(), art.bottom() - 4.0), art.max)
        } else {
            Rect::from_min_max(
                pos2(art.left() + 8.0, art.bottom() - 9.0),
                pos2(art.right() - 8.0, art.bottom() - 5.0),
            )
        };
        let p = p.with_clip_rect(art);
        p.rect_filled(bar, 2.0, Color32::from_white_alpha(70));
        p.rect_filled(
            Rect::from_min_max(bar.min, pos2(bar.left() + bar.width() * f, bar.bottom())),
            2.0,
            theme::played(),
        );
    }
    if t > 0.0 {
        if spec.hover_outline > 0.0 {
            p.rect_stroke(
                art,
                rad,
                Stroke::new(spec.hover_outline, TEXT().gamma_multiply(t)),
                egui::StrokeKind::Outside,
            );
        }
        if wide && text != TileText::Hover {
            p.circle_filled(
                art.center(),
                24.0,
                Color32::from_black_alpha((160.0 * t) as u8),
            );
            let c = art.center() + vec2(2.0, 0.0);
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + vec2(-6.0, -9.0),
                    c + vec2(9.0, 0.0),
                    c + vec2(-6.0, 9.0),
                ],
                Color32::WHITE.gamma_multiply(t),
                Stroke::NONE,
            ));
        }
    }
    let under = rect.top() + size.y;
    match text {
        TileText::Below => {
            let tp = pos2(rect.left() + 2.0, under + 8.0);
            let clip = p.with_clip_rect(Rect::from_min_size(
                pos2(rect.left(), tp.y),
                vec2(size.x, 40.0),
            ));
            clip.text(
                tp,
                Align2::LEFT_TOP,
                &c.title,
                FontId::proportional(14.0),
                if t > 0.0 {
                    TEXT()
                } else {
                    TEXT().gamma_multiply(0.85)
                },
            );
            if let Some(s) = &c.sub {
                clip.text(
                    tp + vec2(0.0, 18.0),
                    Align2::LEFT_TOP,
                    s,
                    FontId::proportional(12.0),
                    MUTED(),
                );
            }
        }
        TileText::Rich => {
            // a round "channel" badge, a two-line title, then the details
            let ac = pos2(rect.left() + 18.0, under + 12.0 + 18.0);
            p.circle_filled(ac, 18.0, crate::util::color_for(&c.title));
            let initial: String = c.title.chars().take(1).collect::<String>().to_uppercase();
            p.text(
                ac,
                Align2::CENTER_CENTER,
                initial,
                crate::ui::black_font(16.0),
                Color32::WHITE,
            );
            let x = rect.left() + 48.0;
            let mut job = egui::text::LayoutJob::simple(
                c.title.clone(),
                FontId::new(15.5, egui::FontFamily::Name("bold".into())),
                TEXT(),
                size.x - 52.0,
            );
            job.wrap.max_rows = 2;
            let g = ui.fonts(|f| f.layout_job(job));
            let gh = g.size().y;
            p.galley(pos2(x, under + 12.0), g, TEXT());
            let meta = if wide {
                c.sub.as_ref()
            } else {
                c.meta.as_ref()
            };
            if let Some(m) = meta {
                p.with_clip_rect(rect).text(
                    pos2(x, under + 16.0 + gh),
                    Align2::LEFT_TOP,
                    m,
                    FontId::proportional(13.5),
                    MUTED(),
                );
            }
        }
        _ => {}
    }
    if hot && text == TileText::Below {
        let font = FontId::new(12.0, egui::FontFamily::Name("bold".into()));
        let g = ui
            .painter()
            .layout_no_wrap("More info".to_owned(), font, Color32::WHITE);
        let lift = if c.progress.is_some() { 16.0 } else { 8.0 };
        let ir = Rect::from_min_size(
            pos2(art.left() + 8.0, art.bottom() - lift - 26.0),
            vec2(g.size().x + 20.0, 26.0),
        );
        let info = ui.interact(ir, resp.id.with("info"), Sense::click());
        let a = if info.hovered() { 235 } else { 170 };
        let p = ui.painter();
        p.rect_filled(ir, 13.0, Color32::from_black_alpha(a));
        p.rect_stroke(
            ir,
            13.0,
            Stroke::new(
                1.0f32,
                Color32::WHITE.gamma_multiply(if info.hovered() { 0.9 } else { 0.4 }),
            ),
            egui::StrokeKind::Inside,
        );
        p.galley(ir.center() - g.size() / 2.0, g, Color32::WHITE);
        if info.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            return Some(Click::Open(c.item.clone()));
        }
    }
    if hot {
        // bookmark, top right: white once the title is on My list
        let br = Rect::from_min_size(pos2(art.right() - 38.0, art.top() + 8.0), vec2(30.0, 30.0));
        let b = ui.interact(br, resp.id.with("bookmark"), Sense::click());
        let p = ui.painter();
        if marked {
            p.circle_filled(br.center(), 15.0, Color32::WHITE);
            chrome::draw_icon(
                p,
                br.center(),
                Icon::BookmarkOn,
                Color32::from_rgb(20, 20, 24),
            );
        } else {
            let a = if b.hovered() { 230 } else { 150 };
            p.circle_filled(br.center(), 15.0, Color32::from_black_alpha(a));
            if b.hovered() {
                p.circle_stroke(
                    br.center(),
                    15.0,
                    Stroke::new(1.0f32, Color32::WHITE.gamma_multiply(0.6)),
                );
            }
            chrome::draw_icon(p, br.center(), Icon::Bookmark, Color32::WHITE);
        }
        let tip = if marked {
            "Remove from My list"
        } else {
            "Add to My list"
        };
        if b.on_hover_text(tip)
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            return Some(Click::Bookmark(c.item.clone()));
        }
    }
    if c.removable {
        let menu = resp.id.with("menu");
        let open = ui.memory(|m| m.is_popup_open(menu));
        if hot || open {
            // left of the bookmark
            let dr =
                Rect::from_min_size(pos2(art.right() - 74.0, art.top() + 8.0), vec2(30.0, 30.0));
            let dots = ui.interact(dr, menu.with("button"), Sense::click());
            let a = if dots.hovered() || open { 230 } else { 150 };
            ui.painter()
                .circle_filled(dr.center(), 15.0, Color32::from_black_alpha(a));
            chrome::draw_icon(ui.painter(), dr.center(), Icon::More, Color32::WHITE);
            let dots = dots
                .on_hover_text("More")
                .on_hover_cursor(CursorIcon::PointingHand);
            if dots.clicked() {
                ui.memory_mut(|m| m.toggle_popup(menu));
                return None;
            }
            let mut hide = false;
            egui::popup::popup_below_widget(
                ui,
                menu,
                &dots,
                egui::PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    ui.set_min_width(240.0);
                    if menu_item(ui, "Remove from Continue watching") {
                        hide = true;
                        ui.memory_mut(|m| m.close_popup());
                    }
                },
            );
            if hide {
                return Some(Click::Hide(c.item.clone()));
            }
        }
    }
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if !wide && text == TileText::Below {
        resp.clone().on_hover_text_at_pointer(&c.title);
    }
    resp.clicked().then(|| match &c.episode {
        Some(ep) => Click::Play(c.item.clone(), Some(ep.clone()), false),
        None if wide => Click::Play(c.item.clone(), None, false),
        None => Click::Open(c.item.clone()),
    })
}

fn grid(
    ui: &mut Ui,
    cards: &[Card],
    marked: &dyn Fn(&str) -> bool,
    images: &mut Images,
) -> Option<Click> {
    grid_cards(ui, cards, false, marked, images)
}

/// [`grid`]; `wide`: Continue watching cards (16:9, play when clicked).
fn grid_cards(
    ui: &mut Ui,
    cards: &[Card],
    wide: bool,
    marked: &dyn Fn(&str) -> bool,
    images: &mut Images,
) -> Option<Click> {
    let mut click = None;
    let lay = &theme::spec().layout;
    let landscape = wide || lay.tiles == Tiles::Landscape;
    let base = if landscape { WIDE } else { POSTER };
    let avail = ui.available_width() - 2.0 * SIDE;
    let cols = ((avail + GAP) / (base.x + GAP)).floor().max(1.0);
    // landscape cards stretch to fill the row, like a video grid
    let size = if landscape {
        let w = (avail - GAP * (cols - 1.0)) / cols;
        vec2(w, w * 9.0 / 16.0)
    } else {
        base
    };
    for chunk in cards.chunks(cols as usize) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            ui.add_space(SIDE);
            for c in chunk {
                if let Some(k) = card_at(ui, c, size, landscape, wide, marked(&c.item), images) {
                    click = Some(k);
                }
            }
        });
        ui.add_space(if lay.text == TileText::Rich {
            22.0
        } else {
            12.0
        });
    }
    click
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffle_is_stable_and_keeps_everything() {
        let items: Vec<Item> = (0..20)
            .map(|n| Item {
                id: format!("movie:{n}"),
                ..Default::default()
            })
            .collect();
        let st = RandomState::new();
        let a = shuffled(items.iter(), "movies", &st);
        let b = shuffled(items.iter(), "movies", &st);
        let ids = |v: &[&Item]| v.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&a), ids(&b)); // same order every frame
        let mut sorted = ids(&a);
        sorted.sort();
        let mut all = ids(&items.iter().collect::<Vec<_>>());
        all.sort();
        assert_eq!(sorted, all); // nothing lost or doubled
    }

    #[test]
    fn sorts_both_ways_with_blanks_last() {
        let item = |id: &str, year: Option<u32>| Item {
            id: id.into(),
            year,
            ..Default::default()
        };
        let all = [
            item("a", Some(2001)),
            item("b", None),
            item("c", Some(1999)),
        ];
        let ids = |v: &[&Item]| v.iter().map(|i| i.id.as_str()).collect::<String>();
        let mut v: Vec<&Item> = all.iter().collect();
        sort_by(&mut v, false, |i| i.year);
        assert_eq!(ids(&v), "cab");
        sort_by(&mut v, true, |i| i.year);
        assert_eq!(ids(&v), "acb");
    }
}
