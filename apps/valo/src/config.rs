//! Settings and watch progress, stored as JSON in ~/.config/valo
//! (%APPDATA%\valo on Windows).
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::util::now_unix;

#[cfg(windows)]
fn config_home() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

#[cfg(not(windows))]
fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

fn dir() -> Option<PathBuf> {
    Some(config_home()?.join("valo"))
}

fn load<T: DeserializeOwned + Default>(name: &str) -> T {
    dir()
        .and_then(|d| fs::read_to_string(d.join(name)).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save<T: Serialize>(name: &str, v: &T) {
    let Some(d) = dir() else { return };
    let write = || -> std::io::Result<()> {
        fs::create_dir_all(&d)?;
        let tmp = d.join(format!("{name}.tmp"));
        fs::write(&tmp, serde_json::to_vec_pretty(v)?)?;
        fs::rename(tmp, d.join(name))
    };
    if let Err(e) = write() {
        eprintln!("can't save {name}: {e}");
    }
}

/// Where one user's progress and list live in progress.json and
/// watchlist.json: keys are this plus the item id. Each signed-in user of a
/// server gets their own; `None` is a server without sign-in.
pub fn user_prefix(base: &str, user: Option<&str>) -> String {
    format!("{base}|@{}|", user.unwrap_or("").to_lowercase())
}

/// Window size and place, in the desktop's logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    /// The normal (not maximized) size, so un-maximizing goes back to it.
    pub w: f32,
    pub h: f32,
    /// Top-left corner; unknown on Wayland, which places windows itself.
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub maximized: bool,
}

impl WindowState {
    /// Something a window can be restored to (not tiny, no broken position).
    pub fn usable(&self) -> bool {
        let size_ok = (560.0..=20000.0).contains(&self.w) && (400.0..=20000.0).contains(&self.h);
        let place_ok = match (self.x, self.y) {
            // monitors left of or above the main one have negative positions
            (Some(x), Some(y)) => x.abs() < 20000.0 && y.abs() < 20000.0,
            _ => true,
        };
        size_ok && place_ok
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Server address as typed by the user, e.g. "192.168.1.20:8080".
    pub server: String,
    pub username: String,
    /// Session token, kept only when "remember me" is ticked.
    pub token: Option<String>,
    /// Whether the saved session's user is an admin (cleared with the token).
    pub admin: bool,
    pub volume: f32,
    /// Play the next episode when one ends.
    pub autoplay_next: bool,
    /// Continue from the saved position instead of the start.
    pub resume: bool,
    pub fullscreen_on_play: bool,
    /// Arrow-key skip, seconds (Shift = ×6).
    pub skip_seconds: u32,
    /// Re-fetch the library this often; 0 = never.
    pub refresh_minutes: u32,
    pub genre_rows: bool,
    /// Interface zoom.
    pub ui_scale: f32,
    /// Keep an app-menu entry for the AppImage (see integrate.rs).
    pub app_menu: bool,
    /// The last version whose "What's new" was shown; empty before 0.7.0.
    pub seen_version: String,
    /// Show what's playing on the user's Discord profile.
    pub discord_presence: bool,
    /// The window as it was last closed; `None` until then.
    pub window: Option<WindowState>,
    /// Color theme: dark, light, …
    pub theme: crate::ui::theme::Mode,
    /// App style: Valo, Netflix, …
    pub style: crate::ui::theme::Skin,
    /// Subtitles in this language are turned on by themselves; `None` = off.
    pub subtitle_language: Option<String>,
    /// … and the SDH kind of them was picked last.
    pub subtitle_sdh: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: String::new(),
            username: String::new(),
            token: None,
            admin: false,
            volume: 0.7,
            autoplay_next: true,
            resume: true,
            fullscreen_on_play: false,
            skip_seconds: 5,
            refresh_minutes: 0,
            genre_rows: true,
            ui_scale: 1.0,
            app_menu: true,
            seen_version: String::new(),
            discord_presence: true,
            window: None,
            theme: Default::default(),
            style: Default::default(),
            subtitle_language: None,
            subtitle_sdh: false,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        load("config.json")
    }
    pub fn save(&self) {
        save("config.json", self);
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub pos: f64,
    pub dur: f64,
    pub updated: i64,
}

/// Watched this share of a title, it counts as finished.
pub const FINISHED: f64 = 0.95;

impl Entry {
    pub fn fraction(&self) -> f32 {
        if self.dur > 0.0 {
            (self.pos / self.dur).clamp(0.0, 1.0) as f32
        } else {
            0.0
        }
    }
    /// Started, but not finished (95% watched counts as finished, which
    /// skips the end credits).
    pub fn in_progress(&self) -> bool {
        self.pos > 30.0 && (self.dur <= 0.0 || self.pos < self.dur * FINISHED)
    }
    pub fn watched(&self) -> bool {
        self.dur > 0.0 && !self.in_progress() && self.pos > 30.0
    }
}

/// Resume points keyed by "<server>|<stream id>". An entry at position 0 is a
/// cleared one, kept so the clearing reaches the server (see sync.rs).
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Progress {
    entries: HashMap<String, Entry>,
    /// Keys changed here that the server hasn't been told about yet.
    unsent: HashSet<String>,
    #[serde(skip)]
    dirty: bool,
}

impl Progress {
    pub fn load() -> Self {
        load("progress.json")
    }

    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.entries.get(key).filter(|e| e.pos > 0.0)
    }

    pub fn set(&mut self, key: &str, pos: f64, dur: f64) {
        self.entries.insert(
            key.to_owned(),
            Entry {
                pos,
                dur,
                updated: now_unix(),
            },
        );
        self.unsent.insert(key.to_owned());
        self.dirty = true;
    }

    /// Entries for one server, newest first.
    pub fn recent(&self, prefix: &str) -> Vec<(&str, &Entry)> {
        let mut v: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, e)| e.pos > 0.0)
            .filter_map(|(k, e)| k.strip_prefix(prefix).map(|id| (id, e)))
            .collect();
        v.sort_by_key(|(_, e)| std::cmp::Reverse(e.updated));
        v
    }

    /// Take the server's copy of each entry it has a newer version of.
    /// Returns whether anything changed.
    pub fn merge(&mut self, prefix: &str, remote: Vec<(String, Entry)>) -> bool {
        let mut changed = false;
        for (id, e) in remote {
            let key = format!("{prefix}{id}");
            if self
                .entries
                .get(&key)
                .is_some_and(|l| l.updated >= e.updated)
            {
                continue;
            }
            self.unsent.remove(&key);
            self.entries.insert(key, e);
            changed = true;
        }
        self.dirty |= changed;
        changed
    }

    /// Changes for one server that still have to be sent.
    pub fn unsent(&self, prefix: &str) -> Vec<(String, Entry)> {
        self.unsent
            .iter()
            .filter_map(|k| {
                Some((
                    k.strip_prefix(prefix)?.to_owned(),
                    self.entries.get(k)?.clone(),
                ))
            })
            .collect()
    }

    /// The server has `id` as it was in `sent`. Unless it changed here in the
    /// meantime, stop sending it and take the server's timestamp, so the next
    /// pull doesn't mistake the server's copy for a newer one.
    pub fn sent(&mut self, prefix: &str, id: &str, sent: &Entry, server_updated: Option<i64>) {
        let key = format!("{prefix}{id}");
        if let Some(l) = self
            .entries
            .get_mut(&key)
            .filter(|l| l.updated == sent.updated)
        {
            if let Some(t) = server_updated {
                l.updated = t;
            }
            self.unsent.remove(&key);
            self.dirty = true;
        }
    }

    pub fn flush(&mut self) {
        if self.dirty {
            save("progress.json", self);
            self.dirty = false;
        }
    }
}

/// Titles the user bookmarked ("My list"), newest first, keyed like [`Progress`].
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Watchlist {
    items: Vec<String>,
    /// When each key was last added or taken off. Taken-off ones stay here,
    /// so a removal made elsewhere can't be undone by an older add.
    changed: HashMap<String, i64>,
    /// Keys changed here that the server hasn't been told about yet.
    unsent: HashSet<String>,
}

impl Watchlist {
    pub fn load() -> Self {
        let mut w: Self = load("watchlist.json");
        // saved before My list synced: send them once
        for k in &w.items {
            if !w.changed.contains_key(k) {
                w.unsent.insert(k.clone());
            }
        }
        w
    }

    pub fn contains(&self, key: &str) -> bool {
        self.items.iter().any(|k| k == key)
    }

    pub fn toggle(&mut self, key: &str) {
        let on = !self.contains(key);
        self.set(key, on, now_unix());
        self.unsent.insert(key.to_owned());
        save("watchlist.json", self);
    }

    fn set(&mut self, key: &str, on: bool, at: i64) {
        self.items.retain(|k| k != key);
        if on {
            // newest first; titles from before syncing count as oldest
            let at_of = |k: &String| self.changed.get(k).copied().unwrap_or(0);
            let i = self.items.iter().position(|k| at_of(k) <= at);
            self.items
                .insert(i.unwrap_or(self.items.len()), key.to_owned());
        }
        self.changed.insert(key.to_owned(), at);
    }

    /// Item ids saved for the server whose keys start with `prefix`.
    pub fn ids<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.items
            .iter()
            .filter_map(move |k| k.strip_prefix(prefix))
    }

    /// Take the server's adds and removals that are newer than ours: (id,
    /// on the list, when). Returns whether anything changed.
    pub fn merge(&mut self, prefix: &str, remote: Vec<(String, bool, i64)>) -> bool {
        let mut changed = false;
        for (id, on, at) in remote {
            let key = format!("{prefix}{id}");
            if self.changed.get(&key).is_some_and(|&t| t >= at) {
                continue;
            }
            self.unsent.remove(&key);
            self.set(&key, on, at);
            changed = true;
        }
        if changed {
            save("watchlist.json", self);
        }
        changed
    }

    /// Changes for one server still to be sent: (id, on the list).
    pub fn unsent(&self, prefix: &str) -> Vec<(String, bool)> {
        self.unsent
            .iter()
            .filter_map(|k| Some((k.strip_prefix(prefix)?.to_owned(), self.contains(k))))
            .collect()
    }

    /// The server has `id` as `on`, since `at` (its clock) if it said.
    pub fn sent(&mut self, prefix: &str, id: &str, on: bool, at: Option<i64>) {
        let key = format!("{prefix}{id}");
        // changed again while this was on its way: that goes next
        if self.contains(&key) != on {
            return;
        }
        self.unsent.remove(&key);
        let t = at
            .or_else(|| self.changed.get(&key).copied())
            .unwrap_or_else(now_unix);
        self.changed.insert(key, t);
        save("watchlist.json", self);
    }
}

/// Titles removed from "Continue watching", keyed like [`Progress`], with when.
/// Movies and series each server and user has already been shown, by
/// [`user_prefix`], so newly added ones can be announced.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Known {
    seen: HashMap<String, HashSet<String>>,
    /// Each title's name ("Inception (2010)") by id, so one that's taken
    /// off the server can still be named.
    names: HashMap<String, HashMap<String, String>>,
}

impl Known {
    pub fn load() -> Self {
        load("known.json")
    }

    /// `None` the first time this library is seen: nothing counts as new yet.
    pub fn get(&self, prefix: &str) -> Option<&HashSet<String>> {
        self.seen.get(prefix)
    }

    /// Everything in the library now is known; titles since removed are forgotten.
    pub fn set(&mut self, prefix: &str, ids: HashSet<String>) {
        if let Some(names) = self.names.get_mut(prefix) {
            names.retain(|id, _| ids.contains(id));
        }
        self.seen.insert(prefix.to_owned(), ids);
        save("known.json", self);
    }

    pub fn name(&self, prefix: &str, id: &str) -> Option<&str> {
        self.names.get(prefix)?.get(id).map(String::as_str)
    }

    /// Remember the titles' names; ones no longer listed are kept until [`Known::set`].
    pub fn name_all(&mut self, prefix: &str, names: impl IntoIterator<Item = (String, String)>) {
        let known = self.names.entry(prefix.to_owned()).or_default();
        let mut changed = false;
        for (id, name) in names {
            if known.get(&id) != Some(&name) {
                known.insert(id, name);
                changed = true;
            }
        }
        if changed {
            save("known.json", self);
        }
    }
}

/// A title comes back once it's watched again after that.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Hidden {
    items: HashMap<String, i64>,
    /// Keys hidden here that the server hasn't been told about yet.
    unsent: HashSet<String>,
}

impl Hidden {
    pub fn load() -> Self {
        load("hidden.json")
    }

    pub fn hide(&mut self, key: &str) {
        self.items.insert(key.to_owned(), now_unix());
        self.unsent.insert(key.to_owned());
        save("hidden.json", self);
    }

    /// Take the server's removals (id, when) that are newer than ours.
    /// Returns whether anything changed.
    pub fn merge(&mut self, prefix: &str, remote: Vec<(String, i64)>) -> bool {
        let mut changed = false;
        for (id, at) in remote {
            let key = format!("{prefix}{id}");
            if self.items.get(&key).is_some_and(|&t| t >= at) {
                continue;
            }
            self.items.insert(key, at);
            changed = true;
        }
        if changed {
            save("hidden.json", self);
        }
        changed
    }

    /// Ids hidden for one server that still have to be sent.
    pub fn unsent(&self, prefix: &str) -> Vec<String> {
        self.unsent
            .iter()
            .filter_map(|k| Some(k.strip_prefix(prefix)?.to_owned()))
            .collect()
    }

    /// The server has `id` hidden, since `at` (its clock) if it said.
    pub fn sent(&mut self, prefix: &str, id: &str, at: Option<i64>) {
        let key = format!("{prefix}{id}");
        self.unsent.remove(&key);
        if let Some(t) = at {
            self.items.insert(key, t);
        }
        save("hidden.json", self);
    }

    /// Whether `key`, last watched at `updated`, was removed since.
    pub fn hides(&self, key: &str, updated: i64) -> bool {
        self.items.get(key).is_some_and(|t| updated <= *t)
    }
}

#[cfg(test)]
mod tests {
    use super::WindowState;

    #[test]
    fn window_state_sanity() {
        let w = |w, h, x, y| WindowState {
            w,
            h,
            x,
            y,
            maximized: false,
        };
        assert!(w(1280.0, 780.0, Some(100.0), Some(80.0)).usable());
        assert!(w(1280.0, 780.0, Some(-1500.0), Some(0.0)).usable()); // a monitor to the left
        assert!(w(1280.0, 780.0, None, None).usable()); // Wayland
        assert!(!w(200.0, 780.0, None, None).usable()); // too small
        assert!(!w(1280.0, 780.0, Some(-32000.0), Some(-32000.0)).usable()); // Windows' "minimized" spot
    }

    use super::Entry;

    #[test]
    fn finished_at_95_percent() {
        let e = |pos| Entry {
            pos,
            dur: 7200.0,
            updated: 0,
        };
        assert!(e(6839.0).in_progress()); // 94.99%
        assert!(e(6840.0).watched()); // 95%
        assert!(!e(20.0).in_progress() && !e(20.0).watched()); // barely started
    }
}
