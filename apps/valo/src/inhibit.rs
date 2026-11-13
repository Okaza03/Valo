//! Keep the screen awake while a video plays, so the desktop doesn't blank,
//! lock or suspend mid-movie. A worker thread holds the OS inhibition: on
//! Linux over D-Bus (it must keep the connection open, since KDE drops an
//! inhibit when its caller disconnects), on Windows per-thread execution state,
//! on macOS an IOKit power assertion.
use std::sync::mpsc::{self, Sender};
use std::thread;

pub struct Inhibit {
    tx: Option<Sender<bool>>,
    on: bool,
}

impl Inhibit {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<bool>();
        let tx = thread::Builder::new()
            .name("inhibit".into())
            .spawn(move || {
                let mut sys = imp::Sys::new();
                // ends when the sender drops; the OS state is released with it
                for on in rx {
                    sys.set(on);
                }
                sys.set(false);
            })
            .ok()
            .map(|_| tx);
        Self { tx, on: false }
    }

    /// Call every frame; only changes are sent to the worker.
    pub fn set(&mut self, on: bool) {
        if on != self.on {
            self.on = on;
            if let Some(tx) = &self.tx {
                let _ = tx.send(on);
            }
        }
    }
}

#[cfg(windows)]
mod imp {
    const ES_CONTINUOUS: u32 = 0x8000_0000;
    const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;
    const ES_DISPLAY_REQUIRED: u32 = 0x0000_0002;

    #[link(name = "kernel32")]
    extern "system" {
        fn SetThreadExecutionState(flags: u32) -> u32;
    }

    pub struct Sys;

    impl Sys {
        pub fn new() -> Self {
            Sys
        }

        pub fn set(&mut self, on: bool) {
            let flags = if on {
                ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
            } else {
                ES_CONTINUOUS
            };
            unsafe { SetThreadExecutionState(flags) };
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::{c_char, c_void};

    type CFStringRef = *const c_void;
    const UTF8: u32 = 0x0800_0100; // kCFStringEncodingUTF8
    const LEVEL_ON: u32 = 255; // kIOPMAssertionLevelOn

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            s: *const c_char,
            encoding: u32,
        ) -> CFStringRef;
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOPMAssertionCreateWithName(
            kind: CFStringRef,
            level: u32,
            name: CFStringRef,
            id: *mut u32,
        ) -> i32;
        fn IOPMAssertionRelease(id: u32) -> i32;
    }

    fn cfstr(s: &std::ffi::CStr) -> CFStringRef {
        unsafe { CFStringCreateWithCString(std::ptr::null(), s.as_ptr(), UTF8) }
    }

    pub struct Sys {
        held: Option<u32>,
    }

    impl Sys {
        pub fn new() -> Self {
            Self { held: None }
        }

        pub fn set(&mut self, on: bool) {
            match (on, self.held) {
                (true, None) => {
                    // keeps the display on, and with it the system
                    let kind = cfstr(c"PreventUserIdleDisplaySleep");
                    let name = cfstr(c"Valo: playing video");
                    if kind.is_null() || name.is_null() {
                        return;
                    }
                    let mut id = 0;
                    let ok = unsafe { IOPMAssertionCreateWithName(kind, LEVEL_ON, name, &mut id) };
                    unsafe {
                        CFRelease(kind);
                        CFRelease(name);
                    }
                    if ok == 0 {
                        self.held = Some(id);
                    }
                }
                (false, Some(id)) => {
                    unsafe { IOPMAssertionRelease(id) };
                    self.held = None;
                }
                _ => {}
            }
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    // zbus is async; each call here is short, so it runs to the end on this thread
    use zbus::{block_on, Connection};

    /// Who answered and the cookie to release with.
    enum Held {
        ScreenSaver(u32),
        Gnome(u32),
    }

    pub struct Sys {
        conn: Option<Connection>,
        held: Option<Held>,
    }

    const REASON: &str = "Playing video";

    impl Sys {
        pub fn new() -> Self {
            Self {
                conn: None,
                held: None,
            }
        }

        pub fn set(&mut self, on: bool) {
            if on == self.held.is_some() {
                return;
            }
            if self.conn.is_none() {
                self.conn = block_on(Connection::session()).ok();
            }
            let Some(conn) = &self.conn else { return };
            if on {
                self.held = screensaver_inhibit(conn)
                    .map(Held::ScreenSaver)
                    .or_else(|| gnome_inhibit(conn).map(Held::Gnome));
            } else {
                match self.held.take() {
                    Some(Held::ScreenSaver(c)) => {
                        let _ = block_on(conn.call_method(
                            Some("org.freedesktop.ScreenSaver"),
                            "/org/freedesktop/ScreenSaver",
                            Some("org.freedesktop.ScreenSaver"),
                            "UnInhibit",
                            &(c,),
                        ));
                    }
                    Some(Held::Gnome(c)) => {
                        let _ = block_on(conn.call_method(
                            Some("org.gnome.SessionManager"),
                            "/org/gnome/SessionManager",
                            Some("org.gnome.SessionManager"),
                            "Uninhibit",
                            &(c,),
                        ));
                    }
                    None => {}
                }
            }
        }
    }

    /// KDE, Xfce, LXQt, Cinnamon and most others.
    fn screensaver_inhibit(conn: &Connection) -> Option<u32> {
        block_on(conn.call_method(
            Some("org.freedesktop.ScreenSaver"),
            "/org/freedesktop/ScreenSaver",
            Some("org.freedesktop.ScreenSaver"),
            "Inhibit",
            &("valo", REASON),
        ))
        .ok()?
        .body()
        .deserialize()
        .ok()
    }

    /// GNOME; flags 4|8 = inhibit suspend and idle.
    fn gnome_inhibit(conn: &Connection) -> Option<u32> {
        block_on(conn.call_method(
            Some("org.gnome.SessionManager"),
            "/org/gnome/SessionManager",
            Some("org.gnome.SessionManager"),
            "Inhibit",
            &("valo", 0u32, REASON, 4u32 | 8u32),
        ))
        .ok()?
        .body()
        .deserialize()
        .ok()
    }
}
