//! Color themes (dark, light, …) and app styles (Valo, Netflix, …).
//!
//! The style decides the layout: how the home page, cards, details pages,
//! navigation and player are arranged, plus an accent color. The color
//! theme decides every other color. Everything reads the current
//! colors through the `BG()`, `TEXT()`, … functions, so a change in Settings
//! shows on the next frame.
use std::sync::RwLock;

use ecolor::Color32;
use serde::{Deserialize, Serialize};

/// The color theme.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", from = "String")]
pub enum Mode {
    /// Dark or light, following the desktop.
    System,
    /// Dark gray, the default.
    #[default]
    Dark,
    Light,
    /// Pure black, for OLED screens.
    Black,
    /// Soft gray.
    Dim,
    /// Deep blue.
    Midnight,
}

/// Unknown names (from a newer Valo) fall back to the default.
impl From<String> for Mode {
    fn from(s: String) -> Self {
        match s.as_str() {
            "system" => Mode::System,
            "light" => Mode::Light,
            "black" => Mode::Black,
            "dim" => Mode::Dim,
            "midnight" => Mode::Midnight,
            _ => Mode::Dark,
        }
    }
}

impl Mode {
    pub const ALL: [(Mode, &'static str); 6] = [
        (Mode::System, "Same as the system"),
        (Mode::Dark, "Dark"),
        (Mode::Light, "Light"),
        (Mode::Black, "Black"),
        (Mode::Dim, "Dim"),
        (Mode::Midnight, "Midnight"),
    ];
}

/// The app style.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", from = "String")]
pub enum Skin {
    #[default]
    Valo,
    Netflix,
    Prime,
    Disney,
    Hbo,
    Youtube,
    Apple,
}

impl From<String> for Skin {
    fn from(s: String) -> Self {
        Skin::ALL
            .into_iter()
            .find(|k| {
                serde_json::to_value(k)
                    .ok()
                    .and_then(|v| v.as_str().map(|v| v == s))
                    == Some(true)
            })
            .unwrap_or_default()
    }
}

impl Skin {
    pub const ALL: [Skin; 7] = [
        Skin::Valo,
        Skin::Netflix,
        Skin::Prime,
        Skin::Disney,
        Skin::Hbo,
        Skin::Youtube,
        Skin::Apple,
    ];

    pub fn spec(self) -> &'static Spec {
        match self {
            Skin::Valo => &VALO,
            Skin::Netflix => &NETFLIX,
            Skin::Prime => &PRIME,
            Skin::Disney => &DISNEY,
            Skin::Hbo => &HBO,
            Skin::Youtube => &YOUTUBE,
            Skin::Apple => &APPLE,
        }
    }
}

/// How small labels, buttons and section headers are set.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Labels {
    /// Uppercase, letter-spaced monospace.
    Mono,
    /// Uppercase, letter-spaced sans.
    Caps,
    /// Sans, as written.
    Sans,
}

/// The top-bar tabs and the filter rows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tabs {
    /// A filled block behind the selected one.
    Block,
    /// Plain text, the selected one bold and bright.
    Plain,
    /// A line under the selected one.
    Underline,
    /// A rounded pill behind the selected one.
    Pill,
}

/// Where the player's play/pause and skip buttons sit.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PlayerLayout {
    /// Big, in the middle of the bottom, with the other buttons at the top.
    Center,
    /// One row under the seek bar, everything in it.
    Row,
}

/// The player's time display.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Times {
    /// Played on the left of the bar, "-left" on the right.
    Split,
    /// Only the time left, right of the bar.
    Left,
    /// "0:12 / 1:58:00" next to the buttons.
    Both,
}

/// Where the player shows the title.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TitleAt {
    TopLeft,
    TopCenter,
    /// In the button row (Netflix) or above the bar.
    Bottom,
}

/// The home page's big banner.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Hero {
    /// Artwork on the right, text on the left (Valo).
    Banner,
    /// Artwork across the whole width, text over its bottom left.
    FullBleed,
    /// A rounded banner with the next ones peeking in at the sides.
    Carousel,
    /// No banner (YouTube).
    None,
}

/// The shape of the cards in rows and grids.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tiles {
    Poster,
    /// 16:9, from the backdrop.
    Landscape,
}

/// Where a card's title goes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TileText {
    /// Under the artwork.
    Below,
    /// Over the bottom of the artwork.
    Over,
    /// Only while hovered, over the artwork (Prime Video).
    Hover,
    /// A channel-style line under the artwork: avatar, title, details (YouTube).
    Rich,
}

/// Home, Movies and TV shows: tabs in the top bar, or a sidebar.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Top,
    Side,
}

/// The movie and series pages.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Details {
    /// Poster beside the text over a dimmed backdrop (Valo).
    Valo,
    /// The backdrop across the top, title and buttons over its bottom left.
    Hero,
    /// Text on the left, the backdrop fading in on the right (Prime Video).
    Split,
    /// A watch page: big picture and description, a list beside it (YouTube).
    Watch,
}

pub struct Layout {
    pub hero: Hero,
    pub tiles: Tiles,
    pub text: TileText,
    pub nav: Nav,
    pub details: Details,
    /// A "Top 10" row with big numbers (Netflix).
    pub top10: bool,
    /// A row of genre tiles under the banner (Disney+).
    pub genre_tiles: bool,
    /// The home page is one grid with filter chips, not rows (YouTube).
    pub home_grid: bool,
    /// Episodes as a numbered list instead of cards.
    pub episode_list: bool,
}

pub struct PlayerLook {
    pub layout: PlayerLayout,
    /// The played part of the seek bar; `None`: the accent.
    pub played: Option<Color32>,
    /// The knob on the seek bar.
    pub knob: Option<Color32>,
    /// Seek bar height, resting and hovered.
    pub bar: (f32, f32),
    pub times: Times,
    pub title: TitleAt,
    /// A back arrow instead of a close cross.
    pub back_arrow: bool,
    /// Round dark buttons behind the controls (Apple, Prime).
    pub discs: bool,
}

pub struct Spec {
    pub name: &'static str,
    pub layout: Layout,
    /// Buttons, progress bars and highlights; everything else comes from
    /// the color theme.
    pub accent: Color32,
    pub labels: Labels,
    pub tabs: Tabs,
    /// Corner radius of artwork cards and of buttons.
    pub card_radius: u8,
    pub button_radius: u8,
    /// The main button is the accent color (else white, or black when light).
    pub primary_accent: bool,
    /// The word mark: colored with the accent, lowercase, with a play badge.
    pub logo_accent: bool,
    pub logo_lower: bool,
    pub logo_badge: bool,
    /// Hovered cards: outline width and how far they grow.
    pub hover_outline: f32,
    pub hover_grow: f32,
    /// Row titles: size, in the label style.
    pub row_title: f32,
    pub player: PlayerLook,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_hi: Color32,
    pub line: Color32,
    pub line_hi: Color32,
    pub accent: Color32,
    pub text: Color32,
    pub muted: Color32,
    /// Text on [`primary`](Self::primary).
    pub ink: Color32,
    /// The main button and the selected tab.
    pub primary: Color32,
    pub error: Color32,
    pub warn: Color32,
    pub ok: Color32,
    /// Text fields, code boxes.
    pub field: Color32,
    /// Panels on the details pages.
    pub card: Color32,
    pub light: bool,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// A dark palette from the few colors a style picks.
#[allow(clippy::too_many_arguments)]
const fn dark(
    bg: Color32,
    surface: Color32,
    surface_hi: Color32,
    line: Color32,
    line_hi: Color32,
    accent: Color32,
    text: Color32,
    muted: Color32,
) -> Palette {
    Palette {
        bg,
        surface,
        surface_hi,
        line,
        line_hi,
        accent,
        text,
        muted,
        ink: rgb(14, 15, 18),
        primary: Color32::WHITE,
        error: rgb(255, 120, 110),
        warn: rgb(242, 184, 75),
        ok: rgb(98, 200, 138),
        field: surface,
        card: surface,
        light: false,
    }
}

const VALO_DARK: Palette = Palette {
    field: rgb(22, 23, 27),
    card: rgb(26, 28, 34),
    ..dark(
        rgb(18, 19, 23),
        rgb(25, 26, 31),
        rgb(34, 35, 41),
        rgb(44, 45, 51),
        rgb(86, 88, 96),
        rgb(74, 124, 247),
        rgb(236, 236, 238),
        rgb(139, 141, 148),
    )
};

const NETFLIX_RED: Color32 = rgb(229, 9, 20);

const VALO: Spec = Spec {
    name: "Valo",
    layout: Layout {
        hero: Hero::Banner,
        tiles: Tiles::Poster,
        text: TileText::Below,
        nav: Nav::Top,
        details: Details::Valo,
        top10: false,
        genre_tiles: false,
        home_grid: false,
        episode_list: false,
    },
    accent: rgb(74, 124, 247),
    labels: Labels::Mono,
    tabs: Tabs::Block,
    card_radius: 6,
    button_radius: 2,
    primary_accent: false,
    logo_accent: false,
    logo_lower: false,
    logo_badge: false,
    hover_outline: 2.0,
    hover_grow: 4.0,
    row_title: 13.0,
    player: PlayerLook {
        layout: PlayerLayout::Center,
        played: Some(NETFLIX_RED),
        knob: Some(NETFLIX_RED),
        bar: (3.0, 5.0),
        times: Times::Split,
        title: TitleAt::TopLeft,
        back_arrow: false,
        discs: false,
    },
};

const NETFLIX: Spec = Spec {
    name: "Netflix",
    layout: Layout {
        hero: Hero::FullBleed,
        tiles: Tiles::Landscape,
        text: TileText::Over,
        nav: Nav::Top,
        details: Details::Hero,
        top10: true,
        genre_tiles: false,
        home_grid: false,
        episode_list: true,
    },
    accent: NETFLIX_RED,
    labels: Labels::Sans,
    tabs: Tabs::Plain,
    card_radius: 4,
    button_radius: 4,
    primary_accent: false,
    logo_accent: true,
    logo_lower: false,
    logo_badge: false,
    hover_outline: 0.0,
    hover_grow: 10.0,
    row_title: 20.0,
    player: PlayerLook {
        layout: PlayerLayout::Row,
        played: Some(NETFLIX_RED),
        knob: Some(NETFLIX_RED),
        bar: (4.0, 6.0),
        times: Times::Left,
        title: TitleAt::Bottom,
        back_arrow: true,
        discs: false,
    },
};

const PRIME_BLUE: Color32 = rgb(0, 168, 225);

const PRIME: Spec = Spec {
    name: "Prime Video",
    layout: Layout {
        hero: Hero::Carousel,
        tiles: Tiles::Landscape,
        text: TileText::Hover,
        nav: Nav::Top,
        details: Details::Split,
        top10: false,
        genre_tiles: false,
        home_grid: false,
        episode_list: true,
    },
    accent: PRIME_BLUE,
    labels: Labels::Sans,
    tabs: Tabs::Pill,
    card_radius: 8,
    button_radius: 8,
    primary_accent: true,
    logo_accent: false,
    logo_lower: true,
    logo_badge: false,
    hover_outline: 0.0,
    hover_grow: 8.0,
    row_title: 19.0,
    player: PlayerLook {
        layout: PlayerLayout::Center,
        played: Some(Color32::WHITE),
        knob: Some(Color32::WHITE),
        bar: (4.0, 6.0),
        times: Times::Split,
        title: TitleAt::TopLeft,
        back_arrow: false,
        discs: true,
    },
};

const DISNEY: Spec = Spec {
    name: "Disney+",
    layout: Layout {
        hero: Hero::Carousel,
        tiles: Tiles::Landscape,
        text: TileText::Over,
        nav: Nav::Top,
        details: Details::Hero,
        top10: false,
        genre_tiles: true,
        home_grid: false,
        episode_list: true,
    },
    accent: rgb(0, 99, 229),
    labels: Labels::Caps,
    tabs: Tabs::Underline,
    card_radius: 10,
    button_radius: 4,
    primary_accent: false,
    logo_accent: false,
    logo_lower: false,
    logo_badge: false,
    hover_outline: 3.0,
    hover_grow: 6.0,
    row_title: 19.0,
    player: PlayerLook {
        layout: PlayerLayout::Center,
        played: Some(rgb(249, 249, 249)),
        knob: Some(rgb(249, 249, 249)),
        bar: (4.0, 6.0),
        times: Times::Left,
        title: TitleAt::TopCenter,
        back_arrow: true,
        discs: false,
    },
};

const HBO: Spec = Spec {
    name: "HBO Max",
    layout: Layout {
        hero: Hero::FullBleed,
        tiles: Tiles::Landscape,
        text: TileText::Below,
        nav: Nav::Top,
        details: Details::Hero,
        top10: false,
        genre_tiles: false,
        home_grid: false,
        episode_list: true,
    },
    accent: rgb(153, 30, 235),
    labels: Labels::Sans,
    tabs: Tabs::Underline,
    card_radius: 8,
    button_radius: 22,
    primary_accent: false,
    logo_accent: false,
    logo_lower: false,
    logo_badge: false,
    hover_outline: 2.0,
    hover_grow: 4.0,
    row_title: 19.0,
    player: PlayerLook {
        layout: PlayerLayout::Row,
        played: None,
        knob: Some(Color32::WHITE),
        bar: (4.0, 6.0),
        times: Times::Split,
        title: TitleAt::TopLeft,
        back_arrow: true,
        discs: false,
    },
};

const YT_RED: Color32 = rgb(255, 0, 0);

const YOUTUBE: Spec = Spec {
    name: "YouTube",
    layout: Layout {
        hero: Hero::None,
        tiles: Tiles::Landscape,
        text: TileText::Rich,
        nav: Nav::Side,
        details: Details::Watch,
        top10: false,
        genre_tiles: false,
        home_grid: true,
        episode_list: false,
    },
    accent: YT_RED,
    labels: Labels::Sans,
    tabs: Tabs::Pill,
    card_radius: 12,
    button_radius: 18,
    primary_accent: false,
    logo_accent: false,
    logo_lower: false,
    logo_badge: true,
    hover_outline: 0.0,
    hover_grow: 0.0,
    row_title: 20.0,
    player: PlayerLook {
        layout: PlayerLayout::Row,
        played: Some(YT_RED),
        knob: Some(YT_RED),
        bar: (3.0, 5.0),
        times: Times::Both,
        title: TitleAt::TopLeft,
        back_arrow: true,
        discs: false,
    },
};

const APPLE: Spec = Spec {
    name: "Apple TV",
    layout: Layout {
        hero: Hero::FullBleed,
        tiles: Tiles::Landscape,
        text: TileText::Below,
        nav: Nav::Side,
        details: Details::Hero,
        top10: false,
        genre_tiles: false,
        home_grid: false,
        episode_list: false,
    },
    accent: rgb(10, 132, 255),
    labels: Labels::Sans,
    tabs: Tabs::Pill,
    card_radius: 12,
    button_radius: 12,
    primary_accent: false,
    logo_accent: false,
    logo_lower: true,
    logo_badge: false,
    hover_outline: 0.0,
    hover_grow: 8.0,
    row_title: 20.0,
    player: PlayerLook {
        layout: PlayerLayout::Center,
        played: Some(Color32::WHITE),
        knob: None,
        bar: (6.0, 9.0),
        times: Times::Split,
        title: TitleAt::Bottom,
        back_arrow: false,
        discs: true,
    },
};

/// The colors for `skin` in `mode` (`System` already resolved).
pub fn palette(skin: Skin, mode: Mode) -> Palette {
    let spec = skin.spec();
    // the color theme decides the colors; a style brings only its accent
    let base = Palette {
        accent: spec.accent,
        ..VALO_DARK
    };
    let mut p = match mode {
        Mode::System | Mode::Dark => base,
        Mode::Black => Palette {
            bg: Color32::BLACK,
            surface: rgb(14, 14, 16),
            surface_hi: rgb(30, 30, 34),
            line: rgb(36, 36, 40),
            line_hi: rgb(74, 74, 80),
            field: rgb(12, 12, 14),
            card: rgb(18, 18, 20),
            ..base
        },
        Mode::Dim => Palette {
            bg: rgb(40, 42, 48),
            surface: rgb(48, 50, 57),
            surface_hi: rgb(60, 62, 70),
            line: rgb(68, 70, 79),
            line_hi: rgb(108, 110, 120),
            field: rgb(44, 46, 52),
            card: rgb(50, 52, 60),
            muted: rgb(168, 170, 178),
            ..base
        },
        Mode::Midnight => Palette {
            bg: rgb(10, 15, 32),
            surface: rgb(17, 24, 46),
            surface_hi: rgb(27, 36, 64),
            line: rgb(35, 45, 76),
            line_hi: rgb(72, 86, 126),
            field: rgb(14, 20, 40),
            card: rgb(20, 28, 52),
            text: rgb(232, 236, 248),
            muted: rgb(138, 150, 182),
            ..base
        },
        Mode::Light => Palette {
            bg: rgb(246, 246, 248),
            surface: Color32::WHITE,
            surface_hi: rgb(232, 232, 237),
            line: rgb(220, 220, 226),
            line_hi: rgb(176, 176, 186),
            text: rgb(20, 20, 24),
            muted: rgb(104, 106, 114),
            ink: Color32::WHITE,
            primary: rgb(20, 20, 24),
            error: rgb(200, 40, 30),
            warn: rgb(170, 105, 0),
            ok: rgb(20, 130, 70),
            field: Color32::WHITE,
            card: Color32::WHITE,
            light: true,
            ..base
        },
    };
    if spec.primary_accent {
        p.primary = p.accent;
        p.ink = Color32::WHITE;
    }
    p
}

struct Current {
    skin: Skin,
    mode: Mode,
    pal: Palette,
}

static CURRENT: RwLock<Current> = RwLock::new(Current {
    skin: Skin::Valo,
    mode: Mode::Dark,
    pal: VALO_DARK,
});

/// Switch to `skin` in `mode` (resolved); false when that's already shown.
pub fn set(skin: Skin, mode: Mode) -> bool {
    let mut c = CURRENT.write().unwrap_or_else(|e| e.into_inner());
    if c.skin == skin && c.mode == mode {
        return false;
    }
    *c = Current {
        skin,
        mode,
        pal: palette(skin, mode),
    };
    true
}

pub fn pal() -> Palette {
    CURRENT.read().unwrap_or_else(|e| e.into_inner()).pal
}

pub fn skin() -> Skin {
    CURRENT.read().unwrap_or_else(|e| e.into_inner()).skin
}

pub fn spec() -> &'static Spec {
    skin().spec()
}

macro_rules! colors {
    ($($(#[$m:meta])* $name:ident => $field:ident),* $(,)?) => {
        $(
            $(#[$m])*
            #[allow(non_snake_case)]
            #[inline]
            pub fn $name() -> Color32 {
                pal().$field
            }
        )*
    };
}

colors!(
    BG => bg,
    SURFACE => surface,
    SURFACE_HI => surface_hi,
    LINE => line,
    LINE_HI => line_hi,
    ACCENT => accent,
    TEXT => text,
    MUTED => muted,
    /// Text on [`PRIMARY`] (selected tabs, main buttons).
    INK => ink,
    /// The main button and the selected tab: white on dark themes.
    PRIMARY => primary,
    ERROR => error,
    WARN => warn,
    OK => ok,
    FIELD => field,
    CARD => card,
);

/// `a` moved `t` (0–1) of the way to `b`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        m(a.r(), b.r()),
        m(a.g(), b.g()),
        m(a.b(), b.b()),
        m(a.a(), b.a()),
    )
}

/// The seek bar's played part.
pub fn played() -> Color32 {
    spec().player.played.unwrap_or_else(ACCENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_names_fall_back() {
        let s: Skin = serde_json::from_str("\"netflix\"").unwrap();
        assert_eq!(s, Skin::Netflix);
        let s: Skin = serde_json::from_str("\"tubi\"").unwrap();
        assert_eq!(s, Skin::Valo);
        let m: Mode = serde_json::from_str("\"sepia\"").unwrap();
        assert_eq!(m, Mode::Dark);
    }

    #[test]
    fn light_buttons_stay_visible() {
        for s in Skin::ALL {
            let p = palette(s, Mode::Light);
            assert_ne!(p.primary, p.bg, "{}", s.spec().name);
            assert_ne!(p.ink, p.primary, "{}", s.spec().name);
        }
    }
}
