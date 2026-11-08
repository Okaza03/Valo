//! Window chrome: our own title bar (the window has no system decorations),
//! edge resizing, and the hairline border.
use iced::alignment::Vertical;
use iced::widget::{button, container, mouse_area, pin, row, space, stack, text, tooltip};
use iced::window::Direction;
use iced::{mouse, Background, Border, Color, Element, Length, Padding, Size};

use super::icon::{self, Icon};
use super::style::{self, BLACK};
use super::Message;

pub const BAR_H: f32 = 46.0;

/// Top bar: word mark, `left` (tabs), `right` (buttons), and the window buttons.
pub fn title_bar<'a>(
    name: &str,
    maximized: bool,
    left: Element<'a, Message>,
    right: Element<'a, Message>,
) -> Element<'a, Message> {
    let bar = row![
        text(name.to_owned())
            .font(BLACK)
            .size(19)
            .color(style::TEXT()),
        space().width(18),
        left,
        space::horizontal(),
        right,
        vsep(),
        window_buttons(maximized),
    ]
    .align_y(Vertical::Center)
    .height(BAR_H);
    mouse_area(
        container(bar)
            .padding(Padding::default().left(18.0))
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(style::BG())),
                ..container::Style::default()
            }),
    )
    .on_press(Message::Drag)
    .on_double_click(Message::ToggleMaximize)
    .into()
}

/// A full-height vertical divider.
pub fn vsep<'a>() -> Element<'a, Message> {
    if crate::ui::theme::skin() != crate::ui::theme::Skin::Valo {
        return space().width(1).into(); // the streaming styles don't rule off their top bar
    }
    container(space())
        .width(1)
        .height(BAR_H)
        .style(|_| container::Style {
            background: Some(Background::Color(style::LINE())),
            ..container::Style::default()
        })
        .into()
}

/// Minimize, maximize and close.
fn window_buttons<'a>(maximized: bool) -> Element<'a, Message> {
    row![
        bar_button(Icon::Minimize, "Minimize", Message::Minimize),
        if maximized {
            bar_button(Icon::Restore, "Restore", Message::ToggleMaximize)
        } else {
            bar_button(Icon::Maximize, "Maximize", Message::ToggleMaximize)
        },
        bar_button(Icon::Close, "Close", Message::Close),
    ]
    .into()
}

/// A square bar button with a drawn icon.
pub fn bar_button<'a>(ic: Icon, tip: &str, msg: Message) -> Element<'a, Message> {
    let close = matches!(ic, Icon::Close);
    let b = button(icon::icon(ic, style::TEXT(), BAR_H))
        .padding(0)
        .width(BAR_H)
        .height(BAR_H)
        .style(move |_, s| {
            let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: hot.then(|| {
                    Background::Color(if close {
                        Color::from_rgb8(196, 43, 28)
                    } else {
                        style::SURFACE_HI()
                    })
                }),
                ..button::Style::default()
            }
        })
        .on_press(msg);
    tooltip(
        b,
        container(text(tip.to_owned()).size(12.5))
            .padding([4, 8])
            .style(style::tip),
        tooltip::Position::Bottom,
    )
    .into()
}

/// Resize handles along the window's edges and a hairline border, over `content`.
/// Maximized and fullscreen windows get neither.
pub fn frame<'a>(content: Element<'a, Message>, size: Size, framed: bool) -> Element<'a, Message> {
    if !framed {
        return content;
    }
    let (w, h, m) = (size.width, size.height, 5.0);
    let edge = |x: f32, y: f32, ew: f32, eh: f32, dir: Direction, cursor| -> Element<'a, Message> {
        pin(mouse_area(space().width(ew).height(eh))
            .interaction(cursor)
            .on_press(Message::Resize(dir)))
        .x(x)
        .y(y)
        .into()
    };
    use mouse::Interaction::{
        ResizingDiagonallyDown as Diag, ResizingDiagonallyUp as Anti,
        ResizingHorizontally as Horiz, ResizingVertically as Vert,
    };
    let border = container(space())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            border: Border {
                color: style::LINE(),
                width: 1.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        });
    stack![
        content,
        border,
        edge(m, 0.0, w - 2.0 * m, m, Direction::North, Vert),
        edge(m, h - m, w - 2.0 * m, m, Direction::South, Vert),
        edge(0.0, m, m, h - 2.0 * m, Direction::West, Horiz),
        edge(w - m, m, m, h - 2.0 * m, Direction::East, Horiz),
        edge(0.0, 0.0, m, m, Direction::NorthWest, Diag),
        edge(w - m, h - m, m, m, Direction::SouthEast, Diag),
        edge(w - m, 0.0, m, m, Direction::NorthEast, Anti),
        edge(0.0, h - m, m, m, Direction::SouthWest, Anti),
    ]
    .into()
}

/// The strip along the bottom: small monospace text on both sides.
pub fn status_bar<'a>(left: String, right: String) -> Element<'a, Message> {
    container(
        row![
            text(left)
                .font(style::MONO)
                .size(10.5)
                .color(style::MUTED())
                .wrapping(text::Wrapping::None),
            space::horizontal(),
            text(right)
                .font(style::MONO)
                .size(10.5)
                .color(style::MUTED())
                .wrapping(text::Wrapping::None),
        ]
        .align_y(Vertical::Center)
        .height(30),
    )
    .padding([0, 18])
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(Background::Color(style::BG())),
        ..container::Style::default()
    })
    .into()
}
