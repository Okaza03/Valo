//! The admin page's look, after shadcn/ui's dashboard blocks: rounded cards
//! with a title and a description, stat cards, pill tabs, outline and solid
//! buttons, and badges. Sentence-case sans text in place of the spaced
//! capitals the rest of the app uses, so the admin reads like a dashboard.
use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, space, text, Column};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::iced_ui::icon;
use crate::iced_ui::style::{self, black, c, BOLD};
use crate::iced_ui::widgets::Kind as Btn;
use crate::ui::browse::details::TEAL;
use crate::ui::theme::{CARD, ERROR, INK, LINE, LINE_HI, PRIMARY, SURFACE_HI};

/// Corner radius of cards; controls are a little rounder than square.
const CARD_R: f32 = 12.0;
const CTL_R: f32 = 8.0;

fn hot(s: button::Status) -> bool {
    matches!(s, button::Status::Hovered | button::Status::Pressed)
}

/// A card: the theme's card color, a hairline border and a soft shadow.
pub fn card_style(_: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(c(CARD()))),
        border: Border {
            color: c(LINE()),
            width: 1.0,
            radius: CARD_R.into(),
        },
        shadow: Shadow {
            color: black(50),
            offset: Vector::new(0.0, 1.0),
            blur_radius: 3.0,
        },
        ..container::Style::default()
    }
}

/// A card around `content`, as wide as its parent.
pub fn panel<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> Element<'a, M> {
    container(content)
        .padding([20, 22])
        .width(Length::Fill)
        .style(card_style)
        .into()
}

/// A card's or section's title. The teal the app's captions use becomes
/// plain text (shadcn's card title); other colors (a warning) stay.
pub fn caps<'a, M: 'a>(t: &str, color: Color) -> Element<'a, M> {
    let col = if color == c(TEAL()) {
        style::TEXT()
    } else {
        color
    };
    text(t.to_owned()).font(BOLD).size(15).color(col).into()
}

/// The muted line under a title.
pub fn description<'a, M: 'a>(t: &str) -> Element<'a, M> {
    text(t.to_owned()).size(13.5).color(style::MUTED()).into()
}

/// A small rounded label in `tone` (a count, a state).
pub fn badge<'a, M: 'a>(t: &str, tone: Color) -> Element<'a, M> {
    container(text(t.to_owned()).size(11.5).color(tone))
        .padding([1, 8])
        .style(move |_| container::Style {
            background: Some(Background::Color(tone.scale_alpha(0.12))),
            border: Border {
                color: tone.scale_alpha(0.35),
                width: 1.0,
                radius: 999.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// A section's heading: a muted kicker, the title and a count badge.
pub fn section_title<'a, M: 'a>(kicker: &str, title: &str, n: usize) -> Element<'a, M> {
    column![
        text(kicker.to_owned()).size(13).color(style::MUTED()),
        space().height(2),
        row![
            text(title.to_owned()).font(BOLD).size(22),
            badge(&n.to_string(), style::MUTED()),
        ]
        .spacing(10)
        .align_y(Vertical::Center),
        space().height(20),
    ]
    .into()
}

/// Tabs in a muted pill; the open one is a raised card.
pub fn segmented<'a, M: Clone + 'a>(
    items: Vec<(String, Option<usize>, bool, M)>,
) -> Element<'a, M> {
    let mut r = row![].spacing(2);
    for (label, count, sel, msg) in items {
        let mut content = row![text(label).size(13.5)].align_y(Vertical::Center);
        if let Some(n) = count {
            content = content
                .push(space().width(8))
                .push(text(n.to_string()).size(12).color(style::MUTED()));
        }
        let b = button(container(content).center_y(30))
            .height(30)
            .padding([0, 12])
            .style(move |_, s| button::Style {
                background: sel.then(|| Background::Color(c(CARD()))),
                text_color: if sel || hot(s) {
                    style::TEXT()
                } else {
                    style::MUTED()
                },
                border: Border {
                    color: if sel { c(LINE()) } else { Color::TRANSPARENT },
                    width: 1.0,
                    radius: 6.0.into(),
                },
                shadow: if sel {
                    Shadow {
                        color: black(60),
                        offset: Vector::new(0.0, 1.0),
                        blur_radius: 2.0,
                    }
                } else {
                    Shadow::default()
                },
                ..button::Style::default()
            });
        r = r.push(if sel { b } else { b.on_press(msg) });
    }
    container(r)
        .padding(3)
        .style(|_| container::Style {
            background: Some(Background::Color(c(SURFACE_HI()))),
            border: iced::border::rounded(CTL_R + 2.0),
            ..container::Style::default()
        })
        .into()
}

/// A button: `Primary` is solid, `Secondary` muted, `Small` an outline.
/// `None` greys it out.
pub fn button_with<'a, M: Clone + 'a>(
    kind: Btn,
    t: &str,
    ic: Option<icon::Icon>,
    msg: Option<M>,
) -> Element<'a, M> {
    let enabled = msg.is_some();
    let h = if kind == Btn::Small { 32.0 } else { 36.0 };
    let paint = move |hovered: bool| -> (Option<Background>, Color, Color) {
        let (bg, fg, border) = match kind {
            Btn::Primary => (
                Some(c(PRIMARY()).scale_alpha(if hovered { 0.88 } else { 1.0 })),
                c(INK()),
                Color::TRANSPARENT,
            ),
            Btn::Secondary => (
                Some(c(SURFACE_HI()).scale_alpha(if hovered { 0.8 } else { 1.0 })),
                style::TEXT(),
                Color::TRANSPARENT,
            ),
            _ => (
                hovered.then(|| c(SURFACE_HI())),
                style::TEXT(),
                c(if hovered { LINE_HI() } else { LINE() }),
            ),
        };
        let fade = if enabled { 1.0 } else { 0.5 };
        (
            bg.map(|b| Background::Color(b.scale_alpha(fade))),
            fg.scale_alpha(fade),
            border,
        )
    };
    let fg = paint(false).1;
    let label = text(t.trim().to_owned()).size(13.5);
    let content: Element<'a, M> = match ic {
        None => label.into(),
        Some(i) => {
            let after = matches!(
                i,
                icon::Icon::External | icon::Icon::ChevronRight | icon::Icon::ArrowRight
            );
            let glyph: Element<'a, M> = icon::icon(i, fg, 15.0);
            if after {
                row![label, glyph]
                    .spacing(6)
                    .align_y(Vertical::Center)
                    .into()
            } else {
                row![glyph, label]
                    .spacing(6)
                    .align_y(Vertical::Center)
                    .into()
            }
        }
    };
    button(container(content).center_y(h))
        .height(h)
        .padding([0.0, if kind == Btn::Small { 12.0 } else { 16.0 }])
        .style(move |_, s| {
            let (bg, fg, border) = paint(hot(s) && enabled);
            button::Style {
                background: bg,
                text_color: fg,
                border: Border {
                    color: border,
                    width: 1.0,
                    radius: CTL_R.into(),
                },
                ..button::Style::default()
            }
        })
        .on_press_maybe(msg)
        .into()
}

/// A red outline button for what can't be undone.
pub fn danger<'a, M: Clone + 'a>(t: &str, msg: Option<M>) -> Element<'a, M> {
    let err = c(ERROR());
    let fade = if msg.is_some() { 1.0 } else { 0.5 };
    button(container(text(t.to_owned()).size(13.5).color(err.scale_alpha(fade))).center_y(32))
        .height(32)
        .padding([0, 12])
        .style(move |_, s| button::Style {
            background: hot(s).then(|| Background::Color(err.scale_alpha(0.12))),
            border: Border {
                color: err.scale_alpha(0.5 * fade),
                width: 1.0,
                radius: CTL_R.into(),
            },
            ..button::Style::default()
        })
        .on_press_maybe(msg)
        .into()
}

/// A stat card: what it counts on top, the number under it (shadcn's
/// dashboard "section cards").
fn stat<'a, M: 'a>(w: f32, value: String, label: &str) -> Element<'a, M> {
    container(
        column![
            text(label.to_owned()).size(13.5).color(style::MUTED()),
            space::vertical(),
            text(value).font(BOLD).size(30),
        ]
        .height(Length::Fill),
    )
    .width(w)
    .height(104)
    .padding(Padding {
        top: 16.0,
        right: 20.0,
        bottom: 16.0,
        left: 20.0,
    })
    .style(card_style)
    .into()
}

/// Stat cards in rows, as many across as fit `w`.
pub fn stats<'a, M: 'a>(tiles: Vec<(String, &str)>, w: f32) -> Element<'a, M> {
    let g = 14.0;
    let n = tiles.len().max(1);
    let fit = ((w + g) / (150.0 + g)).floor().clamp(1.0, n as f32) as usize;
    // rows of even length: 6 tiles where 5 fit are two rows of 3, not 5 + 1
    let cols = n.div_ceil(n.div_ceil(fit));
    let tw = (w - g * (cols as f32 - 1.0)) / cols as f32;
    let mut out: Column<'a, M> = column![].spacing(g);
    let mut it = tiles.into_iter().peekable();
    while it.peek().is_some() {
        let mut r = row![].spacing(g);
        for (v, l) in it.by_ref().take(cols) {
            r = r.push(stat(tw, v, l));
        }
        out = out.push(r);
    }
    out.into()
}

/// A bordered row in a list (users, requests): a card of its own.
pub fn row_frame<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> Element<'a, M> {
    container(content)
        .width(Length::Fill)
        .padding([12, 16])
        .style(card_style)
        .into()
}

/// Small text where the rest of the app uses spaced capitals (years,
/// counts, empty states): plain and a touch larger, so it reads at a glance.
pub fn mono<'a>(t: &str, size: f32) -> iced::widget::Text<'a> {
    text(t.to_owned()).size(size + 2.0)
}

/// A form field's label.
pub fn label<'a>(t: &str, size: f32) -> iced::widget::Text<'a> {
    text(t.to_owned()).size(size + 2.0).color(style::MUTED())
}

/// A [`button_with`] as wide as its parent and `h` high, the label centered.
pub fn button_wide<'a, M: Clone + 'a>(
    kind: Btn,
    t: &str,
    h: f32,
    msg: Option<M>,
) -> Element<'a, M> {
    let enabled = msg.is_some();
    let fade = if enabled { 1.0 } else { 0.5 };
    let primary = kind == Btn::Primary;
    button(
        container(text(t.trim().to_owned()).size(13.5))
            .center_x(Length::Fill)
            .center_y(h),
    )
    .width(Length::Fill)
    .height(h)
    .padding(0)
    .style(move |_, s| {
        let on = hot(s) && enabled;
        button::Style {
            background: if primary {
                Some(Background::Color(
                    c(PRIMARY()).scale_alpha(fade * if on { 0.88 } else { 1.0 }),
                ))
            } else {
                on.then(|| Background::Color(c(SURFACE_HI())))
            },
            text_color: if primary { c(INK()) } else { style::TEXT() }.scale_alpha(fade),
            border: Border {
                color: if primary {
                    Color::TRANSPARENT
                } else {
                    c(LINE())
                },
                width: 1.0,
                radius: CTL_R.into(),
            },
            ..button::Style::default()
        }
    })
    .on_press_maybe(msg)
    .into()
}

/// The chart and bar color: the theme's accent (shadcn's chart-1).
pub fn accent() -> Color {
    c(crate::ui::theme::ACCENT())
}

/// A labelled bar `w` wide: the label and value over a thin track, filled
/// `f` (0–1) in the accent color (shadcn's progress).
pub fn bar<'a, M: 'a>(label: &str, value: String, f: f32, w: f32) -> Element<'a, M> {
    let fill = accent();
    column![
        row![
            text(label.to_owned()).size(14),
            space::horizontal(),
            text(value).size(12.5).color(style::MUTED()),
        ],
        space().height(6),
        container(
            container(space())
                .width((w * f.clamp(0.0, 1.0)).max(if f > 0.0 { 3.0 } else { 0.0 }))
                .height(6)
                .style(move |_| container::Style {
                    background: Some(Background::Color(fill)),
                    border: iced::border::rounded(3),
                    ..container::Style::default()
                }),
        )
        .width(w)
        .height(6)
        .style(|_| container::Style {
            background: Some(Background::Color(c(SURFACE_HI()))),
            border: iced::border::rounded(3),
            ..container::Style::default()
        }),
    ]
    .width(w)
    .into()
}
