//! Artwork cards, the rows and grids they sit in, and artwork cropped to fit.
//! Laid out like the default Valo style: posters with the title below.
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{
    button, column, container, image, mouse_area, row, scrollable, space, stack, text, tooltip,
    Space,
};
use iced::{Background, Border, Color, ContentFit, Element, Length, Padding, Rectangle};

use super::super::icon::{self, Icon};
use super::super::images::{Images, Variant};
use super::super::style::{self, black, c, white, BOLD};
use super::Msg;
use crate::ui::theme;

pub const POSTER: (f32, f32) = (150.0, 225.0);
pub const WIDE: (f32, f32) = (288.0, 162.0);
pub const GAP: f32 = 14.0;
pub const SIDE: f32 = 40.0;
/// Room for the title and year under a card.
const TEXT_ROOM: f32 = 40.0;

/// What a card shows, and what clicking it does.
pub struct Card {
    pub item: String,
    pub episode: Option<String>,
    pub title: String,
    pub sub: Option<String>,
    pub art: Option<String>,
    pub progress: Option<f32>,
    /// A movie that's been watched.
    pub watched: bool,
    /// Being moved between disks: it can't play.
    pub moving: bool,
    /// Can be taken off Continue watching.
    pub removable: bool,
    /// On My list.
    pub marked: bool,
}

/// `url`'s artwork covering `w`×`h`: cropped to fit, keeping the top of tall
/// images (faces, titles) like the egui `image_cover`. A colored stand-in
/// with the title until it's loaded, or if it can't be.
pub fn art<'a>(
    images: &Images,
    url: Option<&str>,
    title: &str,
    w: f32,
    h: f32,
    radius: f32,
) -> Element<'a, Msg> {
    let v = Variant::for_size(w, h);
    if let Some((handle, (iw, ih))) = url.and_then(|u| images.get(u, v)) {
        let (iw, ih) = (iw as f32, ih as f32);
        let (ta, ra) = (iw / ih.max(1.0), w / h.max(1.0));
        let crop = if ta > ra {
            let cw = ih * ra;
            Rectangle {
                x: ((iw - cw) / 2.0) as u32,
                y: 0,
                width: cw as u32,
                height: ih as u32,
            }
        } else {
            let ch = iw / ra;
            Rectangle {
                x: 0,
                y: (0.15 * (ih - ch)) as u32,
                width: iw as u32,
                height: ch as u32,
            }
        };
        return image(handle)
            .crop(crop)
            .width(w)
            .height(h)
            .content_fit(ContentFit::Fill)
            .border_radius(radius)
            .into();
    }
    let loading = url.is_some_and(|u| !images.failed(u, v));
    if loading {
        return container(space())
            .width(w)
            .height(h)
            .style(move |_| container::Style {
                background: Some(Background::Color(style::c(theme::pal().surface))),
                border: iced::border::rounded(radius),
                ..container::Style::default()
            })
            .into();
    }
    placeholder(title, w, h, radius)
}

/// Artwork stand-in: a colored tile with the title on it.
pub fn placeholder<'a>(title: &str, w: f32, h: f32, radius: f32) -> Element<'a, Msg> {
    let col = c(crate::util::color_for(title));
    let size = (w / 9.0).clamp(11.0, 22.0);
    container(
        text(title.to_owned())
            .font(BOLD)
            .size(size)
            .color(Color::WHITE)
            .align_x(Horizontal::Center),
    )
    .padding(8)
    .width(w)
    .height(h)
    .center_x(w)
    .center_y(h)
    .style(move |_| container::Style {
        background: Some(Background::Gradient(
            iced::gradient::Linear::new(iced::Radians(std::f32::consts::PI))
                .add_stop(0.0, col)
                .add_stop(0.5, col)
                .add_stop(
                    1.0,
                    Color {
                        a: 1.0,
                        ..mix(col, Color::BLACK, 0.55)
                    },
                )
                .into(),
        )),
        border: iced::border::rounded(radius),
        ..container::Style::default()
    })
    .into()
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgb(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}

/// A round button over a card's corner (bookmark, remove).
fn corner<'a>(ic: Icon, on: bool, tip: &str, msg: Msg) -> Element<'a, Msg> {
    let b = button(icon::icon(
        ic,
        if on {
            Color::from_rgb8(20, 20, 24)
        } else {
            Color::WHITE
        },
        30.0,
    ))
    .padding(0)
    .width(30)
    .height(30)
    .style(move |_, s| {
        let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(Background::Color(if on {
                Color::WHITE
            } else {
                black(if hot { 230 } else { 150 })
            })),
            border: Border {
                color: if hot && !on {
                    white(150)
                } else {
                    Color::TRANSPARENT
                },
                width: 1.0,
                radius: 15.0.into(),
            },
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

/// The played part of a card, along its bottom.
pub fn progress_bar<'a>(f: f32, w: f32) -> Element<'a, Msg> {
    let bar_w = (w - 16.0).max(1.0);
    let played = c(theme::played());
    container(stack![
        container(space())
            .width(bar_w)
            .height(4)
            .style(|_| container::Style {
                background: Some(Background::Color(white(70))),
                border: iced::border::rounded(2),
                ..container::Style::default()
            }),
        container(space())
            .width(bar_w * f.clamp(0.0, 1.0))
            .height(4)
            .style(move |_| container::Style {
                background: Some(Background::Color(played)),
                border: iced::border::rounded(2),
                ..container::Style::default()
            }),
    ])
    .padding(Padding::default().left(8.0).bottom(5.0))
    .height(Length::Fill)
    .align_y(Vertical::Bottom)
    .into()
}

/// A small label over the art's top left ("WATCHED", "MOVING").
fn badge<'a>(label: &str, fill: Color) -> Element<'a, Msg> {
    container(
        container(
            text(label.to_owned())
                .font(BOLD)
                .size(9.5)
                .color(Color::WHITE),
        )
        .padding([2, 6])
        .style(move |_| container::Style {
            background: Some(Background::Color(fill)),
            border: iced::border::rounded(3),
            ..container::Style::default()
        }),
    )
    .padding(8)
    .into()
}

/// One card. `wide`: a Continue watching card (16:9, plays when clicked).
/// `hovered`: the pointer is over it, so its buttons show.
pub fn card<'a>(
    images: &Images,
    c: &Card,
    wide: bool,
    hovered: bool,
    key: String,
) -> Element<'a, Msg> {
    let spec = theme::spec();
    let (w, h) = if wide { WIDE } else { POSTER };
    let radius = f32::from(spec.card_radius);
    let mut layers: Vec<Element<Msg>> = vec![art(images, c.art.as_deref(), &c.title, w, h, radius)];
    if c.moving {
        layers.push(
            container(space())
                .width(w)
                .height(h)
                .style(move |_| container::Style {
                    background: Some(Background::Color(black(130))),
                    border: iced::border::rounded(radius),
                    ..container::Style::default()
                })
                .into(),
        );
        layers.push(badge("MOVING", Color::from_rgb8(200, 40, 40)));
    } else if c.watched && !hovered {
        layers.push(badge("WATCHED", style::c(theme::pal().accent)));
    }
    if let Some(f) = c.progress {
        layers.push(progress_bar(f, w));
    }
    if hovered {
        // an outline, and the buttons
        let outline = spec.hover_outline;
        layers.push(
            container(space())
                .width(w)
                .height(h)
                .style(move |_| container::Style {
                    border: Border {
                        color: style::TEXT(),
                        width: outline,
                        radius: radius.into(),
                    },
                    ..container::Style::default()
                })
                .into(),
        );
        let mut corners = row![].spacing(6);
        if c.removable {
            corners = corners.push(corner(
                Icon::Close,
                false,
                "Remove from Continue watching",
                Msg::Hide(c.item.clone()),
            ));
        }
        corners = corners.push(corner(
            if c.marked {
                Icon::BookmarkOn
            } else {
                Icon::Bookmark
            },
            c.marked,
            if c.marked {
                "Remove from My list"
            } else {
                "Add to My list"
            },
            Msg::Bookmark(c.item.clone()),
        ));
        layers.push(
            container(corners)
                .width(w)
                .padding(8)
                .align_x(Horizontal::Right)
                .into(),
        );
        let info = button(text("More info").font(BOLD).size(12).color(Color::WHITE))
            .padding([5, 10])
            .style(|_, s| button::Style {
                background: Some(Background::Color(black(
                    if matches!(s, button::Status::Hovered) {
                        235
                    } else {
                        170
                    },
                ))),
                border: Border {
                    color: white(110),
                    width: 1.0,
                    radius: 13.0.into(),
                },
                ..button::Style::default()
            })
            .on_press(Msg::Open(c.item.clone()));
        let lift = if c.progress.is_some() { 16.0 } else { 8.0 };
        layers.push(
            container(info)
                .height(h)
                .padding(Padding::default().left(8.0).bottom(lift))
                .align_y(Vertical::Bottom)
                .into(),
        );
    }
    let click = match (&c.episode, wide) {
        (Some(ep), _) => Msg::Play(c.item.clone(), Some(ep.clone()), false),
        (None, true) => Msg::Play(c.item.clone(), None, false),
        (None, false) => Msg::Open(c.item.clone()),
    };
    let art = mouse_area(stack(layers).width(w).height(h))
        .on_press(click)
        .on_enter(Msg::Hover(Some(key)))
        .on_exit(Msg::Hover(None))
        .interaction(iced::mouse::Interaction::Pointer);
    let mut under = column![text(c.title.clone())
        .size(14)
        .color(if hovered {
            style::TEXT()
        } else {
            style::TEXT().scale_alpha(0.85)
        })
        .wrapping(text::Wrapping::None)]
    .spacing(2);
    if let Some(s) = &c.sub {
        under = under.push(
            text(s.clone())
                .size(12)
                .color(style::MUTED())
                .wrapping(text::Wrapping::None),
        );
    }
    column![
        art,
        container(under)
            .width(w)
            .height(TEXT_ROOM)
            .padding(Padding::default().left(2.0).top(8.0))
            .clip(true),
    ]
    .width(w)
    .into()
}

/// A titled row of cards that scrolls sideways, with arrows to page it.
/// `all`: a "See all" link. `width`: the page's.
pub fn row_of<'a>(
    images: &Images,
    title: &str,
    cards: &[Card],
    wide: bool,
    hovered: Option<&str>,
    all: Option<Msg>,
    width: f32,
) -> Element<'a, Msg> {
    let id = format!("row\u{1f}{title}");
    let mut head = row![text(title.to_owned()).font(BOLD).size(row_title())]
        .spacing(10)
        .align_y(Vertical::Center);
    if let Some(m) = all {
        head = head.push(
            mouse_area(
                text("SEE ALL")
                    .font(style::MONO)
                    .size(11)
                    .color(style::MUTED()),
            )
            .on_press(m)
            .interaction(iced::mouse::Interaction::Pointer),
        );
    }
    let card_w = if wide { WIDE.0 } else { POSTER.0 };
    let overflows = cards.len() as f32 * (card_w + GAP) + 2.0 * SIDE - GAP > width;
    if overflows {
        let step = (width - 2.0 * SIDE).max(card_w);
        head = head.push(space::horizontal()).push(
            row![
                arrow(Icon::ChevronLeft, Msg::Page(id.clone(), -step)),
                arrow(Icon::ChevronRight, Msg::Page(id.clone(), step)),
            ]
            .spacing(4),
        );
    }
    let mut strip = row![Space::new().width(SIDE - GAP)].spacing(GAP);
    for c in cards {
        let key = format!("{title}\u{1f}{}", c.item);
        let hot = hovered == Some(key.as_str());
        strip = strip.push(card(images, c, wide, hot, key));
    }
    strip = strip.push(Space::new().width(SIDE - GAP));
    column![
        container(head).padding(
            Padding::default()
                .left(SIDE)
                .right(SIDE)
                .top(18.0)
                .bottom(6.0)
        ),
        scrollable(strip)
            .id(id)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new().width(0).scroller_width(0),
            ))
            .width(Length::Fill),
    ]
    .into()
}

/// A row's paging arrow.
fn arrow<'a>(ic: Icon, msg: Msg) -> Element<'a, Msg> {
    button(icon::icon(ic, style::TEXT(), 26.0))
        .padding(0)
        .style(style::bare(style::SURFACE_HI(), 13.0))
        .on_press(msg)
        .into()
}

/// Row titles' size in the current style.
pub fn row_title() -> f32 {
    theme::spec().row_title.max(16.0)
}

/// Cards in rows as wide as the page (`width`) allows.
pub fn grid<'a>(
    images: &Images,
    cards: &[Card],
    width: f32,
    hovered: Option<&str>,
    salt: &str,
) -> Element<'a, Msg> {
    grid_at(images, cards, width - 2.0 * SIDE, hovered, salt, SIDE)
}

/// Cards in rows `avail` wide, `left` in from the side.
pub fn grid_at<'a>(
    images: &Images,
    cards: &[Card],
    avail: f32,
    hovered: Option<&str>,
    salt: &str,
    left: f32,
) -> Element<'a, Msg> {
    let cols = ((avail + GAP) / (POSTER.0 + GAP)).floor().max(1.0) as usize;
    let mut col = column![].spacing(12);
    for chunk in cards.chunks(cols) {
        let mut r = row![].spacing(GAP);
        for c in chunk {
            let key = format!("{salt}\u{1f}{}", c.item);
            let hot = hovered == Some(key.as_str());
            r = r.push(card(images, c, false, hot, key));
        }
        col = col.push(r);
    }
    container(col).padding(Padding::default().left(left)).into()
}
