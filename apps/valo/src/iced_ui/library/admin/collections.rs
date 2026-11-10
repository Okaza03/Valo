//! Admin › Collections: hand-made groups of titles ("Batman"). In A to Z
//! the library puts a collection's titles together under its name, oldest
//! first, so "Batman" and "The Batman" end up side by side.
use std::collections::{HashMap, HashSet};
use std::fmt;

use iced::alignment::Vertical;
use iced::mouse::Interaction;
use iced::widget::{
    button, checkbox, column, container, mouse_area, pick_list, row, scrollable, space, text,
    text_input, Column,
};
use iced::{Background, Border, Element, Length, Padding};
use serde_json::json;

use super::super::details::{caps, column_of, gap, panel};
use super::super::profile::{field_style, Remote};
use super::super::{cards, picker, scroll_style, Msg as LMsg};
use super::{danger_button, not_set_up, Asks, Cx, Done, JobFn, Msg as AMsg, Section};
use crate::api::{join, Client, Collection, CollectionItem, Item, Kind};
use crate::iced_ui::icon::Icon;
use crate::iced_ui::spaced::mono;
use crate::iced_ui::style::{self, c, BOLD};
use crate::iced_ui::widgets::{self, Kind as Btn};
use crate::ui::browse::details::TEAL;
use crate::ui::theme::{INK, LINE, PRIMARY, SURFACE_HI};

#[derive(Clone, Debug)]
pub enum Msg {
    /// Open a collection (by id), or a new one (None).
    Open(Option<String>),
    /// Show the movies without a collection.
    Loose,
    Name(String),
    Search(String),
    Find(String),
    /// Add a title to the open collection, or take it out.
    Member(String, bool),
    Remove(usize),
    /// Move the member at one place to before another.
    Shift(usize, usize),
    Grab(usize),
    Over(usize),
    Drop,
    Save,
    Undo,
    Cancel,
    ConfirmDelete(bool),
    Delete,
    ConfirmSort(bool),
    SortAll,
    Detect,
    /// Tick or untick a movie without a collection.
    Pick(String, bool),
    PickAll(Vec<String>),
    UnpickAll,
    Target(Target),
    AddPicked,
}

/// Where ticked movies go: a collection, or a new one (empty id).
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    id: String,
    name: String,
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

#[derive(Default)]
pub struct State {
    list: Remote<Vec<Collection>>,
    /// The open collection's id; `Some("")` while making a new one.
    selected: Option<String>,
    /// The open collection being edited: name and members (our item
    /// ids), in the order the app shows them.
    name: String,
    members: Vec<String>,
    /// What the server has, to tell edits apart.
    saved: (String, Vec<String>),
    /// The member being dragged, and the place it would land before.
    drag: Option<(usize, Option<usize>)>,
    search: String,
    /// Filters the list of collections, by name or by a title in one.
    find: String,
    confirm_delete: bool,
    /// Showing the movies without a collection instead of one collection.
    loose: bool,
    /// The ticked ones, by our id.
    picked: Vec<String>,
    /// Where they go: a collection's id, `""` for a new one.
    target: Option<String>,
    /// Asking "sort every collection by release date?".
    confirm_sort: bool,
}

/// What the server takes to save a collection.
fn save_body(name: &str, members: &[String]) -> serde_json::Value {
    let items: Vec<_> = members
        .iter()
        .filter_map(|m| {
            let (kind, id) = m.split_once(':')?;
            Some(json!({ "type": kind, "id": id }))
        })
        .collect();
    json!({ "name": name, "items": items })
}

/// A collection's members in release order, oldest first: by release date
/// where there is one, else by year; titles with neither keep their place
/// at the end.
fn chronological(members: &[String], items: &[CollectionItem], all: &[Item]) -> Vec<String> {
    let mut out = members.to_vec();
    out.sort_by_cached_key(|m| {
        let item = all.iter().find(|i| i.id == *m);
        let released = item.and_then(|i| i.released.clone()).unwrap_or_default();
        let year = released
            .get(..4)
            .and_then(|y| y.parse::<u32>().ok())
            .or_else(|| item.and_then(|i| i.year))
            .or_else(|| {
                items
                    .iter()
                    .find(|ci| ci.key() == *m)
                    .and_then(|ci| ci.year)
            });
        (year.is_none(), year, released)
    });
    out
}

/// Whether `c` is called something with `q` in it, or has a title that is.
fn matches(c: &Collection, q: &str, cx: &Cx) -> bool {
    q.is_empty()
        || c.name.to_lowercase().contains(q)
        || c.items.iter().any(|ci| {
            let title = ci
                .title
                .clone()
                .or_else(|| cx.find(&ci.key()).map(|i| i.title.clone()));
            title.is_some_and(|t| t.to_lowercase().contains(q))
        })
}

fn send(m: Msg) -> LMsg {
    LMsg::Admin(AMsg::Collections(m))
}

impl State {
    pub fn refresh(&mut self) {
        self.list.retry();
    }

    pub fn missing(&self) -> bool {
        self.list.missing
    }

    pub fn busy(&self) -> bool {
        self.list.loading()
    }

    fn dirty(&self) -> bool {
        (self.name.clone(), self.members.clone()) != self.saved
    }

    fn open(&mut self, c: Option<&Collection>) {
        self.confirm_delete = false;
        self.loose = false;
        self.drag = None;
        self.search.clear();
        match c {
            Some(c) => {
                self.selected = Some(c.id.clone());
                self.name = c.name.clone();
                self.members = c.members();
            }
            None => {
                self.selected = Some(String::new());
                self.name.clear();
                self.members.clear();
            }
        }
        self.saved = (self.name.clone(), self.members.clone());
    }

    pub fn work(&mut self, client: &Client, asks: &mut Asks) {
        if self.list.poll(&mut asks.login) {
            if let Some(l) = self.list.data.as_mut() {
                l.sort_by_key(|c| c.name.to_lowercase());
            }
            // follow the server after a save, unless there are unsaved edits
            let open = self.selected.clone().filter(|s| !s.is_empty());
            let clean = !self.dirty();
            let found = open.and_then(|id| {
                self.list
                    .data
                    .as_ref()?
                    .iter()
                    .find(|c| c.id == id)
                    .cloned()
            });
            match found {
                Some(c) if clean => self.open(Some(&c)),
                Some(_) => {}
                None if self.selected.as_deref() != Some("") => self.selected = None,
                None => {
                    // a new one was saved: open it by name
                    let name = self.name.trim().to_lowercase();
                    let made =
                        self.list.data.as_ref().and_then(|l| {
                            l.iter().find(|c| c.name.to_lowercase() == name).cloned()
                        });
                    if let Some(c) = made.filter(|_| clean) {
                        self.open(Some(&c));
                    }
                }
            }
        }
        if self.list.due() {
            let c = client.clone();
            self.list.load(move || c.admin_collections());
        }
    }

    fn by_id(&self, id: &str) -> Option<Collection> {
        self.list
            .data
            .as_ref()?
            .iter()
            .find(|c| c.id == id)
            .cloned()
    }

    /// `all`: every title we know, for sorting by release date.
    pub fn update(&mut self, m: Msg, all: &[Item]) -> Option<JobFn> {
        match m {
            Msg::Open(Some(id)) => {
                if let Some(c) = self.by_id(&id) {
                    self.open(Some(&c));
                }
            }
            Msg::Open(None) => self.open(None),
            Msg::Loose => {
                self.selected = None;
                self.loose = true;
                self.picked.clear();
                self.search.clear();
            }
            Msg::Name(n) => self.name = n,
            Msg::Search(s) => self.search = s,
            Msg::Find(s) => self.find = s,
            Msg::Member(id, on) => {
                if on {
                    if !self.members.contains(&id) {
                        self.members.push(id);
                    }
                } else {
                    self.members.retain(|m| *m != id);
                }
            }
            Msg::Remove(i) => {
                if i < self.members.len() {
                    self.members.remove(i);
                }
            }
            Msg::Shift(from, to) => {
                if from < self.members.len() {
                    let m = self.members.remove(from);
                    let to = if to > from { to - 1 } else { to };
                    self.members.insert(to.min(self.members.len()), m);
                }
            }
            Msg::Grab(i) => self.drag = Some((i, None)),
            Msg::Over(i) => {
                if let Some((_, to)) = self.drag.as_mut() {
                    *to = Some(i);
                }
            }
            Msg::Drop => {
                if let Some((from, Some(to))) = self.drag.take() {
                    return self.update(Msg::Shift(from, to), all);
                }
            }
            Msg::Undo => (self.name, self.members) = self.saved.clone(),
            Msg::Cancel => self.selected = None,
            Msg::ConfirmDelete(on) => self.confirm_delete = on,
            Msg::ConfirmSort(on) => self.confirm_sort = on,
            Msg::Pick(id, on) => {
                self.picked.retain(|p| *p != id);
                if on {
                    self.picked.push(id);
                }
            }
            Msg::PickAll(ids) => {
                for id in ids {
                    if !self.picked.contains(&id) {
                        self.picked.push(id);
                    }
                }
            }
            Msg::UnpickAll => self.picked.clear(),
            Msg::Target(t) => self.target = Some(t.id),
            Msg::AddPicked => {
                let picked = std::mem::take(&mut self.picked);
                match self
                    .target
                    .clone()
                    .filter(|t| !t.is_empty())
                    .and_then(|t| self.by_id(&t))
                {
                    Some(c) => self.open(Some(&c)),
                    None => self.open(None),
                }
                for id in picked {
                    if !self.members.contains(&id) {
                        self.members.push(id);
                    }
                }
            }
            Msg::Save => {
                let id = self.selected.clone()?;
                let name = self.name.trim().to_owned();
                let body = save_body(&name, &self.members);
                let id = (!id.is_empty()).then_some(id);
                self.saved = (self.name.clone(), self.members.clone());
                return Some(Box::new(move |c: &Client| {
                    c.admin_save_collection(id.as_deref(), &body)?;
                    let verb = if id.is_some() { "Saved" } else { "Created" };
                    Ok(Done {
                        msg: format!("{verb} {name}."),
                        library: true,
                        ..Default::default()
                    })
                }));
            }
            Msg::Delete => {
                let id = self.selected.take()?;
                let name = self.saved.0.clone();
                self.confirm_delete = false;
                return Some(Box::new(move |c: &Client| {
                    c.admin_delete_collection(&id)?;
                    Ok(Done {
                        msg: format!("Deleted {name}."),
                        library: true,
                        ..Default::default()
                    })
                }));
            }
            Msg::SortAll => {
                self.confirm_sort = false;
                let changes: Vec<(String, serde_json::Value)> = self
                    .list
                    .data
                    .iter()
                    .flatten()
                    .filter_map(|c| {
                        let members = c.members();
                        let sorted = chronological(&members, &c.items, all);
                        (sorted != members).then(|| (c.id.clone(), save_body(&c.name, &sorted)))
                    })
                    .collect();
                return Some(Box::new(move |c: &Client| {
                    for (id, body) in &changes {
                        c.admin_save_collection(Some(id), body)?;
                    }
                    Ok(Done {
                        msg: match changes.len() {
                            0 => "Every collection was already in release order.".into(),
                            1 => "Sorted 1 collection by release date.".into(),
                            n => format!("Sorted {n} collections by release date."),
                        },
                        library: !changes.is_empty(),
                        ..Default::default()
                    })
                }));
            }
            Msg::Detect => {
                return Some(Box::new(|c: &Client| {
                    let r = c.admin_call("POST", "collections/detect", None)?;
                    let n = |k: &str| r.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
                    let running = r.get("running").and_then(|v| v.as_bool()) == Some(true);
                    Ok(Done {
                        msg: if running {
                            "Detecting collections on the server; follow the live log, \
                             then press Refresh."
                                .into()
                        } else {
                            format!(
                                "Detected collections: {} new, {} updated.",
                                n("created"),
                                n("updated")
                            )
                        },
                        library: true,
                        ..Default::default()
                    })
                }));
            }
        }
        None
    }

    /// The list on the left, the open collection beside it.
    pub fn view<'a>(&'a self, cx: &Cx<'a>, w: f32) -> Element<'a, LMsg> {
        let (m, inner) = column_of(w);
        let pad = |e: Element<'a, LMsg>| -> Element<'a, LMsg> {
            container(e).padding(Padding::default().left(m)).into()
        };
        if self.list.missing {
            return pad(not_set_up(Section::Collections, &cx.client.info.name));
        }
        let narrow = inner < 900.0;
        let list_w = if narrow { inner } else { 320.0 };
        let g = 32.0;
        let editor_w = if narrow { inner } else { inner - list_w - g };
        let list = container(self.list_panel(cx)).width(list_w);
        let editor = container(self.editor(cx, editor_w)).width(editor_w);
        pad(if narrow {
            column![list, gap(20.0), editor].into()
        } else {
            row![list, space().width(g), editor]
                .align_y(Vertical::Top)
                .into()
        })
    }

    /// Every collection, with how many titles each has.
    fn list_panel<'a>(&'a self, cx: &Cx<'a>) -> Element<'a, LMsg> {
        let all = self.list.data.as_deref().unwrap_or_default();
        let busy = cx.busy;
        let mut col: Column<'a, LMsg> = column![caps("Collections", c(TEAL())), gap(8.0)];
        if let Some(e) = &self.list.error {
            col = col.push(text(e.clone()).size(14.5).color(style::ERROR()));
        }
        let small =
            |t: &str, msg: Option<Msg>| widgets::button_with(Btn::Small, t, None, msg.map(send));
        col = col.push(
            row![
                small("New collection", Some(Msg::Open(None))),
                small("Movies without a collection", Some(Msg::Loose)),
                crate::iced_ui::chrome::tip_below(
                    small(
                        "Sort all by release date",
                        (!busy && !all.is_empty()).then_some(Msg::ConfirmSort(true))
                    ),
                    "Put every collection's titles in release order, oldest first"
                ),
                crate::iced_ui::chrome::tip_below(
                    small("Detect from TMDB", (!busy).then_some(Msg::Detect)),
                    "Make collections from TMDB's (Batman, Harry Potter, …) for every title"
                ),
            ]
            .spacing(8)
            .wrap()
            .vertical_spacing(8),
        );
        if self.confirm_sort {
            col = col
                .push(gap(10.0))
                .push(
                    text(
                        "Put the titles of every collection in release order, oldest first? \
                         Orders you set by hand are replaced.",
                    )
                    .size(14.5),
                )
                .push(gap(6.0))
                .push(
                    row![
                        widgets::primary("Sort all", (!busy).then(|| send(Msg::SortAll))),
                        small("Cancel", Some(Msg::ConfirmSort(false))),
                    ]
                    .spacing(8),
                );
        }
        col = col.push(gap(10.0)).push(
            text_input("Search collections", &self.find)
                .on_input(|s| send(Msg::Find(s)))
                .size(15)
                .padding([9, 10])
                .style(field_style),
        );
        col = col.push(gap(8.0));
        let q = self.find.trim().to_lowercase();
        let shown: Vec<&Collection> = all.iter().filter(|c| matches(c, &q, cx)).collect();
        let mut rows: Column<'a, LMsg> = column![].spacing(2);
        if all.is_empty() && self.list.data.is_some() {
            rows = rows.push(text("None yet.").size(14.5).color(style::MUTED()));
        } else if shown.is_empty() && !all.is_empty() {
            rows = rows.push(
                text("No collections match.")
                    .size(14.5)
                    .color(style::MUTED()),
            );
        }
        for col_ in shown {
            let sel = self.selected.as_deref() == Some(col_.id.as_str());
            rows = rows.push(nav_row(
                &col_.name,
                col_.members().len(),
                sel,
                send(Msg::Open(Some(col_.id.clone()))),
            ));
        }
        col = col.push(
            scrollable(rows)
                .height(Length::Fixed(460.0))
                .style(scroll_style),
        );
        panel(col)
    }

    fn editor<'a>(&'a self, cx: &Cx<'a>, w: f32) -> Element<'a, LMsg> {
        if self.loose {
            return self.loose_movies(cx);
        }
        let Some(id) = self.selected.as_deref() else {
            return panel(column![
                caps("How collections work", c(TEAL())),
                gap(6.0),
                text(
                    "Titles in a collection are sorted under the collection's name in \
                     A to Z, oldest first. So Batman (1989), Batman Returns and The \
                     Batman sit next to each other under B, instead of spread across \
                     the alphabet."
                )
                .size(14.5)
                .color(style::MUTED()),
                gap(10.0),
                text("Pick one from the list, or make a new one.")
                    .size(14.5)
                    .color(style::MUTED()),
            ]);
        };
        let is_new = id.is_empty();
        let busy = cx.busy;
        let dirty = self.dirty();
        let small = |t: &str, msg: Msg| widgets::button_with(Btn::Small, t, None, Some(send(msg)));

        let mut col: Column<'a, LMsg> = column![
            caps(
                if is_new {
                    "New collection"
                } else {
                    "Collection"
                },
                c(TEAL())
            ),
            gap(6.0),
            mono("Name", 10.5).color(style::MUTED()),
            text_input("Batman", &self.name)
                .on_input(|s| send(Msg::Name(s)))
                .size(15)
                .padding([9, 10])
                .style(field_style),
            gap(12.0),
            row![
                text("Titles").font(BOLD).size(20),
                mono(&self.members.len().to_string(), 12.0).color(style::MUTED()),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
            text("Shown in this order in the app, top first. Drag a title to move it.")
                .size(12)
                .color(style::MUTED()),
            gap(8.0),
        ]
        .width(w);
        if self.members.is_empty() {
            col = col.push(
                text("Add titles from the list below.")
                    .size(14.5)
                    .color(style::MUTED()),
            );
        }
        col = col.push(self.member_rows(cx)).push(gap(10.0));

        // actions
        let ok = !busy && dirty && !self.name.trim().is_empty();
        let mut actions = row![widgets::primary(
            if is_new {
                "Create collection"
            } else {
                "Save changes"
            },
            ok.then(|| send(Msg::Save))
        )]
        .spacing(8)
        .align_y(Vertical::Center);
        if dirty && !is_new {
            actions = actions.push(small("Undo changes", Msg::Undo));
        }
        if !is_new {
            actions = actions.push(small("Delete…", Msg::ConfirmDelete(true)));
        } else {
            actions = actions.push(small("Cancel", Msg::Cancel));
        }
        col = col.push(actions.wrap());
        if self.confirm_delete {
            col = col
                .push(gap(10.0))
                .push(
                    text(format!(
                        "Delete {}? Its titles stay in the library; they just go back to their \
                         own place in A to Z.",
                        self.saved.0
                    ))
                    .size(14.5),
                )
                .push(gap(6.0))
                .push(
                    row![
                        danger_button("Delete", (!busy).then(|| send(Msg::Delete))),
                        small("Cancel", Msg::ConfirmDelete(false)),
                    ]
                    .spacing(8),
                );
        }
        col = col.push(gap(24.0));

        // adding titles
        col = col
            .push(caps("Add titles", c(TEAL())))
            .push(gap(6.0))
            .push(
                text_input("Search movies and series", &self.search)
                    .on_input(|s| send(Msg::Search(s)))
                    .size(15)
                    .padding([9, 10])
                    .style(field_style),
            )
            .push(gap(8.0));
        let q = self.search.trim().to_lowercase();
        let mut found: Vec<&Item> = cx
            .movies
            .iter()
            .chain(cx.series)
            .filter(|i| q.is_empty() || i.title.to_lowercase().contains(&q))
            .collect();
        found.sort_by_key(|i| i.title.to_lowercase());
        let mut list: Column<'a, LMsg> = column![].spacing(4);
        for i in found {
            let year = i.year.map(|y| format!(" ({y})")).unwrap_or_default();
            let mut label = format!("{}{year}", i.title);
            if let Some(c) = i.collection.as_ref().filter(|c| **c != self.saved.0) {
                label.push_str(&format!("   · in {c}"));
            }
            let id = i.id.clone();
            list = list.push(
                checkbox(self.members.contains(&i.id))
                    .label(label)
                    .on_toggle(move |on| send(Msg::Member(id.clone(), on)))
                    .size(15)
                    .text_size(14),
            );
        }
        col.push(framed(list, 320.0)).into()
    }

    /// Movies in no collection: tick some and add them to one.
    fn loose_movies<'a>(&'a self, cx: &Cx<'a>) -> Element<'a, LMsg> {
        let all = self.list.data.as_deref().unwrap_or_default();
        let taken: HashSet<String> = all.iter().flat_map(Collection::members).collect();
        let mut loose: Vec<&Item> = cx
            .movies
            .iter()
            .filter(|i| i.collection.is_none() && !taken.contains(&i.id))
            .collect();
        loose.sort_by_key(|i| i.title.to_lowercase());
        let picked: Vec<&String> = self
            .picked
            .iter()
            .filter(|id| loose.iter().any(|i| i.id == **id))
            .collect();
        let small = |t: &str, msg: Msg| widgets::button_with(Btn::Small, t, None, Some(send(msg)));

        let mut col: Column<'a, LMsg> = column![
            caps("Movies without a collection", c(TEAL())),
            gap(6.0),
            text(format!(
                "{} movies aren't in a collection. Tick the ones that belong together and \
                 add them to a collection.",
                loose.len()
            ))
            .size(14.5)
            .color(style::MUTED()),
            gap(10.0),
            text_input("Search movies", &self.search)
                .on_input(|s| send(Msg::Search(s)))
                .size(15)
                .padding([9, 10])
                .style(field_style),
            gap(8.0),
        ];
        let q = self.search.trim().to_lowercase();
        let shown: Vec<&Item> = loose
            .into_iter()
            .filter(|i| q.is_empty() || i.title.to_lowercase().contains(&q))
            .collect();
        let mut ticks = row![small(
            "Tick all shown",
            Msg::PickAll(shown.iter().map(|i| i.id.clone()).collect())
        )]
        .spacing(8);
        if !picked.is_empty() {
            ticks = ticks.push(small("Untick all", Msg::UnpickAll));
        }
        col = col.push(ticks).push(gap(6.0));
        let mut list: Column<'a, LMsg> = column![].spacing(4);
        if shown.is_empty() {
            list = list.push(text("None.").size(14.5).color(style::MUTED()));
        }
        for i in &shown {
            let year = i.year.map(|y| format!(" ({y})")).unwrap_or_default();
            let id = i.id.clone();
            list = list.push(
                checkbox(self.picked.contains(&i.id))
                    .label(format!("{}{year}", i.title))
                    .on_toggle(move |on| send(Msg::Pick(id.clone(), on)))
                    .size(15)
                    .text_size(14),
            );
        }
        col = col.push(framed(list, 420.0)).push(gap(10.0));

        // where they go
        let mut targets = vec![Target {
            id: String::new(),
            name: "New collection".into(),
        }];
        targets.extend(all.iter().map(|c| Target {
            id: c.id.clone(),
            name: c.name.clone(),
        }));
        let chosen = self
            .target
            .as_ref()
            .and_then(|t| targets.iter().find(|x| x.id == *t).cloned());
        let n = picked.len();
        let label = match n {
            1 => "Add 1 movie".to_owned(),
            n => format!("Add {n} movies"),
        };
        let can = n > 0 && chosen.is_some();
        col = col.push(
            row![
                mono("Add to", 10.5).color(style::MUTED()),
                picker(
                    pick_list(targets, chosen, |t| send(Msg::Target(t)))
                        .placeholder("Choose a collection…")
                        .width(220)
                ),
                widgets::primary(&label, can.then(|| send(Msg::AddPicked))),
            ]
            .spacing(10)
            .align_y(Vertical::Center),
        );
        col.push(
            text("The collection opens with them added; press Save to keep it.")
                .size(12)
                .color(style::MUTED()),
        )
        .into()
    }

    /// The members as rows: drag one to a new place, or remove it.
    fn member_rows<'a>(&'a self, cx: &Cx<'a>) -> Element<'a, LMsg> {
        // what the server says about titles neither list has (no file yet)
        let extra: HashMap<String, Item> = self
            .list
            .data
            .iter()
            .flatten()
            .flat_map(|c| &c.items)
            .filter(|ci| ci.title.is_some())
            .map(|ci| {
                let item = Item {
                    id: ci.key(),
                    title: ci.title.clone().unwrap_or_default(),
                    year: ci.year,
                    poster: ci.poster.as_ref().map(|p| join(&cx.client.base, p)),
                    kind: if ci.kind == "series" {
                        Kind::Series
                    } else {
                        Kind::Movie
                    },
                    ..Default::default()
                };
                (ci.key(), item)
            })
            .collect();
        let n = self.members.len();
        let dragging = self.drag.map(|d| d.0);
        let target = self.drag.and_then(|d| d.1);
        let mut col: Column<'a, LMsg> = column![];
        for (idx, id) in self.members.iter().enumerate() {
            let item: Option<&Item> = cx.find(id).or_else(|| extra.get(id));
            let title = item.map_or("(not in the library)", |i| i.title.as_str());
            let sub = item.map(|i| {
                let kind = if i.kind == Kind::Movie {
                    "Movie"
                } else {
                    "Series"
                };
                i.year.map_or(kind.to_owned(), |y| format!("{kind} · {y}"))
            });
            let grip = column![rule(), rule(), rule()].spacing(4).width(12);
            let mut info: Column<'a, LMsg> = column![text(title.to_owned())
                .size(14.5)
                .wrapping(text::Wrapping::None)];
            if let Some(s) = sub {
                info = info.push(mono(&s, 10.0).color(style::MUTED()));
            }
            let mut buttons = row![].spacing(6).align_y(Vertical::Center);
            if idx > 0 {
                buttons = buttons.push(widgets::small_icon_button(
                    Icon::ChevronUp,
                    send(Msg::Shift(idx, idx - 1)),
                ));
            }
            if idx + 1 < n {
                buttons = buttons.push(widgets::small_icon_button(
                    Icon::ChevronDown,
                    send(Msg::Shift(idx, idx + 2)),
                ));
            }
            buttons = buttons.push(danger_button("Remove", Some(send(Msg::Remove(idx)))));
            let line = row![
                mouse_area(container(grip).center_y(46))
                    .on_press(send(Msg::Grab(idx)))
                    .interaction(if dragging.is_some() {
                        Interaction::Grabbing
                    } else {
                        Interaction::Grab
                    }),
                mono(&(idx + 1).to_string(), 11.0).color(style::MUTED()),
                cards::art(
                    cx.images,
                    item.and_then(|i| i.poster.as_deref()),
                    title,
                    31.0,
                    46.0,
                    3.0
                ),
                container(info).width(Length::Fill).clip(true),
                buttons,
            ]
            .spacing(12)
            .align_y(Vertical::Center);
            let lit = dragging == Some(idx);
            let frame = container(line)
                .width(Length::Fill)
                .padding([6, 10])
                .style(move |_| container::Style {
                    background: lit.then(|| Background::Color(c(SURFACE_HI()))),
                    border: Border {
                        color: c(LINE()),
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..container::Style::default()
                });
            col = col
                .push(drop_line(target == Some(idx) && dragging.is_some()))
                .push(mouse_area(frame).on_enter(send(Msg::Over(idx))))
                .push(gap(4.0));
        }
        col = col.push(
            mouse_area(column![
                drop_line(target == Some(n) && dragging.is_some()),
                space().height(10)
            ])
            .on_enter(send(Msg::Over(n))),
        );
        // letting go anywhere on the list drops the dragged title
        mouse_area(col).on_release(send(Msg::Drop)).into()
    }
}

/// A list in a bordered box that scrolls past `max` pixels.
fn framed<'a>(list: Column<'a, LMsg>, max: f32) -> Element<'a, LMsg> {
    container(
        scrollable(list.width(Length::Fill))
            .height(Length::Shrink)
            .style(scroll_style),
    )
    .max_height(max)
    .width(Length::Fill)
    .padding(6)
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

/// A collection in the list: its name and how many titles, filled when open.
fn nav_row<'a>(name: &str, n: usize, selected: bool, msg: LMsg) -> Element<'a, LMsg> {
    let fg = if selected { c(INK()) } else { style::TEXT() };
    let count = if selected {
        c(INK()).scale_alpha(0.6)
    } else {
        style::MUTED()
    };
    button(
        container(
            row![
                text(name.to_owned())
                    .size(14)
                    .color(fg)
                    .wrapping(text::Wrapping::None),
                space::horizontal(),
                mono(&n.to_string(), 11.0).color(count),
            ]
            .align_y(Vertical::Center),
        )
        .center_y(34)
        .clip(true),
    )
    .width(Length::Fill)
    .height(34)
    .padding([0, 10])
    .style(move |_, s| {
        let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: if selected {
                Some(Background::Color(c(PRIMARY())))
            } else if hot {
                Some(Background::Color(c(SURFACE_HI())))
            } else {
                None
            },
            border: iced::border::rounded(4),
            ..button::Style::default()
        }
    })
    .on_press(msg)
    .into()
}

/// One line of a member's grip.
fn rule<'a>() -> Element<'a, LMsg> {
    container(space())
        .width(12)
        .height(1.5)
        .style(|_| container::Style {
            background: Some(Background::Color(style::MUTED())),
            ..container::Style::default()
        })
        .into()
}

/// Where a dragged title would land: a teal line, or nothing.
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
    fn drags_members_and_sorts_by_release() {
        let mut s = State::default();
        s.open(None);
        for id in ["movie:a", "movie:b", "movie:c"] {
            s.update(Msg::Member(id.into(), true), &[]);
        }
        s.update(Msg::Grab(0), &[]);
        s.update(Msg::Over(3), &[]);
        s.update(Msg::Drop, &[]);
        assert_eq!(s.members, ["movie:b", "movie:c", "movie:a"]);
        let all: Vec<Item> = [("movie:a", 1989), ("movie:b", 2022), ("movie:c", 1992)]
            .iter()
            .map(|(id, y)| Item {
                id: id.to_string(),
                year: Some(*y),
                ..Default::default()
            })
            .collect();
        assert_eq!(
            chronological(&s.members, &[], &all),
            ["movie:a", "movie:c", "movie:b"]
        );
    }
}
