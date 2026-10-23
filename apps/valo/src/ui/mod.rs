//! Screens and the shared look.
pub mod browse;
pub mod check;
pub mod chrome;
pub mod login;
pub mod settings;
pub mod setup;
pub mod splash;
pub mod theme;
pub mod watch;
pub mod whats_new;

use std::sync::Arc;

use eframe::egui::{
    self, pos2, text::LayoutJob, vec2, Color32, CornerRadius, FontData, FontDefinitions,
    FontFamily, FontId, Mesh, Painter, Rect, Response, RichText, Sense, Stroke, StrokeKind,
    TextFormat, TextureHandle, Ui,
};

use crate::util::color_for;
use theme::{Labels, Skin, Tabs};

pub use theme::{
    ACCENT, BG, CARD, ERROR, FIELD, INK, LINE, LINE_HI, MUTED, OK, PRIMARY, SURFACE, SURFACE_HI,
    TEXT, WARN,
};

fn font(bytes: &'static [u8]) -> Arc<FontData> {
    Arc::new(FontData::from_static(bytes))
}

pub fn apply_theme(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let fd = &mut fonts.font_data;
    fd.insert(
        "sans".into(),
        font(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf")),
    );
    fd.insert(
        "sans-bold".into(),
        font(include_bytes!("../../assets/fonts/NotoSans-Bold.ttf")),
    );
    fd.insert(
        "sans-black".into(),
        font(include_bytes!("../../assets/fonts/NotoSans-Black.ttf")),
    );
    fd.insert(
        "mono".into(),
        font(include_bytes!("../../assets/fonts/NotoSansMono-Medium.ttf")),
    );
    let fallback = fonts.families[&FontFamily::Proportional].clone(); // keeps emoji/icons
    let fam = &mut fonts.families;
    fam.get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "sans".into());
    fam.get_mut(&FontFamily::Monospace)
        .unwrap()
        .insert(0, "mono".into());
    for (name, first) in [("bold", "sans-bold"), ("black", "sans-black")] {
        let list = std::iter::once(first.to_owned())
            .chain(fallback.iter().cloned())
            .collect();
        fam.insert(FontFamily::Name(name.into()), list);
    }
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    use egui::TextStyle::*;
    style.text_styles = [
        (Heading, FontId::new(24.0, FontFamily::Name("bold".into()))),
        (Body, FontId::proportional(14.5)),
        (Button, FontId::proportional(14.0)),
        (Small, FontId::proportional(12.0)),
        (Monospace, FontId::monospace(12.5)),
    ]
    .into();
    style.spacing.button_padding = vec2(14.0, 6.0);
    style.spacing.item_spacing = vec2(10.0, 8.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.icon_width = 16.0;
    style.spacing.icon_width_inner = 9.0;
    style.spacing.menu_margin = egui::Margin::same(6);

    ctx.set_style(style);
    restyle(ctx);
}

/// Show `skin` in `mode`, `System` following the desktop. Cheap when nothing
/// changed, so it runs every frame.
pub fn set_look(ctx: &egui::Context, skin: theme::Skin, mode: theme::Mode) {
    let mode = match mode {
        theme::Mode::System => match ctx.system_theme() {
            Some(egui::Theme::Light) => theme::Mode::Light,
            _ => theme::Mode::Dark,
        },
        m => m,
    };
    if theme::set(skin, mode) {
        restyle(ctx);
    }
}

/// egui's own widgets (text fields, checkboxes, scroll bars) in the current colors.
fn restyle(ctx: &egui::Context) {
    let pal = theme::pal();
    let radius = theme::spec().button_radius.min(8);
    let mut style = (*ctx.style()).clone();
    let v = &mut style.visuals;
    *v = if pal.light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    v.panel_fill = BG();
    v.window_fill = SURFACE();
    v.extreme_bg_color = FIELD();
    v.faint_bg_color = SURFACE();
    v.override_text_color = Some(TEXT());
    v.hyperlink_color = TEXT();
    v.selection.bg_fill = if pal.light {
        ACCENT().gamma_multiply(0.25)
    } else {
        Color32::from_white_alpha(40)
    };
    v.selection.stroke = Stroke::new(1.0f32, TEXT());
    v.window_corner_radius = CornerRadius::same(radius.max(4));
    v.menu_corner_radius = CornerRadius::same(radius.max(4));
    v.window_stroke = Stroke::new(1.0f32, LINE());
    v.window_shadow = egui::Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if pal.light { 40 } else { 140 }),
    };
    v.popup_shadow = v.window_shadow;
    for (w, fill) in [
        (&mut v.widgets.noninteractive, SURFACE()),
        (&mut v.widgets.inactive, SURFACE()),
        (&mut v.widgets.hovered, SURFACE_HI()),
        (
            &mut v.widgets.active,
            theme::mix(SURFACE_HI(), TEXT(), 0.06),
        ),
        (&mut v.widgets.open, SURFACE_HI()),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.corner_radius = CornerRadius::same(radius);
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0f32, LINE());
    v.widgets.inactive.bg_stroke = Stroke::new(1.0f32, LINE());
    v.widgets.hovered.bg_stroke = Stroke::new(1.0f32, LINE_HI());
    v.widgets.active.bg_stroke = Stroke::new(1.0f32, TEXT());
    v.widgets.open.bg_stroke = Stroke::new(1.0f32, LINE_HI());
    v.widgets.noninteractive.fg_stroke.color = TEXT();
    v.widgets.inactive.fg_stroke.color = TEXT();
    v.widgets.hovered.fg_stroke.color = TEXT();
    v.widgets.active.fg_stroke.color = TEXT();
    ctx.set_style(style);
}

// ----------------------------------------------------------------- text

/// The font, size and spacing of a small label in the current style.
fn label_font(size: f32) -> (FontId, f32, bool) {
    match theme::spec().labels {
        Labels::Mono => (FontId::monospace(size), size * 0.14, true),
        Labels::Caps => (
            FontId::new(size * 1.08, FontFamily::Name("bold".into())),
            size * 0.12,
            true,
        ),
        Labels::Sans => {
            let fam = if size >= 12.0 {
                FontFamily::Name("bold".into())
            } else {
                FontFamily::Proportional
            };
            (FontId::new(size * 1.14, fam), 0.0, false)
        }
    }
}

/// A small label (section headers, tabs, buttons) in the current style:
/// letter-spaced monospace capitals for Valo, plain sans for most others.
pub fn mono(text: impl Into<String>, size: f32) -> RichText {
    let (font, spacing, upper) = label_font(size);
    let text = text.into();
    RichText::new(if upper { text.to_uppercase() } else { text })
        .font(font)
        .extra_letter_spacing(spacing)
}

/// [`mono`] as a layout section.
fn label_job(job: &mut LayoutJob, text: &str, size: f32, color: Color32, leading: f32) {
    let (font_id, extra_letter_spacing, upper) = label_font(size);
    let text = if upper {
        text.to_uppercase()
    } else {
        text.to_owned()
    };
    job.append(
        &text,
        leading,
        TextFormat {
            font_id,
            color,
            extra_letter_spacing,
            ..Default::default()
        },
    );
}

pub fn bold(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text.into())
        .family(FontFamily::Name("bold".into()))
        .size(size)
}

/// A row title on the home page ("Continue watching").
pub fn heading(text: &str) -> RichText {
    let spec = theme::spec();
    match spec.labels {
        Labels::Mono => mono(text, spec.row_title),
        _ => bold(text, spec.row_title),
    }
}

pub fn black_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("black".into()))
}

/// The word mark in the title bar: the server's name, or the app's.
pub fn logo(ui: &mut Ui, text: &str, size: f32) {
    let spec = theme::spec();
    let mut text = if spec.logo_lower {
        text.to_lowercase()
    } else {
        text.to_uppercase()
    };
    if text.chars().count() > 28 {
        text = text.chars().take(27).collect::<String>() + "…";
    }
    if spec.logo_badge {
        // a rounded red badge with a play triangle in front of the name
        let (r, _) = ui.allocate_exact_size(vec2(size * 1.45, size), Sense::hover());
        let p = ui.painter();
        p.rect_filled(r, size * 0.28, ACCENT());
        let c = r.center() + vec2(size * 0.05, 0.0);
        let s = size * 0.24;
        p.add(egui::Shape::convex_polygon(
            vec![c + vec2(-s, -s), c + vec2(s * 1.1, 0.0), c + vec2(-s, s)],
            Color32::WHITE,
            Stroke::NONE,
        ));
        ui.add_space(-2.0);
    }
    let (font, spacing) = if spec.logo_lower || spec.logo_badge {
        (FontId::new(size, FontFamily::Name("bold".into())), -0.3)
    } else {
        (black_font(size), size * 0.04)
    };
    let col = if spec.logo_accent { ACCENT() } else { TEXT() };
    ui.label(
        RichText::new(text)
            .font(font)
            .extra_letter_spacing(spacing)
            .color(col),
    );
}

/// Muted section header, e.g. "PLAYBACK".
pub fn section(ui: &mut Ui, text: &str) {
    ui.add_space(10.0);
    ui.label(mono(text, 11.5).color(MUTED()));
    ui.add_space(2.0);
}

// -------------------------------------------------------------- buttons

/// How a button is painted in the current style.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// White (or the accent): "Play", "Done".
    Primary,
    /// Translucent: "More info", "Start over".
    Secondary,
    /// Small and quiet: "Refresh", "Sign out".
    Small,
}

fn button_radius(h: f32) -> f32 {
    (theme::spec().button_radius as f32).min(h / 2.0)
}

/// Paint a button's background; returns its text color.
fn paint_button(p: &Painter, r: Rect, kind: Kind, hovered: bool, enabled: bool) -> Color32 {
    let skin = theme::skin();
    let pal = theme::pal();
    let rad = button_radius(r.height());
    match kind {
        Kind::Primary => {
            let fill = if !enabled {
                PRIMARY().gamma_multiply(0.45)
            } else if hovered {
                theme::mix(PRIMARY(), BG(), 0.14)
            } else {
                PRIMARY()
            };
            p.rect_filled(r, rad, fill);
            INK()
        }
        Kind::Secondary => {
            if skin == Skin::Valo {
                if !pal.light {
                    p.rect_filled(r, rad, Color32::from_black_alpha(60));
                }
                p.rect_stroke(r, rad, Stroke::new(1.0f32, LINE_HI()), StrokeKind::Inside);
                if hovered {
                    p.rect_filled(r, rad, SURFACE_HI().gamma_multiply(0.6));
                }
            } else if skin == Skin::Disney {
                let a = if hovered { 140 } else { 80 };
                p.rect_filled(r, rad, Color32::from_black_alpha(a));
                p.rect_stroke(r, rad, Stroke::new(1.5f32, TEXT()), StrokeKind::Inside);
            } else {
                // Netflix's gray "More info" and its cousins
                let a = if hovered { 0.32 } else { 0.22 };
                let base = if pal.light { TEXT() } else { Color32::WHITE };
                p.rect_filled(r, rad, base.gamma_multiply(a));
            }
            TEXT()
        }
        Kind::Small => {
            if skin == Skin::Valo {
                if hovered {
                    p.rect_filled(r, rad, SURFACE_HI());
                }
                p.rect_stroke(r, rad, Stroke::new(1.0f32, LINE_HI()), StrokeKind::Inside);
            } else {
                let fill = if hovered {
                    theme::mix(SURFACE_HI(), TEXT(), 0.1)
                } else {
                    SURFACE_HI()
                };
                p.rect_filled(r, rad, fill);
            }
            TEXT()
        }
    }
}

fn text_button(ui: &mut Ui, text: &str, kind: Kind, enabled: bool) -> Response {
    let (size, h) = match kind {
        Kind::Small => (11.5, 30.0),
        _ => (12.5, 38.0),
    };
    let pad = if kind == Kind::Small { 12.0 } else { 18.0 };
    let mut job = LayoutJob::default();
    label_job(&mut job, text, size, Color32::PLACEHOLDER, 0.0);
    let g = ui.fonts(|f| f.layout_job(job));
    let w = (g.size().x + 2.0 * pad).max(ui.spacing().interact_size.x);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (r, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    if ui.is_rect_visible(r) {
        let p = ui.painter();
        let col = paint_button(p, r, kind, resp.hovered() && enabled, enabled);
        let col = if enabled {
            col
        } else {
            col.gamma_multiply(0.5)
        };
        p.galley(r.center() - g.size() / 2.0, g, col);
    }
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

pub fn primary_button(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    text_button(ui, text.trim(), Kind::Primary, enabled)
}

pub fn secondary_button(ui: &mut Ui, text: &str) -> Response {
    text_button(ui, text, Kind::Secondary, true)
}

pub fn admin_help(ui: &mut Ui, missing: crate::api::Missing, base: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(missing.hint()).small().color(MUTED()));
    ui.add_space(6.0);
    let id = egui::Id::new(("admin-note-copied", base));
    let copied = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    ui.horizontal(|ui| {
        let label = if copied {
            "Copied, paste it to the admin"
        } else {
            "Copy message for the admin"
        };
        if small_button(ui, label).clicked() {
            ui.ctx().copy_text(missing.admin_note(base));
            ui.data_mut(|d| d.insert_temp(id, true));
        }
        ui.add_space(6.0);
        if ui.link("API docs").clicked() {
            chrome::open_url(crate::api::API_DOCS);
        }
    });
}

pub fn small_button(ui: &mut Ui, text: &str) -> Response {
    text_button(ui, text, Kind::Small, true)
}

/// A square [`small_button`] with a drawn icon (the "⋮" menu, close).
pub fn small_icon_button(ui: &mut Ui, icon: chrome::Icon, tip: &str) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
    let col = paint_button(ui.painter(), r, Kind::Small, resp.hovered(), true);
    chrome::draw_icon(ui.painter(), r.center(), icon, col);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tip)
}

/// A [`small_button`] with a drawn icon: before the text, or after it for
/// [`Icon::External`](chrome::Icon::External) and chevrons pointing right.
pub fn small_button_icon(ui: &mut Ui, icon: chrome::Icon, text: &str) -> Response {
    icon_text_button(ui, icon, text, false, true)
}

/// A [`primary_button`] with a drawn icon before the text ("▶ Play").
pub fn primary_button_icon(ui: &mut Ui, icon: chrome::Icon, text: &str, enabled: bool) -> Response {
    icon_text_button(ui, icon, text, true, enabled)
}

fn icon_text_button(
    ui: &mut Ui,
    icon: chrome::Icon,
    text: &str,
    primary: bool,
    enabled: bool,
) -> Response {
    use chrome::Icon;
    let (kind, size, h, pad) = if primary {
        (Kind::Primary, 12.5, 38.0, 18.0)
    } else {
        (Kind::Small, 11.5, 30.0, 10.0)
    };
    let after = matches!(icon, Icon::External | Icon::ChevronRight | Icon::ArrowRight);
    let mut job = LayoutJob::default();
    label_job(&mut job, text, size, Color32::PLACEHOLDER, 0.0);
    let g = ui.fonts(|f| f.layout_job(job));
    let text_w = g.size().x;
    let w = text_w + 16.0 + 8.0 + 2.0 * pad;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (r, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    let p = ui.painter();
    let col = paint_button(p, r, kind, resp.hovered() && enabled, enabled);
    let col = if enabled {
        col
    } else {
        col.gamma_multiply(0.4)
    };
    let (icon_x, text_x) = if after {
        (r.left() + pad + text_w + 8.0 + 8.0, r.left() + pad)
    } else {
        (r.left() + pad + 8.0, r.left() + pad + 16.0 + 8.0)
    };
    chrome::draw_icon(p, pos2(icon_x, r.center().y), icon, col);
    p.galley(pos2(text_x, r.center().y - g.size().y / 2.0), g, col);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// A link that opens outside the app, with the "opens in browser" icon.
pub fn ext_link(ui: &mut Ui, text: impl Into<RichText>) -> bool {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let l = ui.link(text.into());
        let (r, i) = ui.allocate_exact_size(vec2(11.0, 11.0), Sense::click());
        let col = if l.hovered() || i.hovered() {
            TEXT()
        } else {
            MUTED()
        };
        chrome::draw_icon(ui.painter(), r.center(), chrome::Icon::External, col);
        l.clicked() || i.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
    })
    .inner
}

pub fn text_field(ui: &mut Ui, value: &mut String, hint: &str, password: bool) -> Response {
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .password(password)
            .desired_width(f32::INFINITY)
            .margin(egui::Margin::symmetric(10, 9))
            .font(FontId::proportional(15.0)),
    )
}

/// A tab's label (and count); the label is [`Color32::PLACEHOLDER`], colored
/// when painted, since hovering changes it.
fn tab_galley(ui: &Ui, label: &str, count: Option<usize>, selected: bool) -> Arc<egui::Galley> {
    let spec = theme::spec();
    let mut job = LayoutJob::default();
    if spec.tabs == Tabs::Plain {
        // Netflix: the selected tab in bold
        let fam = if selected {
            FontFamily::Name("bold".into())
        } else {
            FontFamily::Proportional
        };
        job.append(
            label,
            0.0,
            TextFormat {
                font_id: FontId::new(14.5, fam),
                color: Color32::PLACEHOLDER,
                ..Default::default()
            },
        );
    } else {
        label_job(&mut job, label, 12.0, Color32::PLACEHOLDER, 0.0);
    }
    if let Some(c) = count {
        let on_fill = selected && matches!(spec.tabs, Tabs::Block | Tabs::Pill);
        let col = if on_fill {
            INK().gamma_multiply(0.6)
        } else {
            MUTED()
        };
        label_job(&mut job, &c.to_string(), 12.0, col, 9.0);
    }
    ui.fonts(|f| f.layout_job(job))
}

/// Paint a tab's background; returns its text color.
fn paint_tab(p: &Painter, r: Rect, selected: bool, hovered: bool, chip: bool) -> Color32 {
    let spec = theme::spec();
    let dim = TEXT().gamma_multiply(0.72);
    match spec.tabs {
        Tabs::Block => {
            if selected {
                p.rect_filled(r, if chip { 0.0 } else { 1.0 }, PRIMARY());
                return INK();
            }
            if hovered {
                p.rect_filled(r, if chip { 0.0 } else { 1.0 }, SURFACE_HI());
            }
            TEXT()
        }
        Tabs::Plain | Tabs::Underline if !chip => {
            if spec.tabs == Tabs::Underline && (selected || hovered) {
                let a = if selected { 1.0 } else { 0.45 };
                p.rect_filled(
                    Rect::from_min_max(
                        pos2(r.left() + 6.0, r.bottom() - 2.0),
                        pos2(r.right() - 6.0, r.bottom()),
                    ),
                    1.0,
                    TEXT().gamma_multiply(a),
                );
            }
            if selected || hovered {
                TEXT()
            } else {
                dim
            }
        }
        _ => {
            // pills (and every style's filter chips)
            let rad = (spec.button_radius as f32).min(r.height() / 2.0);
            if selected {
                p.rect_filled(r, rad, PRIMARY());
                return INK();
            }
            let quiet = theme::skin() == Skin::Youtube || chip;
            if quiet || hovered {
                let fill = if hovered {
                    theme::mix(SURFACE_HI(), TEXT(), 0.1)
                } else {
                    SURFACE_HI()
                };
                p.rect_filled(r, rad, fill);
            }
            TEXT()
        }
    }
}

/// A top-bar tab, in the current style: a white block, bold text, an
/// underline or a pill when selected.
pub fn tab(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let g = tab_galley(ui, label, None, selected);
    let pill = theme::spec().tabs == Tabs::Pill;
    let (w, h) = if pill {
        (g.size().x + 26.0, 30.0)
    } else {
        (g.size().x + 16.0, 28.0)
    };
    let (r, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let p = ui.painter();
    let col = paint_tab(p, r, selected, resp.hovered(), false);
    p.galley(r.center() - g.size() / 2.0, g, col);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A row of options, the selected one highlighted (a bordered strip for
/// Valo, chips for the other styles). Returns true on change.
pub fn segmented<T: PartialEq + Clone>(
    ui: &mut Ui,
    value: &mut T,
    items: &[(T, &str, Option<usize>)],
) -> bool {
    let block = theme::spec().tabs == Tabs::Block;
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = if block { 0.0 } else { 8.0 };
        let mut rects = Vec::new();
        for (v, label, count) in items {
            let sel = *value == *v;
            let g = tab_galley(ui, label, *count, sel);
            let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 24.0, 34.0), Sense::click());
            let col = paint_tab(ui.painter(), r, sel, resp.hovered(), true);
            ui.painter().galley(r.center() - g.size() / 2.0, g, col);
            if resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
                && !sel
            {
                *value = v.clone();
                changed = true;
            }
            rects.push(r);
        }
        if let (true, Some(a), Some(b)) = (block, rects.first(), rects.last()) {
            let p = ui.painter();
            for r in &rects[1..] {
                p.vline(r.left(), r.y_range(), Stroke::new(1.0f32, LINE()));
            }
            p.rect_stroke(
                a.union(*b),
                0.0,
                Stroke::new(1.0f32, LINE()),
                StrokeKind::Inside,
            );
        }
    });
    changed
}

/// A full-width row in a dropdown menu.
pub fn menu_item(ui: &mut Ui, text: &str) -> bool {
    let w = ui.available_width().max(200.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 2.0, SURFACE_HI());
    }
    let pos = pos2(r.left() + 10.0, r.center().y);
    ui.painter().text(
        pos,
        egui::Align2::LEFT_CENTER,
        text,
        FontId::proportional(14.5),
        TEXT(),
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

// ---------------------------------------------------------------- panels

/// A centered card used by the setup and login screens.
pub fn card<R>(ctx: &egui::Context, add: impl FnOnce(&mut Ui) -> R) -> R {
    let mut out = None;
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(BG()))
        .show(ctx, |ui| {
            let full = ui.max_rect();
            let w = full.width().clamp(280.0, 440.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(((full.height() - 540.0) / 2.0).max(24.0));
                ui.vertical_centered(|ui| {
                    ui.set_max_width(w);
                    egui::Frame::new()
                        .fill(SURFACE())
                        .stroke(Stroke::new(1.0f32, LINE()))
                        .corner_radius(4)
                        .inner_margin(28)
                        .show(ui, |ui| {
                            ui.set_width(w - 56.0);
                            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                out = Some(add(ui));
                            });
                        });
                });
                ui.add_space(24.0);
            });
        });
    out.unwrap()
}

/// A rect filled with a gradient; colors are top-left, top-right, bottom-right, bottom-left.
pub fn gradient(p: &Painter, r: Rect, c: [Color32; 4]) {
    let mut m = Mesh::default();
    for (pos, col) in [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
    ]
    .into_iter()
    .zip(c)
    {
        m.colored_vertex(pos, col);
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(m);
}

/// Paint `tex` so it covers `r`, cropping whatever doesn't fit.
pub fn image_cover(p: &Painter, r: Rect, tex: &TextureHandle, radius: u8, tint: Color32) {
    let sz = tex.size_vec2();
    let (ta, ra) = (sz.x / sz.y, r.width() / r.height());
    let uv = if ta > ra {
        let w = ra / ta;
        Rect::from_min_max(pos2(0.5 - w / 2.0, 0.0), pos2(0.5 + w / 2.0, 1.0))
    } else {
        let h = ta / ra;
        // keep the top of tall images (faces, titles) rather than the middle
        Rect::from_min_max(pos2(0.0, 0.15 * (1.0 - h)), pos2(1.0, 0.15 * (1.0 - h) + h))
    };
    if radius == 0 {
        p.image(tex.id(), r, uv, tint);
    } else {
        p.add(egui::epaint::RectShape::filled(r, radius, tint).with_texture(tex.id(), uv));
    }
}

/// Artwork stand-in: a colored tile with the title on it.
pub fn placeholder(p: &Painter, r: Rect, title: &str, radius: u8) {
    let c = color_for(title);
    p.rect_filled(r, radius, c);
    let dark = Color32::from_black_alpha(140);
    gradient(
        p,
        r,
        [Color32::TRANSPARENT, Color32::TRANSPARENT, dark, dark],
    );
    let size = (r.width() / 9.0).clamp(11.0, 22.0);
    let font = FontId::new(size, FontFamily::Name("bold".into()));
    let galley = p.layout(title.to_owned(), font, Color32::WHITE, r.width() - 16.0);
    let pos = r.center() - galley.size() / 2.0;
    p.galley(pos, galley, Color32::WHITE);
}

/// A "WATCHED" sash across a poster's top right corner. `fade`: 0 hides it.
pub fn watched_sash(p: &Painter, art: Rect, fade: f32) {
    if fade <= 0.0 {
        return;
    }
    let k = (art.width() / 150.0).clamp(0.8, 1.6);
    // the band runs between two diagonals, `a` and `a + w` in from the corner
    let (a, w) = (34.0 * k, 18.0 * k);
    let (right, top) = (art.right(), art.top());
    let p = p.with_clip_rect(art.intersect(p.clip_rect()));
    p.add(egui::Shape::convex_polygon(
        vec![
            pos2(right - a - w, top),
            pos2(right - a, top),
            pos2(right, top + a),
            pos2(right, top + a + w),
        ],
        ACCENT().gamma_multiply(fade),
        Stroke::new(1.0f32, Color32::from_black_alpha((60.0 * fade) as u8)),
    ));
    let font = FontId::new(9.0 * k, FontFamily::Name("bold".into()));
    let galley = p.layout_no_wrap("WATCHED".to_owned(), font, Color32::WHITE);
    let c = (a + w / 2.0) / 2.0;
    let center = pos2(right - c, top + c);
    let angle = std::f32::consts::FRAC_PI_4;
    let half = galley.size() / 2.0;
    let (sin, cos) = angle.sin_cos();
    let pos = center - vec2(half.x * cos - half.y * sin, half.x * sin + half.y * cos);
    p.add(
        egui::epaint::TextShape::new(pos, galley, Color32::WHITE)
            .with_angle(angle)
            .with_override_text_color(Color32::WHITE.gamma_multiply(fade)),
    );
}

pub fn spinner_text(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(16.0).color(TEXT()));
        ui.label(mono(text, 11.5).color(MUTED()));
    });
}

/// What a screen asks the app to do next.
pub enum Action {
    None,
    /// Run the server check on a newly entered address.
    CheckServer(String),
    /// The server answered; sign in or go straight to the library.
    Connected(crate::api::Client),
    /// The server check passed (and the user signed in if needed).
    Ready {
        client: crate::api::Client,
        user: Option<String>,
    },
    SignedIn {
        client: crate::api::Client,
        user: String,
        remember: bool,
    },
    /// The saved session was rejected.
    NeedLogin(crate::api::Client),
    /// The server says the signed-in user's admin rights changed.
    AdminChanged(bool),
    SignOut(crate::api::Client),
    ChangeServer,
    Refresh,
    OpenFiles,
    CheckUpdates,
    /// Re-show the "update available" notice.
    ShowUpdate,
    OpenSettings,
    Quit,
    Play(Vec<watch::PlayItem>),
}
