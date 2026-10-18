//! Admin › Analytics: plays per day, the most watched titles and each
//! user's watching, from `GET /api/admin/analytics`. Clicking a user opens
//! their page, from `GET /api/admin/analytics/users/<name>`.
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, FontFamily, FontId, Rect, RichText, Sense,
    Stroke, Ui,
};

use super::super::details::{caps, column, kv_rows, link, padded, panel, placed, GOLD, TEAL};
use super::super::people::{ago, bar, columns, columns_with, section_title, stats};
use super::super::{Click, View};
use super::{needs_note, not_set_up, Cx, Remote, Section};
use crate::api::{Analytics, DayStat, UserAnalytics};
use crate::ui::{black_font, bold, chrome, gradient, image_cover, mono, placeholder, segmented};
use crate::ui::{BG, ERROR, LINE, MUTED, TEXT};
use crate::util::{color_for, fmt_runtime};

pub(in super::super) struct State {
    days: u32,
    data: Remote<Analytics>,
    /// The range `data` is for.
    loaded: u32,
    /// Which kind the top lists show: None for both.
    kind: Option<&'static str>,
    /// The user whose page is open.
    user: Option<String>,
    person: Remote<UserAnalytics>,
    /// The user and range `person` is for.
    person_for: Option<(String, u32)>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            days: 30,
            data: Remote::default(),
            loaded: 30,
            kind: None,
            user: None,
            person: Remote::default(),
            person_for: None,
        }
    }
}

impl State {
    pub fn refresh(&mut self) {
        self.data.retry();
        self.person.retry();
    }

    pub fn missing(&self) -> bool {
        self.data.missing
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        self.data.poll(&mut cx.login);
        if self.days != self.loaded && !self.data.loading() {
            self.data.retry();
            self.data.data = None;
        }
        if self.user.is_some() {
            self.user_ui(ui, cx);
            return;
        }
        if self.data.idle() || (self.data.data.is_some() && self.data.stale(300)) {
            let (c, days) = (cx.client.clone(), self.days);
            self.loaded = days;
            self.data.load(&cx.ctx, move || c.admin_analytics(days));
        }
        if self.data.missing {
            padded(ui, |ui| {
                if not_set_up(ui, Section::Analytics, &cx.client.info.name) {
                    self.data.retry();
                }
            });
            return;
        }

        padded(ui, |ui| {
            range(ui, &mut self.days, self.data.loading());
            ui.add_space(18.0);
            if let Some(e) = &self.data.error {
                ui.label(RichText::new(format!("Couldn't load analytics: {e}")).color(ERROR()));
            }
        });
        let Some(a) = &self.data.data else {
            return;
        };

        padded(ui, |ui| {
            stats(
                ui,
                &[
                    (a.plays.to_string(), "Plays"),
                    (format!("{:.0}", a.hours), "Hours watched"),
                    (a.active_users.to_string(), "Active users"),
                    (
                        if a.plays > 0 {
                            format!("{:.1}", a.hours / a.plays as f64)
                        } else {
                            "—".into()
                        },
                        "Hours per play",
                    ),
                ],
            );
            ui.add_space(6.0);
            panel(ui, |ui| {
                caps(ui, "Plays per day", TEAL());
                ui.add_space(12.0);
                day_chart(ui, &a.days);
            });
        });
        ui.add_space(24.0);

        let kind = &mut self.kind;
        let fits =
            |k: &Option<String>, want: Option<&str>| want.is_none_or(|w| k.as_deref() == Some(w));
        let mut open = None;
        columns_with(
            ui,
            kind,
            |ui, kind| {
                caps(ui, "Most watched", TEAL());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(bold("Top titles", 22.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        segmented(
                            ui,
                            kind,
                            &[
                                (None, "Both", None),
                                (Some("movie"), "Movies", None),
                                (Some("series"), "Series", None),
                            ],
                        );
                    });
                });
                ui.add_space(12.0);
                let top: Vec<_> = a.top.iter().filter(|t| fits(&t.kind, *kind)).collect();
                if top.is_empty() {
                    ui.label(RichText::new("Nothing watched in this range.").color(MUTED()));
                }
                let max = top.iter().map(|t| t.plays).max().unwrap_or(1).max(1);
                for t in top.iter().take(15) {
                    let mut value = format!(
                        "{} play{}   ·   {:.1} h",
                        t.plays,
                        if t.plays == 1 { "" } else { "s" },
                        t.hours
                    );
                    if let Some(u) = t.users {
                        value.push_str(&format!(
                            "   ·   {u} viewer{}",
                            if u == 1 { "" } else { "s" }
                        ));
                    }
                    bar(ui, &t.title, &value, t.plays as f32 / max as f32);
                    ui.add_space(4.0);
                }
                let rated: Vec<_> = a
                    .top_rated
                    .iter()
                    .filter(|t| fits(&t.kind, *kind))
                    .collect();
                if !rated.is_empty() {
                    ui.add_space(20.0);
                    caps(ui, "Users' ratings", TEAL());
                    ui.add_space(4.0);
                    ui.label(bold("Top rated", 22.0));
                    ui.add_space(12.0);
                    for t in rated.iter().take(15) {
                        let value = format!(
                            "{:.1} / 10   ·   {} rating{}",
                            t.avg_rating,
                            t.rating_count,
                            if t.rating_count == 1 { "" } else { "s" }
                        );
                        bar(ui, &t.title, &value, t.avg_rating / 10.0);
                        ui.add_space(4.0);
                    }
                }
            },
            |ui, _| {
                panel(ui, |ui| {
                    caps(ui, "By user", TEAL());
                    ui.add_space(10.0);
                    if a.users.is_empty() {
                        ui.label(RichText::new("No one watched in this range.").color(MUTED()));
                    }
                    for u in &a.users {
                        let row = ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
                            crate::ui::chrome::avatar(ui.painter(), r.center(), 13.0, &u.name);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.label(RichText::new(&u.name).color(TEXT()));
                                let mut line = format!("{} plays · {:.1} h", u.plays, u.hours);
                                if let (Some(m), Some(e)) = (u.movies, u.episodes) {
                                    line = format!("{m} movies · {e} episodes · {:.1} h", u.hours);
                                }
                                if let Some(t) = u.last_active {
                                    line.push_str(&format!(" · {}", ago(t)));
                                }
                                ui.label(mono(line, 10.0).color(MUTED()));
                            });
                        });
                        let resp = row
                            .response
                            .interact(Sense::click())
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text(format!("Open {}'s analytics", u.name));
                        if resp.hovered() {
                            ui.painter().rect_filled(
                                resp.rect.expand(4.0),
                                6.0,
                                Color32::from_white_alpha(8),
                            );
                        }
                        if resp.clicked() {
                            open = Some(u.name.clone());
                        }
                        ui.add_space(8.0);
                    }
                });
            },
        );
        if let Some(name) = open {
            self.user = Some(name);
            cx.to_top = true;
        }
    }

    /// One user's page: who they are, how much, when and what they watch.
    fn user_ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let Some(name) = self.user.clone() else {
            return;
        };
        self.person.poll(&mut cx.login);
        let want = (name.clone(), self.days);
        if self.person_for.as_ref() != Some(&want) && !self.person.loading() {
            self.person.retry();
            self.person.data = None;
        }
        if self.person.idle() || (self.person.data.is_some() && self.person.stale(300)) {
            let c = cx.client.clone();
            let (n, days) = want.clone();
            self.person_for = Some(want);
            self.person
                .load(&cx.ctx, move || c.admin_user_analytics(&n, days));
        }

        let mut back = false;
        padded(ui, |ui| {
            if link(ui, "‹  All users", 14.0, MUTED(), None).clicked() {
                back = true;
            }
        });
        ui.add_space(8.0);
        let a = self.person.data.as_ref();
        hero(ui, &name, a);
        ui.add_space(20.0);
        padded(ui, |ui| {
            range(ui, &mut self.days, self.person.loading());
            ui.add_space(18.0);
            if self.person.missing {
                panel(ui, |ui| {
                    caps(ui, "Not set up", TEAL());
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!(
                            "{} can't show one user's analytics yet.",
                            cx.client.info.name
                        ))
                        .color(MUTED()),
                    );
                    ui.add_space(6.0);
                    needs_note(
                        ui,
                        "It needs GET /api/admin/analytics/users/<name>.",
                        Section::Analytics,
                    );
                });
            }
            if let Some(e) = &self.person.error {
                ui.label(
                    RichText::new(format!("Couldn't load {name}'s analytics: {e}")).color(ERROR()),
                );
            }
        });
        if back {
            self.user = None;
            cx.to_top = true;
            return;
        }
        let Some(a) = &self.person.data else {
            return;
        };

        let secs = a.hours * 3600.0;
        let n = |v: Option<u32>| v.map_or("—".into(), |v| v.to_string());
        padded(ui, |ui| {
            stats(
                ui,
                &[
                    (
                        if secs > 0.0 {
                            fmt_runtime(secs)
                        } else {
                            "—".into()
                        },
                        "Time watched",
                    ),
                    (a.plays.to_string(), "Plays"),
                    (n(a.movies), "Movies"),
                    (n(a.episodes), "Episodes"),
                    (n(a.finished), "Finished"),
                    (n(a.in_progress), "Unfinished"),
                ],
            );
            ui.add_space(6.0);
            panel(ui, |ui| {
                caps(ui, "Plays per day", TEAL());
                ui.add_space(12.0);
                day_chart(ui, &a.days);
            });
        });
        ui.add_space(24.0);

        let mut click = None;
        let mut manage = false;
        columns(
            ui,
            |ui| {
                section_title(ui, "History", "Recently watched", a.recent.len());
                if a.recent.is_empty() {
                    ui.label(RichText::new("Nothing watched in this range.").color(MUTED()));
                }
                for r in &a.recent {
                    if let Some(c) = recent_row(ui, cx, r) {
                        click = Some(c);
                    }
                    ui.add_space(4.0);
                }
                if !a.top.is_empty() {
                    ui.add_space(24.0);
                    section_title(ui, "Most watched", "Top titles", a.top.len());
                    let max = a.top.iter().map(|t| t.hours).fold(0.0, f64::max).max(0.1);
                    for t in &a.top {
                        let value = format!(
                            "{} play{}   ·   {:.1} h",
                            t.plays,
                            if t.plays == 1 { "" } else { "s" },
                            t.hours
                        );
                        let resp = ui
                            .scope(|ui| bar(ui, &t.title, &value, (t.hours / max) as f32))
                            .response;
                        if !t.id.is_empty()
                            && resp
                                .interact(Sense::click())
                                .on_hover_cursor(CursorIcon::PointingHand)
                                .clicked()
                        {
                            click = Some(Click::Open(t.id.clone()));
                        }
                        ui.add_space(4.0);
                    }
                }
            },
            |ui| {
                let hours: u32 = a.hours_of_day.iter().sum();
                let week: u32 = a.weekdays.iter().sum();
                if hours > 0 || week > 0 {
                    panel(ui, |ui| {
                        caps(ui, "When they watch", TEAL());
                        if hours > 0 {
                            ui.add_space(12.0);
                            hour_chart(ui, &a.hours_of_day);
                        }
                        if week > 0 {
                            ui.add_space(14.0);
                            let max = a.weekdays.iter().copied().max().unwrap_or(1).max(1);
                            for (d, v) in [
                                "Monday",
                                "Tuesday",
                                "Wednesday",
                                "Thursday",
                                "Friday",
                                "Saturday",
                                "Sunday",
                            ]
                            .iter()
                            .zip(&a.weekdays)
                            {
                                bar(
                                    ui,
                                    d,
                                    &format!("{v} play{}", if *v == 1 { "" } else { "s" }),
                                    *v as f32 / max as f32,
                                );
                                ui.add_space(2.0);
                            }
                        }
                    });
                    ui.add_space(20.0);
                }
                if !a.genres.is_empty() {
                    panel(ui, |ui| {
                        caps(ui, "Favorite genres", TEAL());
                        ui.add_space(12.0);
                        let max = a.genres[0].hours.max(0.1);
                        for g in a.genres.iter().take(8) {
                            bar(
                                ui,
                                &g.name,
                                &fmt_runtime(g.hours * 3600.0),
                                (g.hours / max) as f32,
                            );
                            ui.add_space(6.0);
                        }
                    });
                    ui.add_space(20.0);
                }
                panel(ui, |ui| {
                    caps(ui, "Account", TEAL());
                    ui.add_space(6.0);
                    let mut rows = vec![
                        ("Username", a.name.clone(), TEXT(), 14.0),
                        (
                            "Role",
                            if a.admin { "Admin" } else { "User" }.to_owned(),
                            TEXT(),
                            14.0,
                        ),
                        ("Signed in on", a.sessions.len().to_string(), TEXT(), 14.0),
                    ];
                    if let Some(t) = a.last_active {
                        rows.push(("Last watched", ago(t), TEXT(), 14.0));
                    }
                    if a.plays > 0 {
                        rows.push((
                            "Average play",
                            fmt_runtime(secs / a.plays as f64),
                            TEXT(),
                            14.0,
                        ));
                    }
                    kv_rows(ui, &rows);
                    ui.add_space(8.0);
                    if small_link(ui, "Manage in Users") {
                        manage = true;
                    }
                });
                if !a.sessions.is_empty() {
                    ui.add_space(20.0);
                    panel(ui, |ui| {
                        caps(ui, "Devices", TEAL());
                        ui.add_space(10.0);
                        for s in &a.sessions {
                            ui.label(
                                RichText::new(s.device.as_deref().unwrap_or("Unknown device"))
                                    .color(TEXT()),
                            );
                            let mut line = Vec::new();
                            if let Some(ip) = &s.ip {
                                line.push(ip.clone());
                            }
                            if let Some(t) = s.last_used {
                                line.push(format!("used {}", ago(t)));
                            }
                            if let Some(t) = s.created {
                                line.push(format!("signed in {}", ago(t)));
                            }
                            ui.label(mono(line.join(" · "), 10.0).color(MUTED()));
                            ui.add_space(8.0);
                        }
                    });
                }
            },
        );
        if click.is_some() {
            cx.click = click;
        }
        if manage {
            cx.goto = Some((Section::Users, None));
        }
    }
}

/// The 7 days … Year picker, with a spinner while loading.
fn range(ui: &mut Ui, days: &mut u32, loading: bool) {
    ui.horizontal(|ui| {
        segmented(
            ui,
            days,
            &[
                (7, "7 days", None),
                (30, "30 days", None),
                (90, "90 days", None),
                (365, "Year", None),
            ],
        );
        if loading {
            ui.add_space(10.0);
            ui.add(egui::Spinner::new().size(16.0).color(MUTED()));
        }
    });
}

fn small_link(ui: &mut Ui, text: &str) -> bool {
    link(ui, text, 13.0, GOLD(), None).clicked()
}

/// The user's header, like their own profile page.
fn hero(ui: &mut Ui, name: &str, a: Option<&UserAnalytics>) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 200.0), Sense::hover());
    let p = ui.painter_at(rect);
    let col = color_for(name);
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
    let r = 50.0;
    let c = pos2(rect.left() + m + r, rect.center().y);
    p.circle_filled(c, r + 4.0, Color32::from_black_alpha(110));
    chrome::avatar(&p, c, r, name);
    let text_rect = Rect::from_min_max(
        pos2(c.x + r + 28.0, rect.top() + 24.0),
        pos2(rect.left() + m + inner, rect.bottom() - 16.0),
    );
    placed(ui, text_rect, "user_analytics_hero", 0.5, |ui| {
        caps(ui, "User analytics", TEAL());
        ui.add_space(4.0);
        let shown = a
            .and_then(|a| a.display_name.as_deref())
            .filter(|d| !d.trim().is_empty())
            .unwrap_or(name);
        ui.add(egui::Label::new(RichText::new(shown).font(black_font(40.0)).color(TEXT())).wrap());
        ui.add_space(6.0);
        let mut line = vec![name.to_owned()];
        if a.is_some_and(|a| a.admin) {
            line.push("Admin".into());
        }
        if let Some(t) = a.and_then(|a| a.last_active) {
            line.push(format!("last watched {}", ago(t)));
        }
        ui.label(mono(line.join("   ·   "), 11.5).color(MUTED()));
    });
}

/// One play in the history list; asks to open the title when clicked.
fn recent_row(ui: &mut Ui, cx: &mut Cx, e: &crate::api::RecentPlay) -> Option<Click> {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::click());
    let p = ui.painter_at(r);
    if resp.hovered() {
        p.rect_filled(r, 8.0, Color32::from_white_alpha(8));
    }
    let thumb = Rect::from_min_size(r.min + vec2(6.0, 6.0), vec2(93.0, 52.0));
    let owner = if e.series_id.is_empty() {
        &e.id
    } else {
        &e.series_id
    };
    let item = cx.find(owner);
    let still = cx
        .lib
        .find_episode(&e.id)
        .and_then(|(s, si, ei)| s.seasons[si].episodes[ei].still.as_ref());
    match still
        .or(item.and_then(|i| i.backdrop.as_ref()))
        .or(item.and_then(|i| i.poster.as_ref()))
        .and_then(|u| cx.images.get(u))
    {
        Some(t) => image_cover(&p, thumb, &t, 4, Color32::WHITE),
        None => placeholder(&p, thumb, &e.title, 4),
    }
    let f = if e.completed || e.duration <= 0.0 {
        1.0
    } else {
        (e.position / e.duration).clamp(0.0, 1.0) as f32
    };
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
        pos2(x, r.top() + 12.0),
        Align2::LEFT_TOP,
        &e.title,
        FontId::new(15.0, FontFamily::Name("bold".into())),
        TEXT(),
    );
    let sub = e.episode.clone().unwrap_or_else(|| "Movie".into());
    p.text(
        pos2(x, r.top() + 34.0),
        Align2::LEFT_TOP,
        sub,
        FontId::proportional(12.5),
        MUTED(),
    );
    p.text(
        pos2(r.right() - 10.0, r.top() + 14.0),
        Align2::RIGHT_TOP,
        ago(e.updated),
        FontId::monospace(11.0),
        MUTED(),
    );
    let (state, col) = if e.completed {
        ("Watched".to_owned(), TEAL())
    } else {
        (
            format!("{:.0}%  ·  {}", f * 100.0, fmt_runtime(e.position)),
            GOLD(),
        )
    };
    p.text(
        pos2(r.right() - 10.0, r.top() + 34.0),
        Align2::RIGHT_TOP,
        state,
        FontId::monospace(11.0),
        col,
    );
    if !resp.on_hover_cursor(CursorIcon::PointingHand).clicked() || item.is_none() {
        return None;
    }
    Some(if e.series_id.is_empty() {
        Click::Open(e.id.clone())
    } else {
        Click::Nav(View::Episode(e.series_id.clone(), e.id.clone()))
    })
}

/// Plays per hour of the day, 24 bars; hover for the numbers.
fn hour_chart(ui: &mut Ui, hours: &[u32]) {
    let w = ui.available_width();
    let h = 70.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, h + 18.0), Sense::hover());
    let p = ui.painter();
    let plot = Rect::from_min_max(r.min, pos2(r.right(), r.top() + h));
    let max = hours.iter().copied().max().unwrap_or(0).max(1);
    p.hline(plot.x_range(), plot.bottom(), Stroke::new(1.0f32, LINE()));
    let slot = plot.width() / 24.0;
    let bw = (slot * 0.7).clamp(1.0, 14.0);
    let hover = resp.hover_pos();
    let mut tip = None;
    for (i, v) in hours.iter().take(24).enumerate() {
        let cx = plot.left() + slot * (i as f32 + 0.5);
        let col_rect = Rect::from_min_max(
            pos2(cx - slot / 2.0, plot.top()),
            pos2(cx + slot / 2.0, plot.bottom()),
        );
        let hot = hover.is_some_and(|h| col_rect.contains(h));
        if hot {
            p.rect_filled(col_rect, 2.0, Color32::from_white_alpha(8));
            tip = Some((i, *v));
        }
        if *v > 0 {
            let bh = plot.height() * *v as f32 / max as f32;
            p.rect_filled(
                Rect::from_min_max(
                    pos2(cx - bw / 2.0, plot.bottom() - bh),
                    pos2(cx + bw / 2.0, plot.bottom()),
                ),
                2.0,
                if hot { TEXT() } else { GOLD() },
            );
        }
    }
    for i in [0, 6, 12, 18] {
        p.text(
            pos2(plot.left() + slot * (i as f32 + 0.5), plot.bottom() + 5.0),
            Align2::CENTER_TOP,
            format!("{i:02}"),
            FontId::monospace(9.5),
            MUTED(),
        );
    }
    if let Some((i, v)) = tip {
        resp.on_hover_text(format!(
            "{i:02}:00–{:02}:00\n{v} play{}",
            (i + 1) % 24,
            if v == 1 { "" } else { "s" }
        ));
    }
}

/// One bar per day; hover for the numbers.
fn day_chart(ui: &mut Ui, days: &[DayStat]) {
    let w = ui.available_width();
    let h = 160.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, h + 22.0), Sense::hover());
    let p = ui.painter();
    let plot = Rect::from_min_max(r.min, pos2(r.right(), r.top() + h));
    if days.is_empty() {
        p.text(
            plot.center(),
            Align2::CENTER_CENTER,
            "No plays yet",
            FontId::monospace(11.0),
            MUTED(),
        );
        return;
    }
    let max = days.iter().map(|d| d.plays).max().unwrap_or(0).max(1);
    for f in [0.5, 1.0] {
        let y = plot.bottom() - plot.height() * f;
        p.hline(plot.x_range(), y, Stroke::new(1.0f32, LINE()));
    }
    p.hline(plot.x_range(), plot.bottom(), Stroke::new(1.0f32, LINE()));
    p.text(
        pos2(plot.left(), plot.top()),
        Align2::LEFT_BOTTOM,
        max.to_string(),
        FontId::monospace(9.5),
        MUTED(),
    );
    let slot = plot.width() / days.len() as f32;
    let bw = (slot * 0.7).clamp(1.0, 24.0);
    let hover = resp.hover_pos();
    let mut tip = None;
    for (i, d) in days.iter().enumerate() {
        let cx = plot.left() + slot * (i as f32 + 0.5);
        let bh = plot.height() * d.plays as f32 / max as f32;
        let bar = Rect::from_min_max(
            pos2(cx - bw / 2.0, plot.bottom() - bh),
            pos2(cx + bw / 2.0, plot.bottom()),
        );
        let col_rect = Rect::from_min_max(
            pos2(cx - slot / 2.0, plot.top()),
            pos2(cx + slot / 2.0, plot.bottom()),
        );
        let hot = hover.is_some_and(|h| col_rect.contains(h));
        if hot {
            p.rect_filled(col_rect, 2.0, Color32::from_white_alpha(8));
            tip = Some(d);
        }
        if d.plays > 0 {
            p.rect_filled(bar, 2.0, if hot { TEXT() } else { GOLD() });
        }
    }
    // first and last day under the axis
    for (d, align, x) in [
        (days.first(), Align2::LEFT_TOP, plot.left()),
        (days.last(), Align2::RIGHT_TOP, plot.right()),
    ] {
        if let Some(d) = d {
            p.text(
                pos2(x, plot.bottom() + 6.0),
                align,
                &d.day,
                FontId::monospace(9.5),
                MUTED(),
            );
        }
    }
    if let Some(d) = tip {
        resp.on_hover_text(format!(
            "{}\n{} plays, {:.1} hours",
            d.day, d.plays, d.hours
        ));
    }
}
