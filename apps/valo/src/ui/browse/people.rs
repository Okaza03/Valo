//! Person pages (a cast member, director or creator, built from what the
//! library knows about them) and the signed-in user's profile page.
use std::collections::HashMap;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, FontFamily, FontId, Rect, RichText, Sense,
    Stroke, Ui,
};

use super::details::{
    backdrop, caps, column, image_circle, kv_rows, link, panel, placed, poster_tile, GOLD, NARROW,
    TEAL,
};
use super::{makers, Browse, Click, Filter, View, GAP};
use crate::api::{Item, Kind, Library};
use crate::config::{Entry, Progress};
use crate::images::Images;
use crate::ui::chrome;
use crate::ui::{black_font, bold, gradient, image_cover, mono, placeholder, small_button};
use crate::ui::{Action, BG, LINE, MUTED, TEXT};
use crate::util::{color_for, fmt_runtime, now_unix};

fn same(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// Newest first, titles without a year last.
fn by_year(v: &mut [Credit]) {
    v.sort_by_key(|c| (c.item.year.is_none(), std::cmp::Reverse(c.item.year)));
}

struct Credit<'a> {
    item: &'a Item,
    /// The character played, for acting credits.
    character: Option<&'a str>,
}

/// Everything one person did in the library.
struct Person<'a> {
    movies: Vec<Credit<'a>>,
    shows: Vec<Credit<'a>>,
    directed: Vec<Credit<'a>>,
    created: Vec<Credit<'a>>,
    photo: Option<&'a str>,
}

impl<'a> Person<'a> {
    fn find(lib: &'a Library, name: &str) -> Self {
        let mut p = Person {
            movies: Vec::new(),
            shows: Vec::new(),
            directed: Vec::new(),
            created: Vec::new(),
            photo: None,
        };
        for item in lib.all() {
            if let Some(m) = item.cast.iter().find(|c| same(&c.name, name)) {
                p.photo = p.photo.or(m.photo.as_deref());
                let c = Credit {
                    item,
                    character: m.character.as_deref().filter(|s| !s.trim().is_empty()),
                };
                match item.kind {
                    Kind::Movie => p.movies.push(c),
                    Kind::Series => p.shows.push(c),
                }
            }
            let directed = item
                .director
                .iter()
                .flat_map(|d| d.split(','))
                .any(|d| same(d, name));
            if directed {
                p.directed.push(Credit {
                    item,
                    character: None,
                });
            }
            if item.creators.iter().any(|c| same(c, name)) {
                p.created.push(Credit {
                    item,
                    character: None,
                });
            }
        }
        for v in [&mut p.movies, &mut p.shows, &mut p.directed, &mut p.created] {
            by_year(v);
        }
        p
    }

    /// Every distinct title, each once.
    fn titles(&self) -> Vec<&'a Item> {
        let mut v: Vec<&Item> = Vec::new();
        for c in [&self.movies, &self.shows, &self.directed, &self.created]
            .into_iter()
            .flatten()
        {
            if !v.iter().any(|i| i.id == c.item.id) {
                v.push(c.item);
            }
        }
        v
    }

    fn roles(&self) -> Vec<&'static str> {
        let mut r = Vec::new();
        if !self.movies.is_empty() || !self.shows.is_empty() {
            r.push("Actor");
        }
        if !self.directed.is_empty() {
            r.push("Director");
        }
        if !self.created.is_empty() {
            r.push("Creator");
        }
        r
    }
}

/// "1999–2021", or one year.
fn span(items: &[&Item]) -> Option<String> {
    let lo = items.iter().filter_map(|i| i.year).min()?;
    let hi = items
        .iter()
        .filter_map(|i| i.end_year.or(i.year))
        .max()
        .unwrap_or(lo);
    Some(if hi > lo {
        format!("{lo}–{hi}")
    } else {
        lo.to_string()
    })
}

/// The `n` most common of `names`, most common first, ties A to Z.
fn top<'a>(names: impl Iterator<Item = &'a str>, n: usize) -> Vec<(&'a str, usize)> {
    let mut count: HashMap<&str, usize> = HashMap::new();
    for g in names {
        *count.entry(g).or_default() += 1;
    }
    let mut v: Vec<(&str, usize)> = count.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    v.truncate(n);
    v
}

/// "just now", "3 hours ago", "yesterday", "12 days ago", …
pub(super) fn ago(t: i64) -> String {
    let s = (now_unix() - t).max(0);
    let plural = |n: i64, unit: &str| format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
    match s {
        s if s < 60 => "just now".into(),
        s if s < 3600 => plural(s / 60, "minute"),
        s if s < 86_400 => plural(s / 3600, "hour"),
        s if s < 2 * 86_400 => "yesterday".into(),
        s if s < 60 * 86_400 => plural(s / 86_400, "day"),
        s if s < 365 * 86_400 => plural(s / (30 * 86_400), "month"),
        s => plural(s / (365 * 86_400), "year"),
    }
}

/// A grid of poster tiles, `sub` under each title.
fn credit_grid(ui: &mut Ui, credits: &[Credit], images: &mut Images) -> Option<Click> {
    let mut click = None;
    let w = ui.available_width();
    let cols = ((w + GAP) / (160.0 + GAP)).floor().clamp(1.0, 6.0);
    let tw = (w - GAP * (cols - 1.0)) / cols;
    for row in credits.chunks(cols as usize) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for c in row {
                let year = c.item.year.map(|y| y.to_string());
                let sub = match (c.character, year) {
                    (Some(ch), Some(y)) => Some(format!("as {ch} · {y}")),
                    (Some(ch), None) => Some(format!("as {ch}")),
                    (None, y) => y,
                };
                if poster_tile(ui, c.item, sub.as_deref(), tw, images) {
                    click = Some(Click::Open(c.item.id.clone()));
                }
            }
        });
        ui.add_space(12.0);
    }
    click
}

pub(super) fn section_title(ui: &mut Ui, kicker: &str, title: &str, n: usize) {
    caps(ui, kicker, TEAL());
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(bold(title, 24.0));
        ui.label(mono(n.to_string(), 13.0).color(MUTED()));
    });
    ui.add_space(16.0);
}

/// One big number with a label under it.
pub(super) fn stat(ui: &mut Ui, w: f32, value: &str, label: &str) {
    let (r, _) = ui.allocate_exact_size(vec2(w, 92.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 10.0, crate::ui::CARD());
    p.rect_stroke(
        r,
        10.0,
        Stroke::new(1.0f32, LINE()),
        egui::StrokeKind::Inside,
    );
    p.text(
        pos2(r.left() + 18.0, r.top() + 18.0),
        Align2::LEFT_TOP,
        value,
        FontId::new(28.0, FontFamily::Name("bold".into())),
        TEXT(),
    );
    p.text(
        pos2(r.left() + 18.0, r.bottom() - 16.0),
        Align2::LEFT_BOTTOM,
        label,
        FontId::monospace(11.0),
        MUTED(),
    );
}

/// A row of [`stat`] tiles that wraps on narrow windows.
pub(super) fn stats(ui: &mut Ui, tiles: &[(String, &str)]) {
    let w = ui.available_width();
    let gap = 14.0;
    let cols = ((w + gap) / (170.0 + gap))
        .floor()
        .clamp(1.0, tiles.len() as f32);
    let tw = (w - gap * (cols - 1.0)) / cols;
    for row in tiles.chunks(cols as usize) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (v, l) in row {
                stat(ui, tw, v, l);
            }
        });
        ui.add_space(gap);
    }
}

/// A labelled bar, for "Favorite genres".
pub(super) fn bar(ui: &mut Ui, label: &str, value: &str, f: f32) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 36.0), Sense::hover());
    let p = ui.painter();
    p.text(
        r.left_top(),
        Align2::LEFT_TOP,
        label,
        FontId::proportional(14.0),
        TEXT(),
    );
    p.text(
        r.right_top(),
        Align2::RIGHT_TOP,
        value,
        FontId::monospace(11.0),
        MUTED(),
    );
    let track = Rect::from_min_max(
        pos2(r.left(), r.bottom() - 9.0),
        pos2(r.right(), r.bottom() - 4.0),
    );
    p.rect_filled(track, 2.0, Color32::from_white_alpha(20));
    p.rect_filled(
        Rect::from_min_max(
            track.min,
            pos2(
                track.left() + track.width() * f.clamp(0.0, 1.0),
                track.bottom(),
            ),
        ),
        2.0,
        GOLD(),
    );
}

/// Two columns (main, side) inside the content column; stacked when narrow.
pub(super) fn columns(ui: &mut Ui, main: impl FnOnce(&mut Ui), side: impl FnOnce(&mut Ui)) {
    columns_with(ui, &mut (), |ui, _| main(ui), |ui, _| side(ui));
}

/// [`columns`], with `state` handed to each side in turn.
pub(super) fn columns_with<T>(
    ui: &mut Ui,
    state: &mut T,
    main: impl FnOnce(&mut Ui, &mut T),
    side: impl FnOnce(&mut Ui, &mut T),
) {
    let (m, inner) = column(ui);
    let narrow = inner < NARROW;
    let gap = 48.0;
    let lw = if narrow { inner } else { inner * 0.66 };
    let rw = if narrow { inner } else { inner - lw - gap };
    let sp = ui.spacing().item_spacing.x;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(m);
        if narrow {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.x = sp;
                ui.set_width(lw);
                main(ui, state);
                ui.add_space(24.0);
                side(ui, state);
            });
            return;
        }
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.x = sp;
            ui.set_width(lw);
            main(ui, state);
        });
        ui.add_space(gap);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.x = sp;
            ui.set_width(rw);
            side(ui, state);
        });
    });
}

impl Browse {
    pub(super) fn person_page(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        name: &str,
        images: &mut Images,
        progress: &Progress,
    ) -> Option<Click> {
        let mut click = None;
        let person = Person::find(lib, name);
        let titles = person.titles();

        // header over the backdrop of their newest title
        let w = ui.available_width();
        let h = 380.0;
        let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        let art = titles
            .iter()
            .filter(|i| i.backdrop.is_some())
            .max_by_key(|i| i.year)
            .and_then(|i| i.backdrop.as_ref())
            .and_then(|u| images.get(u));
        backdrop(&ui.painter_at(rect), rect, art.as_ref(), name, 70);
        let back = Rect::from_min_size(rect.min + vec2(28.0, 22.0), vec2(300.0, 24.0));
        placed(ui, back, "person_back", 0.0, |ui| {
            if link(
                ui,
                "Back",
                13.0,
                TEXT().gamma_multiply(0.8),
                Some((chrome::Icon::ChevronLeft, false)),
            )
            .clicked()
            {
                click = Some(Click::Back);
            }
        });

        let (m, inner) = column(ui);
        let r = 92.0;
        let c = pos2(rect.left() + m + r, rect.center().y + 14.0);
        let p = ui.painter_at(rect);
        p.circle_filled(c, r + 4.0, Color32::from_black_alpha(120));
        let wikidata = self.discover.person_photo(name);
        match person
            .photo
            .or(wikidata.as_deref())
            .and_then(|u| images.get(u))
        {
            Some(t) => image_circle(&p, c, r, &t),
            None => chrome::avatar(&p, c, r, name),
        }
        let tx = c.x + r + 36.0;
        let text_rect = Rect::from_min_max(
            pos2(tx, rect.top() + 50.0),
            pos2(rect.left() + m + inner, rect.bottom() - 20.0),
        );
        let roles = person.roles();
        placed(ui, text_rect, "person_hero", 0.5, |ui| {
            let kicker = if roles.is_empty() {
                "Person".to_owned()
            } else {
                roles.join("   ·   ")
            };
            caps(ui, &kicker, TEAL());
            ui.add_space(6.0);
            ui.add(
                egui::Label::new(RichText::new(name).font(black_font(50.0)).color(TEXT())).wrap(),
            );
            ui.add_space(8.0);
            let mut meta = vec![format!(
                "{} title{} in your library",
                titles.len(),
                if titles.len() == 1 { "" } else { "s" }
            )];
            meta.extend(span(&titles));
            ui.label(
                RichText::new(meta.join("   ·   "))
                    .size(15.0)
                    .color(TEXT().gamma_multiply(0.85)),
            );
            // their best-known part: the highest-scored title they acted in
            let known = person.movies.iter().chain(&person.shows).max_by(|a, b| {
                let s = |c: &&Credit| c.item.score.unwrap_or(0.0);
                s(a).total_cmp(&s(b))
            });
            if let Some(k) = known {
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Known for").size(14.0).color(MUTED()));
                    if link(ui, &k.item.title, 14.0, GOLD(), None).clicked() {
                        click = Some(Click::Open(k.item.id.clone()));
                    }
                    if let Some(ch) = k.character {
                        ui.label(RichText::new(format!("as {ch}")).size(14.0).color(MUTED()));
                    }
                });
            }
        });

        ui.add_space(28.0);
        if titles.is_empty() {
            ui.vertical_centered(|ui| {
                ui.label(mono("Nothing in your library with them", 12.0).color(MUTED()))
            });
            ui.add_space(20.0);
            return self
                .filmography_section(ui, lib, name, images, progress)
                .or(click);
        }
        let watched = titles
            .iter()
            .filter(|i| self.status(i, progress) == Filter::Watched)
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
        let mut rows: Vec<(&str, String, Color32, f32)> = Vec::new();
        let mut row = |k, v: String| rows.push((k, v, TEXT(), 14.0));
        if !roles.is_empty() {
            row("Roles", roles.join(", "));
        }
        for (k, n) in [
            ("Movies", person.movies.len()),
            ("TV shows", person.shows.len()),
            ("Directed", person.directed.len()),
            ("Created", person.created.len()),
        ] {
            if n > 0 {
                row(k, n.to_string());
            }
        }
        if let Some(s) = span(&titles) {
            row("Years", s);
        }
        if !genres.is_empty() {
            row(
                "Genres",
                genres.iter().map(|g| g.0).collect::<Vec<_>>().join(", "),
            );
        }
        if !scores.is_empty() {
            row(
                "Average score",
                format!("{:.1}", scores.iter().sum::<f32>() / scores.len() as f32),
            );
        }
        row("You've watched", format!("{watched} of {}", titles.len()));

        let mut main_click = None;
        let mut side_click = None;
        columns(
            ui,
            |ui| {
                for (kicker, title, list) in [
                    ("In your library", "Movies", &person.movies),
                    ("In your library", "TV shows", &person.shows),
                    ("Behind the camera", "Directed", &person.directed),
                    ("Behind the camera", "Created", &person.created),
                ] {
                    if list.is_empty() {
                        continue;
                    }
                    section_title(ui, kicker, title, list.len());
                    if let Some(c) = credit_grid(ui, list, images) {
                        main_click = Some(c);
                    }
                    ui.add_space(24.0);
                }
            },
            |ui| {
                panel(ui, |ui| {
                    caps(ui, "About", TEAL());
                    ui.add_space(6.0);
                    kv_rows(ui, &rows);
                });
                if !costars.is_empty() {
                    ui.add_space(20.0);
                    panel(ui, |ui| {
                        caps(ui, "Often works with", TEAL());
                        ui.add_space(10.0);
                        for (n, k) in &costars {
                            ui.horizontal(|ui| {
                                if link(ui, n, 14.0, TEXT(), None).clicked() {
                                    side_click = Some(Click::Nav(View::Person((*n).to_owned())));
                                }
                                ui.label(
                                    mono(
                                        format!("{k} title{}", if *k == 1 { "" } else { "s" }),
                                        11.0,
                                    )
                                    .color(MUTED()),
                                );
                            });
                            ui.add_space(6.0);
                        }
                    });
                }
            },
        );
        ui.add_space(12.0);
        let all = self.filmography_section(ui, lib, name, images, progress);
        click.or(main_click).or(side_click).or(all)
    }

    pub(super) fn profile_page(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        images: &mut Images,
        progress: &Progress,
        action: &mut Action,
    ) -> Option<Click> {
        let name = self.user.clone().unwrap_or_else(|| "Guest".to_owned());
        let prefix = self.prefix();
        let entries = progress.recent(&prefix);

        // what each entry is: (movie or series, episode label)
        let watched_secs = |e: &Entry| if e.dur > 0.0 { e.pos.min(e.dur) } else { e.pos };
        let (mut movies_done, mut eps_done, mut in_progress) = (0, 0, 0);
        let (mut movie_secs, mut ep_secs) = (0.0, 0.0);
        let mut genre_secs: HashMap<&str, f64> = HashMap::new();
        let mut per_show: HashMap<&str, (usize, f64)> = HashMap::new();
        let mut recent: Vec<(&Item, Option<String>, Option<String>, &Entry)> = Vec::new();
        for (id, e) in &entries {
            let secs = watched_secs(e);
            let item = if let Some(m) = lib.movies.iter().find(|m| m.id == *id) {
                movie_secs += secs;
                movies_done += usize::from(e.watched());
                recent.push((m, None, None, e));
                m
            } else if let Some((s, si, ei)) = lib.find_episode(id) {
                ep_secs += secs;
                eps_done += usize::from(e.watched());
                let show = per_show.entry(s.id.as_str()).or_default();
                show.0 += usize::from(e.watched());
                show.1 += secs;
                let season = &s.seasons[si];
                let ep = &season.episodes[ei];
                recent.push((
                    s,
                    Some(ep.id.clone()),
                    Some(format!(
                        "S{:02}E{:02} · {}",
                        season.number, ep.number, ep.title
                    )),
                    e,
                ));
                s
            } else {
                continue;
            };
            in_progress += usize::from(e.in_progress());
            for g in &item.genres {
                *genre_secs.entry(g.as_str()).or_default() += secs;
            }
        }
        let shows_done = lib
            .series
            .iter()
            .filter(|s| self.status(s, progress) == Filter::Watched)
            .count();
        let on_list = self
            .watchlist
            .ids(&prefix)
            .filter(|id| lib.find(id).is_some())
            .count();
        let total = movie_secs + ep_secs;

        // header
        let w = ui.available_width();
        let h = 260.0;
        let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        let p = ui.painter_at(rect);
        let col = color_for(&name);
        gradient(
            &p,
            rect,
            [
                col.gamma_multiply(0.35),
                col.gamma_multiply(0.15),
                BG(),
                BG(),
            ],
        );
        let (m, inner) = column(ui);
        let r = 62.0;
        let c = pos2(rect.left() + m + r, rect.center().y + 10.0);
        p.circle_filled(c, r + 4.0, Color32::from_black_alpha(110));
        chrome::avatar(&p, c, r, &name);
        let text_rect = Rect::from_min_max(
            pos2(c.x + r + 32.0, rect.top() + 40.0),
            pos2(rect.left() + m + inner, rect.bottom() - 20.0),
        );
        let mut open_admin = false;
        placed(ui, text_rect, "profile_hero", 0.5, |ui| {
            caps(ui, "Profile", TEAL());
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(RichText::new(&name).font(black_font(44.0)).color(TEXT())).wrap(),
            );
            ui.add_space(6.0);
            let how = if self.client.token.is_some() {
                "Signed in to"
            } else {
                "Connected to"
            };
            let mut line = format!("{how} {}   ·   {}", self.client.info.name, self.client.base);
            if self.client.admin {
                line.push_str("   ·   Admin");
            }
            ui.label(mono(line, 11.5).color(MUTED()));
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if small_button(ui, "Settings").clicked() {
                    *action = Action::OpenSettings;
                }
                if self.client.admin && small_button(ui, "Admin").clicked() {
                    open_admin = true;
                }
                if self.client.token.is_some() && small_button(ui, "Sign out").clicked() {
                    *action = Action::SignOut(self.client.clone());
                }
            });
        });

        ui.add_space(20.0);
        let time = |s: f64| {
            if s > 0.0 {
                fmt_runtime(s)
            } else {
                "—".into()
            }
        };
        super::details::padded(ui, |ui| {
            stats(
                ui,
                &[
                    (time(total), "Time watched"),
                    (movies_done.to_string(), "Movies finished"),
                    (eps_done.to_string(), "Episodes finished"),
                    (shows_done.to_string(), "Shows completed"),
                    (in_progress.to_string(), "In progress"),
                    (on_list.to_string(), "On My list"),
                ],
            );
        });
        ui.add_space(20.0);

        let mut genres: Vec<(&str, f64)> = genre_secs.into_iter().collect();
        genres.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
        genres.truncate(6);
        let mut shows: Vec<(&Item, usize, f64)> = per_show
            .into_iter()
            .filter_map(|(id, (n, s))| Some((lib.find(id)?, n, s)))
            .collect();
        shows.sort_by(|a, b| b.2.total_cmp(&a.2));
        shows.truncate(5);

        let mut main_click = None;
        let mut side_click = None;
        columns(
            ui,
            |ui| {
                section_title(ui, "History", "Recently watched", recent.len().min(15));
                if recent.is_empty() {
                    ui.label(RichText::new("Nothing watched yet.").color(MUTED()));
                }
                for (item, ep, sub, e) in recent.iter().take(15) {
                    let (r, resp) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 72.0), Sense::click());
                    let hot = resp.hovered();
                    let p = ui.painter_at(r);
                    if hot {
                        p.rect_filled(r, 8.0, Color32::from_white_alpha(8));
                    }
                    let thumb = Rect::from_min_size(r.min + vec2(6.0, 6.0), vec2(107.0, 60.0));
                    let still = ep
                        .as_deref()
                        .and_then(|id| lib.find_episode(id))
                        .and_then(|(s, si, ei)| s.seasons[si].episodes[ei].still.as_ref());
                    match still
                        .or(item.backdrop.as_ref())
                        .or(item.poster.as_ref())
                        .and_then(|u| images.get(u))
                    {
                        Some(t) => image_cover(&p, thumb, &t, 4, Color32::WHITE),
                        None => placeholder(&p, thumb, &item.title, 4),
                    }
                    let f = if e.watched() { 1.0 } else { e.fraction() };
                    let track = Rect::from_min_max(
                        pos2(thumb.left() + 6.0, thumb.bottom() - 7.0),
                        pos2(thumb.right() - 6.0, thumb.bottom() - 4.0),
                    );
                    p.rect_filled(track, 1.5, Color32::from_white_alpha(70));
                    p.rect_filled(
                        Rect::from_min_max(
                            track.min,
                            pos2(track.left() + track.width() * f, track.bottom()),
                        ),
                        1.5,
                        GOLD(),
                    );
                    let x = thumb.right() + 16.0;
                    p.text(
                        pos2(x, r.top() + 14.0),
                        Align2::LEFT_TOP,
                        &item.title,
                        FontId::new(15.0, FontFamily::Name("bold".into())),
                        TEXT(),
                    );
                    let sub = sub.clone().unwrap_or_else(|| {
                        item.year.map_or("Movie".into(), |y| format!("Movie · {y}"))
                    });
                    p.text(
                        pos2(x, r.top() + 36.0),
                        Align2::LEFT_TOP,
                        sub,
                        FontId::proportional(12.5),
                        MUTED(),
                    );
                    let state = if e.watched() {
                        "Watched".to_owned()
                    } else {
                        format!("{:.0}%", f * 100.0)
                    };
                    p.text(
                        pos2(r.right() - 10.0, r.top() + 16.0),
                        Align2::RIGHT_TOP,
                        ago(e.updated),
                        FontId::monospace(11.0),
                        MUTED(),
                    );
                    p.text(
                        pos2(r.right() - 10.0, r.top() + 36.0),
                        Align2::RIGHT_TOP,
                        state,
                        FontId::monospace(11.0),
                        if e.watched() { TEAL() } else { GOLD() },
                    );
                    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        main_click = Some(match ep {
                            Some(ep) => Click::Nav(View::Episode(item.id.clone(), ep.clone())),
                            None => Click::Open(item.id.clone()),
                        });
                    }
                    ui.add_space(4.0);
                }
            },
            |ui| {
                if !genres.is_empty() {
                    panel(ui, |ui| {
                        caps(ui, "Favorite genres", TEAL());
                        ui.add_space(12.0);
                        let max = genres[0].1.max(1.0);
                        for (g, s) in &genres {
                            bar(ui, g, &fmt_runtime(*s), (*s / max) as f32);
                            ui.add_space(6.0);
                        }
                    });
                    ui.add_space(20.0);
                }
                if !shows.is_empty() {
                    panel(ui, |ui| {
                        caps(ui, "Most watched shows", TEAL());
                        ui.add_space(10.0);
                        for (s, done, secs) in &shows {
                            let eps: usize = s.seasons.iter().map(|x| x.episodes.len()).sum();
                            if link(ui, &s.title, 14.5, TEXT(), None).clicked() {
                                side_click = Some(Click::Open(s.id.clone()));
                            }
                            ui.label(
                                mono(
                                    format!("{done} of {eps} episodes · {}", fmt_runtime(*secs)),
                                    11.0,
                                )
                                .color(MUTED()),
                            );
                            ui.add_space(10.0);
                        }
                    });
                    ui.add_space(20.0);
                }
                panel(ui, |ui| {
                    caps(ui, "Watching", TEAL());
                    ui.add_space(6.0);
                    kv_rows(
                        ui,
                        &[
                            ("Movies", time(movie_secs), TEXT(), 14.0),
                            ("TV shows", time(ep_secs), TEXT(), 14.0),
                            (
                                "Movies & episodes started",
                                entries.len().to_string(),
                                TEXT(),
                                14.0,
                            ),
                        ],
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Counted from your watch history on this server.")
                            .small()
                            .color(MUTED()),
                    );
                });
            },
        );
        self.devices_section(ui, action);
        if open_admin {
            return Some(Click::Nav(View::Admin));
        }
        main_click.or(side_click)
    }
}
