//! Admin › Moving: mark titles whose files are being moved between disks.
//! While marked, nobody can play them and their posters carry a red MOVING
//! band; unmark them once they play from their new place.
use std::collections::HashSet;

use iced::alignment::Vertical;
use iced::widget::{checkbox, column, container, row, text, text_input, Column};
use iced::{Background, Border, Element};

use super::super::details::{caps, gap, panel};
use super::super::profile::Remote;
use super::super::{spin_secs, Msg as LMsg};
use super::{fold, not_set_up, Cx, Done, JobFn, Msg as AMsg, Section};
use crate::api::{Client, Item, Kind};
use crate::iced_ui::spaced::mono;
use crate::iced_ui::style::{self, c};
use crate::ui::browse::details::TEAL;
use crate::ui::theme::{LINE, LINE_HI, SURFACE, SURFACE_HI};

/// How many titles of a kind to list before asking for a search.
const SHOW: usize = 300;

#[derive(Clone, Debug)]
pub enum Msg {
    Search(String),
    /// Tick or untick a title (our id).
    Set(String, bool),
    Fold(Kind),
}

#[derive(Default)]
pub struct State {
    /// Our ids of the marked titles.
    marked: Remote<HashSet<String>>,
    search: String,
    /// Lists folded shut.
    folded: HashSet<Kind>,
}

impl State {
    pub fn refresh(&mut self) {
        self.marked.retry();
    }

    pub fn missing(&self) -> bool {
        self.marked.missing
    }

    pub fn busy(&self) -> bool {
        self.marked.loading()
    }

    pub fn work(&mut self, client: &Client, login: &mut bool) {
        self.marked.poll(login);
        if self.marked.due() {
            let c = client.clone();
            self.marked.load(move || c.admin_moving());
        }
    }

    pub fn update(&mut self, m: Msg, items: &[Item]) -> Option<JobFn> {
        match m {
            Msg::Search(s) => self.search = s,
            Msg::Fold(k) => {
                if !self.folded.remove(&k) {
                    self.folded.insert(k);
                }
            }
            Msg::Set(id, on) => {
                let item = items.iter().find(|i| i.id == id)?.clone();
                // show it at once; the reload after the change confirms it
                if let Some(m) = self.marked.data.as_mut() {
                    if on {
                        m.insert(item.id.clone());
                    } else {
                        m.remove(&item.id);
                    }
                }
                return Some(Box::new(move |c: &Client| {
                    c.admin_set_moving(&item, on)?;
                    let msg = if on {
                        format!("Marked as moving (unavailable): {}", item.title)
                    } else {
                        format!("Available again: {}", item.title)
                    };
                    Ok(Done {
                        msg,
                        library: true,
                        ..Default::default()
                    })
                }));
            }
        }
        None
    }

    pub fn view<'a>(&'a self, cx: &Cx<'a>) -> Element<'a, LMsg> {
        if self.marked.missing {
            return not_set_up(Section::Moving, &cx.client.info.name);
        }
        let mut col: Column<'a, LMsg> = column![];
        if let Some(e) = &self.marked.error {
            col = col
                .push(
                    text(format!("Couldn't load the list: {e}"))
                        .size(14.5)
                        .color(style::ERROR()),
                )
                .push(gap(8.0));
        }
        let Some(marked) = self.marked.data.as_ref() else {
            return col
                .push(crate::iced_ui::icon::spinner(
                    spin_secs(),
                    style::MUTED(),
                    18.0,
                ))
                .into();
        };
        // what's unplayable now, on top
        let now: Vec<&Item> = cx
            .series
            .iter()
            .chain(cx.movies)
            .filter(|i| marked.contains(&i.id))
            .collect();
        let mut top: Column<'a, LMsg> = column![caps("Unavailable now", c(TEAL())), gap(8.0)];
        if now.is_empty() {
            top = top.push(
                text(
                    "Nothing. Tick a title below before you move its files; \
                     untick it once it plays from its new place.",
                )
                .size(14.5)
                .color(style::MUTED()),
            );
        }
        for i in &now {
            top = top.push(title_row(i, true, cx.busy));
        }
        col = col
            .push(panel(top))
            .push(gap(16.0))
            .push(container(field(&self.search)).max_width(420))
            .push(gap(12.0));
        let q = self.search.trim().to_lowercase();
        for (kind, label) in [(Kind::Series, "Series"), (Kind::Movie, "Movies")] {
            let hits: Vec<&Item> = cx
                .titles(kind)
                .iter()
                .filter(|i| q.is_empty() || i.title.to_lowercase().contains(&q))
                .collect();
            let open = !self.folded.contains(&kind);
            col = col.push(fold(
                format!("{label}  ·  {}", hits.len()),
                open,
                LMsg::Admin(AMsg::Moving(Msg::Fold(kind))),
            ));
            if open {
                let mut list: Column<'a, LMsg> =
                    column![].padding(iced::Padding::default().left(20.0));
                for i in hits.iter().take(SHOW) {
                    list = list.push(title_row(i, marked.contains(&i.id), cx.busy));
                }
                if hits.len() > SHOW {
                    list = list.push(
                        text(format!(
                            "and {} more; search to find them",
                            hits.len() - SHOW
                        ))
                        .size(14.5)
                        .color(style::MUTED()),
                    );
                }
                col = col.push(list);
            }
            col = col.push(gap(8.0));
        }
        col.into()
    }
}

/// A title with its box.
fn title_row<'a>(i: &Item, on: bool, busy: bool) -> Element<'a, LMsg> {
    let id = i.id.clone();
    let mut b = checkbox(on).size(16).style(check_style);
    if !busy {
        b = b.on_toggle(move |v| LMsg::Admin(AMsg::Moving(Msg::Set(id.clone(), v))));
    }
    let mut r = row![b, text(i.title.clone()).size(14.5)]
        .spacing(10)
        .align_y(Vertical::Center);
    if let Some(y) = i.year {
        r = r.push(mono(&y.to_string(), 10.5).color(style::MUTED()));
    }
    container(r).padding([4, 0]).into()
}

fn check_style(_: &iced::Theme, s: checkbox::Status) -> checkbox::Style {
    let hot = matches!(s, checkbox::Status::Hovered { .. });
    checkbox::Style {
        background: Background::Color(c(if hot { SURFACE_HI() } else { SURFACE() })),
        icon_color: style::TEXT(),
        border: Border {
            color: c(if hot { LINE_HI() } else { LINE() }),
            width: 1.0,
            radius: 3.0.into(),
        },
        text_color: Some(style::TEXT()),
    }
}

fn field<'a>(v: &str) -> Element<'a, LMsg> {
    text_input("Search titles", v)
        .on_input(|s| LMsg::Admin(AMsg::Moving(Msg::Search(s))))
        .size(15)
        .padding([9, 10])
        .style(super::super::profile::field_style)
        .into()
}
