//! The dialogs over the window, like egui's (`ui::settings`, `ui::whats_new`,
//! the update popup and notice in `update`): Settings, "Update available",
//! "What's new", the update notice in the corner, and the wordmark shown
//! while the library loads after signing in.
use std::time::{Duration, Instant};

use iced::alignment::{Horizontal, Vertical};
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::widget::{
    button, checkbox, column, container, image, mouse_area, pick_list, row, scrollable, space,
    stack, text, Column,
};
use iced::{
    mouse, Background, Border, Color, Element, Length, Padding, Point, Rectangle, Renderer, Size,
    Theme,
};

use super::icon::{self, Icon};
use super::spaced::mono;
use super::style::{self, c, BOLD};
use super::widgets::{self, Kind};
use crate::config::Config;
use crate::ui::theme::{
    self, Hero, Mode, Nav, Palette, Skin, Tiles, ACCENT, LINE, LINE_HI, SURFACE, SURFACE_HI,
};
use crate::update::{Release, ToastView, VERSION};

/// The connection Settings describes.
pub struct Conn {
    pub name: String,
    pub base: String,
    pub user: Option<String>,
    pub signed_in: bool,
}

#[derive(Clone, Debug)]
pub enum Msg {
    /// Done, a click outside, or Escape.
    Close,
    ChangeServer,
    SignOut,
    OpenFiles,
    RefreshNow,
    Quit,
    Resume(bool),
    Autoplay(bool),
    Fullscreen(bool),
    Discord(bool),
    Skip(Choice<u32>),
    RefreshEvery(Choice<u32>),
    GenreRows(bool),
    Style(Skin),
    Theme(Mode),
    Scale(f32),
    AppMenu(bool),
    /// The update popup: install, download, or later.
    Update(UpdatePick),
    /// The corner notice: close it (`true`: download first).
    Dismiss(bool),
    /// A link to open in the browser.
    Open(String),
}

#[derive(Clone, Copy, Debug)]
pub enum UpdatePick {
    Update,
    Download,
    Later,
}

/// A drop-down option: a value and its label.
#[derive(Clone, Copy, Debug)]
pub struct Choice<T>(pub T, pub &'static str);

impl<T: PartialEq> PartialEq for Choice<T> {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}

impl<T> std::fmt::Display for Choice<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.1)
    }
}

const SKIPS: [Choice<u32>; 4] = [
    Choice(5, "5 seconds"),
    Choice(10, "10 seconds"),
    Choice(15, "15 seconds"),
    Choice(30, "30 seconds"),
];
const REFRESHES: [Choice<u32>; 4] = [
    Choice(0, "Only on start"),
    Choice(5, "Every 5 minutes"),
    Choice(15, "Every 15 minutes"),
    Choice(60, "Every hour"),
];

/// What a settings message changes; false when it isn't a setting.
pub fn apply(cfg: &mut Config, msg: &Msg) -> bool {
    match *msg {
        Msg::Resume(v) => cfg.resume = v,
        Msg::Autoplay(v) => cfg.autoplay_next = v,
        Msg::Fullscreen(v) => cfg.fullscreen_on_play = v,
        Msg::Discord(v) => cfg.discord_presence = v,
        Msg::Skip(Choice(v, _)) => cfg.skip_seconds = v,
        Msg::RefreshEvery(Choice(v, _)) => cfg.refresh_minutes = v,
        Msg::GenreRows(v) => cfg.genre_rows = v,
        Msg::Style(s) => cfg.style = s,
        Msg::Theme(m) => cfg.theme = m,
        Msg::Scale(s) => cfg.ui_scale = s,
        Msg::AppMenu(v) => {
            cfg.app_menu = v;
            crate::integrate::sync(v);
        }
        _ => return false,
    }
    true
}

/// `System` as dark or light, the way the desktop is set now.
pub fn resolved(m: Mode, system_light: bool) -> Mode {
    match m {
        Mode::System if system_light => Mode::Light,
        Mode::System => Mode::Dark,
        m => m,
    }
}

// ------------------------------------------------------------- the frame

/// A dialog over `page`: the page darkened, the dialog in the middle,
/// `width` wide at most, scrolling when it's taller than the window
/// (egui's `settings::dialog`). A click outside sends `close`.
pub fn modal<'a, M: Clone + 'a>(
    page: Element<'a, M>,
    content: Element<'a, M>,
    width: f32,
    window: Size,
    close: M,
) -> Element<'a, M> {
    let w = width.min(window.width - 80.0).max(200.0) + 22.0;
    let body = scrollable(
        container(content)
            .padding(Padding::default().right(22.0))
            .width(w),
    )
    .height(Length::Shrink)
    .style(scroll_style);
    let panel = container(body)
        .padding(Padding {
            top: 22.0,
            right: 4.0,
            bottom: 22.0,
            left: 26.0,
        })
        .max_height(window.height - 120.0 + 44.0)
        .style(|_| container::Style {
            background: Some(Background::Color(style::BG())),
            border: Border {
                color: c(LINE()),
                width: 1.0,
                radius: 4.0.into(),
            },
            text_color: Some(style::TEXT()),
            ..container::Style::default()
        });
    stack![
        page,
        mouse_area(
            container(space())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(style::black(160))),
                    ..container::Style::default()
                })
        )
        .on_press(close),
        container(mouse_area(panel)).center(Length::Fill),
    ]
    .into()
}

fn scroll_style(_: &Theme, s: scrollable::Status) -> scrollable::Style {
    let hot = matches!(
        s,
        scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. }
    );
    let rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: Background::Color(if hot { style::MUTED() } else { c(LINE_HI()) }),
            border: iced::border::rounded(4),
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(style::black(200)),
            border: iced::border::rounded(20),
            shadow: iced::Shadow::default(),
            icon: Color::WHITE,
        },
    }
}

/// The dialog's title: the accent badge with a play mark, the title and a
/// line under it (egui's `settings::header`).
pub fn header<'a, M: 'a>(title: &str, sub: &str) -> Element<'a, M> {
    let badge = container(icon::icon(Icon::Play, Color::WHITE, 24.0))
        .center(36)
        .style(|_| container::Style {
            background: Some(Background::Color(c(ACCENT()))),
            border: iced::border::rounded(7),
            ..container::Style::default()
        });
    column![
        row![
            badge,
            column![
                text(title.to_owned()).font(BOLD).size(24),
                mono(sub, 11.0).color(style::MUTED()),
            ]
            .spacing(2),
        ]
        .spacing(14)
        .align_y(Vertical::Center),
        space().height(16),
    ]
    .into()
}

/// A muted section heading ("PLAYBACK"), egui's `ui::section`.
fn section<'a>(t: &str) -> Element<'a, Msg> {
    column![
        space().height(18),
        mono(t, 11.5).color(style::MUTED()),
        space().height(10)
    ]
    .into()
}

/// A label on the left, a control on the right, 38 high at least.
fn row_of<'a>(label: &str, control: Element<'a, Msg>) -> Element<'a, Msg> {
    container(
        row![
            text(label.to_owned()).size(15),
            space::horizontal(),
            control
        ]
        .spacing(10)
        .align_y(Vertical::Center),
    )
    .center_y(38)
    .into()
}

fn check<'a>(on: bool, label: &str, msg: fn(bool) -> Msg) -> Element<'a, Msg> {
    container(
        checkbox(on)
            .label(label.to_owned())
            .on_toggle(msg)
            .size(16)
            .spacing(9)
            .text_size(15)
            .style(|_, s| {
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
            }),
    )
    .padding(Padding::default().top(2.0).bottom(12.0))
    .into()
}

/// A drop-down 200 wide (egui's `combo`).
fn combo<'a, T: PartialEq + Copy + std::fmt::Debug + 'static>(
    options: &'static [Choice<T>],
    value: T,
    msg: fn(Choice<T>) -> Msg,
) -> Element<'a, Msg> {
    let current = options.iter().find(|o| o.0 == value).copied();
    pick_list(options, current, msg)
        .width(200)
        .text_size(14.5)
        .padding([6, 10])
        .style(|_, s| {
            let p = theme::pal();
            let hot = matches!(
                s,
                pick_list::Status::Hovered | pick_list::Status::Opened { .. }
            );
            pick_list::Style {
                text_color: c(p.text),
                placeholder_color: c(p.muted),
                handle_color: c(p.text),
                background: Background::Color(c(if hot { p.surface_hi } else { p.surface })),
                border: Border {
                    color: c(if hot { p.line_hi } else { p.line }),
                    width: 1.0,
                    radius: f32::from(theme::spec().button_radius).min(4.0).into(),
                },
            }
        })
        .menu_style(|_| {
            let p = theme::pal();
            iced::overlay::menu::Style {
                background: Background::Color(c(p.surface)),
                border: Border {
                    color: c(p.line_hi),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                text_color: c(p.text),
                selected_text_color: c(p.text),
                selected_background: Background::Color(c(p.surface_hi)),
                shadow: iced::Shadow::default(),
            }
        })
        .into()
}

fn small<'a>(t: &str, msg: Msg) -> Element<'a, Msg> {
    widgets::button_with(Kind::Small, t, None, Some(msg))
}

fn note<'a>(t: &str) -> Element<'a, Msg> {
    text(t.to_owned()).size(12).color(style::MUTED()).into()
}

/// A link that opens in the browser, with the external-link icon.
fn ext_link<'a>(t: &str, url: String) -> Element<'a, Msg> {
    button(
        row![
            text(t.to_owned()).size(14.5),
            icon::icon_hover(Icon::External, style::MUTED(), style::TEXT(), 11.0),
        ]
        .spacing(4)
        .align_y(Vertical::Center),
    )
    .padding(0)
    .style(|_, _| button::Style {
        background: None,
        text_color: style::TEXT(),
        ..button::Style::default()
    })
    .on_press(Msg::Open(url))
    .into()
}

// ------------------------------------------------------------- Settings

pub fn settings<'a>(cfg: &Config, conn: Option<&Conn>, system_light: bool) -> Element<'a, Msg> {
    let mut col: Column<'a, Msg> = column![header(
        "Settings",
        concat!("Valo ", env!("CARGO_PKG_VERSION"))
    )];

    col = col.push(section("Server"));
    match conn {
        Some(cn) => {
            col = col.push(row_of(
                "Library",
                row![
                    text(cn.name.clone()).size(14.5).color(style::MUTED()),
                    small("Switch server", Msg::ChangeServer),
                ]
                .spacing(16)
                .align_y(Vertical::Center)
                .into(),
            ));
            col = col
                .push(mono(&cn.base, 11.0).color(style::MUTED()))
                .push(space().height(12));
            if cn.signed_in {
                col = col.push(row_of(
                    "Signed in",
                    row![
                        text(cn.user.clone().unwrap_or_default())
                            .size(14.5)
                            .color(style::MUTED()),
                        small("Sign out", Msg::SignOut),
                    ]
                    .spacing(16)
                    .align_y(Vertical::Center)
                    .into(),
                ));
            }
        }
        None => col = col.push(row_of("Not connected", small("Connect", Msg::ChangeServer))),
    }

    col = col
        .push(section("Playback"))
        .push(check(cfg.resume, "Continue where I left off", Msg::Resume))
        .push(check(
            cfg.autoplay_next,
            "Play the next episode automatically",
            Msg::Autoplay,
        ))
        .push(check(
            cfg.fullscreen_on_play,
            "Start videos in fullscreen",
            Msg::Fullscreen,
        ))
        .push(check(
            cfg.discord_presence,
            "Show what I'm watching on Discord",
            Msg::Discord,
        ))
        .push(row_of(
            "Arrow keys skip",
            combo(&SKIPS, cfg.skip_seconds, Msg::Skip),
        ))
        .push(note("Hold Shift to skip six times as far."))
        .push(space().height(8))
        .push(row_of(
            "Play a file from this computer",
            small("Open file…", Msg::OpenFiles),
        ));

    col = col
        .push(section("Library"))
        .push(row_of(
            "Check for new movies and episodes",
            combo(&REFRESHES, cfg.refresh_minutes, Msg::RefreshEvery),
        ))
        .push(space().height(8))
        .push(check(
            cfg.genre_rows,
            "Show a row for each genre on Home",
            Msg::GenreRows,
        ));
    if conn.is_some() {
        col = col.push(row_of(
            "Reload the library now",
            small("Refresh", Msg::RefreshNow),
        ));
    }

    col = col
        .push(section("Appearance"))
        .push(text("App style").size(15))
        .push(note(
            "How the pages and the player are laid out, like the service it's named after.",
        ))
        .push(space().height(14));
    let mut styles = row![].spacing(10);
    for s in Skin::ALL {
        let pal = theme::palette(s, resolved(cfg.theme, system_light));
        styles = styles.push(swatch(
            Pic::Layout(s),
            pal,
            s.spec().name,
            cfg.style == s,
            Msg::Style(s),
        ));
    }
    col = col
        .push(styles.wrap().vertical_spacing(10))
        .push(space().height(20))
        .push(text("Color theme").size(15))
        .push(space().height(14));
    let mut themes = row![].spacing(10);
    for (m, name) in Mode::ALL {
        let pal = theme::palette(cfg.style, resolved(m, system_light));
        themes = themes.push(swatch(
            Pic::Colors,
            pal,
            name,
            cfg.theme == m,
            Msg::Theme(m),
        ));
    }
    col = col
        .push(themes.wrap().vertical_spacing(10))
        .push(space().height(20))
        .push(text("Interface size").size(15))
        .push(space().height(12))
        .push(widgets::segmented(
            [
                (0.9, "Small"),
                (1.0, "Normal"),
                (1.15, "Large"),
                (1.3, "Huge"),
            ]
            .into_iter()
            .map(|(v, l)| {
                (
                    l.to_owned(),
                    None,
                    (cfg.ui_scale - v).abs() < 0.001,
                    Msg::Scale(v),
                )
            })
            .collect(),
        ));

    if crate::integrate::appimage().is_some() {
        col = col.push(section("Desktop")).push(check(
            cfg.app_menu,
            "Show Valo in the app menu",
            Msg::AppMenu,
        ));
    }

    col.push(space().height(30))
        .push(
            row![
                widgets::button_with(Kind::Secondary, "Quit Valo", None, Some(Msg::Quit)),
                space::horizontal(),
                widgets::button_with(Kind::Primary, "   Done   ", None, Some(Msg::Close)),
            ]
            .align_y(Vertical::Center),
        )
        .into()
}

/// What a swatch pictures.
#[derive(Clone, Copy)]
enum Pic {
    /// A style's layout: sidebar or tabs, its banner, its cards.
    Layout(Skin),
    /// A color theme: background, posters, a button, the accent.
    Colors,
}

/// A small picture with the name under it; ringed in the accent when chosen.
fn swatch<'a>(pic: Pic, pal: Palette, name: &str, selected: bool, msg: Msg) -> Element<'a, Msg> {
    let drawing = canvas::Canvas::new(Swatch { pic, pal, selected })
        .width(108)
        .height(66);
    button(
        column![
            drawing,
            text(name.to_owned()).size(13).color(if selected {
                style::TEXT()
            } else {
                style::MUTED()
            }),
        ]
        .spacing(8)
        .width(108),
    )
    .padding(0)
    .height(94)
    .style(|_, _| button::Style::default())
    .on_press(msg)
    .into()
}

struct Swatch {
    pic: Pic,
    pal: Palette,
    selected: bool,
}

impl<M> canvas::Program<M> for Swatch {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        let pal = &self.pal;
        let size = bounds.size();
        let rect =
            |f: &mut Frame, x: f32, y: f32, w: f32, h: f32, r: f32, col: eframe::egui::Color32| {
                f.fill(
                    &Path::rounded_rectangle(Point::new(x, y), Size::new(w, h), r.into()),
                    c(col),
                );
            };
        rect(&mut f, 0.0, 0.0, size.width, size.height, 6.0, pal.bg);
        match self.pic {
            Pic::Colors => {
                for i in 0..3 {
                    let col = if i == 0 { pal.accent } else { pal.surface_hi };
                    rect(&mut f, 10.0 + i as f32 * 22.0, 26.0, 18.0, 28.0, 2.0, col);
                }
                rect(&mut f, 10.0, 10.0, 40.0, 5.0, 2.0, pal.text);
                rect(&mut f, 78.0, 8.0, 20.0, 9.0, 3.0, pal.primary);
            }
            Pic::Layout(skin) => draw_layout(&mut f, skin, pal, size),
        }
        let hot = cursor.is_over(bounds);
        let (w, col) = if self.selected {
            (2.0, ACCENT())
        } else if hot {
            (1.0, LINE_HI())
        } else {
            (1.0, LINE())
        };
        f.stroke(
            &Path::rounded_rectangle(
                Point::new(w / 2.0, w / 2.0),
                Size::new(size.width - w, size.height - w),
                6.0.into(),
            ),
            Stroke::default().with_width(w).with_color(c(col)),
        );
        vec![f.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _: &(),
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

/// A style's layout in miniature (egui's `layout_swatch`).
fn draw_layout(f: &mut Frame, skin: Skin, pal: &Palette, size: Size) {
    let lay = &skin.spec().layout;
    let rect =
        |f: &mut Frame, x: f32, y: f32, w: f32, h: f32, r: f32, col: eframe::egui::Color32| {
            f.fill(
                &Path::rounded_rectangle(
                    Point::new(x, y),
                    Size::new(w.max(0.0), h.max(0.0)),
                    r.into(),
                ),
                c(col),
            );
        };
    // the body, inside a 6px margin
    let (mut left, mut top) = (6.0, 6.0);
    let (right, bottom) = (size.width - 6.0, size.height - 6.0);
    if lay.nav == Nav::Side {
        for i in 0..3 {
            let col = if i == 0 { pal.text } else { pal.muted };
            rect(
                f,
                left + 2.0,
                top + 2.0 + i as f32 * 6.0,
                11.0,
                3.0,
                1.0,
                col,
            );
        }
        left += 20.0;
    } else {
        for i in 0..3 {
            let col = if i == 0 { pal.text } else { pal.muted };
            rect(f, left + i as f32 * 12.0, top, 9.0, 3.0, 1.0, col);
        }
        top += 7.0;
    }
    let art = pal.surface_hi.gamma_multiply(1.6);
    let banner_h = 22.0;
    let width = right - left;
    match lay.hero {
        Hero::Banner => {
            rect(f, left + width / 2.0, top, width / 2.0, banner_h, 2.0, art);
            rect(f, left + 2.0, top + 12.0, 26.0, 4.0, 1.0, pal.text);
        }
        Hero::FullBleed => {
            rect(f, left, top, width, banner_h, 1.0, art);
            rect(f, left + 3.0, top + 12.0, 26.0, 4.0, 1.0, pal.text);
        }
        Hero::Carousel => {
            let (bl, br) = (left + 8.0, right - 8.0);
            let side = art.gamma_multiply(0.6);
            rect(
                f,
                left,
                top + 3.0,
                bl - 2.0 - left,
                banner_h - 6.0,
                2.0,
                side,
            );
            rect(
                f,
                br + 2.0,
                top + 3.0,
                right - br - 2.0,
                banner_h - 6.0,
                2.0,
                side,
            );
            rect(f, bl, top, br - bl, banner_h, 3.0, art);
            rect(f, bl + 3.0, top + 12.0, 22.0, 4.0, 1.0, pal.text);
        }
        Hero::None => {
            for i in 0..4 {
                let col = if i == 0 { pal.text } else { pal.surface_hi };
                rect(f, left + i as f32 * 13.0, top, 11.0, 4.0, 2.0, col);
            }
        }
    }
    let y0 = if lay.hero == Hero::None {
        top + 8.0
    } else {
        top + banner_h + 4.0
    };
    let (tw, th) = if lay.tiles == Tiles::Landscape {
        (20.0, 11.0)
    } else {
        (12.0, 17.0)
    };
    let rows = if lay.hero == Hero::None { 3 } else { 1 };
    for r in 0..rows {
        let y = y0 + r as f32 * (th + 3.0);
        if y + th > bottom + 6.0 {
            break;
        }
        let mut x = left;
        let mut i = 0;
        while x + tw <= right + 0.5 {
            let col = if i == 0 && r == 0 { pal.accent } else { art };
            rect(f, x, y, tw, th, 1.5, col);
            x += tw + 3.0;
            i += 1;
        }
    }
}

// ------------------------------------------------------------- updates

/// "Update available": what the release brings and a way to get it.
pub fn update_available<'a>(rel: &Release, in_place: bool, flatpak: bool) -> Element<'a, Msg> {
    let mut col: Column<'a, Msg> = column![header(
        "Update available",
        &format!("Valo {}  ·  you have {VERSION}", rel.version),
    )];
    let many = rel.changes.len() > 1;
    if many {
        col = col.push(para(&format!(
            "{} updates since your version, newest first.",
            rel.changes.len()
        )));
    }
    for ch in &rel.changes {
        if many {
            col = col.push(version_heading(&ch.version));
        }
        if !ch.summary.is_empty() {
            col = col
                .push(text(ch.summary.clone()).size(15).color(style::TEXT()))
                .push(space().height(22));
        }
        for (title, body) in &ch.notes {
            if !title.is_empty() {
                col = col
                    .push(text(title.clone()).font(BOLD).size(16))
                    .push(space().height(10));
            }
            col = col.push(para(body));
        }
    }
    if rel
        .changes
        .iter()
        .all(|ch| ch.summary.is_empty() && ch.notes.is_empty())
    {
        col = col.push(para("This update brings fixes and small improvements."));
    }
    if flatpak {
        col = col.push(para("Download the new .flatpak and open it to update."));
    }
    let (label, pick) = if in_place {
        ("Update now", UpdatePick::Update)
    } else {
        ("Download", UpdatePick::Download)
    };
    col.push(ext_link("Full release notes", rel.url.clone()))
        .push(space().height(18))
        .push(wide(Kind::Primary, label, 40.0, Msg::Update(pick)))
        .push(space().height(14))
        .push(wide(
            Kind::Small,
            "Later",
            40.0,
            Msg::Update(UpdatePick::Later),
        ))
        .into()
}

/// "What's new" after an update: the notes since `seen`.
pub fn whats_new<'a>(seen: &str) -> Element<'a, Msg> {
    let notes = crate::ui::whats_new::between(seen, VERSION);
    let mut col: Column<'a, Msg> = column![
        header("What's new", &format!("Valo {VERSION}")),
        space().height(6)
    ];
    for (v, list) in &notes {
        if notes.len() > 1 {
            col = col.push(version_heading(v));
        }
        for n in list.iter() {
            col = col
                .push(text(n.title).font(BOLD).size(16))
                .push(space().height(10))
                .push(para(n.text));
        }
    }
    if notes.is_empty() {
        col = col.push(para("This update brings fixes and small improvements."));
    }
    col.push(ext_link(
        "Full release notes",
        format!("https://github.com/Okaza03/Valo/releases/tag/v{VERSION}"),
    ))
    .push(space().height(18))
    .push(wide(Kind::Primary, "Got it", 40.0, Msg::Close))
    .into()
}

/// A muted paragraph and the gap under it.
fn para<'a>(t: &str) -> Element<'a, Msg> {
    column![
        text(t.to_owned()).size(14.5).color(style::MUTED()),
        space().height(22)
    ]
    .into()
}

/// "VALO 0.8.6" over a version's notes.
fn version_heading<'a>(v: &str) -> Element<'a, Msg> {
    column![
        mono(&format!("Valo {v}"), 11.0).color(c(ACCENT())),
        space().height(14)
    ]
    .into()
}

/// A button as wide as the dialog.
fn wide<'a>(kind: Kind, t: &str, h: f32, msg: Msg) -> Element<'a, Msg> {
    widgets::button_wide(kind, t, h, Some(msg))
}

/// The update notice in the bottom right corner.
pub fn toast<'a>(t: &ToastView, secs: f32) -> Element<'a, Msg> {
    let words = column![
        mono(&t.title, 11.0).color(style::MUTED()),
        text(t.body.clone()).size(14.5).color(style::TEXT()),
    ]
    .spacing(2)
    .width(Length::Fill);
    let mut r = row![words].spacing(10).align_y(Vertical::Center);
    if t.busy {
        r = r.push(icon::spinner(secs, style::TEXT(), 18.0));
    } else {
        if t.download {
            r = r.push(small("Download", Msg::Dismiss(true)));
        }
        r = r.push(super::chrome::tip_below(
            widgets::small_icon_button(Icon::Close, Msg::Dismiss(false)),
            "Dismiss",
        ));
    }
    container(
        container(r)
            .padding([12, 16])
            .max_width(412)
            .style(|_| container::Style {
                background: Some(Background::Color(style::BG())),
                border: Border {
                    color: c(LINE()),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                text_color: Some(style::TEXT()),
                ..container::Style::default()
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding::default().right(16.0).bottom(46.0))
    .align_x(Horizontal::Right)
    .align_y(Vertical::Bottom)
    .into()
}

// -------------------------------------------------------------- splash

/// On the way to the library it stays at least this long, so it doesn't just flash.
const HOLD: Duration = Duration::from_millis(1200);
/// Seconds.
const FADE_IN: f32 = 0.45;
const FADE_OUT: f32 = 0.35;

/// The wordmark over the window after signing in, while the library loads
/// (egui's `ui::splash`).
pub struct Splash {
    start: Instant,
    /// When it started fading out.
    leaving: Option<Instant>,
    logo: image::Handle,
    logo_size: (f32, f32),
}

impl Splash {
    pub fn new() -> Self {
        let png = include_bytes!("../../assets/valo-text-white.png");
        let img = ::image::load_from_memory(png)
            .expect("bundled wordmark is a valid PNG")
            .into_rgba8();
        let (w, h) = img.dimensions();
        Self {
            start: Instant::now(),
            leaving: None,
            logo: image::Handle::from_rgba(w, h, img.into_raw()),
            logo_size: (w as f32, h as f32),
        }
    }

    /// `ready`: the page underneath has loaded; `hold`: it's the library,
    /// so stay up for HOLD first (an error shows straight away). False once
    /// it has faded out.
    pub fn tick(&mut self, ready: bool, hold: bool) -> bool {
        let now = Instant::now();
        if self.leaving.is_none() && ready && (!hold || now >= self.start + HOLD) {
            self.leaving = Some(now);
        }
        self.cover() > 0.0
    }

    fn cover(&self) -> f32 {
        match self.leaving {
            Some(t) => 1.0 - t.elapsed().as_secs_f32() / FADE_OUT,
            None => 1.0,
        }
    }

    /// Over the whole window, `size` big; it swallows clicks meant for the page.
    pub fn view<'a, M: Clone + 'a>(&self, size: Size, swallow: M) -> Element<'a, M> {
        let cover = self.cover().clamp(0.0, 1.0);
        let t = (self.start.elapsed().as_secs_f32() / FADE_IN).min(1.0);
        let ease = 1.0 - (1.0 - t).powi(3);
        let w = (size.width * 0.32).clamp(180.0, 360.0) * (0.94 + 0.06 * ease);
        let h = w * self.logo_size.1 / self.logo_size.0;
        let bg = style::BG().scale_alpha(cover);
        mouse_area(
            container(
                image(self.logo.clone())
                    .width(w)
                    .height(h)
                    .opacity(cover * ease),
            )
            .center(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(bg)),
                ..container::Style::default()
            }),
        )
        .on_press(swallow)
        .into()
    }
}
