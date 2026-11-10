//! Admin › Layout: how the app looks for everyone on the server. For now,
//! which rows the home page shows and in what order: the built-in ones,
//! each addon catalog and each genre.
use iced::alignment::Vertical;
use iced::mouse::Interaction;
use iced::widget::{checkbox, column, container, mouse_area, row, space, text, Column};
use iced::{Background, Element, Length, Padding};

use super::super::details::{caps, column_of, gap, panel, NARROW};
use super::super::profile::Remote;
use super::super::{spin_secs, Msg as LMsg};
use super::{not_set_up, Asks, Cx, Done, JobFn, Msg as AMsg, Section};
use crate::addons::catalog_key;
use crate::api::{Addon, Client, ServerLayout};
use crate::iced_ui::chrome::tip_below;
use crate::iced_ui::icon::Icon;
use crate::iced_ui::spaced::mono;
use crate::iced_ui::style::{self, c, BOLD};
use crate::iced_ui::widgets::{self, Kind as Btn};
use crate::ui::browse::details::TEAL;

/// The home page's own rows: (key, name, what it is). The first two always
/// sit at the top; the rest go in the admin's order.
const HOME_ROWS: [(&str, &str, &str); 10] = [
    (
        "featured",
        "Featured banner",
        "The big artwork at the top, in the styles that have one",
    ),
    (
        "genre_tiles",
        "Genre tiles",
        "Big genre buttons under the banner, in the styles that have them",
    ),
    (
        "continue",
        "Continue watching",
        "Each user's unfinished titles",
    ),
    ("mylist", "My list", "What each user saved"),
    (
        "recent",
        "Recently added",
        "The newest titles on the server",
    ),
    (
        "others",
        "Viewed by other users",
        "What everyone else watched lately, with who under each poster",
    ),
    (
        "top",
        "Top rated",
        "Or the Top 10, in the styles that have one",
    ),
    ("movies", "Movies", "A few movies, in a random order"),
    ("series", "TV shows", "A few series, in a random order"),
    (
        "genres",
        "Genre rows",
        "A row for each genre, up to 8, the biggest first. Users can also turn them off in Settings",
    ),
];

#[derive(Clone, Debug)]
pub enum Msg {
    /// Show or hide a row.
    Set(String, bool),
    /// Move the row at one place in the order to another.
    Move(usize, usize),
    /// Start dragging the row at this place.
    Grab(usize),
    /// The pointer is over this place while dragging (the end: past the last row).
    Over(usize),
    /// Let go of a dragged row.
    Drop,
    Save,
    Undo,
    ShowAll,
    UsualOrder,
}

#[derive(Default)]
pub struct State {
    saved: Remote<ServerLayout>,
    /// The admin's changes, not saved yet.
    draft: Option<ServerLayout>,
    /// For the catalog rows.
    addons: Remote<Vec<Addon>>,
    /// The row being dragged, and where it would land (places in the order).
    drag: Option<(usize, Option<usize>)>,
}

/// Show or hide a row.
fn set(l: &mut ServerLayout, key: &str, on: bool) {
    l.home_hidden.retain(|k| k != key);
    if !on {
        l.home_hidden.push(key.to_owned());
    }
}

impl State {
    pub fn refresh(&mut self) {
        self.saved.retry();
        self.addons.retry();
    }

    pub fn missing(&self) -> bool {
        self.saved.missing
    }

    pub fn busy(&self) -> bool {
        self.saved.loading() || self.addons.loading()
    }

    pub fn work(&mut self, client: &Client, asks: &mut Asks) {
        if self.saved.poll(&mut asks.login) {
            self.draft = None; // saved, or reloaded
        }
        self.addons.poll(&mut asks.login);
        if self.saved.due() {
            let c = client.clone();
            self.saved.load(move || c.admin_layout());
        }
        if self.addons.due() {
            let c = client.clone();
            self.addons.load(move || c.admin_addons());
        }
    }

    /// The addon catalogs that can be rows: (layout key, "Name · Movies").
    fn catalogs(&self) -> Vec<(String, String)> {
        self.addons
            .data
            .iter()
            .flatten()
            .flat_map(|a| {
                a.catalogs
                    .iter()
                    .filter(|c| c.kind == "movie" || c.kind == "series")
                    .map(move |c| {
                        let what = if c.kind == "movie" {
                            "Movies"
                        } else {
                            "TV shows"
                        };
                        let name = if c.name.trim().is_empty() {
                            &a.name
                        } else {
                            &c.name
                        };
                        (
                            catalog_key(&a.id, &c.kind, &c.id),
                            format!("{} · {what}", name.trim()),
                        )
                    })
            })
            .collect()
    }

    /// Every row the home page can have, in the usual order.
    fn keys(&self) -> Vec<String> {
        let home = crate::ui::browse::HOME_ROWS;
        let mut keys: Vec<String> = home[..5].iter().map(|k| k.to_string()).collect();
        keys.extend(self.catalogs().into_iter().map(|c| c.0));
        keys.extend(home[5..].iter().map(|k| k.to_string()));
        keys
    }

    /// What's on screen: the draft, else what's saved.
    fn current(&self) -> Option<ServerLayout> {
        self.draft.clone().or_else(|| self.saved.data.clone())
    }

    /// Keep `l` as the draft, unless it's what's saved.
    fn keep(&mut self, l: ServerLayout) {
        self.draft = (self.saved.data.as_ref() != Some(&l)).then_some(l);
    }

    pub fn update(&mut self, m: Msg) -> Option<JobFn> {
        let mut l = self.current()?;
        match m {
            Msg::Set(key, on) => set(&mut l, &key, on),
            Msg::Grab(i) => {
                self.drag = Some((i, None));
                return None;
            }
            Msg::Over(i) => {
                if let Some((_, to)) = self.drag.as_mut() {
                    *to = Some(i);
                }
                return None;
            }
            Msg::Drop => {
                let Some((from, Some(before))) = self.drag.take() else {
                    self.drag = None;
                    return None;
                };
                let to = if before > from { before - 1 } else { before };
                return self.update(Msg::Move(from, to));
            }
            Msg::Move(a, b) => {
                let keys = self.keys();
                let mut order = l.ordered(&keys);
                if a == b || a >= order.len() || b >= order.len() {
                    return None;
                }
                let k = order.remove(a);
                order.insert(b, k);
                // keep the places of rows we can't see now, like an addon that didn't load
                order.extend(l.home_order.iter().filter(|k| !keys.contains(k)).cloned());
                l.home_order = order;
            }
            Msg::Undo => {
                self.draft = None;
                return None;
            }
            Msg::ShowAll => l.home_hidden.clear(),
            Msg::UsualOrder => l.home_order.clear(),
            Msg::Save => {
                return Some(Box::new(move |c: &Client| {
                    c.admin_set_layout(&l)?;
                    Ok(Done {
                        msg: "Saved. Everyone sees the new home page the next time it loads."
                            .into(),
                        layout: true,
                        ..Default::default()
                    })
                }));
            }
        }
        self.keep(l);
        None
    }

    pub fn view<'a>(&'a self, cx: &Cx<'a>, w: f32) -> Element<'a, LMsg> {
        let (m, inner) = column_of(w);
        let pad = |e: Element<'a, LMsg>| -> Element<'a, LMsg> {
            container(e).padding(Padding::default().left(m)).into()
        };
        if self.saved.missing {
            return pad(not_set_up(Section::Layout, &cx.client.info.name));
        }
        let (Some(saved), Some(l)) = (self.saved.data.as_ref(), self.current()) else {
            return pad(match &self.saved.error {
                Some(e) => text(format!("Couldn't load the layout: {e}"))
                    .size(14.5)
                    .color(style::ERROR())
                    .into(),
                None => crate::iced_ui::icon::spinner(spin_secs(), style::MUTED(), 20.0).into(),
            });
        };
        let send = |m: Msg| LMsg::Admin(AMsg::Layout(m));
        let narrow = inner < NARROW;
        let g = 48.0;
        let lw = if narrow { inner } else { inner * 0.66 };
        let rw = if narrow { inner } else { inner - lw - g };

        let mut main: Column<'a, LMsg> = column![
            caps("Home page", c(TEAL())),
            gap(4.0),
            text("Rows").font(BOLD).size(24),
            gap(14.0),
        ]
        .width(lw);
        for (key, name, about) in &HOME_ROWS[..2] {
            main = main.push(toggle(&l, key, name, Some(about)));
        }

        main = main
            .push(gap(18.0))
            .push(caps("Rows, top to bottom", style::MUTED()))
            .push(gap(8.0));
        let catalogs = self.catalogs();
        let order = l.ordered(&self.keys());
        let n = order.len();
        let dragging = self.drag.map(|d| d.0);
        let target = self.drag.and_then(|d| d.1);
        let mut rows: Column<'a, LMsg> = column![];
        for (i, key) in order.iter().enumerate() {
            let (name, about) = match HOME_ROWS.iter().find(|r| r.0 == key) {
                Some(r) => (r.1.to_owned(), Some(r.2)),
                None => match catalogs.iter().find(|c| &c.0 == key) {
                    Some(c) => (c.1.clone(), Some("Addon catalog")),
                    None => continue,
                },
            };
            rows = rows.push(drop_line(target == Some(i) && dragging.is_some()));
            let grip = tip_below(
                mouse_area(grip(dragging == Some(i)))
                    .on_press(send(Msg::Grab(i)))
                    .interaction(if dragging.is_some() {
                        Interaction::Grabbing
                    } else {
                        Interaction::Grab
                    }),
                "Drag to move",
            );
            let arrows = row![
                tip_below(
                    widgets::small_icon_button_maybe(
                        Icon::ChevronUp,
                        (i > 0).then(|| send(Msg::Move(i, i - 1)))
                    ),
                    "Move up"
                ),
                tip_below(
                    widgets::small_icon_button_maybe(
                        Icon::ChevronDown,
                        (i + 1 < n).then(|| send(Msg::Move(i, i + 1)))
                    ),
                    "Move down"
                ),
            ]
            .spacing(4);
            let line = row![
                grip,
                container(toggle(&l, key, &name, about)).width(Length::Fill),
                arrows
            ]
            .spacing(8)
            .align_y(Vertical::Top);
            rows = rows.push(mouse_area(line).on_enter(send(Msg::Over(i))));
        }
        rows = rows.push(
            mouse_area(column![
                drop_line(target == Some(n) && dragging.is_some()),
                space().height(12)
            ])
            .on_enter(send(Msg::Over(n))),
        );
        // letting go anywhere on the list drops the dragged row
        main = main.push(mouse_area(rows).on_release(send(Msg::Drop)));
        if catalogs.is_empty() {
            let why = if self.addons.missing {
                "This server doesn't have addons."
            } else if self.addons.loading() {
                "Loading the addons…"
            } else {
                "No catalog addons yet. Add them under Addons."
            };
            main = main.push(mono(why, 11.0).color(style::MUTED()));
        }

        main = main
            .push(gap(18.0))
            .push(caps("Genres", style::MUTED()))
            .push(gap(8.0));
        let genres = cx.lib.genres();
        if genres.is_empty() {
            main = main.push(mono("The library has no genres", 11.0).color(style::MUTED()));
        }
        let can = l.shows("genres") || l.shows("genre_tiles");
        let mut wrap = row![].spacing(14);
        for g in genres {
            let key = format!("genre:{g}");
            let on = l.shows(&key);
            wrap = wrap.push(
                checkbox(on)
                    .label(g)
                    .on_toggle_maybe(can.then_some(move |v| send(Msg::Set(key.clone(), v))))
                    .size(16)
                    .text_size(14),
            );
        }
        main = main.push(wrap.wrap().vertical_spacing(6));

        // the right column: save, undo and how it works
        let changed = &l != saved;
        let hidden = l.home_hidden.len();
        let moved = !l.home_order.is_empty();
        let summary = if changed {
            "You have changes that aren't saved yet.".to_owned()
        } else if hidden == 0 && !moved {
            "Everything shows on the home page, in the usual order.".to_owned()
        } else if hidden == 0 {
            "Everything shows on the home page, in your order.".to_owned()
        } else {
            format!(
                "{hidden} {} hidden from the home page.",
                if hidden == 1 { "row is" } else { "rows are" }
            )
        };
        let mut buttons = row![].spacing(8);
        if changed {
            buttons = buttons.push(widgets::button_with(
                Btn::Small,
                "Undo changes",
                None,
                Some(send(Msg::Undo)),
            ));
        }
        if hidden > 0 {
            buttons = buttons.push(widgets::button_with(
                Btn::Small,
                "Show everything",
                None,
                Some(send(Msg::ShowAll)),
            ));
        }
        if moved {
            buttons = buttons.push(widgets::button_with(
                Btn::Small,
                "Usual order",
                None,
                Some(send(Msg::UsualOrder)),
            ));
        }
        let mut side: Column<'a, LMsg> = column![
            caps("For everyone", c(TEAL())),
            gap(10.0),
            text(summary).size(14.5),
            gap(10.0),
            widgets::primary("Save", (changed && !cx.busy).then(|| send(Msg::Save)))
                .width(Length::Fill)
                .height(38),
            gap(6.0),
            buttons.wrap(),
            gap(14.0),
        ];
        for line in [
            "Unticked rows don't show on anyone's home page. The pages for \
             Movies, TV shows and Streaming still list everything.",
            "Drag a row by its handle, or use the arrows, to move it. The \
             banner and genre tiles always stay at the top.",
            "Rows with nothing in them never show, like Continue watching for \
             someone who hasn't started anything.",
            "Everyone gets the new home page the next time their library loads.",
        ] {
            side = side
                .push(text(line).size(14.5).color(style::MUTED()))
                .push(gap(6.0));
        }
        let side = container(panel(side)).width(rw);
        let body: Element<'a, LMsg> = if narrow {
            column![main, gap(24.0), side].into()
        } else {
            row![main, space().width(g), side].into()
        };
        pad(body)
    }
}

/// A row's checkbox, with a line about it.
fn toggle<'a>(l: &ServerLayout, key: &str, name: &str, about: Option<&str>) -> Element<'a, LMsg> {
    let k = key.to_owned();
    let mut col = column![checkbox(l.shows(key))
        .label(name.to_owned())
        .on_toggle(move |v| LMsg::Admin(AMsg::Layout(Msg::Set(k.clone(), v))))
        .size(16)
        .text_size(15)];
    if let Some(a) = about {
        col = col.push(
            container(text(a.to_owned()).size(12).color(style::MUTED()))
                .padding(Padding::default().left(26.0)),
        );
    }
    col.push(gap(4.0)).into()
}

/// A row's drag handle: two columns of dots.
fn grip<'a>(active: bool) -> Element<'a, LMsg> {
    let col = if active {
        style::TEXT()
    } else {
        style::MUTED()
    };
    let dot = move || {
        container(space())
            .width(3)
            .height(3)
            .style(move |_| container::Style {
                background: Some(Background::Color(col)),
                border: iced::border::rounded(1.5),
                ..container::Style::default()
            })
    };
    let dots = || column![dot(), dot(), dot()].spacing(2);
    container(row![dots(), dots()].spacing(3))
        .center_x(16)
        .center_y(24)
        .into()
}

/// Where a dragged row would land: a teal line, or nothing.
fn drop_line<'a>(on: bool) -> Element<'a, LMsg> {
    container(space())
        .width(Length::Fill)
        .height(2)
        .style(move |_| container::Style {
            background: on.then(|| Background::Color(c(TEAL()))),
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drags_and_moves_rows() {
        let mut s = State::default();
        s.saved.data = Some(ServerLayout::default());
        // "continue" is first; drop it past "recent" (third place)
        s.update(Msg::Grab(0));
        s.update(Msg::Over(3));
        s.update(Msg::Drop);
        let l = s.current().unwrap();
        let keys = s.keys();
        assert_eq!(&l.ordered(&keys)[..3], ["mylist", "recent", "continue"]);
        assert!(s.drag.is_none());
        // back to how it's saved: no draft
        s.update(Msg::UsualOrder);
        assert!(s.draft.is_none());
    }

    #[test]
    fn hides_and_shows_rows() {
        let mut l = ServerLayout::default();
        assert!(l.shows("recent"));
        set(&mut l, "recent", false);
        set(&mut l, "recent", false);
        assert!(!l.shows("recent"));
        assert_eq!(l.home_hidden, ["recent"]);
        set(&mut l, "recent", true);
        assert!(l.shows("recent") && l.home_hidden.is_empty());
    }

    #[test]
    fn orders_rows() {
        let keys: Vec<String> = ["continue", "catalog:a", "movies", "genres"]
            .map(String::from)
            .to_vec();
        let mut l = ServerLayout::default();
        assert_eq!(l.ordered(&keys), keys);
        l.home_order = ["genres", "gone", "continue", "genres"]
            .map(String::from)
            .to_vec();
        // the rest follow the row they came after
        assert_eq!(
            l.ordered(&keys),
            ["genres", "continue", "catalog:a", "movies"]
        );
        l.home_order = ["movies", "continue"].map(String::from).to_vec();
        assert_eq!(
            l.ordered(&keys),
            ["movies", "genres", "continue", "catalog:a"]
        );
    }
}
