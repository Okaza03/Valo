//! Pieces the screens share, styled like the egui ones in `crate::ui`:
//! buttons, text fields, the centered card, labels and links.
use iced::alignment::Vertical;
use iced::widget::{
    button, column, container, mouse_area, row, scrollable, space, text, text_input, Button,
};
use iced::{Background, Border, Color, Element, Length, Padding, Theme};

use super::icon;
use super::style::{self, c, BOLD, MONO, SANS};
use crate::ui::theme::{self, Labels, Skin};

/// A small label (field names, section headers) in the current style:
/// letter-spaced monospace capitals for Valo, plain sans for most others.
pub fn label<'a>(t: &str, size: f32) -> iced::widget::Text<'a> {
    let (font, size, upper) = match theme::spec().labels {
        Labels::Mono => (MONO, size, true),
        Labels::Caps => (BOLD, size * 1.08, true),
        Labels::Sans => (if size >= 12.0 { BOLD } else { SANS }, size * 1.14, false),
    };
    let t = if upper {
        t.to_uppercase()
    } else {
        t.to_owned()
    };
    text(t).font(font).size(size).color(style::MUTED())
}

/// A section header inside a card.
pub fn section<'a, M: 'a>(t: &str) -> Element<'a, M> {
    container(label(t, 11.5))
        .padding(Padding::default().top(10.0).bottom(2.0))
        .into()
}

fn radius(h: f32) -> f32 {
    f32::from(theme::spec().button_radius).min(h / 2.0)
}

/// The main button ("Connect", "Sign in"), as wide as its parent.
pub fn primary<'a, M: Clone + 'a>(t: &str, on: Option<M>) -> Button<'a, M> {
    let enabled = on.is_some();
    button(
        container(text(t.trim().to_owned()).font(BOLD).size(13.5))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fill)
    .height(42)
    .padding(0)
    .style(move |_, s| {
        let p = theme::pal();
        let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
        let fill = if !enabled {
            c(p.primary).scale_alpha(0.45)
        } else if hot {
            c(theme::mix(p.primary, p.bg, 0.14))
        } else {
            c(p.primary)
        };
        button::Style {
            background: Some(Background::Color(fill)),
            text_color: if enabled {
                c(p.ink)
            } else {
                c(p.ink).scale_alpha(0.5)
            },
            border: iced::border::rounded(radius(42.0)),
            ..button::Style::default()
        }
    })
    .on_press_maybe(on)
}

/// A small, quiet button ("Open file", "Copy report").
pub fn small<'a, M: Clone + 'a>(t: &str, msg: M) -> Button<'a, M> {
    button(text(t.to_owned()).size(12.5))
        .padding([6, 12])
        .height(30)
        .style(|_, s| {
            let p = theme::pal();
            let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
            let valo = theme::skin() == Skin::Valo;
            let fill = match (valo, hot) {
                (true, true) => Some(c(p.surface_hi)),
                (true, false) => None,
                (false, true) => Some(c(theme::mix(p.surface_hi, p.text, 0.1))),
                (false, false) => Some(c(p.surface_hi)),
            };
            button::Style {
                background: fill.map(Background::Color),
                text_color: c(p.text),
                border: Border {
                    color: if valo {
                        c(p.line_hi)
                    } else {
                        Color::TRANSPARENT
                    },
                    width: if valo { 1.0 } else { 0.0 },
                    radius: radius(30.0).into(),
                },
                ..button::Style::default()
            }
        })
        .on_press(msg)
}

/// Text that acts as a link: the accent color, a hand on hover.
pub fn link<'a, M: Clone + 'a>(t: &str, msg: M) -> Element<'a, M> {
    mouse_area(text(t.to_owned()).size(14).color(c(theme::pal().accent)))
        .interaction(iced::mouse::Interaction::Pointer)
        .on_press(msg)
        .into()
}

/// A one-line text field.
pub fn field<'a, M: Clone + 'a>(
    id: &'static str,
    value: &str,
    hint: &str,
    secure: bool,
    on_input: Option<fn(String) -> M>,
    on_submit: Option<M>,
) -> Element<'a, M> {
    let mut f = text_input(hint, value)
        .id(id)
        .secure(secure)
        .size(15)
        .padding([9, 10])
        .style(|_, s| {
            let p = theme::pal();
            let border = match s {
                text_input::Status::Focused { .. } => c(p.text),
                text_input::Status::Hovered => c(p.line_hi),
                _ => c(p.line),
            };
            text_input::Style {
                background: Background::Color(c(p.field)),
                border: Border {
                    color: border,
                    width: 1.0,
                    radius: 3.0.into(),
                },
                icon: c(p.muted),
                placeholder: c(p.muted).scale_alpha(0.7),
                value: c(p.text),
                selection: c(p.accent).scale_alpha(0.4),
            }
        });
    if let Some(on_input) = on_input {
        f = f.on_input(on_input);
    }
    if let Some(m) = on_submit {
        f = f.on_submit(m);
    }
    f.into()
}

/// A spinner and a short line of what's happening.
pub fn spinner_text<'a, M: 'a>(t: &str, secs: f32) -> Element<'a, M> {
    row![icon::spinner(secs, style::TEXT(), 16.0), label(t, 11.5),]
        .spacing(8)
        .align_y(Vertical::Center)
        .into()
}

/// The centered card the setup, check and sign-in screens sit in: in the
/// middle of the window (`height` high), scrolling when it's taller.
pub fn card<'a, M: 'a>(height: f32, content: impl Into<Element<'a, M>>) -> Element<'a, M> {
    let panel = container(content)
        .padding(28)
        .max_width(440)
        .width(Length::Fill)
        .style(|_: &Theme| {
            let p = theme::pal();
            container::Style {
                background: Some(Background::Color(c(p.surface))),
                border: Border {
                    color: c(p.line),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                text_color: Some(c(p.text)),
                ..container::Style::default()
            }
        });
    // like the egui card: a 540-high card would sit in the middle
    let top = ((height - 540.0) / 2.0).max(24.0);
    scrollable(
        container(
            column![space().height(top), panel, space().height(24)]
                .align_x(iced::alignment::Horizontal::Center),
        )
        .center_x(Length::Fill)
        .padding([0, 16]),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Under an error that only the server's admin can fix: what's missing,
/// a message to send them, and the API docs.
pub fn admin_help<'a, M: Clone + 'a>(
    missing: crate::api::Missing,
    copied: bool,
    copy: M,
    docs: M,
) -> Element<'a, M> {
    let copy_label = if copied {
        "Copied, paste it to the admin"
    } else {
        "Copy message for the admin"
    };
    column![
        text(missing.hint()).size(12.5).color(style::MUTED()),
        row![small(copy_label, copy), link("API docs", docs)]
            .spacing(12)
            .align_y(Vertical::Center),
    ]
    .spacing(6)
    .padding(Padding::default().top(4.0))
    .into()
}
