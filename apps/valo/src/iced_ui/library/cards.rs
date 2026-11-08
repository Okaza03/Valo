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
use super::super::spaced::{heading, mono};
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

/// Which card the pointer is over, and whose "⋮" menu is open (by key).
#[derive(Clone, Copy, Default)]
pub struct Hot<'s> {
    pub card: Option<&'s str>,
    pub menu: Option<&'s str>,
}

/// A round 30px button over a card's corner (bookmark, "⋮").
fn corner<'a>(ic: Icon, marked: bool, lit: bool, tip: &str, msg: Msg) -> Element<'a, Msg> {
    let b = button(icon::icon(
        ic,
        if marked {
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
        let hot = lit || matches!(s, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(Background::Color(if marked {
                Color::WHITE
            } else {
                black(if hot { 230 } else { 150 })
            })),
            border: Border {
                color: if hot && !marked && !matches!(ic, Icon::More) {
                    white(153)
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
    super::super::chrome::tip_below(b, tip)
}

/// The played part of a card, along its bottom: 8 in from the sides, 5 up.
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

/// One card, like egui's `card_at` in the Valo style: the artwork (growing
/// a little and outlined while hovered), the played bar, the WATCHED sash
/// and MOVING band, and on hover More info, the bookmark, and for Continue
/// watching a "⋮" menu; the title and year under it. `wide`: a Continue
/// watching card (16:9, plays when clicked, a play disc on hover).
pub fn card<'a>(
    images: &Images,
    c: &Card,
    wide: bool,
    hot: Hot<'_>,
    key: String,
) -> Element<'a, Msg> {
    let spec = theme::spec();
    let (w, h) = if wide { WIDE } else { POSTER };
    let hovered = hot.card == Some(key.as_str());
    let menu_open = hot.menu == Some(key.as_str());
    let radius = f32::from(spec.card_radius);
    let mut layers: Vec<Element<Msg>> = vec![art(images, c.art.as_deref(), &c.title, w, h, radius)];
    if let Some(f) = c.progress {
        layers.push(progress_bar(f, w));
    }
    if c.watched && !hovered {
        layers.push(super::marks::sash(w, h, 1.0).into());
    }
    if c.moving {
        layers.push(super::marks::tape(w, h, radius).into());
    }
    if hovered {
        if spec.hover_outline > 0.0 {
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
        }
        if wide {
            layers.push(
                container(
                    container(icon::icon(Icon::Play, Color::WHITE, 24.0))
                        .center(48)
                        .style(|_| container::Style {
                            background: Some(Background::Color(black(160))),
                            border: iced::border::rounded(24),
                            ..container::Style::default()
                        }),
                )
                .center(Length::Fill)
                .into(),
            );
        }
        let lift = if c.progress.is_some() { 16.0 } else { 8.0 };
        let info = button(
            container(text("More info").font(BOLD).size(12).color(Color::WHITE)).center_y(26),
        )
        .height(26)
        .padding([0, 10])
        .style(|_, s| {
            let on = matches!(s, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(Background::Color(black(if on { 235 } else { 170 }))),
                border: Border {
                    color: Color::WHITE.scale_alpha(if on { 0.9 } else { 0.4 }),
                    width: 1.0,
                    radius: 13.0.into(),
                },
                ..button::Style::default()
            }
        })
        .on_press(Msg::Open(c.item.clone()));
        layers.push(
            container(info)
                .height(h)
                .padding(Padding::default().left(8.0).bottom(lift))
                .align_y(Vertical::Bottom)
                .into(),
        );
    }
    if hovered || menu_open {
        let mut corners = row![].spacing(6);
        if c.removable {
            corners = corners.push(corner(
                Icon::More,
                false,
                menu_open,
                "More",
                Msg::CardMenu((!menu_open).then(|| key.clone())),
            ));
        }
        if hovered {
            corners = corners.push(corner(
                if c.marked {
                    Icon::BookmarkOn
                } else {
                    Icon::Bookmark
                },
                c.marked,
                false,
                if c.marked {
                    "Remove from My list"
                } else {
                    "Add to My list"
                },
                Msg::Bookmark(c.item.clone()),
            ));
        } else {
            corners = corners.push(space().width(30));
        }
        layers.push(
            container(corners)
                .width(w)
                .padding(Padding::default().top(8.0).right(8.0))
                .align_x(Horizontal::Right)
                .into(),
        );
    }
    if menu_open {
        // the "⋮" menu, under its button, over whatever is below the card
        let menu = container(super::super::widgets::menu_item(
            "Remove from Continue watching",
            Msg::Hide(c.item.clone()),
        ))
        .width(252)
        .padding(6)
        .style(super::popup_style);
        let x = w - 74.0;
        layers.push(
            iced::widget::float(container(menu).padding(Padding::default().top(40.0)))
                .translate(move |_, _| iced::Vector::new(x, 0.0))
                .into(),
        );
    }
    let click = match (&c.episode, wide) {
        (Some(ep), _) => Msg::Play(c.item.clone(), Some(ep.clone()), false),
        (None, true) => Msg::Play(c.item.clone(), None, false),
        (None, false) => Msg::Open(c.item.clone()),
    };
    let grow = if hovered {
        (w + 2.0 * spec.hover_grow) / w
    } else {
        1.0
    };
    let art_area: Element<Msg> = iced::widget::float(stack(layers).width(w).height(h))
        .scale(grow)
        .into();
    let art_area = mouse_area(art_area)
        .on_press(click)
        .on_enter(Msg::Hover(Some(key)))
        .on_exit(Msg::Hover(None))
        .interaction(iced::mouse::Interaction::Pointer);
    let art_area: Element<Msg> = if wide {
        art_area.into()
    } else {
        tooltip(
            art_area,
            container(text(c.title.clone()).size(12.5))
                .padding([4, 8])
                .style(style::tip),
            tooltip::Position::FollowCursor,
        )
        .into()
    };
    let mut under = column![text(c.title.clone())
        .size(14)
        .color(if hovered {
            style::TEXT()
        } else {
            style::TEXT().scale_alpha(0.85)
        })
        .wrapping(text::Wrapping::None)];
    if let Some(s) = &c.sub {
        under = under.push(
            text(s.clone())
                .size(12)
                .color(style::MUTED())
                .wrapping(text::Wrapping::None),
        );
    }
    column![
        art_area,
        container(under)
            .width(w)
            .height(TEXT_ROOM)
            .padding(Padding::default().left(2.0).top(8.0))
            .clip(true),
    ]
    .width(w)
    .into()
}

/// A titled row of cards that scrolls sideways (egui's `Browse::row`):
/// the heading, a "See all" link, and while the pointer is over the cards,
/// arrows at the edges that page them. `width`: the page's; `offset`: how
/// far the row is scrolled; `over`: the pointer is over the row.
#[allow(clippy::too_many_arguments)]
pub fn row_of<'a>(
    images: &Images,
    title: &str,
    cards: &[Card],
    wide: bool,
    hot: Hot<'_>,
    all: Option<Msg>,
    width: f32,
    offset: f32,
    over: bool,
) -> Element<'a, Msg> {
    let id = format!("row\u{1f}{title}");
    let mut head = row![heading(title)].align_y(Vertical::Center);
    if let Some(m) = all {
        head = head.push(space().width(10)).push(
            mouse_area(mono("See all", 11.0).color(style::MUTED()))
                .on_press(m)
                .interaction(iced::mouse::Interaction::Pointer),
        );
    }
    let (card_w, card_h) = if wide { WIDE } else { POSTER };
    let mut strip = row![Space::new().width(SIDE - GAP)].spacing(GAP);
    for c in cards {
        let key = format!("{title}\u{1f}{}", c.item);
        strip = strip.push(card(images, c, wide, hot, key));
    }
    strip = strip.push(Space::new().width(SIDE - GAP));
    let content_w = 2.0 * SIDE + cards.len() as f32 * (card_w + GAP) - GAP;
    let max = (content_w - width).max(0.0);
    let scroll_id = id.clone();
    let strip = scrollable(strip)
        .id(id.clone())
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(0).scroller_width(0),
        ))
        .on_scroll(move |v| Msg::Scrolled(scroll_id.clone(), v.absolute_offset().x))
        .width(Length::Fill);
    let h = card_h + TEXT_ROOM;
    let mut layers: Vec<Element<'a, Msg>> = vec![strip.into()];
    if over && max > 0.0 {
        let step = width - 2.0 * SIDE;
        let left = (offset > 1.0).then(|| arrow("‹", Msg::Page(id.clone(), -step)));
        let right = (offset < max - 1.0).then(|| arrow("›", Msg::Page(id.clone(), step)));
        layers.push(
            container(
                row![
                    left.unwrap_or_else(|| space().width(30).into()),
                    space::horizontal(),
                    right.unwrap_or_else(|| space().width(30).into()),
                ]
                .height(h - 40.0),
            )
            .width(width)
            .height(h)
            .padding(Padding::default().left(1.0).right(1.0).bottom(20.0))
            .align_y(Vertical::Center)
            .into(),
        );
    }
    let key = format!("row\u{1f}{title}");
    column![
        container(head).padding(Padding::default().left(SIDE).top(18.0).bottom(14.0)),
        mouse_area(stack(layers).width(Length::Fill).height(h))
            .on_enter(Msg::RowOver(Some(key)))
            .on_exit(Msg::RowOver(None)),
        space().height(8),
    ]
    .into()
}

/// A row's paging arrow: a dark strip with a chevron.
fn arrow<'a>(glyph: &str, msg: Msg) -> Element<'a, Msg> {
    button(
        container(text(glyph.to_owned()).size(30).color(Color::WHITE))
            .center_x(30)
            .center_y(Length::Fill),
    )
    .width(30)
    .height(Length::Fill)
    .padding(0)
    .style(|_, s| button::Style {
        background: Some(Background::Color(black(
            if matches!(s, button::Status::Hovered | button::Status::Pressed) {
                190
            } else {
                130
            },
        ))),
        border: iced::border::rounded(2),
        ..button::Style::default()
    })
    .on_press(msg)
    .into()
}

/// Cards in rows as wide as the page (`width`) allows (egui's `grid`).
pub fn grid<'a>(
    images: &Images,
    cards: &[Card],
    width: f32,
    hot: Hot<'_>,
    salt: &str,
) -> Element<'a, Msg> {
    grid_at(images, cards, width - 2.0 * SIDE, hot, salt, SIDE)
}

/// Cards in rows `avail` wide, `left` in from the side; 20 between rows
/// (egui's 12 and its item spacing).
pub fn grid_at<'a>(
    images: &Images,
    cards: &[Card],
    avail: f32,
    hot: Hot<'_>,
    salt: &str,
    left: f32,
) -> Element<'a, Msg> {
    let cols = ((avail + GAP) / (POSTER.0 + GAP)).floor().max(1.0) as usize;
    let mut col = column![].spacing(20);
    for chunk in cards.chunks(cols) {
        let mut r = row![].spacing(GAP);
        for c in chunk {
            let key = format!("{salt}\u{1f}{}", c.item);
            r = r.push(card(images, c, false, hot, key));
        }
        col = col.push(r);
    }
    container(col).padding(Padding::default().left(left)).into()
}
