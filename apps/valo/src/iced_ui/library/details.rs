//! The details pages in the Valo style: the movie page (banner with the poster, cast,
//! "You might also like", the file and metadata panel, critics), the series
//! page (Episodes, Related and Details tabs) and a single episode. Sections
//! the server sends no data for are left out.
use std::time::Instant;

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{
    button, column, container, image, mouse_area, pick_list, row, scrollable, space, stack, text,
    tooltip, Column,
};
use iced::{Background, Border, Color, Element, Length, Padding};

use super::cards::{self, GAP, SIDE};
use super::{fill_gradient, picker, Env, Library, Msg, SeasonOpt, Tab, View};
use crate::api::{self, CastMember, Episode, FileInfo, Item, Watcher};
use crate::iced_ui::chrome::tip_below;
use crate::iced_ui::icon::{self, Icon};
use crate::iced_ui::images::Variant;
use crate::iced_ui::spaced::mono;
use crate::iced_ui::style::{self, black, c, BLACK, BOLD};
use crate::iced_ui::Job;
use crate::ui::browse::details::{
    all_episodes, bitrate, bytes, date_long, date_short, file_name, merge, minutes, money,
    play_colors, similar, split_names, sxe, watchers_text, years, GOLD, TEAL,
};
use crate::ui::browse::{makers, next_up, play_label};
use crate::ui::theme::{self, CARD, INK, LINE, LINE_HI, MUTED, PRIMARY, SURFACE_HI, TEXT};
use crate::util::{color_for, fmt_runtime, fmt_time};

type Egui = ecolor::Color32;

const MAX_W: f32 = 1420.0;
/// Below this content width, side-by-side columns stack.
pub(super) const NARROW: f32 = 980.0;

/// Who else watched a title, as last loaded from the server.
#[derive(Default)]
pub(super) struct Watchers {
    list: Vec<Watcher>,
    job: Option<Job<anyhow::Result<Vec<Watcher>>>>,
    at: Option<Instant>,
}

/// Probing a stream's file info in the background (see `player::probe`).
pub(super) enum Probe {
    Running(Job<anyhow::Result<(FileInfo, Option<f64>)>>),
    Done(FileInfo, Option<f64>),
    Failed,
}

/// Left margin and width of the centered content column.
pub(super) fn column_of(w: f32) -> (f32, f32) {
    let inner = (w - 2.0 * SIDE).clamp(200.0, MAX_W);
    ((w - inner) / 2.0, inner)
}

/// A gap: `n` plus 8px of item spacing.
pub(super) fn gap<'a>(n: f32) -> Element<'a, Msg> {
    space().height(n + 8.0).into()
}

/// Small spaced capitals.
pub(super) fn caps<'a>(t: &str, color: Color) -> Element<'a, Msg> {
    mono(t, 11.0).color(color).into()
}

/// Text cut to about `rows` lines of `width`, with an ellipsis.
pub(super) fn clamp_text<'a>(
    t: &str,
    size: f32,
    color: Color,
    rows: usize,
    width: f32,
) -> Element<'a, Msg> {
    let per_line = (width / (size * 0.5)).max(10.0) as usize;
    text(super::shorten(t, per_line * rows))
        .size(size)
        .color(color)
        .width(width)
        .into()
}

fn dot<'a>() -> Element<'a, Msg> {
    text("·").size(14.5).color(style::MUTED()).into()
}

/// A small outlined tag: "1080p", "TV-14".
pub(super) fn tag<'a>(t: &str) -> Element<'a, Msg> {
    container(text(t.to_owned()).size(11.5).color(style::TEXT()))
        .padding([3, 6])
        .style(|_| container::Style {
            background: Some(Background::Color(black(80))),
            border: Border {
                color: c(LINE_HI()),
                width: 1.0,
                radius: 3.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// Text (with an icon before it, or after for `after`) that lights up on hover.
pub(super) fn link<'a>(
    t: &str,
    size: f32,
    color: Color,
    ic: Option<(Icon, bool)>,
    msg: Msg,
) -> Element<'a, Msg> {
    let label = text(t.to_owned()).size(size);
    let content: Element<'a, Msg> = match ic {
        None => label.into(),
        Some((i, after)) => {
            let g = icon::icon_hover(i, color, style::TEXT(), 16.0);
            if after {
                row![label, g].align_y(Vertical::Center).into()
            } else {
                row![g, label].align_y(Vertical::Center).into()
            }
        }
    };
    button(content)
        .padding(0)
        .style(move |_, s| button::Style {
            background: None,
            text_color: if hot(s) { style::TEXT() } else { color },
            ..button::Style::default()
        })
        .on_press(msg)
        .into()
}

fn hot(s: button::Status) -> bool {
    matches!(s, button::Status::Hovered | button::Status::Pressed)
}

/// Names as links to their pages, separated by commas.
pub(super) fn people_links<'a>(names: &[&str], size: f32) -> Element<'a, Msg> {
    let mut r = row![].align_y(Vertical::Center);
    for (i, n) in names.iter().enumerate() {
        if i > 0 {
            r = r.push(text(", ").size(size).color(style::MUTED()));
        }
        r = r.push(tip_below(
            link(
                n,
                size,
                style::TEXT(),
                None,
                Msg::Nav(View::Person((*n).to_owned())),
            ),
            "See their movies and shows",
        ));
    }
    r.wrap().into()
}

/// A rounded panel for metadata, critics, info.
pub(super) fn panel<'a>(content: impl Into<Element<'a, Msg>>) -> Element<'a, Msg> {
    container(content)
        .padding(24)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(c(CARD()))),
            border: Border {
                color: c(LINE()),
                width: 1.0,
                radius: 10.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// A hairline across its parent.
pub(super) fn hline<'a>() -> Element<'a, Msg> {
    container(space())
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(Background::Color(c(LINE()))),
            ..container::Style::default()
        })
        .into()
}

/// A key on the left, a value on the right, a hairline under it if `line`.
pub(super) fn kv_row<'a>(k: &str, value: Element<'a, Msg>, line: bool) -> Element<'a, Msg> {
    let r = row![
        text(k.to_owned()).size(14).color(style::MUTED()),
        space::horizontal(),
        value,
    ]
    .spacing(10)
    .align_y(Vertical::Center);
    let mut out = column![space().height(10), r, space().height(10)];
    if line {
        out = out.push(hline());
    }
    out.into()
}

/// Rows of [`kv_row`], a hairline between each.
pub(super) fn kv_rows<'a>(rows: Vec<(&str, String, Color, f32)>) -> Element<'a, Msg> {
    let n = rows.len();
    let mut out = column![];
    for (i, (k, v, color, size)) in rows.into_iter().enumerate() {
        let value = text(v)
            .size(size)
            .color(color)
            .wrapping(text::Wrapping::None);
        out = out.push(kv_row(k, container(value).clip(true).into(), i + 1 < n));
    }
    out.into()
}

/// Label column and value column, no lines (series "Info").
fn info_row<'a>(k: &str, value: Element<'a, Msg>, label_w: f32) -> Element<'a, Msg> {
    column![
        row![
            text(k.to_owned())
                .size(14)
                .color(style::MUTED())
                .width(label_w),
            value
        ],
        space().height(12),
    ]
    .into()
}

/// A filled button with an optional icon; `radius` 0 makes it a pill
///.
pub(super) fn fill_button<'a>(
    t: &str,
    ic: Option<Icon>,
    colors: (Egui, Egui),
    height: f32,
    min_w: f32,
    radius: f32,
    msg: Option<Msg>,
) -> Element<'a, Msg> {
    let (fill, fg) = (c(colors.0), c(colors.1));
    let hover = c(shade(colors.0, 0.88));
    let radius = match (radius, theme::skin()) {
        (0.0, theme::Skin::Valo) => height / 2.0,
        (0.0, _) => f32::from(theme::spec().button_radius).min(height / 2.0),
        (r, _) => r,
    };
    let mut content = row![].align_y(Vertical::Center);
    if let Some(i) = ic {
        content = content.push(icon::icon(i, fg, 22.0));
    }
    content = content.push(text(t.to_owned()).font(BOLD).size(15).color(fg));
    let mut b = button(container(content).center_y(height))
        .height(height)
        .padding([0, 24])
        .style(move |_, s| button::Style {
            background: Some(Background::Color(if hot(s) { hover } else { fill })),
            border: iced::border::rounded(radius),
            ..button::Style::default()
        })
        .on_press_maybe(msg);
    if min_w > 0.0 {
        b = b.width(Length::Fixed(min_w));
    }
    b.into()
}

/// Darker (f < 1) or lighter (f > 1) version of an opaque color.
fn shade(c: Egui, f: f32) -> Egui {
    let s = |v: u8| (f32::from(v) * f).clamp(0.0, 255.0) as u8;
    Egui::from_rgb(s(c.r()), s(c.g()), s(c.b()))
}

/// A round translucent icon button (the bookmark).
fn circle_button<'a>(ic: Icon, tip: &str, msg: Msg) -> Element<'a, Msg> {
    let b = button(container(icon::icon(ic, style::TEXT(), 24.0)).center(42))
        .width(42)
        .height(42)
        .padding(0)
        .style(|_, s| button::Style {
            background: Some(Background::Color(if hot(s) {
                c(theme::mix(SURFACE_HI(), TEXT(), 0.12))
            } else {
                c(SURFACE_HI())
            })),
            border: iced::border::rounded(21),
            ..button::Style::default()
        })
        .on_press(msg);
    tip_below(b, tip)
}

/// The title as artwork (its logo) when the server has one, else big text.
pub(super) fn title_art<'a>(
    item: &Item,
    env: Env<'a>,
    max: (f32, f32),
    size: f32,
) -> Element<'a, Msg> {
    if let Some((h, (iw, ih))) = item
        .logo
        .as_deref()
        .and_then(|u| env.images.get(u, Variant::Full))
    {
        let (iw, ih) = (iw as f32, ih as f32);
        let s = (max.0 / iw).min(max.1 / ih);
        return image(h).width(iw * s).height(ih * s).into();
    }
    text(item.title.clone())
        .font(BLACK)
        .size(size)
        .color(style::TEXT())
        .into()
}

/// Full-width artwork darkened to `dim`, a shade along its left half, fading
/// into the page at the bottom.
pub(super) fn backdrop<'a>(
    env: Env<'a>,
    art: Option<&str>,
    title: &str,
    dim: u8,
    w: f32,
    h: f32,
) -> Element<'a, Msg> {
    let bg = style::BG();
    let clear = Color { a: 0.0, ..bg };
    let mut layers: Vec<Element<'a, Msg>> = vec![container(space())
        .width(w)
        .height(h)
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            ..container::Style::default()
        })
        .into()];
    if art.and_then(|u| env.images.get(u, Variant::Full)).is_some() {
        layers.push(cards::art(env.images, art, title, w, h, 0.0));
        layers.push(
            container(space())
                .width(w)
                .height(h)
                .style(move |_| container::Style {
                    background: Some(Background::Color(black(255 - dim))),
                    ..container::Style::default()
                })
                .into(),
        );
    } else {
        let tint = c(color_for(title));
        let top = Color { a: 0.45, ..tint };
        layers.push(
            container(space())
                .width(w)
                .height(h)
                .style(move |_| fill_gradient(std::f32::consts::PI, top, bg))
                .into(),
        );
    }
    layers.push(
        container(space())
            .width(w * 0.5)
            .height(h)
            .style(|_| fill_gradient(std::f32::consts::FRAC_PI_2, black(110), Color::TRANSPARENT))
            .into(),
    );
    layers.push(
        container(
            container(space())
                .width(w)
                .height(h * 0.45)
                .style(move |_| fill_gradient(std::f32::consts::PI, clear, bg)),
        )
        .height(h)
        .align_y(Vertical::Bottom)
        .into(),
    );
    stack(layers).width(w).height(h).into()
}

/// A watcher's disc: the first letter on the name's color, ringed in the page color.
fn face<'a>(name: &str, shown: &str, r: f32) -> Element<'a, Msg> {
    let initial = shown
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    let fill = c(color_for(name));
    container(text(initial).font(BLACK).size(r).color(Color::WHITE))
        .center(2.0 * (r + 2.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                color: style::BG(),
                width: 2.0,
                radius: (r + 2.0).into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// Up to three overlapping faces (a "+N" one for the rest) and the two
/// latest names; hover for everyone.
pub(super) fn watchers_row<'a>(list: &[Watcher]) -> Option<Element<'a, Msg>> {
    if list.is_empty() {
        return None;
    }
    let r = 14.0;
    let step = r * 1.35;
    let faces = list.len().min(3);
    let n = faces + usize::from(list.len() > 3);
    // right to left, so the latest sits on top
    let mut layers: Vec<Element<'a, Msg>> = Vec::new();
    for i in (0..n).rev() {
        let disc: Element<'a, Msg> = match list.get(i).filter(|_| i < faces) {
            Some(u) => face(&u.name, u.shown(), r),
            None => container(
                text(format!("+{}", list.len() - faces))
                    .font(BOLD)
                    .size(r * 0.8)
                    .color(style::TEXT()),
            )
            .center(2.0 * (r + 2.0))
            .style(|_| container::Style {
                background: Some(Background::Color(style::SURFACE_HI())),
                border: Border {
                    color: style::BG(),
                    width: 2.0,
                    radius: 16.0.into(),
                },
                ..container::Style::default()
            })
            .into(),
        };
        layers.push(
            container(disc)
                .padding(Padding::default().left(step * i as f32))
                .into(),
        );
    }
    let w = 2.0 * (r + 2.0) + step * (n - 1) as f32;
    let tip: Vec<String> = list
        .iter()
        .map(|u| {
            let what = if u.completed { "watched" } else { "started" };
            if u.updated > 0 {
                let ago = crate::ui::browse::people::ago(u.updated);
                format!("{}  ·  {what} {ago}", u.shown())
            } else {
                u.shown().to_owned()
            }
        })
        .collect();
    let content = row![
        stack(layers).width(w).height(2.0 * (r + 2.0)),
        text(watchers_text(list))
            .size(13.5)
            .color(style::TEXT().scale_alpha(0.85)),
    ]
    .spacing(16)
    .align_y(Vertical::Center);
    Some(
        tooltip(
            content,
            container(text(tip.join("\n")).size(12.5))
                .padding([4, 8])
                .style(style::tip),
            tooltip::Position::Bottom,
        )
        .into(),
    )
}

/// A title's play button look: `normal` colors and the play icon, or grey
/// and none while an admin moves its files.
pub(super) fn play_look(item: &Item, normal: (Egui, Egui)) -> (Option<Icon>, (Egui, Egui)) {
    if item.unavailable {
        (None, (SURFACE_HI(), MUTED()))
    } else {
        (Some(Icon::Play), normal)
    }
}

/// A 2px outline over artwork (hovered tiles).
fn outline<'a>(w: f32, h: f32, radius: f32) -> Element<'a, Msg> {
    container(space())
        .width(w)
        .height(h)
        .style(move |_| container::Style {
            border: Border {
                color: style::TEXT(),
                width: 2.0,
                radius: radius.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// An episode's watched part, in gold along the still's bottom.
fn gold_bar<'a>(f: f32, w: f32) -> Element<'a, Msg> {
    let bw = (w - 20.0).max(1.0);
    let gold = c(GOLD());
    container(stack![
        container(space())
            .width(bw)
            .height(4)
            .style(|_| container::Style {
                background: Some(Background::Color(style::white(70))),
                border: iced::border::rounded(2),
                ..container::Style::default()
            }),
        container(space())
            .width(bw * f.clamp(0.0, 1.0))
            .height(4)
            .style(move |_| container::Style {
                background: Some(Background::Color(gold)),
                border: iced::border::rounded(2),
                ..container::Style::default()
            }),
    ])
    .padding(Padding::default().left(10.0).bottom(6.0))
    .height(Length::Fill)
    .align_y(Vertical::Bottom)
    .into()
}

/// A section tab under the series banner: 40 high, underlined when open.
fn section_tab<'a>(label: &str, on: bool, msg: Msg) -> Element<'a, Msg> {
    let underline = container(space())
        .width(Length::Fill)
        .height(2)
        .style(move |_| container::Style {
            background: on.then_some(Background::Color(style::TEXT())),
            ..container::Style::default()
        });
    button(
        column![
            container(text(label.to_owned()).size(15)).center_y(38),
            underline
        ]
        .width(Length::Shrink),
    )
    .padding(0)
    .style(move |_, s| button::Style {
        background: None,
        text_color: if on || hot(s) {
            style::TEXT()
        } else {
            style::MUTED()
        },
        ..button::Style::default()
    })
    .on_press(msg)
    .into()
}

/// `content` that reports the pointer under `key` and does `msg` on a click.
pub(super) fn hover_area<'a>(
    content: impl Into<Element<'a, Msg>>,
    key: String,
    msg: Msg,
) -> Element<'a, Msg> {
    mouse_area(content)
        .on_press(msg)
        .on_enter(Msg::Hover(Some(key)))
        .on_exit(Msg::Hover(None))
        .interaction(iced::mouse::Interaction::Pointer)
        .into()
}

impl Library {
    /// Who else watched `item`, as last loaded.
    pub(super) fn watchers_of(&self, item: &Item) -> &[Watcher] {
        self.watchers.get(&item.id).map_or(&[], |w| &w.list)
    }

    /// File info for `stream`: the server's, completed by the probe.
    pub(super) fn file_for(
        &self,
        stream: &str,
        server: Option<&FileInfo>,
    ) -> (Option<FileInfo>, Option<f64>) {
        match self.probes.get(stream) {
            Some(Probe::Done(f, d)) => (merge(server, Some(f)), *d),
            _ => (server.cloned(), None),
        }
    }

    /// Start (and poll) what the open page wants: who else watched it,
    /// again every minute, and a probe of a stream whose file info the
    /// server left incomplete.
    pub(super) fn details_work(&mut self) {
        for w in self.watchers.values_mut() {
            if let Some(r) = w.job.as_mut().and_then(Job::poll) {
                w.job = None;
                w.at = Some(Instant::now());
                if let Ok(list) = r {
                    w.list = list;
                }
            }
        }
        for p in self.probes.values_mut() {
            if let Probe::Running(j) = p {
                if let Some(r) = j.poll() {
                    *p = r.map_or(Probe::Failed, |(f, d)| Probe::Done(f, d));
                }
            }
        }
        let Some(lib) = self.lib.clone() else {
            return;
        };
        let (item, stream) = match &self.view {
            View::Details(id) => match lib.find(id) {
                Some(i) => (Some(i), i.stream.as_deref().map(|s| (s, i.file.as_ref()))),
                None => return,
            },
            View::Episode(_, ep) => match lib.find_episode(ep) {
                Some((s, si, ei)) => {
                    let e = &s.seasons[si].episodes[ei];
                    (None, Some((e.stream.as_str(), e.file.as_ref())))
                }
                None => return,
            },
            _ => return,
        };
        // the library's titles only, not an addon's
        if let Some(item) =
            item.filter(|i| i.id.starts_with("movie:") || i.id.starts_with("series:"))
        {
            let w = self.watchers.entry(item.id.clone()).or_default();
            if w.job.is_none() && w.at.is_none_or(|t| t.elapsed().as_secs() >= 60) {
                let client = self.client.clone();
                let (kind, id) = (item.kind, item.server_id().to_owned());
                w.job = Some(Job::spawn(move || client.watchers(kind, &id)));
            }
        }
        if let Some((stream, server)) = stream {
            let complete = server.is_some_and(|f| f.quality.is_some() && f.size.is_some());
            if !complete && !self.probes.contains_key(stream) {
                let src = crate::player::Source::Url {
                    url: stream.to_owned(),
                    auth: self.client.auth_header(),
                };
                self.probes.insert(
                    stream.to_owned(),
                    Probe::Running(Job::spawn(move || crate::player::probe(&src))),
                );
            }
        }
    }

    /// Waiting on who watched or a probe: tick quickly.
    pub(super) fn details_busy(&self) -> bool {
        self.watchers.values().any(|w| w.job.is_some())
            || self.probes.values().any(|p| matches!(p, Probe::Running(_)))
    }

    /// The bookmark circle: on My list or not.
    pub(super) fn bookmark<'a>(&self, item: &Item) -> Element<'a, Msg> {
        let (ic, tip) = if self.watchlist.contains(&self.key(&item.id)) {
            (Icon::BookmarkOn, "Remove from My list")
        } else {
            (Icon::Bookmark, "Add to My list")
        };
        circle_button(ic, tip, Msg::Bookmark(item.id.clone()))
    }

    /// Headshots in a grid, name and character under each; each opens the
    /// person's page.
    pub(super) fn cast_grid<'a>(
        &self,
        cast: &[CastMember],
        env: Env<'a>,
        width: f32,
        cell_w: f32,
        r: f32,
    ) -> Element<'a, Msg> {
        let cols = ((width / cell_w).floor() as usize).max(1);
        let cell = width / cols as f32;
        let mut grid = column![].spacing(14);
        for (ri, chunk) in cast.chunks(cols).enumerate() {
            let mut line = row![];
            for (i, m) in chunk.iter().enumerate() {
                let key = format!("cast\u{1f}{ri}\u{1f}{i}\u{1f}{}", m.name);
                let lit = self.hovered.as_deref() == Some(key.as_str());
                let v = Variant::for_size(2.0 * r, 2.0 * r);
                let photo: Element<'a, Msg> = match m
                    .photo
                    .as_deref()
                    .filter(|u| env.images.get(u, v).is_some())
                {
                    Some(u) => cards::art(env.images, Some(u), &m.name, 2.0 * r, 2.0 * r, r),
                    None => super::avatar(&m.name, r),
                };
                let ring = container(photo)
                    .padding(3)
                    .style(move |_| container::Style {
                        border: Border {
                            color: if lit {
                                style::TEXT()
                            } else {
                                Color::TRANSPARENT
                            },
                            width: 2.0,
                            radius: (r + 3.0).into(),
                        },
                        ..container::Style::default()
                    });
                let mut who = column![
                    ring,
                    space().height(6),
                    text(m.name.clone())
                        .size(14)
                        .color(if lit {
                            style::TEXT()
                        } else {
                            style::TEXT().scale_alpha(0.9)
                        })
                        .wrapping(text::Wrapping::None),
                ]
                .align_x(Horizontal::Center);
                if let Some(ch) = &m.character {
                    who = who.push(
                        text(ch.clone())
                            .font(style::MONO)
                            .size(11)
                            .color(style::MUTED())
                            .wrapping(text::Wrapping::None),
                    );
                }
                let cellbox = container(who)
                    .width(cell)
                    .height(2.0 * r + 52.0)
                    .padding([0.0, 6.0])
                    .align_x(Horizontal::Center)
                    .clip(true);
                line = line.push(tip_below(
                    hover_area(cellbox, key, Msg::Nav(View::Person(m.name.clone()))),
                    "See their movies and shows",
                ));
            }
            grid = grid.push(line);
        }
        grid.into()
    }

    /// A poster with the title and `sub` under it.
    pub(super) fn poster_tile<'a>(
        &self,
        item: &Item,
        sub: Option<String>,
        w: f32,
        env: Env<'a>,
    ) -> Element<'a, Msg> {
        let art_h = w * 1.5;
        let key = format!("tile\u{1f}{}", item.id);
        let lit = self.hovered.as_deref() == Some(key.as_str());
        let mut layers = vec![cards::art(
            env.images,
            item.poster.as_deref(),
            &item.title,
            w,
            art_h,
            6.0,
        )];
        if item.unavailable {
            layers.push(super::marks::tape(w, art_h, 6.0).into());
        }
        if self.watched(item, env.progress) {
            layers.push(super::marks::sash(w, art_h, 1.0).into());
        }
        if lit {
            layers.push(outline(w, art_h, 6.0));
        }
        let mut under = column![text(item.title.clone())
            .size(13.5)
            .color(style::TEXT())
            .wrapping(text::Wrapping::None)];
        if let Some(s) = sub {
            under = under.push(text(s).size(11.5).color(style::MUTED()));
        }
        let body = column![
            stack(layers).width(w).height(art_h),
            container(under)
                .width(w)
                .height(44)
                .padding(Padding::default().top(10.0))
                .clip(true),
        ];
        tooltip(
            hover_area(body, key, Msg::Open(item.id.clone())),
            container(text(item.title.clone()).size(12.5))
                .padding([4, 8])
                .style(style::tip),
            tooltip::Position::FollowCursor,
        )
        .into()
    }

    /// Wide artwork with the title and year on it ("Related").
    pub(super) fn wide_tile<'a>(
        &self,
        item: &Item,
        w: f32,
        h: f32,
        env: Env<'a>,
    ) -> Element<'a, Msg> {
        let key = format!("wide\u{1f}{}", item.id);
        let lit = self.hovered.as_deref() == Some(key.as_str());
        let art = item.backdrop.as_deref().or(item.poster.as_deref());
        let mut words = column![text(item.title.clone())
            .font(BOLD)
            .size(14)
            .color(style::TEXT())]
        .spacing(4);
        if let Some(y) = item.year {
            words = words.push(text(y.to_string()).size(11.5).color(style::MUTED()));
        }
        let mut layers =
            vec![
                cards::art(env.images, art, &item.title, w, h, 6.0),
                container(container(space()).width(w).height(70).style(|_| {
                    fill_gradient(std::f32::consts::PI, Color::TRANSPARENT, black(190))
                }))
                .height(h)
                .align_y(Vertical::Bottom)
                .into(),
                container(words)
                    .height(h)
                    .padding(Padding::default().left(12.0).bottom(12.0))
                    .align_y(Vertical::Bottom)
                    .into(),
            ];
        if lit {
            layers.push(outline(w, h, 6.0));
        }
        hover_area(
            stack(layers).width(w).height(h),
            key,
            Msg::Open(item.id.clone()),
        )
    }

    // ------------------------------------------------------------ movie

    pub(super) fn movie_page<'a>(
        &'a self,
        lib: &'a api::Library,
        item: &'a Item,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let (file, probed) = match item.stream.as_deref() {
            Some(s) => self.file_for(s, item.file.as_ref()),
            None => (item.file.clone(), None),
        };
        let runtime = item.duration.filter(|d| *d > 0.0).or(probed);
        let style = theme::spec().layout.details;
        if style == theme::Details::Watch {
            return self.watch_page(lib, item, file.as_ref(), runtime, env, w);
        }
        let h = (w * 0.5).clamp(520.0, 700.0);
        let (m, inner) = column_of(w);
        let show_poster = inner >= 860.0;
        let poster_w = if show_poster {
            (inner * 0.22).clamp(220.0, 320.0)
        } else {
            0.0
        };
        let art = item.backdrop.as_deref().or(item.poster.as_deref());
        let mut layers = vec![
            backdrop(env, art, &item.title, 125, w, h),
            container(link(
                "Back to movies",
                13.0,
                style::TEXT().scale_alpha(0.8),
                Some((Icon::ChevronLeft, false)),
                Msg::Back,
            ))
            .padding(Padding::default().left(28.0).top(22.0))
            .into(),
        ];
        if show_poster {
            let poster_h = poster_w * 1.5;
            layers.push(
                container(self.big_poster(item, file.as_ref(), poster_w, env))
                    .padding(Padding::default().left(m).top(h / 2.0 - poster_h / 2.0))
                    .into(),
            );
        }

        // the words, centered in the space right of the poster
        let tx = m + if show_poster { poster_w + 40.0 } else { 0.0 };
        let tw = (m + inner - tx).max(100.0);
        let mut kinds = vec!["Movie".to_owned()];
        if !item.genres.is_empty() {
            kinds.push(item.genres.join(" · "));
        }
        let mut words: Column<'a, Msg> = column![
            caps(&kinds.join("   ·   "), style::TEXT().scale_alpha(0.65)),
            gap(8.0),
            title_art(item, env, (560.0, 96.0), 54.0),
            gap(14.0),
        ]
        .width(tw);
        if !item.overview.is_empty() {
            words = words
                .push(
                    text(format!("“{}”", item.overview))
                        .size(17)
                        .color(style::TEXT().scale_alpha(0.7)),
                )
                .push(gap(16.0));
        }
        let gold = c(GOLD());
        let mut facts: Vec<Element<'a, Msg>> = Vec::new();
        let mut fact = |parts: Vec<Element<'a, Msg>>| {
            if !facts.is_empty() {
                facts.push(dot());
            }
            facts.extend(parts);
        };
        if let Some(s) = item.score {
            let mut parts: Vec<Element<'a, Msg>> = vec![
                icon::icon(Icon::Star, gold, 14.0).into(),
                text(format!("{s:.1}")).size(15).color(style::TEXT()).into(),
            ];
            if let Some(src) = &item.score_source {
                parts.push(text(src.clone()).size(15).color(style::MUTED()).into());
            }
            fact(parts);
        }
        if let Some(y) = item.year {
            fact(vec![text(y.to_string()).size(15).color(gold).into()]);
        }
        if let Some(d) = runtime {
            fact(vec![text(fmt_runtime(d))
                .size(15)
                .color(style::TEXT())
                .into()]);
        }
        if let Some(q) = file.as_ref().and_then(|f| f.quality.as_deref()) {
            fact(vec![tag(q)]);
        }
        if let Some(r) = &item.rating {
            fact(vec![tag(r)]);
        }
        words = words.push(
            iced::widget::Row::with_children(facts)
                .spacing(10)
                .align_y(Vertical::Center),
        );
        let directors: Vec<&str> = makers(item).collect();
        if !directors.is_empty() {
            words = words.push(gap(14.0)).push(
                row![
                    text("Director").size(14).color(style::MUTED()),
                    people_links(&directors, 14.0),
                ]
                .spacing(10)
                .align_y(Vertical::Center),
            );
        }
        let (label, resume) = play_label(item, env.progress, &|i| self.key(i));
        let label = if label == "Play" {
            "Play movie".to_owned()
        } else {
            label
        };
        let (ic, colors) = play_look(item, play_colors());
        let mut buttons = row![fill_button(
            &label,
            ic,
            colors,
            42.0,
            0.0,
            0.0,
            (!item.unavailable).then(|| Msg::Play(item.id.clone(), None, false)),
        )]
        .spacing(12)
        .align_y(Vertical::Center);
        if resume {
            buttons = buttons.push(fill_button(
                "Start over",
                None,
                (SURFACE_HI(), TEXT()),
                42.0,
                0.0,
                0.0,
                Some(Msg::Play(item.id.clone(), None, true)),
            ));
        }
        buttons = buttons.push(self.bookmark(item));
        let status = match env.progress.get(&self.key(&item.id)) {
            Some(e) if e.watched() => "Watched".to_owned(),
            Some(e) if e.in_progress() => {
                format!("Ready to stream · {:.0}% watched", e.fraction() * 100.0)
            }
            _ => "Ready to stream".to_owned(),
        };
        buttons = buttons.push(space().width(6)).push(
            row![
                container(space())
                    .width(7)
                    .height(7)
                    .style(move |_| container::Style {
                        background: Some(Background::Color(gold)),
                        border: iced::border::rounded(4),
                        ..container::Style::default()
                    }),
                text(status).size(12.5).color(gold),
            ]
            .spacing(10)
            .align_y(Vertical::Center),
        );
        words = words.push(gap(24.0)).push(buttons);
        if let Some(wr) = watchers_row(self.watchers_of(item)) {
            words = words.push(gap(16.0)).push(wr);
        }
        layers.push(
            container(words)
                .width(w)
                .height(h)
                .padding(Padding::default().left(tx).top(60.0).bottom(30.0))
                .align_y(Vertical::Center)
                .into(),
        );
        let banner: Element<'a, Msg> = if style == theme::Details::Valo {
            stack(layers).width(w).height(h).into()
        } else {
            self.styled_header(item, file.as_ref(), runtime, env, w)
        };

        // cast and similar titles on the left, the file and critics on the right
        let narrow = inner < NARROW;
        let gap_x = 48.0;
        let lw = if narrow { inner } else { inner * 0.64 };
        let rw = if narrow { inner } else { inner - lw - gap_x };
        let mut left = column![].width(lw);
        if !item.cast.is_empty() {
            let n = item.cast.len().min(10);
            left = left
                .push(caps("Cast", c(TEAL())))
                .push(gap(4.0))
                .push(text("Top billed").font(BOLD).size(24))
                .push(gap(16.0))
                .push(self.cast_grid(&item.cast[..n], env, lw, 150.0, 50.0))
                .push(gap(24.0));
        }
        let more = similar(lib, item, 10, &self.shuffle);
        if !more.is_empty() {
            left = left
                .push(caps("From your library", c(TEAL())))
                .push(gap(4.0))
                .push(text("You might also like").font(BOLD).size(24))
                .push(gap(16.0));
            let cols = ((lw + GAP) / (170.0 + GAP)).floor().clamp(1.0, 5.0);
            let tw = (lw - GAP * (cols - 1.0)) / cols;
            let mut grid = column![].spacing(20);
            for chunk in more.chunks(cols as usize) {
                let mut r = row![].spacing(GAP);
                for i in chunk {
                    r = r.push(self.poster_tile(i, i.year.map(|y| y.to_string()), tw, env));
                }
                grid = grid.push(r);
            }
            left = left.push(grid);
        }
        let side = self.movie_side(item, file.as_ref(), runtime, rw);
        let below: Element<'a, Msg> = if narrow {
            left.push(gap(12.0)).push(side).into()
        } else {
            row![left, space().width(gap_x), side].into()
        };
        column![
            banner,
            gap(28.0),
            container(below).padding(Padding::default().left(m))
        ]
        .into()
    }

    /// The poster on the movie banner: "A FILM BY …" over its top, the title
    /// and year over its bottom, the quality and size under it.
    fn big_poster<'a>(
        &self,
        item: &Item,
        file: Option<&FileInfo>,
        pw: f32,
        env: Env<'a>,
    ) -> Element<'a, Msg> {
        let ph = pw * 1.5;
        let gold = c(GOLD());
        let mut layers = vec![cards::art(
            env.images,
            item.poster.as_deref(),
            &item.title,
            pw,
            ph,
            8.0,
        )];
        if item.unavailable {
            layers.push(super::marks::tape(pw, ph, 8.0).into());
        }
        if let Some(d) = &item.director {
            layers.push(
                container(space())
                    .width(pw)
                    .height(70)
                    .style(|_| fill_gradient(std::f32::consts::PI, black(150), Color::TRANSPARENT))
                    .into(),
            );
            // the first director's page; the header lists them all
            let first = split_names(d).first().map(|n| (*n).to_owned());
            let by = button(
                text(format!("A FILM BY {}", d.to_uppercase()))
                    .font(BOLD)
                    .size(11.5)
                    .color(gold),
            )
            .padding(0)
            .style(|_, _| button::Style::default())
            .on_press_maybe(first.map(|n| Msg::Nav(View::Person(n))));
            layers.push(
                container(tip_below(by, "See their movies and shows"))
                    .padding(Padding::default().left(20.0).top(22.0))
                    .into(),
            );
        }
        layers.push(
            container(
                container(space())
                    .width(pw)
                    .height(120)
                    .style(|_| fill_gradient(std::f32::consts::PI, Color::TRANSPARENT, black(200))),
            )
            .height(ph)
            .align_y(Vertical::Bottom)
            .into(),
        );
        let mut words = column![text(item.title.clone())
            .font(BOLD)
            .size(24)
            .color(style::TEXT())]
        .spacing(8);
        if let Some(y) = item.year {
            words = words.push(text(y.to_string()).font(BOLD).size(14).color(gold));
        }
        layers.push(
            container(words)
                .width(pw)
                .height(ph)
                .padding(Padding::default().left(20.0).right(20.0).bottom(20.0))
                .align_y(Vertical::Bottom)
                .into(),
        );
        let under: Vec<String> = [
            file.and_then(|f| f.quality.clone()),
            file.and_then(|f| f.size).map(bytes),
        ]
        .into_iter()
        .flatten()
        .collect();
        column![
            stack(layers).width(pw).height(ph),
            space().height(14),
            text(under.join("   ·   ")).size(12).color(style::MUTED()),
        ]
        .into()
    }

    /// The file and metadata panel, and the critics', next to the cast.
    fn movie_side<'a>(
        &self,
        item: &Item,
        f: Option<&FileInfo>,
        runtime: Option<f64>,
        width: f32,
    ) -> Element<'a, Msg> {
        let gold = c(GOLD());
        let plain = style::TEXT();
        let faint = style::TEXT().scale_alpha(0.8);
        let opt = |v: Option<String>, color: Color, size: f32| v.map(|v| (v, color, size));
        let candidates = [
            (
                "Released",
                opt(item.released.as_deref().map(date_long), plain, 14.0),
            ),
            ("Runtime", opt(runtime.map(fmt_runtime), plain, 14.0)),
            ("Budget", opt(item.budget.map(money), plain, 14.0)),
            ("Box office", opt(item.revenue.map(money), plain, 14.0)),
            (
                "Quality",
                opt(f.and_then(|f| f.quality.clone()), gold, 14.0),
            ),
            (
                "Container",
                opt(
                    f.map(|f| {
                        [f.container.clone(), f.video_codec.clone()]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                            .join(" · ")
                    })
                    .filter(|s| !s.is_empty()),
                    gold,
                    14.0,
                ),
            ),
            (
                "Audio",
                opt(
                    f.and_then(|f| {
                        let mut a = f.audio_codec.clone()?;
                        if let Some(ch) = f.audio_channels {
                            a.push_str(&format!(" · {ch}"));
                        }
                        Some(a)
                    }),
                    gold,
                    14.0,
                ),
            ),
            (
                "Bitrate",
                opt(f.and_then(|f| f.bitrate).map(bitrate), gold, 14.0),
            ),
            (
                "File size",
                opt(f.and_then(|f| f.size).map(bytes), plain, 14.0),
            ),
            (
                "File",
                opt(
                    item.stream.as_deref().and_then(|s| file_name(f, s)),
                    faint,
                    12.0,
                ),
            ),
            ("Path", opt(f.and_then(|f| f.path.clone()), faint, 12.0)),
        ];
        let mut rows: Vec<(&str, String, Color, f32)> = candidates
            .into_iter()
            .filter_map(|(k, v)| v.map(|(v, c, s)| (k, v, c, s)))
            .collect();
        // where it streams: links when opened from Streaming, else a row
        let linked = self.came_from_streaming();
        let services = self.discover.services(&item.id);
        if !linked && !services.is_empty() {
            rows.insert(0, ("Streaming on", services.join(", "), c(TEAL()), 14.0));
        }
        let mut meta = column![caps("File · Metadata", c(TEAL())), space().height(6)];
        let directors = item
            .director
            .as_deref()
            .map(split_names)
            .unwrap_or_default();
        if !directors.is_empty() {
            let more = !rows.is_empty() || (linked && !services.is_empty());
            meta = meta.push(kv_row("Director", people_links(&directors, 14.0), more));
        }
        if linked && !services.is_empty() {
            meta = meta.push(kv_row(
                "Streaming on",
                super::discover::service_links(&services, &item.title, 14.0, c(TEAL())),
                !rows.is_empty(),
            ));
        }
        meta = meta.push(kv_rows(rows));
        let mut side = column![panel(meta)].width(width);
        if !item.reviews.is_empty() {
            let mut critics = column![caps("From critics", style::MUTED())];
            for r in &item.reviews {
                critics = critics.push(gap(12.0)).push(
                    text(format!("“{}”", r.quote))
                        .size(16)
                        .color(style::TEXT().scale_alpha(0.85)),
                );
                let by: Vec<&str> = [r.source.as_deref(), r.score.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect();
                if !by.is_empty() {
                    critics = critics.push(gap(4.0)).push(
                        text(format!("— {}", by.join(" · ")))
                            .size(12)
                            .color(style::MUTED()),
                    );
                }
            }
            side = side.push(gap(20.0)).push(panel(critics));
        }
        side.into()
    }

    // ----------------------------------------------------------- series

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
        let style = theme::spec().layout.details;
        if style == theme::Details::Watch {
            return self.watch_page(lib, item, None, None, env, w);
        }
        let h = (w * 0.42).clamp(460.0, 640.0);
        let (m, inner) = column_of(w);
        let art = item.backdrop.as_deref().or(item.poster.as_deref());

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
        let tw = inner.min(600.0);
        let mut words: Column<'a, Msg> = column![
            title_art(item, env, (380.0, 90.0), 54.0),
            gap(10.0),
            text(meta.join("   ·   "))
                .size(14)
                .color(style::TEXT().scale_alpha(0.85)),
            gap(10.0),
        ]
        .width(tw);
        if !item.overview.is_empty() {
            words = words
                .push(clamp_text(
                    &item.overview,
                    15.0,
                    style::TEXT().scale_alpha(0.85),
                    3,
                    tw,
                ))
                .push(gap(18.0));
        }
        let (label, _) = play_label(item, env.progress, &|i| self.key(i));
        let (ic, colors) = play_look(item, (PRIMARY(), INK()));
        let mut buttons = row![
            fill_button(
                &label,
                ic,
                colors,
                44.0,
                168.0,
                4.0,
                (!item.unavailable).then(|| Msg::Play(item.id.clone(), None, false)),
            ),
            self.bookmark(item),
        ]
        .spacing(12)
        .align_y(Vertical::Center);
        if item.seasons.len() > 1 {
            buttons = buttons.push(self.season_picker(item, season, 120.0));
        }
        words = words.push(buttons);
        if let Some(wr) = watchers_row(self.watchers_of(item)) {
            words = words.push(gap(14.0)).push(wr);
        }
        if !item.creators.is_empty() {
            let names: Vec<&str> = item.creators.iter().map(String::as_str).collect();
            words = words.push(gap(14.0)).push(
                row![
                    text("Created by").size(14).color(style::MUTED()),
                    people_links(&names, 14.0)
                ]
                .spacing(10)
                .align_y(Vertical::Center),
            );
        }
        let banner: Element<'a, Msg> = if style == theme::Details::Valo {
            stack![
                backdrop(env, art, &item.title, 120, w, h),
                container(words)
                    .width(w)
                    .height(h)
                    .padding(Padding::default().left(m).top(40.0).bottom(24.0))
                    .align_y(Vertical::Bottom),
            ]
            .into()
        } else {
            self.styled_header(item, None, None, env, w)
        };

        let mut tabs = row![].spacing(32);
        for (t, label) in [
            (Tab::Episodes, "Episodes"),
            (Tab::Related, "Related"),
            (Tab::Details, "Details"),
        ] {
            tabs = tabs.push(section_tab(label, self.tab == t, Msg::Tab(t)));
        }
        let body: Element<'a, Msg> = match self.tab {
            Tab::Episodes => {
                // the Valo header has the season picker; the styled ones don't
                let mut col = column![];
                if style != theme::Details::Valo && item.seasons.len() > 1 {
                    col = col
                        .push(self.season_picker(item, season, 160.0))
                        .push(gap(14.0));
                }
                col.push(if theme::spec().layout.episode_list {
                    self.episode_list(item, season, env, inner)
                } else {
                    match item.seasons.get(season) {
                        Some(s) => self.episodes_tab(item, season, &s.episodes, env, inner),
                        None => space().into(),
                    }
                })
                .into()
            }
            Tab::Related => {
                let more = similar(lib, item, 20, &self.shuffle);
                if more.is_empty() {
                    text("Nothing similar in your library yet.")
                        .size(14.5)
                        .color(style::MUTED())
                        .into()
                } else {
                    let mut strip = row![].spacing(10);
                    for i in more {
                        strip = strip.push(self.wide_tile(i, 300.0, 170.0, env));
                    }
                    column![
                        text("You might also like").font(BOLD).size(20),
                        gap(14.0),
                        scrollable(strip).direction(scrollable::Direction::Horizontal(
                            scrollable::Scrollbar::new().width(4).scroller_width(4),
                        )),
                    ]
                    .into()
                }
            }
            Tab::Details => self.series_details_tab(item, env, inner),
        };
        column![
            banner,
            gap(18.0),
            container(tabs).padding(Padding::default().left(m)),
            hline(),
            gap(28.0),
            container(container(body).width(inner)).padding(Padding::default().left(m)),
        ]
        .into()
    }

    pub(super) fn season_picker<'a>(
        &self,
        item: &Item,
        season: usize,
        width: f32,
    ) -> Element<'a, Msg> {
        let opts: Vec<SeasonOpt> = item
            .seasons
            .iter()
            .enumerate()
            .map(|(i, s)| SeasonOpt(i, s.label()))
            .collect();
        let current = opts.get(season).cloned();
        picker(pick_list(opts, current, Msg::Season).width(width))
    }

    /// A season's episodes as cards, up to four a row.
    fn episodes_tab<'a>(
        &self,
        series: &Item,
        si: usize,
        eps: &[Episode],
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let n = eps.len();
        let gap_x = 24.0;
        let cols = ((w + gap_x) / (300.0 + gap_x)).floor().clamp(1.0, 4.0);
        let cw = (w - gap_x * (cols - 1.0)) / cols;
        let mut list = column![
            text(format!("{n} episode{}", if n == 1 { "" } else { "s" }))
                .size(14)
                .color(style::MUTED())
        ];
        if let Some(note) = crate::ui::browse::seen_note(series, si, env.progress, &|i| self.key(i))
        {
            list = list
                .push(gap(10.0))
                .push(text(note).font(BOLD).size(14).color(c(TEAL())));
        }
        list = list.push(gap(14.0));
        for chunk in eps.chunks(cols as usize) {
            let mut r = row![].spacing(gap_x);
            for ep in chunk {
                r = r.push(self.episode_card(series, ep, cw, env));
            }
            list = list.push(r).push(gap(26.0));
        }
        list.into()
    }

    /// An episode: its still (hover for a play button; the rest opens the
    /// episode), "N. Title", two lines of synopsis, the rating, length and date.
    fn episode_card<'a>(
        &self,
        series: &Item,
        ep: &Episode,
        w: f32,
        env: Env<'a>,
    ) -> Element<'a, Msg> {
        let h = w * 9.0 / 16.0;
        let key = format!("ep\u{1f}{}", ep.id);
        let lit = self.hovered.as_deref() == Some(key.as_str());
        let art = ep.still.as_deref().or(series.backdrop.as_deref());
        let mut layers: Vec<Element<'a, Msg>> =
            vec![cards::art(env.images, art, &ep.title, w, h, 6.0)];
        if let Some(e) = env.progress.get(&self.key(&ep.id)) {
            let f = if e.watched() { 1.0 } else { e.fraction() };
            layers.push(gold_bar(f, w));
        }
        let open = Msg::Nav(View::Episode(series.id.clone(), ep.id.clone()));
        if lit {
            layers.push(outline(w, h, 6.0));
            let play = button(container(icon::icon(Icon::Play, style::TEXT(), 24.0)).center(56))
                .width(56)
                .height(56)
                .padding(0)
                .style(|_, s| button::Style {
                    background: Some(Background::Color(black(if hot(s) { 230 } else { 170 }))),
                    border: iced::border::rounded(28),
                    ..button::Style::default()
                })
                .on_press(Msg::Play(series.id.clone(), Some(ep.id.clone()), false));
            layers.push(
                container(tip_below(play, "Play"))
                    .center(Length::Fill)
                    .into(),
            );
        }
        let still = hover_area(stack(layers).width(w).height(h), key, open.clone());
        let title = mouse_area(
            text(format!("{}. {}", ep.number, ep.title))
                .font(BOLD)
                .size(15)
                .color(style::TEXT())
                .wrapping(text::Wrapping::None),
        )
        .on_press(open)
        .interaction(iced::mouse::Interaction::Pointer);
        let mut card = column![still, gap(10.0), container(title).width(w).clip(true)].width(w);
        if !ep.overview.is_empty() {
            card = card
                .push(gap(2.0))
                .push(clamp_text(&ep.overview, 12.5, style::MUTED(), 2, w));
        }
        let mut facts = row![].spacing(8).align_y(Vertical::Center);
        if let Some(r) = &series.rating {
            facts = facts.push(tag(r));
        }
        if let Some(d) = ep.duration.filter(|d| *d > 0.0) {
            facts = facts.push(
                text(format!("{}min", (d / 60.0).round()))
                    .size(12)
                    .color(style::MUTED()),
            );
        }
        if let Some(a) = &ep.aired {
            facts = facts.push(text(date_short(a)).size(12).color(style::MUTED()));
        }
        if crate::ui::browse::episode_watched(env.progress, &|i| self.key(i), &ep.id) {
            facts = facts.push(text("Watched").font(BOLD).size(12).color(c(TEAL())));
        }
        card.push(gap(6.0)).push(facts).into()
    }

    /// About and Info side by side, then the cast.
    fn series_details_tab<'a>(&self, item: &Item, env: Env<'a>, w: f32) -> Element<'a, Msg> {
        let narrow = w < 900.0;
        let cw = if narrow {
            w
        } else {
            ((w - 24.0) / 2.0).min(440.0)
        };
        let mut about = column![text(item.title.clone()).font(BOLD).size(19), gap(6.0)];
        if !item.genres.is_empty() {
            about = about
                .push(text(item.genres.join(" · ")).size(14).color(style::MUTED()))
                .push(gap(8.0));
        }
        let mut line = row![].spacing(10).align_y(Vertical::Center);
        if let Some(y) = years(item) {
            line = line.push(text(y).size(12).color(style::MUTED()));
        }
        let n = item.season_count();
        line = line.push(
            text(format!("{n} season{}", if n == 1 { "" } else { "s" }))
                .size(12)
                .color(style::MUTED()),
        );
        if let Some(r) = &item.rating {
            line = line.push(tag(r));
        }
        about = about.push(line);
        if !item.overview.is_empty() {
            about = about.push(gap(14.0)).push(
                text(item.overview.clone())
                    .size(14)
                    .color(style::TEXT().scale_alpha(0.85)),
            );
        }
        let mut left = column![panel(about)].width(cw);
        let names: Vec<&str> = item.cast.iter().take(8).map(|c| c.name.as_str()).collect();
        if !item.creators.is_empty() || !names.is_empty() {
            let mut people = column![text("Creators and Cast").font(BOLD).size(17), gap(16.0)];
            if !item.creators.is_empty() {
                let creators: Vec<&str> = item.creators.iter().map(String::as_str).collect();
                people = people.push(info_row("Creators", people_links(&creators, 14.0), 96.0));
            }
            if !names.is_empty() {
                people = people.push(info_row(
                    "Cast",
                    text(names.join(", ")).size(14).into(),
                    96.0,
                ));
            }
            left = left.push(gap(24.0)).push(panel(people));
        }
        let eps: usize = item.seasons.iter().map(|s| s.episodes.len()).sum();
        let linked = self.came_from_streaming();
        let services = self.discover.services(&item.id);
        let mut rows = Vec::new();
        if !linked && !services.is_empty() {
            rows.push(("Streaming on", services.join(", ")));
        }
        if let Some(n) = &item.network {
            rows.push(("Network", n.clone()));
        }
        if let Some(s) = &item.status {
            rows.push(("Status", s.clone()));
        }
        rows.push((
            "In library",
            format!("{eps} episode{}", if eps == 1 { "" } else { "s" }),
        ));
        let mut info = column![text("Info").font(BOLD).size(17), gap(16.0)];
        if linked && !services.is_empty() {
            info = info.push(info_row(
                "Streaming on",
                super::discover::service_links(&services, &item.title, 14.0, c(TEAL())),
                112.0,
            ));
        }
        for (k, v) in rows {
            info = info.push(info_row(k, text(v).size(14).into(), 112.0));
        }
        let info = column![panel(info)].width(cw);
        let top: Element<'a, Msg> = if narrow {
            column![left, gap(24.0), info].into()
        } else {
            row![left, info].spacing(24).into()
        };
        if item.cast.is_empty() {
            return top;
        }
        let n = item.cast.len().min(20);
        column![
            top,
            gap(30.0),
            caps("Cast", c(TEAL())),
            gap(4.0),
            text("Series cast").font(BOLD).size(22),
            gap(16.0),
            self.cast_grid(&item.cast[..n], env, w, 150.0, 46.0),
        ]
        .into()
    }

    // ---------------------------------------------------------- episode

    pub(super) fn episode_page<'a>(
        &'a self,
        series: &'a Item,
        si: usize,
        ei: usize,
        env: Env<'a>,
        w: f32,
    ) -> Element<'a, Msg> {
        let season = &series.seasons[si];
        let ep = &season.episodes[ei];
        let order = all_episodes(series);
        let pos = order.iter().position(|x| *x == (si, ei)).unwrap_or(0);
        let ep_at = |i: usize| {
            order
                .get(i)
                .map(|&(s, e)| (&series.seasons[s], &series.seasons[s].episodes[e]))
        };
        let next = ep_at(pos + 1);
        let gold = c(GOLD());

        let h = (w * 0.36).clamp(380.0, 580.0);
        let (m, inner) = column_of(w);
        let art = ep.still.as_deref().or(series.backdrop.as_deref());
        let mut layers = vec![
            backdrop(env, art, &ep.title, 100, w, h),
            container(link(
                &series.title,
                13.0,
                style::TEXT().scale_alpha(0.8),
                Some((Icon::ChevronLeft, false)),
                Msg::Back,
            ))
            .padding(Padding::default().left(28.0).top(20.0))
            .into(),
        ];
        // the big play button, a little above the middle
        let hover_gold = c(shade(GOLD(), 1.08));
        let play = button(container(icon::icon(Icon::Play, c(INK()), 40.0)).center(82))
            .width(92)
            .height(92)
            .padding(5)
            .style(move |_, s| button::Style {
                background: Some(Background::Color(if hot(s) { hover_gold } else { gold })),
                border: Border {
                    color: black(90),
                    width: 5.0,
                    radius: 46.0.into(),
                },
                ..button::Style::default()
            })
            .on_press(Msg::Play(series.id.clone(), Some(ep.id.clone()), false));
        layers.push(
            container(play)
                .width(w)
                .padding(Padding::default().top(h * 0.42 - 46.0))
                .align_x(Horizontal::Center)
                .into(),
        );
        let code = match api::featurette(season.number, ep.number) {
            Some(f) => format!("{}  ·  {f}", series.title),
            None => format!(
                "{}  ·  S{:02}  ·  E{:02}",
                series.title, season.number, ep.number
            ),
        };
        layers.push(
            container(
                column![
                    caps(&code, style::TEXT().scale_alpha(0.7)),
                    gap(8.0),
                    text(ep.title.clone())
                        .font(BOLD)
                        .size(52)
                        .color(style::TEXT()),
                ]
                .width(inner * 0.75),
            )
            .width(w)
            .height(h)
            .padding(Padding::default().left(m).bottom(26.0))
            .align_y(Vertical::Bottom)
            .into(),
        );
        if let Some((_, n)) = next {
            layers.push(
                container(tip_below(
                    link(
                        "Next",
                        13.0,
                        style::TEXT().scale_alpha(0.8),
                        Some((Icon::ArrowRight, true)),
                        Msg::Nav(View::Episode(series.id.clone(), n.id.clone())),
                    ),
                    &n.title,
                ))
                .width(m + inner)
                .height(h)
                .padding(Padding::default().bottom(38.0))
                .align_x(Horizontal::Right)
                .align_y(Vertical::Bottom)
                .into(),
            );
        }
        let banner = stack(layers).width(w).height(h);

        let (file, probed) = self.file_for(&ep.stream, ep.file.as_ref());
        let file = file.as_ref();
        let runtime = ep.duration.filter(|d| *d > 0.0).or(probed);
        let faint = style::TEXT().scale_alpha(0.85);
        let mut facts = row![text(format!(
            "{} of {}",
            sxe(season.number, ep.number),
            season.episodes.len()
        ))
        .size(14)
        .color(gold)]
        .spacing(16)
        .align_y(Vertical::Center);
        if let Some(d) = runtime {
            facts = facts
                .push(dot())
                .push(text(minutes(d)).size(14).color(faint));
        }
        if let Some(a) = &ep.aired {
            facts = facts.push(dot()).push(
                text(format!("Aired {}", date_long(a)))
                    .size(14)
                    .color(faint),
            );
        }
        if let Some(s) = ep.score {
            facts = facts
                .push(dot())
                .push(icon::icon(Icon::Star, gold, 14.0))
                .push(text(format!("{s:.1}")).size(14).color(faint));
        }
        if let Some(q) = file.and_then(|f| f.quality.as_deref()) {
            facts = facts.push(dot()).push(tag(q));
        }

        let narrow = inner < NARROW;
        let gap_x = 56.0;
        let lw = if narrow { inner } else { inner * 0.64 };
        let rw = if narrow { inner } else { inner - lw - gap_x };
        let mut left = column![].width(lw);
        if !ep.overview.is_empty() {
            left = left
                .push(caps("Synopsis", style::MUTED()))
                .push(gap(6.0))
                .push(text(ep.title.clone()).font(BOLD).size(30))
                .push(gap(12.0))
                .push(
                    text(ep.overview.clone())
                        .size(16)
                        .color(style::TEXT().scale_alpha(0.8)),
                )
                .push(gap(30.0));
        }
        let mut cells = Vec::new();
        if let Some(n) = file_name(file, &ep.stream) {
            cells.push(("Filename", n, style::TEXT(), 12.5));
        }
        if let Some(q) = file.and_then(|f| f.quality.clone()) {
            cells.push(("Resolution", q, c(TEAL()), 14.0));
        }
        cells.push((
            "Episode",
            sxe(season.number, ep.number),
            style::TEXT(),
            14.0,
        ));
        if let Some(d) = runtime {
            cells.push(("Runtime", minutes(d), style::TEXT(), 14.0));
        }
        if let Some(s) = file.and_then(|f| f.size) {
            cells.push(("File size", bytes(s), style::TEXT(), 14.0));
        }
        let half = (lw - 48.0) / 2.0 - 20.0;
        let file_panel: Element<'a, Msg> = if half < 220.0 {
            kv_rows(cells)
        } else {
            let (mut l, mut r) = (Vec::new(), Vec::new());
            for (i, cell) in cells.into_iter().enumerate() {
                if i % 2 == 0 {
                    l.push(cell);
                } else {
                    r.push(cell);
                }
            }
            row![
                container(kv_rows(l)).width(half),
                container(kv_rows(r)).width(half)
            ]
            .spacing(40)
            .into()
        };
        left = left
            .push(caps("File", style::MUTED()))
            .push(gap(10.0))
            .push(panel(file_panel));
        if !series.cast.is_empty() {
            let n = series.cast.len().min(10);
            left = left
                .push(gap(30.0))
                .push(caps("Series cast", style::MUTED()))
                .push(gap(12.0))
                .push(self.cast_grid(&series.cast[..n], env, lw, 160.0, 40.0));
        }

        // the side: play, what's next, the series
        let label = match env.progress.get(&self.key(&ep.id)) {
            Some(e) if e.in_progress() => format!("Resume from {}", fmt_time(e.pos)),
            Some(e) if e.watched() => "Watch Again".to_owned(),
            _ => "Play Episode".to_owned(),
        };
        let mut side = column![fill_button(
            &label,
            Some(Icon::Play),
            play_colors(),
            48.0,
            rw,
            6.0,
            Some(Msg::Play(series.id.clone(), Some(ep.id.clone()), false)),
        )]
        .width(rw);
        let upcoming: Vec<_> = (pos + 1..pos + 4).filter_map(ep_at).collect();
        if !upcoming.is_empty() {
            side = side
                .push(gap(26.0))
                .push(caps("Up next", style::MUTED()))
                .push(gap(10.0));
            for (s, e) in upcoming {
                let key = format!("next\u{1f}{}", e.id);
                let lit = self.hovered.as_deref() == Some(key.as_str());
                let thumb = cards::art(
                    env.images,
                    e.still.as_deref().or(series.backdrop.as_deref()),
                    &e.title,
                    96.0,
                    54.0,
                    4.0,
                );
                let entry = row![
                    thumb,
                    column![
                        text(sxe(s.number, e.number)).size(11).color(style::MUTED()),
                        text(e.title.clone())
                            .size(14.5)
                            .color(if lit {
                                style::TEXT()
                            } else {
                                style::TEXT().scale_alpha(0.9)
                            })
                            .wrapping(text::Wrapping::None),
                    ]
                    .spacing(2)
                    .padding(Padding::default().top(6.0)),
                ]
                .spacing(14)
                .height(58);
                side = side
                    .push(hover_area(
                        container(entry).width(Length::Fill).clip(true),
                        key,
                        Msg::Nav(View::Episode(series.id.clone(), e.id.clone())),
                    ))
                    .push(gap(14.0));
            }
        }
        let mut about = column![caps("About the series", style::MUTED())];
        if !series.overview.is_empty() {
            about = about.push(gap(10.0)).push(
                text(series.overview.clone())
                    .size(14)
                    .color(style::TEXT().scale_alpha(0.8)),
            );
        }
        about = about.push(gap(14.0)).push(link(
            "View all episodes",
            12.5,
            gold,
            Some((Icon::ArrowRight, true)),
            Msg::Nav(View::Details(series.id.clone())),
        ));
        side = side.push(gap(20.0)).push(panel(about));

        let below: Element<'a, Msg> = if narrow {
            left.push(gap(20.0)).push(side).into()
        } else {
            row![left, space().width(gap_x), side].into()
        };
        column![
            banner,
            gap(26.0),
            container(facts).padding(Padding::default().left(m)),
            gap(30.0),
            container(below).padding(Padding::default().left(m)),
        ]
        .into()
    }
}
