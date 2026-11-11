//! Artwork cards, the rows and grids they sit in, and artwork cropped to fit.
//! Shaped by the style: posters or 16:9 tiles, the title below, over the
//! artwork, on hover, or a channel-style line.
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
/// The room a card's text takes under its artwork.
pub fn text_room(t: theme::TileText) -> f32 {
    match t {
        theme::TileText::Below => 40.0,
        theme::TileText::Rich => 78.0,
        theme::TileText::Over | theme::TileText::Hover => 0.0,
    }
}

/// Whether cards are 16:9 tiles in this style (Continue watching always is).
pub fn landscape(wide: bool) -> bool {
    wide || theme::spec().layout.tiles == theme::Tiles::Landscape
}

/// What a card shows, and what clicking it does.
pub struct Card {
    pub item: String,
    pub episode: Option<String>,
    pub title: String,
    pub sub: Option<String>,
    pub art: Option<String>,
    /// 16:9 artwork, for the styles with landscape cards.
    pub wide_art: Option<String>,
    /// "Movie · 2014 · 1h 26m", for the styles that show it on the card.
    pub meta: Option<String>,
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
/// images (faces, titles). A colored stand-in
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

/// One card in the Valo style: the artwork (growing
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
    let land = landscape(wide);
    let size = if land { WIDE } else { POSTER };
    card_at(images, c, size, land, wide, hot, key)
}

/// [`card`] at any size (the video-style grid sizes its cards to the width).
pub fn card_at<'a>(
    images: &Images,
    c: &Card,
    size: (f32, f32),
    land: bool,
    wide: bool,
    hot: Hot<'_>,
    key: String,
) -> Element<'a, Msg> {
    use theme::TileText;
    let spec = theme::spec();
    let words = spec.layout.text;
    let (w, h) = size;
    let hovered = hot.card == Some(key.as_str());
    let menu_open = hot.menu == Some(key.as_str());
    let radius = f32::from(spec.card_radius);
    let url = if land {
        c.wide_art.as_deref().or(c.art.as_deref())
    } else {
        c.art.as_deref()
    };
    // the title goes over the art anyway: don't print it twice
    let title_on_art = if words == TileText::Over {
        ""
    } else {
        &c.title
    };
    let mut layers: Vec<Element<Msg>> = vec![art(images, url, title_on_art, w, h, radius)];
    let lift = if c.progress.is_some() { 10.0 } else { 0.0 };
    match words {
        TileText::Over => {
            layers.push(shade(w, h, 0.6, 0.75, radius));
            let mut over = column![text(c.title.clone())
                .font(style::BLACK)
                .size(if land { 17 } else { 15 })
                .color(Color::WHITE)];
            if let (true, Some(s)) = (wide, &c.sub) {
                over = over.push(
                    text(s.clone())
                        .size(12.5)
                        .color(white(200))
                        .wrapping(text::Wrapping::None),
                );
            }
            layers.push(
                container(over.width(w - 24.0))
                    .height(h)
                    .padding(Padding::default().left(12.0).bottom(12.0 + lift))
                    .align_y(Vertical::Bottom)
                    .clip(true)
                    .into(),
            );
        }
        TileText::Hover if hovered => {
            layers.push(shade(w, h, 0.7, 0.85, radius));
            let disc = container(icon::icon(Icon::Play, Color::WHITE, 16.0))
                .center(30)
                .style(|_| container::Style {
                    background: Some(Background::Color(style::c(theme::ACCENT()))),
                    border: iced::border::rounded(15),
                    ..container::Style::default()
                });
            let mut info = column![text(c.title.clone())
                .font(BOLD)
                .size(15)
                .color(Color::WHITE)
                .wrapping(text::Wrapping::None)];
            if let Some(m) = c.sub.as_ref().filter(|_| wide).or(c.meta.as_ref()) {
                info = info.push(
                    text(m.clone())
                        .size(12.5)
                        .color(white(200))
                        .wrapping(text::Wrapping::None),
                );
            }
            layers.push(
                container(
                    row![disc, info.spacing(2)]
                        .spacing(10)
                        .align_y(Vertical::Center),
                )
                .width(w)
                .height(h)
                .padding(Padding::default().left(11.0).right(4.0).bottom(13.0 + lift))
                .align_y(Vertical::Bottom)
                .clip(true)
                .into(),
            );
        }
        _ => {}
    }
    if let Some(f) = c.progress {
        layers.push(if words == TileText::Rich {
            edge_bar(f, w)
        } else {
            progress_bar(f, w)
        });
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
        if wide && words != TileText::Hover {
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
        if words == TileText::Below {
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
    let art_area: Element<Msg> =
        crate::iced_ui::grow::grow(stack(layers).width(w).height(h), grow).into();
    let art_area = mouse_area(art_area)
        .on_press(click)
        .on_enter(Msg::Hover(Some(key)))
        .on_exit(Msg::Hover(None))
        .interaction(iced::mouse::Interaction::Pointer);
    let art_area: Element<Msg> = if wide || words != TileText::Below {
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
    let room = text_room(words);
    let under: Option<Element<Msg>> = match words {
        TileText::Below => {
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
            Some(
                container(under)
                    .width(w)
                    .height(room)
                    .padding(Padding::default().left(2.0).top(8.0))
                    .clip(true)
                    .into(),
            )
        }
        TileText::Rich => {
            // a round "channel" badge, a two-line title, then the details
            let initial: String = c.title.chars().take(1).collect::<String>().to_uppercase();
            let badge_col = style::c(crate::util::color_for(&c.title));
            let badge = container(
                text(initial)
                    .font(style::BLACK)
                    .size(16)
                    .color(Color::WHITE),
            )
            .center(36)
            .style(move |_| container::Style {
                background: Some(Background::Color(badge_col)),
                border: iced::border::rounded(18),
                ..container::Style::default()
            });
            let mut info = column![container(
                text(c.title.clone())
                    .font(BOLD)
                    .size(15.5)
                    .color(style::TEXT())
            )
            .max_height(42.0)
            .clip(true)];
            let meta = if wide {
                c.sub.as_ref()
            } else {
                c.meta.as_ref()
            };
            if let Some(m) = meta {
                info = info.push(
                    text(m.clone())
                        .size(13.5)
                        .color(style::MUTED())
                        .wrapping(text::Wrapping::None),
                );
            }
            Some(
                container(
                    row![badge, info.spacing(4).width(w - 48.0)]
                        .spacing(12)
                        .align_y(Vertical::Top),
                )
                .width(w)
                .height(room)
                .padding(Padding::default().top(12.0))
                .clip(true)
                .into(),
            )
        }
        TileText::Over | TileText::Hover => None,
    };
    let mut col = column![art_area].width(w);
    if let Some(u) = under {
        col = col.push(u);
    }
    col.into()
}

/// A dark fade over the bottom `frac` of a card, `a` opaque at the bottom.
fn shade<'a>(w: f32, h: f32, frac: f32, a: f32, radius: f32) -> Element<'a, Msg> {
    let dark = Color::from_rgba(0.0, 0.0, 0.0, a);
    container(
        container(space())
            .width(w)
            .height(h * frac)
            .style(move |_| container::Style {
                border: iced::border::rounded(iced::border::Radius::default().bottom(radius)),
                ..super::fill_gradient(std::f32::consts::PI, Color::TRANSPARENT, dark)
            }),
    )
    .height(h)
    .align_y(Vertical::Bottom)
    .into()
}

/// The played part along a card's bottom edge (the video style's red line).
fn edge_bar<'a>(f: f32, w: f32) -> Element<'a, Msg> {
    let played = c(theme::played());
    container(stack![
        container(space())
            .width(w)
            .height(4)
            .style(|_| container::Style {
                background: Some(Background::Color(white(70))),
                ..container::Style::default()
            }),
        container(space())
            .width(w * f.clamp(0.0, 1.0))
            .height(4)
            .style(move |_| container::Style {
                background: Some(Background::Color(played)),
                ..container::Style::default()
            }),
    ])
    .height(Length::Fill)
    .align_y(Vertical::Bottom)
    .into()
}

/// A titled row of cards that scrolls sideways:
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
    let (card_w, card_h) = if landscape(wide) { WIDE } else { POSTER };
    // a hovered card grows past the row's top, and the row's scroll area clips
    // what's drawn over it: room for that above the cards, taken off the gap
    // under the heading so the page doesn't move
    let rise = hover_rise(card_w, card_h);
    let mut strip = row![Space::new().width(SIDE - GAP)].spacing(GAP);
    for c in cards {
        let key = format!("{title}\u{1f}{}", c.item);
        strip = strip.push(card(images, c, wide, hot, key));
    }
    strip = strip.push(Space::new().width(SIDE - GAP));
    let content_w = 2.0 * SIDE + cards.len() as f32 * (card_w + GAP) - GAP;
    let max = (content_w - width).max(0.0);
    let scroll_id = id.clone();
    let strip = scrollable(container(strip).padding(Padding::default().top(rise)))
        .id(id.clone())
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(0).scroller_width(0),
        ))
        .on_scroll(move |v| Msg::Scrolled(scroll_id.clone(), v.absolute_offset().x))
        .width(Length::Fill);
    let h = card_h + text_room(theme::spec().layout.text);
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
            .height(h + rise)
            .padding(
                Padding::default()
                    .left(1.0)
                    .right(1.0)
                    .top(rise)
                    .bottom(20.0),
            )
            .align_y(Vertical::Center)
            .into(),
        );
    }
    let key = format!("row\u{1f}{title}");
    column![
        container(head).padding(
            Padding::default()
                .left(SIDE)
                .top(18.0)
                .bottom((14.0 - rise).max(0.0))
        ),
        mouse_area(stack(layers).width(Length::Fill).height(h + rise))
            .on_enter(Msg::RowOver(Some(key)))
            .on_exit(Msg::RowOver(None)),
        space().height(8),
    ]
    .into()
}

/// How far a hovered `w`×`h` card grows above its top edge.
fn hover_rise(w: f32, h: f32) -> f32 {
    let g = theme::spec().hover_grow;
    (g * h / w.max(1.0)).ceil() + 1.0
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

/// Cards in rows as wide as the page (`width`) allows.
pub fn grid<'a>(
    images: &Images,
    cards: &[Card],
    width: f32,
    hot: Hot<'_>,
    salt: &str,
) -> Element<'a, Msg> {
    grid_at(images, cards, width - 2.0 * SIDE, hot, salt, SIDE)
}

/// Cards in rows `avail` wide, `left` in from the side; 20 between rows.
pub fn grid_at<'a>(
    images: &Images,
    cards: &[Card],
    avail: f32,
    hot: Hot<'_>,
    salt: &str,
    left: f32,
) -> Element<'a, Msg> {
    grid_fit(images, cards, avail, hot, salt, left, false)
}

/// [`grid_at`] for any cards: landscape ones stretch to fill the row, like a
/// video grid. `wide`: Continue watching cards.
pub fn grid_fit<'a>(
    images: &Images,
    cards: &[Card],
    avail: f32,
    hot: Hot<'_>,
    salt: &str,
    left: f32,
    wide: bool,
) -> Element<'a, Msg> {
    let land = landscape(wide);
    let base = if land { WIDE } else { POSTER };
    let cols = ((avail + GAP) / (base.0 + GAP)).floor().max(1.0);
    let size = if land {
        let w = (avail - GAP * (cols - 1.0)) / cols;
        (w, w * 9.0 / 16.0)
    } else {
        base
    };
    let rich = theme::spec().layout.text == theme::TileText::Rich;
    let mut col = column![].spacing(if rich { 8 } else { 20 });
    for chunk in cards.chunks(cols as usize) {
        let mut r = row![].spacing(GAP);
        for c in chunk {
            let key = format!("{salt}\u{1f}{}", c.item);
            r = r.push(card_at(images, c, size, land, wide, hot, key));
        }
        col = col.push(r);
    }
    container(col).padding(Padding::default().left(left)).into()
}
