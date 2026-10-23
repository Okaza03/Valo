//! The details pages: movie, series (Episodes / Related / Details tabs) and
//! a single episode. Sections the server sends no data for are left out.
mod styled;

use eframe::egui::{
    self, epaint::Vertex, pos2, text::LayoutJob, vec2, Align, Align2, Color32, CursorIcon,
    FontFamily, FontId, Layout, Mesh, Painter, Rect, Response, RichText, ScrollArea, Sense, Stroke,
    TextFormat, TextureHandle, Ui, UiBuilder,
};

use std::hash::{BuildHasher, RandomState};

use super::{Browse, Click, Probe, Tab, View, GAP, SIDE};
use crate::api::{featurette, CastMember, Episode, FileInfo, Item, Library};
use crate::config::Progress;
use crate::images::Images;
use crate::ui::chrome::{self, Icon};
use crate::ui::theme::Details;
use crate::ui::{black_font, bold, gradient, image_cover, mono, placeholder};
use crate::ui::{BG, INK, LINE, LINE_HI, MUTED, PRIMARY, SURFACE_HI, TEXT};
use crate::util::{color_for, fmt_runtime, fmt_time, percent_decode};

/// Ratings, quality and the main button on Valo's details pages.
#[allow(non_snake_case)]
pub(super) fn GOLD() -> Color32 {
    if crate::ui::theme::pal().light {
        Color32::from_rgb(168, 118, 10)
    } else {
        Color32::from_rgb(232, 196, 106)
    }
}

/// Small captions ("Cast", "File · Metadata").
#[allow(non_snake_case)]
pub(super) fn TEAL() -> Color32 {
    if crate::ui::theme::pal().light {
        Color32::from_rgb(0, 128, 112)
    } else {
        Color32::from_rgb(110, 205, 190)
    }
}

/// The main button: gold for Valo, the style's own button color otherwise.
pub(super) fn play_colors() -> (Color32, Color32) {
    if crate::ui::theme::skin() == crate::ui::theme::Skin::Valo {
        (GOLD(), INK())
    } else {
        (PRIMARY(), INK())
    }
}

const MAX_W: f32 = 1420.0;
/// Below this content width, side-by-side columns stack.
pub(super) const NARROW: f32 = 980.0;

// ------------------------------------------------------------- formatting

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// "2002-10-31" -> (2002, 10, 31)
fn ymd(s: &str) -> Option<(u32, usize, u32)> {
    let mut it = s.get(..10)?.split('-');
    let y = it.next()?.parse().ok()?;
    let m: usize = it.next()?.parse().ok()?;
    let d = it.next()?.parse().ok()?;
    (1..=12).contains(&m).then_some((y, m, d))
}

/// "31 Oct 2002"
pub(super) fn date_long(s: &str) -> String {
    ymd(s).map_or_else(
        || s.to_owned(),
        |(y, m, d)| format!("{d} {} {y}", MONTHS[m - 1]),
    )
}

/// Unix seconds -> "31 Oct 2002" (UTC).
pub(super) fn date_unix(t: i64) -> String {
    // days to a civil date: Howard Hinnant's days_from_civil, backwards
    let z = t.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{d} {} {y}", MONTHS[m as usize - 1])
}

/// "Oct 31, 2002"
fn date_short(s: &str) -> String {
    ymd(s).map_or_else(
        || s.to_owned(),
        |(y, m, d)| format!("{} {d}, {y}", MONTHS[m - 1]),
    )
}

/// 8000000 -> "$8,000,000"
fn money(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    format!("${out}")
}

/// 2090000000 -> "2.09 GB"
pub(super) fn bytes(n: u64) -> String {
    let n = n as f64;
    match n {
        n if n >= 1e9 => format!("{:.2} GB", n / 1e9),
        n if n >= 1e6 => format!("{:.0} MB", n / 1e6),
        n => format!("{:.0} KB", n / 1e3),
    }
}

/// 2600000 -> "2.6 Mb/s"
fn bitrate(n: u64) -> String {
    format!("{:.1} Mb/s", n as f64 / 1e6)
}

fn minutes(secs: f64) -> String {
    format!("{} min", (secs / 60.0).round())
}

/// "2021–2024", or just the start year.
fn years(item: &Item) -> Option<String> {
    let y = item.year?;
    Some(match item.end_year.filter(|e| *e != y) {
        Some(e) => format!("{y}–{e}"),
        None => y.to_string(),
    })
}

/// "Arcane.S01E01.1080p.mp4": the server's file name, else the stream URL's
/// when it looks like one (not "/api/stream/movie/42").
fn file_name(file: Option<&FileInfo>, stream: &str) -> Option<String> {
    file.and_then(|f| f.name.clone()).or_else(|| {
        let last = stream.split(['?', '#']).next().unwrap_or(stream);
        let name = percent_decode(last.rsplit('/').next().unwrap_or(last));
        let ext = name.rsplit_once('.')?.1.to_lowercase();
        crate::api::VIDEO_EXTS
            .contains(&ext.as_str())
            .then_some(name)
    })
}

/// The server's file info with gaps filled from probing the stream.
fn merge(server: Option<&FileInfo>, probed: Option<&FileInfo>) -> Option<FileInfo> {
    let (s, p) = match (server, probed) {
        (None, None) => return None,
        (Some(s), None) => return Some(s.clone()),
        (None, Some(p)) => return Some(p.clone()),
        (Some(s), Some(p)) => (s, p),
    };
    Some(FileInfo {
        quality: s.quality.clone().or_else(|| p.quality.clone()),
        container: s.container.clone().or_else(|| p.container.clone()),
        video_codec: s.video_codec.clone().or_else(|| p.video_codec.clone()),
        audio_codec: s.audio_codec.clone().or_else(|| p.audio_codec.clone()),
        audio_channels: s.audio_channels.or(p.audio_channels),
        bitrate: s.bitrate.or(p.bitrate),
        size: s.size.or(p.size),
        name: s.name.clone().or_else(|| p.name.clone()),
        path: s.path.clone().or_else(|| p.path.clone()),
    })
}

fn sxe(season: u32, ep: u32) -> String {
    featurette(season, ep).unwrap_or_else(|| format!("S{season:02}E{ep:02}"))
}

// ------------------------------------------------------------- layout

/// Left margin and width of the centered content column.
pub(super) fn column(ui: &Ui) -> (f32, f32) {
    let w = ui.available_width();
    let inner = (w - 2.0 * SIDE).clamp(200.0, MAX_W);
    ((w - inner) / 2.0, inner)
}

/// Run `add` inside the centered content column.
pub(super) fn padded<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let (m, inner) = column(ui);
    let sp = ui.spacing().item_spacing.x;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(m);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.x = sp;
            ui.set_width(inner);
            add(ui)
        })
        .inner
    })
    .inner
}

/// A child Ui over `r` whose content is pushed down by `align` (0 = top,
/// 0.5 = centered, 1 = bottom) using last frame's content height.
pub(super) fn placed(ui: &mut Ui, r: Rect, salt: &str, align: f32, add: impl FnOnce(&mut Ui)) {
    let id = ui.id().with(salt);
    let prev: f32 = ui.data(|d| d.get_temp(id)).unwrap_or(r.height());
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(r)
            .layout(Layout::top_down(Align::Min)),
    );
    child.add_space(((r.height() - prev) * align).max(0.0));
    let top = child.cursor().top();
    add(&mut child);
    let used = child.cursor().top() - top;
    if (used - prev).abs() > 0.5 {
        ui.data_mut(|d| d.insert_temp(id, used));
        ui.ctx().request_repaint();
    }
}

/// Full-width artwork, darkened, fading into the page at the bottom.
pub(super) fn backdrop(p: &Painter, rect: Rect, art: Option<&TextureHandle>, title: &str, dim: u8) {
    p.rect_filled(rect, 0.0, BG());
    match art {
        Some(t) => image_cover(p, rect, t, 0, Color32::from_gray(dim)),
        None => {
            let c = color_for(title);
            gradient(
                p,
                rect,
                [c.gamma_multiply(0.45), c.gamma_multiply(0.25), BG(), BG()],
            );
        }
    }
    let side = Rect::from_min_max(
        rect.min,
        pos2(rect.left() + rect.width() * 0.5, rect.bottom()),
    );
    let shade = Color32::from_black_alpha(110);
    gradient(
        p,
        side,
        [shade, Color32::TRANSPARENT, Color32::TRANSPARENT, shade],
    );
    // fade into the page last, so nothing darker shows at the seam
    let h = rect.height();
    let fade = Rect::from_min_max(pos2(rect.left(), rect.bottom() - h * 0.45), rect.max);
    gradient(
        p,
        fade,
        [Color32::TRANSPARENT, Color32::TRANSPARENT, BG(), BG()],
    );
}

/// Darker (f < 1) or lighter (f > 1) version of an opaque color.
fn shade(c: Color32, f: f32) -> Color32 {
    let s = |v: u8| (v as f32 * f).clamp(0.0, 255.0) as u8;
    Color32::from_rgb(s(c.r()), s(c.g()), s(c.b()))
}

// ------------------------------------------------------------- widgets

/// A filled button with an optional icon; `radius` 0 makes it a pill.
pub(super) fn fill_button(
    ui: &mut Ui,
    text: &str,
    icon: Option<Icon>,
    colors: (Color32, Color32),
    height: f32,
    min_w: f32,
    radius: f32,
) -> Response {
    let (fill, fg) = colors;
    let font = FontId::new(15.0, FontFamily::Name("bold".into()));
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, fg);
    let icon_w = if icon.is_some() { 22.0 } else { 0.0 };
    let w = (galley.size().x + icon_w + 48.0).max(min_w);
    let (r, resp) = ui.allocate_exact_size(vec2(w, height), Sense::click());
    let radius = match (radius, crate::ui::theme::skin()) {
        (0.0, crate::ui::theme::Skin::Valo) => height / 2.0,
        (0.0, _) => (crate::ui::theme::spec().button_radius as f32).min(height / 2.0),
        (r, _) => r,
    };
    let fill = if resp.hovered() {
        shade(fill, 0.88)
    } else {
        fill
    };
    let p = ui.painter();
    p.rect_filled(r, radius, fill);
    let x = r.center().x - (icon_w + galley.size().x) / 2.0;
    if let Some(i) = icon {
        chrome::draw_icon(p, pos2(x + 6.0, r.center().y), i, fg);
    }
    p.galley(
        pos2(x + icon_w, r.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// A round translucent icon button (bookmark).
fn circle_button(ui: &mut Ui, icon: Icon, tip: &str) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(42.0, 42.0), Sense::click());
    let fill = if resp.hovered() {
        crate::ui::theme::mix(SURFACE_HI(), TEXT(), 0.12)
    } else {
        SURFACE_HI()
    };
    ui.painter().circle_filled(r.center(), 21.0, fill);
    chrome::draw_icon(ui.painter(), r.center(), icon, TEXT());
    resp.on_hover_text(tip)
        .on_hover_cursor(CursorIcon::PointingHand)
}

/// Text (with an optional leading/trailing icon) that lights up on hover.
pub(super) fn link(
    ui: &mut Ui,
    text: &str,
    size: f32,
    col: Color32,
    icon: Option<(Icon, bool)>,
) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), FontId::proportional(size), col);
    let iw = if icon.is_some() { 16.0 } else { 0.0 };
    let (r, resp) = ui.allocate_exact_size(galley.size() + vec2(iw, 4.0), Sense::click());
    let col = if resp.hovered() { TEXT() } else { col };
    let p = ui.painter();
    let (tx, ix) = match icon {
        Some((_, true)) => (r.left(), r.right() - 6.0),
        _ => (r.left() + iw, r.left() + 5.0),
    };
    p.text(
        pos2(tx, r.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(size),
        col,
    );
    if let Some((i, _)) = icon {
        chrome::draw_icon(p, pos2(ix, r.center().y), i, col);
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// A small outlined tag: "1080p", "TV-14".
pub(super) fn tag(ui: &mut Ui, text: &str) {
    tag_with(ui, text, Sense::hover());
}

/// A [`tag`] that can be clicked.
pub(super) fn tag_link(ui: &mut Ui, text: &str) -> Response {
    tag_with(ui, text, Sense::click()).on_hover_cursor(CursorIcon::PointingHand)
}

fn tag_with(ui: &mut Ui, text: &str, sense: Sense) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), FontId::proportional(11.5), TEXT());
    let (r, resp) = ui.allocate_exact_size(galley.size() + vec2(12.0, 6.0), sense);
    let p = ui.painter();
    let shade = if resp.hovered() { 160 } else { 80 };
    p.rect_filled(r, 3.0, Color32::from_black_alpha(shade));
    p.rect_stroke(
        r,
        3.0,
        Stroke::new(1.0f32, LINE_HI()),
        egui::StrokeKind::Inside,
    );
    p.galley(r.min + vec2(6.0, 3.0), galley, TEXT());
    resp
}

pub(super) fn dot(ui: &mut Ui) {
    ui.label(RichText::new("·").color(MUTED()));
}

pub(super) fn caps(ui: &mut Ui, text: &str, col: Color32) {
    ui.label(mono(text, 11.0).color(col));
}

/// Text cut to `rows` lines with an ellipsis.
fn clamp_text(ui: &mut Ui, text: &str, size: f32, col: Color32, rows: usize) {
    let mut job = LayoutJob::single_section(
        text.to_owned(),
        TextFormat::simple(FontId::proportional(size), col),
    );
    job.wrap.max_width = ui.available_width();
    job.wrap.max_rows = rows;
    job.wrap.overflow_character = Some('…');
    let g = ui.fonts(|f| f.layout_job(job));
    ui.label(g);
}

/// `tex` cropped to a circle (faces sit near the top of headshots).
pub(super) fn image_circle(p: &Painter, c: egui::Pos2, r: f32, tex: &TextureHandle) {
    let sz = tex.size_vec2();
    let side = sz.x.min(sz.y);
    let (du, dv) = (side / sz.x, side / sz.y);
    let (u0, v0) = ((1.0 - du) / 2.0, (1.0 - dv) * 0.2);
    let mut m = Mesh::with_texture(tex.id());
    let vert = |pos, uv| Vertex {
        pos,
        uv,
        color: Color32::WHITE,
    };
    m.vertices.push(vert(c, pos2(u0 + du / 2.0, v0 + dv / 2.0)));
    let n = 48;
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let (s, co) = a.sin_cos();
        m.vertices.push(vert(
            c + vec2(co, s) * r,
            pos2(u0 + du * (0.5 + 0.5 * co), v0 + dv * (0.5 + 0.5 * s)),
        ));
    }
    for i in 1..=n as u32 {
        m.add_triangle(0, i, i + 1);
    }
    p.add(m);
}

/// Headshots in a grid, name and character under each. Returns whose
/// person page to open.
pub(super) fn cast_grid(
    ui: &mut Ui,
    cast: &[CastMember],
    images: &mut Images,
    cell_w: f32,
    r: f32,
) -> Option<Click> {
    let mut click = None;
    let w = ui.available_width();
    let cols = ((w / cell_w).floor() as usize).max(1);
    let cell_w = w / cols as f32;
    for (ri, row) in cast.chunks(cols).enumerate() {
        let (rect, _) = ui.allocate_exact_size(vec2(w, 2.0 * r + 52.0), Sense::hover());
        for (i, m) in row.iter().enumerate() {
            let cell = Rect::from_min_size(
                pos2(rect.left() + cell_w * i as f32, rect.top()),
                vec2(cell_w, rect.height()),
            );
            let resp = ui.interact(cell, ui.id().with(("cast", ri, i)), Sense::click());
            let hot = resp.hovered();
            let p = ui.painter();
            let c = pos2(cell.center().x, rect.top() + r);
            match m.photo.as_ref().and_then(|u| images.get(u)) {
                Some(t) => image_circle(p, c, r, &t),
                None => chrome::avatar(p, c, r, &m.name),
            }
            if hot {
                p.circle_stroke(c, r + 3.0, Stroke::new(2.0f32, TEXT()));
            }
            // one line each, shortened with "…" to fit the cell
            let line = |text: &str, font: FontId, col: Color32, y: f32| {
                let mut job =
                    LayoutJob::single_section(text.to_owned(), TextFormat::simple(font, col));
                job.wrap.max_width = cell_w - 12.0;
                job.wrap.max_rows = 1;
                job.wrap.overflow_character = Some('…');
                let g = p.layout_job(job);
                p.galley(pos2(c.x - g.size().x / 2.0, y), g, col);
            };
            let name_col = if hot {
                TEXT()
            } else {
                TEXT().gamma_multiply(0.9)
            };
            line(
                &m.name,
                FontId::proportional(14.0),
                name_col,
                c.y + r + 12.0,
            );
            if let Some(ch) = &m.character {
                line(ch, FontId::monospace(11.0), MUTED(), c.y + r + 32.0);
            }
            if resp
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text("See their movies and shows")
                .clicked()
            {
                click = Some(Click::Nav(View::Person(m.name.clone())));
            }
        }
        ui.add_space(14.0);
    }
    click
}

/// Names as links to their person pages, separated by commas.
pub(super) fn people_links(ui: &mut Ui, names: &[&str], size: f32) -> Option<Click> {
    let mut click = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, n) in names.iter().enumerate() {
            if i > 0 {
                ui.label(RichText::new(", ").size(size).color(MUTED()));
            }
            if link(ui, n, size, TEXT(), None)
                .on_hover_text("See their movies and shows")
                .clicked()
            {
                click = Some(Click::Nav(View::Person((*n).to_owned())));
            }
        }
    });
    click
}

/// "A, B" as names, for a server that sends several directors in one string.
pub(super) fn split_names(s: &str) -> Vec<&str> {
    s.split(',')
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .collect()
}

/// A [`kv_rows`] row whose value is names that open each person's page.
/// `line`: a hairline under it, as when more rows follow.
fn kv_people_row(ui: &mut Ui, key: &str, names: &[&str], line: bool) -> Option<Click> {
    let mut click = None;
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(key).size(14.0).color(MUTED()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            // laid out from the right: last name first
            for (i, n) in names.iter().enumerate().rev() {
                if link(ui, n, 14.0, TEXT(), None)
                    .on_hover_text("See their movies and shows")
                    .clicked()
                {
                    click = Some(Click::Nav(View::Person((*n).to_owned())));
                }
                if i > 0 {
                    ui.label(RichText::new(", ").size(14.0).color(MUTED()));
                }
            }
        });
    });
    ui.add_space(10.0);
    if line {
        let r = ui.available_rect_before_wrap();
        ui.painter()
            .hline(r.x_range(), r.top(), Stroke::new(1.0f32, LINE()));
    }
    click
}

/// An [`info_rows`] row whose value is names that open each person's page.
pub(super) fn info_people_row(
    ui: &mut Ui,
    key: &str,
    names: &[&str],
    label_w: f32,
) -> Option<Click> {
    let mut click = None;
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(label_w, 18.0), Sense::hover());
        ui.painter().text(
            r.left_center(),
            Align2::LEFT_CENTER,
            key,
            FontId::proportional(14.0),
            MUTED(),
        );
        click = people_links(ui, names, 14.0);
    });
    ui.add_space(12.0);
    click
}

/// Rounded panel used for metadata, critics, info.
pub(super) fn panel<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(crate::ui::CARD())
        .stroke(Stroke::new(1.0f32, LINE()))
        .corner_radius(10)
        .inner_margin(24)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Label on the left, value on the right, a hairline under each row.
pub(super) fn kv_rows(ui: &mut Ui, rows: &[(&str, String, Color32, f32)]) {
    for (i, (k, v, col, size)) in rows.iter().enumerate() {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(*k).size(14.0).color(MUTED()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new(v).size(*size).color(*col)).truncate());
            });
        });
        ui.add_space(10.0);
        if i + 1 < rows.len() {
            let r = ui.available_rect_before_wrap();
            ui.painter()
                .hline(r.x_range(), r.top(), Stroke::new(1.0f32, LINE()));
        }
    }
}

/// Label column + value column, no lines (series "Info").
pub(super) fn info_rows(ui: &mut Ui, rows: &[(&str, String)], label_w: f32) {
    for (k, v) in rows {
        ui.horizontal_top(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(label_w, 18.0), Sense::hover());
            ui.painter().text(
                r.left_center(),
                Align2::LEFT_CENTER,
                *k,
                FontId::proportional(14.0),
                MUTED(),
            );
            ui.add(egui::Label::new(RichText::new(v).size(14.0).color(TEXT())).wrap());
        });
        ui.add_space(12.0);
    }
}

/// The title as artwork (logo) when the server has one, else big text.
pub(super) fn title_art(ui: &mut Ui, item: &Item, images: &mut Images, max: egui::Vec2, size: f32) {
    if let Some(t) = item.logo.as_ref().and_then(|u| images.get(u)) {
        let sz = t.size_vec2();
        let s = (max.x / sz.x).min(max.y / sz.y);
        let (r, _) = ui.allocate_exact_size(sz * s, Sense::hover());
        ui.painter().image(
            t.id(),
            r,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        ui.add(
            egui::Label::new(
                RichText::new(&item.title)
                    .font(black_font(size))
                    .color(TEXT()),
            )
            .wrap(),
        );
        // (over artwork the caller sets the text color)
    }
}

/// A poster with title and year under it.
/// `sub` goes under the title (the year, a character).
pub(super) fn poster_tile(
    ui: &mut Ui,
    item: &Item,
    sub: Option<&str>,
    w: f32,
    watched: bool,
    images: &mut Images,
) -> bool {
    let art_h = w * 1.5;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, art_h + 44.0), Sense::click());
    if !ui.is_rect_visible(rect) {
        return false;
    }
    let art = Rect::from_min_size(rect.min, vec2(w, art_h));
    let p = ui.painter();
    match item.poster.as_ref().and_then(|u| images.get(u)) {
        Some(t) => image_cover(p, art, &t, 6, Color32::WHITE),
        None => placeholder(p, art, &item.title, 6),
    }
    if watched {
        crate::ui::watched_sash(p, art, 1.0);
    }
    if resp.hovered() {
        p.rect_stroke(
            art,
            6.0,
            Stroke::new(2.0f32, TEXT()),
            egui::StrokeKind::Outside,
        );
    }
    let clip = p.with_clip_rect(rect);
    clip.text(
        pos2(art.left(), art.bottom() + 10.0),
        Align2::LEFT_TOP,
        &item.title,
        FontId::proportional(13.5),
        TEXT(),
    );
    if let Some(s) = sub {
        clip.text(
            pos2(art.left(), art.bottom() + 28.0),
            Align2::LEFT_TOP,
            s,
            FontId::proportional(11.5),
            MUTED(),
        );
    }
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    resp.clone().on_hover_text_at_pointer(&item.title);
    resp.clicked()
}

/// Wide artwork with the title and year on it ("Related").
fn wide_tile(ui: &mut Ui, item: &Item, size: egui::Vec2, images: &mut Images) -> bool {
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    if !ui.is_rect_visible(r) {
        return false;
    }
    let p = ui.painter();
    let art = item.backdrop.as_ref().or(item.poster.as_ref());
    match art.and_then(|u| images.get(u)) {
        Some(t) => image_cover(p, r, &t, 6, Color32::WHITE),
        None => placeholder(p, r, &item.title, 6),
    }
    let low = Rect::from_min_max(pos2(r.left(), r.bottom() - 70.0), r.max);
    let dark = Color32::from_black_alpha(190);
    gradient(
        p,
        low,
        [Color32::TRANSPARENT, Color32::TRANSPARENT, dark, dark],
    );
    p.text(
        pos2(r.left() + 12.0, r.bottom() - 32.0),
        Align2::LEFT_BOTTOM,
        &item.title,
        FontId::new(14.0, FontFamily::Name("bold".into())),
        TEXT(),
    );
    if let Some(y) = item.year {
        p.text(
            pos2(r.left() + 12.0, r.bottom() - 12.0),
            Align2::LEFT_BOTTOM,
            y.to_string(),
            FontId::proportional(11.5),
            MUTED(),
        );
    }
    if resp.hovered() {
        p.rect_stroke(
            r,
            6.0,
            Stroke::new(2.0f32, TEXT()),
            egui::StrokeKind::Outside,
        );
    }
    resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
}

/// `n` random items sharing a genre with `item`, the same ones each time
/// for one `shuffle`.
pub(super) fn similar<'a>(
    lib: &'a Library,
    item: &'a Item,
    n: usize,
    shuffle: &RandomState,
) -> Vec<&'a Item> {
    let mut v: Vec<&Item> = lib
        .all()
        .filter(|i| i.id != item.id && i.genres.iter().any(|g| item.genres.contains(g)))
        .collect();
    v.sort_by_cached_key(|i| shuffle.hash_one((&item.id, &i.id)));
    v.truncate(n);
    v
}

/// (season index, episode index) of every episode, in order.
fn all_episodes(s: &Item) -> Vec<(usize, usize)> {
    s.seasons
        .iter()
        .enumerate()
        .flat_map(|(si, season)| (0..season.episodes.len()).map(move |ei| (si, ei)))
        .collect()
}

// ------------------------------------------------------------- pages

impl Browse {
    /// File info for `stream`: what the server sent, completed by probing the
    /// stream once in the background (cached), plus the probed duration.
    fn file_for(
        &mut self,
        ctx: &egui::Context,
        stream: &str,
        server: Option<&FileInfo>,
    ) -> (Option<FileInfo>, Option<f64>) {
        let complete = server.is_some_and(|f| f.quality.is_some() && f.size.is_some());
        if !complete && !self.probes.contains_key(stream) {
            let src = crate::player::Source::Url {
                url: stream.to_owned(),
                auth: self.client.auth_header(),
            };
            self.probes.insert(
                stream.to_owned(),
                Probe::Running(crate::util::Task::spawn(ctx, move || {
                    crate::player::probe(&src)
                })),
            );
        }
        if let Some(Probe::Running(t)) = self.probes.get_mut(stream) {
            if let Some(r) = t.poll() {
                let done = r.map_or(Probe::Failed, |(f, d)| Probe::Done(f, d));
                self.probes.insert(stream.to_owned(), done);
            }
        }
        match self.probes.get(stream) {
            Some(Probe::Done(f, d)) => (merge(server, Some(f)), *d),
            _ => (server.cloned(), None),
        }
    }

    fn bookmark(&mut self, ui: &mut Ui, item: &Item) {
        let key = self.key(&item.id);
        let on = self.watchlist.contains(&key);
        let (icon, tip) = if on {
            (Icon::BookmarkOn, "Remove from My list")
        } else {
            (Icon::Bookmark, "Add to My list")
        };
        if circle_button(ui, icon, tip).clicked() {
            self.watchlist.toggle(&key);
        }
    }

    pub(super) fn movie_page(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        item: &Item,
        images: &mut Images,
        progress: &Progress,
    ) -> Option<Click> {
        let mut click = None;
        let (file, probed) = match item.stream.as_deref() {
            Some(s) => self.file_for(ui.ctx(), s, item.file.as_ref()),
            None => (item.file.clone(), None),
        };
        let runtime = item.duration.filter(|d| *d > 0.0).or(probed);
        let details = crate::ui::theme::spec().layout.details;
        if details == Details::Watch {
            return self.watch_page(ui, lib, item, images, progress, file.as_ref(), runtime);
        }
        if details != Details::Valo {
            click = self.styled_header(ui, item, images, progress, file.as_ref(), runtime);
        } else {
            let w = ui.available_width();
            let h = (w * 0.5).clamp(520.0, 700.0);
            let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
            let art = item.backdrop.as_ref().or(item.poster.as_ref());
            let tex = art.and_then(|u| images.get(u));
            backdrop(&ui.painter_at(rect), rect, tex.as_ref(), &item.title, 125);

            let back = Rect::from_min_size(rect.min + vec2(28.0, 22.0), vec2(300.0, 24.0));
            placed(ui, back, "movie_back", 0.0, |ui| {
                let r = link(
                    ui,
                    "Back to movies",
                    13.0,
                    TEXT().gamma_multiply(0.8),
                    Some((Icon::ChevronLeft, false)),
                );
                if r.clicked() {
                    click = Some(Click::Back);
                }
            });

            let (m, inner) = column(ui);
            let left = rect.left() + m;
            let show_poster = inner >= 860.0;
            let poster_w = if show_poster {
                (inner * 0.22).clamp(220.0, 320.0)
            } else {
                0.0
            };
            if show_poster {
                let poster_h = poster_w * 1.5;
                let pr = Rect::from_min_size(
                    pos2(left, rect.center().y - poster_h / 2.0),
                    vec2(poster_w, poster_h),
                );
                let p = ui.painter_at(rect);
                match item.poster.as_ref().and_then(|u| images.get(u)) {
                    Some(t) => image_cover(&p, pr, &t, 8, Color32::WHITE),
                    None => placeholder(&p, pr, &item.title, 8),
                }
                if let Some(d) = &item.director {
                    let top = Rect::from_min_size(pr.min, vec2(pr.width(), 70.0));
                    let dark = Color32::from_black_alpha(150);
                    gradient(
                        &p,
                        top,
                        [dark, dark, Color32::TRANSPARENT, Color32::TRANSPARENT],
                    );
                    let at = p.text(
                        pr.min + vec2(20.0, 22.0),
                        Align2::LEFT_TOP,
                        format!("A FILM BY {}", d.to_uppercase()),
                        FontId::new(11.5, FontFamily::Name("bold".into())),
                        GOLD(),
                    );
                    // the first director's page; the header lists them all
                    let by = ui
                        .interact(at.expand(4.0), ui.id().with("film_by"), Sense::click())
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("See their movies and shows");
                    if by.clicked() {
                        if let Some(n) = split_names(d).first() {
                            click = Some(Click::Nav(View::Person((*n).to_owned())));
                        }
                    }
                }
                let low = Rect::from_min_max(pos2(pr.left(), pr.bottom() - 120.0), pr.max);
                let dark = Color32::from_black_alpha(200);
                gradient(
                    &p,
                    low,
                    [Color32::TRANSPARENT, Color32::TRANSPARENT, dark, dark],
                );
                p.text(
                    pos2(pr.left() + 20.0, pr.bottom() - 46.0),
                    Align2::LEFT_BOTTOM,
                    &item.title,
                    FontId::new(24.0, FontFamily::Name("bold".into())),
                    TEXT(),
                );
                if let Some(y) = item.year {
                    p.text(
                        pos2(pr.left() + 20.0, pr.bottom() - 20.0),
                        Align2::LEFT_BOTTOM,
                        y.to_string(),
                        FontId::new(14.0, FontFamily::Name("bold".into())),
                        GOLD(),
                    );
                }
                let f = file.as_ref();
                let under: Vec<String> = [
                    f.and_then(|f| f.quality.clone()),
                    f.and_then(|f| f.size).map(bytes),
                ]
                .into_iter()
                .flatten()
                .collect();
                p.text(
                    pos2(pr.left(), pr.bottom() + 14.0),
                    Align2::LEFT_TOP,
                    under.join("   ·   "),
                    FontId::proportional(12.0),
                    MUTED(),
                );
            }

            let tx = left + if show_poster { poster_w + 40.0 } else { 0.0 };
            let text_rect = Rect::from_min_max(
                pos2(tx, rect.top() + 60.0),
                pos2(left + inner, rect.bottom() - 30.0),
            );
            placed(ui, text_rect, "movie_hero", 0.5, |ui| {
                let mut kinds = vec!["Movie".to_owned()];
                if !item.genres.is_empty() {
                    kinds.push(item.genres.join(" · "));
                }
                caps(ui, &kinds.join("   ·   "), TEXT().gamma_multiply(0.65));
                ui.add_space(8.0);
                title_art(ui, item, images, vec2(560.0, 96.0), 54.0);
                ui.add_space(14.0);
                if !item.overview.is_empty() {
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("“{}”", item.overview))
                                .italics()
                                .size(17.0)
                                .color(TEXT().gamma_multiply(0.7)),
                        )
                        .wrap(),
                    );
                    ui.add_space(16.0);
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    let mut first = true;
                    let mut sep = |ui: &mut Ui| {
                        if !std::mem::take(&mut first) {
                            dot(ui);
                        }
                    };
                    if let Some(s) = item.score {
                        sep(ui);
                        let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                        chrome::draw_icon(ui.painter(), r.center(), Icon::Star, GOLD());
                        ui.label(RichText::new(format!("{s:.1}")).size(15.0).color(TEXT()));
                        if let Some(src) = &item.score_source {
                            ui.label(RichText::new(src).size(15.0).color(MUTED()));
                        }
                    }
                    if let Some(y) = item.year {
                        sep(ui);
                        ui.label(RichText::new(y.to_string()).size(15.0).color(GOLD()));
                    }
                    if let Some(d) = runtime {
                        sep(ui);
                        ui.label(RichText::new(fmt_runtime(d)).size(15.0).color(TEXT()));
                    }
                    if let Some(q) = file.as_ref().and_then(|f| f.quality.as_deref()) {
                        sep(ui);
                        tag(ui, q);
                    }
                    if let Some(r) = &item.rating {
                        sep(ui);
                        tag(ui, r);
                    }
                });
                let directors: Vec<&str> = super::makers(item).collect();
                if !directors.is_empty() {
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Director").size(14.0).color(MUTED()));
                        if let Some(c) = people_links(ui, &directors, 14.0) {
                            click = Some(c);
                        }
                    });
                }
                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    let (label, resume) = self.play_label(item, progress);
                    let label = if label == "Play" {
                        "Play movie".into()
                    } else {
                        label
                    };
                    if fill_button(ui, &label, Some(Icon::Play), play_colors(), 42.0, 0.0, 0.0)
                        .clicked()
                    {
                        click = Some(Click::Play(item.id.clone(), None, false));
                    }
                    if resume
                        && fill_button(
                            ui,
                            "Start over",
                            None,
                            (SURFACE_HI(), TEXT()),
                            42.0,
                            0.0,
                            0.0,
                        )
                        .clicked()
                    {
                        click = Some(Click::Play(item.id.clone(), None, true));
                    }
                    self.bookmark(ui, item);
                    ui.add_space(6.0);
                    let entry = progress.get(&self.key(&item.id));
                    let status = match entry {
                        Some(e) if e.watched() => "Watched".to_owned(),
                        Some(e) if e.in_progress() => {
                            format!("Ready to stream · {:.0}% watched", e.fraction() * 100.0)
                        }
                        _ => "Ready to stream".to_owned(),
                    };
                    let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 3.5, GOLD());
                    ui.label(RichText::new(status).size(12.5).color(GOLD()));
                });
            });
        }

        ui.add_space(28.0);
        let (m, inner) = column(ui);
        let narrow = inner < NARROW;
        let gap = 48.0;
        let lw = if narrow { inner } else { inner * 0.64 };
        let rw = if narrow { inner } else { inner - lw - gap };
        let sp = ui.spacing().item_spacing.x;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(m);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.x = sp;
                ui.set_width(lw);
                if !item.cast.is_empty() {
                    caps(ui, "Cast", TEAL());
                    ui.add_space(4.0);
                    ui.label(bold("Top billed", 24.0));
                    ui.add_space(16.0);
                    let n = item.cast.len().min(10);
                    if let Some(c) = cast_grid(ui, &item.cast[..n], images, 150.0, 50.0) {
                        click = Some(c);
                    }
                    ui.add_space(24.0);
                }
                let more = similar(lib, item, 10, &self.shuffle);
                if !more.is_empty() {
                    caps(ui, "From your library", TEAL());
                    ui.add_space(4.0);
                    ui.label(bold("You might also like", 24.0));
                    ui.add_space(16.0);
                    let cols = ((lw + GAP) / (170.0 + GAP)).floor().clamp(1.0, 5.0);
                    let tw = (lw - GAP * (cols - 1.0)) / cols;
                    for row in more.chunks(cols as usize) {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = GAP;
                            for i in row {
                                let year = i.year.map(|y| y.to_string());
                                if poster_tile(ui, i, year.as_deref(), tw, self.watched(i), images)
                                {
                                    click = Some(Click::Open(i.id.clone()));
                                }
                            }
                        });
                        ui.add_space(12.0);
                    }
                }
                if narrow {
                    ui.add_space(12.0);
                    if let Some(c) = self.movie_side(ui, item, file.as_ref(), runtime) {
                        click = Some(c);
                    }
                }
            });
            if !narrow {
                ui.add_space(gap);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.x = sp;
                    ui.set_width(rw);
                    if let Some(c) = self.movie_side(ui, item, file.as_ref(), runtime) {
                        click = Some(c);
                    }
                });
            }
        });
        click
    }

    /// File/metadata and critics panels next to the movie's cast. True when
    /// where to go: the Admin page, or a director's.
    fn movie_side(
        &self,
        ui: &mut Ui,
        item: &Item,
        f: Option<&FileInfo>,
        runtime: Option<f64>,
    ) -> Option<Click> {
        let mut click = None;
        let opt = |v: Option<String>, col: Color32, size: f32| v.map(|v| (v, col, size));
        let candidates = [
            (
                "Released",
                opt(item.released.as_deref().map(date_long), TEXT(), 14.0),
            ),
            ("Runtime", opt(runtime.map(fmt_runtime), TEXT(), 14.0)),
            ("Budget", opt(item.budget.map(money), TEXT(), 14.0)),
            ("Box office", opt(item.revenue.map(money), TEXT(), 14.0)),
            (
                "Quality",
                opt(f.and_then(|f| f.quality.clone()), GOLD(), 14.0),
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
                    GOLD(),
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
                    GOLD(),
                    14.0,
                ),
            ),
            (
                "Bitrate",
                opt(f.and_then(|f| f.bitrate).map(bitrate), GOLD(), 14.0),
            ),
            (
                "File size",
                opt(f.and_then(|f| f.size).map(bytes), TEXT(), 14.0),
            ),
            (
                "File",
                opt(
                    item.stream.as_deref().and_then(|s| file_name(f, s)),
                    TEXT().gamma_multiply(0.8),
                    12.0,
                ),
            ),
            (
                "Path",
                opt(
                    f.and_then(|f| f.path.clone()),
                    TEXT().gamma_multiply(0.8),
                    12.0,
                ),
            ),
        ];
        let mut rows: Vec<(&str, String, Color32, f32)> = candidates
            .into_iter()
            .filter_map(|(k, v)| v.map(|(v, c, s)| (k, v, c, s)))
            .collect();
        if let Some(s) = self.discover.streaming_on(&item.id) {
            rows.insert(0, ("Streaming on", s, TEAL(), 14.0));
        }
        panel(ui, |ui| {
            caps(ui, "File · Metadata", TEAL());
            ui.add_space(6.0);
            let directors = item
                .director
                .as_deref()
                .map(split_names)
                .unwrap_or_default();
            if !directors.is_empty() {
                click = kv_people_row(ui, "Director", &directors, !rows.is_empty());
            }
            kv_rows(ui, &rows);
            if self.client.admin {
                ui.add_space(12.0);
                let b = egui::Button::new(RichText::new("Admin").size(13.0).color(TEXT()))
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::new(1.0f32, LINE_HI()))
                    .corner_radius(8)
                    .min_size(vec2(ui.available_width(), 34.0));
                if ui.add(b).clicked() {
                    click = Some(Click::Nav(View::Admin));
                }
            }
        });
        if !item.reviews.is_empty() {
            ui.add_space(20.0);
            panel(ui, |ui| {
                caps(ui, "From critics", MUTED());
                for r in &item.reviews {
                    ui.add_space(12.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("“{}”", r.quote))
                                .italics()
                                .size(16.0)
                                .color(TEXT().gamma_multiply(0.85)),
                        )
                        .wrap(),
                    );
                    let by: Vec<&str> = [r.source.as_deref(), r.score.as_deref()]
                        .into_iter()
                        .flatten()
                        .collect();
                    if !by.is_empty() {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("— {}", by.join(" · ")))
                                .size(12.0)
                                .color(MUTED()),
                        );
                    }
                }
            });
        }
        click
    }

    pub(super) fn series_page(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        item: &Item,
        images: &mut Images,
        progress: &Progress,
    ) -> Option<Click> {
        let mut click = None;
        if self.season >= item.seasons.len() {
            self.season = self.next_up(item, progress).map_or(0, |n| n.si);
        }
        let details = crate::ui::theme::spec().layout.details;
        if details == Details::Watch {
            return self.watch_page(ui, lib, item, images, progress, None, None);
        }
        if details != Details::Valo {
            click = self.styled_header(ui, item, images, progress, None, None);
        } else {
            let w = ui.available_width();
            let h = (w * 0.42).clamp(460.0, 640.0);
            let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
            let art = item.backdrop.as_ref().or(item.poster.as_ref());
            let tex = art.and_then(|u| images.get(u));
            backdrop(&ui.painter_at(rect), rect, tex.as_ref(), &item.title, 120);

            let (m, inner) = column(ui);
            let text_rect = Rect::from_min_max(
                pos2(rect.left() + m, rect.top() + 40.0),
                pos2(rect.left() + m + inner.min(600.0), rect.bottom() - 24.0),
            );
            placed(ui, text_rect, "series_hero", 1.0, |ui| {
                title_art(ui, item, images, vec2(380.0, 90.0), 54.0);
                ui.add_space(10.0);
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
                ui.label(
                    RichText::new(meta.join("   ·   "))
                        .size(14.0)
                        .color(TEXT().gamma_multiply(0.85)),
                );
                ui.add_space(10.0);
                if !item.overview.is_empty() {
                    clamp_text(ui, &item.overview, 15.0, TEXT().gamma_multiply(0.85), 3);
                    ui.add_space(18.0);
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    let (label, _) = self.play_label(item, progress);
                    if fill_button(
                        ui,
                        &label,
                        Some(Icon::Play),
                        (PRIMARY(), INK()),
                        44.0,
                        168.0,
                        4.0,
                    )
                    .clicked()
                    {
                        click = Some(Click::Play(item.id.clone(), None, false));
                    }
                    self.bookmark(ui, item);
                    if item.seasons.len() > 1 {
                        let mut pick = self.season;
                        egui::ComboBox::from_id_salt("season_pick")
                            .selected_text(item.seasons[pick].label())
                            .width(120.0)
                            .show_ui(ui, |ui| {
                                for (i, s) in item.seasons.iter().enumerate() {
                                    ui.selectable_value(&mut pick, i, s.label());
                                }
                            });
                        if pick != self.season {
                            self.season = pick;
                            self.tab = Tab::Episodes;
                        }
                    }
                });
                if !item.creators.is_empty() {
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Created by").size(14.0).color(MUTED()));
                        let names: Vec<&str> = item.creators.iter().map(String::as_str).collect();
                        if let Some(c) = people_links(ui, &names, 14.0) {
                            click = Some(c);
                        }
                    });
                }
            });
        }

        // tabs
        ui.add_space(18.0);
        padded(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 32.0;
                for (t, label) in [
                    (Tab::Episodes, "Episodes"),
                    (Tab::Related, "Related"),
                    (Tab::Details, "Details"),
                ] {
                    let on = self.tab == t;
                    let g = ui.painter().layout_no_wrap(
                        label.to_owned(),
                        FontId::proportional(15.0),
                        TEXT(),
                    );
                    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x, 40.0), Sense::click());
                    let col = if on || resp.hovered() {
                        TEXT()
                    } else {
                        MUTED()
                    };
                    ui.painter()
                        .galley(pos2(r.left(), r.center().y - g.size().y / 2.0), g, col);
                    if on {
                        ui.painter().hline(
                            r.x_range(),
                            r.bottom() - 1.0,
                            Stroke::new(2.0f32, TEXT()),
                        );
                    }
                    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        self.tab = t;
                    }
                }
            });
        });
        let r = ui.available_rect_before_wrap();
        ui.painter()
            .hline(r.x_range(), r.top(), Stroke::new(1.0f32, LINE()));
        ui.add_space(28.0);

        let c = match self.tab {
            Tab::Episodes => padded(ui, |ui| {
                if details != Details::Valo && item.seasons.len() > 1 {
                    // the Valo header has the season picker; the styled ones don't
                    let mut pick = self.season;
                    egui::ComboBox::from_id_salt("season_pick_styled")
                        .selected_text(item.seasons[pick].label())
                        .width(160.0)
                        .show_ui(ui, |ui| {
                            for (i, s) in item.seasons.iter().enumerate() {
                                ui.selectable_value(&mut pick, i, s.label());
                            }
                        });
                    self.season = pick;
                    ui.add_space(14.0);
                }
                if crate::ui::theme::spec().layout.episode_list {
                    self.episode_list(ui, item, images, progress)
                } else {
                    self.episodes_tab(ui, item, images, progress)
                }
            }),
            Tab::Related => padded(ui, |ui| {
                let more = similar(lib, item, 20, &self.shuffle);
                if more.is_empty() {
                    ui.label(RichText::new("Nothing similar in your library yet.").color(MUTED()));
                    return None;
                }
                ui.label(bold("You might also like", 20.0));
                ui.add_space(14.0);
                let mut c = None;
                ScrollArea::horizontal().id_salt("related").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        for i in more {
                            if wide_tile(ui, i, vec2(300.0, 170.0), images) {
                                c = Some(Click::Open(i.id.clone()));
                            }
                        }
                    });
                });
                c
            }),
            Tab::Details => padded(ui, |ui| self.series_details_tab(ui, item, images)),
        };
        click.or(c)
    }

    fn episodes_tab(
        &mut self,
        ui: &mut Ui,
        item: &Item,
        images: &mut Images,
        progress: &Progress,
    ) -> Option<Click> {
        let season = &item.seasons[self.season];
        let n = season.episodes.len();
        ui.label(
            RichText::new(format!("{n} episode{}", if n == 1 { "" } else { "s" }))
                .size(14.0)
                .color(MUTED()),
        );
        ui.add_space(14.0);
        let w = ui.available_width();
        let gap = 24.0;
        let cols = ((w + gap) / (300.0 + gap)).floor().clamp(1.0, 4.0);
        let cw = (w - gap * (cols - 1.0)) / cols;
        let mut click = None;
        for row in season.episodes.chunks(cols as usize) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for ep in row {
                    ui.vertical(|ui| {
                        ui.set_width(cw);
                        if let Some(c) = self.episode_card(ui, item, ep, cw, images, progress) {
                            click = Some(c);
                        }
                    });
                }
            });
            ui.add_space(26.0);
        }
        click
    }

    #[allow(clippy::too_many_arguments)]
    fn episode_card(
        &self,
        ui: &mut Ui,
        series: &Item,
        ep: &Episode,
        w: f32,
        images: &mut Images,
        progress: &Progress,
    ) -> Option<Click> {
        let mut click = None;
        let still_h = w * 9.0 / 16.0;
        let (r, resp) = ui.allocate_exact_size(vec2(w, still_h), Sense::click());
        let p = ui.painter();
        let art = ep.still.as_ref().or(series.backdrop.as_ref());
        match art.and_then(|u| images.get(u)) {
            Some(t) => image_cover(p, r, &t, 6, Color32::WHITE),
            None => placeholder(p, r, &ep.title, 6),
        }
        if let Some(e) = progress.get(&self.key(&ep.id)) {
            let bar = Rect::from_min_max(
                pos2(r.left() + 10.0, r.bottom() - 10.0),
                pos2(r.right() - 10.0, r.bottom() - 6.0),
            );
            p.rect_filled(bar, 2.0, Color32::from_white_alpha(70));
            let f = if e.watched() { 1.0 } else { e.fraction() };
            p.rect_filled(
                Rect::from_min_max(bar.min, pos2(bar.left() + bar.width() * f, bar.bottom())),
                2.0,
                GOLD(),
            );
        }
        let open = Click::Nav(View::Episode(series.id.clone(), ep.id.clone()));
        if resp.hovered() || resp.contains_pointer() {
            p.rect_stroke(
                r,
                6.0,
                Stroke::new(2.0f32, TEXT()),
                egui::StrokeKind::Outside,
            );
            // the round button plays; the rest of the card opens the episode
            let pr = Rect::from_center_size(r.center(), vec2(56.0, 56.0));
            let play = ui.interact(pr, resp.id.with("play"), Sense::click());
            let a = if play.hovered() { 230 } else { 170 };
            ui.painter()
                .circle_filled(pr.center(), 28.0, Color32::from_black_alpha(a));
            chrome::draw_icon(
                ui.painter(),
                pr.center() + vec2(2.0, 0.0),
                Icon::Play,
                TEXT(),
            );
            if play.on_hover_text("Play").clicked() {
                click = Some(Click::Play(series.id.clone(), Some(ep.id.clone()), false));
            }
        }
        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() && click.is_none() {
            click = Some(open.clone());
        }
        ui.add_space(10.0);
        let title = format!("{}. {}", ep.number, ep.title);
        let t = ui.add(
            egui::Label::new(bold(title, 15.0).color(TEXT()))
                .truncate()
                .sense(Sense::click()),
        );
        if t.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            click = Some(open);
        }
        if !ep.overview.is_empty() {
            ui.add_space(2.0);
            clamp_text(ui, &ep.overview, 12.5, MUTED(), 2);
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if let Some(r) = &series.rating {
                tag(ui, r);
            }
            if let Some(d) = ep.duration.filter(|d| *d > 0.0) {
                ui.label(
                    RichText::new(format!("{}min", (d / 60.0).round()))
                        .size(12.0)
                        .color(MUTED()),
                );
            }
            if let Some(a) = &ep.aired {
                ui.label(RichText::new(date_short(a)).size(12.0).color(MUTED()));
            }
        });
        click
    }

    fn series_details_tab(&self, ui: &mut Ui, item: &Item, images: &mut Images) -> Option<Click> {
        let mut click = None;
        let w = ui.available_width();
        let narrow = w < 900.0;
        let cw = if narrow {
            w
        } else {
            ((w - 24.0) / 2.0).min(440.0)
        };
        let mut about = |ui: &mut Ui| {
            panel(ui, |ui| {
                ui.label(bold(&item.title, 19.0));
                ui.add_space(6.0);
                if !item.genres.is_empty() {
                    ui.label(
                        RichText::new(item.genres.join(" · "))
                            .size(14.0)
                            .color(MUTED()),
                    );
                    ui.add_space(8.0);
                }
                ui.horizontal(|ui| {
                    if let Some(y) = years(item) {
                        ui.label(RichText::new(y).size(12.0).color(MUTED()));
                    }
                    let n = item.season_count();
                    ui.label(
                        RichText::new(format!("{n} season{}", if n == 1 { "" } else { "s" }))
                            .size(12.0)
                            .color(MUTED()),
                    );
                    if let Some(r) = &item.rating {
                        tag(ui, r);
                    }
                });
                if !item.overview.is_empty() {
                    ui.add_space(14.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(&item.overview)
                                .size(14.0)
                                .color(TEXT().gamma_multiply(0.85)),
                        )
                        .wrap(),
                    );
                }
            });
            let names: Vec<&str> = item.cast.iter().take(8).map(|c| c.name.as_str()).collect();
            if !item.creators.is_empty() || !names.is_empty() {
                ui.add_space(24.0);
                panel(ui, |ui| {
                    ui.label(bold("Creators and Cast", 17.0));
                    ui.add_space(16.0);
                    if !item.creators.is_empty() {
                        let creators: Vec<&str> =
                            item.creators.iter().map(String::as_str).collect();
                        if let Some(c) = info_people_row(ui, "Creators", &creators, 96.0) {
                            click = Some(c);
                        }
                    }
                    if !names.is_empty() {
                        info_rows(ui, &[("Cast", names.join(", "))], 96.0);
                    }
                });
            }
        };
        let eps: usize = item.seasons.iter().map(|s| s.episodes.len()).sum();
        let mut rows = Vec::new();
        if let Some(s) = self.discover.streaming_on(&item.id) {
            rows.push(("Streaming on", s));
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
        let info = |ui: &mut Ui| {
            panel(ui, |ui| {
                ui.label(bold("Info", 17.0));
                ui.add_space(16.0);
                info_rows(ui, &rows, 112.0);
            });
        };
        if narrow {
            about(ui);
            ui.add_space(24.0);
            info(ui);
        } else {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 24.0;
                ui.vertical(|ui| {
                    ui.set_width(cw);
                    about(ui);
                });
                ui.vertical(|ui| {
                    ui.set_width(cw);
                    info(ui);
                });
            });
        }
        if item.cast.is_empty() {
            return click;
        }
        ui.add_space(30.0);
        caps(ui, "Cast", TEAL());
        ui.add_space(4.0);
        ui.label(bold("Series cast", 22.0));
        ui.add_space(16.0);
        let n = item.cast.len().min(20);
        cast_grid(ui, &item.cast[..n], images, 150.0, 46.0).or(click)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn episode_page(
        &mut self,
        ui: &mut Ui,
        series: &Item,
        si: usize,
        ei: usize,
        images: &mut Images,
        progress: &Progress,
    ) -> Option<Click> {
        let mut click = None;
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

        let w = ui.available_width();
        let h = (w * 0.36).clamp(380.0, 580.0);
        let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        let art = ep.still.as_ref().or(series.backdrop.as_ref());
        let tex = art.and_then(|u| images.get(u));
        backdrop(&ui.painter_at(rect), rect, tex.as_ref(), &ep.title, 100);

        let back = Rect::from_min_size(rect.min + vec2(28.0, 20.0), vec2(400.0, 24.0));
        placed(ui, back, "ep_back", 0.0, |ui| {
            let r = link(
                ui,
                &series.title,
                13.0,
                TEXT().gamma_multiply(0.8),
                Some((Icon::ChevronLeft, false)),
            );
            if r.clicked() {
                click = Some(Click::Back);
            }
        });

        // big play button in the middle
        let pc = pos2(rect.center().x, rect.top() + h * 0.42);
        let pr = Rect::from_center_size(pc, vec2(84.0, 84.0));
        let play = ui.interact(pr, ui.id().with("ep_play"), Sense::click());
        let p = ui.painter();
        p.circle_filled(pc, 46.0, Color32::from_black_alpha(90));
        p.circle_filled(
            pc,
            41.0,
            if play.hovered() {
                shade(GOLD(), 1.08)
            } else {
                GOLD()
            },
        );
        p.add(egui::Shape::convex_polygon(
            vec![
                pc + vec2(-9.0, -13.0),
                pc + vec2(15.0, 0.0),
                pc + vec2(-9.0, 13.0),
            ],
            INK(),
            Stroke::NONE,
        ));
        if play.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            click = Some(Click::Play(series.id.clone(), Some(ep.id.clone()), false));
        }

        let (m, inner) = column(ui);
        let text_rect = Rect::from_min_max(
            pos2(rect.left() + m, rect.top() + h * 0.6),
            pos2(rect.left() + m + inner * 0.75, rect.bottom() - 26.0),
        );
        placed(ui, text_rect, "ep_hero", 1.0, |ui| {
            caps(
                ui,
                &match featurette(season.number, ep.number) {
                    Some(f) => format!("{}  ·  {f}", series.title),
                    None => format!(
                        "{}  ·  S{:02}  ·  E{:02}",
                        series.title, season.number, ep.number
                    ),
                },
                TEXT().gamma_multiply(0.7),
            );
            ui.add_space(8.0);
            ui.add(egui::Label::new(bold(&ep.title, 52.0).color(TEXT())).wrap());
        });
        if let Some((_, n)) = next {
            let nr = Rect::from_min_size(
                pos2(rect.left() + m + inner - 70.0, rect.bottom() - 62.0),
                vec2(70.0, 24.0),
            );
            placed(ui, nr, "ep_next", 0.0, |ui| {
                if link(
                    ui,
                    "Next",
                    13.0,
                    TEXT().gamma_multiply(0.8),
                    Some((Icon::ArrowRight, true)),
                )
                .on_hover_text(&n.title)
                .clicked()
                {
                    click = Some(Click::Nav(View::Episode(series.id.clone(), n.id.clone())));
                }
            });
        }

        ui.add_space(26.0);
        let (file, probed) = self.file_for(ui.ctx(), &ep.stream, ep.file.as_ref());
        let file = file.as_ref();
        let runtime = ep.duration.filter(|d| *d > 0.0).or(probed);
        padded(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                ui.label(
                    RichText::new(format!(
                        "{} of {}",
                        sxe(season.number, ep.number),
                        season.episodes.len()
                    ))
                    .size(14.0)
                    .color(GOLD()),
                );
                if let Some(d) = runtime {
                    dot(ui);
                    ui.label(
                        RichText::new(minutes(d))
                            .size(14.0)
                            .color(TEXT().gamma_multiply(0.85)),
                    );
                }
                if let Some(a) = &ep.aired {
                    dot(ui);
                    ui.label(
                        RichText::new(format!("Aired {}", date_long(a)))
                            .size(14.0)
                            .color(TEXT().gamma_multiply(0.85)),
                    );
                }
                if let Some(s) = ep.score {
                    dot(ui);
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    chrome::draw_icon(ui.painter(), r.center(), Icon::Star, GOLD());
                    ui.label(
                        RichText::new(format!("{s:.1}"))
                            .size(14.0)
                            .color(TEXT().gamma_multiply(0.85)),
                    );
                }
                if let Some(q) = file.and_then(|f| f.quality.as_deref()) {
                    dot(ui);
                    tag(ui, q);
                }
            });
        });
        ui.add_space(30.0);

        let (m, inner) = column(ui);
        let narrow = inner < NARROW;
        let gap = 56.0;
        let lw = if narrow { inner } else { inner * 0.64 };
        let rw = if narrow { inner } else { inner - lw - gap };
        let sp = ui.spacing().item_spacing.x;
        let side = |ui: &mut Ui, click: &mut Option<Click>, images: &mut Images| {
            let (label, _) = match progress.get(&self.key(&ep.id)) {
                Some(e) if e.in_progress() => (format!("Resume from {}", fmt_time(e.pos)), true),
                Some(e) if e.watched() => ("Watch Again".to_owned(), false),
                _ => ("Play Episode".to_owned(), false),
            };
            if fill_button(
                ui,
                &label,
                Some(Icon::Play),
                play_colors(),
                48.0,
                ui.available_width(),
                6.0,
            )
            .clicked()
            {
                *click = Some(Click::Play(series.id.clone(), Some(ep.id.clone()), false));
            }
            let upcoming: Vec<_> = (pos + 1..pos + 4).filter_map(ep_at).collect();
            if !upcoming.is_empty() {
                ui.add_space(26.0);
                caps(ui, "Up next", MUTED());
                ui.add_space(10.0);
                for (s, e) in upcoming {
                    let (r, resp) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::click());
                    let thumb = Rect::from_min_size(r.min, vec2(96.0, 54.0));
                    let p = &ui.painter_at(r);
                    match e
                        .still
                        .as_ref()
                        .or(series.backdrop.as_ref())
                        .and_then(|u| images.get(u))
                    {
                        Some(t) => image_cover(p, thumb, &t, 4, Color32::WHITE),
                        None => placeholder(p, thumb, &e.title, 4),
                    }
                    p.text(
                        pos2(thumb.right() + 14.0, thumb.top() + 8.0),
                        Align2::LEFT_TOP,
                        sxe(s.number, e.number),
                        FontId::proportional(11.0),
                        MUTED(),
                    );
                    p.text(
                        pos2(thumb.right() + 14.0, thumb.top() + 24.0),
                        Align2::LEFT_TOP,
                        &e.title,
                        FontId::proportional(14.5),
                        if resp.hovered() {
                            TEXT()
                        } else {
                            TEXT().gamma_multiply(0.9)
                        },
                    );
                    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        *click = Some(Click::Nav(View::Episode(series.id.clone(), e.id.clone())));
                    }
                    ui.add_space(14.0);
                }
            }
            ui.add_space(20.0);
            panel(ui, |ui| {
                caps(ui, "About the series", MUTED());
                if !series.overview.is_empty() {
                    ui.add_space(10.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(&series.overview)
                                .size(14.0)
                                .color(TEXT().gamma_multiply(0.8)),
                        )
                        .wrap(),
                    );
                }
                ui.add_space(14.0);
                if link(
                    ui,
                    "View all episodes",
                    12.5,
                    GOLD(),
                    Some((Icon::ArrowRight, true)),
                )
                .clicked()
                {
                    *click = Some(Click::Nav(View::Details(series.id.clone())));
                }
            });
        };
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(m);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.x = sp;
                ui.set_width(lw);
                if !ep.overview.is_empty() {
                    caps(ui, "Synopsis", MUTED());
                    ui.add_space(6.0);
                    ui.label(bold(&ep.title, 30.0));
                    ui.add_space(12.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(&ep.overview)
                                .size(16.0)
                                .color(TEXT().gamma_multiply(0.8)),
                        )
                        .wrap(),
                    );
                    ui.add_space(30.0);
                }
                caps(ui, "File", MUTED());
                ui.add_space(10.0);
                panel(ui, |ui| {
                    let mut cells = Vec::new();
                    if let Some(n) = file_name(file, &ep.stream) {
                        cells.push(("Filename", n, TEXT(), 12.5));
                    }
                    if let Some(q) = file.and_then(|f| f.quality.clone()) {
                        cells.push(("Resolution", q, TEAL(), 14.0));
                    }
                    cells.push(("Episode", sxe(season.number, ep.number), TEXT(), 14.0));
                    if let Some(d) = runtime {
                        cells.push(("Runtime", minutes(d), TEXT(), 14.0));
                    }
                    if let Some(s) = file.and_then(|f| f.size) {
                        cells.push(("File size", bytes(s), TEXT(), 14.0));
                    }
                    let half = ui.available_width() / 2.0 - 20.0;
                    if half < 220.0 {
                        kv_rows(ui, &cells);
                    } else {
                        let (mut l, mut r) = (Vec::new(), Vec::new());
                        for (i, c) in cells.into_iter().enumerate() {
                            if i.is_multiple_of(2) {
                                l.push(c)
                            } else {
                                r.push(c)
                            }
                        }
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 40.0;
                            ui.vertical(|ui| {
                                ui.set_width(half);
                                kv_rows(ui, &l);
                            });
                            ui.vertical(|ui| {
                                ui.set_width(half);
                                kv_rows(ui, &r);
                            });
                        });
                    }
                });
                if !series.cast.is_empty() {
                    ui.add_space(30.0);
                    caps(ui, "Series cast", MUTED());
                    ui.add_space(12.0);
                    let n = series.cast.len().min(10);
                    if let Some(c) = cast_grid(ui, &series.cast[..n], images, 160.0, 40.0) {
                        click = Some(c);
                    }
                }
                if narrow {
                    ui.add_space(20.0);
                    side(ui, &mut click, images);
                }
            });
            if !narrow {
                ui.add_space(gap);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.x = sp;
                    ui.set_width(rw);
                    side(ui, &mut click, images);
                });
            }
        });
        click
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(date_long("2002-10-31"), "31 Oct 2002");
        assert_eq!(date_short("2021-11-06"), "Nov 6, 2021");
        assert_eq!(date_long("soon"), "soon");
        assert_eq!(date_unix(0), "1 Jan 1970");
        assert_eq!(date_unix(1_792_101_466), "15 Oct 2026");
        assert_eq!(date_unix(951_782_400), "29 Feb 2000");
        assert_eq!(money(8_000_000), "$8,000,000");
        assert_eq!(money(82_784_517), "$82,784,517");
        assert_eq!(money(999), "$999");
        assert_eq!(bytes(2_090_000_000), "2.09 GB");
        assert_eq!(bitrate(2_600_000), "2.6 Mb/s");
        assert_eq!(
            file_name(
                None,
                "http://h/media/Arcane/Arcane.S01E01.1080p%20x.mp4?token=1"
            )
            .as_deref(),
            Some("Arcane.S01E01.1080p x.mp4")
        );
        assert_eq!(file_name(None, "http://h/api/stream/movie/42"), None);
    }
}
