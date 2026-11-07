//! The Iced UI, built with `--features iced`. It's being ported from the egui
//! UI (`crate::ui`) one screen at a time; so far it has the window chrome and
//! the player. The library, sign-in and settings screens are still egui-only.
//!
//! Frames are presented with `PresentMode::Fifo` (vsync, no tearing), and
//! fullscreen is a borderless window covering the screen, never exclusive mode.
mod chrome;
mod icon;
mod seekbar;
mod style;
mod video;
mod watch;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use iced::alignment::Horizontal;
use iced::keyboard::{self, Key, Modifiers};
use iced::widget::{button, column, container, text};
use iced::window::{self, Direction};
use iced::{event, Background, Element, Length, Point, Size, Subscription, Task, Theme};

use crate::audio::AudioOut;
use crate::config::{self, Config, Progress};
use crate::presence;
use crate::ui::watch::PlayItem;
use watch::Watch;

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

/// Linux desktops take the icon from the .desktop file named by the app id.
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
    window: Option<window::Id>,
    size: Size,
    fullscreen: bool,
    maximized: bool,
    /// The window's last size and place while neither maximized nor fullscreen.
    placed: (Option<Size>, Option<Point>),
    /// Discord Rich Presence.
    presence: presence::Presence,
}

#[derive(Clone, Debug)]
pub enum Message {
    WindowId(Option<window::Id>),
    Watch(watch::Msg),
    Key(Key, Modifiers),
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
        let cfg = Config::load();
        let watch = (!files.is_empty())
            .then(|| Watch::new(files.into_iter().map(PlayItem::local).collect()));
        let app = Self {
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
        };
        (app, window::oldest().map(Message::WindowId))
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
        let window = event::listen_with(|e, _, _| match e {
            iced::Event::Window(
                e @ (window::Event::Resized(_)
                | window::Event::Moved(_)
                | window::Event::CloseRequested
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
            None => Subscription::none(),
        };
        Subscription::batch([keys, window, ticks])
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
            Message::Key(key, mods) => {
                if self.watch.is_some() {
                    self.update_watch(watch::Msg::Key(key, mods))
                } else if mods.command() && matches!(key.as_ref(), Key::Character("o" | "O")) {
                    self.update(Message::OpenFiles)
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
                        Some(w) => w.add_files(vec![path]),
                        None => self.watch = Some(Watch::new(vec![PlayItem::local(path)])),
                    }
                    Task::none()
                }
                window::Event::CloseRequested => self.update(Message::Close),
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
            Message::OpenFiles => {
                if let Some(files) = crate::pick_files() {
                    self.watch = Some(Watch::new(files.into_iter().map(PlayItem::local).collect()));
                }
                Task::none()
            }
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

    /// Discord: what's playing, else nothing.
    fn update_presence(&mut self) {
        let activity = if !self.cfg.discord_presence {
            None
        } else {
            self.watch.as_ref().and_then(Watch::now_playing).map(
                |(title, sub, pos, dur, paused)| {
                    presence::Activity::watching(title, sub, pos, dur, paused)
                },
            )
        };
        self.presence.set(activity);
    }

    fn on_exit(&mut self) {
        if let Some(w) = self.watch.as_mut() {
            w.save_progress(&mut self.progress);
        }
        self.watch = None;
        self.progress.flush();
        self.note_window();
        self.cfg.save();
    }

    fn view(&self) -> Element<'_, Message> {
        let framed = !self.maximized && !self.fullscreen;
        let content: Element<Message> = match &self.watch {
            Some(w) => w
                .view(watch::View {
                    volume: self.cfg.volume,
                    muted: self.muted,
                    skip: f64::from(self.cfg.skip_seconds.max(1)),
                    fullscreen: self.fullscreen,
                })
                .map(Message::Watch),
            None => self.home(),
        };
        chrome::frame(content, self.size, framed)
    }

    /// Nothing playing: what this build can do so far.
    fn home(&self) -> Element<'_, Message> {
        let bar = chrome::title_bar("Valo", self.maximized, column![].into());
        let open = button(text("Open videos…").size(15))
            .padding([10, 22])
            .style(|_, s| button::Style {
                background: Some(Background::Color(if matches!(s, button::Status::Hovered) {
                    style::c(crate::ui::theme::pal().primary).scale_alpha(0.85)
                } else {
                    style::c(crate::ui::theme::pal().primary)
                })),
                text_color: style::c(crate::ui::theme::pal().ink),
                border: iced::border::rounded(f32::from(crate::ui::theme::spec().button_radius)),
                ..button::Style::default()
            })
            .on_press(Message::OpenFiles);
        let body = container(
            column![
                text("Drop a video here or press Ctrl+O")
                    .size(18)
                    .color(style::MUTED()),
                open,
            ]
            .spacing(18)
            .align_x(Horizontal::Center),
        )
        .center(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(style::BG())),
            ..container::Style::default()
        });
        column![bar, body].into()
    }
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
