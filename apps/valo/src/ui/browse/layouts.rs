//! The home page and navigation pieces of the app styles: full-width and
//! carousel banners, the Top 10 row, genre tiles, the video-style grid and
//! the sidebar.
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CursorIcon, FontId, Layout, Rect, RichText, Sense,
    Stroke, Ui, UiBuilder,
};

use super::details::title_art;
use super::{grid_cards, meta_line, shorten, shuffled, Browse, Click, View, POSTER, SIDE};
use crate::api::{Item, Kind, Library};
use crate::config::Progress;
use crate::images::Images;
use crate::ui::chrome::{self, Icon};
use crate::ui::theme::{self, Skin};
use crate::ui::{
    bold, gradient, heading, image_cover, primary_button_icon, secondary_button, ACCENT, BG, MUTED,
    SURFACE, SURFACE_HI, TEXT,
};

/// `c` at `alpha` (0–1).
fn alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a * 255.0) as u8)
}

/// The dots under a banner and, while hovered, the arrows at its sides.
/// Returns a step (-1/+1) when an arrow is clicked.
fn carousel_marks(
    ui: &mut Ui,
    area: Rect,
    dots_y: f32,
    cur: usize,
    n: usize,
    hovered: bool,
) -> Option<isize> {
    if n < 2 {
        return None;
    }
    let p = ui.painter();
    let x0 = area.center().x - (n as f32 * 16.0) / 2.0;
    for i in 0..n {
        let c = pos2(x0 + i as f32 * 16.0 + 8.0, dots_y);
        if i == cur {
            p.rect_filled(Rect::from_center_size(c, vec2(14.0, 4.0)), 2.0, TEXT());
        } else {
            p.circle_filled(c, 2.5, MUTED().gamma_multiply(0.7));
        }
    }
    let mut step = None;
    if hovered {
        for (dir, x) in [(-1isize, area.left() + 20.0), (1, area.right() - 20.0)] {
            let r = Rect::from_center_size(pos2(x, area.center().y), vec2(36.0, 64.0));
            let a = ui.interact(r, ui.id().with(("hero_arrow", dir)), Sense::click());
            let p = ui.painter();
            p.rect_filled(
                r,
                6.0,
                Color32::from_black_alpha(if a.hovered() { 170 } else { 90 }),
            );
            p.text(
                r.center(),
                Align2::CENTER_CENTER,
                if dir < 0 { "‹" } else { "›" },
                FontId::proportional(32.0),
                Color32::WHITE,
            );
            if a.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                step = Some(dir);
            }
        }
    }
    step
}

impl Browse {
    /// Netflix, HBO Max and Apple TV: the artwork across the whole width,
    /// title, details and buttons over its bottom left.
    pub(super) fn hero_full(
        &mut self,
        ui: &mut Ui,
        item: &Item,
        images: &mut Images,
        progress: &Progress,
        carousel: (usize, usize),
    ) -> (Option<Click>, Option<isize>) {
        let skin = theme::skin();
        let w = ui.available_width();
        let h = (w * if skin == Skin::Apple { 0.52 } else { 0.46 }).clamp(380.0, 780.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        // opens from anywhere on the banner, like More info
        let hit = ui.interact(rect, ui.id().with("banner"), Sense::click());
        let p = ui.painter_at(rect);
        let art = item.backdrop.as_ref().or(item.poster.as_ref());
        match art.and_then(|u| images.get(u)) {
            Some(t) => image_cover(&p, rect, &t, 0, Color32::WHITE),
            None => {
                let c = crate::util::color_for(&item.title);
                gradient(
                    &p,
                    rect,
                    [c.gamma_multiply(0.6), c, c.gamma_multiply(0.4), BG()],
                );
            }
        }
        // shade the left and the bottom so the text reads, fading into the page
        let left = Rect::from_min_max(rect.min, pos2(rect.left() + w * 0.62, rect.bottom()));
        let bg = BG();
        gradient(
            &p,
            left,
            [
                alpha(bg, 0.85),
                Color32::TRANSPARENT,
                Color32::TRANSPARENT,
                alpha(bg, 0.85),
            ],
        );
        let low = Rect::from_min_max(pos2(rect.left(), rect.bottom() - h * 0.4), rect.max);
        gradient(
            &p,
            low,
            [Color32::TRANSPARENT, Color32::TRANSPARENT, bg, bg],
        );

        let mut click = None;
        let text_w = (w * 0.42).clamp(320.0, 600.0).min(w - 2.0 * SIDE);
        let x = rect.left()
            + if skin == Skin::Netflix {
                56.0
            } else {
                SIDE + 8.0
            };
        let inner = Rect::from_min_size(pos2(x, rect.top() + 40.0), vec2(text_w, h - 40.0 - 64.0));
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(inner)
                .layout(Layout::bottom_up(Align::Min)),
        );
        let ui2 = &mut child;
        ui2.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let (label, restart) = self.play_label(item, progress);
            if primary_button_icon(ui, Icon::Play, &label, true).clicked() {
                click = Some(Click::Play(item.id.clone(), None, false));
            }
            if restart && skin != Skin::Netflix && secondary_button(ui, "Start over").clicked() {
                click = Some(Click::Play(item.id.clone(), None, true));
            }
            if secondary_button(ui, "More info").clicked() {
                click = Some(Click::Open(item.id.clone()));
            }
        });
        ui2.add_space(14.0);
        if !item.overview.is_empty() && skin != Skin::Apple {
            ui2.add(
                egui::Label::new(
                    RichText::new(shorten(&item.overview, 240))
                        .size(16.0)
                        .color(TEXT()),
                )
                .wrap(),
            );
            ui2.add_space(10.0);
        }
        ui2.label(
            RichText::new(meta_line(item))
                .size(14.0)
                .color(TEXT().gamma_multiply(0.75)),
        );
        ui2.add_space(8.0);
        title_art(
            ui2,
            item,
            images,
            vec2(text_w.min(520.0), 140.0),
            if skin == Skin::Apple { 58.0 } else { 52.0 },
        );
        let label = match skin {
            Skin::Netflix => Some((
                if item.kind == Kind::Movie {
                    "F I L M"
                } else {
                    "S E R I E S"
                },
                ACCENT(),
            )),
            Skin::Apple => Some(("Featured", MUTED())),
            _ => None,
        };
        if let Some((l, c)) = label {
            ui2.add_space(6.0);
            ui2.label(bold(l, 13.0).color(c));
        }

        let step = carousel_marks(
            ui,
            rect,
            rect.bottom() - 26.0,
            carousel.0,
            carousel.1,
            resp.hovered() || resp.contains_pointer(),
        );
        if click.is_none() && hit.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            click = Some(Click::Open(item.id.clone()));
        }
        (click, step)
    }

    /// Prime Video and Disney+: a rounded banner with the ones before and
    /// after peeking in at the sides.
    pub(super) fn hero_carousel(
        &mut self,
        ui: &mut Ui,
        featured: &[&Item],
        idx: usize,
        images: &mut Images,
        progress: &Progress,
    ) -> (Option<Click>, Option<isize>) {
        let skin = theme::skin();
        let rad = theme::spec().card_radius.max(6);
        let n = featured.len();
        let item = featured[idx];
        let w = ui.available_width();
        let m = SIDE + 20.0;
        let bw = w - 2.0 * m;
        let bh = (bw * 0.36).clamp(240.0, 500.0);
        ui.add_space(16.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, bh + 34.0), Sense::hover());
        let banner = Rect::from_min_size(pos2(rect.left() + m, rect.top()), vec2(bw, bh));
        let p = ui.painter_at(rect);
        let paint =
            |p: &egui::Painter, r: Rect, it: &Item, images: &mut Images, tint: Color32| match it
                .backdrop
                .as_ref()
                .or(it.poster.as_ref())
                .and_then(|u| images.get(u))
            {
                Some(t) => image_cover(p, r, &t, rad, tint),
                None => {
                    p.rect_filled(
                        r,
                        rad,
                        crate::util::color_for(&it.title).gamma_multiply(tint.r() as f32 / 255.0),
                    );
                }
            };
        let mut step = None;
        if n > 1 {
            let gap = 14.0;
            let side = [
                (
                    -1isize,
                    banner.translate(vec2(-(bw + gap), 0.0)),
                    featured[(idx + n - 1) % n],
                ),
                (
                    1,
                    banner.translate(vec2(bw + gap, 0.0)),
                    featured[(idx + 1) % n],
                ),
            ];
            for (dir, r, it) in side {
                paint(&p, r, it, images, Color32::from_gray(110));
                let vis = r.intersect(rect);
                let a = ui.interact(vis, ui.id().with(("peek", dir)), Sense::click());
                if a.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    step = Some(dir);
                }
            }
        }
        paint(&p, banner, item, images, Color32::WHITE);
        let bp = p.with_clip_rect(banner);
        let left = Rect::from_min_max(banner.min, pos2(banner.left() + bw * 0.6, banner.bottom()));
        let dark = Color32::from_black_alpha(200);
        gradient(
            &bp,
            left,
            [dark, Color32::TRANSPARENT, Color32::TRANSPARENT, dark],
        );

        let mut click = None;
        let hit = ui.interact(banner, ui.id().with("banner"), Sense::click());
        if skin == Skin::Disney && (hit.hovered() || hit.contains_pointer()) {
            p.rect_stroke(
                banner,
                rad,
                Stroke::new(3.0f32, Color32::WHITE),
                egui::StrokeKind::Outside,
            );
        }
        let inner = Rect::from_min_max(
            pos2(banner.left() + 36.0, banner.top() + 24.0),
            pos2(
                banner.left() + (bw * 0.5).max(320.0),
                banner.bottom() - 28.0,
            ),
        );
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(inner)
                .layout(Layout::bottom_up(Align::Min)),
        );
        let ui2 = &mut child;
        // the text sits on the darkened artwork: always light
        ui2.visuals_mut().override_text_color = Some(Color32::WHITE);
        if skin == Skin::Prime {
            ui2.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let (label, _) = self.play_label(item, progress);
                if primary_button_icon(ui, Icon::Play, &label, true).clicked() {
                    click = Some(Click::Play(item.id.clone(), None, false));
                }
            });
            ui2.add_space(14.0);
        }
        ui2.label(
            RichText::new(meta_line(item))
                .size(14.0)
                .color(Color32::from_white_alpha(200)),
        );
        ui2.add_space(6.0);
        if let Some(t) = item.logo.as_ref().and_then(|u| images.get(u)) {
            let sz = t.size_vec2();
            let s = (360.0 / sz.x).min(110.0 / sz.y);
            let (r, _) = ui2.allocate_exact_size(sz * s, Sense::hover());
            ui2.painter().image(
                t.id(),
                r,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            ui2.add(
                egui::Label::new(
                    RichText::new(&item.title)
                        .font(crate::ui::black_font(40.0))
                        .color(Color32::WHITE),
                )
                .wrap(),
            );
        }
        if click.is_none() && hit.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            click = Some(Click::Open(item.id.clone()));
        }

        let marks = carousel_marks(ui, banner, banner.bottom() + 20.0, idx, n, false);
        let _ = resp;
        (click, step.or(marks))
    }

    /// Netflix's "Top 10": big outlined numbers beside the posters.
    pub(super) fn top10_row(
        &mut self,
        ui: &mut Ui,
        items: &[&Item],
        images: &mut Images,
    ) -> Option<Click> {
        let mut click = None;
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(SIDE);
            ui.label(heading("Top 10 in your library"));
        });
        ui.add_space(6.0);
        egui::ScrollArea::horizontal()
            .id_salt("top10")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.add_space(SIDE);
                    for (i, item) in items.iter().take(10).enumerate() {
                        let num_w = if i == 9 { 150.0 } else { 100.0 };
                        let (r, resp) = ui
                            .allocate_exact_size(vec2(num_w + POSTER.x, POSTER.y), Sense::click());
                        if !ui.is_rect_visible(r) {
                            continue;
                        }
                        let hot = resp.hovered();
                        let p = ui.painter();
                        let num = (i + 1).to_string();
                        let font = crate::ui::black_font(190.0);
                        let at = pos2(r.left() + num_w + 18.0, r.bottom() + 26.0);
                        // an outlined numeral: the outline, then the page color inside
                        for (dx, dy) in [
                            (-2.5, 0.0),
                            (2.5, 0.0),
                            (0.0, -2.5),
                            (0.0, 2.5),
                            (-1.8, -1.8),
                            (1.8, 1.8),
                            (-1.8, 1.8),
                            (1.8, -1.8),
                        ] {
                            p.text(
                                at + vec2(dx, dy),
                                Align2::RIGHT_BOTTOM,
                                &num,
                                font.clone(),
                                Color32::from_gray(150),
                            );
                        }
                        p.text(at, Align2::RIGHT_BOTTOM, &num, font, BG());
                        let pr = Rect::from_min_size(pos2(r.left() + num_w, r.top()), POSTER)
                            .expand(if hot { 4.0 } else { 0.0 });
                        match item.poster.as_ref().and_then(|u| images.get(u)) {
                            Some(t) => image_cover(p, pr, &t, 4, Color32::WHITE),
                            None => crate::ui::placeholder(p, pr, &item.title, 4),
                        }
                        if resp
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text_at_pointer(&item.title)
                            .clicked()
                        {
                            click = Some(Click::Open(item.id.clone()));
                        }
                    }
                    ui.add_space(SIDE);
                });
            });
        click
    }

    /// Disney+'s row of tiles under the banner, here one per genre.
    pub(super) fn genre_tiles(&mut self, ui: &mut Ui, lib: &Library) -> Option<Click> {
        let genres: Vec<String> = lib.genres().into_iter().take(5).collect();
        if genres.len() < 3 {
            return None;
        }
        let mut click = None;
        let n = genres.len() as f32;
        let gap = 18.0;
        let tw = (ui.available_width() - 2.0 * SIDE - gap * (n - 1.0)) / n;
        let th = (tw * 0.56).clamp(70.0, 150.0);
        ui.add_space(22.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.add_space(SIDE);
            for g in &genres {
                let (r, resp) = ui.allocate_exact_size(vec2(tw, th), Sense::click());
                let hot = resp.hovered();
                let t = ui.ctx().animate_bool_with_time(resp.id, hot, 0.12);
                let r = r.expand(t * 5.0);
                let p = ui.painter();
                let c = crate::util::color_for(g);
                let mut m = egui::Mesh::default();
                for (pos, col) in [
                    (r.left_top(), c.gamma_multiply(0.55)),
                    (r.right_top(), c.gamma_multiply(0.85)),
                    (r.right_bottom(), c),
                    (r.left_bottom(), c.gamma_multiply(0.4)),
                ] {
                    m.colored_vertex(pos, col);
                }
                m.add_triangle(0, 1, 2);
                m.add_triangle(0, 2, 3);
                p.rect_filled(r, 10.0, c.gamma_multiply(0.5));
                p.add(egui::Shape::mesh(m));
                p.rect_stroke(
                    r,
                    10.0,
                    if hot {
                        Stroke::new(3.0f32, Color32::WHITE)
                    } else {
                        Stroke::new(2.0f32, Color32::from_white_alpha(40))
                    },
                    egui::StrokeKind::Inside,
                );
                p.text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    g.to_uppercase(),
                    FontId::new(17.0, egui::FontFamily::Name("black".into())),
                    Color32::WHITE,
                );
                if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    let movies = lib.movies.iter().any(|i| i.genres.contains(g));
                    self.genre = Some(g.clone());
                    click = Some(Click::Nav(if movies { View::Movies } else { View::Series }));
                }
            }
        });
        click
    }

    /// YouTube: filter chips over one big grid of videos.
    pub(super) fn home_grid(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        images: &mut Images,
        progress: &Progress,
    ) -> Option<Click> {
        let mut click = None;
        let genres: Vec<String> = lib.genres().into_iter().take(8).collect();
        let mut chips: Vec<(usize, &str, Option<usize>)> =
            vec![(0, "All", None), (1, "Movies", None), (2, "TV shows", None)];
        for (i, g) in genres.iter().enumerate() {
            chips.push((3 + i, g.as_str(), None));
        }
        if self.chip >= chips.len() {
            self.chip = 0;
        }
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(SIDE - 22.0);
            crate::ui::segmented(ui, &mut self.chip, &chips);
        });
        ui.add_space(18.0);
        let marked = |b: &Browse, id: &str| b.watchlist.contains(&b.key(id));
        if self.chip == 0 {
            let cont = self.continue_cards(lib, progress);
            if !cont.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(SIDE);
                    ui.label(heading("Continue watching"));
                });
                ui.add_space(10.0);
                let m = |id: &str| marked(self, id);
                click = click.or(grid_cards(ui, &cont, true, &m, images));
                ui.add_space(8.0);
            }
        }
        let items: Vec<&Item> = match self.chip {
            0 => shuffled(lib.all(), "grid", &self.shuffle),
            1 => shuffled(lib.movies.iter(), "grid", &self.shuffle),
            2 => shuffled(lib.series.iter(), "grid", &self.shuffle),
            n => {
                let g = &genres[n - 3];
                shuffled(
                    lib.all().filter(|i| i.genres.contains(g)),
                    "grid",
                    &self.shuffle,
                )
            }
        };
        let cards = self.poster_cards(items.into_iter(), progress);
        let m = |id: &str| marked(self, id);
        click.or(grid_cards(ui, &cards, false, &m, images))
    }

    /// YouTube and Apple TV: Home, Movies and TV shows down the left, with
    /// My list under them.
    pub(super) fn nav_sidebar(&mut self, ctx: &egui::Context, images: &mut Images) {
        let skin = theme::skin();
        let fill = if skin == Skin::Apple { SURFACE() } else { BG() };
        let mut go = None;
        egui::SidePanel::left("nav_side")
            .exact_width(216.0)
            .resizable(false)
            .show_separator_line(skin == Skin::Apple)
            .frame(
                egui::Frame::new()
                    .fill(fill)
                    .inner_margin(egui::Margin::symmetric(12, 14)),
            )
            .show(ctx, |ui| {
                let searching = !self.search.is_empty();
                let item = |ui: &mut Ui, icon: Icon, label: &str, selected: bool| -> bool {
                    let (r, resp) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
                    let p = ui.painter();
                    let rad = if skin == Skin::Apple { 8.0 } else { 10.0 };
                    if selected {
                        p.rect_filled(r, rad, SURFACE_HI());
                    } else if resp.hovered() {
                        p.rect_filled(r, rad, SURFACE_HI().gamma_multiply(0.6));
                    }
                    let col = if skin == Skin::Apple && selected {
                        ACCENT()
                    } else {
                        TEXT()
                    };
                    chrome::draw_icon(p, pos2(r.left() + 22.0, r.center().y), icon, col);
                    let font = if selected {
                        FontId::new(14.5, egui::FontFamily::Name("bold".into()))
                    } else {
                        FontId::proportional(14.5)
                    };
                    p.text(
                        pos2(r.left() + 50.0, r.center().y),
                        Align2::LEFT_CENTER,
                        label,
                        font,
                        TEXT(),
                    );
                    resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
                };
                let mut nav = vec![
                    (Icon::Home, "Home", View::Home),
                    (Icon::Film, "Movies", View::Movies),
                    (Icon::Tv, "TV Shows", View::Series),
                ];
                if !self.discover.shown.is_empty() {
                    nav.push((Icon::List, "Streaming", View::Streaming));
                }
                for (icon, label, v) in nav {
                    if item(ui, icon, label, !searching && self.view == v) {
                        go = Some(Click::Nav(v));
                    }
                }
                let Some(lib) = self.lib.clone() else { return };
                let prefix = self.prefix();
                let mine: Vec<&Item> = self
                    .watchlist
                    .ids(&prefix)
                    .filter_map(|id| lib.find(id))
                    .take(12)
                    .collect();
                if mine.is_empty() {
                    return;
                }
                ui.add_space(10.0);
                let r = ui.available_rect_before_wrap();
                ui.painter()
                    .hline(r.x_range(), r.top(), Stroke::new(1.0f32, crate::ui::LINE()));
                ui.add_space(12.0);
                ui.label(bold("My list", 15.0));
                ui.add_space(4.0);
                for i in mine {
                    let (r, resp) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
                    let p = ui.painter();
                    if resp.hovered() {
                        p.rect_filled(r, 8.0, SURFACE_HI().gamma_multiply(0.6));
                    }
                    let th = Rect::from_center_size(
                        pos2(r.left() + 22.0, r.center().y),
                        vec2(36.0, 22.0),
                    );
                    match i
                        .thumb
                        .as_ref()
                        .or(i.backdrop.as_ref())
                        .or(i.poster.as_ref())
                        .and_then(|u| images.get(u))
                    {
                        Some(t) => image_cover(p, th, &t, 4, Color32::WHITE),
                        None => {
                            p.rect_filled(th, 4.0, crate::util::color_for(&i.title));
                        }
                    }
                    p.with_clip_rect(r).text(
                        pos2(r.left() + 50.0, r.center().y),
                        Align2::LEFT_CENTER,
                        &i.title,
                        FontId::proportional(13.5),
                        TEXT(),
                    );
                    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        go = Some(Click::Open(i.id.clone()));
                    }
                }
            });
        match go {
            Some(Click::Nav(v)) => {
                self.go(v);
                self.search.clear();
            }
            Some(Click::Open(id)) => self.go(View::Details(id)),
            _ => {}
        }
    }
}
