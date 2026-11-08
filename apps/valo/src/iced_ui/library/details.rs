//! The movie and series pages: a banner with the artwork, what it is and
//! the play button; then the cast and similar titles, or the episodes.
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, mouse_area, row, space, stack, text};
use iced::{Background, Border, Color, Element, Length, Padding};

use super::cards::{self, SIDE};
use super::{picker, pill, Env, Library, Msg, SeasonOpt, Tab, View};
use crate::api::{self, Episode, Item};
use crate::iced_ui::icon::{self, Icon};
use crate::iced_ui::style::{self, black, BOLD, MONO};
use crate::iced_ui::widgets;
use crate::ui::browse::details::{bytes, date_short, similar, split_names, years};
use crate::ui::browse::{next_up, play_label};
use crate::util::fmt_runtime;

/// The page's text column: its left edge and width.
fn column_of(w: f32) -> (f32, f32) {
    let inner = (w - 2.0 * 56.0).min(1400.0);
    ((w - inner) / 2.0, inner)
}

impl Library {
    /// The artwork across the page, dimmed so text reads over it, fading
    /// into the page at the bottom.
    fn backdrop<'a>(&self, item: &Item, env: Env<'a>, w: f32, h: f32) -> Element<'a, Msg> {
        let bg = style::BG();
        let art = item.backdrop.as_deref().or(item.poster.as_deref());
        stack![
            cards::art(env.images, art, &item.title, w, h, 0.0),
            container(space())
                .width(w)
                .height(h)
                .style(|_| container::Style {
                    background: Some(Background::Color(black(125))),
                    ..container::Style::default()
                }),
            container(
                container(space())
                    .width(w)
                    .height(h * 0.45)
                    .style(move |_| {
                        super::fill_gradient(std::f32::consts::PI, Color { a: 0.0, ..bg }, bg)
                    })
            )
            .height(h)
            .align_y(Vertical::Bottom),
        ]
        .into()
    }

    /// The bookmark button: on My list or not.
    fn bookmark<'a>(&self, item: &Item) -> Element<'a, Msg> {
        let on = self.watchlist.contains(&self.key(&item.id));
        let label = if on { "On My list" } else { "My list" };
        let ic = if on { Icon::BookmarkOn } else { Icon::Bookmark };
        pill(label, Some(ic), false, Some(Msg::Bookmark(item.id.clone())))
    }

    /// Names that open their page ("Directed by A, B").
    fn people<'a>(&self, lead: &str, names: &[&str]) -> Element<'a, Msg> {
        let mut r = row![text(lead.to_owned()).size(14).color(style::MUTED())]
            .spacing(6)
            .align_y(Vertical::Center);
        for (i, n) in names.iter().enumerate() {
            let sep = if i + 1 < names.len() { "," } else { "" };
            r = r.push(
                mouse_area(text(format!("{n}{sep}")).size(14).color(style::TEXT()))
                    .interaction(iced::mouse::Interaction::Pointer)
                    .on_press(Msg::Nav(View::Person((*n).to_owned()))),
            );
        }
        r.wrap().into()
    }

    pub(super) fn movie_page<'a>(
        &'a self,
        lib: &'a api::Library,
        item: &'a Item,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let h = (w * 0.5).clamp(520.0, 700.0);
        let (left, inner) = column_of(w);
        let show_poster = inner >= 860.0;
        let poster_w = if show_poster {
            (inner * 0.22).clamp(220.0, 320.0)
        } else {
            0.0
        };

        let mut kinds = vec!["Movie".to_owned()];
        if !item.genres.is_empty() {
            kinds.push(item.genres.join(" · "));
        }
        let mut words = column![
            widgets::label(&kinds.join("   ·   "), 11.5),
            text(item.title.clone()).font(BOLD).size(50),
        ]
        .spacing(8);
        if !item.overview.is_empty() {
            words = words.push(
                text(format!("“{}”", item.overview))
                    .size(17)
                    .color(style::TEXT().scale_alpha(0.75)),
            );
        }
        let mut facts: Vec<String> = Vec::new();
        if let Some(s) = item.score {
            facts.push(match &item.score_source {
                Some(src) => format!("★ {s:.1} {src}"),
                None => format!("★ {s:.1}"),
            });
        }
        facts.extend(item.year.map(|y| y.to_string()));
        facts.extend(item.duration.filter(|d| *d > 0.0).map(fmt_runtime));
        facts.extend(item.rating.clone());
        if let Some(q) = item.file.as_ref().and_then(|f| f.quality.clone()) {
            facts.push(q);
        }
        if !facts.is_empty() {
            words = words.push(text(facts.join("   ·   ")).size(15));
        }
        let (label, restart) = play_label(item, env.progress, &|i| self.key(i));
        let mut buttons = row![pill(
            &label,
            Some(Icon::Play),
            true,
            (!item.unavailable).then(|| Msg::Play(item.id.clone(), None, false)),
        )]
        .spacing(10);
        if restart {
            buttons = buttons.push(pill(
                "Start over",
                None,
                false,
                Some(Msg::Play(item.id.clone(), None, true)),
            ));
        }
        buttons = buttons.push(self.bookmark(item));
        words = words.push(container(buttons).padding(Padding::default().top(10.0)));
        if let Some(d) = &item.director {
            words = words.push(self.people("Directed by", &split_names(d)));
        }

        let mut top = row![].spacing(40).align_y(Vertical::Center);
        if show_poster {
            top = top.push(cards::art(
                env.images,
                item.poster.as_deref(),
                &item.title,
                poster_w,
                poster_w * 1.5,
                8.0,
            ));
        }
        top = top.push(words.width(Length::Fill));
        let banner = stack![
            self.backdrop(item, env, w, h),
            container(top)
                .width(w)
                .height(h)
                .padding(Padding::default().left(left).right(left).top(40.0))
                .align_y(Vertical::Center),
        ];

        let mut page = column![banner];
        if !item.cast.is_empty() {
            page = page.push(self.cast(item, env));
        }
        let more = similar(lib, item, 20, &self.shuffle);
        if !more.is_empty() {
            let cards = self.poster_cards(more.into_iter(), env.progress);
            page = page.push(cards::row_of(
                env.images,
                "More like this",
                &cards,
                false,
                self.hovered.as_deref(),
                None,
                w,
            ));
        }
        if let Some(f) = &item.file {
            page = page.push(file_facts(f, left));
        }
        page.into()
    }

    /// The cast with their photos, in a row that scrolls sideways.
    fn cast<'a>(&self, item: &Item, env: Env<'a>) -> Element<'a, Msg> {
        let mut strip = row![space().width(SIDE - 18.0)].spacing(18);
        for m in item.cast.iter().take(30) {
            let photo = cards::art(env.images, m.photo.as_deref(), &m.name, 96.0, 96.0, 48.0);
            let mut who = column![
                photo,
                text(m.name.clone())
                    .size(13)
                    .font(BOLD)
                    .width(110)
                    .align_x(Horizontal::Center),
            ]
            .spacing(6)
            .align_x(Horizontal::Center)
            .width(110);
            if let Some(ch) = &m.character {
                who = who.push(
                    text(ch.clone())
                        .size(12)
                        .color(style::MUTED())
                        .width(110)
                        .align_x(Horizontal::Center),
                );
            }
            strip = strip.push(who);
        }
        strip = strip.push(space().width(SIDE - 18.0));
        column![
            container(text("Cast").font(BOLD).size(cards::row_title()))
                .padding(Padding::default().left(SIDE).top(18.0).bottom(10.0)),
            iced::widget::scrollable(strip).direction(
                iced::widget::scrollable::Direction::Horizontal(
                    iced::widget::scrollable::Scrollbar::new()
                        .width(0)
                        .scroller_width(0),
                )
            ),
        ]
        .into()
    }

    pub(super) fn series_page<'a>(
        &'a self,
        lib: &'a api::Library,
        item: &'a Item,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let season = if self.season < item.seasons.len() {
            self.season
        } else {
            next_up(item, env.progress, &|i| self.key(i)).map_or(0, |n| n.si)
        };
        let h = (w * 0.42).clamp(460.0, 640.0);
        let (left, inner) = column_of(w);

        let mut meta = Vec::new();
        if !item.genres.is_empty() {
            meta.push(
                item.genres
                    .iter()
                    .take(2)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" · "),
            );
        }
        meta.extend(years(item));
        let n = item.season_count();
        meta.push(format!("{n} Season{}", if n == 1 { "" } else { "s" }));
        meta.extend(item.rating.clone());
        let mut words = column![
            text(item.title.clone()).font(BOLD).size(50),
            text(meta.join("   ·   "))
                .size(14)
                .color(style::TEXT().scale_alpha(0.85)),
        ]
        .spacing(10)
        .width(inner.min(600.0));
        if !item.overview.is_empty() {
            words = words.push(
                text(super::shorten(&item.overview, 420))
                    .size(15)
                    .color(style::TEXT().scale_alpha(0.85)),
            );
        }
        let (label, _) = play_label(item, env.progress, &|i| self.key(i));
        let mut buttons = row![
            pill(
                &label,
                Some(Icon::Play),
                true,
                (!item.unavailable).then(|| Msg::Play(item.id.clone(), None, false)),
            ),
            self.bookmark(item),
        ]
        .spacing(12)
        .align_y(Vertical::Center);
        if item.seasons.len() > 1 {
            let opts: Vec<SeasonOpt> = item
                .seasons
                .iter()
                .enumerate()
                .map(|(i, s)| SeasonOpt(i, s.label()))
                .collect();
            let current = opts.get(season).cloned();
            buttons = buttons.push(picker(iced::widget::pick_list(opts, current, Msg::Season)));
        }
        words = words.push(container(buttons).padding(Padding::default().top(8.0)));
        if !item.creators.is_empty() {
            let names: Vec<&str> = item.creators.iter().map(String::as_str).collect();
            words = words.push(self.people("Created by", &names));
        }
        let banner = stack![
            self.backdrop(item, env, w, h),
            container(words)
                .width(w)
                .height(h)
                .padding(Padding::default().left(left).bottom(24.0))
                .align_y(Vertical::Bottom),
        ];

        let mut tabs = row![].spacing(32);
        for (t, label) in [(Tab::Episodes, "Episodes"), (Tab::Related, "Related")] {
            tabs = tabs.push(section_tab(label, self.tab == t, Msg::Tab(t)));
        }
        let rule = container(space())
            .width(Length::Fill)
            .height(1)
            .style(|_| container::Style {
                background: Some(Background::Color(style::LINE())),
                ..container::Style::default()
            });
        let body: Element<Msg> = match self.tab {
            Tab::Episodes => match item.seasons.get(season) {
                Some(s) => self.episodes(item, &s.episodes, env, inner),
                None => text("No episodes yet.").color(style::MUTED()).into(),
            },
            Tab::Related => {
                let more = similar(lib, item, 20, &self.shuffle);
                if more.is_empty() {
                    text("Nothing similar in your library yet.")
                        .color(style::MUTED())
                        .into()
                } else {
                    let cards = self.poster_cards(more.into_iter(), env.progress);
                    cards::grid_at(
                        env.images,
                        &cards,
                        inner,
                        self.hovered.as_deref(),
                        "related",
                        0.0,
                    )
                }
            }
        };
        column![
            banner,
            container(tabs).padding(Padding::default().left(left).top(18.0)),
            rule,
            container(body).padding(Padding::default().left(left).right(left).top(28.0)),
        ]
        .into()
    }

    /// A season's episodes, in rows of up to four.
    fn episodes<'a>(
        &self,
        series: &Item,
        eps: &[Episode],
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let n = eps.len();
        let gap = 24.0;
        let cols = ((w + gap) / (300.0 + gap)).floor().clamp(1.0, 4.0);
        let cw = (w - gap * (cols - 1.0)) / cols;
        let mut list = column![
            text(format!("{n} episode{}", if n == 1 { "" } else { "s" }))
                .size(14)
                .color(style::MUTED())
        ]
        .spacing(26);
        for chunk in eps.chunks(cols as usize) {
            let mut r = row![].spacing(gap);
            for ep in chunk {
                r = r.push(self.episode_card(series, ep, cw, env));
            }
            list = list.push(r);
        }
        list.into()
    }

    fn episode_card<'a>(
        &self,
        series: &Item,
        ep: &Episode,
        w: f32,
        env: Env<'a>,
    ) -> Element<'a, Msg> {
        let h = w * 9.0 / 16.0;
        let key = format!("ep\u{1f}{}", ep.id);
        let hot = self.hovered.as_deref() == Some(key.as_str());
        let art = ep.still.as_deref().or(series.backdrop.as_deref());
        let mut layers: Vec<Element<Msg>> = vec![cards::art(env.images, art, &ep.title, w, h, 6.0)];
        if let Some(e) = env.progress.get(&self.key(&ep.id)) {
            let f = if e.watched() { 1.0 } else { e.fraction() };
            layers.push(cards::progress_bar(f, w));
        }
        if ep.unavailable || series.unavailable {
            layers.push(
                container(
                    container(text("MOVING").font(BOLD).size(9.5).color(Color::WHITE))
                        .padding([2, 6])
                        .style(|_| container::Style {
                            background: Some(Background::Color(Color::from_rgb8(200, 40, 40))),
                            border: iced::border::rounded(3),
                            ..container::Style::default()
                        }),
                )
                .padding(8)
                .into(),
            );
        } else if hot {
            layers.push(
                container(space())
                    .width(w)
                    .height(h)
                    .style(|_| container::Style {
                        border: Border {
                            color: style::TEXT(),
                            width: 2.0,
                            radius: 6.0.into(),
                        },
                        ..container::Style::default()
                    })
                    .into(),
            );
            layers.push(
                container(
                    button(icon::icon(Icon::Play, Color::WHITE, 56.0))
                        .padding(0)
                        .style(|_, s| button::Style {
                            background: Some(Background::Color(black(
                                if matches!(s, button::Status::Hovered) {
                                    230
                                } else {
                                    170
                                },
                            ))),
                            border: iced::border::rounded(28),
                            ..button::Style::default()
                        })
                        .on_press(Msg::Play(series.id.clone(), Some(ep.id.clone()), false)),
                )
                .center(Length::Fill)
                .into(),
            );
        }
        let still = mouse_area(stack(layers).width(w).height(h))
            .on_enter(Msg::Hover(Some(key)))
            .on_exit(Msg::Hover(None));
        let mut col = column![
            still,
            text(format!("{}. {}", ep.number, ep.title))
                .font(BOLD)
                .size(15)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(8)
        .width(w);
        if !ep.overview.is_empty() {
            col = col.push(
                text(super::shorten(&ep.overview, 160))
                    .size(12.5)
                    .color(style::MUTED()),
            );
        }
        let mut facts = Vec::new();
        facts.extend(
            ep.duration
                .filter(|d| *d > 0.0)
                .map(|d| format!("{}min", (d / 60.0).round())),
        );
        facts.extend(ep.aired.as_deref().map(date_short));
        if !facts.is_empty() {
            col = col.push(text(facts.join("  ·  ")).size(12).color(style::MUTED()));
        }
        col.into()
    }
}

/// A section tab under the banner: underlined when it's open.
fn section_tab<'a>(label: &str, on: bool, msg: Msg) -> Element<'a, Msg> {
    let line = container(space())
        .width(Length::Fill)
        .height(2)
        .style(move |_| container::Style {
            background: on.then_some(Background::Color(style::TEXT())),
            ..container::Style::default()
        });
    button(
        column![
            container(text(label.to_owned()).size(15)).padding([10, 0]),
            line,
        ]
        .width(iced::Length::Shrink),
    )
    .padding(0)
    .style(move |_, s| {
        let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: None,
            text_color: if on || hot {
                style::TEXT()
            } else {
                style::MUTED()
            },
            ..button::Style::default()
        }
    })
    .on_press(msg)
    .into()
}

/// About the video file: quality, format, size.
fn file_facts<'a>(f: &api::FileInfo, left: f32) -> Element<'a, Msg> {
    let mut rows = column![text("File").font(BOLD).size(cards::row_title())].spacing(6);
    let mut add = |k: &str, v: Option<String>| {
        if let Some(v) = v {
            rows = std::mem::replace(&mut rows, column![]).push(
                row![
                    text(k.to_owned())
                        .font(MONO)
                        .size(11.5)
                        .color(style::MUTED())
                        .width(120),
                    text(v).size(13.5),
                ]
                .align_y(Vertical::Center),
            );
        }
    };
    add("Name", f.name.clone());
    add("Quality", f.quality.clone());
    add(
        "Format",
        Some(
            [
                f.container.clone(),
                f.video_codec.clone(),
                f.audio_codec.clone(),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · "),
        )
        .filter(|s| !s.is_empty()),
    );
    add("Size", f.size.map(bytes));
    container(rows)
        .padding(Padding::default().left(left).top(28.0))
        .into()
}
