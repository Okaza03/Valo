//! The movie and series pages of the other app styles: a backdrop across
//! the top or beside the text,
//! a numbered episode list, and YouTube's watch page.
use iced::alignment::Vertical;
use iced::widget::{column, container, mouse_area, row, space, stack, text, Column};
use iced::{Background, Border, Color, Element, Length, Padding};

use super::cards::{self, SIDE};
use super::details::{
    clamp_text, gap, link, people_links, play_look, tag, title_art, watchers_row,
};
use super::{avatar, fill_gradient, Env, Library, Msg};
use crate::api::{self, FileInfo, Item, Kind};
use crate::iced_ui::icon::Icon;
use crate::iced_ui::style::{self, black, c, BOLD};
use crate::ui::browse::details::{minutes, play_colors, similar, years, GOLD, TEAL};
use crate::ui::browse::details::{watchers_episode_text, watchers_season_notes};
use crate::ui::browse::{episode_watched, makers, play_label, seen_note};
use crate::ui::theme::{self, Details, INK, LINE, PRIMARY, SURFACE_HI, TEXT};
use crate::util::fmt_runtime;

/// A fill that's `a` opaque.
fn tint(col: Color, a: f32) -> Color {
    Color { a, ..col }
}

/// The played part along a thumbnail's bottom edge.
fn edge<'a>(f: f32, w: f32) -> Element<'a, Msg> {
    let played = c(theme::played());
    container(stack![
        container(space())
            .width(w)
            .height(3)
            .style(|_| container::Style {
                background: Some(Background::Color(style::white(70))),
                ..container::Style::default()
            }),
        container(space())
            .width(w * f.clamp(0.0, 1.0))
            .height(3)
            .style(move |_| container::Style {
                background: Some(Background::Color(played)),
                ..container::Style::default()
            }),
    ])
    .height(Length::Fill)
    .align_y(Vertical::Bottom)
    .into()
}

impl Library {
    /// The top of a movie or series page for the Hero and Split layouts:
    /// artwork, title, details, buttons and the story.
    pub(super) fn styled_header<'a>(
        &'a self,
        item: &'a Item,
        file: Option<&FileInfo>,
        runtime: Option<f64>,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let split = theme::spec().layout.details == Details::Split;
        let h = if split {
            (w * 0.42).clamp(460.0, 660.0)
        } else {
            (w * 0.5).clamp(480.0, 780.0)
        };
        let bg = style::BG();
        let url = item.backdrop.as_deref().or(item.poster.as_deref());
        let mut layers: Vec<Element<'a, Msg>> = Vec::new();
        if split {
            // Prime Video: the picture on the right, fading into the page
            let aw = w * 0.64;
            layers.push(
                container(stack![
                    cards::art(env.images, url, &item.title, aw, h, 0.0),
                    container(space())
                        .width(aw * 0.5)
                        .height(h)
                        .style(move |_| fill_gradient(
                            std::f32::consts::FRAC_PI_2,
                            bg,
                            tint(bg, 0.0)
                        )),
                ])
                .padding(Padding::default().left(w - aw))
                .into(),
            );
        } else {
            layers.push(cards::art(env.images, url, &item.title, w, h, 0.0));
            layers.push(
                container(space())
                    .width(w * 0.6)
                    .height(h)
                    .style(move |_| {
                        fill_gradient(std::f32::consts::FRAC_PI_2, tint(bg, 0.88), tint(bg, 0.0))
                    })
                    .into(),
            );
        }
        layers.push(
            container(
                container(space())
                    .width(w)
                    .height(h * 0.35)
                    .style(move |_| fill_gradient(std::f32::consts::PI, tint(bg, 0.0), bg)),
            )
            .height(h)
            .align_y(Vertical::Bottom)
            .into(),
        );
        layers.push(
            container(link(
                "Back",
                13.0,
                style::TEXT().scale_alpha(0.85),
                Some((Icon::ChevronLeft, false)),
                Msg::Back,
            ))
            .padding(Padding::default().left(SIDE - 8.0).top(20.0))
            .into(),
        );

        let tw = if split {
            w * 0.42
        } else {
            (w * 0.45).clamp(360.0, 640.0)
        };
        let mut words: Column<'a, Msg> = column![
            title_art(
                item,
                env,
                (tw.min(520.0), 130.0),
                if split { 44.0 } else { 52.0 }
            ),
            gap(12.0),
        ]
        .width(tw);
        let mut facts = row![].spacing(12).align_y(Vertical::Center);
        if let Some(s) = item.score {
            let src = item.score_source.as_deref().unwrap_or("Score");
            facts = facts.push(
                text(format!("{src} {s:.1}"))
                    .font(BOLD)
                    .size(15)
                    .color(c(if split { theme::ACCENT() } else { GOLD() })),
            );
        }
        let mut bits: Vec<String> = Vec::new();
        match item.kind {
            Kind::Movie => {
                bits.extend(item.year.map(|y| y.to_string()));
                bits.extend(runtime.map(fmt_runtime));
            }
            Kind::Series => {
                bits.extend(years(item));
                let n = item.season_count();
                bits.push(format!("{n} season{}", if n == 1 { "" } else { "s" }));
            }
        }
        for b in bits {
            facts = facts.push(text(b).size(15).color(style::TEXT().scale_alpha(0.85)));
        }
        if let Some(r) = &item.rating {
            facts = facts.push(tag(r));
        }
        if let Some(q) = file.and_then(|f| f.quality.as_deref()) {
            facts = facts.push(tag(q));
        }
        words = words.push(facts);
        if !item.genres.is_empty() {
            words = words
                .push(gap(4.0))
                .push(text(item.genres.join(" · ")).size(14).color(style::MUTED()));
        }
        let (label, resume) = play_label(item, env.progress, &|i| self.key(i));
        let (bh, min_w) = if split { (52.0, 220.0) } else { (46.0, 150.0) };
        let (ic, colors) = play_look(item, play_colors());
        let mut buttons = row![super::details::fill_button(
            &label,
            ic,
            colors,
            bh,
            min_w,
            0.0,
            (!item.unavailable).then(|| Msg::Play(item.id.clone(), None, false)),
        )]
        .spacing(12)
        .align_y(Vertical::Center);
        if resume && item.kind == Kind::Movie {
            buttons = buttons.push(super::details::fill_button(
                "Start over",
                None,
                (SURFACE_HI(), TEXT()),
                bh,
                0.0,
                0.0,
                Some(Msg::Play(item.id.clone(), None, true)),
            ));
        }
        buttons = buttons.push(self.bookmark(item));
        words = words.push(gap(18.0)).push(buttons);
        if let Some(wr) = watchers_row(self.watchers_of(item)) {
            words = words.push(gap(14.0)).push(wr);
        }
        if !item.overview.is_empty() {
            words = words.push(gap(18.0)).push(clamp_text(
                &item.overview,
                16.0,
                style::TEXT().scale_alpha(0.9),
                if split { 5 } else { 3 },
                tw,
            ));
        }
        let (label, names): (&str, Vec<&str>) = match item.kind {
            Kind::Movie => ("Director", makers(item).collect()),
            Kind::Series => (
                "Created by",
                item.creators.iter().map(String::as_str).collect(),
            ),
        };
        if !names.is_empty() {
            words = words.push(gap(12.0)).push(
                row![
                    text(label).size(14).color(style::MUTED()),
                    people_links(&names, 14.0),
                ]
                .spacing(10)
                .align_y(Vertical::Center),
            );
        }
        let words = container(words)
            .height(h)
            .padding(Padding::default().left(SIDE + 8.0).top(56.0).bottom(28.0))
            .align_y(if split {
                Vertical::Center
            } else {
                Vertical::Bottom
            });
        layers.push(words.into());
        stack(layers).width(w).height(h).into()
    }

    /// Episodes as a numbered list: number, picture, title and length, story.
    pub(super) fn episode_list<'a>(
        &'a self,
        item: &'a Item,
        si: usize,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let Some(season) = item.seasons.get(si) else {
            return space().into();
        };
        let (tw, th) = (220.0, 124.0);
        let key = |i: &str| self.key(i);
        let mut col: Column<'a, Msg> = column![];
        if let Some(note) = seen_note(item, si, env.progress, &key) {
            col = col
                .push(text(note).font(BOLD).size(14).color(c(TEAL())))
                .push(gap(12.0));
        }
        let others = self.watchers_of(item);
        for note in watchers_season_notes(others, item, si) {
            col = col
                .push(text(note).font(BOLD).size(14).color(style::MUTED()))
                .push(gap(12.0));
        }
        for ep in &season.episodes {
            let seen = episode_watched(env.progress, &key, &ep.id);
            let hk = format!("epl\u{1f}{}", ep.id);
            let lit = self.hovered.as_deref() == Some(hk.as_str());
            let art = ep.still.as_deref().or(item.backdrop.as_deref());
            let mut thumb: Vec<Element<'a, Msg>> =
                vec![cards::art(env.images, art, &ep.title, tw, th, 4.0)];
            if let Some(e) = env.progress.get(&self.key(&ep.id)) {
                thumb.push(edge(if seen { 1.0 } else { e.fraction() }, tw));
            }
            if lit {
                thumb.push(
                    container(
                        container(crate::iced_ui::icon::icon(Icon::Play, Color::WHITE, 20.0))
                            .center(44)
                            .style(|_| container::Style {
                                background: Some(Background::Color(black(150))),
                                border: iced::border::rounded(22),
                                ..container::Style::default()
                            }),
                    )
                    .center(Length::Fill)
                    .into(),
                );
            }
            let mut head = row![text(ep.title.clone()).font(BOLD).size(16)]
                .spacing(8)
                .align_y(Vertical::Center);
            head = head.push(space::horizontal());
            let mut right = column![].align_x(iced::alignment::Horizontal::Right);
            if let Some(d) = ep.duration.filter(|d| *d > 0.0) {
                right = right.push(text(minutes(d)).size(15));
            }
            if seen {
                right = right.push(text("Watched").font(BOLD).size(13).color(c(TEAL())));
            }
            if let Some(who) = watchers_episode_text(others, item, &ep.id) {
                right = right.push(text(who).size(13).color(style::MUTED()));
            }
            head = head.push(right);
            let mut info: Column<'a, Msg> = column![head].spacing(8).width(Length::Fill);
            if !ep.overview.is_empty() {
                info = info.push(clamp_text(
                    &ep.overview,
                    14.0,
                    style::MUTED(),
                    3,
                    w - 60.0 - tw - 36.0,
                ));
            }
            let line = row![
                container(text(ep.number.to_string()).size(24).color(style::MUTED()))
                    .center_x(60)
                    .center_y(th),
                stack(thumb).width(tw).height(th),
                info,
            ]
            .spacing(20)
            .padding(Padding::default().top(14.0).bottom(14.0).right(16.0));
            let line = container(line).width(w).style(move |_| container::Style {
                background: lit.then(|| Background::Color(c(SURFACE_HI()).scale_alpha(0.7))),
                border: Border {
                    radius: 6.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });
            col = col
                .push(
                    mouse_area(line)
                        .on_press(Msg::Play(item.id.clone(), Some(ep.id.clone()), false))
                        .on_enter(Msg::Hover(Some(hk)))
                        .on_exit(Msg::Hover(None))
                        .interaction(iced::mouse::Interaction::Pointer),
                )
                .push(super::details::hline());
        }
        col.into()
    }

    /// YouTube's watch page: the big picture, title, channel line and
    /// description, with the episodes (or more movies) in a list beside it.
    pub(super) fn watch_page<'a>(
        &'a self,
        lib: &'a api::Library,
        item: &'a Item,
        file: Option<&FileInfo>,
        runtime: Option<f64>,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let pad = 24.0;
        let side_w = if w > 1100.0 { 400.0 } else { 0.0 };
        let g = if side_w > 0.0 { 24.0 } else { 0.0 };
        let main_w = (w - 2.0 * pad - side_w - g).min(1280.0);
        let main = self.watch_main(item, file, runtime, env, main_w);
        let body: Element<'a, Msg> = if side_w > 0.0 {
            row![
                main,
                space().width(g),
                self.watch_side(lib, item, env, side_w)
            ]
            .into()
        } else {
            column![main, gap(20.0), self.watch_side(lib, item, env, main_w)].into()
        };
        container(body)
            .padding(Padding::default().left(pad).top(20.0))
            .into()
    }

    fn watch_main<'a>(
        &'a self,
        item: &'a Item,
        file: Option<&FileInfo>,
        runtime: Option<f64>,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let h = w * 9.0 / 16.0;
        let lit = self.hovered.as_deref() == Some("watch-player");
        let url = item.backdrop.as_deref().or(item.poster.as_deref());
        let mut player: Vec<Element<'a, Msg>> = vec![
            cards::art(env.images, url, &item.title, w, h, 12.0),
            container(space())
                .width(w)
                .height(h)
                .style(move |_| container::Style {
                    background: Some(Background::Color(black(if lit { 40 } else { 70 }))),
                    border: iced::border::rounded(12),
                    ..container::Style::default()
                })
                .into(),
            container(
                container(crate::iced_ui::icon::icon(Icon::Play, Color::WHITE, 22.0))
                    .center_x(72)
                    .center_y(50)
                    .style(move |_| container::Style {
                        background: Some(Background::Color(if lit {
                            c(theme::ACCENT())
                        } else {
                            black(200)
                        })),
                        border: iced::border::rounded(14),
                        ..container::Style::default()
                    }),
            )
            .center(Length::Fill)
            .into(),
        ];
        if item.kind == Kind::Movie {
            if let Some(e) = env
                .progress
                .get(&self.key(&item.id))
                .filter(|e| e.in_progress())
            {
                player.push(
                    container(edge(e.fraction(), w - 12.0))
                        .padding(Padding::default().left(6.0))
                        .into(),
                );
            }
        }
        let player = mouse_area(stack(player).width(w).height(h))
            .on_press(Msg::Play(item.id.clone(), None, false))
            .on_enter(Msg::Hover(Some("watch-player".into())))
            .on_exit(Msg::Hover(None))
            .interaction(iced::mouse::Interaction::Pointer);

        // the "channel" line: the server, then the buttons
        let server = self.client.info.name.clone();
        let n = item.seasons.iter().map(|s| s.episodes.len()).sum::<usize>();
        let sub = match item.kind {
            Kind::Movie => "Movie".to_owned(),
            Kind::Series => format!("{n} episodes"),
        };
        let (label, resume) = play_label(item, env.progress, &|i| self.key(i));
        let (ic, colors) = play_look(item, (PRIMARY(), INK()));
        let mut buttons = row![super::details::fill_button(
            &label,
            ic,
            colors,
            38.0,
            0.0,
            19.0,
            (!item.unavailable).then(|| Msg::Play(item.id.clone(), None, false)),
        )]
        .spacing(10);
        if resume && item.kind == Kind::Movie {
            buttons = buttons.push(super::details::fill_button(
                "Start over",
                None,
                (SURFACE_HI(), TEXT()),
                38.0,
                0.0,
                19.0,
                Some(Msg::Play(item.id.clone(), None, true)),
            ));
        }
        let on = self.watchlist.contains(&self.key(&item.id));
        buttons = buttons.push(super::details::fill_button(
            if on { "Saved" } else { "Save" },
            Some(if on { Icon::BookmarkOn } else { Icon::Bookmark }),
            (SURFACE_HI(), TEXT()),
            38.0,
            0.0,
            19.0,
            Some(Msg::Bookmark(item.id.clone())),
        ));
        let channel = row![
            avatar(&server, 20.0),
            column![
                text(server.clone()).font(BOLD).size(15),
                text(sub).size(12.5).color(style::MUTED()),
            ],
            space::horizontal(),
            buttons,
        ]
        .spacing(10)
        .align_y(Vertical::Center);

        // the description box
        let mut bits: Vec<String> = Vec::new();
        match item.kind {
            Kind::Movie => {
                bits.extend(item.year.map(|y| y.to_string()));
                bits.extend(runtime.map(fmt_runtime));
            }
            Kind::Series => bits.extend(years(item)),
        }
        bits.extend(item.rating.clone());
        bits.extend(file.and_then(|f| f.quality.clone()));
        if let Some(s) = item.score {
            bits.push(format!("★ {s:.1}"));
        }
        let mut about: Column<'a, Msg> =
            column![text(bits.join("  ·  ")).font(BOLD).size(14)].spacing(4);
        if !item.genres.is_empty() {
            about = about.push(
                text(
                    item.genres
                        .iter()
                        .map(|g| format!("#{}", g.replace(' ', "")))
                        .collect::<Vec<_>>()
                        .join(" "),
                )
                .size(13.5)
                .color(c(theme::ACCENT())),
            );
        }
        if !item.overview.is_empty() {
            about = about.push(text(item.overview.clone()).size(14.5));
        }
        let names: Vec<&str> = makers(item).collect();
        if !names.is_empty() {
            let l = if item.kind == Kind::Movie {
                "Director"
            } else {
                "Created by"
            };
            about = about.push(
                row![
                    text(l).size(13.5).color(style::MUTED()),
                    people_links(&names, 13.5)
                ]
                .spacing(8)
                .wrap(),
            );
        }
        if !item.cast.is_empty() {
            let names: Vec<&str> = item.cast.iter().take(6).map(|c| c.name.as_str()).collect();
            about = about.push(
                row![
                    text("Starring").size(13.5).color(style::MUTED()),
                    people_links(&names, 13.5)
                ]
                .spacing(8)
                .wrap(),
            );
        }
        let about = container(about)
            .width(w)
            .padding(14)
            .style(|_| container::Style {
                background: Some(Background::Color(c(SURFACE_HI()))),
                border: iced::border::rounded(12),
                ..container::Style::default()
            });
        column![
            player,
            gap(6.0),
            text(item.title.clone()).font(BOLD).size(21),
            gap(2.0),
            channel,
            gap(6.0),
            about,
        ]
        .spacing(8)
        .width(w)
        .into()
    }

    /// One row of the watch page's list: thumbnail, title and details.
    #[allow(clippy::too_many_arguments)]
    fn watch_row<'a>(
        &self,
        env: Env<'a>,
        url: Option<&str>,
        title: &str,
        sub: &str,
        idx: Option<u32>,
        frac: Option<f32>,
        w: f32,
        key: String,
        msg: Msg,
    ) -> Element<'a, Msg> {
        let lit = self.hovered.as_deref() == Some(key.as_str());
        let mut thumb: Vec<Element<'a, Msg>> =
            vec![cards::art(env.images, url, title, 156.0, 88.0, 8.0)];
        if let Some(f) = frac {
            thumb.push(edge(f, 156.0));
        }
        let mut line = row![].spacing(10).align_y(Vertical::Center);
        if let Some(n) = idx {
            line = line
                .push(container(text(n.to_string()).size(13).color(style::MUTED())).center_x(20));
        }
        line = line.push(stack(thumb).width(156).height(88)).push(
            column![
                container(text(title.to_owned()).font(BOLD).size(14))
                    .max_height(38.0)
                    .clip(true),
                text(sub.to_owned())
                    .size(12.5)
                    .color(style::MUTED())
                    .wrapping(text::Wrapping::None),
            ]
            .spacing(4)
            .width(Length::Fill),
        );
        let frame = container(line)
            .width(w)
            .height(100)
            .padding([6, 6])
            .style(move |_| container::Style {
                background: lit.then(|| Background::Color(c(SURFACE_HI()).scale_alpha(0.7))),
                border: iced::border::rounded(10),
                ..container::Style::default()
            });
        mouse_area(frame)
            .on_press(msg)
            .on_enter(Msg::Hover(Some(key)))
            .on_exit(Msg::Hover(None))
            .interaction(iced::mouse::Interaction::Pointer)
            .into()
    }

    /// Beside the watch page: the season as a playlist, or more movies.
    fn watch_side<'a>(
        &'a self,
        lib: &'a api::Library,
        item: &'a Item,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        if item.kind == Kind::Series && !item.seasons.is_empty() {
            let si = if self.season < item.seasons.len() {
                self.season
            } else {
                crate::ui::browse::next_up(item, env.progress, &|i| self.key(i)).map_or(0, |n| n.si)
            };
            let key = |i: &str| self.key(i);
            let mut col: Column<'a, Msg> = column![
                container(text(item.title.clone()).font(BOLD).size(17))
                    .padding(Padding::default().left(6.0)),
                row![
                    self.season_picker(item, si, 150.0),
                    text(format!("{} episodes", item.seasons[si].episodes.len()))
                        .size(12.5)
                        .color(style::MUTED()),
                ]
                .spacing(10)
                .align_y(Vertical::Center)
                .padding(Padding::default().left(6.0)),
                gap(6.0),
            ];
            if let Some(note) = seen_note(item, si, env.progress, &key) {
                col = col
                    .push(
                        container(text(note).font(BOLD).size(14).color(c(TEAL())))
                            .padding(Padding::default().left(6.0)),
                    )
                    .push(gap(6.0));
            }
            let others = self.watchers_of(item);
            for note in watchers_season_notes(others, item, si) {
                col = col
                    .push(
                        container(text(note).font(BOLD).size(14).color(style::MUTED()))
                            .padding(Padding::default().left(6.0)),
                    )
                    .push(gap(6.0));
            }
            for ep in &item.seasons[si].episodes {
                let seen = episode_watched(env.progress, &key, &ep.id);
                let frac =
                    env.progress
                        .get(&self.key(&ep.id))
                        .map(|e| if seen { 1.0 } else { e.fraction() });
                let mut sub = ep
                    .duration
                    .filter(|d| *d > 0.0)
                    .map(minutes)
                    .unwrap_or_default();
                if seen {
                    sub = if sub.is_empty() {
                        "Watched".into()
                    } else {
                        format!("{sub} · Watched")
                    };
                }
                if let Some(who) = watchers_episode_text(others, item, &ep.id) {
                    sub = if sub.is_empty() {
                        who
                    } else {
                        format!("{sub} · {who}")
                    };
                }
                let thumb = ep.still.as_deref().or(item.backdrop.as_deref());
                col = col.push(self.watch_row(
                    env,
                    thumb,
                    &ep.title,
                    &sub,
                    Some(ep.number),
                    frac,
                    w - 16.0,
                    format!("wr\u{1f}{}", ep.id),
                    Msg::Play(item.id.clone(), Some(ep.id.clone()), false),
                ));
            }
            return container(col)
                .width(w)
                .padding([12, 8])
                .style(|_| container::Style {
                    border: Border {
                        color: c(LINE()),
                        width: 1.0,
                        radius: 12.0.into(),
                    },
                    ..container::Style::default()
                })
                .into();
        }
        let mut col: Column<'a, Msg> = column![text("Up next").font(BOLD).size(17), gap(4.0)];
        for i in similar(lib, item, 14, &self.shuffle) {
            let sub = crate::ui::browse::short_meta(i);
            let thumb = i
                .thumb
                .as_deref()
                .or(i.backdrop.as_deref())
                .or(i.poster.as_deref());
            col = col.push(self.watch_row(
                env,
                thumb,
                &i.title,
                &sub,
                None,
                None,
                w,
                format!("wr\u{1f}{}", i.id),
                Msg::Open(i.id.clone()),
            ));
        }
        col.width(w).into()
    }
}
