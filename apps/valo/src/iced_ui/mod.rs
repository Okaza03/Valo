//! The app's UI, drawn with Iced: the window chrome, the player, setup, the
//! server check, sign-in, settings, updates, the library, profiles and admin,
//! in every app style. What the screens share (the styles, what to show and
//! play) is in `crate::ui`.
//!
//! Frames are presented with `PresentMode::Fifo` (vsync, no tearing), and
//! fullscreen is a borderless window covering the screen, never exclusive mode.
mod check;
mod chrome;
mod dialogs;
mod grow;
mod icon;
mod images;
mod library;
mod login;
mod seekbar;
mod setup;
mod spaced;
mod style;
mod video;
mod watch;
mod widgets;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use iced::keyboard::{self, key::Named, Key, Modifiers};
use iced::widget::{column, container, operation, row, space};
use iced::window::{self, Direction};
use iced::{event, Background, Element, Length, Point, Size, Subscription, Task, Theme};

use crate::api::{Client, Mode};
use crate::audio::AudioOut;
use crate::config::{self, Config, Progress};
use crate::presence;
use crate::ui::watch::PlayItem;
use crate::ui::Action;
use crate::update;
use check::Check;
use icon::Icon;
use images::Images;
use library::Library;
use login::Login;
use setup::Setup;
use watch::Watch;

/// What a screen asks the app to do after a message or a tick.
pub enum Out {
    None,
    Act(Action),
    /// Put this on the clipboard.
    Copy(String),
    /// Move the keyboard to this field.
    Focus(&'static str),
    /// Put this scrollable back at the top.
    Top(&'static str),
    /// Scroll this scrollable sideways by this much.
    ScrollBy(String, f32),
}

/// The screen under the player.
enum Stage {
    Setup(Setup),
    Check(Check),
    Login(Login),
    /// Connected (and signed in when the server wants it).
    Library(Box<Library>),
}

impl Stage {
    /// Waiting on the network: the app ticks to poll it and turn spinners.
    fn busy(&self) -> bool {
        match self {
            Stage::Setup(s) => s.busy(),
            Stage::Check(c) => c.busy(),
            Stage::Login(l) => l.busy(),
            Stage::Library(l) => l.busy(),
        }
    }
}

/// A value computed on a background thread; polled on the app's ticks.
pub struct Job<T>(Receiver<T>);

impl<T: Send + 'static> Job<T> {
    pub fn spawn(f: impl FnOnce() -> T + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        Self(rx)
    }

    pub fn poll(&mut self) -> Option<T> {
        self.0.try_recv().ok()
    }
}

pub fn run(files: Vec<PathBuf>) -> iced::Result {
    // Fifo: wait for the display's refresh, never tear. Iced reads its present
    // mode from here; a value the user set wins.
    if std::env::var_os("ICED_PRESENT_MODE").is_none() {
        std::env::set_var("ICED_PRESENT_MODE", "fifo");
    }
    let cfg = Config::load();
    let scale = cfg.ui_scale.clamp(0.5, 2.0);
    // open the way it was closed: size, place, maximized
    let mut settings = window::Settings {
        // Iced scales the size by the app's zoom; the saved size is the desktop's
        size: Size::new(1280.0 / scale, 780.0 / scale),
        min_size: Some(Size::new(560.0 / scale, 400.0 / scale)),
        decorations: false, // we draw our own title bar
        transparent: false,
        exit_on_close_request: false, // save first, see Message::Close
        icon: window_icon(),
        ..window::Settings::default()
    };
    #[cfg(target_os = "linux")]
    {
        settings.platform_specific.application_id = crate::app_id();
    }
    if let Some(w) = cfg.window.filter(config::WindowState::usable) {
        settings.size = Size::new(w.w / scale, w.h / scale);
        settings.maximized = w.maximized;
        if let (Some(x), Some(y)) = (w.x, w.y) {
            settings.position = window::Position::Specific(Point::new(x, y));
        }
    }
    let mut app = iced::application(move || App::new(files.clone()), App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(App::theme)
        .scale_factor(|a: &App| a.cfg.ui_scale.clamp(0.5, 2.0))
        // solid black behind everything: nothing behind the window shows through
        .style(|_, _| iced::theme::Style {
            background_color: iced::Color::BLACK,
            text_color: style::TEXT(),
        })
        .window(settings)
        .default_font(style::SANS);
    for f in style::FONTS {
        app = app.font(f);
    }
    app.run()
}

#[cfg(windows)]
fn window_icon() -> Option<window::Icon> {
    let png = include_bytes!("../../../../packaging/windows/valo.png");
    let img = image::load_from_memory(png).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    window::icon::from_rgba(img.into_raw(), w, h).ok()
}

/// Linux desktops take the icon from the .desktop file named by the app id,
/// macOS from the app bundle.
#[cfg(not(windows))]
fn window_icon() -> Option<window::Icon> {
    None
}

struct App {
    cfg: Config,
    progress: Progress,
    audio: AudioOut,
    muted: bool,
    watch: Option<Watch>,
    stage: Stage,
    /// A server check waiting for the user to sign in.
    pending_check: Option<Check>,
    window: Option<window::Id>,
    size: Size,
    fullscreen: bool,
    maximized: bool,
    /// The window's last size and place while neither maximized nor fullscreen.
    placed: (Option<Size>, Option<Point>),
    /// Discord Rich Presence.
    presence: presence::Presence,
    /// Posters and backdrops.
    images: Images,
    /// Watch progress, kept in step with the server.
    sync: crate::sync::Sync,
    /// Checking for, offering and installing a newer Valo.
    updates: update::Updates,
    /// Settings is open.
    settings: bool,
    /// The wordmark over the window after signing in.
    splash: Option<dialogs::Splash>,
    /// The desktop is set to light (for the System theme).
    system_light: bool,
}

/// The dialog over the page, if any: Settings first, then the update
/// popup, then "What's new".
#[derive(Clone, Copy, PartialEq)]
enum Modal {
    Settings,
    Update,
    WhatsNew,
}

#[derive(Clone, Debug)]
pub enum Message {
    WindowId(Option<window::Id>),
    Watch(watch::Msg),
    Setup(setup::Msg),
    Check(check::Msg),
    Login(login::Msg),
    Library(library::Msg),
    Dialog(dialogs::Msg),
    /// The desktop's light or dark setting, at start and when it changes.
    SystemTheme(iced::theme::Mode),
    /// A click on the splash: nothing happens.
    Swallow,
    /// Poll the screen's background work.
    Tick,
    /// Reload the library (the status line was clicked).
    Refresh,
    OpenSettings,
    CheckUpdates,
    ShowUpdate,
    /// Escape, even one a text field took.
    Escape,
    Key(Key, Modifiers),
    /// The mouse's back (false) or forward (true) button.
    MouseNav(bool),
    Window(window::Event),
    Maximized(bool),
    OpenFiles,
    Drag,
    ToggleMaximize,
    Minimize,
    Resize(Direction),
    /// The close button or the system asked to close: save, then exit.
    Close,
}

impl App {
    fn new(files: Vec<PathBuf>) -> (Self, Task<Message>) {
        let mut cfg = Config::load();
        if cfg.server.is_empty() && cfg.seen_version.is_empty() {
            // a new install: nothing is new to this user
            cfg.seen_version = update::VERSION.to_owned();
            cfg.save();
        }
        crate::integrate::sync(cfg.app_menu);
        // reconnect to the saved server; files on the command line play first
        let setup = Setup::new(cfg.server.clone(), files.is_empty());
        let focus = if setup.wants_focus() {
            operation::focus(setup::FIELD)
        } else {
            Task::none()
        };
        let watch = (!files.is_empty())
            .then(|| Watch::new(files.into_iter().map(PlayItem::local).collect()));
        let app = Self {
            stage: Stage::Setup(setup),
            pending_check: None,
            maximized: cfg.window.is_some_and(|w| w.maximized),
            cfg,
            progress: Progress::load(),
            audio: AudioOut::new(),
            muted: false,
            watch,
            window: None,
            size: Size::new(1280.0, 780.0),
            fullscreen: false,
            placed: (None, None),
            presence: presence::Presence::new(),
            images: Images::new(),
            sync: crate::sync::Sync::default(),
            updates: {
                let mut u = update::Updates::default();
                u.check(crate::util::Waker::none(), false); // quietly, once per start
                u
            },
            settings: false,
            splash: None,
            system_light: false,
        };
        app.apply_look();
        (
            app,
            Task::batch([
                window::oldest().map(Message::WindowId),
                focus,
                iced::system::theme().map(Message::SystemTheme),
            ]),
        )
    }

    fn title(&self) -> String {
        match self.watch.as_ref().and_then(Watch::title) {
            Some(t) => format!("{t} — Valo"),
            None => "Valo".to_owned(),
        }
    }

    fn theme(&self) -> Theme {
        style::iced_theme()
    }

    fn subscription(&self) -> Subscription<Message> {
        let keys = keyboard::listen().filter_map(|e| match e {
            keyboard::Event::KeyPressed { key, modifiers, .. } => {
                Some(Message::Key(key, modifiers))
            }
            _ => None,
        });
        let escape = event::listen_with(|e, _, _| match e {
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(Named::Escape),
                ..
            }) => Some(Message::Escape),
            _ => None,
        });
        let window = event::listen_with(|e, _, _| match e {
            iced::Event::Window(
                e @ (window::Event::Resized(_)
                | window::Event::Moved(_)
                | window::Event::CloseRequested
                | window::Event::Focused
                | window::Event::FileDropped(_)),
            ) => Some(Message::Window(e)),
            _ => None,
        });
        // playing: a tick per displayed frame; else now and then, for
        // loads finishing, the volume slider closing, a click turning into a pause
        let ticks = match &self.watch {
            Some(w) if w.playing() => window::frames().map(|_| Message::Watch(watch::Msg::Tick)),
            Some(_) => iced::time::every(Duration::from_millis(50))
                .map(|_| Message::Watch(watch::Msg::Tick)),
            None if self.stage.busy() || self.images.busy() => {
                iced::time::every(Duration::from_millis(50)).map(|_| Message::Tick)
            }
            // the library: its sync, refreshes and the featured banner
            None if matches!(self.stage, Stage::Library(_)) => {
                iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick)
            }
            None => Subscription::none(),
        };
        let mouse_nav = event::listen_with(|e, _, _| match e {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Back)) => {
                Some(Message::MouseNav(false))
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Forward)) => {
                Some(Message::MouseNav(true))
            }
            _ => None,
        });
        let system = iced::system::theme_changes().map(Message::SystemTheme);
        // the splash fades a frame at a time; checks and installs report back
        let extra = if self.splash.is_some() && self.watch.is_none() {
            window::frames().map(|_| Message::Tick)
        } else if self.updates.busy() && self.watch.is_none() {
            iced::time::every(Duration::from_millis(100)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        };
        Subscription::batch([keys, escape, mouse_nav, window, ticks, system, extra])
    }

    fn set_fullscreen(&mut self, on: bool) -> Task<Message> {
        let Some(id) = self.window else {
            return Task::none();
        };
        self.fullscreen = on;
        crate::fullscreen::set(on);
        let mode = if on {
            window::Mode::Fullscreen
        } else {
            window::Mode::Windowed
        };
        window::set_mode(id, mode)
    }

    /// Remember the window's size and place for the next start (not while
    /// fullscreen or maximized, which keep the size from before).
    fn note_window(&mut self) {
        let scale = self.cfg.ui_scale.clamp(0.5, 2.0);
        let mut w = self.cfg.window.unwrap_or(config::WindowState {
            w: 1280.0,
            h: 780.0,
            x: None,
            y: None,
            maximized: false,
        });
        w.maximized = self.maximized;
        if !self.maximized && !self.fullscreen {
            if let Some(s) = self.placed.0 {
                w.w = (s.width * scale).round();
                w.h = (s.height * scale).round();
            }
            // minimized windows report odd places (Windows: -32000, -32000)
            if let Some(p) = self.placed.1.filter(|p| p.x > -10000.0 && p.y > -10000.0) {
                w.x = Some((p.x * scale).round());
                w.y = Some((p.y * scale).round());
            }
        }
        self.cfg.window = Some(w);
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let task = match message {
            Message::WindowId(id) => {
                self.window = id;
                match id {
                    Some(id) => install_fullscreen_fix(id),
                    None => Task::none(),
                }
            }
            Message::Watch(msg) => self.update_watch(msg),
            Message::Setup(msg) => match &mut self.stage {
                Stage::Setup(s) => {
                    let out = s.update(msg);
                    self.out(out)
                }
                _ => Task::none(),
            },
            Message::Check(msg) => match &mut self.stage {
                Stage::Check(c) => {
                    let out = c.update(msg);
                    self.out(out)
                }
                _ => Task::none(),
            },
            Message::Login(msg) => match &mut self.stage {
                Stage::Login(l) => {
                    let out = l.update(msg);
                    self.out(out)
                }
                _ => Task::none(),
            },
            Message::Library(msg) => match &mut self.stage {
                Stage::Library(l) => {
                    let out = l.update(msg, &self.progress, &self.cfg);
                    self.out(out)
                }
                _ => Task::none(),
            },
            Message::Dialog(msg) => self.update_dialog(msg),
            Message::SystemTheme(m) => {
                self.system_light = m == iced::theme::Mode::Light;
                self.apply_look();
                Task::none()
            }
            Message::Swallow => Task::none(),
            Message::Tick => {
                if self.updates.poll() {
                    // the new version is starting; it takes over from here
                    return self.update(Message::Close);
                }
                let quiet = self.splash.is_none() && self.modal().is_none();
                if let Stage::Library(l) = &mut self.stage {
                    l.set_quiet(quiet);
                }
                if let Some(sp) = &mut self.splash {
                    let (ready, hold) = match &self.stage {
                        Stage::Library(l) => (l.settled(), l.has_library()),
                        _ => (true, false),
                    };
                    if !sp.tick(ready, hold) {
                        self.splash = None;
                    }
                }
                self.images.poll();
                let out = match &mut self.stage {
                    Stage::Setup(s) => s.tick(),
                    Stage::Check(c) => c.tick(),
                    Stage::Login(l) => l.tick(),
                    Stage::Library(l) => {
                        self.sync.tick(
                            crate::util::Waker::none(),
                            l.client(),
                            l.user(),
                            &mut self.progress,
                            false,
                        );
                        l.tick(&self.cfg, &mut self.images)
                    }
                };
                self.out(out)
            }
            Message::Refresh => self.handle(Action::Refresh),
            Message::OpenSettings => self.handle(Action::OpenSettings),
            Message::CheckUpdates => self.handle(Action::CheckUpdates),
            Message::ShowUpdate => self.handle(Action::ShowUpdate),
            Message::Escape if self.watch.is_none() && self.modal().is_some() => {
                self.update_dialog(dialogs::Msg::Close)
            }
            Message::Escape => match (&self.watch, &mut self.stage) {
                (None, Stage::Library(l)) if l.popup_close().is_some() => {
                    let msg = l.popup_close().expect("checked");
                    let out = l.update(msg, &self.progress, &self.cfg);
                    self.out(out)
                }
                (None, Stage::Library(l)) => {
                    let out = l.escape();
                    self.out(out)
                }
                _ => Task::none(),
            },
            Message::Key(key, mods) => {
                if self.watch.is_some() {
                    self.update_watch(watch::Msg::Key(key, mods))
                } else if mods.command() && matches!(key.as_ref(), Key::Character("o" | "O")) {
                    self.update(Message::OpenFiles)
                } else if let Stage::Library(l) = &mut self.stage {
                    match key.as_ref() {
                        Key::Character("f" | "F") if mods.command() => {
                            operation::focus(library::SEARCH)
                        }
                        Key::Named(Named::Backspace) => {
                            let out = l.key_back();
                            self.out(out)
                        }
                        _ => Task::none(),
                    }
                } else {
                    Task::none()
                }
            }
            // like Backspace: the player leaves fullscreen, then closes; a
            // dialog closes; the library goes back (or forward)
            Message::MouseNav(forward) => {
                if self.watch.is_some() {
                    if forward {
                        return Task::none();
                    }
                    let back = Key::Named(Named::Backspace);
                    self.update_watch(watch::Msg::Key(back, Modifiers::empty()))
                } else if self.modal().is_some() {
                    if forward {
                        return Task::none();
                    }
                    self.update_dialog(dialogs::Msg::Close)
                } else if let Stage::Library(l) = &mut self.stage {
                    let out = if forward { l.forward() } else { l.key_back() };
                    self.out(out)
                } else {
                    Task::none()
                }
            }
            Message::Window(e) => match e {
                window::Event::Resized(s) => {
                    self.size = s;
                    if !self.fullscreen {
                        self.placed.0 = Some(s);
                    }
                    self.check_maximized()
                }
                window::Event::Moved(p) => {
                    if !self.fullscreen {
                        self.placed.1 = Some(p);
                    }
                    self.check_maximized()
                }
                window::Event::FileDropped(path) => {
                    match self.watch.as_mut() {
                        Some(w) => w.add_dropped(path),
                        None => self.watch = Some(Watch::new(vec![PlayItem::local(path)])),
                    }
                    Task::none()
                }
                window::Event::CloseRequested => self.update(Message::Close),
                window::Event::Focused => {
                    if let Stage::Library(l) = &mut self.stage {
                        l.focused();
                    }
                    Task::none()
                }
                _ => Task::none(),
            },
            Message::Maximized(m) => {
                // the size and place from before maximizing stay remembered
                if m && !self.maximized {
                    self.placed = (None, None);
                }
                self.maximized = m;
                self.note_window();
                Task::none()
            }
            Message::OpenFiles => self.handle(Action::OpenFiles),
            Message::Drag => self.window.map_or_else(Task::none, window::drag),
            Message::ToggleMaximize => self.window.map_or_else(Task::none, window::toggle_maximize),
            Message::Minimize => self
                .window
                .map_or_else(Task::none, |id| window::minimize(id, true)),
            Message::Resize(dir) => self
                .window
                .map_or_else(Task::none, |id| window::drag_resize(id, dir)),
            Message::Close => {
                self.on_exit();
                match self.window {
                    Some(id) => window::close(id).chain(iced::exit()),
                    None => iced::exit(),
                }
            }
        };
        self.update_presence();
        task
    }

    fn out(&mut self, out: Out) -> Task<Message> {
        match out {
            Out::None => Task::none(),
            Out::Act(a) => self.handle(a),
            Out::Copy(t) => iced::clipboard::write(t),
            Out::Focus(id) => operation::focus(id),
            Out::Top(id) => operation::snap_to(id, operation::RelativeOffset::START),
            Out::ScrollBy(id, dx) => {
                operation::scroll_by(id, operation::AbsoluteOffset { x: dx, y: 0.0 })
            }
        }
    }

    /// Save the address from the setup or check screen. A different server
    /// forgets the old one's session.
    fn remember_server(&mut self) {
        let address = match &self.stage {
            Stage::Setup(s) => s.address(),
            Stage::Check(c) => c.address(),
            _ => return,
        }
        .trim()
        .to_owned();
        if address != self.cfg.server {
            self.cfg.server = address;
            self.cfg.token = None; // belonged to the old server
            self.cfg.admin = false;
        }
        self.cfg.save();
    }

    /// The update control: the refresh icon that checks for a new Valo, a
    /// spinner while it does, and an "Update" tab when one is out.
    fn update_button(&self) -> Element<'_, Message> {
        let mut r = row![].align_y(iced::alignment::Vertical::Center);
        if let Some(rel) = &self.updates.available {
            r = r.push(space().width(8)).push(chrome::tip_below(
                widgets::tab(
                    &format!("Update {}", rel.version),
                    true,
                    Message::ShowUpdate,
                ),
                &format!("Valo {} is available — click to update", rel.version),
            ));
            r = r.push(space().width(8));
        }
        if self.updates.checking() {
            r = r.push(
                container(icon::spinner(spin_secs(), style::TEXT(), 16.0)).center(chrome::BAR_H),
            );
        } else {
            r = r.push(chrome::bar_button(
                Icon::Refresh,
                "Check for updates",
                Message::CheckUpdates,
            ));
        }
        r.into()
    }

    /// Show the library of `client`'s server, the wordmark over it while it loads.
    fn open_library(&mut self, client: Client, user: Option<String>) {
        self.images.set_client(Some(client.clone()));
        self.stage = Stage::Library(Box::new(Library::new(client, user)));
        self.splash = Some(dialogs::Splash::new());
    }

    /// Show the style and theme from the settings (System: as the desktop is).
    fn apply_look(&self) {
        crate::ui::theme::set(
            self.cfg.style,
            dialogs::resolved(self.cfg.theme, self.system_light),
        );
    }

    /// The dialog to show now, if any.
    fn modal(&self) -> Option<Modal> {
        if self.settings {
            return Some(Modal::Settings);
        }
        if self.splash.is_some() {
            return None;
        }
        if self.updates.popup().is_some() {
            return Some(Modal::Update);
        }
        match &self.stage {
            Stage::Library(l) if l.settled() && self.cfg.seen_version != update::VERSION => {
                Some(Modal::WhatsNew)
            }
            _ => None,
        }
    }

    fn update_dialog(&mut self, msg: dialogs::Msg) -> Task<Message> {
        use dialogs::Msg as D;
        if dialogs::apply(&mut self.cfg, &msg) {
            self.cfg.save();
            self.apply_look();
            return Task::none();
        }
        match msg {
            D::Close => match self.modal() {
                Some(Modal::Settings) => self.settings = false,
                Some(Modal::Update) => self
                    .updates
                    .choose(update::UpdateChoice::Later, crate::util::Waker::none()),
                Some(Modal::WhatsNew) => {
                    self.cfg.seen_version = update::VERSION.to_owned();
                    self.cfg.save();
                }
                None => {}
            },
            D::Update(pick) => self.updates.choose(
                match pick {
                    dialogs::UpdatePick::Update => update::UpdateChoice::Update,
                    dialogs::UpdatePick::Download => update::UpdateChoice::Download,
                    dialogs::UpdatePick::Later => update::UpdateChoice::Later,
                },
                crate::util::Waker::none(),
            ),
            D::Dismiss(download) => self.updates.dismiss(download),
            D::Open(url) => crate::ui::chrome::open_url(&url),
            D::ChangeServer => {
                self.settings = false;
                return self.handle(Action::ChangeServer);
            }
            D::SignOut => {
                self.settings = false;
                if let Stage::Library(l) = &self.stage {
                    let c = l.client().clone();
                    return self.handle(Action::SignOut(c));
                }
            }
            D::OpenFiles => {
                self.settings = false;
                return self.handle(Action::OpenFiles);
            }
            D::RefreshNow => {
                self.settings = false;
                return self.handle(Action::Refresh);
            }
            D::Quit => return self.update(Message::Close),
            _ => {}
        }
        Task::none()
    }

    /// The connection Settings describes.
    fn conn(&self) -> Option<dialogs::Conn> {
        let conn = |c: &Client, user: Option<&str>| dialogs::Conn {
            name: c.info.name.clone(),
            base: c.base.clone(),
            user: user.map(str::to_owned),
            signed_in: c.token.is_some(),
        };
        match &self.stage {
            Stage::Library(l) => Some(conn(l.client(), l.user())),
            Stage::Login(l) => Some(conn(l.client(), None)),
            Stage::Setup(_) | Stage::Check(_) => None,
        }
    }

    /// Sign in to `client`'s server, the keyboard in the first empty field.
    fn login(&mut self, client: Client, error: Option<String>) -> Task<Message> {
        let l = Login::new(client, self.cfg.username.clone(), error);
        let focus = operation::focus(l.first_field());
        self.stage = Stage::Login(l);
        focus
    }

    /// What the screens ask for.
    fn handle(&mut self, action: Action) -> Task<Message> {
        match action {
            Action::CheckServer(address) => {
                self.pending_check = None;
                self.stage = Stage::Check(Check::new(address));
            }
            Action::Connected(mut client) => {
                self.remember_server();
                if matches!(self.stage, Stage::Check(_)) {
                    // a newly added server: sign in, then (admins) finish the check
                    let check = std::mem::replace(
                        &mut self.stage,
                        Stage::Setup(Setup::new(String::new(), false)),
                    );
                    if let Stage::Check(c) = check {
                        self.pending_check = Some(c);
                    }
                    return self.login(client, None);
                } else if !client.info.auth {
                    self.open_library(client, None);
                } else if let Some(t) = self.cfg.token.clone() {
                    client.token = Some(t);
                    client.admin = self.cfg.admin;
                    let user = Some(self.cfg.username.clone()).filter(|u| !u.is_empty());
                    self.open_library(client, user);
                } else {
                    return self.login(client, None);
                }
            }
            Action::SignedIn {
                client,
                user,
                remember,
            } => {
                self.cfg.username = user.clone();
                self.cfg.token = if remember { client.token.clone() } else { None };
                self.cfg.admin = remember && client.admin;
                self.cfg.save();
                // a new server's check goes on after signing in for admins
                // only: everyone else goes straight to the library
                match self.pending_check.take() {
                    Some(mut c) if client.admin => {
                        c.signed_in(client, Some(user));
                        self.stage = Stage::Check(c);
                    }
                    _ => self.open_library(client, Some(user)),
                }
            }
            Action::Ready { client, user } => {
                self.remember_server();
                self.open_library(client, user);
            }
            Action::NeedLogin(mut client) => {
                client.token = None;
                client.admin = false;
                self.cfg.token = None;
                self.cfg.admin = false;
                self.cfg.save();
                let msg = (client.info.mode == Mode::Api)
                    .then(|| "Your session has expired. Please sign in again.".to_owned());
                return self.login(client, msg);
            }
            Action::AdminChanged(admin) => {
                if self.cfg.token.is_some() {
                    self.cfg.admin = admin;
                    self.cfg.save();
                }
            }
            Action::SignOut(mut client) => {
                let c = client.clone();
                std::thread::spawn(move || c.logout());
                client.token = None;
                client.admin = false;
                self.cfg.token = None;
                self.cfg.admin = false;
                self.cfg.save();
                return self.login(client, None);
            }
            Action::ChangeServer => {
                // keep what the user typed if they're backing out of a check
                let pending = self.pending_check.take();
                let address = match (&self.stage, &pending) {
                    (Stage::Check(c), _) | (_, Some(c)) => c.address().to_owned(),
                    _ => self.cfg.server.clone(),
                };
                self.stage = Stage::Setup(Setup::new(address, false));
                return operation::focus(setup::FIELD);
            }
            Action::OpenFiles => {
                if let Some(files) = crate::pick_files() {
                    self.watch = Some(Watch::new(files.into_iter().map(PlayItem::local).collect()));
                }
            }
            Action::Play(items) => {
                let fs = if self.cfg.fullscreen_on_play {
                    self.set_fullscreen(true)
                } else {
                    Task::none()
                };
                self.watch = Some(Watch::new(items));
                return fs;
            }
            Action::Quit => return self.update(Message::Close),
            Action::Refresh => {
                if let Stage::Library(l) = &mut self.stage {
                    l.reload();
                }
                self.sync.pull_now();
            }
            Action::OpenSettings => self.settings = true,
            Action::CheckUpdates => self.updates.check(crate::util::Waker::none(), true),
            Action::ShowUpdate => self.updates.show(),
        }
        Task::none()
    }

    fn check_maximized(&self) -> Task<Message> {
        self.window.map_or_else(Task::none, |id| {
            window::is_maximized(id).map(Message::Maximized)
        })
    }

    fn update_watch(&mut self, msg: watch::Msg) -> Task<Message> {
        let Some(w) = self.watch.as_mut() else {
            return Task::none();
        };
        let mut env = watch::Env {
            audio: &self.audio,
            cfg: &mut self.cfg,
            muted: &mut self.muted,
            progress: &mut self.progress,
            fullscreen: self.fullscreen,
        };
        match w.update(msg, &mut env) {
            watch::Out::None => Task::none(),
            watch::Out::Close => {
                self.watch = None;
                self.cfg.save();
                self.sync.push_now();
                if self.fullscreen {
                    self.set_fullscreen(false)
                } else {
                    Task::none()
                }
            }
            watch::Out::Fullscreen(on) => self.set_fullscreen(on),
            watch::Out::Drag => self.update(Message::Drag),
            watch::Out::ToggleMaximize => self.update(Message::ToggleMaximize),
        }
    }

    /// Discord: what's playing, else browsing, else nothing.
    fn update_presence(&mut self) {
        let activity = if !self.cfg.discord_presence {
            None
        } else if let Some(w) = &self.watch {
            w.now_playing().map(|(title, sub, pos, dur, paused)| {
                presence::Activity::watching(title, sub, pos, dur, paused)
            })
        } else if matches!(self.stage, Stage::Library(_)) {
            Some(presence::Activity::browsing())
        } else {
            None
        };
        self.presence.set(activity);
    }

    fn on_exit(&mut self) {
        if let Some(w) = self.watch.as_mut() {
            w.save_progress(&mut self.progress);
        }
        self.watch = None;
        if let Stage::Library(l) = &self.stage {
            self.sync.finish(l.client(), l.user(), &mut self.progress);
        }
        self.progress.flush();
        self.note_window();
        self.cfg.save();
    }

    fn view(&self) -> Element<'_, Message> {
        let framed = !self.maximized && !self.fullscreen;
        if self.watch.is_none() {
            let page = self.with_dialogs(self.screen());
            return chrome::frame(page, self.size, framed);
        }
        let content: Element<Message> = match &self.watch {
            Some(w) => w
                .view(watch::View {
                    volume: self.cfg.volume,
                    muted: self.muted,
                    skip: f64::from(self.cfg.skip_seconds.max(1)),
                    fullscreen: self.fullscreen,
                })
                .map(Message::Watch),
            None => self.screen(),
        };
        chrome::frame(content, self.size, framed)
    }

    /// `page` with the open dialog, the update notice and the splash over it.
    fn with_dialogs<'a>(&'a self, page: Element<'a, Message>) -> Element<'a, Message> {
        let mut page = page;
        if let Some(m) = self.modal() {
            let conn = self.conn();
            let (content, width) = match m {
                Modal::Settings => (
                    dialogs::settings(&self.cfg, conn.as_ref(), self.system_light),
                    500.0,
                ),
                Modal::Update => {
                    let (rel, in_place, flatpak) = self.updates.popup().expect("popup is up");
                    (dialogs::update_available(rel, in_place, flatpak), 460.0)
                }
                Modal::WhatsNew => (dialogs::whats_new(&self.cfg.seen_version), 460.0),
            };
            page = dialogs::modal(
                page,
                content.map(Message::Dialog),
                width,
                self.size,
                Message::Dialog(dialogs::Msg::Close),
            );
        }
        if let (None, Stage::Library(l)) = (self.modal(), &self.stage) {
            let env = library::Env {
                images: &self.images,
                progress: &self.progress,
                cfg: &self.cfg,
            };
            if let Some((content, width, close)) = l.popup(env) {
                page = dialogs::modal(
                    page,
                    content.map(Message::Library),
                    width,
                    self.size,
                    Message::Library(close),
                );
            }
        }
        if let Some(t) = self.updates.toast() {
            page = iced::widget::stack![page, dialogs::toast(&t, spin_secs()).map(Message::Dialog)]
                .into();
        }
        if let Some(sp) = &self.splash {
            page = iced::widget::stack![page, sp.view(self.size, Message::Swallow)].into();
        }
        page
    }

    /// The title bar, the current screen, and the status line.
    fn screen(&self) -> Element<'_, Message> {
        let name = match &self.stage {
            Stage::Login(l) => l.client().info.name.clone(),
            Stage::Library(l) => l.client().info.name.clone(),
            _ => "Valo".to_owned(),
        };
        let update = self.update_button();
        let settings = chrome::bar_button(Icon::Settings, "Settings", Message::OpenSettings);
        let (left, right): (Element<Message>, Element<Message>) = match &self.stage {
            Stage::Library(l) => (
                l.bar_left().map(Message::Library),
                row![
                    space().width(10),
                    l.search_field().map(Message::Library),
                    space().width(10),
                    chrome::vsep(),
                    update,
                    chrome::vsep(),
                    settings,
                    chrome::vsep(),
                    l.profile_button().map(Message::Library),
                ]
                .align_y(iced::alignment::Vertical::Center)
                .into(),
            ),
            _ => (
                column![].into(),
                row![update, chrome::vsep(), settings]
                    .align_y(iced::alignment::Vertical::Center)
                    .into(),
            ),
        };
        let bar = chrome::title_bar(&name, self.maximized, left, right);
        let body: Element<Message> = match &self.stage {
            Stage::Setup(s) => s.view().map(Message::Setup),
            Stage::Check(c) => c.view().map(Message::Check),
            Stage::Login(l) => l.view().map(Message::Login),
            Stage::Library(l) => l
                .view(library::Env {
                    images: &self.images,
                    progress: &self.progress,
                    cfg: &self.cfg,
                })
                .map(Message::Library),
        };
        let body = container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(style::BG())),
                ..container::Style::default()
            });
        let left = match &self.stage {
            Stage::Login(l) => {
                format!("Connected to {}  ·  sign-in required", l.client().info.name)
            }
            Stage::Check(c) => format!("Checking {}", c.address()),
            Stage::Library(l) => l.status().0,
            Stage::Setup(_) => "Not connected".to_owned(),
        };
        let (right, refresh) = match &self.stage {
            Stage::Library(l) => (l.status().1, l.can_refresh().then_some(Message::Refresh)),
            _ => (
                format!(
                    "Valo {}  ·  {} {}",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS,
                    std::env::consts::ARCH
                ),
                None,
            ),
        };
        let page: Element<Message> =
            column![bar, body, chrome::status_bar(left, right, refresh)].into();
        match &self.stage {
            Stage::Library(l) => match l.menu_overlay() {
                Some(menu) => iced::widget::stack![page, menu.map(Message::Library)].into(),
                None => page,
            },
            _ => page,
        }
    }
}

/// How long the app has run, for spinners.
fn spin_secs() -> f32 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs_f32()
}

/// On Windows, keep the taskbar's strip from showing white when a maximized
/// window goes fullscreen (see `crate::fullscreen`).
#[cfg(windows)]
fn install_fullscreen_fix(id: window::Id) -> Task<Message> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    window::run(id, |w| {
        if let Ok(h) = w.window_handle() {
            if let RawWindowHandle::Win32(h) = h.as_raw() {
                crate::fullscreen::install_hwnd(h.hwnd.get());
            }
        }
    })
    .discard()
}

#[cfg(not(windows))]
fn install_fullscreen_fix(_: window::Id) -> Task<Message> {
    Task::none()
}
