//! Pieces the screens share, styled like the egui ones in `crate::ui`:
//! buttons, text fields, the centered card, labels and links.
use iced::alignment::Vertical;
use iced::widget::{
    button, column, container, mouse_area, row, scrollable, space, text, text_input, Button,
};
use iced::{Background, Border, Color, Element, Length, Padding, Theme};

use super::icon;
use super::spaced::{mono, Spaced};
use super::style::{self, c, BOLD};
use crate::ui::theme::{self, Skin};

/// A small label (field names, section headers) in the current style:
/// letter-spaced monospace capitals for Valo, plain sans for most others.
pub fn label(t: &str, size: f32) -> Spaced {
    mono(t, size).color(style::MUTED())
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

// ------------------------------------------------- the egui kit, ported

use crate::ui::theme::{Tabs, ACCENT, BG, INK, LINE, LINE_HI, PRIMARY, SURFACE_HI, TEXT};
use eframe::egui::Color32;

/// How a button is painted in the current style (egui's `ui::Kind`).
#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    /// White (or the accent): "Play", "Done".
    Primary,
    /// Translucent: "More info", "Start over".
    Secondary,
    /// Small and quiet: "Refresh", "Sign out".
    Small,
}

fn hot(s: button::Status) -> bool {
    matches!(s, button::Status::Hovered | button::Status::Pressed)
}

/// A button's look: fill, outline and text color, like egui's `paint_button`.
fn paint_button(kind: Kind, hovered: bool, enabled: bool, h: f32) -> button::Style {
    let skin = theme::skin();
    let pal = theme::pal();
    let rad = radius(h);
    let (fill, line, col): (Option<Color32>, Option<(Color32, f32)>, Color32) = match kind {
        Kind::Primary => {
            let fill = if !enabled {
                PRIMARY().gamma_multiply(0.45)
            } else if hovered {
                theme::mix(PRIMARY(), BG(), 0.14)
            } else {
                PRIMARY()
            };
            (Some(fill), None, INK())
        }
        Kind::Secondary if skin == Skin::Valo => {
            let base = (!pal.light).then(|| Color32::from_black_alpha(60));
            let fill = if hovered {
                Some(over(base, SURFACE_HI().gamma_multiply(0.6)))
            } else {
                base
            };
            (fill, Some((LINE_HI(), 1.0)), TEXT())
        }
        Kind::Secondary if skin == Skin::Disney => {
            let a = if hovered { 140 } else { 80 };
            (
                Some(Color32::from_black_alpha(a)),
                Some((TEXT(), 1.5)),
                TEXT(),
            )
        }
        Kind::Secondary => {
            let a = if hovered { 0.32 } else { 0.22 };
            let base = if pal.light { TEXT() } else { Color32::WHITE };
            (Some(base.gamma_multiply(a)), None, TEXT())
        }
        Kind::Small if skin == Skin::Valo => {
            (hovered.then(SURFACE_HI), Some((LINE_HI(), 1.0)), TEXT())
        }
        Kind::Small => {
            let fill = if hovered {
                theme::mix(SURFACE_HI(), TEXT(), 0.1)
            } else {
                SURFACE_HI()
            };
            (Some(fill), None, TEXT())
        }
    };
    button::Style {
        background: fill.map(|f| Background::Color(c(f))),
        text_color: c(col),
        border: Border {
            color: line.map_or(Color::TRANSPARENT, |l| c(l.0)),
            width: line.map_or(0.0, |l| l.1),
            radius: rad.into(),
        },
        ..button::Style::default()
    }
}

/// `top` painted over `base` (egui paints fills one after the other).
fn over(base: Option<Color32>, top: Color32) -> Color32 {
    let Some(b) = base else {
        return top;
    };
    let (b, t) = (c(b), c(top));
    let a = t.a + b.a * (1.0 - t.a);
    if a <= 0.0 {
        return top;
    }
    let mixc = |x: f32, y: f32| (y * t.a + x * b.a * (1.0 - t.a)) / a;
    Color32::from_rgba_unmultiplied(
        (mixc(b.r, t.r) * 255.0) as u8,
        (mixc(b.g, t.g) * 255.0) as u8,
        (mixc(b.b, t.b) * 255.0) as u8,
        (a * 255.0) as u8,
    )
}

/// A button in the current style with a label and an optional icon (before
/// the text, or after it for external links and right arrows). `None`
/// greys it out. Like egui's `primary_button`, `secondary_button`,
/// `small_button` and their `_icon` versions.
pub fn button_with<'a, M: Clone + 'a>(
    kind: Kind,
    t: &str,
    ic: Option<icon::Icon>,
    msg: Option<M>,
) -> Element<'a, M> {
    let enabled = msg.is_some();
    let (size, h, pad) = match kind {
        Kind::Small => (11.5, 30.0, if ic.is_some() { 10.0 } else { 12.0 }),
        _ => (12.5, 38.0, 18.0),
    };
    // the label's color follows the button's state, so it's set in the style
    let col = move |hovered: bool| {
        let s = paint_button(kind, hovered && enabled, enabled, h);
        if enabled {
            s.text_color
        } else {
            s.text_color
                .scale_alpha(if ic.is_some() { 0.4 } else { 0.5 })
        }
    };
    let content: Element<'a, M> = match ic {
        None => mono(t.trim(), size).into(),
        Some(i) => {
            let after = matches!(
                i,
                icon::Icon::External | icon::Icon::ChevronRight | icon::Icon::ArrowRight
            );
            let glyph: Element<'a, M> = icon::icon(i, col(false), 16.0).into();
            let label: Element<'a, M> = mono(t, size).into();
            if after {
                row![label, glyph]
                    .spacing(8)
                    .align_y(Vertical::Center)
                    .into()
            } else {
                row![glyph, label]
                    .spacing(8)
                    .align_y(Vertical::Center)
                    .into()
            }
        }
    };
    button(container(content).center_y(h))
        .height(h)
        .padding([0.0, pad])
        .style(move |_, s| {
            let mut st = paint_button(kind, hot(s) && enabled, enabled, h);
            st.text_color = col(hot(s));
            st
        })
        .on_press_maybe(msg)
        .into()
}

/// A [`button_with`] as wide as its parent and `h` high, the label centered
/// (egui's `add_sized` buttons in dialogs).
pub fn button_wide<'a, M: Clone + 'a>(
    kind: Kind,
    t: &str,
    h: f32,
    msg: Option<M>,
) -> Element<'a, M> {
    let enabled = msg.is_some();
    let size = if kind == Kind::Small { 11.5 } else { 12.5 };
    button(
        container(mono(t.trim(), size))
            .center_x(Length::Fill)
            .center_y(h),
    )
    .width(Length::Fill)
    .height(h)
    .padding(0)
    .style(move |_, s| {
        let mut st = paint_button(kind, hot(s) && enabled, enabled, h);
        if !enabled {
            st.text_color = st.text_color.scale_alpha(0.5);
        }
        st
    })
    .on_press_maybe(msg)
    .into()
}

/// A square 30px small button with a drawn icon (the "⋮" menu, close),
/// egui's `small_icon_button`.
pub fn small_icon_button<'a, M: Clone + 'a>(ic: icon::Icon, msg: M) -> Element<'a, M> {
    let col = style::TEXT();
    button(container(icon::icon(ic, col, 30.0)).center(30))
        .width(30)
        .height(30)
        .padding(0)
        .style(|_, s| paint_button(Kind::Small, hot(s), true, 30.0))
        .on_press(msg)
        .into()
}

/// A top-bar tab in the current style: a filled block, bold text, an
/// underline or a pill when selected (egui's `ui::tab`).
pub fn tab<'a, M: Clone + 'a>(t: &str, selected: bool, msg: M) -> Element<'a, M> {
    let spec = theme::spec();
    let label: Element<'a, M> = if spec.tabs == Tabs::Plain {
        super::spaced::Spaced::new(t, 14.5, 0.0, if selected { BOLD } else { style::SANS }).into()
    } else {
        mono(t, 12.0).into()
    };
    let pill = spec.tabs == Tabs::Pill;
    let (padx, h) = if pill { (13.0, 30.0) } else { (8.0, 28.0) };
    let underline = spec.tabs == Tabs::Underline;
    let inner: Element<'a, M> = if underline {
        let bar = container(space())
            .height(2)
            .width(Length::Fill)
            .padding([0, 6]);
        column![container(label).center_y(h - 2.0), bar].into()
    } else {
        container(label).center_y(h).into()
    };
    button(inner)
        .height(h)
        .padding([0.0, padx])
        .style(move |_, s| tab_style(selected, hot(s), false, h))
        .on_press(msg)
        .into()
}

/// A tab's look (egui's `paint_tab`). `chip`: a filter chip, not a top tab.
fn tab_style(selected: bool, hovered: bool, chip: bool, h: f32) -> button::Style {
    let spec = theme::spec();
    let dim = TEXT().gamma_multiply(0.72);
    let (fill, col, rad) = match spec.tabs {
        Tabs::Block => {
            let rad = if chip { 0.0 } else { 1.0 };
            if selected {
                (Some(PRIMARY()), INK(), rad)
            } else {
                (hovered.then(SURFACE_HI), TEXT(), rad)
            }
        }
        Tabs::Plain | Tabs::Underline if !chip => {
            (None, if selected || hovered { TEXT() } else { dim }, 0.0)
        }
        _ => {
            let rad = f32::from(spec.button_radius).min(h / 2.0);
            if selected {
                (Some(PRIMARY()), INK(), rad)
            } else {
                let quiet = theme::skin() == Skin::Youtube || chip;
                let fill = if hovered {
                    Some(theme::mix(SURFACE_HI(), TEXT(), 0.1))
                } else if quiet {
                    Some(SURFACE_HI())
                } else {
                    None
                };
                (fill, TEXT(), rad)
            }
        }
    };
    button::Style {
        background: fill.map(|f| Background::Color(c(f))),
        text_color: c(col),
        border: iced::border::rounded(rad),
        ..button::Style::default()
    }
}

/// A row of options with one chosen (egui's `ui::segmented`): a bordered
/// strip for Valo, chips for the other styles. Each item: its label, a
/// count, whether it's chosen, and its message.
pub fn segmented<'a, M: Clone + 'a>(
    items: Vec<(String, Option<usize>, bool, M)>,
) -> Element<'a, M> {
    let block = theme::spec().tabs == Tabs::Block;
    let mut r = row![].spacing(if block { 0 } else { 8 });
    for (i, (label, count, sel, msg)) in items.into_iter().enumerate() {
        let on_fill = sel && matches!(theme::spec().tabs, Tabs::Block | Tabs::Pill);
        let count_col = if on_fill {
            c(INK().gamma_multiply(0.6))
        } else {
            style::MUTED()
        };
        let mut content = row![mono(&label, 12.0)].align_y(Vertical::Center);
        if let Some(n) = count {
            content = content
                .push(space().width(9))
                .push(mono(&n.to_string(), 12.0).color(count_col));
        }
        if block && i > 0 {
            r = r.push(vline(34.0));
        }
        let b = button(container(content).center_y(34))
            .height(34)
            .padding([0, 12])
            .style(move |_, s| tab_style(sel, hot(s), true, 34.0));
        r = r.push(if sel { b } else { b.on_press(msg) });
    }
    if !block {
        return r.into();
    }
    // the strip's outline, over the cells' fills
    iced::widget::stack![
        r,
        container(space())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                border: Border {
                    color: c(LINE()),
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            }),
    ]
    .into()
}

/// A one-pixel vertical line in the theme's line color.
pub fn vline<'a, M: 'a>(h: f32) -> Element<'a, M> {
    container(space())
        .width(1)
        .height(h)
        .style(|_| container::Style {
            background: Some(Background::Color(c(LINE()))),
            ..container::Style::default()
        })
        .into()
}

/// A full-width row in a drop-down menu (egui's `ui::menu_item`).
pub fn menu_item<'a, M: Clone + 'a>(t: &str, msg: M) -> Element<'a, M> {
    button(container(text(t.to_owned()).size(14.5)).center_y(32))
        .width(Length::Fill)
        .height(32)
        .padding([0, 10])
        .style(|_, s| button::Style {
            background: hot(s).then(|| Background::Color(style::SURFACE_HI())),
            text_color: style::TEXT(),
            border: iced::border::rounded(2),
            ..button::Style::default()
        })
        .on_press(msg)
        .into()
}

/// The word mark in the title bar: the server's name, or the app's (egui's
/// `ui::logo`).
pub fn logo<'a, M: 'a>(t: &str, size: f32) -> Element<'a, M> {
    let spec = theme::spec();
    let mut t = if spec.logo_lower {
        t.to_lowercase()
    } else {
        t.to_uppercase()
    };
    if t.chars().count() > 28 {
        t = t.chars().take(27).collect::<String>() + "…";
    }
    let (font, spacing) = if spec.logo_lower || spec.logo_badge {
        (BOLD, -0.3)
    } else {
        (style::BLACK, size * 0.04)
    };
    let col = if spec.logo_accent {
        c(ACCENT())
    } else {
        style::TEXT()
    };
    let word = super::spaced::Spaced::new(t, size, spacing, font).color(col);
    if !spec.logo_badge {
        return word.into();
    }
    // a rounded badge with a play triangle in front of the name
    let badge = container(icon::icon(icon::Icon::Play, Color::WHITE, size * 0.7))
        .center_x(size * 1.45)
        .center_y(size)
        .style(move |_| container::Style {
            background: Some(Background::Color(c(ACCENT()))),
            border: iced::border::rounded(size * 0.28),
            ..container::Style::default()
        });
    row![badge, word]
        .spacing(4)
        .align_y(Vertical::Center)
        .into()
}
