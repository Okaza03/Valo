//! Full-window playback of a queue (an episode run, a movie, or local files).
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Key, Rect, Response, Sense, Vec2,
};

use super::chrome::{self, Icon};
use super::theme::{self, PlayerLayout, Skin, Times, TitleAt};
use super::{gradient, mono, ERROR, MUTED, TEXT};
use crate::addons::{self, Lookup};
use crate::api::SubtitleTrack;
use crate::audio::AudioOut;
use crate::config::{Config, Progress};
use crate::inhibit::Inhibit;
use crate::player::{Player, Source};
use crate::subs::{self, Subs};
use crate::util::{fmt_time, Task};

const TOP_H: f32 = 64.0;
const BOTTOM_H: f32 = 120.0;

/// An icon button over the video; lights up on hover and names itself.
/// `disc`: always on a frosted circle (the Apple and Prime styles).
fn ctl_button(ui: &mut egui::Ui, r: Rect, icon: Icon, tip: &str, disc: bool) -> Response {
    let id = ui
        .id()
        .with(("ctl", r.center().x as i32, r.center().y as i32));
    let resp = ui.interact(r, id, Sense::click());
    if resp.hovered() || disc {
        let a = match (disc, resp.hovered()) {
            (true, true) => 60,
            (true, false) => 36,
            _ => 28,
        };
        ui.painter()
            .circle_filled(r.center(), r.width() / 2.0, Color32::from_white_alpha(a));
    }
    chrome::draw_icon(ui.painter(), r.center(), icon, Color32::WHITE);
    resp.on_hover_text(tip)
        .on_hover_cursor(CursorIcon::PointingHand)
}

#[derive(Clone, Copy, PartialEq)]
enum Menu {
    Settings,
    Subtitles,
}

pub struct PlayItem {
    pub title: String,
    /// Under the title: "2020 · 2h 0m", "S01E01 · Pilot".
    pub subtitle: String,
    /// An episode of a series (the next button says "Next Episode").
    pub episode: bool,
    pub source: Source,
    /// Progress key; None for local files.
    pub key: Option<String>,
    /// Resume position in seconds.
    pub start: f64,
    /// Subtitle files on the server.
    pub subtitles: Vec<SubtitleTrack>,
    /// Ask the admin's subtitle addons for more; None once they've answered.
    pub addons: Option<Lookup>,
}

impl PlayItem {
    pub fn local(path: PathBuf) -> Self {
        let source = Source::File(path);
        Self {
            title: source.name(),
            subtitle: String::new(),
            episode: false,
            source,
            key: None,
            start: 0.0,
            subtitles: Vec::new(),
            addons: None,
        }
    }
}

/// The addons' subtitles for an item, or why they couldn't be asked.
type AddonSubs = Result<Vec<SubtitleTrack>, String>;

/// The subtitle track that's on, and its file.
enum SubState {
    Off,
    Loading(usize, Task<Result<Subs>>),
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

/// A row in the player's menus.
const MENU_ROW: f32 = 34.0;

/// How long the volume slider stays up after ↑/↓.
const VOL_SHOW: Duration = Duration::from_secs(3);

pub struct Watch {
    items: Vec<PlayItem>,
    current: Option<usize>,
    player: Option<Player>,
    opening: Option<(usize, Task<Result<Player>>)>,
    queued: Option<usize>,
    tex: Option<egui::TextureHandle>,
    seek_drag: Option<f64>,
    status: String,
    show_list: bool,
    last_tick: Instant,
    last_motion: Instant,
    last_save: Instant,
    menu: Option<Menu>,
    /// Last time the pointer was over the volume button or its slider.
    vol_hover: Option<Instant>,
    /// Last time ↑/↓ changed the volume: the slider shows for a while after.
    vol_key: Option<Instant>,
    /// Keeps the screen from blanking or sleeping while playing.
    inhibit: Inhibit,
    subs: SubState,
    /// The addons' subtitles for an item, on their way.
    addon_subs: Option<(usize, Task<AddonSubs>)>,
    /// Why the addons couldn't be asked about an item's subtitles.
    addon_note: Option<(usize, String)>,
    /// Subtitles were picked or turned off by hand for this video.
    subs_chosen: bool,
    /// Seconds the subtitles are shifted: negative shows them earlier. For
    /// files made for a slightly different cut (an addon's, usually).
    sub_delay: f64,
    /// When the delay last changed, to show it for a moment.
    delay_changed: Option<Instant>,
}

impl Watch {
    pub fn new(items: Vec<PlayItem>) -> Self {
        Self {
            show_list: items.iter().any(|i| i.key.is_none()),
            queued: (!items.is_empty()).then_some(0),
            items,
            current: None,
            player: None,
            opening: None,
            tex: None,
            seek_drag: None,
            status: String::new(),
            last_tick: Instant::now(),
            last_motion: Instant::now(),
            last_save: Instant::now(),
            menu: None,
            vol_hover: None,
            vol_key: None,
            inhibit: Inhibit::new(),
            subs: SubState::Off,
            addon_subs: None,
            addon_note: None,
            sub_delay: 0.0,
            delay_changed: None,
            subs_chosen: false,
        }
    }

    pub fn add_files(&mut self, files: Vec<PathBuf>) {
        let first = self.items.len();
        self.items.extend(files.into_iter().map(PlayItem::local));
        if self.items.len() > first {
            self.queued = Some(first);
            self.show_list = true;
        }
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
        if let Some(key) = &self.items[i].key {
            let pos = if p.finished() {
                p.meta.duration
            } else {
                p.clock()
            };
            if pos > 1.0 {
                progress.set(key, pos, p.meta.duration);
            }
        }
        self.last_save = Instant::now();
    }

    /// Turn on track `t` of the current item (None: off) and fetch its file.
    fn set_subs(&mut self, ctx: &egui::Context, t: Option<usize>) {
        self.sub_delay = 0.0; // another file, its own timing
        let item = self.current.and_then(|i| self.items.get(i));
        self.subs = match (item, t) {
            (Some(item), Some(t)) if t < item.subtitles.len() => {
                let track = item.subtitles[t].clone();
                let auth = match &item.source {
                    Source::Url { auth, .. } if !track.external => auth.clone(),
                    _ => None, // never hand our token to an addon
                };
                SubState::Loading(
                    t,
                    Task::spawn(ctx, move || subs::load(&track, auth.as_deref())),
                )
            }
            _ => SubState::Off,
        };
    }

    /// A pick from the menu (or C), remembered for the next video.
    fn choose_subs(&mut self, ctx: &egui::Context, cfg: &mut Config, t: Option<usize>) {
        let item = self.current.and_then(|i| self.items.get(i));
        let track = item.and_then(|it| t.and_then(|t| it.subtitles.get(t)));
        cfg.subtitle_language = track.map(|tr| tr.language.clone());
        if let Some(tr) = track {
            cfg.subtitle_sdh = tr.sdh;
        }
        cfg.save();
        self.subs_chosen = true;
        self.set_subs(ctx, t);
    }

    fn open_index(
        &mut self,
        ctx: &egui::Context,
        i: usize,
        audio: &AudioOut,
        progress: &mut Progress,
        cfg: &Config,
    ) {
        if self.opening.is_some() {
            self.queued = Some(i); // one open at a time; picked up when it lands
            return;
        }
        self.save_progress(progress);
        self.player = None; // drop (and join) the old decoder first
        self.tex = None;
        self.current = Some(i);
        self.status.clear();
        let Some(item) = self.items.get(i) else {
            return;
        };
        let t = subs::pick(
            &item.subtitles,
            cfg.subtitle_language.as_deref(),
            cfg.subtitle_sdh,
        );
        self.set_subs(ctx, t);
        self.subs_chosen = false;
        self.addon_subs = None;
        let Some(item) = self.items.get(i) else {
            return;
        };
        if let Some(l) = item.addons.clone() {
            self.addon_subs = Some((i, Task::spawn(ctx, move || addons::subtitles(&l))));
        }
        let (src, shared) = (item.source.clone(), audio.shared.clone());
        self.opening = Some((i, Task::spawn(ctx, move || Player::open(&src, shared))));
        let title = if item.subtitle.is_empty() {
            item.title.clone()
        } else {
            format!("{} — {}", item.title, item.subtitle)
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("{title} — Valo")));
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

    fn toggle_fullscreen(ctx: &egui::Context) {
        let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fs));
    }

    /// Draws the player; returns true when the user wants to leave it.
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        audio: &AudioOut,
        cfg: &mut Config,
        muted: &mut bool,
        progress: &mut Progress,
    ) -> bool {
        let (skip, autoplay) = (f64::from(cfg.skip_seconds.max(1)), cfg.autoplay_next);
        let mut vol = cfg.volume;
        let volume = &mut vol;
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;

        // a finished open
        if let Some((i, mut task)) = self.opening.take() {
            match task.poll() {
                None => self.opening = Some((i, task)),
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
        if let SubState::Loading(t, task) = &mut self.subs {
            if let Some(r) = task.poll() {
                let t = *t;
                self.subs = match r {
                    Ok(s) => SubState::Ready(t, s),
                    Err(e) => SubState::Failed(t, format!("{e:#}")),
                };
            }
        }
        if let Some((i, task)) = self.addon_subs.as_mut() {
            if let Some(found) = task.poll() {
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
                        cfg.subtitle_language.as_deref(),
                        cfg.subtitle_sdh,
                    );
                    if t.is_some() {
                        self.set_subs(ctx, t);
                    }
                }
            }
        }
        if self.opening.is_none() {
            if let Some(q) = self.queued.take() {
                self.open_index(ctx, q, audio, progress, cfg);
            }
        }

        let (mut close, mut do_open, mut do_prev, mut do_next, mut do_toggle) =
            (false, false, false, false, false);
        let mut seek_to: Option<f64> = None;
        let mut skip_by: Option<f64> = None;
        let mut pick: Option<usize> = None;
        // a subtitle track picked (Some(None): turned off)
        let mut sub_pick: Option<Option<usize>> = None;
        // subtitles shifted by this many seconds (0: back in sync)
        let mut delay_by: Option<f64> = None;
        let tracks: Vec<String> = self
            .current
            .and_then(|i| self.items.get(i))
            .map(|it| it.subtitles.iter().map(SubtitleTrack::label).collect())
            .unwrap_or_default();
        let sub_on = self.subs.track();
        // where the addons' tracks start in the list
        let first_addon = self
            .current
            .and_then(|i| self.items.get(i))
            .and_then(|it| it.subtitles.iter().position(|t| t.external));
        let none_yet = match (&self.addon_subs, &self.addon_note) {
            (Some((i, _)), _) if self.current == Some(*i) => "Looking for subtitles…".to_owned(),
            (_, Some((i, note))) if self.current == Some(*i) => note.clone(),
            _ => "No subtitles for this video".to_owned(),
        };

        // keys. Only read them here: anything that talks back to egui (like
        // leaving fullscreen) waits until the input lock is released, or the app freezes.
        let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        let (mut esc, mut back, mut vol_key) = (false, false, false);
        ctx.input(|i| {
            let shift = i.modifiers.shift;
            let step = if shift { skip * 6.0 } else { skip };
            if i.modifiers.command && i.key_pressed(Key::O) {
                do_open = true;
            }
            do_toggle |= i.key_pressed(Key::Space);
            if i.key_pressed(Key::ArrowRight) {
                skip_by = Some(step);
            }
            if i.key_pressed(Key::ArrowLeft) {
                skip_by = Some(-step);
            }
            if i.key_pressed(Key::ArrowUp) {
                *volume = (*volume + 0.05).min(1.0);
                vol_key = true;
            }
            if i.key_pressed(Key::ArrowDown) {
                *volume = (*volume - 0.05).max(0.0);
                vol_key = true;
            }
            if i.key_pressed(Key::M) {
                *muted = !*muted;
            }
            // C: subtitles on (the language used last, else the first) or off
            if i.key_pressed(Key::C) && !tracks.is_empty() {
                sub_pick = Some(match sub_on {
                    Some(_) => None,
                    None => Some(
                        self.current
                            .and_then(|c| {
                                let it = &self.items[c];
                                let lang = cfg
                                    .subtitle_language
                                    .as_deref()
                                    .or_else(|| it.subtitles.first().map(|t| t.language.as_str()));
                                subs::pick(&it.subtitles, lang, cfg.subtitle_sdh)
                            })
                            .unwrap_or(0),
                    ),
                });
            }
            // Z / X: subtitles earlier / later (Shift: a whole second)
            let step = if shift { 1.0 } else { 0.1 };
            if i.key_pressed(Key::Z) && sub_on.is_some() {
                delay_by = Some(-step);
            }
            if i.key_pressed(Key::X) && sub_on.is_some() {
                delay_by = Some(step);
            }
            do_next |= i.key_pressed(Key::N);
            do_prev |= i.key_pressed(Key::P);
            esc = i.key_pressed(Key::Escape);
            back = i.key_pressed(Key::Backspace);
        });
        if vol_key {
            self.vol_key = Some(Instant::now());
            ctx.request_repaint_after(VOL_SHOW); // to close it again, paused or not
        }
        // The player has no text fields, so nothing keeps keyboard focus: a clicked
        // button or slider would otherwise also react to Space and the arrow keys
        // (Space would press the play button too, undoing the pause).
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        // Esc: close a menu, else leave fullscreen. Backspace: leave fullscreen, else go back.
        if esc && self.menu.is_some() {
            self.menu = None;
        } else if (esc || back) && fullscreen {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        } else if back {
            close = true;
        }
        if let Some(d) = skip_by.take() {
            self.skip(d);
        }
        if ctx.input(|i| !i.modifiers.command && (i.key_pressed(Key::F) || i.key_pressed(Key::F11)))
        {
            Self::toggle_fullscreen(ctx);
        }
        if ctx.input(|i| i.key_pressed(Key::L)) {
            self.show_list = !self.show_list;
        }

        audio.shared.set_volume(*volume);
        audio.shared.set_muted(*muted);

        // advance playback, upload the newest due frame
        let mut finished = false;
        if let Some(p) = self.player.as_mut() {
            p.tick(dt);
            if let Some(f) = p.take_frame() {
                let img = egui::ColorImage::from_rgba_unmultiplied([f.w, f.h], &f.rgba);
                match self.tex.as_mut() {
                    Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                    None => {
                        self.tex =
                            Some(ctx.load_texture("video", img, egui::TextureOptions::LINEAR))
                    }
                }
            }
            finished = p.finished() && !p.paused;
        }
        if finished {
            self.save_progress(progress);
            match self.current {
                Some(i) if autoplay && i + 1 < self.items.len() => {
                    self.open_index(ctx, i + 1, audio, progress, cfg)
                }
                _ => {
                    if let Some(p) = self.player.as_mut() {
                        p.set_paused(true);
                    }
                }
            }
        }
        if self.last_save.elapsed() > Duration::from_secs(5) {
            self.save_progress(progress);
        }

        // auto-hide controls while playing
        if ctx.input(|i| {
            i.pointer.delta() != Vec2::ZERO || i.pointer.any_pressed() || !i.keys_down.is_empty()
        }) {
            self.last_motion = now;
        }
        let playing = self.player.as_ref().is_some_and(|p| !p.paused);
        self.inhibit.set(playing);
        let vol_keyed = self.vol_key.is_some_and(|t| t.elapsed() < VOL_SHOW);
        let hide_ui = playing
            && !vol_keyed
            && self.last_motion.elapsed()
                > Duration::from_millis(if fullscreen { 2500 } else { 3500 });

        if self.show_list && self.items.len() > 1 && !hide_ui {
            egui::SidePanel::right("queue")
                .default_width(260.0)
                .show(ctx, |ui| {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(mono("Up next", 12.0));
                        if ui
                            .small_button("Add…")
                            .on_hover_text("Add files (Ctrl+O)")
                            .clicked()
                        {
                            do_open = true;
                        }
                    });
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (i, it) in self.items.iter().enumerate() {
                            let label = if it.subtitle.is_empty() {
                                it.title.clone()
                            } else {
                                it.subtitle.clone()
                            };
                            if ui
                                .selectable_label(self.current == Some(i), label)
                                .clicked()
                            {
                                pick = Some(i);
                            }
                        }
                    });
                });
        }

        let (pos, dur) = self
            .player
            .as_ref()
            .map_or((0.0, 0.0), |p| (p.clock(), p.meta.duration));
        let has_audio = self.player.as_ref().is_some_and(|p| p.meta.has_audio);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::BLACK))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let resp = ui.allocate_rect(rect, Sense::click());
                if let Some(t) = &self.tex {
                    let sz = t.size_vec2();
                    let scale = (rect.width() / sz.x).min(rect.height() / sz.y);
                    let dest = Rect::from_center_size(rect.center(), sz * scale);
                    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                    ui.painter().image(t.id(), dest, uv, Color32::WHITE);
                    if let SubState::Ready(_, subs) = &self.subs {
                        if let Some(text) = subs.at(pos - self.sub_delay) {
                            // above the controls while they're up
                            let mut base = dest.bottom().min(rect.bottom()) - dest.height() * 0.06;
                            if !hide_ui {
                                base = base.min(rect.bottom() - BOTTOM_H - 4.0);
                            }
                            draw_subtitle(ui.painter(), dest, base, &text);
                        }
                    }
                    // the new timing, for a moment after Z / X
                    if self
                        .delay_changed
                        .is_some_and(|t| t.elapsed() < Duration::from_secs(2))
                    {
                        let p = ui.painter();
                        let text = format!(
                            "Subtitles: {}",
                            delay_text(self.sub_delay).replace("Timing: ", "")
                        );
                        let g = p.layout_no_wrap(text, FontId::proportional(16.0), Color32::WHITE);
                        let at = pos2(rect.center().x, rect.top() + TOP_H + 24.0);
                        let bg = Rect::from_center_size(at, g.size() + vec2(28.0, 14.0));
                        p.rect_filled(bg, 8.0, Color32::from_black_alpha(170));
                        p.galley(bg.center() - g.size() / 2.0, g, Color32::WHITE);
                    }
                } else if !self.status.is_empty() {
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        &self.status,
                        FontId::proportional(18.0),
                        ERROR(),
                    );
                } else if self.opening.is_some() || self.player.is_some() {
                    let c = rect.center();
                    let sp = Rect::from_center_size(c - vec2(0.0, 20.0), vec2(40.0, 40.0));
                    egui::Spinner::new()
                        .size(40.0)
                        .color(TEXT())
                        .paint_at(ui, sp);
                    let name = self
                        .current
                        .map(|i| self.items[i].title.clone())
                        .unwrap_or_default();
                    ui.painter().text(
                        c + vec2(0.0, 24.0),
                        Align2::CENTER_TOP,
                        name,
                        FontId::proportional(16.0),
                        MUTED(),
                    );
                } else {
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        "Drop a video here or press Ctrl+O",
                        FontId::proportional(18.0),
                        Color32::GRAY,
                    );
                }

                let top = Rect::from_min_size(rect.min, vec2(rect.width(), TOP_H));
                let bottom =
                    Rect::from_min_max(pos2(rect.left(), rect.bottom() - BOTTOM_H), rect.max);
                let mut on_controls = false;
                if !hide_ui {
                    let p = ui.painter().clone();
                    let dark = Color32::from_black_alpha(235);
                    let fade = Color32::from_black_alpha(0);
                    let shade_top = Rect::from_min_size(top.min, vec2(top.width(), TOP_H + 30.0));
                    gradient(&p, shade_top, [dark, dark, fade, fade]);
                    let shade_bottom =
                        Rect::from_min_max(pos2(bottom.left(), bottom.top() - 40.0), bottom.max);
                    gradient(&p, shade_bottom, [fade, fade, dark, dark]);
                    on_controls = ui.rect_contains_pointer(top) || ui.rect_contains_pointer(bottom);

                    // laid out like the chosen style's player
                    let look = &theme::spec().player;
                    let row = look.layout == PlayerLayout::Row;
                    let bold = |s: f32| FontId::new(s, egui::FontFamily::Name("bold".into()));

                    // ---- top bar
                    chrome::drag_area(ui, top);
                    let cy = top.top() + 30.0;
                    let close_r =
                        Rect::from_center_size(pos2(top.left() + 28.0, cy), vec2(36.0, 36.0));
                    let (close_icon, close_tip) = if look.back_arrow {
                        (Icon::ChevronLeft, "Back")
                    } else {
                        (Icon::Close, "Close")
                    };
                    close |= ctl_button(ui, close_r, close_icon, close_tip, look.discs).clicked();
                    let item = self.current.and_then(|i| self.items.get(i));
                    if let Some(it) = item {
                        let (x, align) = match look.title {
                            TitleAt::TopLeft => (top.left() + 58.0, 0.0),
                            TitleAt::TopCenter => (top.center().x, 0.5),
                            TitleAt::Bottom => (0.0, -1.0),
                        };
                        if align >= 0.0 {
                            let a = |v| {
                                if align > 0.0 {
                                    Align2([egui::Align::Center, v])
                                } else {
                                    Align2([egui::Align::Min, v])
                                }
                            };
                            p.text(
                                pos2(x, cy - 2.0),
                                a(egui::Align::Max),
                                &it.title,
                                bold(15.0),
                                Color32::WHITE,
                            );
                            if !it.subtitle.is_empty() {
                                p.text(
                                    pos2(x, cy + 2.0),
                                    a(egui::Align::Min),
                                    &it.subtitle,
                                    FontId::proportional(12.5),
                                    Color32::from_gray(160),
                                );
                            }
                        }
                    }

                    // where the rows go: the seek bar, and (Row) the buttons under it
                    let row_cy = bottom.bottom() - 32.0;
                    let by = if row {
                        bottom.bottom() - 68.0
                    } else {
                        bottom.bottom() - 86.0
                    };

                    // ---- the other buttons: top right, or the right of the bottom row
                    let (slot_y, step) = if row { (row_cy, 44.0) } else { (cy, 40.0) };
                    let mut x = top.right() - 28.0;
                    let mut next_slot = || {
                        let r = Rect::from_center_size(pos2(x, slot_y), vec2(36.0, 36.0));
                        x -= step;
                        r
                    };
                    let (more_r, fs_r, vol_r, sub_r);
                    if row {
                        fs_r = next_slot();
                        more_r = next_slot();
                        sub_r = next_slot();
                        vol_r = Rect::from_center_size(
                            pos2(bottom.left() + 168.0, row_cy),
                            vec2(36.0, 36.0),
                        );
                    } else {
                        more_r = next_slot();
                        fs_r = next_slot();
                        vol_r = next_slot();
                        sub_r = next_slot();
                    }
                    let list_r = (self.items.len() > 1).then(&mut next_slot);
                    let free_right = x + step - 22.0;
                    let disc = look.discs && !row;
                    if ctl_button(ui, more_r, Icon::More, "Settings", disc).clicked() {
                        self.menu = (self.menu != Some(Menu::Settings)).then_some(Menu::Settings);
                    }
                    let fs_icon = if fullscreen {
                        Icon::FullscreenExit
                    } else {
                        Icon::Fullscreen
                    };
                    let fs_tip = if fullscreen {
                        "Exit fullscreen (F)"
                    } else {
                        "Fullscreen (F)"
                    };
                    if ctl_button(ui, fs_r, fs_icon, fs_tip, disc).clicked() {
                        Self::toggle_fullscreen(ctx);
                    }
                    let vol_icon = if *muted || *volume <= 0.0 {
                        Icon::VolumeMuted
                    } else {
                        Icon::Volume
                    };
                    let vb = ctl_button(ui, vol_r, vol_icon, "Volume (M to mute)", disc);
                    if vb.clicked() {
                        *muted = !*muted;
                    }
                    if vb.hovered() {
                        self.vol_hover = Some(now);
                    }
                    if ctl_button(ui, sub_r, Icon::Subtitles, "Subtitles", disc).clicked() {
                        self.menu = (self.menu != Some(Menu::Subtitles)).then_some(Menu::Subtitles);
                    }
                    if let Some(lr) = list_r {
                        if ctl_button(ui, lr, Icon::List, "Queue (L)", disc).clicked() {
                            self.show_list = !self.show_list;
                        }
                    }

                    // ---- volume slider, open while the button or the slider is hovered
                    let pop = if row {
                        Rect::from_min_size(
                            pos2(vol_r.center().x - 20.0, vol_r.top() - 6.0 - 130.0),
                            vec2(40.0, 130.0),
                        )
                    } else {
                        Rect::from_min_size(
                            pos2(vol_r.center().x - 20.0, vol_r.bottom() + 6.0),
                            vec2(40.0, 130.0),
                        )
                    };
                    if ui.rect_contains_pointer(pop.expand(4.0)) && self.vol_hover.is_some() {
                        self.vol_hover = Some(now);
                    }
                    // open while hovered, and for a while after ↑/↓
                    if vol_keyed
                        || self
                            .vol_hover
                            .is_some_and(|t| now.duration_since(t) < Duration::from_millis(350))
                    {
                        on_controls = true;
                        let lp = ui.ctx().layer_painter(egui::LayerId::new(
                            egui::Order::Foreground,
                            egui::Id::new("volume_pop"),
                        ));
                        lp.rect_filled(pop, 8.0, Color32::from_rgba_unmultiplied(18, 18, 20, 235));
                        lp.rect_stroke(
                            pop,
                            8.0,
                            egui::Stroke::new(1.0f32, Color32::from_gray(50)),
                            egui::StrokeKind::Inside,
                        );
                        let track =
                            Rect::from_center_size(pop.center(), vec2(3.0, pop.height() - 28.0));
                        lp.rect_filled(track, 2.0, Color32::from_white_alpha(50));
                        let level = if *muted { 0.0 } else { *volume };
                        let filled = Rect::from_min_max(
                            pos2(track.left(), track.bottom() - track.height() * level),
                            track.max,
                        );
                        lp.rect_filled(filled, 2.0, Color32::WHITE);
                        let s =
                            ui.interact(pop, ui.id().with("volume_pop"), Sense::click_and_drag());
                        if s.is_pointer_button_down_on() {
                            if let Some(pp) = s.interact_pointer_pos() {
                                *volume =
                                    ((track.bottom() - pp.y) / track.height()).clamp(0.0, 1.0);
                                *muted = false;
                            }
                        }
                        ctx.request_repaint();
                    }

                    // ---- menus (opening upward from the bottom row)
                    if let Some(m) = self.menu {
                        let anchor = if m == Menu::Settings { more_r } else { sub_r };
                        let w = 280.0;
                        let h = if m == Menu::Settings {
                            44.0 + MENU_ROW + 22.0 + 44.0 + 14.0
                        } else {
                            let room = rect.height() - TOP_H - BOTTOM_H;
                            (44.0 + MENU_ROW * (tracks.len() + 1) as f32 + 10.0)
                                .min(room.max(140.0))
                        };
                        let left = (anchor.right() - w).max(rect.left() + 8.0);
                        let mr = if row {
                            Rect::from_min_size(pos2(left, anchor.top() - 8.0 - h), vec2(w, h))
                        } else {
                            Rect::from_min_size(pos2(left, anchor.bottom() + 8.0), vec2(w, h))
                        };
                        let area = egui::Area::new(egui::Id::new("player_menu"))
                            .order(egui::Order::Foreground)
                            .fixed_pos(mr.min)
                            .show(ctx, |ui| {
                                ui.set_min_size(mr.size());
                                ui.set_max_size(mr.size());
                                let lp = ui.painter();
                                lp.rect_filled(
                                    mr,
                                    8.0,
                                    Color32::from_rgba_unmultiplied(28, 28, 31, 245),
                                );
                                lp.rect_stroke(
                                    mr,
                                    8.0,
                                    egui::Stroke::new(1.0f32, Color32::from_gray(48)),
                                    egui::StrokeKind::Inside,
                                );
                                let heading = |ui: &mut egui::Ui, t: &str| {
                                    let (r, _) =
                                        ui.allocate_exact_size(vec2(w, 44.0), Sense::hover());
                                    ui.painter().text(
                                        pos2(r.left() + 18.0, r.top() + 16.0),
                                        Align2::LEFT_TOP,
                                        t,
                                        bold(11.5),
                                        Color32::from_gray(150),
                                    );
                                };
                                ui.spacing_mut().item_spacing = Vec2::ZERO;
                                let current = sub_on.and_then(|t| tracks.get(t));
                                if m == Menu::Settings {
                                    heading(ui, "SUBTITLES");
                                    let label = match (current, tracks.is_empty()) {
                                        (_, true) => none_yet.as_str(),
                                        (Some(l), _) => l.as_str(),
                                        (None, _) => "Off",
                                    };
                                    let more = !tracks.is_empty();
                                    if menu_row(ui, w, label, None, more).clicked() && more {
                                        self.menu = Some(Menu::Subtitles);
                                    }
                                    ui.add_space(4.0);
                                    let y = ui.cursor().top() + 9.0;
                                    ui.painter().hline(
                                        mr.x_range().shrink(18.0),
                                        y,
                                        egui::Stroke::new(1.0f32, Color32::from_gray(48)),
                                    );
                                    ui.add_space(18.0);
                                    heading(ui, "AUDIO");
                                    let a = if has_audio {
                                        "Default"
                                    } else {
                                        "No audio track"
                                    };
                                    ui.painter().text(
                                        pos2(mr.left() + 18.0, ui.cursor().top() - 8.0),
                                        Align2::LEFT_TOP,
                                        a,
                                        FontId::proportional(14.0),
                                        Color32::from_gray(200),
                                    );
                                } else {
                                    heading(ui, "SUBTITLES");
                                    if tracks.is_empty() {
                                        ui.painter().text(
                                            pos2(mr.left() + 18.0, ui.cursor().top() - 6.0),
                                            Align2::LEFT_TOP,
                                            none_yet.as_str(),
                                            FontId::proportional(14.0),
                                            Color32::from_gray(200),
                                        );
                                        return;
                                    }
                                    let note = match &self.subs {
                                        SubState::Loading(..) => Some(("Loading…", MUTED())),
                                        SubState::Failed(_, e) => Some((e.as_str(), ERROR())),
                                        _ => None,
                                    };
                                    egui::ScrollArea::vertical()
                                        .max_height(h - 54.0)
                                        .auto_shrink([false, true])
                                        .show(ui, |ui| {
                                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                                            let off = sub_on.is_none();
                                            if menu_row(ui, w, "Off", Some(off), false).clicked() {
                                                sub_pick = Some(None);
                                            }
                                            for (t, label) in tracks.iter().enumerate() {
                                                if first_addon == Some(t) {
                                                    let (r, _) = ui.allocate_exact_size(
                                                        vec2(w, 30.0),
                                                        Sense::hover(),
                                                    );
                                                    ui.painter().text(
                                                        pos2(r.left() + 18.0, r.bottom() - 8.0),
                                                        Align2::LEFT_BOTTOM,
                                                        "FROM ADDONS",
                                                        FontId::proportional(11.0),
                                                        Color32::from_gray(150),
                                                    );
                                                }
                                                let on = sub_on == Some(t);
                                                let r = menu_row(ui, w, label, Some(on), false);
                                                let r = match note {
                                                    Some((n, _)) if on => r.on_hover_text(n),
                                                    _ => r,
                                                };
                                                if on {
                                                    if let Some((n, c)) = note {
                                                        let short = if n.chars().count() > 22 {
                                                            "Couldn't load"
                                                        } else {
                                                            n
                                                        };
                                                        ui.painter().text(
                                                            pos2(
                                                                r.rect.right() - 14.0,
                                                                r.rect.center().y,
                                                            ),
                                                            Align2::RIGHT_CENTER,
                                                            short,
                                                            FontId::proportional(12.0),
                                                            c,
                                                        );
                                                    }
                                                }
                                                if r.clicked() {
                                                    sub_pick = Some(Some(t));
                                                }
                                            }
                                        });
                                    if sub_on.is_some() {
                                        ui.add_space(6.0);
                                        let y = ui.cursor().top();
                                        ui.painter().hline(
                                            mr.x_range().shrink(18.0),
                                            y,
                                            egui::Stroke::new(1.0f32, Color32::from_gray(48)),
                                        );
                                        ui.add_space(8.0);
                                        ui.horizontal(|ui| {
                                            ui.add_space(18.0);
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            ui.label(
                                                egui::RichText::new(delay_text(self.sub_delay))
                                                    .size(13.0)
                                                    .color(Color32::from_gray(200)),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.add_space(14.0);
                                                    let b = |t: &str| {
                                                        egui::Button::new(
                                                            egui::RichText::new(t)
                                                                .size(12.5)
                                                                .color(Color32::WHITE),
                                                        )
                                                        .fill(Color32::from_white_alpha(20))
                                                        .corner_radius(5)
                                                    };
                                                    if ui
                                                        .add(b("Later"))
                                                        .on_hover_text("X")
                                                        .clicked()
                                                    {
                                                        delay_by = Some(0.1);
                                                    }
                                                    if ui
                                                        .add(b("Earlier"))
                                                        .on_hover_text("Z")
                                                        .clicked()
                                                    {
                                                        delay_by = Some(-0.1);
                                                    }
                                                    if self.sub_delay != 0.0
                                                        && ui.add(b("Reset")).clicked()
                                                    {
                                                        delay_by = Some(0.0);
                                                    }
                                                },
                                            );
                                        });
                                        ui.add_space(8.0);
                                    }
                                }
                            });
                        let area_r = area.response.rect.union(mr);
                        on_controls |= ui.rect_contains_pointer(area_r);
                        let clicked_away = ui.input(|i| i.pointer.any_click())
                            && !ui.rect_contains_pointer(area_r)
                            && !ui.rect_contains_pointer(anchor);
                        if clicked_away {
                            self.menu = None;
                        }
                    }

                    // ---- seek bar and times
                    let font = FontId::proportional(12.5);
                    let time_col = Color32::from_gray(210);
                    let shown = self.seek_drag.unwrap_or(pos);
                    let left = (dur - shown).max(0.0);
                    let (inset_l, inset_r) = match look.times {
                        Times::Split => (66.0, 74.0),
                        Times::Left => (24.0, 96.0),
                        Times::Both => (16.0, 16.0),
                    };
                    match look.times {
                        Times::Split => {
                            p.text(
                                pos2(bottom.left() + 24.0, by),
                                Align2::LEFT_CENTER,
                                fmt_time(shown),
                                font.clone(),
                                time_col,
                            );
                            // time left, counting down (while seeking: left after the drop point)
                            p.text(
                                pos2(bottom.right() - 16.0, by),
                                Align2::RIGHT_CENTER,
                                format!("-{}", fmt_time(left)),
                                font.clone(),
                                time_col,
                            );
                        }
                        Times::Left => {
                            p.text(
                                pos2(bottom.right() - 24.0, by),
                                Align2::RIGHT_CENTER,
                                fmt_time(left),
                                bold(14.0),
                                Color32::WHITE,
                            );
                        }
                        Times::Both => {
                            p.text(
                                pos2(bottom.left() + 200.0, row_cy),
                                Align2::LEFT_CENTER,
                                format!("{} / {}", fmt_time(shown), fmt_time(dur)),
                                FontId::proportional(13.5),
                                Color32::WHITE,
                            );
                        }
                    }
                    if let (TitleAt::Bottom, Some(it)) = (look.title, item) {
                        if row {
                            // Netflix: "Title  S1:E2 Episode" in the middle of the button row
                            let mut job = egui::text::LayoutJob::default();
                            let fmt = |f: FontId, c: Color32| egui::TextFormat {
                                font_id: f,
                                color: c,
                                ..Default::default()
                            };
                            job.append(&it.title, 0.0, fmt(bold(16.0), Color32::WHITE));
                            if !it.subtitle.is_empty() {
                                job.append(
                                    &it.subtitle,
                                    12.0,
                                    fmt(FontId::proportional(15.0), Color32::from_gray(190)),
                                );
                            }
                            let g = ui.fonts(|f| f.layout_job(job));
                            p.galley(
                                pos2(
                                    bottom.center().x - g.size().x / 2.0,
                                    row_cy - g.size().y / 2.0,
                                ),
                                g,
                                Color32::WHITE,
                            );
                        } else {
                            // Apple: the title over the bar
                            p.text(
                                pos2(bottom.left() + 24.0, by - 18.0),
                                Align2::LEFT_BOTTOM,
                                &it.title,
                                bold(19.0),
                                Color32::WHITE,
                            );
                            if !it.subtitle.is_empty() {
                                p.text(
                                    pos2(bottom.left() + 24.0, by - 42.0),
                                    Align2::LEFT_BOTTOM,
                                    &it.subtitle,
                                    FontId::proportional(13.0),
                                    Color32::from_gray(190),
                                );
                            }
                        }
                    }
                    let bar = Rect::from_min_max(
                        pos2(bottom.left() + inset_l, by - 8.0),
                        pos2(bottom.right() - inset_r, by + 8.0),
                    );
                    let s = ui.interact(bar, ui.id().with("seekbar"), Sense::click_and_drag());
                    let hot = s.hovered() || s.dragged();
                    let th = if hot { look.bar.1 } else { look.bar.0 };
                    let track = Rect::from_center_size(bar.center(), vec2(bar.width(), th));
                    p.rect_filled(track, th / 2.0, Color32::from_white_alpha(60));
                    let f = if dur > 0.0 {
                        (shown / dur).clamp(0.0, 1.0) as f32
                    } else {
                        0.0
                    };
                    let played_x = track.left() + track.width() * f;
                    p.rect_filled(
                        Rect::from_min_max(track.min, pos2(played_x, track.bottom())),
                        th / 2.0,
                        theme::played(),
                    );
                    let at = |x: f32| {
                        (((x - track.left()) / track.width()).clamp(0.0, 1.0) as f64) * dur
                    };
                    if let Some(k) = look.knob {
                        if hot || row {
                            let r = if hot { 8.0 } else { 6.5 };
                            p.circle_filled(pos2(played_x, track.center().y), r, k);
                        }
                    }
                    if hot {
                        if let Some(hp) = s.hover_pos() {
                            p.text(
                                pos2(hp.x, bar.top() - 6.0),
                                Align2::CENTER_BOTTOM,
                                fmt_time(at(hp.x)),
                                FontId::proportional(12.0),
                                Color32::WHITE,
                            );
                        }
                    }
                    if dur > 0.0 {
                        if s.dragged() {
                            if let Some(pp) = s.interact_pointer_pos() {
                                self.seek_drag = Some(at(pp.x));
                            }
                        }
                        if s.drag_stopped() {
                            seek_to = self.seek_drag.take();
                        } else if s.clicked() {
                            if let Some(pp) = s.interact_pointer_pos() {
                                seek_to = Some(at(pp.x));
                            }
                        }
                    }
                    let s = s.on_hover_cursor(CursorIcon::PointingHand);
                    let _ = s;

                    // ---- transport: big in the middle, or at the start of the row
                    let secs = format!("{}", skip as u32);
                    let (play_r, back_r, fwd_r) = if row {
                        let at = |dx: f32, s: f32| {
                            Rect::from_center_size(pos2(bottom.left() + dx, row_cy), vec2(s, s))
                        };
                        (at(34.0, 40.0), at(80.0, 40.0), at(124.0, 40.0))
                    } else {
                        let cy = bottom.bottom() - 40.0;
                        let cx = bottom.center().x;
                        let at =
                            |dx: f32, s: f32| Rect::from_center_size(pos2(cx + dx, cy), vec2(s, s));
                        (at(0.0, 56.0), at(-82.0, 44.0), at(82.0, 44.0))
                    };
                    if ctl_button(
                        ui,
                        back_r,
                        Icon::SkipBack,
                        &format!("Back {secs} seconds (left arrow key)"),
                        disc,
                    )
                    .clicked()
                    {
                        skip_by = Some(-skip);
                    }
                    if ctl_button(
                        ui,
                        fwd_r,
                        Icon::SkipForward,
                        &format!("Forward {secs} seconds (right arrow key)"),
                        disc,
                    )
                    .clicked()
                    {
                        skip_by = Some(skip);
                    }
                    for r in [back_r, fwd_r] {
                        p.text(
                            r.center() + vec2(0.0, 1.5),
                            Align2::CENTER_CENTER,
                            &secs,
                            bold(8.5),
                            Color32::WHITE,
                        );
                    }
                    let pb = ui.interact(play_r, ui.id().with("play_btn"), Sense::click());
                    let icon = if playing { Icon::Pause } else { Icon::Play };
                    let mut icon_col = Color32::WHITE;
                    if !row {
                        let fill = match theme::skin() {
                            // Apple and Prime: a frosted disc; Disney: just the icon
                            _ if look.discs => {
                                Color32::from_white_alpha(if pb.hovered() { 60 } else { 36 })
                            }
                            Skin::Disney => Color32::TRANSPARENT,
                            _ if pb.hovered() => Color32::from_gray(70),
                            _ => Color32::from_gray(48),
                        };
                        p.circle_filled(play_r.center(), 28.0, fill);
                    } else if pb.hovered() {
                        p.circle_filled(play_r.center(), 20.0, Color32::from_white_alpha(28));
                    } else {
                        icon_col = Color32::from_gray(235);
                    }
                    chrome::draw_icon(
                        &p,
                        play_r.center() + vec2(if playing { 0.0 } else { 1.5 }, 0.0),
                        icon,
                        icon_col,
                    );
                    let tip = if playing {
                        "Pause (Space)"
                    } else {
                        "Play (Space)"
                    };
                    do_toggle |= pb
                        .on_hover_text(tip)
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked();

                    if self.has_next() {
                        let episode = self.current.is_some_and(|i| self.items[i].episode);
                        let label = if episode { "Next Episode" } else { "Next" };
                        let g = p.layout_no_wrap(
                            label.to_owned(),
                            FontId::proportional(14.5),
                            Color32::WHITE,
                        );
                        let (right, ncy) = if row {
                            (free_right, row_cy)
                        } else {
                            (bottom.right() - 22.0, bottom.bottom() - 40.0)
                        };
                        let nr = Rect::from_min_size(
                            pos2(right - g.size().x - 50.0, ncy - 20.0),
                            vec2(g.size().x + 50.0, 40.0),
                        );
                        let nb = ui.interact(nr, ui.id().with("next_btn"), Sense::click());
                        let fill = if nb.hovered() {
                            Color32::from_gray(64)
                        } else {
                            Color32::from_gray(40)
                        };
                        p.rect_filled(nr, 4.0, fill);
                        p.galley(
                            pos2(nr.left() + 16.0, nr.center().y - g.size().y / 2.0),
                            g,
                            Color32::WHITE,
                        );
                        chrome::draw_icon(
                            &p,
                            pos2(nr.right() - 18.0, nr.center().y),
                            Icon::ChevronRight,
                            Color32::WHITE,
                        );
                        do_next |= nb
                            .on_hover_text("Next (N)")
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .clicked();
                    }
                }

                if resp.double_clicked() && !on_controls {
                    Self::toggle_fullscreen(ctx);
                } else if resp.clicked() && !on_controls {
                    if self.menu.is_some() {
                        self.menu = None;
                    } else {
                        do_toggle = true;
                    }
                }
            });
        if let Some(d) = skip_by.take() {
            self.skip(d);
        }

        if hide_ui {
            ctx.set_cursor_icon(CursorIcon::None);
        }

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            self.add_files(dropped);
        }
        if do_open {
            if let Some(files) = crate::pick_files() {
                self.add_files(files);
            }
        }
        if do_prev {
            let restart = self.player.as_ref().is_some_and(|p| p.clock() > 3.0);
            match self.current {
                _ if restart => seek_to = Some(0.0),
                Some(i) if i > 0 => pick = Some(i - 1),
                _ => {}
            }
        }
        if do_next && self.has_next() {
            pick = self.current.map(|i| i + 1);
        }
        if do_toggle {
            self.toggle_play();
        }
        if let (Some(t), Some(p)) = (seek_to, self.player.as_mut()) {
            p.seek(t);
        }
        if let Some(i) = pick {
            self.open_index(ctx, i, audio, progress, cfg);
        }

        if close {
            self.save_progress(progress);
            progress.flush();
            if fullscreen {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            }
        }
        if let Some(t) = sub_pick {
            self.choose_subs(ctx, cfg, t);
        }
        if let Some(d) = delay_by {
            self.sub_delay = if d == 0.0 {
                0.0
            } else {
                ((self.sub_delay + d) * 10.0).round() / 10.0
            };
            self.delay_changed = Some(Instant::now());
        }
        cfg.volume = vol;
        ctx.request_repaint_after(Duration::from_millis(if playing { 8 } else { 100 }));
        close
    }
}

/// A clickable line in a player menu: a check when `checked` is Some(true),
/// a chevron when it opens more.
fn menu_row(ui: &mut egui::Ui, w: f32, label: &str, checked: Option<bool>, more: bool) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(w, MENU_ROW), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(
            r.shrink2(vec2(6.0, 1.0)),
            5.0,
            Color32::from_white_alpha(18),
        );
    }
    let x = if checked.is_some() {
        r.left() + 40.0
    } else {
        r.left() + 18.0
    };
    if checked == Some(true) {
        let c = pos2(r.left() + 22.0, r.center().y);
        let s = egui::Stroke::new(2.0f32, Color32::WHITE);
        p.line_segment([c + vec2(-5.0, 0.0), c + vec2(-1.5, 3.5)], s);
        p.line_segment([c + vec2(-1.5, 3.5), c + vec2(5.5, -4.0)], s);
    }
    let on = checked == Some(true);
    p.text(
        pos2(x, r.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        if on {
            Color32::WHITE
        } else {
            Color32::from_gray(200)
        },
    );
    if more {
        chrome::draw_icon(
            p,
            pos2(r.right() - 20.0, r.center().y),
            Icon::ChevronRight,
            Color32::from_gray(170),
        );
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Subtitle text centered over the video, its last line ending at `base`,
/// white with a dark outline so it reads on any picture.
fn draw_subtitle(p: &egui::Painter, video: Rect, base: f32, text: &str) {
    let size = (video.height() * 0.048).clamp(16.0, 64.0);
    let layout = |color: Color32| {
        let mut job = egui::text::LayoutJob::simple(
            text.to_owned(),
            FontId::proportional(size),
            color,
            video.width() * 0.86,
        );
        job.halign = egui::Align::Center;
        p.layout_job(job)
    };
    let (fg, shadow) = (
        layout(Color32::WHITE),
        layout(Color32::from_black_alpha(230)),
    );
    let top = pos2(video.center().x, base - fg.size().y);
    let o = (size / 14.0).max(1.5);
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
        p.galley(top + vec2(dx, dy), shadow.clone(), Color32::BLACK);
    }
    p.galley(top, fg, Color32::WHITE);
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
