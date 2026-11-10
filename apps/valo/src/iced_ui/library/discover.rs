//! What the admin's Stremio addons add: the Streaming page, the home page's catalog rows,
//! search results from beyond the server, the page of a title that isn't on
//! the server (with Request), and person pages with their filmography
//!. The state is `crate::ui::browse::discover::State`.
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{
    column, container, image as _image, row, scrollable, space, stack, text, Column,
};
use iced::{Background, Border, Color, Element, Length, Padding};

use super::cards::{self, Card, GAP, SIDE};
use super::details::{backdrop, caps, column_of, gap, kv_rows, link, panel, people_links, NARROW};
use super::{muted_center, spin_secs, Env, Library, Msg, View};
use crate::addons;
use crate::api::{self, Episode, Item, Kind};
use crate::iced_ui::icon::{self, Icon};
use crate::iced_ui::spaced::{heading, mono};
use crate::iced_ui::style::{self, black, c, BLACK, BOLD};
use crate::iced_ui::widgets;
use crate::ui::browse::details::{
    date_long, date_unix, play_colors, similar, split_names, GOLD, TEAL,
};
use crate::ui::browse::discover::FilmFilter;
use crate::ui::browse::makers;
use crate::ui::browse::people::{same, span, top, Credit, Person};
use crate::ui::theme::{ERROR, LINE, OK, WARN};
use crate::util::{fmt_runtime, Waker};

/// Titles per catalog row on the home page.
const HOME_ROW: usize = 40;

impl Library {
    /// Keep the addons' work going: catalogs, details the open page wants,
    /// cast photos, filmographies, requests, and search beyond the server.
    pub(super) fn discover_work(&mut self) {
        self.discover
            .poll(Waker::none(), &self.client, &mut self.lib);
        let Some(lib) = self.lib.clone() else {
            return;
        };
        match self.view.clone() {
            View::Details(id) => match lib.find(&id) {
                Some(item) => self.discover.want_item(item),
                None => {
                    if let Some(item) = self.discover.remote.get(&id).cloned() {
                        self.discover
                            .want_remote(Waker::none(), &self.client, &item);
                    }
                }
            },
            View::Person(name) => {
                if let Some(Ok(f)) = self.discover.films(Waker::none(), &name).cloned() {
                    if self.credits.as_ref().is_none_or(|(n, _)| *n != name) {
                        let ids = self.discover.credits(&lib, &f);
                        self.credits = Some((name, ids));
                    }
                }
            }
            _ => {}
        }
        if !self.search.trim().is_empty() {
            self.found = self.discover.search(Waker::none(), &self.search);
        }
    }

    /// This page was opened from the Streaming tab.
    pub(super) fn came_from_streaming(&self) -> bool {
        self.history.last() == Some(&View::Streaming)
    }

    /// Cards for catalog titles: ours as usual, the addon's marked as not
    /// on the server.
    pub(super) fn catalog_cards<'a>(
        &self,
        lib: &'a api::Library,
        ids: impl Iterator<Item = &'a String>,
        env: Env<'_>,
    ) -> Vec<Card> {
        let items = ids.filter_map(|id| lib.find(id).or_else(|| self.discover.remote.get(id)));
        let mut out = self.poster_cards(items, env.progress);
        for c in &mut out {
            if self.discover.remote.contains_key(&c.item) {
                c.sub = Some(match c.sub.take() {
                    Some(y) => format!("{y} · not on server"),
                    None => "Not on server".into(),
                });
            }
        }
        out
    }

    /// The catalogs as home page rows: (key, title, cards).
    pub(super) fn catalog_rows(
        &self,
        lib: &api::Library,
        env: Env<'_>,
    ) -> Vec<(String, String, Vec<Card>)> {
        self.discover
            .shown
            .iter()
            .filter(|s| self.layout.shows(&s.key))
            .map(|s| {
                let cards = self.catalog_cards(lib, s.ids.iter().take(HOME_ROW), env);
                (s.key.clone(), s.title.clone(), cards)
            })
            .collect()
    }

    /// Search's titles from beyond the server, under ours.
    pub(super) fn search_beyond<'a>(
        &'a self,
        lib: &'a api::Library,
        env: Env<'a>,
        w: f32,
        none_here: bool,
    ) -> Element<'a, Msg> {
        let (found, more) = &self.found;
        let mut col: Column<'a, Msg> = column![];
        if none_here && found.is_empty() && !more {
            return col.push(muted_center("Nothing matches")).into();
        }
        if none_here {
            col = col.push(
                container(mono("Nothing on the server matches", 12.0).color(style::MUTED()))
                    .padding(Padding::default().left(SIDE)),
            );
        }
        if !found.is_empty() {
            let cards = self.catalog_cards(lib, found.iter(), env);
            col = col
                .push(gap(28.0))
                .push(
                    row![
                        heading("Not on the server"),
                        mono(&found.len().to_string(), 13.0).color(style::MUTED())
                    ]
                    .spacing(10)
                    .align_y(Vertical::Center)
                    .padding(Padding::default().left(SIDE)),
                )
                .push(
                    container(
                        text("Open one to see what it is and ask for it.")
                            .size(14.5)
                            .color(style::MUTED()),
                    )
                    .padding(Padding::default().left(SIDE).top(8.0)),
                )
                .push(gap(12.0))
                .push(cards::grid(env.images, &cards, w, self.hot(), "beyond"));
        }
        if *more {
            col = col.push(gap(20.0)).push(
                row![
                    crate::iced_ui::icon::spinner(spin_secs(), style::MUTED(), 14.0),
                    mono("Searching beyond the server…", 11.0).color(style::MUTED()),
                ]
                .spacing(10)
                .align_y(Vertical::Center)
                .padding(Padding::default().left(SIDE)),
            );
        }
        col.into()
    }

    // ---------------------------------------------------------- streaming

    /// Everything on one streaming service (or other catalog), as a grid.
    pub(super) fn streaming_page<'a>(
        &'a self,
        lib: &'a api::Library,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let sources: Vec<String> = self
            .discover
            .sources()
            .into_iter()
            .map(str::to_owned)
            .collect();
        if sources.is_empty() {
            let body: Element<'a, Msg> = if self.discover.busy() {
                column![
                    crate::iced_ui::icon::spinner(spin_secs(), style::MUTED(), 22.0),
                    mono("Loading the catalogs", 12.0).color(style::MUTED()),
                ]
                .spacing(10)
                .align_x(Horizontal::Center)
                .into()
            } else {
                column![
                    mono("No catalogs yet", 13.0),
                    text("An admin can add them under Admin › Addons, for example Streaming Catalogs.")
                        .size(14.5)
                        .color(style::MUTED()),
                ]
                .spacing(4)
                .align_x(Horizontal::Center)
                .into()
            };
            return container(body)
                .center_x(Length::Fill)
                .padding(Padding::default().top(116.0))
                .into();
        }
        let pick = match &self.discover.service {
            Some(s) if sources.contains(s) => s.clone(),
            _ => sources[0].clone(),
        };
        let mine = self.discover.of_source(&pick);
        let count = |k: Kind| -> usize {
            mine.iter()
                .filter(|(sk, _)| *sk == k)
                .map(|(_, ids)| ids.len())
                .sum()
        };
        let (movies, series) = (count(Kind::Movie), count(Kind::Series));
        let kind = self.discover.service_kind.filter(|k| count(*k) > 0);

        let mut page: Column<'a, Msg> = column![
            column![
                caps("Streaming", c(TEAL())),
                gap(4.0),
                text(pick.clone()).font(BOLD).size(34),
                gap(4.0),
                text("Play what's already on your server, and request anything that isn't.")
                    .size(14.5)
                    .color(style::MUTED()),
            ]
            .padding(Padding::default().left(SIDE).top(36.0)),
            gap(16.0),
            container(
                scrollable(widgets::segmented(
                    sources
                        .iter()
                        .map(|s| (s.clone(), None, *s == pick, Msg::Source(s.clone())))
                        .collect(),
                ))
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::new().width(4).scroller_width(4),
                )),
            )
            .padding(Padding::default().left(SIDE - 22.0)),
        ];
        if movies > 0 && series > 0 {
            page = page.push(gap(10.0)).push(
                container(widgets::segmented(vec![
                    (
                        "All".into(),
                        Some(movies + series),
                        kind.is_none(),
                        Msg::SourceKind(None),
                    ),
                    (
                        "Movies".into(),
                        Some(movies),
                        kind == Some(Kind::Movie),
                        Msg::SourceKind(Some(Kind::Movie)),
                    ),
                    (
                        "TV shows".into(),
                        Some(series),
                        kind == Some(Kind::Series),
                        Msg::SourceKind(Some(Kind::Series)),
                    ),
                ]))
                .padding(Padding::default().left(SIDE - 22.0)),
            );
        }
        let groups: Vec<(&str, Vec<String>)> =
            [(Kind::Movie, "Movies"), (Kind::Series, "TV shows")]
                .into_iter()
                .filter(|(k, _)| kind.is_none_or(|w| w == *k))
                .map(|(k, label)| {
                    let mut ids: Vec<String> = Vec::new();
                    for (_, list) in mine.iter().filter(|(sk, _)| *sk == k) {
                        for id in list {
                            if !ids.contains(id) {
                                ids.push(id.clone());
                            }
                        }
                    }
                    (label, ids)
                })
                .filter(|(_, ids)| !ids.is_empty())
                .collect();
        let both = groups.len() > 1;
        for (label, ids) in groups {
            page = page.push(gap(22.0));
            if both {
                page = page
                    .push(
                        row![
                            heading(label),
                            mono(&ids.len().to_string(), 13.0).color(style::MUTED())
                        ]
                        .spacing(10)
                        .align_y(Vertical::Center)
                        .padding(Padding::default().left(SIDE)),
                    )
                    .push(gap(8.0));
            }
            let cards = self.catalog_cards(lib, ids.iter(), env);
            page = page.push(cards::grid(env.images, &cards, w, self.hot(), label));
        }
        page.into()
    }

    // ------------------------------------------------------ remote title

    /// A title from an addon that isn't on the server: its details and a
    /// Request button.
    pub(super) fn remote_page<'a>(
        &'a self,
        lib: &'a api::Library,
        item: &'a Item,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let st = &self.discover;
        let h = (w * 0.42).clamp(440.0, 620.0);
        let (m, inner) = column_of(w);
        let show_poster = inner >= 860.0;
        let poster_w = if show_poster {
            (inner * 0.2).clamp(200.0, 280.0)
        } else {
            0.0
        };
        let art = item.backdrop.as_deref().or(item.poster.as_deref());
        let mut layers = vec![
            backdrop(env, art, &item.title, 140, w, h),
            container(link(
                "Back",
                13.0,
                style::TEXT().scale_alpha(0.8),
                Some((Icon::ChevronLeft, false)),
                Msg::Back,
            ))
            .padding(Padding::default().left(28.0).top(22.0))
            .into(),
        ];
        if show_poster {
            let ph = poster_w * 1.5;
            layers.push(
                container(cards::art(
                    env.images,
                    item.poster.as_deref(),
                    &item.title,
                    poster_w,
                    ph,
                    8.0,
                ))
                .padding(Padding::default().left(m).top(h / 2.0 - ph / 2.0))
                .into(),
            );
        }
        let tx = m + if show_poster { poster_w + 40.0 } else { 0.0 };
        let tw = (m + inner - tx).max(100.0);
        let mut kinds = vec![match item.kind {
            Kind::Movie => "Movie".to_owned(),
            Kind::Series => "Series".to_owned(),
        }];
        if !item.genres.is_empty() {
            kinds.push(item.genres.join(" · "));
        }
        let mut words: Column<'a, Msg> = column![
            caps(&kinds.join("   ·   "), style::TEXT().scale_alpha(0.65)),
            gap(8.0),
            title_text(item, env, 50.0),
        ]
        .width(tw);
        let services = st.services(&item.id);
        if !services.is_empty() {
            let teal = c(TEAL());
            let mut r = row![text("Streaming on ").size(15).color(teal)].align_y(Vertical::Center);
            if self.came_from_streaming() {
                r = r.push(service_links(&services, &item.title, 15.0, teal));
            } else {
                r = r.push(text(services.join(", ")).size(15).color(teal));
            }
            words = words.push(gap(8.0)).push(r);
        }
        words = words.push(gap(14.0));
        if !item.overview.is_empty() {
            words = words
                .push(
                    text(item.overview.clone())
                        .size(16)
                        .color(style::TEXT().scale_alpha(0.75)),
                )
                .push(gap(14.0));
        }
        let gold = c(GOLD());
        let mut facts: Vec<Element<'a, Msg>> = Vec::new();
        let mut fact = |parts: Vec<Element<'a, Msg>>| {
            if !facts.is_empty() {
                facts.push(text("·").size(14.5).color(style::MUTED()).into());
            }
            facts.extend(parts);
        };
        if let Some(s) = item.score {
            let mut parts: Vec<Element<'a, Msg>> = vec![
                icon::icon(Icon::Star, gold, 14.0).into(),
                text(format!("{s:.1}")).size(15).into(),
            ];
            if let Some(src) = &item.score_source {
                parts.push(text(src.clone()).size(15).color(style::MUTED()).into());
            }
            fact(parts);
        }
        if let Some(y) = item.year {
            let years = match item.end_year {
                Some(e) if e != y => format!("{y}–{e}"),
                _ => y.to_string(),
            };
            fact(vec![text(years).size(15).color(gold).into()]);
        }
        if let Some(d) = item.duration {
            fact(vec![text(fmt_runtime(d)).size(15).into()]);
        }
        if let Some(d) = &item.director {
            let tags: Vec<Element<'a, Msg>> = split_names(d)
                .into_iter()
                .map(|n| tag_link(n, Msg::Nav(View::Person(n.to_owned()))))
                .collect();
            fact(tags);
        }
        words = words
            .push(
                iced::widget::Row::with_children(facts)
                    .spacing(10)
                    .align_y(Vertical::Center),
            )
            .push(gap(22.0))
            .push(self.request_line(item));
        if let Some(e) = &st.send_error {
            words = words.push(gap(8.0)).push(
                text(format!("Couldn't send the request: {e}"))
                    .size(14.5)
                    .color(style::ERROR()),
            );
        }
        layers.push(
            container(words)
                .width(w)
                .height(h)
                .padding(Padding::default().left(tx).top(60.0).bottom(30.0))
                .align_y(Vertical::Center)
                .into(),
        );
        let banner = stack(layers).width(w).height(h);

        let narrow = inner < NARROW;
        let g = 48.0;
        let lw = if narrow { inner } else { inner * 0.64 };
        let rw = if narrow { inner } else { inner - lw - g };
        let main = self.remote_main(lib, item, env, lw);
        let side = remote_side(item, st.busy(), rw);
        let below: Element<'a, Msg> = if narrow {
            column![main, gap(12.0), side].into()
        } else {
            row![main, space().width(g), side].into()
        };
        column![
            banner,
            gap(28.0),
            container(below).padding(Padding::default().left(m))
        ]
        .into()
    }

    /// Requested (by whom, when, how it stands), or Request.
    fn request_line<'a>(&self, item: &Item) -> Element<'a, Msg> {
        let st = &self.discover;
        let dot = |col: Color| -> Element<'a, Msg> {
            container(space())
                .width(7)
                .height(7)
                .style(move |_| container::Style {
                    background: Some(Background::Color(col)),
                    border: iced::border::rounded(4),
                    ..container::Style::default()
                })
                .into()
        };
        let mut r = row![].spacing(12).align_y(Vertical::Center);
        match st.request_for(item, self.user.as_deref()) {
            Some(req) => {
                let (col, status) = match req.status.as_str() {
                    "approved" => (TEAL(), "approved, on its way"),
                    "available" => (OK(), "now available"),
                    "denied" => (ERROR(), "denied"),
                    _ => (WARN(), "waiting for the admin"),
                };
                let col = c(col);
                let mut t = "Requested".to_owned();
                if let Some(by) = req.requested_by.as_ref().filter(|b| !b.is_empty()) {
                    t += &format!(" by {by}");
                }
                if let Some(at) = req.created {
                    t += &format!(" on {}", date_unix(at));
                }
                r = r
                    .push(dot(col))
                    .push(text(format!("{t} · {status}")).size(13).color(col));
                if let Some(n) = req.note.as_ref().filter(|n| !n.trim().is_empty()) {
                    r = r.push(text(format!("“{n}”")).size(14.5).color(style::MUTED()));
                }
            }
            None => {
                r = r
                    .push(dot(style::MUTED()))
                    .push(text("Not on the server").size(13).color(style::MUTED()))
                    .push(space().width(8));
                match &st.requests {
                    Some(Err(e)) => r = r.push(text(e.clone()).size(13).color(style::MUTED())),
                    _ if self.client.token.is_none() => {}
                    _ => {
                        let sending = st.sending.is_some();
                        let label = if sending { "Requesting…" } else { "Request" };
                        r = r.push(super::details::fill_button(
                            label,
                            None,
                            play_colors(),
                            42.0,
                            0.0,
                            0.0,
                            (!sending).then(|| Msg::Request(item.id.clone())),
                        ));
                    }
                }
            }
        }
        r.into()
    }

    /// Cast, a series' episodes, and titles like it that are on the server.
    fn remote_main<'a>(
        &'a self,
        lib: &'a api::Library,
        item: &'a Item,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let mut col: Column<'a, Msg> = column![].width(w);
        if !item.cast.is_empty() {
            let n = item.cast.len().min(15);
            col = col
                .push(caps("Cast", c(TEAL())))
                .push(gap(4.0))
                .push(text("Top billed").font(BOLD).size(24))
                .push(gap(16.0))
                .push(self.cast_grid(&item.cast[..n], env, w, 150.0, 50.0))
                .push(gap(24.0));
        }
        if item.kind == Kind::Series && !item.seasons.is_empty() {
            let seasons = &item.seasons;
            let pick = if self.discover.season.0 == item.id {
                self.discover.season.1
            } else {
                // the first real season, not the specials
                seasons.iter().position(|s| s.number > 0).unwrap_or(0)
            }
            .min(seasons.len() - 1);
            let real = seasons.iter().filter(|s| s.number > 0).count();
            let eps: usize = seasons.iter().map(|s| s.episodes.len()).sum();
            col = col
                .push(caps("Episodes", c(TEAL())))
                .push(gap(4.0))
                .push(
                    text(format!(
                        "{real} season{} · {eps} episodes",
                        if real == 1 { "" } else { "s" }
                    ))
                    .font(BOLD)
                    .size(24),
                )
                .push(gap(12.0))
                .push(
                    scrollable(widgets::segmented(
                        seasons
                            .iter()
                            .enumerate()
                            .map(|(i, s)| {
                                (
                                    s.label(),
                                    Some(s.episodes.len()),
                                    i == pick,
                                    Msg::RemoteSeason(item.id.clone(), i),
                                )
                            })
                            .collect(),
                    ))
                    .direction(scrollable::Direction::Horizontal(
                        scrollable::Scrollbar::new().width(4).scroller_width(4),
                    )),
                )
                .push(gap(12.0));
            let season = &seasons[pick];
            let mut list: Vec<&Episode> = season.episodes.iter().collect();
            list.sort_by_key(|e| e.number);
            for e in list {
                col = col
                    .push(episode_row(season.number, e, env, w))
                    .push(gap(8.0));
            }
            col = col.push(gap(16.0));
        }
        let more = similar(lib, item, 10, &self.shuffle);
        if !more.is_empty() {
            col = col
                .push(caps("On the server", c(TEAL())))
                .push(gap(4.0))
                .push(text("Like this, ready to watch").font(BOLD).size(24))
                .push(gap(16.0));
            let cols = ((w + GAP) / (170.0 + GAP)).floor().clamp(1.0, 5.0);
            let tw = (w - GAP * (cols - 1.0)) / cols;
            let mut grid = column![].spacing(20);
            for chunk in more.chunks(cols as usize) {
                let mut r = row![].spacing(GAP);
                for i in chunk {
                    r = r.push(self.poster_tile(i, i.year.map(|y| y.to_string()), tw, env));
                }
                grid = grid.push(r);
            }
            col = col.push(grid);
        }
        col.into()
    }

    // ----------------------------------------------------------- people

    /// A person's page, built from what the library knows about them, and
    /// their filmography from Wikidata.
    pub(super) fn person_page<'a>(
        &'a self,
        lib: &'a api::Library,
        name: &'a str,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let person = Person::find(lib, name);
        let titles = person.titles();
        let h = 380.0;
        let (m, inner) = column_of(w);
        let art = titles
            .iter()
            .filter(|i| i.backdrop.is_some())
            .max_by_key(|i| i.year)
            .and_then(|i| i.backdrop.as_deref());
        let r = 92.0;
        let wikidata = self.discover.person_photo(name);
        let photo = person.photo.map(str::to_owned).or(wikidata);
        let face: Element<'a, Msg> = match photo.as_deref().filter(|u| {
            env.images
                .get(
                    u,
                    crate::iced_ui::images::Variant::for_size(2.0 * r, 2.0 * r),
                )
                .is_some()
        }) {
            Some(u) => cards::art(env.images, Some(u), name, 2.0 * r, 2.0 * r, r),
            None => super::avatar(name, r),
        };
        let ring = container(face).padding(4).style(move |_| container::Style {
            background: Some(Background::Color(black(120))),
            border: iced::border::rounded(r + 4.0),
            ..container::Style::default()
        });
        let roles = person.roles();
        let kicker = if roles.is_empty() {
            "Person".to_owned()
        } else {
            roles.join("   ·   ")
        };
        let mut meta = vec![format!(
            "{} title{} in your library",
            titles.len(),
            if titles.len() == 1 { "" } else { "s" }
        )];
        meta.extend(span(&titles));
        let mut words: Column<'a, Msg> = column![
            caps(&kicker, c(TEAL())),
            gap(6.0),
            text(name.to_owned()).font(BLACK).size(50),
            gap(8.0),
            text(meta.join("   ·   "))
                .size(15)
                .color(style::TEXT().scale_alpha(0.85)),
        ];
        // their best-known part: the highest-scored title they acted in
        let known = person.movies.iter().chain(&person.shows).max_by(|a, b| {
            let s = |c: &&Credit| c.item.score.unwrap_or(0.0);
            s(a).total_cmp(&s(b))
        });
        if let Some(k) = known {
            let mut kr = row![
                text("Known for").size(14).color(style::MUTED()),
                link(
                    &k.item.title,
                    14.0,
                    c(GOLD()),
                    None,
                    Msg::Open(k.item.id.clone())
                ),
            ]
            .spacing(10)
            .align_y(Vertical::Center);
            if let Some(ch) = k.character {
                kr = kr.push(text(format!("as {ch}")).size(14).color(style::MUTED()));
            }
            words = words.push(gap(12.0)).push(kr);
        }
        let header = stack![
            backdrop(env, art, name, 70, w, h),
            container(link(
                "Back",
                13.0,
                style::TEXT().scale_alpha(0.8),
                Some((Icon::ChevronLeft, false)),
                Msg::Back,
            ))
            .padding(Padding::default().left(28.0).top(22.0)),
            container(
                row![ring, space().width(36), words.width(inner - 2.0 * r - 44.0)]
                    .align_y(Vertical::Center)
            )
            .width(w)
            .height(h)
            .padding(Padding::default().left(m - 4.0).top(28.0))
            .align_y(Vertical::Center),
        ];
        let page: Column<'a, Msg> = column![header, gap(28.0)];
        if titles.is_empty() {
            return page
                .push(
                    container(
                        mono("Nothing in your library with them", 12.0).color(style::MUTED()),
                    )
                    .center_x(Length::Fill),
                )
                .push(gap(20.0))
                .push(self.filmography(lib, name, env, w))
                .into();
        }
        let watched = titles
            .iter()
            .filter(|i| self.status_of(i, env.progress) == super::Filter::Watched)
            .count();
        let genres = top(titles.iter().flat_map(|i| &i.genres).map(String::as_str), 4);
        let scores: Vec<f32> = titles.iter().filter_map(|i| i.score).collect();
        // who they share the most titles with
        let costars = top(
            titles
                .iter()
                .flat_map(|i| {
                    let mut names: Vec<&str> = i.cast.iter().map(|c| c.name.as_str()).collect();
                    names.extend(makers(i));
                    names.sort();
                    names.dedup();
                    names
                })
                .filter(|n| !same(n, name)),
            8,
        );
        let plain = style::TEXT();
        let mut rows: Vec<(&str, String, Color, f32)> = Vec::new();
        if !roles.is_empty() {
            rows.push(("Roles", roles.join(", "), plain, 14.0));
        }
        for (k, n) in [
            ("Movies", person.movies.len()),
            ("TV shows", person.shows.len()),
            ("Directed", person.directed.len()),
            ("Created", person.created.len()),
        ] {
            if n > 0 {
                rows.push((k, n.to_string(), plain, 14.0));
            }
        }
        if let Some(s) = span(&titles) {
            rows.push(("Years", s, plain, 14.0));
        }
        if !genres.is_empty() {
            rows.push((
                "Genres",
                genres.iter().map(|g| g.0).collect::<Vec<_>>().join(", "),
                plain,
                14.0,
            ));
        }
        if !scores.is_empty() {
            rows.push((
                "Average score",
                format!("{:.1}", scores.iter().sum::<f32>() / scores.len() as f32),
                plain,
                14.0,
            ));
        }
        rows.push((
            "You've watched",
            format!("{watched} of {}", titles.len()),
            plain,
            14.0,
        ));

        let narrow = inner < NARROW;
        let g = 48.0;
        let lw = if narrow { inner } else { inner * 0.66 };
        let rw = if narrow { inner } else { inner - lw - g };
        let mut main: Column<'a, Msg> = column![].width(lw);
        for (kicker, title, list) in [
            ("In your library", "Movies", &person.movies),
            ("In your library", "TV shows", &person.shows),
            ("Behind the camera", "Directed", &person.directed),
            ("Behind the camera", "Created", &person.created),
        ] {
            if list.is_empty() {
                continue;
            }
            main = main
                .push(section_title(kicker, title, list.len()))
                .push(self.credit_grid(list, lw, env))
                .push(gap(24.0));
        }
        let mut side: Column<'a, Msg> = column![panel(column![
            caps("About", c(TEAL())),
            space().height(6),
            kv_rows(rows)
        ])]
        .width(rw);
        if !costars.is_empty() {
            let mut p: Column<'a, Msg> = column![caps("Often works with", c(TEAL())), gap(10.0)];
            for (n, k) in &costars {
                p = p
                    .push(
                        row![
                            link(
                                n,
                                14.0,
                                style::TEXT(),
                                None,
                                Msg::Nav(View::Person((*n).to_owned()))
                            ),
                            mono(
                                &format!("{k} title{}", if *k == 1 { "" } else { "s" }),
                                11.0
                            )
                            .color(style::MUTED()),
                        ]
                        .spacing(10)
                        .align_y(Vertical::Center),
                    )
                    .push(gap(6.0));
            }
            side = side.push(gap(20.0)).push(panel(p));
        }
        let body: Element<'a, Msg> = if narrow {
            column![main, gap(24.0), side].into()
        } else {
            row![main, space().width(g), side].into()
        };
        page.push(container(body).padding(Padding::default().left(m)))
            .push(gap(12.0))
            .push(self.filmography(lib, name, env, w))
            .into()
    }

    /// A grid of poster tiles, the part played and year under each.
    fn credit_grid<'a>(&self, credits: &[Credit], w: f32, env: Env<'a>) -> Element<'a, Msg> {
        let cols = ((w + GAP) / (160.0 + GAP)).floor().clamp(1.0, 6.0);
        let tw = (w - GAP * (cols - 1.0)) / cols;
        let mut grid = column![].spacing(20);
        for chunk in credits.chunks(cols as usize) {
            let mut r = row![].spacing(GAP);
            for c in chunk {
                let year = c.item.year.map(|y| y.to_string());
                let sub = match (c.character, year) {
                    (Some(ch), Some(y)) => Some(format!("as {ch} · {y}")),
                    (Some(ch), None) => Some(format!("as {ch}")),
                    (None, y) => y,
                };
                r = r.push(self.poster_tile(c.item, sub, tw, env));
            }
            grid = grid.push(r);
        }
        grid.into()
    }

    /// Everything a person has been in, and which of it is on the server.
    fn filmography<'a>(
        &'a self,
        lib: &'a api::Library,
        name: &str,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let heading = |n: Option<usize>| -> Element<'a, Msg> {
            let mut r = row![text("Filmography").font(BOLD).size(24)]
                .spacing(10)
                .align_y(Vertical::Center);
            if let Some(n) = n {
                r = r.push(mono(&n.to_string(), 13.0).color(style::MUTED()));
            }
            column![caps("Everything they've been in", c(TEAL())), gap(4.0), r]
                .padding(Padding::default().left(SIDE))
                .into()
        };
        let mut col: Column<'a, Msg> = column![gap(12.0)];
        let ids = match self.discover.films_loaded(name) {
            None => {
                return col
                    .push(heading(None))
                    .push(gap(12.0))
                    .push(
                        row![
                            crate::iced_ui::icon::spinner(spin_secs(), style::MUTED(), 14.0),
                            mono("Looking them up…", 11.0).color(style::MUTED()),
                        ]
                        .spacing(10)
                        .align_y(Vertical::Center)
                        .padding(Padding::default().left(SIDE)),
                    )
                    .into();
            }
            Some(Err(e)) => {
                return col
                    .push(heading(None))
                    .push(
                        container(
                            text(format!("Couldn't look them up: {e}"))
                                .size(14.5)
                                .color(style::MUTED()),
                        )
                        .padding(Padding::default().left(SIDE).top(8.0)),
                    )
                    .into();
            }
            Some(Ok(f)) if f.credits.is_empty() => return col.into(),
            Some(Ok(_)) => match &self.credits {
                Some((n, ids)) if n == name => ids,
                _ => return col.push(heading(None)).into(),
            },
        };
        let on = ids.iter().filter(|(_, o)| *o).count();
        let filter = self.discover.film_filter;
        col = col.push(heading(Some(ids.len()))).push(gap(12.0)).push(
            container(widgets::segmented(vec![
                (
                    "All".into(),
                    Some(ids.len()),
                    filter == FilmFilter::All,
                    Msg::Films(FilmFilter::All),
                ),
                (
                    "On the server".into(),
                    Some(on),
                    filter == FilmFilter::OnServer,
                    Msg::Films(FilmFilter::OnServer),
                ),
                (
                    "Not on the server".into(),
                    Some(ids.len() - on),
                    filter == FilmFilter::NotOnServer,
                    Msg::Films(FilmFilter::NotOnServer),
                ),
            ]))
            .padding(Padding::default().left(SIDE - 22.0)),
        );
        col = col.push(gap(12.0));
        let shown: Vec<String> = ids
            .iter()
            .filter(|(_, o)| match filter {
                FilmFilter::All => true,
                FilmFilter::OnServer => *o,
                FilmFilter::NotOnServer => !*o,
            })
            .map(|(id, _)| id.clone())
            .collect();
        if shown.is_empty() {
            return col
                .push(
                    container(mono("Nothing here", 12.0).color(style::MUTED()))
                        .padding(Padding::default().left(SIDE)),
                )
                .into();
        }
        let cards = self.catalog_cards(lib, shown.iter(), env);
        col.push(cards::grid(env.images, &cards, w, self.hot(), "films"))
            .into()
    }
}

/// The title as its logo when there is one, else big black text.
fn title_text<'a>(item: &Item, env: Env<'a>, size: f32) -> Element<'a, Msg> {
    if let Some((h, (iw, ih))) = item
        .logo
        .as_deref()
        .and_then(|u| env.images.get(u, crate::iced_ui::images::Variant::Full))
    {
        let (iw, ih) = (iw as f32, ih as f32);
        let s = (560.0 / iw).min(96.0 / ih);
        return _image(h).width(iw * s).height(ih * s).into();
    }
    text(item.title.clone()).font(BLACK).size(size).into()
}

/// A section heading: kicker, title and a count.
fn section_title<'a>(kicker: &str, title: &str, n: usize) -> Element<'a, Msg> {
    column![
        caps(kicker, c(TEAL())),
        gap(4.0),
        row![
            text(title.to_owned()).font(BOLD).size(24),
            mono(&n.to_string(), 13.0).color(style::MUTED())
        ]
        .spacing(10)
        .align_y(Vertical::Center),
        gap(16.0),
    ]
    .into()
}

/// A small outlined tag that can be clicked.
fn tag_link<'a>(t: &str, msg: Msg) -> Element<'a, Msg> {
    let b = iced::widget::button(text(t.to_owned()).size(11.5).color(style::TEXT()))
        .padding([3, 6])
        .style(|_, s| iced::widget::button::Style {
            background: Some(Background::Color(black(
                if matches!(
                    s,
                    iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
                ) {
                    160
                } else {
                    80
                },
            ))),
            border: Border {
                color: c(crate::ui::theme::LINE_HI()),
                width: 1.0,
                radius: 3.0.into(),
            },
            ..iced::widget::button::Style::default()
        })
        .on_press(msg);
    crate::iced_ui::chrome::tip_below(b, "See their movies and shows")
}

/// Streaming services as names that open the title on each, in the
/// browser; one we have no link for stays plain text.
pub(super) fn service_links<'a>(
    services: &[&str],
    title: &str,
    size: f32,
    col: Color,
) -> Element<'a, Msg> {
    let mut r = row![].align_y(Vertical::Center);
    for (i, s) in services.iter().enumerate() {
        if i > 0 {
            r = r.push(text(", ").size(size).color(style::MUTED()));
        }
        r = r.push(match addons::service_url(s, title) {
            Some(url) => crate::iced_ui::chrome::tip_below(
                link(s, size, col, None, Msg::OpenUrl(url)),
                &format!("Find it on {s}"),
            ),
            None => text((*s).to_owned()).size(size).color(col).into(),
        });
    }
    r.wrap().into()
}

/// One episode of a series that isn't on the server.
fn episode_row<'a>(season: u32, e: &Episode, env: Env<'a>, w: f32) -> Element<'a, Msg> {
    let title = if e.title.is_empty() {
        format!("Episode {}", e.number)
    } else {
        e.title.clone()
    };
    let mut meta = vec![format!("S{season:02}E{:02}", e.number)];
    if let Some(a) = &e.aired {
        meta.push(date_long(a));
    }
    let mut words = column![
        text(title).font(BOLD).size(15),
        mono(&meta.join("   ·   "), 10.5).color(style::MUTED()),
    ]
    .spacing(4);
    if !e.overview.is_empty() {
        words = words.push(
            text(e.overview.clone())
                .size(13)
                .color(style::TEXT().scale_alpha(0.8)),
        );
    }
    container(
        row![
            cards::art(env.images, e.still.as_deref(), &e.title, 160.0, 90.0, 4.0),
            words
        ]
        .spacing(16)
        .align_y(Vertical::Top),
    )
    .width(w)
    .padding(10)
    .style(|_| container::Style {
        border: Border {
            color: c(LINE()),
            width: 1.0,
            radius: 8.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

/// The details panel: who made it, when, where, and links out.
fn remote_side<'a>(item: &Item, loading: bool, w: f32) -> Element<'a, Msg> {
    let extra = item.addon.clone().unwrap_or_default();
    let people = |v: &[String]| (!v.is_empty()).then(|| v.join(", "));
    let (maker_key, makers_list): (&str, Vec<&str>) = match item.kind {
        Kind::Series if !item.creators.is_empty() => (
            "Created by",
            item.creators.iter().map(String::as_str).collect(),
        ),
        Kind::Series => (
            "Created by",
            item.director
                .as_deref()
                .map(split_names)
                .unwrap_or_default(),
        ),
        Kind::Movie => (
            "Director",
            item.director
                .as_deref()
                .map(split_names)
                .unwrap_or_default(),
        ),
    };
    let candidates = [
        (
            "Writers",
            (item.kind == Kind::Movie)
                .then(|| people(&extra.writers))
                .flatten(),
        ),
        ("Released", item.released.as_deref().map(date_long)),
        (
            "Years",
            match (item.kind, item.year, item.end_year) {
                (Kind::Series, Some(a), Some(b)) if a != b => Some(format!("{a}–{b}")),
                (Kind::Series, Some(a), _) => Some(format!("{a}–")),
                _ => None,
            },
        ),
        (
            if item.kind == Kind::Series {
                "Episode length"
            } else {
                "Runtime"
            },
            item.duration.map(fmt_runtime),
        ),
        ("Country", extra.country.clone()),
        ("Genres", people(&item.genres)),
        (
            "Rating",
            item.score.map(|s| {
                format!(
                    "{s:.1} {}",
                    item.score_source.as_deref().unwrap_or_default()
                )
                .trim()
                .to_owned()
            }),
        ),
        ("Awards", extra.awards.clone()),
    ];
    let rows: Vec<(&str, String)> = candidates
        .into_iter()
        .filter_map(|(k, v)| Some((k, v?)))
        .collect();
    let info = |k: &str, v: Element<'a, Msg>| -> Element<'a, Msg> {
        column![
            row![
                text(k.to_owned()).size(14).color(style::MUTED()).width(110),
                v
            ],
            space().height(12),
        ]
        .into()
    };
    let mut p: Column<'a, Msg> = column![caps("Details", c(TEAL())), gap(14.0)];
    if rows.is_empty() && makers_list.is_empty() {
        p = p.push(
            text("No details for this title.")
                .size(14.5)
                .color(style::MUTED()),
        );
    }
    if !makers_list.is_empty() {
        p = p.push(info(maker_key, people_links(&makers_list, 14.0)));
    }
    for (k, v) in rows {
        p = p.push(info(k, text(v).size(14).into()));
    }
    let mut out_links = row![].spacing(12);
    if let Some(t) = &extra.trailer {
        out_links = out_links.push(link(
            "Watch the trailer",
            14.5,
            style::TEXT(),
            Some((Icon::External, true)),
            Msg::OpenUrl(format!("https://www.youtube.com/watch?v={t}")),
        ));
    }
    if let Some(id) = &item.imdb_id {
        out_links = out_links.push(link(
            "IMDb",
            14.5,
            style::TEXT(),
            Some((Icon::External, true)),
            Msg::OpenUrl(format!("https://www.imdb.com/title/{id}/")),
        ));
    }
    let from = if loading {
        "Loading more details…".to_owned()
    } else if extra.source.is_empty() {
        "From a Stremio addon".to_owned()
    } else {
        format!("Details from {}", extra.source)
    };
    container(panel(
        p.push(out_links)
            .push(gap(8.0))
            .push(mono(&from, 10.0).color(style::MUTED())),
    ))
    .width(w)
    .into()
}
