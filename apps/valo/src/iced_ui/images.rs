//! Posters and backdrops for the Iced UI: downloaded and decoded on worker
//! threads (like `crate::images`), kept as Iced image handles.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use iced::widget::image::Handle;

use crate::api::Client;

const WORKERS: usize = 6;
/// Backdrops, banners and page posters are decoded this big at most …
const MAX_SIDE: u32 = 1280;
/// … and card art this big: enough for a card on a 2× display.
const CARD_SIDE: u32 = 512;
/// Downloaded art is kept on disk and used again for this long, as long as
/// the server itself lets browsers cache it.
const KEEP: Duration = Duration::from_secs(24 * 60 * 60);

/// Which copy of an image: card-sized or full. The same URL can be both,
/// a poster on a card and on the movie page, and each is kept apart.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Variant {
    Card,
    Full,
}

impl Variant {
    /// The copy to show `w`×`h` big.
    pub fn for_size(w: f32, h: f32) -> Self {
        if w.max(h) <= 320.0 {
            Variant::Card
        } else {
            Variant::Full
        }
    }

    fn max_side(self) -> u32 {
        match self {
            Variant::Card => CARD_SIDE,
            Variant::Full => MAX_SIDE,
        }
    }
}

type Key = (String, Variant);

enum State {
    Loading,
    Ready(Handle, (u32, u32)),
    Failed,
}

type Done = (Key, Option<(u32, u32, Vec<u8>)>);

/// Images waiting for a worker, taken from the end. What a newer screen
/// asked for goes first (older asks may have scrolled away), and within one
/// screen, top to bottom and left to right: the order the view asks in.
#[derive(Default)]
struct Queue {
    urls: Mutex<Vec<Key>>,
    ready: Condvar,
}

impl Queue {
    /// Queue `key` just after everything asked for before `mark`, so the
    /// asks since then are taken in the order they came.
    fn insert(&self, mark: usize, key: Key) {
        let mut urls = self.urls.lock().unwrap();
        let at = mark.min(urls.len());
        urls.insert(at, key);
        self.ready.notify_one();
    }

    fn len(&self) -> usize {
        self.urls.lock().unwrap().len()
    }

    fn pop(&self) -> Key {
        let mut urls = self.urls.lock().unwrap();
        loop {
            if let Some(u) = urls.pop() {
                return u;
            }
            urls = self.ready.wait(urls).unwrap();
        }
    }
}

pub struct Images {
    /// Asked for from views, which only borrow the app: hence the cells.
    map: RefCell<HashMap<Key, State>>,
    /// Downloads not back yet: the app polls while there are any.
    waiting: Cell<usize>,
    req: Arc<Queue>,
    /// How much of the queue was there at the last poll: what's asked for
    /// since then goes on top of it, in order.
    mark: Cell<usize>,
    done: Receiver<Done>,
    client: Arc<Mutex<Option<Client>>>,
}

impl Images {
    pub fn new() -> Self {
        let req = Arc::new(Queue::default());
        let (done_tx, done) = mpsc::channel::<Done>();
        let client: Arc<Mutex<Option<Client>>> = Arc::default();
        for _ in 0..WORKERS {
            let (queue, tx, client) = (req.clone(), done_tx.clone(), client.clone());
            thread::spawn(move || loop {
                let key = queue.pop();
                let c = client.lock().unwrap().clone();
                let img = c.and_then(|c| load(&c, &key.0, key.1.max_side()));
                if tx.send((key, img)).is_err() {
                    return;
                }
            });
        }
        Self {
            map: RefCell::default(),
            waiting: Cell::new(0),
            req,
            mark: Cell::new(0),
            done,
            client,
        }
    }

    /// Credentials for the server the images come from; clears the cache.
    pub fn set_client(&mut self, c: Option<Client>) {
        *self.client.lock().unwrap() = c;
        self.map.get_mut().clear();
        self.waiting.set(0);
    }

    /// The image's `v` copy and its size if it's ready; starts loading it
    /// otherwise.
    pub fn get(&self, url: &str, v: Variant) -> Option<(Handle, (u32, u32))> {
        let mut map = self.map.borrow_mut();
        let key = (url.to_owned(), v);
        match map.get(&key) {
            Some(State::Ready(h, size)) => Some((h.clone(), *size)),
            Some(_) => None,
            None => {
                map.insert(key.clone(), State::Loading);
                self.waiting.set(self.waiting.get() + 1);
                self.req.insert(self.mark.get(), key);
                None
            }
        }
    }

    /// Its `v` copy couldn't be downloaded or decoded: show a stand-in for good.
    pub fn failed(&self, url: &str, v: Variant) -> bool {
        matches!(
            self.map.borrow().get(&(url.to_owned(), v)),
            Some(State::Failed)
        )
    }

    /// Drop every copy of `url`, so it's downloaded again (after it changed
    /// on the server under the same address).
    pub fn forget(&self, url: &str) {
        let mut map = self.map.borrow_mut();
        map.retain(|(u, _), state| {
            if u != url {
                return true;
            }
            if matches!(state, State::Loading) {
                self.waiting.set(self.waiting.get().saturating_sub(1));
            }
            false
        });
        if let Some(f) = disk_path(url) {
            let _ = std::fs::remove_file(f);
        }
    }

    /// Downloads are on their way: poll for them.
    pub fn busy(&self) -> bool {
        self.waiting.get() > 0
    }

    /// Take in the images that arrived. True when any did (redraw).
    pub fn poll(&mut self) -> bool {
        self.mark.set(self.req.len());
        let mut any = false;
        while let Ok((key, img)) = self.done.try_recv() {
            let map = self.map.get_mut();
            let Some(slot) = map.get_mut(&key) else {
                continue; // the cache was cleared
            };
            *slot = match img {
                Some((w, h, rgba)) => State::Ready(Handle::from_rgba(w, h, rgba), (w, h)),
                None => State::Failed,
            };
            self.waiting.set(self.waiting.get().saturating_sub(1));
            any = true;
        }
        any
    }
}

/// Where the downloaded art for `url` is kept between runs.
fn disk_path(url: &str) -> Option<PathBuf> {
    use sha2::{Digest, Sha256};
    #[cfg(windows)]
    let home = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let home = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")));
    let hash = Sha256::digest(url.as_bytes());
    let name: String = hash.iter().take(16).map(|b| format!("{b:02x}")).collect();
    Some(home?.join("valo").join("art").join(name))
}

/// The art's bytes: from disk when it was kept less than [`KEEP`] ago,
/// downloaded (and kept) otherwise, or an older copy when that fails.
fn bytes(c: &Client, url: &str) -> Option<Vec<u8>> {
    let path = disk_path(url);
    let kept = path.as_ref().and_then(|p| {
        let age = std::fs::metadata(p).ok()?.modified().ok()?;
        let fresh = SystemTime::now()
            .duration_since(age)
            .is_ok_and(|a| a < KEEP);
        Some((std::fs::read(p).ok()?, fresh))
    });
    if let Some((b, true)) = &kept {
        return Some(b.clone());
    }
    match c.fetch(url) {
        Ok(b) => {
            if let Some(p) = &path {
                let tmp = p.with_extension("part");
                let _ = p
                    .parent()
                    .map(std::fs::create_dir_all)
                    .transpose()
                    .and_then(|_| std::fs::write(&tmp, &b))
                    .and_then(|_| std::fs::rename(&tmp, p));
            }
            Some(b)
        }
        Err(e) => {
            eprintln!("image {url}: {e:#}");
            kept.map(|(b, _)| b)
        }
    }
}

fn load(c: &Client, url: &str, max_side: u32) -> Option<(u32, u32, Vec<u8>)> {
    let bytes = bytes(c, url)?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| {
            eprintln!("image {url}: {e}");
            // don't keep what can't be shown
            if let Some(p) = disk_path(url) {
                let _ = std::fs::remove_file(p);
            }
        })
        .ok()?;
    let img = if img.width() > max_side || img.height() > max_side {
        img.resize(max_side, max_side, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    Some((rgba.width(), rgba.height(), rgba.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::{Queue, Variant};

    fn key(s: &str) -> (String, Variant) {
        (s.to_owned(), Variant::Card)
    }

    #[test]
    fn what_is_on_screen_now_loads_first_top_to_bottom() {
        let q = Queue::default();
        // a first screen asks for a, b, c in that order
        for k in ["a", "b", "c"] {
            q.insert(0, key(k));
        }
        assert_eq!(q.pop().0, "a");
        // scrolled on: the new screen's d, e go ahead of the old b, c
        let mark = q.len();
        for k in ["d", "e"] {
            q.insert(mark, key(k));
        }
        let order: Vec<String> = (0..4).map(|_| q.pop().0).collect();
        assert_eq!(order, ["d", "e", "b", "c"]);
    }

    #[test]
    fn cards_get_the_small_copy_pages_the_full_one() {
        assert_eq!(Variant::for_size(150.0, 225.0), Variant::Card); // poster card
        assert_eq!(Variant::for_size(288.0, 162.0), Variant::Card); // Continue watching
        assert_eq!(Variant::for_size(220.0, 330.0), Variant::Full); // movie page poster
        assert_eq!(Variant::for_size(960.0, 560.0), Variant::Full); // banner
    }
}
