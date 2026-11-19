//! Admin › Images: Upgrade images (replace every image a larger TMDB version
//! exists for), followed while it runs; what the last run changed or
//! couldn't; earlier runs; and how the library's images break down by
//! resolution.
use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, space, text, Column};
use iced::{Background, Color, Element, Length, Padding};

use super::super::details::gap;
use super::super::profile::Remote;
use super::super::{spin_secs, Msg as LMsg};
use super::ui::{self, caps, mono, panel, stats};
use super::{not_set_up, Cx, Done, JobFn, Msg as AMsg, Section};
use crate::api::{Client, ImageChange, ImagesReport, UpgradeRun};
use crate::iced_ui::style::{self, c, BOLD};
use crate::iced_ui::widgets::Kind as Btn;
use crate::ui::browse::details::TEAL;
use crate::ui::browse::people::ago;
use crate::ui::theme::{ERROR, LINE, OK, WARN};

/// While a run is going, look again this often.
const FOLLOW_SECS: u64 = 3;
/// Changes listed before "and N more".
const SHOW: usize = 200;

/// The artwork slots, as the server names them, with their labels.
const SLOTS: [(&str, &str); 4] = [
    ("poster", "Posters"),
    ("backdrop", "Backdrops"),
    ("thumb", "Thumbnails"),
    ("logo", "Logos"),
];
/// Resolution tiers, largest first, then images that aren't there.
const TIERS: [&str; 6] = ["4K", "1440p", "1080p", "720p", "SD", "missing"];

#[derive(Clone, Debug)]
pub enum Msg {
    /// Start Upgrade images.
    Run,
    /// Count the library's images again, not from the server's cache.
    CountAgain,
    /// Show only one slot's changes; `None`: all.
    Filter(Option<&'static str>),
    /// Open a title in Library (our id); handled by the admin page.
    Open(String),
}

#[derive(Default)]
pub struct State {
    report: Remote<ImagesReport>,
    /// The next load counts the images again.
    fresh: bool,
    filter: Option<&'static str>,
}

impl State {
    pub fn refresh(&mut self) {
        self.report.retry();
    }

    pub fn missing(&self) -> bool {
        self.report.missing
    }

    pub fn work(&mut self, client: &Client, login: &mut bool) {
        self.report.poll(login);
        let running = self.report.data.as_ref().is_some_and(|r| r.running);
        if self.report.due() || (running && self.report.stale(FOLLOW_SECS)) {
            let (c, fresh) = (client.clone(), std::mem::take(&mut self.fresh));
            self.report.load(move || c.admin_images_report(fresh));
        }
    }

    pub fn update(&mut self, m: Msg) -> Option<JobFn> {
        match m {
            Msg::Run => {
                return Some(Box::new(|c: &Client| {
                    c.admin_sync("images")?;
                    Ok(Done::msg(
                        "Upgrade images started. This page follows it; it can take a while.",
                    ))
                }));
            }
            Msg::CountAgain => {
                self.fresh = true;
                self.report.retry();
            }
            Msg::Filter(f) => self.filter = f,
            Msg::Open(_) => {} // the admin page opens Library
        }
        None
    }

    pub fn view<'a>(&'a self, cx: &Cx<'a>, w: f32) -> Element<'a, LMsg> {
        if self.report.missing {
            return not_set_up(Section::Images, &cx.client.info.name);
        }
        let mut col: Column<'a, LMsg> = column![];
        if let Some(e) = &self.report.error {
            col = col
                .push(
                    text(format!("Couldn't load the report: {e}"))
                        .size(14.5)
                        .color(style::ERROR()),
                )
                .push(gap(8.0));
        }
        let Some(r) = self.report.data.as_ref() else {
            return col
                .push(crate::iced_ui::icon::spinner(
                    spin_secs(),
                    style::MUTED(),
                    18.0,
                ))
                .into();
        };
        let pw = w - 36.0; // inside a panel
        col = col.push(run_panel(r, cx.busy, pw)).push(gap(16.0));
        if let Some(last) = r.last.as_ref() {
            col = col
                .push(caps("Last run", c(TEAL())))
                .push(gap(8.0))
                .push(stats(
                    vec![
                        (last.upgraded.to_string(), "Upgraded"),
                        (last.added.to_string(), "Added (were missing)"),
                        (last.kept.to_string(), "Already the largest"),
                        (last.failed.to_string(), "Failed"),
                    ],
                    w,
                ))
                .push(gap(16.0));
        }
        col = col
            .push(library_panel(r, self.report.loading(), pw))
            .push(gap(16.0));
        if let Some(last) = r.last.as_ref() {
            col = col.push(self.changes_panel(last, pw)).push(gap(16.0));
            if !last.failures.is_empty() {
                col = col.push(failures_panel(last)).push(gap(16.0));
            }
        }
        if !r.runs.is_empty() {
            col = col.push(history_panel(&r.runs));
        }
        col.into()
    }

    fn changes_panel<'a>(&self, last: &'a UpgradeRun, w: f32) -> Element<'a, LMsg> {
        let mut col: Column<'a, LMsg> =
            column![caps("Changed in the last run", c(TEAL())), gap(8.0)];
        if last.changes.is_empty() {
            return panel(
                col.push(
                    text("Nothing: every image was already the largest TMDB has.")
                        .size(14.5)
                        .color(style::MUTED()),
                ),
            );
        }
        let count = |s: &str| last.changes.iter().filter(|ch| ch.slot == s).count();
        let mut tabs = vec![(
            "All".to_owned(),
            Some(last.changes.len()),
            self.filter.is_none(),
            send(Msg::Filter(None)),
        )];
        for (slot, label) in SLOTS {
            let n = count(slot);
            if n > 0 {
                tabs.push((
                    label.to_owned(),
                    Some(n),
                    self.filter == Some(slot),
                    send(Msg::Filter(Some(slot))),
                ));
            }
        }
        col = col.push(ui::segmented(tabs)).push(gap(10.0));
        let shown: Vec<&ImageChange> = last
            .changes
            .iter()
            .filter(|ch| self.filter.is_none_or(|f| ch.slot == f))
            .collect();
        for ch in shown.iter().take(SHOW) {
            col = col.push(change_row(ch, w));
        }
        if shown.len() > SHOW {
            col = col.push(
                text(format!("and {} more", shown.len() - SHOW))
                    .size(13)
                    .color(style::MUTED()),
            );
        }
        if last.changes.len() as u64 > last.upgraded + last.added {
            // the server keeps a few thousand
            col = col.push(
                text("The server only kept the first changes of a big run.")
                    .size(12)
                    .color(style::MUTED()),
            );
        }
        panel(col)
    }
}

fn send(m: Msg) -> LMsg {
    LMsg::Admin(AMsg::Images(m))
}

/// "Posters", … for a slot the server names.
fn slot_label(slot: &str) -> &'static str {
    match slot {
        "poster" => "Poster",
        "backdrop" => "Backdrop",
        "thumb" => "Thumbnail",
        "logo" => "Logo",
        _ => "All images",
    }
}

/// Our id for a title the report names ("movie" + 12 → "movie:12").
fn our_id(kind: &str, id: &str) -> String {
    format!("{}:{id}", if kind == "series" { "series" } else { "movie" })
}

/// "4 min", "1 h 12 min".
fn took(s: i64) -> String {
    match s {
        s if s < 60 => format!("{s} s"),
        s if s < 3600 => format!("{} min", s / 60),
        s => format!("{} h {} min", s / 3600, s % 3600 / 60),
    }
}

/// The run going on now, or the button to start one.
fn run_panel<'a>(r: &'a ImagesReport, busy: bool, w: f32) -> Element<'a, LMsg> {
    let mut col: Column<'a, LMsg> = column![caps("Upgrade images", c(TEAL())), gap(6.0)];
    col = col.push(
        text(
            "Checks every movie's and series' poster, backdrop, thumbnail and logo against \
             TMDB and downloads the largest version (4K when there is one) where it's bigger \
             than the image there, or the image is missing. Images set by hand are kept. \
             Syncs only fetch images that are missing.",
        )
        .size(13)
        .color(style::MUTED()),
    );
    col = col.push(gap(12.0));
    match r.current.as_ref().filter(|_| r.running) {
        Some(now) => {
            let f = if now.total > 0 {
                now.checked as f32 / now.total as f32
            } else {
                0.0
            };
            col = col
                .push(ui::bar(
                    "Checking titles",
                    format!("{} of {}", now.checked, now.total),
                    f,
                    w,
                ))
                .push(gap(8.0))
                .push(
                    row![
                        crate::iced_ui::icon::spinner(spin_secs(), style::MUTED(), 14.0),
                        text(format!(
                            "{} upgraded · {} added · {} already the largest · {} failed so far",
                            now.upgraded, now.added, now.kept, now.failed
                        ))
                        .size(13),
                    ]
                    .spacing(8)
                    .align_y(Vertical::Center),
                );
        }
        None => {
            let can = r.tmdb && !busy;
            let mut line = row![ui::button_with(
                Btn::Small,
                "Run Upgrade images",
                None,
                can.then(|| send(Msg::Run))
            )]
            .spacing(12)
            .align_y(Vertical::Center);
            if !r.tmdb {
                line = line.push(
                    text("This server has no TMDB key.")
                        .size(13)
                        .color(c(WARN())),
                );
            } else if let Some(last) = r.last.as_ref() {
                let mut s = String::from("Last run");
                if let Some(t) = last.when() {
                    s = format!("{s} {}", ago(t));
                }
                if let Some(d) = last.took() {
                    s = format!("{s}, took {}", took(d));
                }
                line = line.push(text(s).size(13).color(style::MUTED()));
            } else {
                line = line.push(text("It hasn't run yet.").size(13).color(style::MUTED()));
            }
            col = col.push(line);
        }
    }
    panel(col)
}

/// Each slot's images by resolution: a bar in parts and the numbers.
fn library_panel<'a>(r: &'a ImagesReport, loading: bool, w: f32) -> Element<'a, LMsg> {
    let mut head = row![
        caps("Your library's images", c(TEAL())),
        space::horizontal()
    ]
    .align_y(Vertical::Center);
    head = head.push(if loading {
        Element::from(crate::iced_ui::icon::spinner(
            spin_secs(),
            style::MUTED(),
            14.0,
        ))
    } else {
        ui::button_with(Btn::Small, "Count again", None, Some(send(Msg::CountAgain)))
    });
    let mut col: Column<'a, LMsg> = column![head, gap(10.0)];
    if r.library.is_empty() {
        return panel(
            col.push(
                text("The server didn't count them.")
                    .size(14.5)
                    .color(style::MUTED()),
            ),
        );
    }
    // the legend once, on top
    let mut legend = row![].spacing(14);
    for t in TIERS {
        legend = legend.push(
            row![
                swatch(tier_color(t)),
                text(t).size(12).color(style::MUTED())
            ]
            .spacing(5)
            .align_y(Vertical::Center),
        );
    }
    col = col.push(legend).push(gap(12.0));
    for (slot, label) in SLOTS {
        let Some(counts) = r.library.get(slot) else {
            continue;
        };
        let n = |t: &str| counts.get(t).copied().unwrap_or(0);
        let total: u64 = TIERS.iter().map(|t| n(t)).sum();
        let top = if total > 0 {
            format!("{} of {total} in 4K", n("4K"))
        } else {
            "none".into()
        };
        col = col
            .push(
                row![
                    text(label).font(BOLD).size(14),
                    space::horizontal(),
                    text(top).size(12.5).color(style::MUTED()),
                ]
                .align_y(Vertical::Center),
            )
            .push(gap(6.0))
            .push(parts_bar(TIERS.map(|t| (n(t), tier_color(t))), total, w))
            .push(gap(4.0));
        let mut nums: Vec<String> = TIERS
            .iter()
            .filter(|t| n(t) > 0)
            .map(|t| format!("{t} {}", n(t)))
            .collect();
        if n("locked") > 0 {
            nums.push(format!("{} set by hand", n("locked")));
        }
        col = col
            .push(text(nums.join("  ·  ")).size(12).color(style::MUTED()))
            .push(gap(14.0));
    }
    panel(col)
}

/// The color a tier is shown in.
fn tier_color(t: &str) -> Color {
    match t {
        "4K" => c(OK()),
        "1440p" => c(TEAL()),
        "1080p" => ui::accent(),
        "720p" => c(WARN()),
        "SD" => c(ERROR()),
        _ => c(LINE()),
    }
}

fn swatch<'a>(col: Color) -> Element<'a, LMsg> {
    container(space())
        .width(10)
        .height(10)
        .style(move |_| container::Style {
            background: Some(Background::Color(col)),
            border: iced::border::rounded(2),
            ..container::Style::default()
        })
        .into()
}

/// A bar `w` wide in parts, each as long as its share of `total`.
fn parts_bar<'a>(parts: [(u64, Color); 6], total: u64, w: f32) -> Element<'a, LMsg> {
    let mut r = row![].spacing(0);
    if total > 0 {
        for (n, col) in parts {
            if n == 0 {
                continue;
            }
            let pw = (w * n as f32 / total as f32).max(2.0);
            r = r.push(
                container(space())
                    .width(pw)
                    .height(10)
                    .style(move |_| container::Style {
                        background: Some(Background::Color(col)),
                        ..container::Style::default()
                    }),
            );
        }
    }
    container(r)
        .width(w)
        .height(10)
        .clip(true)
        .style(|_| container::Style {
            background: Some(Background::Color(c(LINE()))),
            border: iced::border::rounded(3),
            ..container::Style::default()
        })
        .into()
}

/// One change: the title (opens it in Library), which image, from → to.
fn change_row<'a>(ch: &'a ImageChange, w: f32) -> Element<'a, LMsg> {
    let size = match ch.from {
        Some([fw, fh]) => format!("{fw} × {fh}  →  {} × {}", ch.to[0], ch.to[1]),
        None => format!("added  ·  {} × {}", ch.to[0], ch.to[1]),
    };
    let title = button(text(ch.title.clone()).size(14))
        .padding(0)
        .style(|_, s| button::Style {
            text_color: if matches!(s, button::Status::Hovered) {
                ui::accent()
            } else {
                style::TEXT()
            },
            ..button::Style::default()
        })
        .on_press(send(Msg::Open(our_id(&ch.kind, &ch.id))));
    container(
        row![
            container(title).width(Length::FillPortion(5)),
            mono(slot_label(&ch.slot), 11.5)
                .color(style::MUTED())
                .width(Length::FillPortion(2)),
            text(size).size(12.5).width(Length::FillPortion(4)),
            // a fixed cell: "4K" and "1440p" badges differ in width
            container(ui::badge(&ch.tier, tier_color(&ch.tier)))
                .width(64)
                .align_x(iced::alignment::Horizontal::Right),
        ]
        .spacing(12)
        .align_y(Vertical::Center),
    )
    .width(w)
    .padding(Padding::default().top(5.0).bottom(5.0))
    .into()
}

fn failures_panel<'a>(last: &'a UpgradeRun) -> Element<'a, LMsg> {
    let mut col: Column<'a, LMsg> = column![
        caps(&format!("Failed · {}", last.failures.len()), c(ERROR())),
        gap(8.0),
    ];
    for f in last.failures.iter().take(SHOW) {
        col = col.push(
            row![
                button(text(f.title.clone()).size(14))
                    .padding(0)
                    .style(|_, _| button::Style {
                        text_color: style::TEXT(),
                        ..button::Style::default()
                    })
                    .on_press(send(Msg::Open(our_id(&f.kind, &f.id)))),
                mono(slot_label(&f.slot), 11.5).color(style::MUTED()),
                text(f.reason.clone()).size(12.5).color(style::MUTED()),
            ]
            .spacing(12)
            .align_y(Vertical::Center),
        );
    }
    col = col.push(gap(4.0)).push(
        text("Run it again to retry these; the rest are kept.")
            .size(12)
            .color(style::MUTED()),
    );
    panel(col)
}

fn history_panel<'a>(runs: &'a [UpgradeRun]) -> Element<'a, LMsg> {
    let mut col: Column<'a, LMsg> = column![caps("Earlier runs", c(TEAL())), gap(8.0)];
    for run in runs {
        let when = run.when().map_or("—".to_owned(), ago);
        let mut what = format!(
            "{} upgraded · {} added · {} already the largest",
            run.upgraded, run.added, run.kept
        );
        if run.failed > 0 {
            what = format!("{what} · {} failed", run.failed);
        }
        col = col.push(
            row![
                text(when).size(13.5).width(140),
                text(what).size(13.5).width(Length::Fill),
                text(run.took().map(took).unwrap_or_default())
                    .size(12.5)
                    .color(style::MUTED()),
            ]
            .spacing(12)
            .align_y(Vertical::Center),
        );
    }
    panel(col)
}
