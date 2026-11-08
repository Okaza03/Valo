//! The library: the home page, the Movies and TV shows grids, search, a
//! director's or creator's titles, and the movie and series pages. Ported
//! from `crate::ui::browse`, in the default Valo style; the other styles'
//! layouts, Streaming (addon catalogs), profiles, admin and the arrivals
//! popup are still egui-only.
mod cards;
mod details;
mod marks;

use std::hash::RandomState;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{
    button, column, container, pick_list, responsive, row, scrollable, space, stack, text,
    text_input,
};
use iced::{Background, Border, Color, Element, Length, Padding};

use super::icon::{self, Icon};
use super::images::Images;
use super::spaced::mono;
use super::style::{self, black, c, BOLD, MONO};
use super::widgets;
use super::{Job, Out};
use crate::api::{self, Client, Item, Kind, Me, Missing, Mode, ServerLayout, Unauthorized};
use crate::config::{Config, Entry, Hidden, Progress, Watchlist};
use crate::sync::ListSync;
use crate::ui::browse::{
    featured, maker, makers, meta_line, next_up, play_items, play_label, shorten, shuffled, sort_by,
};
use crate::ui::theme;
use crate::ui::Action;
use crate::util::{fmt_runtime, Waker};
use cards::{Card, SIDE};

/// The page's scrollable, put back at the top when another page opens.
const PAGE: &str = "library-page";
/// The search field, for Ctrl+F.
pub const SEARCH: &str = "library-search";
const HERO_ROTATE: Duration = Duration::from_secs(9);
/// The home page's own rows in their usual order (their layout keys).
const HOME_ROWS: [&str; 6] = ["continue", "mylist", "recent", "top", "movies", "series"];

#[derive(Clone, Debug, PartialEq)]
pub enum View {
    Home,
    Movies,
    Series,
    Details(String),
    /// series id, episode id
    Episode(String, String),
    /// A director's or creator's titles, by name.
    Person(String),
}

impl View {
    /// Pages opened from another page: Back returns to it.
    fn nested(&self) -> bool {
        matches!(self, View::Details(_) | View::Episode(..) | View::Person(_))
    }
}

/// The series page's sections.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tab {
    Episodes,
    Related,
    Details,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filter {
    All,
    InProgress,
    Unwatched,
    Watched,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sort {
    Title,
    Year,
    Added,
    Director,
}

/// A sort in the picker, with its label ("Director" or "Creator").
#[derive(Clone, Copy, Debug)]
pub struct SortOpt(Sort, &'static str);

impl PartialEq for SortOpt {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}

impl std::fmt::Display for SortOpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.1)
    }
}

/// A season in the picker: its index and label.
#[derive(Clone, Debug)]
pub struct SeasonOpt(usize, String);

impl PartialEq for SeasonOpt {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}

impl std::fmt::Display for SeasonOpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}

const ALL_GENRES: &str = "All genres";

#[derive(Clone, Debug)]
pub enum Msg {
    Nav(View),
    Back,
    Open(String),
    /// item id, episode id (series), start from the beginning
    Play(String, Option<String>, bool),
    /// Take an item off Continue watching.
    Hide(String),
    /// Add an item to My list, or take it off.
    Bookmark(String),
    /// The pointer is over this card (its row and id), or none.
    Hover(Option<String>),
    /// Scroll a row (by its scrollable id) sideways by this much.
    Page(String, f32),
    /// A row was scrolled to this offset.
    Scrolled(String, f32),
    /// The pointer is over this row (its key), or none.
    RowOver(Option<String>),
    /// Open a card's "⋮" menu (by its key), or close it.
    CardMenu(Option<String>),
    Search(String),
    Sort(SortOpt),
    Desc,
    Genre(String),
    Filter(Filter),
    Season(SeasonOpt),
    Tab(Tab),
    /// Step the featured banner.
    Hero(isize),
    Retry,
    ChangeServer,
    /// Open or close the profile menu.
    Menu(bool),
    SignOut,
    Quit,
    CopyNote,
    Docs,
}

/// What the views read from the app.
#[derive(Clone, Copy)]
pub struct Env<'a> {
    pub images: &'a Images,
    pub progress: &'a Progress,
    pub cfg: &'a Config,
}

pub struct Library {
    client: Client,
    user: Option<String>,
    lib: Option<Arc<api::Library>>,
    loading: Option<Job<Result<api::Library>>>,
    loaded_at: Option<Instant>,
    error: Option<String>,
    missing: Option<Missing>,
    copied: bool,
    view: View,
    history: Vec<View>,
    search: String,
    sort: Sort,
    /// Sort direction.
    desc: bool,
    genre: Option<String>,
    filter: Filter,
    /// The series page's season; `usize::MAX` picks the one to continue.
    season: usize,
    tab: Tab,
    /// The featured title shown, counting from the first; taken modulo
    /// their number when drawn, so stepping back from the first wraps.
    hero: isize,
    hero_since: Instant,
    hovered: Option<String>,
    /// The profile menu is open.
    menu: bool,
    /// How far each row is scrolled, by its scrollable id.
    row_offsets: std::collections::HashMap<String, f32>,
    /// The row the pointer is over.
    row_over: Option<String>,
    /// The card whose "⋮" menu is open.
    card_menu: Option<String>,
    /// Who else watched each title, by id.
    watchers: std::collections::HashMap<String, details::Watchers>,
    /// File info probed from streams, by stream URL.
    probes: std::collections::HashMap<String, details::Probe>,
    watchlist: Watchlist,
    hidden: Hidden,
    /// My list and Continue watching removals, kept in step with the server.
    lists: ListSync,
    /// Orders Featured and the rows: random, but the same all session.
    shuffle: RandomState,
    /// Re-checking a remembered session's admin rights.
    me: Option<Job<Option<Me>>>,
    /// The home page as the server's admin set it up.
    layout: ServerLayout,
    layout_task: Option<Job<Result<ServerLayout>>>,
}

impl Library {
    pub fn new(client: Client, user: Option<String>) -> Self {
        let me = client.token.is_some().then(|| {
            let c = client.clone();
            Job::spawn(move || c.me())
        });
        let mut l = Self {
            client,
            user,
            lib: None,
            loading: None,
            loaded_at: None,
            error: None,
            missing: None,
            copied: false,
            view: View::Home,
            history: Vec::new(),
            search: String::new(),
            sort: Sort::Title,
            desc: false,
            genre: None,
            filter: Filter::All,
            season: usize::MAX,
            tab: Tab::Episodes,
            hero: 0,
            hero_since: Instant::now(),
            hovered: None,
            menu: false,
            row_offsets: std::collections::HashMap::new(),
            row_over: None,
            card_menu: None,
            watchers: std::collections::HashMap::new(),
            probes: std::collections::HashMap::new(),
            watchlist: Watchlist::load(),
            hidden: Hidden::load(),
            lists: ListSync::default(),
            shuffle: RandomState::new(),
            me,
            layout: ServerLayout::default(),
            layout_task: None,
        };
        l.reload();
        l
    }

    pub fn reload(&mut self) {
        let c = self.client.clone();
        self.loading = Some(Job::spawn(move || c.library()));
        let c = self.client.clone();
        self.layout_task = Some(Job::spawn(move || c.layout()));
        self.lists.pull_now();
        self.error = None;
        self.missing = None;
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    /// Loading the library: tick quickly.
    pub fn busy(&self) -> bool {
        self.loading.is_some()
            || self.me.is_some()
            || self.layout_task.is_some()
            || self.details_busy()
    }

    fn prefix(&self) -> String {
        crate::config::user_prefix(&self.client.base, self.user.as_deref())
    }

    fn key(&self, id: &str) -> String {
        format!("{}{id}", self.prefix())
    }

    /// A movie that's been watched to the end.
    fn watched(&self, item: &Item, progress: &Progress) -> bool {
        item.kind == Kind::Movie
            && progress
                .get(&self.key(&item.id))
                .is_some_and(Entry::watched)
    }

    /// The hovered card and the open card menu, for drawing cards.
    pub(super) fn hot(&self) -> cards::Hot<'_> {
        cards::Hot {
            card: self.hovered.as_deref(),
            menu: self.card_menu.as_deref(),
        }
    }

    fn go(&mut self, v: View) -> Out {
        self.card_menu = None;
        if v == self.view && self.search.is_empty() {
            return Out::None;
        }
        if v.nested() {
            if v != self.view {
                self.history.push(self.view.clone());
            }
            if let View::Details(_) = v {
                self.season = usize::MAX;
                self.tab = Tab::Episodes;
            }
        } else {
            self.history.clear();
        }
        self.view = v;
        self.details_work();
        self.search.clear();
        self.hovered = None;
        Out::Top(PAGE)
    }

    fn back(&mut self) -> Out {
        if self.history.is_empty() && !self.view.nested() {
            return Out::None;
        }
        self.view = self.history.pop().unwrap_or(View::Home);
        self.hovered = None;
        Out::Top(PAGE)
    }

    /// Backspace or the mouse's back button, outside a text field.
    pub fn key_back(&mut self) -> Out {
        if !self.search.is_empty() {
            self.search.clear();
            return Out::Top(PAGE);
        }
        self.back()
    }

    /// Poll the background work: the library, the layout, the account, the
    /// lists' sync. Also turns the featured banner and refreshes the library.
    pub fn tick(&mut self, cfg: &Config, images: &mut Images) -> Out {
        let mut out = Out::None;
        self.lists.tick(
            Waker::none(),
            &self.client,
            self.user.as_deref(),
            &mut self.watchlist,
            &mut self.hidden,
        );
        if let Some(r) = self.me.as_mut().and_then(Job::poll) {
            self.me = None;
            if let Some(me) = r {
                if me.admin != self.client.admin {
                    self.client.admin = me.admin;
                    out = Out::Act(Action::AdminChanged(me.admin));
                }
            }
        }
        if let Some(r) = self.layout_task.as_mut().and_then(Job::poll) {
            self.layout_task = None;
            // can't load it: keep what we had (everything, at first)
            if let Ok(l) = r {
                self.layout = l;
            }
        }
        if let Some(r) = self.loading.as_mut().and_then(Job::poll) {
            self.loading = None;
            match r {
                Ok(lib) => {
                    let gone = match &self.view {
                        View::Details(id) => lib.find(id).is_none(),
                        View::Episode(_, ep) => lib.find_episode(ep).is_none(),
                        _ => false,
                    };
                    if gone {
                        self.view = View::Home;
                        self.history.clear();
                        out = Out::Top(PAGE);
                    }
                    crate::ui::set_moving_art(lib.moving_art());
                    if self.lib.is_none() {
                        images.set_client(Some(self.client.clone()));
                    }
                    self.lib = Some(Arc::new(lib));
                    self.loaded_at = Some(Instant::now());
                }
                Err(e) if e.downcast_ref::<Unauthorized>().is_some() => {
                    return Out::Act(Action::NeedLogin(self.client.clone()));
                }
                Err(e) => {
                    self.missing = e.downcast_ref::<Missing>().copied();
                    self.error = Some(format!("{e:#}"));
                    self.loaded_at = Some(Instant::now()); // don't retry in a tight loop
                }
            }
        }
        let every = Duration::from_secs(u64::from(cfg.refresh_minutes) * 60);
        if cfg.refresh_minutes > 0
            && self.loading.is_none()
            && self.loaded_at.is_some_and(|t| t.elapsed() > every)
        {
            self.reload();
        }
        if self.hero_since.elapsed() > HERO_ROTATE && self.view == View::Home {
            self.hero = self.hero.wrapping_add(1);
            self.hero_since = Instant::now();
        }
        self.details_work();
        out
    }

    /// The window got focus back: titles may have been added or removed since.
    pub fn focused(&mut self) {
        if self.loading.is_none()
            && self
                .loaded_at
                .is_some_and(|t| t.elapsed() > Duration::from_secs(60))
        {
            self.reload();
        }
    }

    pub fn update(&mut self, msg: Msg, progress: &Progress, cfg: &Config) -> Out {
        match msg {
            Msg::Nav(v) => return self.go(v),
            Msg::Back => return self.key_back(),
            Msg::Open(id) => return self.go(View::Details(id)),
            Msg::Play(id, ep, _) if self.moving(&id, ep.as_deref()) => {
                // its page says why it can't play
                return self.go(View::Details(id));
            }
            Msg::Play(id, ep, restart) => {
                let lib = self.lib.clone();
                let key = |i: &str| self.key(i);
                if let Some(items) = lib.and_then(|lib| {
                    play_items(
                        &self.client,
                        &lib,
                        &id,
                        ep.as_deref(),
                        restart || !cfg.resume,
                        progress,
                        &key,
                    )
                }) {
                    return Out::Act(Action::Play(items));
                }
            }
            Msg::Hide(id) => {
                self.card_menu = None;
                let key = self.key(&id);
                self.hidden.hide(&key);
            }
            Msg::Bookmark(id) => {
                let key = self.key(&id);
                self.watchlist.toggle(&key);
            }
            Msg::Hover(k) => self.hovered = k,
            Msg::Page(id, dx) => return Out::ScrollBy(id, dx),
            Msg::Scrolled(id, x) => {
                self.row_offsets.insert(id, x);
            }
            Msg::RowOver(k) => self.row_over = k,
            Msg::CardMenu(k) => self.card_menu = k,
            Msg::Search(s) => {
                let was = self.search.trim().is_empty();
                self.search = s;
                self.hovered = None;
                if was != self.search.trim().is_empty() {
                    return Out::Top(PAGE);
                }
            }
            Msg::Sort(SortOpt(s, _)) => {
                if s != self.sort {
                    self.sort = s;
                    // newest first, names A to Z
                    self.desc = matches!(s, Sort::Year | Sort::Added);
                }
            }
            Msg::Desc => self.desc = !self.desc,
            Msg::Genre(g) => self.genre = (g != ALL_GENRES).then_some(g),
            Msg::Filter(f) => self.filter = f,
            Msg::Season(SeasonOpt(i, _)) => {
                self.season = i;
                self.tab = Tab::Episodes;
            }
            Msg::Tab(t) => self.tab = t,
            Msg::Hero(step) => {
                self.hero = self.hero.wrapping_add(step);
                self.hero_since = Instant::now();
            }
            Msg::Retry => self.reload(),
            Msg::ChangeServer => return Out::Act(Action::ChangeServer),
            Msg::Menu(open) => self.menu = open,
            Msg::SignOut => {
                self.menu = false;
                return Out::Act(Action::SignOut(self.client.clone()));
            }
            Msg::Quit => {
                self.menu = false;
                return Out::Act(Action::Quit);
            }
            Msg::CopyNote => {
                if let Some(m) = self.missing {
                    self.copied = true;
                    return Out::Copy(m.admin_note(&self.client.base));
                }
            }
            Msg::Docs => crate::ui::chrome::open_url(api::API_DOCS),
        }
        Out::None
    }

    /// An admin is moving this title's files (or the episode's), so it can't play.
    fn moving(&self, id: &str, ep: Option<&str>) -> bool {
        let Some(lib) = &self.lib else {
            return false;
        };
        let ep_moving = ep
            .and_then(|e| lib.find_episode(e))
            .is_some_and(|(s, si, ei)| s.seasons[si].episodes[ei].unavailable);
        lib.find(id).is_some_and(|i| i.unavailable) || ep_moving
    }

    // ------------------------------------------------------------ the bar

    /// The title bar's left side: Back on nested pages, and the page tabs.
    pub fn bar_left(&self) -> Element<'_, Msg> {
        let searching = !self.search.is_empty();
        let mut r = row![].spacing(6).align_y(Vertical::Center);
        if self.view.nested() {
            r = r.push(crate::iced_ui::chrome::tip_below(
                widgets::tab("‹ Back", false, Msg::Back),
                "Backspace",
            ));
        }
        if theme::spec().layout.nav == theme::Nav::Top {
            for (label, v) in [
                ("Home", View::Home),
                ("Movies", View::Movies),
                ("TV Shows", View::Series),
            ] {
                let on = !searching && self.view == v;
                r = r.push(widgets::tab(label, on, Msg::Nav(v)));
            }
        }
        r.into()
    }

    /// The title bar's search field (egui: 240 wide at most, 10×6 margins).
    pub fn search_field(&self) -> Element<'_, Msg> {
        let p = theme::pal();
        text_input("Search movies & shows", &self.search)
            .id(SEARCH)
            .on_input(Msg::Search)
            .size(14.5)
            .padding([6, 10])
            .width(240)
            .style(move |_, s| {
                let border = match s {
                    text_input::Status::Focused { .. } => c(p.text),
                    text_input::Status::Hovered => c(p.line_hi),
                    _ => c(p.line),
                };
                text_input::Style {
                    background: Background::Color(c(p.field)),
                    border: Border {
                        color: border,
                        width: 1.0,
                        radius: 3.0.into(),
                    },
                    icon: c(p.muted),
                    placeholder: c(p.muted),
                    value: c(p.text),
                    selection: c(p.accent).scale_alpha(0.4),
                }
            })
            .into()
    }

    /// Escape in the search field: clear it.
    pub fn escape(&mut self) -> Out {
        if self.search.is_empty() {
            return Out::None;
        }
        self.search.clear();
        Out::Top(PAGE)
    }

    fn shown_name(&self) -> String {
        self.user.clone().unwrap_or_else(|| "Guest".to_owned())
    }

    /// The profile button: avatar, name and a caret (egui's `profile_menu`).
    pub fn profile_button(&self) -> Element<'_, Msg> {
        let name = self.shown_name();
        let open = self.menu;
        let content = row![
            space().width(14),
            avatar(&name, 12.0),
            space().width(8),
            crate::iced_ui::spaced::Spaced::new(name.to_uppercase(), 11.5, 11.5 * 0.14, MONO)
                .color(style::TEXT()),
            space().width(12),
            icon::icon(Icon::ChevronDown, style::MUTED(), 12.0),
            space().width(9),
        ]
        .align_y(Vertical::Center)
        .height(crate::iced_ui::chrome::BAR_H);
        button(content)
            .padding(0)
            .style(move |_, s| {
                let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: (hot || open).then(|| Background::Color(style::SURFACE_HI())),
                    ..button::Style::default()
                }
            })
            .on_press(Msg::Menu(!open))
            .into()
    }

    /// Where the profile button's left edge is, from the window's right.
    fn profile_offset(&self) -> f32 {
        let n = self.shown_name().chars().count() as f32;
        let label = n * 11.5 * 0.74 - 11.5 * 0.14;
        let profile_w = label + 72.0;
        3.0 * crate::iced_ui::chrome::BAR_H + 1.0 + profile_w
    }

    /// The open profile menu, under its button; a click elsewhere closes it.
    pub fn menu_overlay(&self) -> Option<Element<'_, Msg>> {
        if !self.menu {
            return None;
        }
        let name = self.shown_name();
        let mut items = column![
            row![
                avatar(&name, 17.0),
                column![
                    text(name.clone()).font(BOLD).size(15),
                    widgets::label(&self.client.info.name, 10.5),
                ]
                .spacing(0),
            ]
            .spacing(8)
            .align_y(Vertical::Center)
            .padding([4, 2]),
            separator(),
        ]
        .spacing(2)
        .width(250);
        if self.client.token.is_some() {
            items = items.push(widgets::menu_item("Sign out", Msg::SignOut));
        }
        items = items.push(widgets::menu_item("Quit", Msg::Quit));
        let panel = container(items).padding(6).style(|_| {
            let p = theme::pal();
            container::Style {
                background: Some(Background::Color(c(p.surface))),
                border: Border {
                    color: c(p.line_hi),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                shadow: iced::Shadow {
                    color: black(120),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                text_color: Some(c(p.text)),
                ..container::Style::default()
            }
        });
        let right = (self.profile_offset() - 262.0).max(0.0);
        Some(
            stack![
                iced::widget::mouse_area(space().width(Length::Fill).height(Length::Fill))
                    .on_press(Msg::Menu(false)),
                container(panel)
                    .width(Length::Fill)
                    .align_x(Horizontal::Right)
                    .padding(
                        Padding::default()
                            .top(crate::iced_ui::chrome::BAR_H + 2.0)
                            .right(right)
                    ),
            ]
            .into(),
        )
    }

    /// The status line: when the library was loaded and from where; the counts.
    pub fn status(&self) -> (String, String) {
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
        (
            format!("{when}  ·  {host}"),
            format!("{right}Valo {}", crate::update::VERSION),
        )
    }

    /// Whether clicking the status line may reload (not while loading).
    pub fn can_refresh(&self) -> bool {
        self.loading.is_none()
    }

    // -------------------------------------------------------------- pages

    pub fn view<'a>(&'a self, env: Env<'a>) -> Element<'a, Msg> {
        let Some(lib) = &self.lib else {
            return self.not_loaded();
        };
        responsive(move |size| {
            let w = size.width;
            let page: Element<Msg> = if !self.search.trim().is_empty() {
                self.search_page(lib, env, w)
            } else {
                match &self.view {
                    View::Home => self.home(lib, env, w),
                    View::Movies => self.grid_page(lib, Kind::Movie, env, w),
                    View::Series => self.grid_page(lib, Kind::Series, env, w),
                    View::Person(name) => self.person_page(lib, name, env, w),
                    View::Details(id) => match lib.find(id) {
                        Some(item) if item.kind == Kind::Movie => {
                            self.movie_page(lib, item, env, w)
                        }
                        Some(item) => self.series_page(lib, item, env, w),
                        None => muted_center("This title isn't on the server any more"),
                    },
                    View::Episode(_, ep) => match lib.find_episode(ep) {
                        Some((s, si, ei)) => self.episode_page(s, si, ei, env, w),
                        None => muted_center("This episode isn't on the server any more"),
                    },
                }
            };
            let page = scrollable(column![page, space().height(40)])
                .id(PAGE)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(scroll_style);
            if self.card_menu.is_some() {
                // a click outside the open menu closes it
                stack![
                    page,
                    iced::widget::mouse_area(space().width(Length::Fill).height(Length::Fill))
                        .on_press(Msg::CardMenu(None)),
                ]
                .into()
            } else {
                page.into()
            }
        })
        .into()
    }

    /// Loading, or why it couldn't.
    fn not_loaded(&self) -> Element<'_, Msg> {
        let body: Element<Msg> = match &self.error {
            Some(e) => {
                let mut col = column![
                    widgets::label("Couldn't load your library", 13.0),
                    text(e.clone()).color(style::ERROR()),
                ]
                .spacing(6)
                .align_x(Horizontal::Center)
                .max_width(520);
                if let Some(m) = self.missing {
                    col = col.push(widgets::admin_help(
                        m,
                        self.copied,
                        Msg::CopyNote,
                        Msg::Docs,
                    ));
                }
                col.push(space().height(8))
                    .push(
                        row![
                            widgets::small("Try again", Msg::Retry),
                            widgets::small("Switch server", Msg::ChangeServer),
                        ]
                        .spacing(10),
                    )
                    .into()
            }
            None => column![
                icon::spinner(spin_secs(), style::TEXT(), 28.0),
                widgets::label("Loading your library", 12.0),
            ]
            .spacing(10)
            .align_x(Horizontal::Center)
            .into(),
        };
        container(body).center(Length::Fill).padding(24).into()
    }

    fn home<'a>(&'a self, lib: &'a api::Library, env: Env<'a>, w: f32) -> Element<'a, Msg> {
        if lib.movies.is_empty() && lib.series.is_empty() {
            return column![
                space().height(120),
                mono("Your library is empty", 13.0),
                text("No movies or series were found on this server.")
                    .size(14.5)
                    .color(style::MUTED()),
            ]
            .spacing(4)
            .width(Length::Fill)
            .align_x(Horizontal::Center)
            .into();
        }
        let mut page = column![];
        let feat = featured(lib, &self.shuffle);
        if !feat.is_empty() && self.layout.shows("featured") {
            let i = self.hero.rem_euclid(feat.len() as isize) as usize;
            page = page.push(self.hero(feat[i], (i, feat.len()), env, w));
        }
        let row_of = |title: &str, cards: &[Card], wide: bool, all: Option<View>| {
            self.row(env, title, cards, wide, all.map(Msg::Nav), w)
        };
        let keys: Vec<String> = HOME_ROWS
            .iter()
            .map(|k| (*k).to_owned())
            .chain(["genres".to_owned()])
            .collect();
        for key in self.layout.ordered(&keys) {
            if !self.layout.shows(&key) {
                continue;
            }
            match key.as_str() {
                "continue" => {
                    let cont = self.continue_cards(lib, env.progress);
                    if !cont.is_empty() {
                        page = page.push(row_of("Continue watching", &cont, true, None));
                    }
                }
                "mylist" => {
                    let prefix = self.prefix();
                    let mine: Vec<&Item> = self
                        .watchlist
                        .ids(&prefix)
                        .filter_map(|id| lib.find(id))
                        .collect();
                    if !mine.is_empty() {
                        let cards = self.poster_cards(mine.into_iter(), env.progress);
                        page = page.push(row_of("My list", &cards, false, None));
                    }
                }
                "recent" if lib.all().any(|i| i.added.is_some()) => {
                    let mut recent: Vec<&Item> = lib.all().filter(|i| i.added.is_some()).collect();
                    recent.sort_by_key(|i| std::cmp::Reverse(i.added));
                    let cards = self.poster_cards(recent.into_iter().take(24), env.progress);
                    page = page.push(row_of("Recently added", &cards, false, None));
                }
                "top" => {
                    // best rated first; ties in a random order
                    let rated: Vec<&Item> = lib.all().filter(|i| i.score.is_some()).collect();
                    if rated.len() < 5 {
                        continue;
                    }
                    let mut rated = shuffled(rated.into_iter(), "top", &self.shuffle);
                    rated.sort_by(|a, b| b.score.unwrap_or(0.0).total_cmp(&a.score.unwrap_or(0.0)));
                    let cards = self.poster_cards(rated.into_iter().take(24), env.progress);
                    page = page.push(row_of("Top rated", &cards, false, None));
                }
                // the rest in a random order (the same all session), so the home page isn't A to Z
                "movies" if !lib.movies.is_empty() => {
                    let movies = shuffled(lib.movies.iter(), "movies", &self.shuffle);
                    let cards = self.poster_cards(movies.into_iter().take(30), env.progress);
                    page = page.push(row_of("Movies", &cards, false, Some(View::Movies)));
                }
                "series" if !lib.series.is_empty() => {
                    let series = shuffled(lib.series.iter(), "series", &self.shuffle);
                    let cards = self.poster_cards(series.into_iter().take(30), env.progress);
                    page = page.push(row_of("TV shows", &cards, false, Some(View::Series)));
                }
                "genres" if env.cfg.genre_rows => {
                    let mut genres = lib.genres();
                    genres.retain(|g| self.layout.shows(&format!("genre:{g}")));
                    for g in genres.into_iter().take(8) {
                        let items = shuffled(
                            lib.all().filter(|i| i.genres.contains(&g)),
                            &g,
                            &self.shuffle,
                        );
                        let cards = self.poster_cards(items.into_iter().take(30), env.progress);
                        if cards.len() >= 2 {
                            page = page.push(row_of(&g, &cards, false, None));
                        }
                    }
                }
                _ => {}
            }
        }
        page.into()
    }

    /// A titled row of cards that pages sideways.
    pub(super) fn row<'a>(
        &self,
        env: Env<'a>,
        title: &str,
        cards: &[Card],
        wide: bool,
        all: Option<Msg>,
        w: f32,
    ) -> Element<'a, Msg> {
        let id = format!("row\u{1f}{title}");
        cards::row_of(
            env.images,
            title,
            cards,
            wide,
            self.hot(),
            all,
            w,
            self.row_offsets.get(&id).copied().unwrap_or(0.0),
            self.row_over.as_deref() == Some(id.as_str()),
        )
    }

    /// The featured banner (egui's `Browse::hero`, as a carousel): artwork
    /// on the right fading into the page, the title, what it is and the
    /// buttons on the left, dots for each featured title, and arrows while
    /// the pointer is over it. A click anywhere else opens the title.
    fn hero<'a>(
        &'a self,
        item: &'a Item,
        at: (usize, usize),
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let h = (w * 0.40).clamp(300.0, 560.0);
        let art_x = w * 0.25;
        let art_w = w - art_x;
        let bg = style::BG();
        let clear = Color { a: 0.0, ..bg };
        let url = item.backdrop.as_deref().or(item.poster.as_deref());
        let loaded = url
            .and_then(|u| env.images.get(u, crate::iced_ui::images::Variant::Full))
            .is_some();
        let mut layers: Vec<Element<'a, Msg>> = Vec::new();
        if loaded {
            layers.push(
                container(space())
                    .width(w)
                    .height(h)
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::BLACK)),
                        ..container::Style::default()
                    })
                    .into(),
            );
            layers.push(
                row![
                    container(space())
                        .width(art_x + 1.0)
                        .height(h)
                        .style(move |_| {
                            container::Style {
                                background: Some(Background::Color(bg)),
                                ..container::Style::default()
                            }
                        }),
                    stack![
                        cards::art(env.images, url, &item.title, art_w - 1.0, h, 0.0),
                        container(space())
                            .width((art_w - 1.0) * 0.45)
                            .height(h)
                            .style(move |_| fill_gradient(std::f32::consts::FRAC_PI_2, bg, clear)),
                    ],
                ]
                .into(),
            );
        } else {
            // no artwork: the title's color, fading into the page both sides
            let col = c(crate::util::color_for(&item.title));
            let half = Color { a: 0.5, ..col };
            layers.push(
                row![
                    container(space())
                        .width(w / 2.0)
                        .height(h)
                        .style(move |_| fill_gradient(std::f32::consts::FRAC_PI_2, bg, half)),
                    container(space())
                        .width(w / 2.0)
                        .height(h)
                        .style(move |_| fill_gradient(std::f32::consts::FRAC_PI_2, col, bg)),
                ]
                .into(),
            );
        }
        // the bottom fades into the page
        layers.push(
            container(
                container(space())
                    .width(w)
                    .height(h * 0.35)
                    .style(move |_| fill_gradient(std::f32::consts::PI, clear, bg)),
            )
            .height(h)
            .align_y(Vertical::Bottom)
            .into(),
        );

        let (label, restart) = play_label(item, env.progress, &|i| self.key(i));
        let mut buttons = row![widgets::button_with(
            widgets::Kind::Primary,
            &label,
            Some(Icon::Play),
            (!item.unavailable).then(|| Msg::Play(item.id.clone(), None, false)),
        )]
        .spacing(8);
        if restart {
            buttons = buttons.push(widgets::button_with(
                widgets::Kind::Secondary,
                "Start over",
                None,
                Some(Msg::Play(item.id.clone(), None, true)),
            ));
        }
        buttons = buttons.push(widgets::button_with(
            widgets::Kind::Secondary,
            "More info",
            None,
            Some(Msg::Open(item.id.clone())),
        ));
        let text_w = (w * 0.45).clamp(320.0, 640.0).min(w - 2.0 * SIDE);
        let kind = if item.kind == Kind::Movie {
            "Featured movie"
        } else {
            "Featured series"
        };
        let mut words = column![
            mono(kind, 11.0).color(c(theme::pal().accent)),
            text(item.title.clone()).font(BOLD).size(40),
            space().height(2),
            mono(&meta_line(item), 11.5).color(style::MUTED()),
        ]
        .spacing(3)
        .width(text_w);
        if !item.overview.is_empty() {
            words = words
                .push(
                    text(shorten(&item.overview, 260))
                        .size(15)
                        .color(style::TEXT()),
                )
                .push(space().height(4));
        }
        words = words.push(space().height(8)).push(buttons);
        layers.push(
            container(words)
                .height(h)
                .padding(Padding::default().left(SIDE).bottom(32.0))
                .align_y(Vertical::Bottom)
                .into(),
        );
        let over = self.row_over.as_deref() == Some("hero");
        if at.1 > 1 {
            let mut dots = row![].align_y(Vertical::Center);
            for i in 0..at.1 {
                let on = i == at.0;
                dots = dots.push(
                    container(
                        container(space())
                            .width(if on { 14 } else { 6 })
                            .height(3)
                            .style(move |_| container::Style {
                                background: Some(Background::Color(if on {
                                    style::TEXT()
                                } else {
                                    c(theme::MUTED().gamma_multiply(0.6))
                                })),
                                ..container::Style::default()
                            }),
                    )
                    .center_x(16),
                );
            }
            layers.push(
                container(dots)
                    .width(w)
                    .height(h)
                    .padding(Padding::default().right(SIDE + 8.0).bottom(20.5))
                    .align_x(Horizontal::Right)
                    .align_y(Vertical::Bottom)
                    .into(),
            );
            if over {
                layers.push(
                    container(row![
                        hero_arrow("‹", Msg::Hero(-1)),
                        space::horizontal(),
                        hero_arrow("›", Msg::Hero(1)),
                    ])
                    .width(w)
                    .height(h)
                    .padding([0, 2])
                    .align_y(Vertical::Center)
                    .into(),
                );
            }
        }
        iced::widget::mouse_area(stack(layers).width(w).height(h))
            .on_press(Msg::Open(item.id.clone()))
            .on_enter(Msg::RowOver(Some("hero".to_owned())))
            .on_exit(Msg::RowOver(None))
            .interaction(iced::mouse::Interaction::Pointer)
            .into()
    }

    fn continue_cards(&self, lib: &api::Library, progress: &Progress) -> Vec<Card> {
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
                        progress: Some(e.fraction()),
                        watched: false,
                        moving: m.unavailable,
                        removable: true,
                        marked: self.watchlist.contains(&self.key(&m.id)),
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
                if let Some(n) = next_up(s, progress, &|i| self.key(i)) {
                    let season = &s.seasons[n.si];
                    let ep = &season.episodes[n.ei];
                    let code = api::featurette(season.number, ep.number)
                        .unwrap_or_else(|| format!("S{} E{}", season.number, ep.number));
                    out.push(Card {
                        item: s.id.clone(),
                        episode: Some(ep.id.clone()),
                        title: s.title.clone(),
                        sub: Some(format!("{code} · {}", ep.title)),
                        art: ep
                            .still
                            .clone()
                            .or(s.thumb.clone())
                            .or(s.backdrop.clone())
                            .or(s.poster.clone()),
                        progress: Some(n.resume.map_or(0.0, Entry::fraction)),
                        watched: false,
                        moving: s.unavailable || ep.unavailable,
                        removable: true,
                        marked: self.watchlist.contains(&self.key(&s.id)),
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
                progress: (i.kind == Kind::Movie)
                    .then(|| {
                        progress
                            .get(&self.key(&i.id))
                            .filter(|e| e.in_progress())
                            .map(Entry::fraction)
                    })
                    .flatten(),
                watched: self.watched(i, progress),
                moving: i.unavailable,
                removable: false,
                marked: self.watchlist.contains(&self.key(&i.id)),
            })
            .collect()
    }

    fn status_of(&self, i: &Item, progress: &Progress) -> Filter {
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

    /// Every movie or series, filtered and sorted.
    fn grid_page<'a>(
        &'a self,
        lib: &'a api::Library,
        kind: Kind,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let all = if kind == Kind::Movie {
            &lib.movies
        } else {
            &lib.series
        };
        let mut genres: Vec<String> = all.iter().flat_map(|i| i.genres.iter().cloned()).collect();
        genres.sort();
        genres.dedup();
        // a genre this list doesn't have counts as all of them
        let genre = self.genre.as_ref().filter(|g| genres.contains(g));
        let in_genre: Vec<&Item> = all
            .iter()
            .filter(|i| genre.is_none_or(|g| i.genres.contains(g)))
            .collect();
        let status: Vec<Filter> = in_genre
            .iter()
            .map(|i| self.status_of(i, env.progress))
            .collect();
        let count = |f: Filter| status.iter().filter(|s| **s == f).count();
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

        // egui: the filter strip on the left; on the right, from the edge:
        // genre, its label, direction, sort, its label
        let chips = widgets::segmented(
            [
                (Filter::All, "All", in_genre.len()),
                (Filter::InProgress, "In progress", count(Filter::InProgress)),
                (Filter::Unwatched, "Unwatched", count(Filter::Unwatched)),
                (Filter::Watched, "Watched", count(Filter::Watched)),
            ]
            .into_iter()
            .map(|(f, label, n)| (label.to_owned(), Some(n), self.filter == f, Msg::Filter(f)))
            .collect(),
        );
        let mut sorts = vec![
            SortOpt(Sort::Title, "A – Z"),
            SortOpt(Sort::Year, "Release year"),
        ];
        if all.iter().any(|i| i.added.is_some()) {
            sorts.push(SortOpt(Sort::Added, "Recently added"));
        }
        if all.iter().any(|i| maker(i).is_some()) {
            sorts.push(SortOpt(Sort::Director, maker_label));
        }
        let current = sorts.iter().find(|s| s.0 == self.sort).copied();
        let (dir_icon, dir_label) = if self.desc {
            (Icon::ArrowDown, "Descending")
        } else {
            (Icon::ArrowUp, "Ascending")
        };
        let mut controls = row![
            widgets::label("Sort", 11.0),
            space().width(10),
            picker(pick_list(sorts, current, Msg::Sort).width(170)),
            space().width(16),
            crate::iced_ui::chrome::tip_below(
                widgets::button_with(
                    widgets::Kind::Small,
                    dir_label,
                    Some(dir_icon),
                    Some(Msg::Desc)
                ),
                "Sort direction",
            ),
        ]
        .align_y(Vertical::Center);
        if !genres.is_empty() {
            let mut opts = vec![ALL_GENRES.to_owned()];
            opts.extend(genres);
            let current = genre.cloned().unwrap_or_else(|| ALL_GENRES.to_owned());
            controls = controls
                .push(space().width(24))
                .push(widgets::label("Genre", 11.0))
                .push(space().width(10))
                .push(picker(
                    pick_list(opts, Some(current), Msg::Genre).width(170),
                ));
        }
        let head = row![chips, space::horizontal(), controls]
            .align_y(Vertical::Center)
            .padding(
                Padding::default()
                    .left(SIDE - 22.0)
                    .right(SIDE - 22.0)
                    .top(18.0)
                    .bottom(36.0),
            );
        if items.is_empty() {
            return column![head, space().height(4), muted_center("Nothing here")].into();
        }
        let mut cards = self.poster_cards(items.iter().copied(), env.progress);
        if self.sort == Sort::Director {
            for (c, i) in cards.iter_mut().zip(&items) {
                c.sub = maker(i).map(str::to_owned);
            }
        }
        let salt = if kind == Kind::Movie {
            "movies"
        } else {
            "series"
        };
        column![head, cards::grid(env.images, &cards, w, self.hot(), salt)].into()
    }

    fn search_page<'a>(&'a self, lib: &'a api::Library, env: Env<'a>, w: f32) -> Element<'a, Msg> {
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
        let mut page = column![row![
            mono(&format!("Results for “{}”", self.search.trim()), 13.0),
            mono(&hits.len().to_string(), 13.0).color(style::MUTED()),
        ]
        .spacing(10)
        .padding(Padding::default().left(SIDE).top(24.0))];
        if !people.is_empty() {
            let mut names = row![
                widgets::label("Directors & creators", 11.0),
                space().width(6)
            ]
            .spacing(10)
            .align_y(Vertical::Center);
            for d in people {
                names = names.push(widgets::button_with(
                    widgets::Kind::Secondary,
                    d,
                    None,
                    Some(Msg::Nav(View::Person(d.to_owned()))),
                ));
            }
            page = page.push(
                container(names.wrap())
                    .padding(Padding::default().left(SIDE).right(SIDE).top(10.0)),
            );
        }
        if hits.is_empty() {
            return page
                .push(space().height(16))
                .push(muted_center("Nothing matches"))
                .into();
        }
        let cards = self.poster_cards(hits.into_iter(), env.progress);
        page.push(space().height(16))
            .push(cards::grid(env.images, &cards, w, self.hot(), "search"))
            .into()
    }

    /// Everything a director or creator made.
    fn person_page<'a>(
        &'a self,
        lib: &'a api::Library,
        name: &str,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let mut items: Vec<&Item> = lib.all().filter(|i| makers(i).any(|m| m == name)).collect();
        sort_by(&mut items, true, |i| i.year);
        let movies = items.iter().filter(|i| i.kind == Kind::Movie).count();
        let series = items.len() - movies;
        let mut what = Vec::new();
        if movies > 0 {
            what.push(format!(
                "{movies} movie{}",
                if movies == 1 { "" } else { "s" }
            ));
        }
        if series > 0 {
            what.push(format!("{series} series"));
        }
        let head = column![
            widgets::label("Director & creator", 11.0),
            text(name.to_owned()).font(BOLD).size(34),
            text(what.join("  ·  ")).color(style::MUTED()),
        ]
        .spacing(4)
        .padding(Padding::default().left(SIDE).top(28.0).bottom(24.0));
        if items.is_empty() {
            return column![head, muted_center("Nothing on the server")].into();
        }
        let cards = self.poster_cards(items.into_iter(), env.progress);
        column![
            head,
            cards::grid(env.images, &cards, w, self.hot(), "person")
        ]
        .into()
    }
}

/// The banner's arrow: a dark box with a chevron, brighter on hover.
fn hero_arrow<'a>(glyph: &str, msg: Msg) -> Element<'a, Msg> {
    button(
        container(text(glyph.to_owned()).size(30))
            .center_x(32)
            .center_y(64),
    )
    .width(32)
    .height(64)
    .padding(0)
    .style(|_, s| {
        let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(Background::Color(black(if hot { 160 } else { 80 }))),
            text_color: Color::WHITE.scale_alpha(if hot { 1.0 } else { 0.7 }),
            border: iced::border::rounded(2),
            ..button::Style::default()
        }
    })
    .on_press(msg)
    .into()
}

/// How long the app has run, for spinners.
fn spin_secs() -> f32 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

fn fill_gradient(angle: f32, from: Color, to: Color) -> container::Style {
    container::Style {
        background: Some(Background::Gradient(
            iced::gradient::Linear::new(iced::Radians(angle))
                .add_stop(0.0, from)
                .add_stop(1.0, to)
                .into(),
        )),
        ..container::Style::default()
    }
}

/// A gray line in the middle of the page ("Nothing here").
fn muted_center<'a>(t: &str) -> Element<'a, Msg> {
    container(mono(t, 12.0).color(style::MUTED()))
        .padding(Padding::default().top(40.0))
        .center_x(Length::Fill)
        .into()
}

/// A round avatar with the name's initial (egui's `chrome::avatar`).
pub fn avatar<'a>(name: &str, r: f32) -> Element<'a, Msg> {
    let initial = name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    container(
        text(initial)
            .font(style::BLACK)
            .size(r * 1.05)
            .color(style::TEXT()),
    )
    .center_x(2.0 * r)
    .center_y(2.0 * r)
    .style(move |_| container::Style {
        background: Some(Background::Color(c(theme::pal().accent))),
        border: iced::border::rounded(r),
        ..container::Style::default()
    })
    .into()
}

/// A drop-down panel (menus): egui's popup frame.
pub(super) fn popup_style(_: &iced::Theme) -> container::Style {
    let p = theme::pal();
    container::Style {
        background: Some(Background::Color(c(p.surface))),
        border: Border {
            color: c(p.line_hi),
            width: 1.0,
            radius: 4.0.into(),
        },
        shadow: iced::Shadow {
            color: black(120),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        text_color: Some(c(p.text)),
        ..container::Style::default()
    }
}

/// A menu's dividing line.
fn separator<'a>() -> Element<'a, Msg> {
    container(space())
        .width(Length::Fill)
        .height(1)
        .padding([0, 0])
        .style(|_| container::Style {
            background: Some(Background::Color(style::LINE())),
            ..container::Style::default()
        })
        .into()
}

/// A drop-down in the app's colors.
fn picker<'a, T, L, V>(p: iced::widget::PickList<'a, T, L, V, Msg>) -> Element<'a, Msg>
where
    T: ToString + PartialEq + Clone + 'a,
    L: std::borrow::Borrow<[T]> + 'a,
    V: std::borrow::Borrow<T> + 'a,
{
    p.text_size(13)
        .padding([6, 10])
        .style(|_, s| {
            let p = theme::pal();
            let hot = matches!(
                s,
                pick_list::Status::Hovered | pick_list::Status::Opened { .. }
            );
            pick_list::Style {
                text_color: c(p.text),
                placeholder_color: c(p.muted),
                handle_color: c(p.muted),
                background: Background::Color(c(p.field)),
                border: Border {
                    color: if hot { c(p.line_hi) } else { c(p.line) },
                    width: 1.0,
                    radius: 3.0.into(),
                },
            }
        })
        .menu_style(|_| {
            let p = theme::pal();
            iced::overlay::menu::Style {
                background: Background::Color(c(p.surface)),
                border: Border {
                    color: c(p.line_hi),
                    width: 1.0,
                    radius: 3.0.into(),
                },
                text_color: c(p.text),
                selected_text_color: c(p.text),
                selected_background: Background::Color(c(p.surface_hi)),
                shadow: iced::Shadow::default(),
            }
        })
        .into()
}

/// The page's scrollbar: thin, in the theme's line color.
fn scroll_style(_: &iced::Theme, s: scrollable::Status) -> scrollable::Style {
    let hot = matches!(
        s,
        scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. }
    );
    let rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: Background::Color(if hot { style::MUTED() } else { style::LINE() }),
            border: iced::border::rounded(4),
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(black(200)),
            border: iced::border::rounded(20),
            shadow: iced::Shadow::default(),
            icon: Color::WHITE,
        },
    }
}
