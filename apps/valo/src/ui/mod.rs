//! What the screens share: the app styles (`theme`), what to show and play
//! (`browse`), the player's queue items, and what a screen asks the app to do.
//! The screens themselves are drawn by the Iced UI (`crate::iced_ui`).
pub mod browse;
pub mod chrome;
pub mod theme;
pub mod watch;
pub mod whats_new;

/// What a screen asks the app to do next.
pub enum Action {
    /// Run the server check on a newly entered address.
    CheckServer(String),
    /// The server answered; sign in or go straight to the library.
    Connected(crate::api::Client),
    /// The server check passed (and the user signed in if needed).
    Ready {
        client: crate::api::Client,
        user: Option<String>,
    },
    SignedIn {
        client: crate::api::Client,
        user: String,
        remember: bool,
    },
    /// The saved session was rejected.
    NeedLogin(crate::api::Client),
    /// The server says the signed-in user's admin rights changed.
    AdminChanged(bool),
    SignOut(crate::api::Client),
    ChangeServer,
    Refresh,
    OpenFiles,
    CheckUpdates,
    /// Re-show the "update available" notice.
    ShowUpdate,
    OpenSettings,
    Quit,
    Play(Vec<watch::PlayItem>),
}
