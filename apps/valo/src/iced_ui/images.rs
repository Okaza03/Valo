//! Posters and backdrops for the Iced UI: downloaded and decoded on worker
//! threads (like `crate::images`), kept as Iced image handles.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use iced::widget::image::Handle;

use crate::api::Client;

const WORKERS: usize = 6;
/// Backdrops, banners and page posters are decoded this big at most …
const MAX_SIDE: u32 = 1280;
/// … and card art this big: enough for a card on a 2× display.
const CARD_SIDE: u32 = 512;

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

/// Images waiting for a worker. The newest go first: they're what's on
/// screen now, while older ones may have scrolled away.
#[derive(Default)]
struct Queue {
    urls: Mutex<Vec<Key>>,
    ready: Condvar,
}

impl Queue {
    fn push(&self, key: Key) {
        self.urls.lock().unwrap().push(key);
        self.ready.notify_one();
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
                self.req.push(key);
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

    /// Downloads are on their way: poll for them.
    pub fn busy(&self) -> bool {
        self.waiting.get() > 0
    }

    /// Take in the images that arrived. True when any did (redraw).
    pub fn poll(&mut self) -> bool {
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

fn load(c: &Client, url: &str, max_side: u32) -> Option<(u32, u32, Vec<u8>)> {
    let bytes = c
        .fetch(url)
        .map_err(|e| eprintln!("image {url}: {e:#}"))
        .ok()?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| eprintln!("image {url}: {e}"))
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
    use super::Variant;

    #[test]
    fn cards_get_the_small_copy_pages_the_full_one() {
        assert_eq!(Variant::for_size(150.0, 225.0), Variant::Card); // poster card
        assert_eq!(Variant::for_size(288.0, 162.0), Variant::Card); // Continue watching
        assert_eq!(Variant::for_size(220.0, 330.0), Variant::Full); // movie page poster
        assert_eq!(Variant::for_size(960.0, 560.0), Variant::Full); // banner
    }
}
