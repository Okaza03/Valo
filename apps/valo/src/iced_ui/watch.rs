//! Full-window playback of a queue (an episode run, a movie, or local files):
//! the Iced port of `ui::watch`. The playback logic is the same; the controls
//! are laid out at the same spots, per the chosen style's player.
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use iced::alignment::{Horizontal, Vertical};
use iced::keyboard::{key::Named, Key, Modifiers};
use iced::widget::canvas::{self as cv, Frame, Geometry, Text};
use iced::widget::{
    button, canvas, column, container, mouse_area, pin, responsive, row, scrollable, shader, space,
    stack, text, tooltip, vertical_slider, Space,
};
use iced::{
    gradient, mouse, Background, Color, Element, Length, Padding, Point, Radians, Rectangle,
    Renderer, Size, Theme,
};

use super::icon::{self, Icon};
use super::seekbar::{self, SeekBar};
use super::style::{self, black, gray, white, BOLD, SANS};
use super::video::{self, Video};
use super::Job;
use crate::addons;
use crate::api::SubtitleTrack;
use crate::audio::AudioOut;
use crate::config::{Config, Progress, MOVIE_FINISHED};
use crate::inhibit::Inhibit;
use crate::player::{Player, Source};
use crate::subs::{self, Subs};
use crate::ui::theme::{self, PlayerLayout, Skin, Times, TitleAt};
use crate::ui::watch::PlayItem;
use crate::util::fmt_time;

const TOP_H: f32 = 64.0;
const BOTTOM_H: f32 = 120.0;
/// A row in the player's menus.
const MENU_ROW: f32 = 34.0;
/// A line of the Info panel; long values wrap.
const INFO_ROW: f32 = 24.0;
/// How long the volume slider stays up after ↑/↓.
const VOL_SHOW: Duration = Duration::from_secs(3);
/// A click on the video waits this long for a second one (a double click
/// toggles fullscreen) before it plays or pauses.
const DOUBLE_CLICK: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Menu {
    Settings,
    Subtitles,
    /// What's in the file: container, video and audio formats.
    Info,
}

#[derive(Clone, Debug)]
pub enum Msg {
    Tick,
    Key(Key, Modifiers),
    /// The pointer moved: bring the controls back.
    Moved,
    VideoPress,
    VideoDouble,
    TogglePlay,
    /// Skip back (-1) or forward (1) by the configured seconds.
    Skip(f64),
    SeekDrag(f64),
    SeekDrop,
    Volume(f32),
    ToggleMute,
    VolHover(bool),
    ToggleFullscreen,
    Close,
    Next,
    Prev,
    Pick(usize),
    ToggleList,
    OpenFiles,
    /// Open this menu, or close it when it's the one open.
    Menu(Menu),
    /// Switch the open menu to this one.
    MenuGo(Menu),
    CloseMenu,
    /// A subtitle track picked (None: off).
    SubPick(Option<usize>),
    /// Shift the subtitles by this many seconds (0: back in sync).
    Delay(f64),
    /// The top bar was pressed: move the window.
    Drag,
    /// The top bar was double-clicked: maximize or restore.
    ToggleMaximize,
}

/// What the app should do after a message.
#[derive(PartialEq)]
pub enum Out {
    None,
    /// Leave the player.
    Close,
    /// Enter (true) or leave (false) fullscreen.
    Fullscreen(bool),
    Drag,
    ToggleMaximize,
}

/// What the player borrows from the app.
pub struct Env<'a> {
    pub audio: &'a AudioOut,
    pub cfg: &'a mut Config,
    pub muted: &'a mut bool,
    pub progress: &'a mut Progress,
    pub fullscreen: bool,
}

/// The addons' subtitles for an item, or why they couldn't be asked.
type AddonSubs = Result<Vec<SubtitleTrack>, String>;

/// The subtitle track that's on, and its file.
enum SubState {
    Off,
    Loading(usize, Job<Result<Subs>>),
    Ready(usize, Subs),
    Failed(usize, String),
}

impl SubState {
    fn track(&self) -> Option<usize> {
        match self {
            SubState::Off => None,
            SubState::Loading(i, _) | SubState::Ready(i, _) | SubState::Failed(i, _) => Some(*i),
        }
    }
}

pub struct Watch {
    items: Vec<PlayItem>,
    current: Option<usize>,
    player: Option<Player>,
    opening: Option<(usize, Job<Result<Player>>)>,
    queued: Option<usize>,
    /// The episode left with Next: saved as watched to the end.
    moved_on: Option<usize>,
    frame: Option<Arc<video::Frame>>,
    serial: u64,
    seek_drag: Option<f64>,
    status: String,
    show_list: bool,
    started: Instant,
    last_tick: Instant,
    last_motion: Instant,
    last_save: Instant,
    menu: Option<Menu>,
    /// The pointer is over the volume button or its slider.
    vol_over: bool,
    /// When the pointer last left them: the slider stays a moment.
    vol_left: Option<Instant>,
    /// Last time ↑/↓ changed the volume: the slider shows for a while after.
    vol_key: Option<Instant>,
    /// Keeps the screen from blanking or sleeping while playing.
    inhibit: Inhibit,
    subs: SubState,
    /// The addons' subtitles for an item, on their way.
    addon_subs: Option<(usize, Job<AddonSubs>)>,
    /// Why the addons couldn't be asked about an item's subtitles.
    addon_note: Option<(usize, String)>,
    /// Subtitles were picked or turned off by hand for this video.
    subs_chosen: bool,
    /// Seconds the subtitles are shifted: negative shows them earlier.
    sub_delay: f64,
    /// When the delay last changed, to show it for a moment.
    delay_changed: Option<Instant>,
    /// A click on the video waiting out the double-click window.
    click_at: Option<Instant>,
}

impl Watch {
    pub fn new(items: Vec<PlayItem>) -> Self {
        let now = Instant::now();
        Self {
            show_list: items.iter().any(|i| i.key.is_none()),
            queued: (!items.is_empty()).then_some(0),
            items,
            current: None,
            player: None,
            opening: None,
            frame: None,
            serial: 0,
            seek_drag: None,
            status: String::new(),
            started: now,
            last_tick: now,
            last_motion: now,
            last_save: now,
            moved_on: None,
            menu: None,
            vol_over: false,
            vol_left: None,
            vol_key: None,
            inhibit: Inhibit::new(),
            subs: SubState::Off,
            addon_subs: None,
            addon_note: None,
            subs_chosen: false,
            sub_delay: 0.0,
            delay_changed: None,
            click_at: None,
        }
    }

    /// Files chosen in the file picker: they play next, from the first.
    pub fn add_files(&mut self, files: Vec<PathBuf>) {
        self.add(files, true);
    }

    /// A dropped file. Iced sends one event per file, so dropping several
    /// arrives as several calls: playback starts from the first of them.
    pub fn add_dropped(&mut self, file: PathBuf) {
        self.add(vec![file], false);
    }

    /// `replace`: play from these files even when another start is pending.
    fn add(&mut self, files: Vec<PathBuf>, replace: bool) {
        let first = self.items.len();
        self.items.extend(files.into_iter().map(PlayItem::local));
        if self.items.len() > first {
            if replace || self.queued.is_none() {
                self.queued = Some(first);
            }
            self.show_list = true;
        }
    }

    /// Playing (not paused): the app redraws every frame instead of now and then.
    pub fn playing(&self) -> bool {
        self.player.as_ref().is_some_and(|p| !p.paused)
    }

    /// The window title: what's playing.
    pub fn title(&self) -> Option<String> {
        let item = self.items.get(self.current?)?;
        Some(if item.subtitle.is_empty() {
            item.title.clone()
        } else {
            format!("{} — {}", item.title, item.subtitle)
        })
    }

    /// What's playing, for Discord: title, subtitle, position, length, paused.
    pub fn now_playing(&self) -> Option<(&str, &str, f64, f64, bool)> {
        let (i, p) = (self.current?, self.player.as_ref()?);
        let item = &self.items[i];
        Some((
            &item.title,
            &item.subtitle,
            p.clock(),
            p.meta.duration,
            p.paused,
        ))
    }

    pub fn save_progress(&mut self, progress: &mut Progress) {
        let (Some(i), Some(p)) = (self.current, self.player.as_ref()) else {
            return;
        };
        let item = &self.items[i];
        if let Some(key) = &item.key {
            let dur = p.meta.duration;
            let done = p.finished()
                // moved on with Next: this episode counts as watched
                || self.moved_on == Some(i)
                || (!item.episode && dur > 0.0 && p.clock() >= dur * MOVIE_FINISHED);
            let pos = if done && dur > 0.0 { dur } else { p.clock() };
            if pos > 1.0 {
                progress.set(key, pos, dur);
            }
        }
        self.last_save = Instant::now();
    }

    /// Turn on track `t` of the current item (None: off) and fetch its file.
    fn set_subs(&mut self, t: Option<usize>) {
        self.sub_delay = 0.0; // another file, its own timing
        let item = self.current.and_then(|i| self.items.get(i));
        self.subs = match (item, t) {
            (Some(item), Some(t)) if t < item.subtitles.len() => {
                let track = item.subtitles[t].clone();
                let auth = match &item.source {
                    Source::Url { auth, .. } if !track.external => auth.clone(),
                    _ => None, // never hand our token to an addon
                };
                SubState::Loading(t, Job::spawn(move || subs::load(&track, auth.as_deref())))
            }
            _ => SubState::Off,
        };
    }

    /// A pick from the menu (or C), remembered for the next video.
    fn choose_subs(&mut self, cfg: &mut Config, t: Option<usize>) {
        let item = self.current.and_then(|i| self.items.get(i));
        let track = item.and_then(|it| t.and_then(|t| it.subtitles.get(t)));
        cfg.subtitle_language = track.map(|tr| tr.language.clone());
        if let Some(tr) = track {
            cfg.subtitle_sdh = tr.sdh;
        }
        cfg.save();
        self.subs_chosen = true;
        self.set_subs(t);
    }

    fn open_index(&mut self, i: usize, env: &mut Env) {
        if self.opening.is_some() {
            self.queued = Some(i); // one open at a time; picked up when it lands
            return;
        }
        self.save_progress(env.progress);
        self.moved_on = None;
        self.player = None; // drop (and join) the old decoder first
        self.frame = None;
        self.current = Some(i);
        self.status.clear();
        let Some(item) = self.items.get(i) else {
            return;
        };
        let t = subs::pick(
            &item.subtitles,
            env.cfg.subtitle_language.as_deref(),
            env.cfg.subtitle_sdh,
        );
        self.set_subs(t);
        self.subs_chosen = false;
        self.addon_subs = None;
        let item = &self.items[i];
        if let Some(l) = item.addons.clone() {
            self.addon_subs = Some((i, Job::spawn(move || addons::subtitles(&l))));
        }
        let (src, shared) = (item.source.clone(), env.audio.shared.clone());
        self.opening = Some((i, Job::spawn(move || Player::open(&src, shared))));
    }

    fn toggle_play(&mut self) {
        if let Some(p) = self.player.as_mut() {
            if p.finished() {
                p.seek(0.0);
                p.set_paused(false);
            } else {
                let paused = p.paused;
                p.set_paused(!paused);
            }
        }
    }

    fn skip(&mut self, delta: f64) {
        if let Some(p) = self.player.as_mut() {
            let mut t = p.clock() + delta;
            if p.meta.duration > 0.0 {
                t = t.min(p.meta.duration);
            }
            p.seek(t);
        }
    }

    fn has_next(&self) -> bool {
        self.current.is_some_and(|i| i + 1 < self.items.len())
    }

    fn vol_shown(&self) -> bool {
        let keyed = self.vol_key.is_some_and(|t| t.elapsed() < VOL_SHOW);
        let hovered = self.vol_over
            || self
                .vol_left
                .is_some_and(|t| t.elapsed() < Duration::from_millis(350));
        keyed || hovered
    }

    fn hide_ui(&self, fullscreen: bool) -> bool {
        let keyed = self.vol_key.is_some_and(|t| t.elapsed() < VOL_SHOW);
        let idle = Duration::from_millis(if fullscreen { 2500 } else { 3500 });
        self.playing()
            && !keyed
            && self.menu.is_none()
            && !self.vol_shown()
            && self.seek_drag.is_none()
            && self.last_motion.elapsed() > idle
    }

    /// C: subtitles on (the language used last, else the first) or off.
    fn subs_toggle_pick(&self, cfg: &Config) -> Option<usize> {
        if self.subs.track().is_some() {
            return None;
        }
        let it = &self.items[self.current?];
        let lang = cfg
            .subtitle_language
            .as_deref()
            .or_else(|| it.subtitles.first().map(|t| t.language.as_str()));
        Some(subs::pick(&it.subtitles, lang, cfg.subtitle_sdh).unwrap_or(0))
    }

    pub fn update(&mut self, msg: Msg, env: &mut Env) -> Out {
        let skip = f64::from(env.cfg.skip_seconds.max(1));
        let mut out = Out::None;
        match msg {
            Msg::Tick => self.tick(env),
            Msg::Key(key, mods) => return self.key(&key, mods, skip, env),
            Msg::Moved => self.last_motion = Instant::now(),
            Msg::VideoPress => {
                if self.menu.is_some() {
                    self.menu = None;
                } else {
                    self.click_at = Some(Instant::now());
                }
            }
            Msg::VideoDouble => {
                self.click_at = None;
                out = Out::Fullscreen(!env.fullscreen);
            }
            Msg::TogglePlay => self.toggle_play(),
            Msg::Skip(dir) => self.skip(dir * skip),
            Msg::SeekDrag(t) => self.seek_drag = Some(t),
            Msg::SeekDrop => {
                if let (Some(t), Some(p)) = (self.seek_drag.take(), self.player.as_mut()) {
                    p.seek(t);
                }
            }
            Msg::Volume(v) => {
                env.cfg.volume = v.clamp(0.0, 1.0);
                *env.muted = false;
            }
            Msg::ToggleMute => *env.muted = !*env.muted,
            Msg::VolHover(over) => {
                self.vol_over = over;
                if !over {
                    self.vol_left = Some(Instant::now());
                }
            }
            Msg::ToggleFullscreen => out = Out::Fullscreen(!env.fullscreen),
            Msg::Close => {
                self.save_progress(env.progress);
                env.progress.flush();
                out = Out::Close;
            }
            Msg::Next => {
                if let Some(i) = self.current.filter(|_| self.has_next()) {
                    // the episode left for the next one counts as watched
                    if self.items[i].episode {
                        self.moved_on = Some(i);
                    }
                    self.open_index(i + 1, env);
                }
            }
            Msg::Prev => {
                let restart = self.player.as_ref().is_some_and(|p| p.clock() > 3.0);
                match self.current {
                    _ if restart => {
                        if let Some(p) = self.player.as_mut() {
                            p.seek(0.0);
                        }
                    }
                    Some(i) if i > 0 => self.open_index(i - 1, env),
                    _ => {}
                }
            }
            Msg::Pick(i) => self.open_index(i, env),
            Msg::ToggleList => self.show_list = !self.show_list,
            Msg::OpenFiles => {
                if let Some(files) = crate::pick_files() {
                    self.add_files(files);
                }
            }
            Msg::Menu(m) => self.menu = (self.menu != Some(m)).then_some(m),
            Msg::MenuGo(m) => self.menu = Some(m),
            Msg::CloseMenu => self.menu = None,
            Msg::SubPick(t) => self.choose_subs(env.cfg, t),
            Msg::Delay(d) => {
                self.sub_delay = if d == 0.0 {
                    0.0
                } else {
                    ((self.sub_delay + d) * 10.0).round() / 10.0
                };
                self.delay_changed = Some(Instant::now());
            }
            Msg::Drag => out = Out::Drag,
            Msg::ToggleMaximize => out = Out::ToggleMaximize,
        }
        // anything done with the controls keeps them up
        if !matches!(out, Out::Close) {
            env.audio.shared.set_volume(env.cfg.volume);
            env.audio.shared.set_muted(*env.muted);
        }
        out
    }

    fn key(&mut self, key: &Key, mods: Modifiers, skip: f64, env: &mut Env) -> Out {
        self.last_motion = Instant::now();
        let step = if mods.shift() { skip * 6.0 } else { skip };
        let ch = match key.as_ref() {
            Key::Character(c) => Some(c.to_lowercase()),
            _ => None,
        };
        let has_tracks = self
            .current
            .and_then(|i| self.items.get(i))
            .is_some_and(|it| !it.subtitles.is_empty());
        let sub_on = self.subs.track().is_some();
        let mut out = Out::None;
        match (key.as_ref(), ch.as_deref()) {
            (_, Some("o")) if mods.command() => return self.update(Msg::OpenFiles, env),
            (Key::Named(Named::Space), _) => self.toggle_play(),
            (Key::Named(Named::ArrowRight), _) => self.skip(step),
            (Key::Named(Named::ArrowLeft), _) => self.skip(-step),
            (Key::Named(Named::ArrowUp), _) => {
                env.cfg.volume = (env.cfg.volume + 0.05).min(1.0);
                self.vol_key = Some(Instant::now());
            }
            (Key::Named(Named::ArrowDown), _) => {
                env.cfg.volume = (env.cfg.volume - 0.05).max(0.0);
                self.vol_key = Some(Instant::now());
            }
            (_, Some("m")) => *env.muted = !*env.muted,
            (_, Some("c")) if has_tracks => {
                let t = self.subs_toggle_pick(env.cfg);
                self.choose_subs(env.cfg, t);
            }
            // Z / X: subtitles earlier / later (Shift: a whole second)
            (_, Some("z")) if sub_on => {
                return self.update(Msg::Delay(if mods.shift() { -1.0 } else { -0.1 }), env)
            }
            (_, Some("x")) if sub_on => {
                return self.update(Msg::Delay(if mods.shift() { 1.0 } else { 0.1 }), env)
            }
            (_, Some("n")) => return self.update(Msg::Next, env),
            (_, Some("p")) => return self.update(Msg::Prev, env),
            (_, Some("l")) => self.show_list = !self.show_list,
            (_, Some("f")) | (Key::Named(Named::F11), _) if !mods.command() => {
                out = Out::Fullscreen(!env.fullscreen)
            }
            // Esc: close a menu, else leave fullscreen. Backspace: leave fullscreen, else go back.
            (Key::Named(Named::Escape), _) if self.menu.is_some() => self.menu = None,
            (Key::Named(Named::Escape | Named::Backspace), _) if env.fullscreen => {
                out = Out::Fullscreen(false)
            }
            (Key::Named(Named::Backspace), _) => return self.update(Msg::Close, env),
            _ => {}
        }
        env.audio.shared.set_volume(env.cfg.volume);
        env.audio.shared.set_muted(*env.muted);
        out
    }

    /// Advance playback: finished opens and loads, the newest due frame, autoplay.
    fn tick(&mut self, env: &mut Env) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;

        // a finished open
        if let Some((i, mut job)) = self.opening.take() {
            match job.poll() {
                None => self.opening = Some((i, job)),
                Some(r) if self.queued.is_some() => drop(r), // superseded, Drop cleans up
                Some(Ok(mut p)) => {
                    let start = self.items[i].start;
                    if start > 0.0 && (p.meta.duration <= 0.0 || start < p.meta.duration - 5.0) {
                        p.seek(start);
                    }
                    self.items[i].start = 0.0;
                    self.player = Some(p);
                }
                Some(Err(e)) => {
                    self.status = format!("Can't play {}: {e:#}", self.items[i].source.name())
                }
            }
        }
        if let SubState::Loading(t, job) = &mut self.subs {
            if let Some(r) = job.poll() {
                let t = *t;
                self.subs = match r {
                    Ok(s) => SubState::Ready(t, s),
                    Err(e) => SubState::Failed(t, format!("{e:#}")),
                };
            }
        }
        if let Some((i, job)) = self.addon_subs.as_mut() {
            if let Some(found) = job.poll() {
                let i = *i;
                self.addon_subs = None;
                let item = &mut self.items[i];
                item.addons = None;
                match found {
                    Ok(found) => item.subtitles.extend(found),
                    Err(e) => self.addon_note = Some((i, e)),
                }
                // nothing on yet: the language used last may be here now
                if self.current == Some(i) && self.subs.track().is_none() && !self.subs_chosen {
                    let t = subs::pick(
                        &item.subtitles,
                        env.cfg.subtitle_language.as_deref(),
                        env.cfg.subtitle_sdh,
                    );
                    if t.is_some() {
                        self.set_subs(t);
                    }
                }
            }
        }
        if self.opening.is_none() {
            if let Some(q) = self.queued.take() {
                self.open_index(q, env);
            }
        }

        // advance playback, hand the newest due frame to the GPU
        let mut finished = false;
        if let Some(p) = self.player.as_mut() {
            p.tick(dt);
            if let Some(f) = p.take_frame() {
                self.serial += 1;
                self.frame = Some(Arc::new(video::Frame {
                    serial: self.serial,
                    w: f.w as u32,
                    h: f.h as u32,
                    rgba: f.rgba,
                }));
            }
            finished = p.finished() && !p.paused;
        }
        if finished {
            self.save_progress(env.progress);
            match self.current {
                Some(i) if env.cfg.autoplay_next && i + 1 < self.items.len() => {
                    self.open_index(i + 1, env)
                }
                _ => {
                    if let Some(p) = self.player.as_mut() {
                        p.set_paused(true);
                    }
                }
            }
        }
        if self.last_save.elapsed() > Duration::from_secs(5) {
            self.save_progress(env.progress);
        }
        if self.click_at.is_some_and(|t| t.elapsed() >= DOUBLE_CLICK) {
            self.click_at = None;
            self.toggle_play();
        }
        self.inhibit.set(self.playing());
    }

    /// Draws the player. `volume`, `muted` and `skip` come from the settings.
    pub fn view(&self, now: View) -> Element<'_, Msg> {
        let hide = self.hide_ui(now.fullscreen);
        let screen = responsive(move |size| self.screen(size, now, hide));
        let body: Element<Msg> = if self.show_list && self.items.len() > 1 && !hide {
            row![screen, self.queue()].into()
        } else {
            screen.into()
        };
        mouse_area(body)
            .on_move(|_| Msg::Moved)
            .interaction(if hide {
                mouse::Interaction::Hidden
            } else {
                mouse::Interaction::default()
            })
            .into()
    }

    fn queue(&self) -> Element<'_, Msg> {
        let mut list = column![].spacing(2);
        for (i, it) in self.items.iter().enumerate() {
            let label = if it.subtitle.is_empty() {
                it.title.clone()
            } else {
                it.subtitle.clone()
            };
            let on = self.current == Some(i);
            list = list.push(
                button(
                    text(label)
                        .size(14)
                        .color(if on { Color::WHITE } else { gray(200) }),
                )
                .width(Length::Fill)
                .padding([6, 10])
                .style(move |t, s| {
                    let mut st = style::bare(white(18), 5.0)(t, s);
                    if on {
                        st.background = Some(Background::Color(white(30)));
                    }
                    st
                })
                .on_press(Msg::Pick(i)),
            );
        }
        let head = row![
            text("Up next").font(style::MONO).size(12).color(gray(170)),
            space::horizontal(),
            tip(
                button(text("Add…").size(12))
                    .padding([3, 10])
                    .style(style::bare(white(24), 5.0))
                    .on_press(Msg::OpenFiles),
                "Add files (Ctrl+O)",
                tooltip::Position::Bottom,
            ),
        ]
        .align_y(Vertical::Center);
        container(column![head, scrollable(list)].spacing(8))
            .width(260)
            .height(Length::Fill)
            .padding(10)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgb8(18, 18, 20))),
                border: iced::Border {
                    color: gray(36),
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            })
            .into()
    }

    /// Everything over the video, placed at the same spots as the egui player.
    fn screen(&self, size: Size, now: View, hide: bool) -> Element<'_, Msg> {
        let rect = R::new(0.0, 0.0, size.width, size.height);
        let pos = self.player.as_ref().map_or(0.0, Player::clock);
        let dur = self.player.as_ref().map_or(0.0, |p| p.meta.duration);
        let mut layers: Vec<Element<Msg>> = Vec::new();

        // ---- the picture, or what's happening instead
        if let Some(f) = &self.frame {
            layers.push(
                shader(Video(f.clone()))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
            );
            let dest = fit(rect, f.w as f32, f.h as f32);
            if let SubState::Ready(_, subs) = &self.subs {
                if let Some(t) = subs.at(pos - self.sub_delay) {
                    // above the controls while they're up
                    let mut base = dest.bottom().min(rect.bottom()) - dest.h * 0.06;
                    if !hide {
                        base = base.min(rect.bottom() - BOTTOM_H - 4.0);
                    }
                    layers.push(
                        canvas(Subtitle {
                            text: t,
                            dest,
                            base,
                        })
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into(),
                    );
                }
            }
        } else if !self.status.is_empty() {
            layers.push(centered(
                text(self.status.clone()).size(18).color(style::ERROR()),
            ));
        } else if self.opening.is_some() || self.player.is_some() {
            let name = self
                .current
                .map(|i| self.items[i].title.clone())
                .unwrap_or_default();
            let t = self.started.elapsed().as_secs_f32();
            layers.push(centered(
                column![
                    icon::spinner(t, style::TEXT(), 40.0),
                    text(name).size(16).color(style::MUTED()),
                ]
                .spacing(8)
                .align_x(Horizontal::Center),
            ));
        } else {
            layers.push(centered(
                text("Drop a video here or press Ctrl+O")
                    .size(18)
                    .color(gray(128)),
            ));
        }

        // clicks on the video: play/pause, double: fullscreen
        layers.push(
            mouse_area(space().width(Length::Fill).height(Length::Fill))
                .on_press(Msg::VideoPress)
                .on_double_click(Msg::VideoDouble)
                .into(),
        );

        // the new timing, for a moment after Z / X
        if self
            .delay_changed
            .is_some_and(|t| t.elapsed() < Duration::from_secs(2))
        {
            let label = format!(
                "Subtitles: {}",
                delay_text(self.sub_delay).replace("Timing: ", "")
            );
            layers.push(place(
                rect.center_x(),
                TOP_H + 24.0,
                0.5,
                0.5,
                container(text(label).size(16).color(Color::WHITE))
                    .padding([7, 14])
                    .style(|_| container::Style {
                        background: Some(Background::Color(black(170))),
                        border: iced::border::rounded(8),
                        ..container::Style::default()
                    }),
            ));
        }

        if !hide {
            self.controls(&mut layers, rect, pos, dur, now);
        }
        stack(layers)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn controls<'a>(
        &'a self,
        layers: &mut Vec<Element<'a, Msg>>,
        rect: R,
        pos: f64,
        dur: f64,
        now: View,
    ) {
        let top = R::new(rect.x, rect.y, rect.w, TOP_H);
        let bottom = R::new(rect.x, rect.bottom() - BOTTOM_H, rect.w, BOTTOM_H);
        let shade = |r: R, down: bool| -> Element<'a, Msg> {
            let (dark, fade) = (black(235), black(0));
            let g = gradient::Linear::new(Radians(std::f32::consts::PI));
            let g = if down {
                g.add_stop(0.0, dark)
                    .add_stop(0.5, dark)
                    .add_stop(1.0, fade)
            } else {
                g.add_stop(0.0, fade)
                    .add_stop(0.4, fade)
                    .add_stop(1.0, dark)
            };
            at(
                r,
                container(space())
                    .width(r.w)
                    .height(r.h)
                    .style(move |_| container::Style {
                        background: Some(Background::Gradient(g.into())),
                        ..container::Style::default()
                    }),
            )
        };
        layers.push(shade(R::new(top.x, top.y, top.w, TOP_H + 30.0), true));
        layers.push(shade(
            R::new(bottom.x, bottom.y - 40.0, bottom.w, BOTTOM_H + 40.0),
            false,
        ));

        // laid out like the chosen style's player
        let look = &theme::spec().player;
        let row_layout = look.layout == PlayerLayout::Row;

        // ---- top bar: dragging it moves the window
        layers.push(at(
            top,
            mouse_area(space().width(top.w).height(top.h))
                .on_press(Msg::Drag)
                .on_double_click(Msg::ToggleMaximize),
        ));
        let cy = top.y + 30.0;
        let close_r = R::centered(top.x + 28.0, cy, 36.0);
        let (close_icon, close_tip) = if look.back_arrow {
            (Icon::ChevronLeft, "Back")
        } else {
            (Icon::Close, "Close")
        };
        layers.push(at(
            close_r,
            ctl(
                close_r,
                close_icon,
                close_tip,
                look.discs,
                Msg::Close,
                Below,
            ),
        ));
        let item = self.current.and_then(|i| self.items.get(i));
        if let Some(it) = item {
            let (x, ax) = match look.title {
                TitleAt::TopLeft => (top.x + 58.0, 0.0),
                TitleAt::TopCenter => (top.center_x(), 0.5),
                TitleAt::Bottom => (0.0, -1.0),
            };
            if ax >= 0.0 {
                layers.push(place(
                    x,
                    cy - 2.0,
                    ax,
                    1.0,
                    text(it.title.clone())
                        .font(BOLD)
                        .size(15)
                        .color(Color::WHITE),
                ));
                if !it.subtitle.is_empty() {
                    layers.push(place(
                        x,
                        cy + 2.0,
                        ax,
                        0.0,
                        text(it.subtitle.clone()).size(12.5).color(gray(160)),
                    ));
                }
            }
        }

        // where the rows go: the seek bar, and (Row) the buttons under it
        let row_cy = bottom.bottom() - 32.0;
        let by = if row_layout {
            bottom.bottom() - 68.0
        } else {
            bottom.bottom() - 86.0
        };

        // ---- the other buttons: top right, or the right of the bottom row
        let (slot_y, step) = if row_layout {
            (row_cy, 44.0)
        } else {
            (cy, 40.0)
        };
        let mut x = top.right() - 28.0;
        let mut next_slot = || {
            let r = R::centered(x, slot_y, 36.0);
            x -= step;
            r
        };
        let (more_r, fs_r, vol_r, sub_r);
        if row_layout {
            fs_r = next_slot();
            more_r = next_slot();
            sub_r = next_slot();
            vol_r = R::centered(bottom.x + 168.0, row_cy, 36.0);
        } else {
            more_r = next_slot();
            fs_r = next_slot();
            vol_r = next_slot();
            sub_r = next_slot();
        }
        let list_r = (self.items.len() > 1).then(&mut next_slot);
        let free_right = x + step - 22.0;
        let disc = look.discs && !row_layout;
        let tip_at = if row_layout { Above } else { Below };
        layers.push(at(
            more_r,
            ctl(
                more_r,
                Icon::More,
                "Settings",
                disc,
                Msg::Menu(Menu::Settings),
                tip_at,
            ),
        ));
        let (fs_icon, fs_tip) = if now.fullscreen {
            (Icon::FullscreenExit, "Exit fullscreen (F)")
        } else {
            (Icon::Fullscreen, "Fullscreen (F)")
        };
        layers.push(at(
            fs_r,
            ctl(fs_r, fs_icon, fs_tip, disc, Msg::ToggleFullscreen, tip_at),
        ));
        let vol_icon = if now.muted || now.volume <= 0.0 {
            Icon::VolumeMuted
        } else {
            Icon::Volume
        };
        layers.push(at(
            vol_r,
            mouse_area(ctl(
                vol_r,
                vol_icon,
                "Volume (M to mute)",
                disc,
                Msg::ToggleMute,
                tip_at,
            ))
            .on_enter(Msg::VolHover(true))
            .on_exit(Msg::VolHover(false)),
        ));
        layers.push(at(
            sub_r,
            ctl(
                sub_r,
                Icon::Subtitles,
                "Subtitles",
                disc,
                Msg::Menu(Menu::Subtitles),
                tip_at,
            ),
        ));
        if let Some(lr) = list_r {
            layers.push(at(
                lr,
                ctl(lr, Icon::List, "Queue (L)", disc, Msg::ToggleList, tip_at),
            ));
        }

        // ---- seek bar and times
        let shown = self.seek_drag.unwrap_or(pos);
        let left = (dur - shown).max(0.0);
        let time_col = gray(210);
        let (inset_l, inset_r) = match look.times {
            Times::Split => (66.0, 74.0),
            Times::Left => (24.0, 96.0),
            Times::Both => (16.0, 16.0),
        };
        match look.times {
            Times::Split => {
                layers.push(place(
                    bottom.x + 24.0,
                    by,
                    0.0,
                    0.5,
                    text(fmt_time(shown)).size(12.5).color(time_col),
                ));
                // time left, counting down (while seeking: left after the drop point)
                layers.push(place(
                    bottom.right() - 16.0,
                    by,
                    1.0,
                    0.5,
                    text(format!("-{}", fmt_time(left)))
                        .size(12.5)
                        .color(time_col),
                ));
            }
            Times::Left => layers.push(place(
                bottom.right() - 24.0,
                by,
                1.0,
                0.5,
                text(fmt_time(left)).font(BOLD).size(14).color(Color::WHITE),
            )),
            Times::Both => layers.push(place(
                bottom.x + 200.0,
                row_cy,
                0.0,
                0.5,
                text(format!("{} / {}", fmt_time(shown), fmt_time(dur)))
                    .size(13.5)
                    .color(Color::WHITE),
            )),
        }
        if let (TitleAt::Bottom, Some(it)) = (look.title, item) {
            if row_layout {
                // Netflix: "Title  S1:E2 Episode" in the middle of the button row
                let mut line = row![text(it.title.clone())
                    .font(BOLD)
                    .size(16)
                    .color(Color::WHITE)]
                .spacing(12)
                .align_y(Vertical::Center);
                if !it.subtitle.is_empty() {
                    line = line.push(text(it.subtitle.clone()).size(15).color(gray(190)));
                }
                layers.push(place(bottom.center_x(), row_cy, 0.5, 0.5, line));
            } else {
                // Apple: the title over the bar
                layers.push(place(
                    bottom.x + 24.0,
                    by - 18.0,
                    0.0,
                    1.0,
                    text(it.title.clone())
                        .font(BOLD)
                        .size(19)
                        .color(Color::WHITE),
                ));
                if !it.subtitle.is_empty() {
                    layers.push(place(
                        bottom.x + 24.0,
                        by - 42.0,
                        0.0,
                        1.0,
                        text(it.subtitle.clone()).size(13).color(gray(190)),
                    ));
                }
            }
        }
        let bar = R::new(
            bottom.x + inset_l,
            by - 8.0 - seekbar::ABOVE,
            (bottom.w - inset_l - inset_r).max(10.0),
            seekbar::ABOVE + seekbar::HIT,
        );
        layers.push(at(
            bar,
            canvas(SeekBar {
                shown,
                dur,
                thickness: look.bar,
                played: style::c(theme::played()),
                knob: look.knob.map(style::c),
                always_knob: row_layout,
                on_drag: Msg::SeekDrag,
                on_drop: Msg::SeekDrop,
            })
            .width(bar.w)
            .height(bar.h),
        ));

        // ---- transport: big in the middle, or at the start of the row
        let secs = format!("{}", now.skip as u32);
        let (play_r, back_r, fwd_r) = if row_layout {
            let a = |dx: f32, s: f32| R::centered(bottom.x + dx, row_cy, s);
            (a(34.0, 40.0), a(80.0, 40.0), a(124.0, 40.0))
        } else {
            let (cx, cy) = (bottom.center_x(), bottom.bottom() - 40.0);
            let a = |dx: f32, s: f32| R::centered(cx + dx, cy, s);
            (a(0.0, 56.0), a(-82.0, 44.0), a(82.0, 44.0))
        };
        for (r, ic, dir, tip_text) in [
            (
                back_r,
                Icon::SkipBack,
                -1.0,
                format!("Back {secs} seconds (left arrow key)"),
            ),
            (
                fwd_r,
                Icon::SkipForward,
                1.0,
                format!("Forward {secs} seconds (right arrow key)"),
            ),
        ] {
            let face = stack![
                container(icon::icon(ic, Color::WHITE, r.w)).center(r.w),
                container(text(secs.clone()).font(BOLD).size(8.5).color(Color::WHITE))
                    .padding(Padding::default().top(1.5))
                    .center(r.w),
            ];
            layers.push(at(
                r,
                tip(
                    button(face)
                        .padding(0)
                        .width(r.w)
                        .height(r.h)
                        .style(style::ctl(disc))
                        .on_press(Msg::Skip(dir)),
                    tip_text,
                    tip_at,
                ),
            ));
        }
        let playing = self.playing();
        let play_icon = if playing { Icon::Pause } else { Icon::Play };
        let play_tip = if playing {
            "Pause (Space)"
        } else {
            "Play (Space)"
        };
        let skin = theme::skin();
        let discs = look.discs;
        let play_style = move |_: &Theme, s: button::Status| {
            let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
            let fill = if row_layout {
                hot.then(|| white(28))
            } else if discs {
                // Apple and Prime: a frosted disc; Disney: just the icon
                Some(white(if hot { 60 } else { 36 }))
            } else if skin == Skin::Disney {
                None
            } else {
                Some(gray(if hot { 70 } else { 48 }))
            };
            button::Style {
                background: fill.map(Background::Color),
                border: iced::border::rounded(100),
                ..button::Style::default()
            }
        };
        let icon_col = if row_layout { gray(235) } else { Color::WHITE };
        // the play triangle sits a little right of center, like the egui player
        let nudge = if playing { 0.0 } else { 3.0 };
        layers.push(at(
            play_r,
            tip(
                button(
                    container(icon::icon(play_icon, icon_col, play_r.w))
                        .padding(Padding::default().left(nudge))
                        .center(play_r.w),
                )
                .padding(0)
                .width(play_r.w)
                .height(play_r.h)
                .style(play_style)
                .on_press(Msg::TogglePlay),
                play_tip,
                tip_at,
            ),
        ));

        if self.has_next() {
            let episode = self.current.is_some_and(|i| self.items[i].episode);
            let label = if episode { "Next Episode" } else { "Next" };
            let (right, ncy) = if row_layout {
                (free_right, row_cy)
            } else {
                (bottom.right() - 22.0, bottom.bottom() - 40.0)
            };
            let b = button(
                row![
                    text(label).size(14.5).color(Color::WHITE),
                    icon::icon(Icon::ChevronRight, Color::WHITE, 16.0),
                ]
                .spacing(10)
                .align_y(Vertical::Center),
            )
            .padding([10, 14])
            .height(40)
            .style(|_, s| button::Style {
                background: Some(Background::Color(gray(
                    if matches!(s, button::Status::Hovered | button::Status::Pressed) {
                        64
                    } else {
                        40
                    },
                ))),
                border: iced::border::rounded(4),
                ..button::Style::default()
            })
            .on_press(Msg::Next);
            layers.push(place(right, ncy, 1.0, 0.5, tip(b, "Next (N)", tip_at)));
        }

        // ---- volume slider, open while the button or the slider is hovered
        if self.vol_shown() {
            let pop = if row_layout {
                R::new(vol_r.center_x() - 20.0, vol_r.y - 6.0 - 130.0, 40.0, 130.0)
            } else {
                R::new(vol_r.center_x() - 20.0, vol_r.bottom() + 6.0, 40.0, 130.0)
            };
            let level = if now.muted { 0.0 } else { now.volume };
            layers.push(at(
                pop,
                mouse_area(
                    container(
                        vertical_slider(0.0..=1.0, level, Msg::Volume)
                            .step(0.01)
                            .style(|_, _| iced::widget::slider::Style {
                                rail: iced::widget::slider::Rail {
                                    backgrounds: (
                                        Background::Color(Color::WHITE),
                                        Background::Color(white(50)),
                                    ),
                                    width: 3.0,
                                    border: iced::border::rounded(2),
                                },
                                handle: iced::widget::slider::Handle {
                                    shape: iced::widget::slider::HandleShape::Circle {
                                        radius: 6.0,
                                    },
                                    background: Background::Color(Color::WHITE),
                                    border_width: 0.0,
                                    border_color: Color::TRANSPARENT,
                                },
                            }),
                    )
                    .padding([14, 0])
                    .center_x(pop.w)
                    .height(pop.h)
                    .style(style::popup),
                )
                .on_enter(Msg::VolHover(true))
                .on_exit(Msg::VolHover(false)),
            ));
        }

        // ---- menus (opening upward from the bottom row)
        if let Some(m) = self.menu {
            let anchor = if m == Menu::Subtitles { sub_r } else { more_r };
            let (w, h, body) = self.menu_body(m, rect);
            let left = (anchor.right() - w).max(rect.x + 8.0);
            let mr = if row_layout {
                R::new(left, anchor.y - 8.0 - h, w, h)
            } else {
                R::new(left, anchor.bottom() + 8.0, w, h)
            };
            // a click anywhere else closes it
            layers.push(
                mouse_area(space().width(Length::Fill).height(Length::Fill))
                    .on_press(Msg::CloseMenu)
                    .into(),
            );
            layers.push(at(
                mr,
                container(body).width(w).height(h).style(style::popup),
            ));
        }
    }

    /// The open menu: its width, height and contents.
    fn menu_body(&self, m: Menu, rect: R) -> (f32, f32, Element<'_, Msg>) {
        let room = rect.h - TOP_H - BOTTOM_H;
        let item = self.current.and_then(|i| self.items.get(i));
        let tracks: Vec<String> = item
            .map(|it| it.subtitles.iter().map(SubtitleTrack::label).collect())
            .unwrap_or_default();
        let sub_on = self.subs.track();
        let none_yet = match (&self.addon_subs, &self.addon_note) {
            (Some((i, _)), _) if self.current == Some(*i) => "Looking for subtitles…".to_owned(),
            (_, Some((i, note))) if self.current == Some(*i) => note.clone(),
            _ => "No subtitles for this video".to_owned(),
        };
        let heading = |t: &str| -> Element<'_, Msg> {
            container(text(t.to_owned()).font(BOLD).size(11.5).color(gray(150)))
                .padding(Padding::default().left(18.0).top(16.0))
                .height(44)
                .into()
        };
        let rule = || -> Element<'_, Msg> {
            container(container(space()).width(Length::Fill).height(1).style(|_| {
                container::Style {
                    background: Some(Background::Color(gray(48))),
                    ..container::Style::default()
                }
            }))
            .padding([0, 18])
            .into()
        };
        match m {
            Menu::Settings => {
                let w = 280.0;
                let h = 44.0 + MENU_ROW + 22.0 + 44.0 + 22.0 + 22.0 + MENU_ROW + 8.0;
                let current = sub_on.and_then(|t| tracks.get(t));
                let label = match (current, tracks.is_empty()) {
                    (_, true) => none_yet,
                    (Some(l), _) => l.clone(),
                    (None, _) => "Off".to_owned(),
                };
                let more = !tracks.is_empty();
                let has_audio = self.player.as_ref().is_some_and(|p| p.meta.has_audio);
                let audio = if has_audio {
                    "Default"
                } else {
                    "No audio track"
                };
                let body = column![
                    heading("SUBTITLES"),
                    menu_row(
                        w,
                        label,
                        None,
                        more,
                        more.then_some(Msg::MenuGo(Menu::Subtitles))
                    ),
                    space().height(9),
                    rule(),
                    space().height(4),
                    heading("AUDIO"),
                    container(text(audio).size(14).color(gray(200)))
                        .padding(Padding::default().left(18.0)),
                    space().height(9),
                    rule(),
                    space().height(12),
                    menu_row(w, "Info".into(), None, true, Some(Msg::MenuGo(Menu::Info))),
                ];
                (w, h, body.into())
            }
            Menu::Info => {
                let w = 440.0;
                let info = match (self.player.as_ref(), item) {
                    (Some(p), Some(it)) => {
                        let title = if it.subtitle.is_empty() || !it.episode {
                            it.title.clone()
                        } else {
                            format!("{} · {}", it.title, it.subtitle)
                        };
                        p.meta
                            .specs
                            .sections(p.clock(), it.file_name.as_deref(), &title)
                    }
                    _ => Vec::new(),
                };
                let rows: usize = info.iter().map(|(_, r)| r.len()).sum();
                let h =
                    (12.0 + info.len() as f32 * 44.0 + rows as f32 * INFO_ROW).min(room.max(160.0));
                let mut col = column![];
                for (name, rows) in info {
                    col = col.push(heading(name));
                    for (label, value) in rows {
                        col = col.push(
                            row![
                                text(label).size(13).color(gray(150)).width(150),
                                text(value).size(13).color(gray(225)).width(Length::Fill),
                            ]
                            .spacing(0)
                            .padding(Padding::default().left(18.0).right(18.0).bottom(6.0)),
                        );
                    }
                }
                col = col.push(space().height(12));
                (w, h, scrollable(col).height(Length::Fill).into())
            }
            Menu::Subtitles => {
                let w = 280.0;
                let h = (44.0
                    + MENU_ROW * (tracks.len() + 1) as f32
                    + 10.0
                    + if sub_on.is_some() { 50.0 } else { 0.0 })
                .min(room.max(140.0));
                if tracks.is_empty() {
                    let body = column![
                        heading("SUBTITLES"),
                        container(text(none_yet).size(14).color(gray(200)))
                            .padding(Padding::default().left(18.0).right(18.0)),
                    ];
                    return (w, h, body.into());
                }
                // where the addons' tracks start in the list
                let first_addon = item.and_then(|it| it.subtitles.iter().position(|t| t.external));
                let note = match &self.subs {
                    SubState::Loading(..) => Some(("Loading…".to_owned(), style::MUTED())),
                    SubState::Failed(_, e) => Some((e.clone(), style::ERROR())),
                    _ => None,
                };
                let mut list = column![menu_row(
                    w,
                    "Off".into(),
                    Some(sub_on.is_none()),
                    false,
                    Some(Msg::SubPick(None)),
                )];
                for (t, label) in tracks.into_iter().enumerate() {
                    if first_addon == Some(t) {
                        list = list.push(
                            container(text("FROM ADDONS").size(11).color(gray(150)))
                                .padding(Padding::default().left(18.0).top(12.0))
                                .height(30),
                        );
                    }
                    let on = sub_on == Some(t);
                    let r = menu_row(w, label, Some(on), false, Some(Msg::SubPick(Some(t))));
                    let r = match &note {
                        Some((n, c)) if on => {
                            let short = if n.chars().count() > 22 {
                                "Couldn't load".to_owned()
                            } else {
                                n.clone()
                            };
                            let c = *c;
                            stack![
                                r,
                                container(text(short).size(12).color(c))
                                    .width(w)
                                    .height(MENU_ROW)
                                    .padding(Padding::default().right(14.0))
                                    .align_x(Horizontal::Right)
                                    .align_y(Vertical::Center),
                            ]
                            .into()
                        }
                        _ => r,
                    };
                    list = list.push(r);
                }
                let mut body = column![heading("SUBTITLES"), scrollable(list).height(Length::Fill)];
                if sub_on.is_some() {
                    let b = |t: &'static str, d: f64| {
                        button(text(t).size(12.5).color(Color::WHITE))
                            .padding([4, 10])
                            .style(|_, s| button::Style {
                                background: Some(Background::Color(white(
                                    if matches!(s, button::Status::Hovered) {
                                        36
                                    } else {
                                        20
                                    },
                                ))),
                                border: iced::border::rounded(5),
                                ..button::Style::default()
                            })
                            .on_press(Msg::Delay(d))
                    };
                    let mut buttons = row![].spacing(6);
                    if self.sub_delay != 0.0 {
                        buttons = buttons.push(b("Reset", 0.0));
                    }
                    buttons = buttons.push(tip(b("Earlier", -0.1), "Z", Above));
                    buttons = buttons.push(tip(b("Later", 0.1), "X", Above));
                    body = body.push(space().height(6)).push(rule()).push(
                        row![
                            text(delay_text(self.sub_delay)).size(13).color(gray(200)),
                            space::horizontal(),
                            buttons,
                        ]
                        .align_y(Vertical::Center)
                        .padding(
                            Padding::default()
                                .left(18.0)
                                .right(14.0)
                                .top(8.0)
                                .bottom(8.0),
                        ),
                    );
                }
                (w, h, body.into())
            }
        }
    }
}

/// What the view needs from the app's settings and window.
#[derive(Clone, Copy)]
pub struct View {
    pub volume: f32,
    pub muted: bool,
    pub skip: f64,
    pub fullscreen: bool,
}

/// A rectangle in window coordinates.
#[derive(Clone, Copy, Debug)]
struct R {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl R {
    fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    /// A `s`-square centered on (cx, cy).
    fn centered(cx: f32, cy: f32, s: f32) -> Self {
        Self::new(cx - s / 2.0, cy - s / 2.0, s, s)
    }
    fn right(&self) -> f32 {
        self.x + self.w
    }
    fn bottom(&self) -> f32 {
        self.y + self.h
    }
    fn center_x(&self) -> f32 {
        self.x + self.w / 2.0
    }
}

/// The video's place in `r`: as big as fits, its shape kept.
fn fit(r: R, w: f32, h: f32) -> R {
    let scale = (r.w / w.max(1.0)).min(r.h / h.max(1.0));
    let (dw, dh) = (w * scale, h * scale);
    R::new(r.x + (r.w - dw) / 2.0, r.y + (r.h - dh) / 2.0, dw, dh)
}

/// `el` with its top left corner at `r`'s.
fn at<'a>(r: R, el: impl Into<Element<'a, Msg>>) -> Element<'a, Msg> {
    pin(el).x(r.x).y(r.y).into()
}

/// `el` placed by an anchor: `ax`/`ay` 0 puts its left/top at (x, y),
/// 0.5 its center, 1 its right/bottom.
fn place<'a>(
    x: f32,
    y: f32,
    ax: f32,
    ay: f32,
    el: impl Into<Element<'a, Msg>>,
) -> Element<'a, Msg> {
    // a generous box around the anchor, the content aligned inside it
    const W: f32 = 1200.0;
    const H: f32 = 120.0;
    let align_x = match ax {
        a if a < 0.25 => Horizontal::Left,
        a if a > 0.75 => Horizontal::Right,
        _ => Horizontal::Center,
    };
    let align_y = match ay {
        a if a < 0.25 => Vertical::Top,
        a if a > 0.75 => Vertical::Bottom,
        _ => Vertical::Center,
    };
    let boxed = container(el)
        .width(W)
        .height(H)
        .align_x(align_x)
        .align_y(align_y);
    pin(boxed).x(x - W * ax).y(y - H * ay).into()
}

fn centered<'a>(el: impl Into<Element<'a, Msg>>) -> Element<'a, Msg> {
    container(el).center(Length::Fill).into()
}

use tooltip::Position::{Bottom as Below, Top as Above};

fn tip<'a>(
    el: impl Into<Element<'a, Msg>>,
    label: impl Into<String>,
    pos: tooltip::Position,
) -> Element<'a, Msg> {
    tooltip(
        el,
        container(text(label.into()).size(12.5))
            .padding([4, 8])
            .style(style::tip),
        pos,
    )
    .gap(6)
    .into()
}

/// An icon button over the video; lights up on hover and names itself.
/// `disc`: always on a frosted circle (the Apple and Prime styles).
fn ctl<'a>(
    r: R,
    ic: Icon,
    label: &str,
    disc: bool,
    msg: Msg,
    pos: tooltip::Position,
) -> Element<'a, Msg> {
    tip(
        button(icon::icon(ic, Color::WHITE, r.w))
            .padding(0)
            .width(r.w)
            .height(r.h)
            .style(style::ctl(disc))
            .on_press(msg),
        label.to_owned(),
        pos,
    )
}

/// A clickable line in a player menu: a check when `checked` is Some(true),
/// a chevron when it opens more.
fn menu_row<'a>(
    w: f32,
    label: String,
    checked: Option<bool>,
    more: bool,
    msg: Option<Msg>,
) -> Element<'a, Msg> {
    let on = checked == Some(true);
    let mut line = row![].align_y(Vertical::Center).height(MENU_ROW);
    if checked.is_some() {
        let check: Element<Msg> = if on {
            canvas(Check).width(22).height(MENU_ROW).into()
        } else {
            Space::new().width(22).into()
        };
        line = line.push(
            container(check)
                .padding(Padding::default().left(12.0))
                .width(40),
        );
    } else {
        line = line.push(Space::new().width(18));
    }
    line = line.push(
        text(label)
            .size(14)
            .color(if on { Color::WHITE } else { gray(200) })
            .wrapping(text::Wrapping::None),
    );
    line = line.push(space::horizontal());
    if more {
        line = line.push(icon::icon(Icon::ChevronRight, gray(170), 28.0));
        line = line.push(Space::new().width(6));
    }
    let b = button(line)
        .padding(0)
        .width(w)
        .height(MENU_ROW)
        .style(style::bare(white(18), 5.0));
    match msg {
        Some(m) => b.on_press(m).into(),
        None => b.into(),
    }
}

/// The check mark beside the chosen menu row.
struct Check;

impl<M> cv::Program<M> for Check {
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
        let c = Point::new(10.0, bounds.height / 2.0);
        let p = cv::Path::new(|b| {
            b.move_to(Point::new(c.x - 5.0, c.y));
            b.line_to(Point::new(c.x - 1.5, c.y + 3.5));
            b.line_to(Point::new(c.x + 5.5, c.y - 4.0));
        });
        f.stroke(
            &p,
            cv::Stroke::default()
                .with_width(2.0)
                .with_color(Color::WHITE),
        );
        vec![f.into_geometry()]
    }
}

/// Subtitle text centered over the video, its last line ending at `base`,
/// white with a dark outline so it reads on any picture.
struct Subtitle {
    text: String,
    dest: R,
    base: f32,
}

impl<M> cv::Program<M> for Subtitle {
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
        let size = (self.dest.h * 0.048).clamp(16.0, 64.0);
        let o = (size / 14.0).max(1.5);
        let line = |dx: f32, dy: f32, color: Color| Text {
            content: self.text.clone(),
            position: Point::new(self.dest.center_x() + dx, self.base + dy),
            max_width: self.dest.w * 0.86,
            color,
            size: size.into(),
            font: SANS,
            align_x: iced::widget::text::Alignment::Center,
            align_y: Vertical::Bottom,
            ..Text::default()
        };
        for (dx, dy) in [
            (-o, 0.0),
            (o, 0.0),
            (0.0, -o),
            (0.0, o),
            (-o, -o),
            (o, -o),
            (-o, o),
            (o, o),
        ] {
            f.fill_text(line(dx, dy, black(230)));
        }
        f.fill_text(line(0.0, 0.0, Color::WHITE));
        vec![f.into_geometry()]
    }
}

/// "In sync", "0.8 s earlier", "1.5 s later".
fn delay_text(d: f64) -> String {
    if d.abs() < 0.05 {
        "Timing: in sync".into()
    } else if d < 0.0 {
        format!("{:.1} s earlier", -d)
    } else {
        format!("{d:.1} s later")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_files_play_from_the_first() {
        // one FileDropped event per file, as Iced sends them
        let mut w = Watch::new(vec![PlayItem::local("a.mkv".into())]);
        w.add_dropped("b.mkv".into());
        w.add_dropped("c.mkv".into());
        assert_eq!(w.queued, Some(0));
        assert!(w.show_list);

        // once the first has started, a new drop plays the first file it added
        let mut w = Watch::new(Vec::new());
        w.add_dropped("a.mkv".into());
        w.add_dropped("b.mkv".into());
        assert_eq!(w.queued, Some(0));
        w.queued = None; // opened
        w.add_dropped("c.mkv".into());
        w.add_dropped("d.mkv".into());
        assert_eq!(w.queued, Some(2));
    }

    #[test]
    fn picked_files_replace_a_pending_start() {
        let mut w = Watch::new(vec![PlayItem::local("a.mkv".into())]);
        assert_eq!(w.queued, Some(0));
        w.add_files(vec!["b.mkv".into(), "c.mkv".into()]);
        assert_eq!(w.queued, Some(1));
        assert!(w.show_list);
    }
}
