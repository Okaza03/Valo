//! The shared look: fonts, the theme's colors as Iced colors, and the styles
//! the screens reuse. Colors come from `crate::ui::theme`.
use iced::widget::{button, container};
use iced::{border, font, Background, Border, Color, Font, Theme};

use crate::ui::theme;

pub const SANS: Font = Font::with_name("Noto Sans");
pub const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..SANS
};
pub const BLACK: Font = Font {
    weight: font::Weight::Black,
    ..SANS
};
pub const MONO: Font = Font {
    weight: font::Weight::Medium,
    ..Font::with_name("Noto Sans Mono")
};

pub const FONTS: [&[u8]; 4] = [
    include_bytes!("../../assets/fonts/NotoSans-Regular.ttf"),
    include_bytes!("../../assets/fonts/NotoSans-Bold.ttf"),
    include_bytes!("../../assets/fonts/NotoSans-Black.ttf"),
    include_bytes!("../../assets/fonts/NotoSansMono-Medium.ttf"),
];

/// A style color (`ecolor`, as the app styles define them) as an Iced one.
pub fn c(e: ecolor::Color32) -> Color {
    let [r, g, b, a] = e.to_srgba_unmultiplied();
    Color::from_rgba8(r, g, b, f32::from(a) / 255.0)
}

pub fn gray(v: u8) -> Color {
    Color::from_rgb8(v, v, v)
}

/// White at `a` (0–255) opacity.
pub fn white(a: u8) -> Color {
    Color::from_rgba8(255, 255, 255, f32::from(a) / 255.0)
}

/// Black at `a` (0–255) opacity.
pub fn black(a: u8) -> Color {
    Color::from_rgba8(0, 0, 0, f32::from(a) / 255.0)
}

#[allow(non_snake_case)]
pub fn BG() -> Color {
    c(theme::BG())
}
#[allow(non_snake_case)]
pub fn TEXT() -> Color {
    c(theme::TEXT())
}
#[allow(non_snake_case)]
pub fn MUTED() -> Color {
    c(theme::MUTED())
}
#[allow(non_snake_case)]
pub fn LINE() -> Color {
    c(theme::LINE())
}
#[allow(non_snake_case)]
pub fn SURFACE_HI() -> Color {
    c(theme::SURFACE_HI())
}
#[allow(non_snake_case)]
pub fn ERROR() -> Color {
    c(theme::ERROR())
}

/// The Iced theme built from the current palette.
pub fn iced_theme() -> Theme {
    let p = theme::pal();
    Theme::custom(
        "Valo".to_owned(),
        iced::theme::Palette {
            background: c(p.bg),
            text: c(p.text),
            primary: c(p.accent),
            success: c(p.ok),
            warning: c(p.warn),
            danger: c(p.error),
        },
    )
}

/// A button with no frame: `hover` fills it while the pointer is over it.
pub fn bare(hover: Color, radius: f32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(hover)),
        text_color: TEXT(),
        border: border::rounded(radius),
        ..button::Style::default()
    }
}

/// A round button over the video: lit on hover, or always on a frosted disc.
pub fn ctl(disc: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let a = match (disc, hot) {
            (true, true) => Some(60),
            (true, false) => Some(36),
            (false, true) => Some(28),
            (false, false) => None,
        };
        button::Style {
            background: a.map(|a| Background::Color(white(a))),
            text_color: Color::WHITE,
            border: border::rounded(100),
            ..button::Style::default()
        }
    }
}

/// The dark panels the player's menus and volume slider sit on.
pub fn popup(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(28, 28, 31, 0.96))),
        border: Border {
            color: gray(48),
            width: 1.0,
            radius: 8.0.into(),
        },
        text_color: Some(gray(200)),
        ..container::Style::default()
    }
}

/// A hover tip.
pub fn tip(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(24, 24, 27, 0.97))),
        border: Border {
            color: gray(60),
            width: 1.0,
            radius: 5.0.into(),
        },
        text_color: Some(gray(225)),
        ..container::Style::default()
    }
}
