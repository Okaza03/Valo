//! Small helpers shared by the screens.
use std::sync::mpsc::{self, Receiver};
use std::thread;

use eframe::egui;

/// Wakes the UI when background work finishes: egui redraws; the Iced UI
/// polls on its own ticks, so it passes [`Waker::none`].
#[derive(Clone)]
pub struct Waker(Option<std::sync::Arc<dyn Fn(std::time::Duration) + Send + Sync>>);

impl Waker {
    #[cfg_attr(not(feature = "iced"), allow(dead_code))]
    pub fn none() -> Self {
        Self(None)
    }

    /// Now.
    pub fn wake(&self) {
        self.wake_after(std::time::Duration::ZERO);
    }

    /// Once `d` has passed, to do something then (egui only redraws when asked).
    pub fn wake_after(&self, d: std::time::Duration) {
        if let Some(f) = &self.0 {
            f(d);
        }
    }
}

impl From<&egui::Context> for Waker {
    fn from(ctx: &egui::Context) -> Self {
        let ctx = ctx.clone();
        Self(Some(std::sync::Arc::new(move |d| {
            ctx.request_repaint_after(d)
        })))
    }
}

/// A value computed on a background thread; poll it once per frame.
pub struct Task<T>(Receiver<T>);

impl<T: Send + 'static> Task<T> {
    pub fn spawn(wake: impl Into<Waker>, f: impl FnOnce() -> T + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let wake = wake.into();
        thread::spawn(move || {
            let _ = tx.send(f());
            wake.wake();
        });
        Self(rx)
    }

    pub fn poll(&mut self) -> Option<T> {
        self.0.try_recv().ok()
    }
}

pub fn fmt_time(s: f64) -> String {
    let s = s.max(0.0) as u64;
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m:02}:{sec:02}")
    }
}

/// "1h 52m" / "48m"
pub fn fmt_runtime(s: f64) -> String {
    let m = (s / 60.0).round() as u64;
    if m >= 60 {
        format!("{}h {:02}m", m / 60, m % 60)
    } else {
        format!("{m}m")
    }
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Stable pleasant color for items without artwork.
pub fn color_for(s: &str) -> egui::Color32 {
    let h = s
        .bytes()
        .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    let hue = (h % 360) as f32 / 360.0;
    egui::ecolor::Hsva::new(hue, 0.45, 0.42, 1.0).into()
}
