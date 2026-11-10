//! The home page and navigation pieces of the app styles (egui's
//! `browse::layouts`): full-width and carousel banners, the Top 10 row,
//! genre tiles, the video-style grid and the sidebar.
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{
    button, column, container, image, mouse_area, row, scrollable, space, stack, text, Column,
};
use iced::{Background, Border, Color, Element, Length, Padding};

use super::cards::{self, Card, POSTER, SIDE};
use super::details::title_art;
use super::{fill_gradient, Env, Library, Msg, View};
use crate::api::{self, Item, Kind};
use crate::iced_ui::icon::{self, Icon};
use crate::iced_ui::images::Variant;
use crate::iced_ui::spaced::heading;
use crate::iced_ui::style::{self, black, c, white, BOLD};
use crate::iced_ui::widgets;
use crate::ui::browse::{meta_line, play_label, shorten, shuffled};
use crate::ui::theme::{self, Skin};

/// The dots under a banner: one per featured title, the current one a bar.
fn dots<'a>(cur: usize, n: usize) -> Element<'a, Msg> {
    let mut r = row![].align_y(Vertical::Center);
    for i in 0..n {
        let on = i == cur;
        r = r.push(
            container(
                container(space())
                    .width(if on { 14 } else { 5 })
                    .height(if on { 4 } else { 5 })
                    .style(move |_| container::Style {
                        background: Some(Background::Color(if on {
                            style::TEXT()
                        } else {
                            c(theme::MUTED().gamma_multiply(0.7))
                        })),
                        border: iced::border::rounded(2.5),
                        ..container::Style::default()
                    }),
            )
            .center_x(16),
        );
    }
    r.into()
}

/// A banner's side arrow: a dark rounded block with a chevron.
fn side_arrow<'a>(glyph: &str, msg: Msg) -> Element<'a, Msg> {
    button(
        container(text(glyph.to_owned()).size(32).color(Color::WHITE))
            .center_x(36)
            .center_y(64),
    )
    .width(36)
    .height(64)
    .padding(0)
    .style(|_, s| button::Style {
        background: Some(Background::Color(black(
            if matches!(s, button::Status::Hovered | button::Status::Pressed) {
                170
            } else {
                90
            },
        ))),
        border: iced::border::rounded(6),
        ..button::Style::default()
    })
    .on_press(msg)
    .into()
}

/// A fill that's `a` opaque.
fn tint(col: Color, a: f32) -> Color {
    Color { a, ..col }
}

impl Library {
    /// Netflix, HBO Max and Apple TV: the artwork across the whole width,
    /// title, details and buttons over its bottom left.
    pub(super) fn hero_full<'a>(
        &'a self,
        item: &'a Item,
        at: (usize, usize),
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let skin = theme::skin();
        let h = (w * if skin == Skin::Apple { 0.52 } else { 0.46 }).clamp(380.0, 780.0);
        let bg = style::BG();
        let url = item.backdrop.as_deref().or(item.poster.as_deref());
        let mut layers: Vec<Element<'a, Msg>> = Vec::new();
        if url.and_then(|u| env.images.get(u, Variant::Full)).is_some() {
            layers.push(cards::art(env.images, url, &item.title, w, h, 0.0));
        } else {
            let col = c(crate::util::color_for(&item.title));
            layers.push(
                container(space())
                    .width(w)
                    .height(h)
                    .style(move |_| fill_gradient(0.0, col, bg))
                    .into(),
            );
        }
        // shade the left and the bottom so the text reads, fading into the page
        layers.push(
            container(space())
                .width(w * 0.62)
                .height(h)
                .style(move |_| {
                    fill_gradient(std::f32::consts::FRAC_PI_2, tint(bg, 0.85), tint(bg, 0.0))
                })
                .into(),
        );
        layers.push(
            container(
                container(space())
                    .width(w)
                    .height(h * 0.4)
                    .style(move |_| fill_gradient(std::f32::consts::PI, tint(bg, 0.0), bg)),
            )
            .height(h)
            .align_y(Vertical::Bottom)
            .into(),
        );

        let text_w = (w * 0.42).clamp(320.0, 600.0).min(w - 2.0 * SIDE);
        let x = if skin == Skin::Netflix {
            56.0
        } else {
            SIDE + 8.0
        };
        let mut words: Column<'a, Msg> = column![].spacing(0).width(text_w);
        let label = match skin {
            Skin::Netflix => Some((
                if item.kind == Kind::Movie {
                    "F I L M"
                } else {
                    "S E R I E S"
                },
                c(theme::ACCENT()),
            )),
            Skin::Apple => Some(("Featured", style::MUTED())),
            _ => None,
        };
        if let Some((l, col)) = label {
            words = words
                .push(text(l).font(BOLD).size(13).color(col))
                .push(space().height(6));
        }
        words = words
            .push(title_art(
                item,
                env,
                (text_w.min(520.0), 140.0),
                if skin == Skin::Apple { 58.0 } else { 52.0 },
            ))
            .push(space().height(8))
            .push(
                text(meta_line(item))
                    .size(14)
                    .color(style::TEXT().scale_alpha(0.75)),
            );
        if !item.overview.is_empty() && skin != Skin::Apple {
            words = words
                .push(space().height(10))
                .push(text(shorten(&item.overview, 240)).size(16));
        }
        let (label, restart) = play_label(item, env.progress, &|i| self.key(i));
        let mut buttons = row![widgets::button_with(
            widgets::Kind::Primary,
            &label,
            Some(Icon::Play),
            (!item.unavailable).then(|| Msg::Play(item.id.clone(), None, false)),
        )]
        .spacing(10);
        if restart && skin != Skin::Netflix {
            buttons = buttons.push(widgets::button_with(
                widgets::Kind::Secondary,
                "Start over",
                None,
                Some(Msg::Play(item.id.clone(), None, true)),
            ));
        }
        buttons = buttons.push(widgets::button_with(
            widgets::Kind::Secondary,
            "More info",
            None,
            Some(Msg::Open(item.id.clone())),
        ));
        words = words.push(space().height(14)).push(buttons);
        layers.push(
            container(words)
                .height(h)
                .padding(Padding::default().left(x).bottom(64.0))
                .align_y(Vertical::Bottom)
                .into(),
        );
        self.banner_marks(&mut layers, at, w, h, h - 26.0);
        mouse_area(stack(layers).width(w).height(h))
            .on_press(Msg::Open(item.id.clone()))
            .on_enter(Msg::RowOver(Some("hero".to_owned())))
            .on_exit(Msg::RowOver(None))
            .interaction(iced::mouse::Interaction::Pointer)
            .into()
    }

    /// The dots at `dots_y` and, while hovered, the arrows at the sides.
    fn banner_marks<'a>(
        &self,
        layers: &mut Vec<Element<'a, Msg>>,
        at: (usize, usize),
        w: f32,
        h: f32,
        dots_y: f32,
    ) {
        if at.1 < 2 {
            return;
        }
        layers.push(
            container(dots(at.0, at.1))
                .width(w)
                .padding(Padding::default().top(dots_y - 3.0))
                .align_x(Horizontal::Center)
                .into(),
        );
        if self.row_over.as_deref() == Some("hero") {
            layers.push(
                container(row![
                    side_arrow("‹", Msg::Hero(-1)),
                    space::horizontal(),
                    side_arrow("›", Msg::Hero(1)),
                ])
                .width(w)
                .height(h)
                .padding([0, 2])
                .align_y(Vertical::Center)
                .into(),
            );
        }
    }

    /// Prime Video and Disney+: a rounded banner with the ones before and
    /// after peeking in at the sides.
    pub(super) fn hero_carousel<'a>(
        &'a self,
        featured: &[&'a Item],
        idx: usize,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let skin = theme::skin();
        let rad = f32::from(theme::spec().card_radius.max(6));
        let n = featured.len();
        let item = featured[idx];
        let m = SIDE + 20.0;
        let bw = w - 2.0 * m;
        let bh = (bw * 0.36).clamp(240.0, 500.0);
        let paint = |it: &'a Item, dim: bool, aw: f32| -> Element<'a, Msg> {
            let url = it.backdrop.as_deref().or(it.poster.as_deref());
            let art = cards::art(env.images, url, &it.title, aw, bh, rad);
            if !dim {
                return art;
            }
            stack![
                art,
                container(space())
                    .width(aw)
                    .height(bh)
                    .style(move |_| container::Style {
                        background: Some(Background::Color(black(145))),
                        border: iced::border::rounded(rad),
                        ..container::Style::default()
                    }),
            ]
            .into()
        };
        // the ones before and after show as slices in the margins
        let gap = 14.0;
        let peek = (m - gap).max(0.0);
        let mut strip = row![].spacing(gap);
        strip = strip.push(if n > 1 && peek > 0.0 {
            Element::from(
                mouse_area(paint(featured[(idx + n - 1) % n], true, peek))
                    .on_press(Msg::Hero(-1))
                    .interaction(iced::mouse::Interaction::Pointer),
            )
        } else {
            space().width(peek).into()
        });
        // the banner: artwork, a dark fade on the left, then the text
        let dark = black(200);
        let lit = self.row_over.as_deref() == Some("hero") && skin == Skin::Disney;
        let mut words: Column<'a, Msg> = column![].width((bw * 0.5).max(320.0));
        if let Some((h, (iw, ih))) = item
            .logo
            .as_deref()
            .and_then(|u| env.images.get(u, Variant::Full))
        {
            let (iw, ih) = (iw as f32, ih as f32);
            let s = (360.0 / iw).min(110.0 / ih);
            words = words.push(image(h).width(iw * s).height(ih * s));
        } else {
            words = words.push(
                text(item.title.clone())
                    .font(style::BLACK)
                    .size(40)
                    .color(Color::WHITE),
            );
        }
        words = words
            .push(space().height(6))
            .push(text(meta_line(item)).size(14).color(white(200)));
        if skin == Skin::Prime {
            let (label, _) = play_label(item, env.progress, &|i| self.key(i));
            words = words.push(space().height(14)).push(widgets::button_with(
                widgets::Kind::Primary,
                &label,
                Some(Icon::Play),
                (!item.unavailable).then(|| Msg::Play(item.id.clone(), None, false)),
            ));
        }
        let banner = stack![
            paint(item, false, bw),
            container(space())
                .width(bw * 0.6)
                .height(bh)
                .style(move |_| container::Style {
                    border: iced::border::rounded(iced::border::Radius::default().left(rad)),
                    ..fill_gradient(std::f32::consts::FRAC_PI_2, dark, Color::TRANSPARENT)
                }),
            container(words)
                .width(bw)
                .height(bh)
                .padding(Padding::default().left(36.0).bottom(28.0))
                .align_y(Vertical::Bottom),
            container(space())
                .width(bw)
                .height(bh)
                .style(move |_| container::Style {
                    border: Border {
                        color: if lit {
                            Color::WHITE
                        } else {
                            Color::TRANSPARENT
                        },
                        width: 3.0,
                        radius: rad.into(),
                    },
                    ..container::Style::default()
                }),
        ];
        strip = strip.push(
            mouse_area(banner)
                .on_press(Msg::Open(item.id.clone()))
                .on_enter(Msg::RowOver(Some("hero".to_owned())))
                .on_exit(Msg::RowOver(None))
                .interaction(iced::mouse::Interaction::Pointer),
        );
        if n > 1 && peek > 0.0 {
            strip = strip.push(
                mouse_area(paint(featured[(idx + 1) % n], true, peek))
                    .on_press(Msg::Hero(1))
                    .interaction(iced::mouse::Interaction::Pointer),
            );
        }
        let strip = container(strip).width(w).height(bh).clip(true);
        let mut col = column![space().height(16), strip];
        if n > 1 {
            col = col.push(
                container(dots(idx, n))
                    .width(w)
                    .padding(Padding::default().top(16.0))
                    .align_x(Horizontal::Center),
            );
        }
        col.push(space().height(14)).into()
    }

    /// Netflix's "Top 10": big outlined numbers beside the posters.
    pub(super) fn top10_row<'a>(
        &'a self,
        items: &[&Item],
        env: Env<'a>,
        progress: &crate::config::Progress,
    ) -> Element<'a, Msg> {
        let (pw, ph) = POSTER;
        let mut strip = row![space().width(SIDE - 6.0)].spacing(6);
        for (i, item) in items.iter().take(10).enumerate() {
            let num_w = if i == 9 { 150.0 } else { 100.0 };
            let key = format!("top10\u{1f}{}", item.id);
            let lit = self.hovered.as_deref() == Some(key.as_str());
            let num = (i + 1).to_string();
            // an outlined numeral: the outline, then the page color inside
            let mut numeral = vec![];
            for (dx, dy) in [
                (-2.5, 0.0),
                (2.5, 0.0),
                (0.0, -2.5),
                (0.0, 2.5),
                (-1.8, -1.8),
                (1.8, 1.8),
                (-1.8, 1.8),
                (1.8, -1.8),
                (0.0, 0.0),
            ] {
                let col = if dx == 0.0 && dy == 0.0 {
                    style::BG()
                } else {
                    Color::from_rgb8(150, 150, 150)
                };
                numeral.push(
                    container(
                        text(num.clone())
                            .font(style::BLACK)
                            .size(190)
                            .color(col)
                            .line_height(1.0),
                    )
                    .width(num_w + 18.0)
                    .height(ph)
                    .padding(Padding::default().left(dx).top(dy + ph - 168.0))
                    .align_x(Horizontal::Right)
                    .into(),
                );
            }
            let mut poster: Vec<Element<'a, Msg>> = vec![cards::art(
                env.images,
                item.poster.as_deref(),
                &item.title,
                pw,
                ph,
                4.0,
            )];
            if self.watched(item, progress) {
                poster.push(super::marks::sash(pw, ph, 1.0).into());
            }
            if item.unavailable {
                poster.push(super::marks::tape(pw, ph, 4.0).into());
            }
            let grow = if lit { (pw + 8.0) / pw } else { 1.0 };
            let tile = stack![
                stack(numeral).width(num_w + 18.0).height(ph),
                container(iced::widget::float(stack(poster).width(pw).height(ph)).scale(grow))
                    .padding(Padding::default().left(num_w)),
            ]
            .width(num_w + pw)
            .height(ph);
            strip = strip.push(crate::iced_ui::chrome::tip_below(
                mouse_area(tile)
                    .on_press(Msg::Open(item.id.clone()))
                    .on_enter(Msg::Hover(Some(key)))
                    .on_exit(Msg::Hover(None))
                    .interaction(iced::mouse::Interaction::Pointer),
                &item.title,
            ));
        }
        strip = strip.push(space().width(SIDE - 6.0));
        column![
            container(heading("Top 10 in your library"))
                .padding(Padding::default().left(SIDE).top(18.0).bottom(14.0)),
            scrollable(strip)
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::new().width(0).scroller_width(0),
                ))
                .width(Length::Fill),
            space().height(8),
        ]
        .into()
    }

    /// Disney+'s row of tiles under the banner, here one per genre.
    pub(super) fn genre_tiles<'a>(
        &'a self,
        lib: &api::Library,
        w: f32,
    ) -> Option<Element<'a, Msg>> {
        let mut genres = lib.genres();
        genres.retain(|g| self.layout.shows(&format!("genre:{g}")));
        genres.truncate(5);
        if genres.len() < 3 {
            return None;
        }
        let n = genres.len() as f32;
        let gap = 18.0;
        let tw = (w - 2.0 * SIDE - gap * (n - 1.0)) / n;
        let th = (tw * 0.56).clamp(70.0, 150.0);
        let mut r = row![].spacing(gap);
        for g in genres {
            let key = format!("genre\u{1f}{g}");
            let lit = self.hovered.as_deref() == Some(key.as_str());
            let col = c(crate::util::color_for(&g));
            let low = Color {
                r: col.r * 0.4,
                g: col.g * 0.4,
                b: col.b * 0.4,
                a: 1.0,
            };
            let tile = container(
                text(g.to_uppercase())
                    .font(style::BLACK)
                    .size(17)
                    .color(Color::WHITE),
            )
            .center_x(tw)
            .center_y(th)
            .style(move |_| container::Style {
                border: Border {
                    color: if lit { Color::WHITE } else { white(40) },
                    width: if lit { 3.0 } else { 2.0 },
                    radius: 10.0.into(),
                },
                ..fill_gradient(2.3, low, col)
            });
            let grow = if lit { (tw + 10.0) / tw } else { 1.0 };
            let movies = lib.movies.iter().any(|i| i.genres.contains(&g));
            r = r.push(
                mouse_area(iced::widget::float(tile).scale(grow))
                    .on_press(Msg::GenreTile(g, movies))
                    .on_enter(Msg::Hover(Some(key)))
                    .on_exit(Msg::Hover(None))
                    .interaction(iced::mouse::Interaction::Pointer),
            );
        }
        Some(
            container(r)
                .padding(Padding::default().left(SIDE).top(22.0))
                .into(),
        )
    }

    /// YouTube: filter chips over one big grid of videos.
    pub(super) fn home_grid<'a>(
        &'a self,
        lib: &'a api::Library,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let genres: Vec<String> = if self.layout.shows("genres") {
            let mut g = lib.genres();
            g.retain(|g| self.layout.shows(&format!("genre:{g}")));
            g.into_iter().take(8).collect()
        } else {
            Vec::new()
        };
        let mut chips: Vec<String> = vec!["All".into(), "Movies".into(), "TV shows".into()];
        chips.extend(genres.iter().cloned());
        let chip = if self.chip >= chips.len() {
            0
        } else {
            self.chip
        };
        let mut page: Column<'a, Msg> = column![
            space().height(16),
            container(widgets::segmented(
                chips
                    .into_iter()
                    .enumerate()
                    .map(|(i, l)| (l, None, chip == i, Msg::Chip(i)))
                    .collect()
            ))
            .padding(Padding::default().left(SIDE - 22.0)),
            space().height(18),
        ];
        let avail = w - 2.0 * SIDE;
        if chip == 0 && self.layout.shows("continue") {
            let cont = self.continue_cards(lib, env.progress);
            if !cont.is_empty() {
                page = page
                    .push(
                        container(heading("Continue watching"))
                            .padding(Padding::default().left(SIDE)),
                    )
                    .push(space().height(10))
                    .push(cards::grid_fit(
                        env.images,
                        &cont,
                        avail,
                        self.hot(),
                        "grid-cont",
                        SIDE,
                        true,
                    ))
                    .push(space().height(8));
            }
        }
        let items: Vec<&Item> = match chip {
            0 => shuffled(lib.all(), "grid", &self.shuffle),
            1 => shuffled(lib.movies.iter(), "grid", &self.shuffle),
            2 => shuffled(lib.series.iter(), "grid", &self.shuffle),
            n => {
                let g = &genres[n - 3];
                shuffled(
                    lib.all().filter(|i| i.genres.contains(g)),
                    "grid",
                    &self.shuffle,
                )
            }
        };
        let cards: Vec<Card> = self.poster_cards(items.into_iter(), env.progress);
        page.push(cards::grid_fit(
            env.images,
            &cards,
            avail,
            self.hot(),
            "grid",
            SIDE,
            false,
        ))
        .into()
    }

    /// YouTube and Apple TV: Home, Movies and TV shows down the left, with
    /// My list under them.
    pub(super) fn nav_sidebar<'a>(&'a self, env: Env<'a>) -> Element<'a, Msg> {
        let skin = theme::skin();
        let searching = !self.search.is_empty();
        let rad = if skin == Skin::Apple { 8.0 } else { 10.0 };
        let entry = |ic: Icon, label: &str, selected: bool, msg: Msg| -> Element<'a, Msg> {
            let col = if skin == Skin::Apple && selected {
                c(theme::ACCENT())
            } else {
                style::TEXT()
            };
            let mut t = text(label.to_owned()).size(14.5).color(style::TEXT());
            if selected {
                t = t.font(BOLD);
            }
            button(
                row![container(icon::icon(ic, col, 18.0)).center_x(28), t,]
                    .spacing(14)
                    .align_y(Vertical::Center),
            )
            .width(Length::Fill)
            .height(40)
            .padding([0, 8])
            .style(move |_, s| {
                let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
                let hi = c(theme::SURFACE_HI());
                button::Style {
                    background: if selected {
                        Some(Background::Color(hi))
                    } else if hot {
                        Some(Background::Color(hi.scale_alpha(0.6)))
                    } else {
                        None
                    },
                    border: iced::border::rounded(rad),
                    ..button::Style::default()
                }
            })
            .on_press(msg)
            .into()
        };
        let mut nav = vec![
            (Icon::Home, "Home", View::Home),
            (Icon::Film, "Movies", View::Movies),
            (Icon::Tv, "TV Shows", View::Series),
        ];
        if !self.discover.shown.is_empty() {
            nav.push((Icon::List, "Streaming", View::Streaming));
        }
        let mut col: Column<'a, Msg> = column![].spacing(2);
        for (ic, label, v) in nav {
            let on = !searching && self.view == v;
            col = col.push(entry(ic, label, on, Msg::Nav(v)));
        }
        if let Some(lib) = self.lib.as_deref() {
            let prefix = self.prefix();
            let mine: Vec<&Item> = self
                .watchlist
                .ids(&prefix)
                .filter_map(|id| lib.find(id))
                .take(12)
                .collect();
            if !mine.is_empty() {
                col = col
                    .push(space().height(10))
                    .push(super::details::hline())
                    .push(space().height(12))
                    .push(text("My list").font(BOLD).size(15))
                    .push(space().height(4));
                for i in mine {
                    let url = i
                        .thumb
                        .as_deref()
                        .or(i.backdrop.as_deref())
                        .or(i.poster.as_deref());
                    col = col.push(
                        button(
                            row![
                                container(cards::art(env.images, url, "", 36.0, 22.0, 4.0))
                                    .center_x(28),
                                text(i.title.clone())
                                    .size(13.5)
                                    .color(style::TEXT())
                                    .wrapping(text::Wrapping::None),
                            ]
                            .spacing(14)
                            .align_y(Vertical::Center),
                        )
                        .width(Length::Fill)
                        .height(34)
                        .padding([0, 8])
                        .style(|_, s| {
                            let hot =
                                matches!(s, button::Status::Hovered | button::Status::Pressed);
                            button::Style {
                                background: hot.then(|| {
                                    Background::Color(c(theme::SURFACE_HI()).scale_alpha(0.6))
                                }),
                                border: iced::border::rounded(8),
                                ..button::Style::default()
                            }
                        })
                        .on_press(Msg::Open(i.id.clone())),
                    );
                }
            }
        }
        let fill = if skin == Skin::Apple {
            c(theme::SURFACE())
        } else {
            style::BG()
        };
        let line = skin == Skin::Apple;
        container(col)
            .width(216)
            .height(Length::Fill)
            .padding([14, 12])
            .clip(true)
            .style(move |_| container::Style {
                background: Some(Background::Color(fill)),
                border: Border {
                    color: if line {
                        c(theme::LINE())
                    } else {
                        Color::TRANSPARENT
                    },
                    width: if line { 1.0 } else { 0.0 },
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            })
            .into()
    }
}
