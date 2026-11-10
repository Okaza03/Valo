//! Window chrome: our own title bar (the window has no system decorations),
//! the status bar, and edge resizing.

#[derive(Clone, Copy)]
pub enum Icon {
    Refresh,
    Settings,
    Minimize,
    Maximize,
    Restore,
    Close,
    Bookmark,
    BookmarkOn,
    Play,
    ChevronLeft,
    ArrowRight,
    Star,
    Pause,
    ChevronRight,
    Subtitles,
    Volume,
    VolumeMuted,
    Fullscreen,
    FullscreenExit,
    More,
    List,
    /// Circular arrows; the caller writes the seconds inside.
    SkipBack,
    SkipForward,
    ChevronUp,
    ChevronDown,
    ArrowUp,
    ArrowDown,
    /// Opens outside the app (a web page): an arrow out of a corner.
    External,
    /// Sidebar: a house.
    Home,
    /// Sidebar: a film frame with a play mark.
    Film,
    /// Sidebar: a screen on a stand.
    Tv,
}

/// Open a link in the user's browser.
pub fn open_url(url: &str) {
    // `cmd /c start` would treat `&` in the URL as a command separator
    #[cfg(windows)]
    let res = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(not(windows))]
    let res = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = res {
        eprintln!("can't open {url}: {e}");
    }
}
