//! Window chrome: our own title bar (the window has no system decorations),
//! the status bar, and edge resizing.
use eframe::egui::{
    self, vec2, Align, Color32, CursorIcon, Id, Layout, Pos2, Rect, ResizeDirection, Response,
    Sense, Stroke, Ui, ViewportCommand,
};

use super::{mono, BG, LINE, MUTED, SURFACE_HI, TEXT};

pub const BAR_H: f32 = 46.0;

#[derive(Clone, Copy)]
pub enum Icon {
    Refresh,
    Settings,
    Minimize,
    Maximize,
    Restore,
    Close,
    Bookmark,
    BookmarkOn,
    Play,
    ChevronLeft,
    ArrowRight,
    Star,
    Pause,
    ChevronRight,
    Subtitles,
    Volume,
    VolumeMuted,
    Fullscreen,
    FullscreenExit,
    More,
    List,
    /// Circular arrows; the caller writes the seconds inside.
    SkipBack,
    SkipForward,
    ChevronUp,
    ChevronDown,
    ArrowUp,
    ArrowDown,
    /// Opens outside the app (a web page): an arrow out of a corner.
    External,
    /// Sidebar: a house.
    Home,
    /// Sidebar: a film frame with a play mark.
    Film,
    /// Sidebar: a screen on a stand.
    Tv,
}

/// Top bar: word mark, `left` (tabs), `right` (search, buttons), window buttons.
pub fn title_bar(
    ctx: &egui::Context,
    name: &str,
    left: impl FnOnce(&mut Ui),
    right: impl FnOnce(&mut Ui),
) {
    egui::TopBottomPanel::top("title_bar")
        .exact_height(BAR_H)
        .frame(egui::Frame::new().fill(BG()).inner_margin(egui::Margin {
            left: 18,
            right: 0,
            top: 0,
            bottom: 0,
        }))
        .show(ctx, |ui| {
            drag_area(ui, ui.max_rect());
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                super::logo(ui, name, 19.0);
                ui.add_space(14.0);
                vsep(ui);
                ui.add_space(10.0);
                left(ui);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    window_buttons(ui);
                    vsep(ui);
                    right(ui);
                });
            });
        });
}

/// Dragging moves the window; double-click maximizes.
pub fn drag_area(ui: &mut Ui, rect: Rect) {
    let r = ui.interact(
        rect,
        Id::new("window_drag").with(rect.top() as i32),
        Sense::click_and_drag(),
    );
    if r.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if r.double_clicked() {
        toggle_maximized(ui.ctx());
    }
}

fn toggle_maximized(ctx: &egui::Context) {
    let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    ctx.send_viewport_cmd(ViewportCommand::Maximized(!max));
}

/// Minimize / maximize / close, laid out right-to-left.
pub fn window_buttons(ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    if icon_button(ui, Icon::Close, "Close").clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
    let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if icon_button(
        ui,
        if max { Icon::Restore } else { Icon::Maximize },
        if max { "Restore" } else { "Maximize" },
    )
    .clicked()
    {
        toggle_maximized(&ctx);
    }
    if icon_button(ui, Icon::Minimize, "Minimize").clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
    }
}

/// The refresh icon that checks for a new Valo: a spinner while checking, and an
/// "Update" tab next to it when one is out.
pub fn update_button(
    ui: &mut Ui,
    updates: &crate::update::Updates,
) -> Option<crate::update::Click> {
    use crate::update::Click;
    let mut click = None;
    if updates.checking() {
        let (r, _) = ui.allocate_exact_size(vec2(BAR_H, BAR_H), Sense::hover());
        egui::Spinner::new()
            .size(16.0)
            .color(TEXT())
            .paint_at(ui, Rect::from_center_size(r.center(), vec2(16.0, 16.0)));
    } else {
        if icon_button(ui, Icon::Refresh, "Check for updates").clicked() {
            click = Some(Click::Check);
        }
    }
    if let Some(r) = &updates.available {
        ui.add_space(8.0);
        let tip = format!("Valo {} is available — click to update", r.version);
        if super::tab(ui, &format!("Update {}", r.version), true)
            .on_hover_text(tip)
            .clicked()
        {
            click = Some(Click::ShowUpdate);
        }
        ui.add_space(8.0);
    }
    click
}

/// A full-height vertical divider.
pub fn vsep(ui: &mut Ui) {
    let h = ui.available_height().min(BAR_H);
    let (r, _) = ui.allocate_exact_size(vec2(1.0, h), Sense::hover());
    if super::theme::skin() != super::theme::Skin::Valo {
        return; // the streaming styles don't rule off their top bar
    }
    ui.painter()
        .vline(r.center().x, r.y_range(), Stroke::new(1.0f32, LINE()));
}

/// A square bar button with a drawn icon.
pub fn icon_button(ui: &mut Ui, icon: Icon, tip: &str) -> Response {
    let (r, resp) = ui.allocate_exact_size(
        vec2(BAR_H, BAR_H.min(ui.available_height())),
        Sense::click(),
    );
    let p = ui.painter();
    let hot = resp.hovered();
    if hot {
        let fill = if matches!(icon, Icon::Close) {
            Color32::from_rgb(196, 43, 28)
        } else {
            SURFACE_HI()
        };
        p.rect_filled(r, 0.0, fill);
    }
    let col = if hot && matches!(icon, Icon::Close) {
        Color32::WHITE
    } else if hot {
        TEXT()
    } else {
        TEXT().gamma_multiply(0.82)
    };
    draw_icon(p, r.center(), icon, col);
    resp.on_hover_text(tip)
}

pub fn draw_icon(p: &egui::Painter, c: Pos2, icon: Icon, col: Color32) {
    let s = Stroke::new(1.4f32, col);
    match icon {
        Icon::Minimize => {
            p.hline(c.x - 6.0..=c.x + 6.0, c.y, s);
        }
        Icon::Maximize => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(11.0, 11.0)),
                0.0,
                s,
                egui::StrokeKind::Inside,
            );
        }
        Icon::Restore => {
            p.rect_stroke(
                Rect::from_center_size(c + vec2(-1.5, 1.5), vec2(9.0, 9.0)),
                0.0,
                s,
                egui::StrokeKind::Inside,
            );
            p.line_segment([c + vec2(-1.5, -4.5), c + vec2(4.5, -4.5)], s);
            p.line_segment([c + vec2(4.5, -4.5), c + vec2(4.5, 1.5)], s);
        }
        Icon::Close => {
            p.line_segment([c + vec2(-6.0, -6.0), c + vec2(6.0, 6.0)], s);
            p.line_segment([c + vec2(6.0, -6.0), c + vec2(-6.0, 6.0)], s);
        }
        Icon::Refresh => {
            let s = Stroke::new(1.8f32, col);
            let r = 7.0;
            let pts: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let a = -0.35 * std::f32::consts::PI
                        + i as f32 / 24.0 * 1.65 * std::f32::consts::PI;
                    c + vec2(a.cos(), a.sin()) * r
                })
                .collect();
            let tip = pts[0];
            p.add(egui::Shape::line(pts, s));
            p.add(egui::Shape::convex_polygon(
                vec![
                    tip + vec2(-4.5, -2.0),
                    tip + vec2(2.5, -3.5),
                    tip + vec2(1.0, 3.5),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Icon::Bookmark | Icon::BookmarkOn => {
            let pts = vec![
                c + vec2(-5.0, -7.0),
                c + vec2(5.0, -7.0),
                c + vec2(5.0, 7.0),
                c + vec2(0.0, 3.0),
                c + vec2(-5.0, 7.0),
            ];
            if matches!(icon, Icon::BookmarkOn) {
                // concave at the notch: fill as two halves
                for half in [[pts[0], pts[3], pts[4]], [pts[0], pts[1], pts[3]]] {
                    p.add(egui::Shape::convex_polygon(
                        half.to_vec(),
                        col,
                        Stroke::NONE,
                    ));
                }
                p.add(egui::Shape::convex_polygon(
                    vec![pts[1], pts[2], pts[3]],
                    col,
                    Stroke::NONE,
                ));
            }
            p.add(egui::Shape::closed_line(pts, Stroke::new(1.6f32, col)));
        }
        Icon::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + vec2(-4.0, -6.0),
                    c + vec2(6.0, 0.0),
                    c + vec2(-4.0, 6.0),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Icon::ChevronLeft => {
            p.line_segment([c + vec2(2.0, -4.0), c + vec2(-2.0, 0.0)], s);
            p.line_segment([c + vec2(-2.0, 0.0), c + vec2(2.0, 4.0)], s);
        }
        Icon::ArrowRight => {
            p.line_segment([c + vec2(-5.0, 0.0), c + vec2(5.0, 0.0)], s);
            p.line_segment([c + vec2(1.0, -4.0), c + vec2(5.0, 0.0)], s);
            p.line_segment([c + vec2(5.0, 0.0), c + vec2(1.0, 4.0)], s);
        }
        Icon::Star => {
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                    let r = if i % 2 == 0 { 6.0 } else { 2.6 };
                    c + vec2(a.cos(), a.sin()) * r
                })
                .collect();
            // a star isn't convex: fan of triangles from the center
            for i in 0..10 {
                p.add(egui::Shape::convex_polygon(
                    vec![c, pts[i], pts[(i + 1) % 10]],
                    col,
                    Stroke::NONE,
                ));
            }
        }
        Icon::Pause => {
            for dx in [-3.5, 3.5] {
                p.rect_filled(
                    Rect::from_center_size(c + vec2(dx, 0.0), vec2(3.5, 13.0)),
                    1.0,
                    col,
                );
            }
        }
        Icon::ChevronRight => {
            p.line_segment([c + vec2(-2.0, -4.0), c + vec2(2.0, 0.0)], s);
            p.line_segment([c + vec2(2.0, 0.0), c + vec2(-2.0, 4.0)], s);
        }
        Icon::Subtitles => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(16.0, 12.0)),
                2.0,
                s,
                egui::StrokeKind::Middle,
            );
            p.hline(c.x - 4.5..=c.x - 1.5, c.y + 1.0, s);
            p.hline(c.x + 0.5..=c.x + 4.5, c.y + 1.0, s);
            p.hline(c.x - 4.5..=c.x + 1.5, c.y - 2.5, s);
        }
        Icon::Volume | Icon::VolumeMuted => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + vec2(-7.0, -2.5),
                    c + vec2(-4.0, -2.5),
                    c + vec2(0.0, -6.5),
                    c + vec2(0.0, 6.5),
                    c + vec2(-4.0, 2.5),
                    c + vec2(-7.0, 2.5),
                ],
                Color32::TRANSPARENT,
                s,
            ));
            if matches!(icon, Icon::VolumeMuted) {
                p.line_segment([c + vec2(3.0, -3.0), c + vec2(8.0, 3.0)], s);
                p.line_segment([c + vec2(8.0, -3.0), c + vec2(3.0, 3.0)], s);
            } else {
                for r in [4.0f32, 7.5] {
                    let pts: Vec<Pos2> = (0..=10)
                        .map(|i| {
                            let a = -0.9 + i as f32 / 10.0 * 1.8;
                            c + vec2(a.cos(), a.sin()) * r + vec2(0.5, 0.0)
                        })
                        .collect();
                    p.add(egui::Shape::line(pts, s));
                }
            }
        }
        Icon::Fullscreen | Icon::FullscreenExit => {
            let out = matches!(icon, Icon::Fullscreen);
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let corner = c + vec2(sx * 7.0, sy * 7.0);
                let (a, b) = if out {
                    (corner - vec2(sx * 4.0, 0.0), corner - vec2(0.0, sy * 4.0))
                } else {
                    let k = c + vec2(sx * 3.0, sy * 3.0);
                    (k + vec2(sx * 4.0, 0.0), k + vec2(0.0, sy * 4.0))
                };
                let k = if out {
                    corner
                } else {
                    c + vec2(sx * 3.0, sy * 3.0)
                };
                p.line_segment([a, k], s);
                p.line_segment([k, b], s);
            }
        }
        Icon::More => {
            for dy in [-5.5, 0.0, 5.5] {
                p.circle_filled(c + vec2(0.0, dy), 1.7, col);
            }
        }
        Icon::ChevronUp => {
            p.line_segment([c + vec2(-4.0, 2.0), c + vec2(0.0, -2.0)], s);
            p.line_segment([c + vec2(0.0, -2.0), c + vec2(4.0, 2.0)], s);
        }
        Icon::ChevronDown => {
            p.line_segment([c + vec2(-4.0, -2.0), c + vec2(0.0, 2.0)], s);
            p.line_segment([c + vec2(0.0, 2.0), c + vec2(4.0, -2.0)], s);
        }
        Icon::ArrowUp => {
            p.line_segment([c + vec2(0.0, 5.0), c + vec2(0.0, -5.0)], s);
            p.line_segment([c + vec2(-4.0, -1.0), c + vec2(0.0, -5.0)], s);
            p.line_segment([c + vec2(0.0, -5.0), c + vec2(4.0, -1.0)], s);
        }
        Icon::ArrowDown => {
            p.line_segment([c + vec2(0.0, -5.0), c + vec2(0.0, 5.0)], s);
            p.line_segment([c + vec2(-4.0, 1.0), c + vec2(0.0, 5.0)], s);
            p.line_segment([c + vec2(0.0, 5.0), c + vec2(4.0, 1.0)], s);
        }
        Icon::External => {
            // a box open at the top right, and an arrow leaving it
            let s = Stroke::new(1.2f32, col);
            p.line_segment([c + vec2(-1.0, -4.0), c + vec2(-4.0, -4.0)], s);
            p.line_segment([c + vec2(-4.0, -4.0), c + vec2(-4.0, 4.0)], s);
            p.line_segment([c + vec2(-4.0, 4.0), c + vec2(4.0, 4.0)], s);
            p.line_segment([c + vec2(4.0, 4.0), c + vec2(4.0, 1.0)], s);
            p.line_segment([c + vec2(-0.5, 0.5), c + vec2(4.5, -4.5)], s);
            p.line_segment([c + vec2(1.0, -4.5), c + vec2(4.5, -4.5)], s);
            p.line_segment([c + vec2(4.5, -4.5), c + vec2(4.5, -1.0)], s);
        }
        Icon::Home => {
            let s = Stroke::new(1.6f32, col);
            p.line_segment([c + vec2(-8.0, -0.5), c + vec2(0.0, -8.0)], s);
            p.line_segment([c + vec2(0.0, -8.0), c + vec2(8.0, -0.5)], s);
            let body = Rect::from_min_max(c + vec2(-5.5, -2.5), c + vec2(5.5, 7.0));
            p.rect_stroke(body, 1.0, s, egui::StrokeKind::Middle);
        }
        Icon::Film => {
            let s = Stroke::new(1.6f32, col);
            let r = Rect::from_center_size(c, vec2(16.0, 13.0));
            p.rect_stroke(r, 2.5, s, egui::StrokeKind::Middle);
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + vec2(-2.5, -3.5),
                    c + vec2(4.0, 0.0),
                    c + vec2(-2.5, 3.5),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Icon::Tv => {
            let s = Stroke::new(1.6f32, col);
            let r = Rect::from_center_size(c + vec2(0.0, -1.5), vec2(17.0, 12.0));
            p.rect_stroke(r, 2.0, s, egui::StrokeKind::Middle);
            p.hline(c.x - 4.0..=c.x + 4.0, c.y + 7.5, s);
        }
        Icon::List => {
            for dy in [-5.0, 0.0, 5.0] {
                p.hline(c.x - 7.0..=c.x + 7.0, c.y + dy, s);
            }
        }
        Icon::SkipBack | Icon::SkipForward => {
            let back = matches!(icon, Icon::SkipBack);
            let s = Stroke::new(1.8f32, col);
            let r = 11.0;
            // an open circle, its gap at the top, with an arrowhead on one end
            let (a0, a1) = (-1.25f32, 4.0f32);
            let pts: Vec<Pos2> = (0..=28)
                .map(|i| {
                    let a = a0 + i as f32 / 28.0 * (a1 - a0);
                    let a = if back { std::f32::consts::PI - a } else { a };
                    c + vec2(a.cos(), a.sin()) * r
                })
                .collect();
            let tip = pts[0];
            p.add(egui::Shape::line(pts, s));
            let d = if back { -1.0 } else { 1.0 };
            p.add(egui::Shape::convex_polygon(
                vec![
                    tip + vec2(-4.0 * d, -4.0),
                    tip + vec2(3.0 * d, 0.5),
                    tip + vec2(-4.0 * d, 4.0),
                ],
                col,
                Stroke::NONE,
            ));
        }
        Icon::Settings => {
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([c + d * 5.0, c + d * 8.5], Stroke::new(3.0f32, col));
            }
            p.circle_filled(c, 6.2, col);
            p.circle_filled(c, 2.6, BG());
        }
    }
}

/// Bottom status line: small monospace text on both sides.
/// The strip along the bottom. With `refresh`, the left text is a button;
/// true when it's clicked.
pub fn status_bar(ctx: &egui::Context, left: &str, right: &str, refresh: bool) -> bool {
    egui::TopBottomPanel::bottom("status_bar")
        .exact_height(30.0)
        .frame(
            egui::Frame::new()
                .fill(BG())
                .inner_margin(egui::Margin::symmetric(18, 0)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                // hovered last frame: light up like a link
                let id = ui.id().with("status_left");
                let hovered = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
                let col = if refresh && hovered { TEXT() } else { MUTED() };
                let r = ui.add(
                    egui::Label::new(mono(left, 10.5).color(col))
                        .truncate()
                        .sense(if refresh {
                            Sense::click()
                        } else {
                            Sense::hover()
                        }),
                );
                ui.data_mut(|d| d.insert_temp(id, r.hovered()));
                let clicked = refresh
                    && r.on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text("Refresh the library")
                        .clicked();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(mono(right, 10.5).color(MUTED()));
                });
                clicked
            })
            .inner
        })
        .inner
}

/// Enter or leave fullscreen. Always use this instead of sending
/// `ViewportCommand::Fullscreen` directly: see `crate::fullscreen`.
pub fn set_fullscreen(ctx: &egui::Context, on: bool) {
    crate::fullscreen::set(on);
    ctx.send_viewport_cmd(ViewportCommand::Fullscreen(on));
}

/// Resize from the window edges (the window has no system frame), plus a hairline border.
pub fn window_frame(ctx: &egui::Context) {
    let (max, fs) = ctx.input(|i| {
        (
            i.viewport().maximized.unwrap_or(false),
            i.viewport().fullscreen.unwrap_or(false),
        )
    });
    if max || fs {
        return;
    }
    let screen = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        Id::new("window_border"),
    ))
    .rect_stroke(
        screen,
        0.0,
        Stroke::new(1.0f32, LINE()),
        egui::StrokeKind::Inside,
    );
    let Some(p) = ctx.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    let m = 5.0;
    let (l, r, t, b) = (
        p.x - screen.left() < m,
        screen.right() - p.x < m,
        p.y - screen.top() < m,
        screen.bottom() - p.y < m,
    );
    use ResizeDirection::*;
    let dir = match (l, r, t, b) {
        (true, _, true, _) => NorthWest,
        (_, true, true, _) => NorthEast,
        (true, _, _, true) => SouthWest,
        (_, true, _, true) => SouthEast,
        (true, ..) => West,
        (_, true, ..) => East,
        (_, _, true, _) => North,
        (.., true) => South,
        _ => return,
    };
    ctx.set_cursor_icon(match dir {
        NorthWest => CursorIcon::ResizeNorthWest,
        NorthEast => CursorIcon::ResizeNorthEast,
        SouthWest => CursorIcon::ResizeSouthWest,
        SouthEast => CursorIcon::ResizeSouthEast,
        West => CursorIcon::ResizeWest,
        East => CursorIcon::ResizeEast,
        North => CursorIcon::ResizeNorth,
        South => CursorIcon::ResizeSouth,
    });
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
    }
}

/// Open a link in the user's browser.
pub fn open_url(url: &str) {
    // `cmd /c start` would treat `&` in the URL as a command separator
    #[cfg(windows)]
    let res = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(not(windows))]
    let res = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = res {
        eprintln!("can't open {url}: {e}");
    }
}

/// Avatar circle with the user's initial.
pub fn avatar(p: &egui::Painter, c: Pos2, r: f32, name: &str) {
    p.circle_filled(c, r, super::ACCENT());
    let initial = name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    p.text(
        c,
        egui::Align2::CENTER_CENTER,
        initial,
        super::black_font(r * 1.05),
        TEXT(),
    );
}
