//! The movie and series pages of the app styles: a full-width header
//! (Netflix, Disney+, HBO Max, Apple TV), a split one (Prime Video), a
//! numbered episode list, and YouTube's watch page.
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, FontFamily, FontId, Rect, RichText, Sense,
    Stroke, Ui,
};

use super::super::{fmt_runtime, Browse, Click, SIDE};
use super::{
    clamp_text, fill_button, minutes, people_links, placed, play_colors, similar, tag, title_art,
    years, GOLD,
};
use crate::api::{FileInfo, Item, Kind, Library};
use crate::config::Progress;
use crate::images::Images;
use crate::ui::chrome::{self, Icon};
use crate::ui::theme::{self, Details};
use crate::ui::{
    bold, gradient, image_cover, placeholder, ACCENT, BG, INK, LINE, MUTED, PRIMARY, SURFACE_HI,
    TEXT,
};

fn alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a * 255.0) as u8)
}

fn bold_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

impl Browse {
    /// The top of a movie or series page for the Hero and Split layouts:
    /// artwork, title, details, buttons and the story.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn styled_header(
        &mut self,
        ui: &mut Ui,
        item: &Item,
        images: &mut Images,
        progress: &Progress,
        file: Option<&FileInfo>,
        runtime: Option<f64>,
    ) -> Option<Click> {
        let mut click = None;
        let split = theme::spec().layout.details == Details::Split;
        let w = ui.available_width();
        let h = if split {
            (w * 0.42).clamp(460.0, 660.0)
        } else {
            (w * 0.5).clamp(480.0, 780.0)
        };
        let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        let p = ui.painter_at(rect);
        let tex = item
            .backdrop
            .as_ref()
            .or(item.poster.as_ref())
            .and_then(|u| images.get(u));
        let bg = BG();
        if split {
            // Prime Video: the picture on the right, fading into the page
            let ar = Rect::from_min_max(pos2(rect.left() + w * 0.36, rect.top()), rect.max);
            match &tex {
                Some(t) => image_cover(&p, ar, t, 0, Color32::WHITE),
                None => {
                    p.rect_filled(ar, 0.0, crate::util::color_for(&item.title));
                }
            }
            let fade = Rect::from_min_max(ar.min, pos2(ar.left() + ar.width() * 0.5, ar.bottom()));
            gradient(
                &p,
                fade,
                [bg, Color32::TRANSPARENT, Color32::TRANSPARENT, bg],
            );
        } else {
            match &tex {
                Some(t) => image_cover(&p, rect, t, 0, Color32::WHITE),
                None => {
                    p.rect_filled(rect, 0.0, crate::util::color_for(&item.title));
                }
            }
            let left = Rect::from_min_max(rect.min, pos2(rect.left() + w * 0.6, rect.bottom()));
            gradient(
                &p,
                left,
                [
                    alpha(bg, 0.88),
                    Color32::TRANSPARENT,
                    Color32::TRANSPARENT,
                    alpha(bg, 0.88),
                ],
            );
        }
        let low = Rect::from_min_max(pos2(rect.left(), rect.bottom() - h * 0.35), rect.max);
        gradient(
            &p,
            low,
            [Color32::TRANSPARENT, Color32::TRANSPARENT, bg, bg],
        );

        let back = Rect::from_min_size(rect.min + vec2(SIDE - 8.0, 20.0), vec2(300.0, 24.0));
        placed(ui, back, "styled_back", 0.0, |ui| {
            let r = super::link(
                ui,
                "Back",
                13.0,
                TEXT().gamma_multiply(0.85),
                Some((Icon::ChevronLeft, false)),
            );
            if r.clicked() {
                click = Some(Click::Back);
            }
        });

        let tw = if split {
            w * 0.42
        } else {
            (w * 0.45).clamp(360.0, 640.0)
        };
        let text_rect = Rect::from_min_max(
            pos2(rect.left() + SIDE + 8.0, rect.top() + 56.0),
            pos2(rect.left() + SIDE + 8.0 + tw, rect.bottom() - 28.0),
        );
        placed(
            ui,
            text_rect,
            "styled_hero",
            if split { 0.5 } else { 1.0 },
            |ui| {
                title_art(
                    ui,
                    item,
                    images,
                    vec2(tw.min(520.0), 130.0),
                    if split { 44.0 } else { 52.0 },
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    if let Some(s) = item.score {
                        let src = item.score_source.as_deref().unwrap_or("Score");
                        ui.label(bold(format!("{src} {s:.1}"), 15.0).color(if split {
                            ACCENT()
                        } else {
                            GOLD()
                        }));
                    }
                    let mut bits: Vec<String> = Vec::new();
                    match item.kind {
                        Kind::Movie => {
                            bits.extend(item.year.map(|y| y.to_string()));
                            bits.extend(runtime.map(fmt_runtime));
                        }
                        Kind::Series => {
                            bits.extend(years(item));
                            let n = item.season_count();
                            bits.push(format!("{n} season{}", if n == 1 { "" } else { "s" }));
                        }
                    }
                    for b in bits {
                        ui.label(
                            RichText::new(b)
                                .size(15.0)
                                .color(TEXT().gamma_multiply(0.85)),
                        );
                    }
                    if let Some(r) = &item.rating {
                        tag(ui, r);
                    }
                    if let Some(q) = file.and_then(|f| f.quality.as_deref()) {
                        tag(ui, q);
                    }
                });
                if !item.genres.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(item.genres.join(" · "))
                            .size(14.0)
                            .color(MUTED()),
                    );
                }
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    let (label, resume) = self.play_label(item, progress);
                    let (bh, min_w) = if split { (52.0, 220.0) } else { (46.0, 150.0) };
                    if fill_button(ui, &label, Some(Icon::Play), play_colors(), bh, min_w, 0.0)
                        .clicked()
                    {
                        click = Some(Click::Play(item.id.clone(), None, false));
                    }
                    if resume
                        && item.kind == Kind::Movie
                        && fill_button(ui, "Start over", None, (SURFACE_HI(), TEXT()), bh, 0.0, 0.0)
                            .clicked()
                    {
                        click = Some(Click::Play(item.id.clone(), None, true));
                    }
                    self.bookmark(ui, item);
                });
                if !item.overview.is_empty() {
                    ui.add_space(18.0);
                    clamp_text(
                        ui,
                        &item.overview,
                        16.0,
                        TEXT().gamma_multiply(0.9),
                        if split { 5 } else { 3 },
                    );
                }
                let (label, names): (&str, Vec<&str>) = match item.kind {
                    Kind::Movie => ("Director", super::super::makers(item).collect()),
                    Kind::Series => (
                        "Created by",
                        item.creators.iter().map(String::as_str).collect(),
                    ),
                };
                if !names.is_empty() {
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).size(14.0).color(MUTED()));
                        if let Some(c) = people_links(ui, &names, 14.0) {
                            click = Some(c);
                        }
                    });
                }
            },
        );
        click
    }

    /// Episodes as a numbered list: number, picture, title and length, story.
    pub(super) fn episode_list(
        &mut self,
        ui: &mut Ui,
        item: &Item,
        images: &mut Images,
        progress: &Progress,
    ) -> Option<Click> {
        let mut click = None;
        let season = &item.seasons[self.season];
        let w = ui.available_width();
        let (tw, th) = (220.0, 124.0);
        for ep in &season.episodes {
            let (r, resp) = ui.allocate_exact_size(vec2(w, th + 28.0), Sense::click());
            if !ui.is_rect_visible(r) {
                continue;
            }
            let hot = resp.hovered();
            let p = ui.painter();
            if hot {
                p.rect_filled(r, 6.0, SURFACE_HI().gamma_multiply(0.7));
            }
            p.hline(r.x_range(), r.bottom(), Stroke::new(1.0f32, LINE()));
            p.text(
                pos2(r.left() + 30.0, r.center().y),
                Align2::CENTER_CENTER,
                ep.number.to_string(),
                FontId::proportional(24.0),
                MUTED(),
            );
            let thumb = Rect::from_min_size(pos2(r.left() + 60.0, r.top() + 14.0), vec2(tw, th));
            match ep
                .still
                .as_ref()
                .or(item.backdrop.as_ref())
                .and_then(|u| images.get(u))
            {
                Some(t) => image_cover(p, thumb, &t, 4, Color32::WHITE),
                None => placeholder(p, thumb, &ep.title, 4),
            }
            if let Some(e) = progress.get(&self.key(&ep.id)) {
                let bar = Rect::from_min_max(pos2(thumb.left(), thumb.bottom() - 4.0), thumb.max);
                p.rect_filled(bar, 0.0, Color32::from_white_alpha(70));
                p.rect_filled(
                    Rect::from_min_max(
                        bar.min,
                        pos2(bar.left() + bar.width() * e.fraction(), bar.bottom()),
                    ),
                    0.0,
                    theme::played(),
                );
            }
            if hot {
                p.circle_filled(thumb.center(), 22.0, Color32::from_black_alpha(150));
                chrome::draw_icon(
                    p,
                    thumb.center() + vec2(1.5, 0.0),
                    Icon::Play,
                    Color32::WHITE,
                );
            }
            let x = thumb.right() + 20.0;
            let right = r.right() - 16.0;
            p.text(
                pos2(x, thumb.top() + 4.0),
                Align2::LEFT_TOP,
                &ep.title,
                bold_font(16.0),
                TEXT(),
            );
            if let Some(d) = ep.duration.filter(|d| *d > 0.0) {
                p.text(
                    pos2(right, thumb.top() + 4.0),
                    Align2::RIGHT_TOP,
                    minutes(d),
                    FontId::proportional(15.0),
                    TEXT(),
                );
            }
            if !ep.overview.is_empty() {
                let mut job = egui::text::LayoutJob::simple(
                    ep.overview.clone(),
                    FontId::proportional(14.0),
                    MUTED(),
                    (right - x).max(100.0),
                );
                job.wrap.max_rows = 3;
                let g = ui.fonts(|f| f.layout_job(job));
                ui.painter().galley(pos2(x, thumb.top() + 30.0), g, MUTED());
            }
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                click = Some(Click::Play(item.id.clone(), Some(ep.id.clone()), false));
            }
        }
        click
    }

    /// YouTube's watch page: the big picture, title, channel line and
    /// description, with the episodes (or more movies) in a list beside it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn watch_page(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        item: &Item,
        images: &mut Images,
        progress: &Progress,
        file: Option<&FileInfo>,
        runtime: Option<f64>,
    ) -> Option<Click> {
        let mut click = None;
        if item.kind == Kind::Series && self.season >= item.seasons.len() {
            self.season = self.next_up(item, progress).map_or(0, |n| n.si);
        }
        let avail = ui.available_width();
        let pad = 24.0;
        let side_w = if avail > 1100.0 { 400.0 } else { 0.0 };
        let gap = if side_w > 0.0 { 24.0 } else { 0.0 };
        let main_w = (avail - 2.0 * pad - side_w - gap).min(1280.0);
        ui.add_space(20.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(pad);
            ui.vertical(|ui| {
                ui.set_width(main_w);
                if let Some(c) = self.watch_main(ui, item, images, progress, file, runtime, main_w)
                {
                    click = Some(c);
                }
                if side_w == 0.0 {
                    ui.add_space(20.0);
                    if let Some(c) = self.watch_side(ui, lib, item, images, progress, main_w) {
                        click = Some(c);
                    }
                }
            });
            if side_w > 0.0 {
                ui.add_space(gap);
                ui.vertical(|ui| {
                    ui.set_width(side_w);
                    if let Some(c) = self.watch_side(ui, lib, item, images, progress, side_w) {
                        click = Some(c);
                    }
                });
            }
        });
        click
    }

    #[allow(clippy::too_many_arguments)]
    fn watch_main(
        &mut self,
        ui: &mut Ui,
        item: &Item,
        images: &mut Images,
        progress: &Progress,
        file: Option<&FileInfo>,
        runtime: Option<f64>,
        w: f32,
    ) -> Option<Click> {
        let mut click = None;
        let spacing = ui.spacing().item_spacing;
        ui.spacing_mut().item_spacing = vec2(10.0, 8.0);
        // the "player": the picture with a red play button
        let (r, resp) = ui.allocate_exact_size(vec2(w, w * 9.0 / 16.0), Sense::click());
        let p = ui.painter();
        match item
            .backdrop
            .as_ref()
            .or(item.poster.as_ref())
            .and_then(|u| images.get(u))
        {
            Some(t) => image_cover(p, r, &t, 12, Color32::WHITE),
            None => placeholder(p, r, &item.title, 12),
        }
        p.rect_filled(
            r,
            12.0,
            Color32::from_black_alpha(if resp.hovered() { 40 } else { 70 }),
        );
        let btn = Rect::from_center_size(r.center(), vec2(72.0, 50.0));
        p.rect_filled(
            btn,
            14.0,
            if resp.hovered() {
                ACCENT()
            } else {
                Color32::from_black_alpha(200)
            },
        );
        chrome::draw_icon(p, btn.center() + vec2(2.0, 0.0), Icon::Play, Color32::WHITE);
        if item.kind == Kind::Movie {
            if let Some(e) = progress
                .get(&self.key(&item.id))
                .filter(|e| e.in_progress())
            {
                let bar = Rect::from_min_max(pos2(r.left(), r.bottom() - 4.0), r.max);
                let p = p.with_clip_rect(r.shrink2(vec2(6.0, 0.0)));
                p.rect_filled(bar, 0.0, Color32::from_white_alpha(70));
                p.rect_filled(
                    Rect::from_min_max(
                        bar.min,
                        pos2(bar.left() + bar.width() * e.fraction(), bar.bottom()),
                    ),
                    0.0,
                    theme::played(),
                );
            }
        }
        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            click = Some(Click::Play(item.id.clone(), None, false));
        }
        ui.add_space(6.0);
        ui.add(egui::Label::new(bold(&item.title, 21.0)).wrap());
        ui.add_space(2.0);
        // the "channel" line: the server, then the buttons
        ui.horizontal(|ui| {
            let (ar, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
            let server = self.client.info.name.clone();
            chrome::avatar(ui.painter(), ar.center(), 20.0, &server);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(bold(&server, 15.0));
                let n = item.seasons.iter().map(|s| s.episodes.len()).sum::<usize>();
                let sub = match item.kind {
                    Kind::Movie => "Movie".to_owned(),
                    Kind::Series => format!("{n} episodes"),
                };
                ui.label(RichText::new(sub).size(12.5).color(MUTED()));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let on = self.watchlist.contains(&self.key(&item.id));
                let save = if on { "Saved" } else { "Save" };
                let icon = if on { Icon::BookmarkOn } else { Icon::Bookmark };
                if fill_button(
                    ui,
                    save,
                    Some(icon),
                    (SURFACE_HI(), TEXT()),
                    38.0,
                    0.0,
                    19.0,
                )
                .clicked()
                {
                    let key = self.key(&item.id);
                    self.watchlist.toggle(&key);
                }
                let (label, resume) = self.play_label(item, progress);
                if resume
                    && item.kind == Kind::Movie
                    && fill_button(
                        ui,
                        "Start over",
                        None,
                        (SURFACE_HI(), TEXT()),
                        38.0,
                        0.0,
                        19.0,
                    )
                    .clicked()
                {
                    click = Some(Click::Play(item.id.clone(), None, true));
                }
                if fill_button(
                    ui,
                    &label,
                    Some(Icon::Play),
                    (PRIMARY(), INK()),
                    38.0,
                    0.0,
                    19.0,
                )
                .clicked()
                {
                    click = Some(Click::Play(item.id.clone(), None, false));
                }
            });
        });
        ui.add_space(6.0);
        // the description box
        egui::Frame::new()
            .fill(SURFACE_HI())
            .corner_radius(12)
            .inner_margin(14)
            .show(ui, |ui| {
                ui.set_width(w - 28.0);
                let mut bits: Vec<String> = Vec::new();
                match item.kind {
                    Kind::Movie => {
                        bits.extend(item.year.map(|y| y.to_string()));
                        bits.extend(runtime.map(fmt_runtime));
                    }
                    Kind::Series => bits.extend(years(item)),
                }
                bits.extend(item.rating.clone());
                bits.extend(file.and_then(|f| f.quality.clone()));
                if let Some(s) = item.score {
                    bits.push(format!("★ {s:.1}"));
                }
                ui.label(bold(bits.join("  ·  "), 14.0));
                if !item.genres.is_empty() {
                    ui.label(
                        RichText::new(
                            item.genres
                                .iter()
                                .map(|g| format!("#{}", g.replace(' ', "")))
                                .collect::<Vec<_>>()
                                .join(" "),
                        )
                        .size(13.5)
                        .color(ACCENT()),
                    );
                }
                if !item.overview.is_empty() {
                    ui.add_space(4.0);
                    ui.add(egui::Label::new(RichText::new(&item.overview).size(14.5)).wrap());
                }
                let names: Vec<&str> = super::super::makers(item).collect();
                if !names.is_empty() {
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        let l = if item.kind == Kind::Movie {
                            "Director"
                        } else {
                            "Created by"
                        };
                        ui.label(RichText::new(l).size(13.5).color(MUTED()));
                        if let Some(c) = people_links(ui, &names, 13.5) {
                            click = Some(c);
                        }
                    });
                }
                if !item.cast.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("Starring").size(13.5).color(MUTED()));
                        let names: Vec<&str> =
                            item.cast.iter().take(6).map(|c| c.name.as_str()).collect();
                        if let Some(c) = people_links(ui, &names, 13.5) {
                            click = Some(c);
                        }
                    });
                }
            });
        ui.spacing_mut().item_spacing = spacing;
        click
    }

    /// Beside the watch page: the season as a playlist, or more movies.
    fn watch_side(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        item: &Item,
        images: &mut Images,
        progress: &Progress,
        w: f32,
    ) -> Option<Click> {
        let mut click = None;
        let row = |ui: &mut Ui,
                   thumb_url: Option<&String>,
                   title: &str,
                   sub: &str,
                   idx: Option<u32>,
                   frac: Option<f32>,
                   images: &mut Images|
         -> bool {
            let (r, resp) = ui.allocate_exact_size(vec2(w, 100.0), Sense::click());
            let p = ui.painter();
            if resp.hovered() {
                p.rect_filled(r, 10.0, SURFACE_HI().gamma_multiply(0.7));
            }
            let mut x = r.left() + 6.0;
            if let Some(n) = idx {
                p.text(
                    pos2(x + 10.0, r.center().y),
                    Align2::CENTER_CENTER,
                    n.to_string(),
                    FontId::proportional(13.0),
                    MUTED(),
                );
                x += 24.0;
            }
            let th = Rect::from_min_size(pos2(x, r.top() + 6.0), vec2(156.0, 88.0));
            match thumb_url.and_then(|u| images.get(u)) {
                Some(t) => image_cover(p, th, &t, 8, Color32::WHITE),
                None => placeholder(p, th, title, 8),
            }
            if let Some(f) = frac {
                let bar = Rect::from_min_max(pos2(th.left(), th.bottom() - 3.0), th.max);
                p.rect_filled(bar, 0.0, Color32::from_white_alpha(70));
                p.rect_filled(
                    Rect::from_min_max(bar.min, pos2(bar.left() + bar.width() * f, bar.bottom())),
                    0.0,
                    theme::played(),
                );
            }
            let tx = th.right() + 10.0;
            let mut job = egui::text::LayoutJob::simple(
                title.to_owned(),
                bold_font(14.0),
                TEXT(),
                (r.right() - tx - 6.0).max(60.0),
            );
            job.wrap.max_rows = 2;
            let g = ui.fonts(|f| f.layout_job(job));
            let gh = g.size().y;
            ui.painter().galley(pos2(tx, th.top() + 2.0), g, TEXT());
            ui.painter().with_clip_rect(r).text(
                pos2(tx, th.top() + 6.0 + gh),
                Align2::LEFT_TOP,
                sub,
                FontId::proportional(12.5),
                MUTED(),
            );
            resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
        };
        match item.kind {
            Kind::Series if !item.seasons.is_empty() => {
                egui::Frame::new()
                    .stroke(Stroke::new(1.0f32, LINE()))
                    .corner_radius(12)
                    .inner_margin(egui::Margin::symmetric(8, 12))
                    .show(ui, |ui| {
                        ui.set_width(w - 16.0);
                        ui.horizontal(|ui| {
                            ui.add_space(6.0);
                            ui.label(bold(&item.title, 17.0));
                        });
                        ui.horizontal(|ui| {
                            ui.add_space(6.0);
                            let mut pick = self.season;
                            egui::ComboBox::from_id_salt("watch_season")
                                .selected_text(item.seasons[pick].label())
                                .width(150.0)
                                .show_ui(ui, |ui| {
                                    for (i, s) in item.seasons.iter().enumerate() {
                                        ui.selectable_value(&mut pick, i, s.label());
                                    }
                                });
                            self.season = pick;
                            let n = item.seasons[pick].episodes.len();
                            ui.label(
                                RichText::new(format!("{n} episodes"))
                                    .size(12.5)
                                    .color(MUTED()),
                            );
                        });
                        ui.add_space(6.0);
                        let season = &item.seasons[self.season];
                        for ep in &season.episodes {
                            let frac = progress.get(&self.key(&ep.id)).map(|e| e.fraction());
                            let sub = ep
                                .duration
                                .filter(|d| *d > 0.0)
                                .map(minutes)
                                .unwrap_or_default();
                            let thumb = ep.still.as_ref().or(item.backdrop.as_ref());
                            if row(ui, thumb, &ep.title, &sub, Some(ep.number), frac, images) {
                                click =
                                    Some(Click::Play(item.id.clone(), Some(ep.id.clone()), false));
                            }
                        }
                    });
            }
            _ => {
                ui.label(bold("Up next", 17.0));
                ui.add_space(4.0);
                for i in similar(lib, item, 14, &self.shuffle) {
                    let sub = super::super::short_meta(i);
                    let thumb = i
                        .thumb
                        .as_ref()
                        .or(i.backdrop.as_ref())
                        .or(i.poster.as_ref());
                    if row(ui, thumb, &i.title, &sub, None, None, images) {
                        click = Some(Click::Open(i.id.clone()));
                    }
                }
            }
        }
        click
    }
}
