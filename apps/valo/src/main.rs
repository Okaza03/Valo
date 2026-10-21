// no console window behind the app on Windows (debug builds keep it for logs)
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod addons;
mod api;
mod audio;
mod config;
mod images;
mod inhibit;
mod integrate;
mod player;
mod presence;
mod subs;
mod sync;
mod ui;
mod update;
mod util;

use std::path::PathBuf;

use eframe::egui;

use api::{Client, Mode};
use audio::AudioOut;
use config::{Config, Progress};
use images::Images;
use ui::browse::Browse;
use ui::check::Check;
use ui::chrome::{self, Icon};
use ui::login::Login;
use ui::settings::{self as dialogs, Conn};
use ui::setup::Setup;
use ui::splash::Splash;
use ui::watch::{PlayItem, Watch};
use ui::Action;
use update::Updates;

fn main() -> eframe::Result<()> {
    let paths: Vec<PathBuf> = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .collect();
    // Linux desktops take the icon from the .desktop file named by the app id
    #[cfg(windows)]
    let viewport = egui::ViewportBuilder::default().with_icon(window_icon());
    #[cfg(not(windows))]
    let viewport = egui::ViewportBuilder::default();
    // open the way it was closed: size, place, maximized
    let viewport = match Config::load().window.filter(|w| w.usable()) {
        Some(w) => {
            let v = viewport
                .with_inner_size([w.w, w.h])
                .with_maximized(w.maximized);
            match (w.x, w.y) {
                (Some(x), Some(y)) => v.with_position([x, y]),
                _ => v,
            }
        }
        None => viewport.with_inner_size([1280.0, 780.0]),
    };
    let options = eframe::NativeOptions {
        viewport: viewport
            .with_min_inner_size([560.0, 400.0])
            .with_title("Valo")
            .with_decorations(false) // we draw our own title bar
            .with_transparent(false)
            .with_app_id(app_id()),
        ..Default::default()
    };
    eframe::run_native(
        "Valo",
        options,
        Box::new(move |cc| {
            ui::apply_theme(&cc.egui_ctx);
            Ok(Box::new(App::new(&cc.egui_ctx, paths)))
        }),
    )
}

/// Names the .desktop file the window's icon comes from: the Flatpak's renamed
/// copy of valo.desktop, the AppImage's own entry, or valo.desktop.
fn app_id() -> String {
    if let Ok(id) = std::env::var("FLATPAK_ID") {
        id
    } else if integrate::appimage().is_some() {
        integrate::APP_ID.into()
    } else {
        "valo".into()
    }
}

/// Title bar, taskbar and Alt+Tab icon.
#[cfg(windows)]
fn window_icon() -> egui::IconData {
    let png = include_bytes!("../../../packaging/windows/valo.png");
    let img = image::load_from_memory(png)
        .expect("bundled icon is a valid PNG")
        .into_rgba8();
    egui::IconData {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    }
}

pub fn pick_files() -> Option<Vec<PathBuf>> {
    let home = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let start = std::env::var_os(home).map(|h| PathBuf::from(h).join("Videos"));
    let mut dlg = rfd::FileDialog::new()
        .add_filter("Video", api::VIDEO_EXTS)
        .add_filter("All files", &["*"]);
    if let Some(s) = start.filter(|s| s.is_dir()) {
        dlg = dlg.set_directory(s);
    }
    dlg.pick_files()
}

enum Stage {
    Setup(Setup),
    Check(Check),
    Login(Login),
    Browse(Box<Browse>),
}

struct App {
    cfg: Config,
    progress: Progress,
    sync: sync::Sync,
    audio: AudioOut,
    images: Images,
    muted: bool,
    stage: Stage,
    /// A server check waiting for the user to sign in.
    pending_check: Option<Check>,
    watch: Option<Watch>,
    /// The wordmark over the window after signing in.
    splash: Option<Splash>,
    dialog: Option<Dialog>,
    updates: Updates,
    /// Discord Rich Presence.
    presence: presence::Presence,
}

#[derive(Clone, Copy, PartialEq)]
enum Dialog {
    Settings,
}

impl App {
    /// Keep track of the window's size, place and maximized state, for the next start.
    /// Fullscreen (the player) isn't remembered, and doesn't overwrite the rest.
    fn note_window(&mut self, ctx: &egui::Context) {
        let (inner, outer, maximized, fullscreen, minimized) = ctx.input(|i| {
            let v = i.viewport();
            (
                v.inner_rect,
                v.outer_rect,
                v.maximized.unwrap_or(false),
                v.fullscreen.unwrap_or(false),
                v.minimized.unwrap_or(false),
            )
        });
        // minimized windows report odd places (Windows: -32000, -32000)
        if fullscreen || minimized {
            return;
        }
        // egui's points include the app's zoom (Settings > text size); the desktop's don't
        let zoom = ctx.zoom_factor();
        let mut w = self.cfg.window.unwrap_or(config::WindowState {
            w: 1280.0,
            h: 780.0,
            x: None,
            y: None,
            maximized: false,
        });
        w.maximized = maximized;
        if !maximized {
            if let Some(r) = inner {
                w.w = (r.width() * zoom).round();
                w.h = (r.height() * zoom).round();
            }
            if let Some(r) = outer {
                w.x = Some((r.min.x * zoom).round());
                w.y = Some((r.min.y * zoom).round());
            }
        }
        self.cfg.window = Some(w);
    }

    fn new(ctx: &egui::Context, files: Vec<PathBuf>) -> Self {
        let mut cfg = Config::load();
        if cfg.server.is_empty() && cfg.seen_version.is_empty() {
            // a new install: nothing is new to this user
            cfg.seen_version = update::VERSION.to_owned();
            cfg.save();
        }
        integrate::sync(cfg.app_menu);
        // reconnect to the saved server; files on the command line play first
        let setup = Setup::new(ctx, cfg.server.clone(), files.is_empty());
        let watch = (!files.is_empty())
            .then(|| Watch::new(files.into_iter().map(PlayItem::local).collect()));
        Self {
            progress: Progress::load(),
            sync: sync::Sync::default(),
            audio: AudioOut::new(),
            images: Images::new(ctx),
            muted: false,
            stage: Stage::Setup(setup),
            pending_check: None,
            watch,
            splash: None,
            cfg,
            dialog: None,
            presence: presence::Presence::new(),
            updates: {
                let mut u = Updates::default();
                u.check(ctx, false); // quietly, once per start
                u
            },
        }
    }

    fn browse(&mut self, ctx: &egui::Context, client: Client, user: Option<String>) {
        self.images.set_client(Some(client.clone()));
        self.stage = Stage::Browse(Box::new(Browse::new(ctx, client, user)));
        self.splash = Some(Splash::new(ctx));
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

    fn conn(&self) -> Option<Conn> {
        let conn = |c: &Client, user: Option<&str>| Conn {
            name: c.info.name.clone(),
            base: c.base.clone(),
            user: user.map(str::to_owned),
            signed_in: c.token.is_some(),
        };
        match &self.stage {
            Stage::Browse(b) => Some(conn(b.client(), b.user())),
            Stage::Login(l) => Some(conn(l.client(), None)),
            Stage::Setup(_) | Stage::Check(_) => None,
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(d) = self.dialog else { return };
        let conn = self.conn();
        let out = match (d, &conn) {
            (Dialog::Settings, _) => dialogs::settings(ctx, &mut self.cfg, conn.as_ref()),
        };
        match out {
            dialogs::Pick::Keep => {}
            dialogs::Pick::Close => self.dialog = None,
            dialogs::Pick::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            dialogs::Pick::Refresh => {
                self.dialog = None;
                self.handle(ctx, Action::Refresh);
            }
            dialogs::Pick::OpenFiles => {
                self.dialog = None;
                self.handle(ctx, Action::OpenFiles);
            }
            dialogs::Pick::ChangeServer => {
                self.dialog = None;
                self.handle(ctx, Action::ChangeServer);
            }
            dialogs::Pick::SignOut => {
                self.dialog = None;
                if let Stage::Browse(b) = &self.stage {
                    let c = b.client().clone();
                    self.handle(ctx, Action::SignOut(c));
                }
            }
        }
    }

    fn handle(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::None => {}
            Action::Refresh => {
                if let Stage::Browse(b) = &mut self.stage {
                    b.reload(ctx);
                }
                self.sync.pull_now();
            }
            Action::OpenSettings => self.dialog = Some(Dialog::Settings),
            Action::CheckUpdates => self.updates.check(ctx, true),
            Action::ShowUpdate => self.updates.show(),
            Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Action::CheckServer(address) => {
                self.pending_check = None;
                self.stage = Stage::Check(Check::new(ctx, address));
            }
            Action::Connected(mut client) => {
                self.remember_server();
                if matches!(self.stage, Stage::Check(_)) {
                    // a newly added server: sign in, then finish the check
                    let login = Login::new(client, self.cfg.username.clone(), None);
                    if let Stage::Check(c) = std::mem::replace(&mut self.stage, Stage::Login(login))
                    {
                        self.pending_check = Some(c);
                    }
                } else if !client.info.auth {
                    self.browse(ctx, client, None);
                } else if let Some(t) = self.cfg.token.clone() {
                    client.token = Some(t);
                    client.admin = self.cfg.admin;
                    let user = Some(self.cfg.username.clone()).filter(|u| !u.is_empty());
                    self.browse(ctx, client, user);
                } else {
                    self.stage = Stage::Login(Login::new(client, self.cfg.username.clone(), None));
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
                match self.pending_check.take() {
                    Some(mut c) => {
                        c.signed_in(ctx, client, Some(user));
                        self.stage = Stage::Check(c);
                    }
                    None => self.browse(ctx, client, Some(user)),
                }
            }
            Action::Ready { client, user } => {
                self.remember_server();
                self.browse(ctx, client, user);
            }
            Action::NeedLogin(mut client) => {
                client.token = None;
                client.admin = false;
                self.cfg.token = None;
                self.cfg.admin = false;
                self.cfg.save();
                self.images.set_client(None);
                let msg = (client.info.mode == Mode::Api)
                    .then(|| "Your session has expired. Please sign in again.".to_owned());
                self.stage = Stage::Login(Login::new(client, self.cfg.username.clone(), msg));
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
                self.images.set_client(None);
                self.stage = Stage::Login(Login::new(client, self.cfg.username.clone(), None));
            }
            Action::ChangeServer => {
                self.images.set_client(None);
                // keep what the user typed if they're backing out of a check
                let pending = self.pending_check.take();
                let address = match (&self.stage, &pending) {
                    (Stage::Check(c), _) | (_, Some(c)) => c.address().to_owned(),
                    _ => self.cfg.server.clone(),
                };
                self.stage = Stage::Setup(Setup::new(ctx, address, false));
            }
            Action::OpenFiles => {
                if let Some(files) = pick_files() {
                    self.watch = Some(Watch::new(files.into_iter().map(PlayItem::local).collect()));
                }
            }
            Action::Play(items) => {
                if self.cfg.fullscreen_on_play {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                }
                self.watch = Some(Watch::new(items));
            }
        }
    }
}

impl eframe::App for App {
    /// Solid black, not eframe's see-through default: with a half-transparent
    /// clear, desktops can show what's behind the window (or the taskbar's
    /// previews) through it while the player redraws.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.images.poll(ctx);
        self.note_window(ctx);
        ui::set_look(ctx, self.cfg.style, self.cfg.theme);
        if (ctx.zoom_factor() - self.cfg.ui_scale).abs() > 0.001 {
            ctx.set_zoom_factor(self.cfg.ui_scale.clamp(0.5, 2.0));
        }

        // Discord: what's playing, else browsing, else nothing
        let activity = if !self.cfg.discord_presence {
            None
        } else if let Some(w) = &self.watch {
            w.now_playing().map(|(title, sub, pos, dur, paused)| {
                presence::Activity::watching(title, sub, pos, dur, paused)
            })
        } else if matches!(self.stage, Stage::Browse(_)) {
            Some(presence::Activity::browsing())
        } else {
            None
        };
        self.presence.set(activity);

        if let Stage::Browse(b) = &self.stage {
            let watching = self.watch.is_some();
            self.sync
                .tick(ctx, b.client(), b.user(), &mut self.progress, watching);
        }

        if let Some(w) = self.watch.as_mut() {
            if w.ui(
                ctx,
                &self.audio,
                &mut self.cfg,
                &mut self.muted,
                &mut self.progress,
            ) {
                self.watch = None;
                self.cfg.save();
                self.sync.push_now();
                ctx.send_viewport_cmd(egui::ViewportCommand::Title("Valo".into()));
            }
            chrome::window_frame(ctx);
            return;
        }

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            self.watch = Some(Watch::new(
                dropped.into_iter().map(PlayItem::local).collect(),
            ));
        }

        if !matches!(self.stage, Stage::Browse(_)) {
            let (mut open_settings, mut click) = (false, None);
            let name = match &self.stage {
                Stage::Login(l) => l.client().info.name.clone(),
                _ => "Valo".to_owned(),
            };
            chrome::title_bar(
                ctx,
                &name,
                |_| {},
                |ui| {
                    open_settings = chrome::icon_button(ui, Icon::Settings, "Settings").clicked();
                    chrome::vsep(ui);
                    click = chrome::update_button(ui, &self.updates);
                },
            );
            if open_settings {
                self.dialog = Some(Dialog::Settings);
            }
            match click {
                Some(update::Click::Check) => self.updates.check(ctx, true),
                Some(update::Click::ShowUpdate) => self.updates.show(),
                None => {}
            }
            let left = match &self.stage {
                Stage::Login(l) => {
                    format!("Connected to {}  ·  sign-in required", l.client().info.name)
                }
                Stage::Check(c) => format!("Checking {}", c.address()),
                _ => "Not connected".to_owned(),
            };
            let right = format!(
                "Valo {}  ·  {} {}",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH
            );
            chrome::status_bar(ctx, &left, &right, false);
        }

        // "What's new" and new arrivals wait for the splash and other dialogs
        let whats_new = match &self.stage {
            Stage::Browse(b) if self.cfg.seen_version != update::VERSION => {
                ui::whats_new::pending(&self.cfg.seen_version)
            }
            _ => false,
        };
        let popups = self.splash.is_none() && self.dialog.is_none() && !whats_new;
        let action = match &mut self.stage {
            Stage::Setup(s) => s.ui(ctx),
            Stage::Check(c) => c.ui(ctx),
            Stage::Login(l) => l.ui(ctx),
            Stage::Browse(b) => b.ui(
                ctx,
                &mut self.images,
                &self.progress,
                &self.cfg,
                &self.updates,
                popups,
            ),
        };
        self.handle(ctx, action);
        if let Some(s) = &mut self.splash {
            let (ready, hold) = match &self.stage {
                Stage::Browse(b) => (b.settled(), b.has_library()),
                _ => (true, false),
            };
            if !s.ui(ctx, ready, hold) {
                self.splash = None;
            }
        }
        self.updates.ui(ctx, popups);
        self.dialogs(ctx);
        if let Stage::Browse(b) = &self.stage {
            let quiet = self.splash.is_none() && self.dialog.is_none() && b.settled();
            let seen = self.cfg.seen_version != update::VERSION;
            if seen && quiet && (!whats_new || ui::whats_new::ui(ctx, &self.cfg.seen_version)) {
                self.cfg.seen_version = update::VERSION.to_owned();
                self.cfg.save();
            }
        }
        chrome::window_frame(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(w) = self.watch.as_mut() {
            w.save_progress(&mut self.progress);
        }
        self.watch = None;
        if let Stage::Browse(b) = &self.stage {
            self.sync.finish(b.client(), b.user(), &mut self.progress);
        }
        self.progress.flush();
        self.cfg.save();
    }
}
