//! The editor's other tabs (files, subtitles, episodes) and its extra
//! actions (TMDB matching, deleting a title), each on its own endpoint.
use eframe::egui::{self, vec2, Color32, RichText, Sense, Ui};
use serde_json::{json, Value};

use super::super::super::details::{bytes, caps, kv_rows, panel, TEAL};
use super::super::users::danger_button;
use super::super::{needs_note, Cx, Done, Remote, Section};
use super::{field, State};
use crate::api::{
    encode_segment, Client, Episode, Item, Kind, Season, Subtitle, SubtitleResult, TmdbMatch,
};
use crate::ui::{image_cover, mono, placeholder, primary_button, small_button, text_field};
use crate::ui::{ERROR, MUTED, TEXT};
use crate::util::fmt_runtime;

/// One episode being edited.
#[derive(Clone, Default)]
pub(super) struct EpisodeForm {
    id: String,
    title: String,
    aired: String,
    /// Minutes.
    runtime: String,
    overview: String,
}

/// What the subtitle and TMDB lookups found, per title.
#[derive(Default)]
pub(super) struct Extras {
    subs: Remote<Vec<Subtitle>>,
    found: Remote<Vec<SubtitleResult>>,
    lang: String,
    episode: String,
    tmdb_q: String,
    matches: Remote<Vec<TmdbMatch>>,
    confirm_delete: bool,
    edit: Option<EpisodeForm>,
}

impl Extras {
    /// Start over for another title.
    pub fn reset(&mut self) {
        *self = Self {
            lang: "en".into(),
            ..Default::default()
        };
    }

    /// Load the subtitle list again (after a change).
    pub fn refresh(&mut self) {
        self.subs.retry();
    }
}

/// The video file a subtitle search is for: the movie's, or for a series the
/// episode typed in ("S01E02"), else its first episode with a file.
fn video_name(item: &Item, episode: &str) -> String {
    let file = |f: &Option<crate::api::FileInfo>| {
        f.as_ref()
            .map(|f| {
                let name = f.name.clone().unwrap_or_default();
                format!("{name} {}", f.quality.clone().unwrap_or_default())
            })
            .filter(|n| !n.trim().is_empty())
    };
    if item.kind != Kind::Series {
        return file(&item.file).unwrap_or_default();
    }
    let code = episode.trim().to_lowercase();
    let eps = || {
        item.seasons
            .iter()
            .flat_map(|s| s.episodes.iter().map(move |e| (s.number, e)))
    };
    eps()
        .find(|(s, e)| format!("s{s:02}e{:02}", e.number) == code)
        .and_then(|(_, e)| file(&e.file))
        .or_else(|| eps().find_map(|(_, e)| file(&e.file)))
        .unwrap_or_default()
}

/// An episode's code as the subtitle endpoints take it: "S01E02".
fn episode_code(season: u32, episode: u32) -> String {
    format!("S{season:02}E{episode:02}")
}

/// The season and episode of a code like "S01E02" (any case).
fn parse_code(code: &str) -> Option<(u32, u32)> {
    let code = code.trim().to_lowercase();
    let (s, e) = code.strip_prefix('s')?.split_once('e')?;
    Some((s.parse().ok()?, e.parse().ok()?))
}

/// Season and episode dropdowns for the subtitle's episode, kept in
/// `code` ("S01E02"; empty: none picked).
fn episode_picker(ui: &mut Ui, seasons: &[Season], code: &mut String) {
    let picked = parse_code(code);
    let season = picked.and_then(|(n, _)| seasons.iter().find(|s| s.number == n));
    ui.vertical(|ui| {
        ui.label(mono("Season", 10.5).color(MUTED()));
        egui::ComboBox::from_id_salt("subtitle-season")
            .width(140.0)
            .selected_text(season.map_or("Choose…".into(), Season::label))
            .show_ui(ui, |ui| {
                for s in seasons {
                    let on = season.is_some_and(|c| c.number == s.number);
                    if ui.selectable_label(on, s.label()).clicked() && !on {
                        // the season's first episode, so a choice is always made
                        *code = s
                            .episodes
                            .first()
                            .map(|e| episode_code(s.number, e.number))
                            .unwrap_or_default();
                    }
                }
            });
    });
    let Some(season) = season else { return };
    let ep = picked.and_then(|(_, n)| season.episodes.iter().find(|e| e.number == n));
    let name = |e: &Episode| match e.title.trim() {
        "" => format!("Episode {}", e.number),
        t => format!("{}. {t}", e.number),
    };
    ui.vertical(|ui| {
        ui.label(mono("Episode", 10.5).color(MUTED()));
        egui::ComboBox::from_id_salt("subtitle-episode")
            .width(260.0)
            .selected_text(ep.map_or("Choose…".into(), name))
            .show_ui(ui, |ui| {
                for e in &season.episodes {
                    let on = ep.is_some_and(|c| c.number == e.number);
                    if ui.selectable_label(on, name(e)).clicked() {
                        *code = episode_code(season.number, e.number);
                    }
                }
            });
    });
}

fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// "bluray", "web", … for a release name: subtitles from the same kind of
/// source share the cut and the frame rate, others are often seconds off.
fn source_of(words: &[String]) -> Option<&'static str> {
    words.iter().find_map(|w| match w.as_str() {
        "bluray" | "bdrip" | "brrip" | "bdremux" | "remux" | "uhd" | "bd" => Some("bluray"),
        "web" | "webdl" | "webrip" | "amzn" | "nf" | "dsnp" | "hmax" | "atvp" => Some("web"),
        "hdrip" | "fhdrip" => Some("hdrip"),
        "dvdrip" | "dvd" => Some("dvd"),
        "hdtv" => Some("hdtv"),
        _ => None,
    })
}

const EDITIONS: [&str; 7] = [
    "bw",
    "black",
    "criterion",
    "extended",
    "directors",
    "uncut",
    "imax",
];

/// How well a subtitle's release fits the video file; higher is better,
/// 4 or more counts as a match.
fn release_score(video: &str, release: &str) -> i32 {
    let (v, r) = (words(video), words(release));
    if v.is_empty() || r.is_empty() {
        return 0;
    }
    let mut score = 0;
    // the release group: "…x265-YAWNTiC.mkv"
    let stem = video.rsplit_once('.').map_or(video, |(s, _)| s);
    if let Some((_, group)) = stem
        .split_whitespace()
        .next()
        .unwrap_or(stem)
        .rsplit_once('-')
    {
        let g = group.to_lowercase();
        if g.len() > 1 && r.contains(&g) {
            score += 8;
        }
    }
    match (source_of(&v), source_of(&r)) {
        (Some(a), Some(b)) if a == b => score += 4,
        (Some(_), Some(_)) => score -= 4,
        _ => {}
    }
    for res in ["2160p", "1080p", "720p", "480p"] {
        let res = res.to_owned();
        if v.contains(&res) && r.contains(&res) {
            score += 1;
        }
    }
    for e in EDITIONS {
        let e = e.to_owned();
        if r.contains(&e) && !v.contains(&e) {
            score -= 3;
        }
    }
    score
}

/// An episode's id as the server knows it (ours carries "episode:").
fn episode_id(ep: &Episode) -> &str {
    ep.id.strip_prefix("episode:").unwrap_or(&ep.id)
}

impl State {
    pub(super) fn files_tab(&mut self, ui: &mut Ui, cx: &mut Cx, item: &Item) {
        let file_rows = |ui: &mut Ui, f: &crate::api::FileInfo| {
            let mut rows: Vec<(&str, String, Color32, f32)> = Vec::new();
            let mut add = |k: &'static str, v: Option<String>| {
                if let Some(v) = v {
                    rows.push((k, v, TEXT(), 14.0));
                }
            };
            add("File", f.name.clone());
            add("Folder", f.path.clone());
            add("Quality", f.quality.clone());
            add("Container", f.container.clone());
            add("Video", f.video_codec.clone());
            let audio = match (&f.audio_codec, f.audio_channels) {
                (Some(a), Some(c)) => Some(format!("{a}, {c} channels")),
                (a, _) => a.clone(),
            };
            add("Audio", audio);
            add(
                "Bitrate",
                f.bitrate.map(|b| format!("{:.1} Mbit/s", b as f64 / 1e6)),
            );
            add("Size", f.size.map(bytes));
            if rows.is_empty() {
                ui.label(RichText::new("The server doesn't describe this file.").color(MUTED()));
            } else {
                kv_rows(ui, &rows);
            }
        };
        if item.kind == Kind::Movie {
            panel(ui, |ui| {
                caps(ui, "Video file", TEAL());
                ui.add_space(6.0);
                match &item.file {
                    Some(f) => file_rows(ui, f),
                    None => needs_note(
                        ui,
                        "The server doesn't send file details for this title.",
                        Section::Library,
                    ),
                }
            });
            return;
        }
        let seasons = seasons_of(item, cx);
        if seasons.is_empty() {
            ui.label(RichText::new("No episodes.").color(MUTED()));
        }
        for s in &seasons {
            egui::CollapsingHeader::new(s.label())
                .id_salt(("files-season", s.number))
                .default_open(seasons.len() == 1)
                .show(ui, |ui| {
                    for e in &s.episodes {
                        let f = e.file.clone().unwrap_or_default();
                        let mut bits = vec![format!("E{:02} {}", e.number, e.title)];
                        bits.extend(f.quality.clone());
                        bits.extend(f.container.clone());
                        bits.extend(f.video_codec.clone());
                        bits.extend(f.size.map(bytes));
                        ui.label(mono(bits.join("   ·   "), 10.5).color(TEXT()))
                            .on_hover_text(f.name.clone().unwrap_or_default());
                    }
                });
        }
    }

    pub(super) fn subtitles_tab(&mut self, ui: &mut Ui, cx: &mut Cx, item: &Item) {
        let path = Client::item_path(item);
        let x = &mut self.extras;
        x.subs.poll(&mut cx.login);
        x.found.poll(&mut cx.login);
        if x.subs.due() {
            let (c, p) = (cx.client.clone(), path.clone());
            x.subs
                .load(&cx.ctx, move || c.admin_get(&format!("{p}/subtitles")));
        }
        if x.subs.missing {
            needs_note(
                ui,
                "This server can't manage subtitles yet.",
                Section::Library,
            );
            return;
        }
        let series = item.kind == Kind::Series;
        let seasons = if series {
            seasons_of(item, cx)
        } else {
            Vec::new()
        };
        panel(ui, |ui| {
            caps(ui, "Subtitles", TEAL());
            ui.add_space(8.0);
            match &x.subs.data {
                None if x.subs.loading() => {
                    ui.add(egui::Spinner::new().size(16.0).color(MUTED()));
                }
                Some(list) if list.is_empty() => {
                    ui.label(RichText::new("None yet.").color(MUTED()));
                }
                Some(list) => {
                    for s in list {
                        ui.horizontal(|ui| {
                            let mut text = s.label.clone().unwrap_or_else(|| s.language.clone());
                            if let Some(e) = &s.episode {
                                text = format!("{e}   {text}");
                            }
                            ui.label(RichText::new(text).color(TEXT()));
                            if let Some(src) = &s.source {
                                ui.label(mono(src, 9.5).color(MUTED()));
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_enabled_ui(!cx.busy, |ui| small_button(ui, "Remove"))
                                        .inner
                                        .clicked()
                                    {
                                        let (p, id) = (path.clone(), s.id.clone());
                                        cx.run(move |c| {
                                            c.admin_call(
                                                "DELETE",
                                                &format!("{p}/subtitles/{}", encode_segment(&id)),
                                                None,
                                            )?;
                                            Ok(Done::msg("Removed the subtitle."))
                                        });
                                    }
                                },
                            );
                        });
                    }
                }
                None => {
                    if let Some(e) = &x.subs.error {
                        ui.label(RichText::new(e).color(ERROR()));
                    }
                }
            }
        });
        ui.add_space(16.0);
        panel(ui, |ui| {
            caps(ui, "Add a subtitle", TEAL());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(120.0);
                    ui.label(mono("Language", 10.5).color(MUTED()));
                    text_field(ui, &mut x.lang, "en", false);
                });
                if series {
                    episode_picker(ui, &seasons, &mut x.episode);
                }
            });
            ui.add_space(8.0);
            let mut query = format!("language={}", encode_segment(x.lang.trim()));
            if series && !x.episode.trim().is_empty() {
                query.push_str(&format!("&episode={}", encode_segment(x.episode.trim())));
            }
            ui.horizontal(|ui| {
                let ok = !cx.busy && !x.lang.trim().is_empty();
                if ui
                    .add_enabled_ui(ok, |ui| small_button(ui, "Upload .srt / .vtt…"))
                    .inner
                    .clicked()
                {
                    if let Some(file) = rfd::FileDialog::new()
                        .add_filter("Subtitles", &["srt", "vtt", "ass", "ssa"])
                        .pick_file()
                    {
                        let (p, q) = (path.clone(), query.clone());
                        cx.run(move |c| {
                            let data = std::fs::read(&file)?;
                            c.admin_upload(&format!("{p}/subtitles?{q}"), &data, "text/plain")?;
                            Ok(Done::msg("Uploaded the subtitle."))
                        });
                    }
                }
                if ui
                    .add_enabled_ui(ok && !x.found.loading(), |ui| {
                        small_button(ui, "Search online")
                    })
                    .inner
                    .clicked()
                {
                    let (c, p, q) = (cx.client.clone(), path.clone(), query.clone());
                    x.found = Remote::default();
                    x.found.load(&cx.ctx, move || {
                        c.admin_get(&format!("{p}/subtitles/search?{q}"))
                    });
                }
                if x.found.loading() {
                    ui.add(egui::Spinner::new().size(14.0).color(MUTED()));
                }
            });
            if x.found.missing {
                needs_note(
                    ui,
                    "This server can't search for subtitles online.",
                    Section::Library,
                );
            } else if let Some(e) = &x.found.error {
                ui.label(RichText::new(e).small().color(ERROR()));
            }
            if let Some(results) = &x.found.data {
                ui.add_space(8.0);
                if results.is_empty() {
                    ui.label(RichText::new("Nothing found.").color(MUTED()));
                }
                // subtitles are timed to one release: ones made for this file
                // first, then the server's order (most downloaded)
                let video = video_name(item, &x.episode);
                let mut ranked: Vec<(i32, &SubtitleResult)> = results
                    .iter()
                    .map(|r| {
                        let rel = r.release.as_deref().unwrap_or(&r.name);
                        let s = if r.moviehash_match {
                            100
                        } else {
                            release_score(&video, rel)
                        };
                        (s, r)
                    })
                    .collect();
                ranked.sort_by_key(|(s, _)| -s);
                if !video.trim().is_empty() && !results.is_empty() {
                    ui.label(
                        RichText::new(format!("Your file: {}", video.trim()))
                            .small()
                            .color(MUTED()),
                    );
                    ui.add_space(4.0);
                }
                for (score, r) in ranked.into_iter().take(30) {
                    ui.horizontal(|ui| {
                        let mut line = r.release.clone().unwrap_or_else(|| r.name.clone());
                        if let Some(e) = &r.episode {
                            line = format!("{e}   {line}");
                        }
                        if score >= 4 {
                            ui.label(mono("MATCHES YOUR FILE", 9.5).color(TEAL()));
                        }
                        ui.add(
                            egui::Label::new(RichText::new(line).small().color(TEXT())).truncate(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add_enabled_ui(!cx.busy, |ui| small_button(ui, "Download"))
                                .inner
                                .clicked()
                            {
                                // which episode it's for: the one picked, else the result's
                                let episode = Some(x.episode.trim())
                                    .filter(|e| !e.is_empty())
                                    .map(str::to_owned)
                                    .or_else(|| r.episode.clone());
                                let (p, id) = (path.clone(), r.id.clone());
                                let q = episode.as_ref().map_or(String::new(), |e| {
                                    format!("?episode={}", encode_segment(e))
                                });
                                let body = match &episode {
                                    Some(e) => json!({ "id": id, "episode": e }),
                                    None => json!({ "id": id }),
                                };
                                cx.run(move |c| {
                                    c.admin_call(
                                        "POST",
                                        &format!("{p}/subtitles/download{q}"),
                                        Some(&body),
                                    )?;
                                    Ok(Done::msg("Downloaded the subtitle."))
                                });
                            }
                            if let Some(d) = r.downloads {
                                ui.label(mono(format!("{d} downloads"), 9.5).color(MUTED()));
                            }
                        });
                    });
                }
            }
        });
    }

    pub(super) fn episodes_tab(&mut self, ui: &mut Ui, cx: &mut Cx, item: &Item) {
        let seasons = seasons_of(item, cx);
        if seasons.is_empty() {
            ui.label(RichText::new("No episodes.").color(MUTED()));
            return;
        }
        let series_path = Client::item_path(item);
        for s in &seasons {
            egui::CollapsingHeader::new(format!(
                "{}   ·   {} episodes",
                s.label(),
                s.episodes.len()
            ))
            .id_salt(("episodes-season", s.number))
            .default_open(seasons.len() == 1)
            .show(ui, |ui| {
                if ui
                    .add_enabled_ui(!cx.busy, |ui| small_button(ui, "Refresh season from TMDB"))
                    .inner
                    .clicked()
                {
                    let (p, n) = (series_path.clone(), s.number);
                    cx.run(move |c| {
                        c.admin_call("POST", &format!("{p}/seasons/{n}/refresh"), None)?;
                        Ok(Done {
                            msg: format!("Refreshed season {n}."),
                            library: true,
                            ..Default::default()
                        })
                    });
                }
                ui.add_space(6.0);
                for e in &s.episodes {
                    self.episode_row(ui, cx, e);
                    ui.add_space(4.0);
                }
            });
        }
    }

    fn episode_row(&mut self, ui: &mut Ui, cx: &mut Cx, e: &Episode) {
        let id = episode_id(e).to_owned();
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(96.0, 54.0), Sense::hover());
            match e.still.as_ref().and_then(|u| cx.images.get(u)) {
                Some(t) => image_cover(ui.painter(), r, &t, 3, Color32::WHITE),
                None => placeholder(ui.painter(), r, &e.title, 3),
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(format!("{}. {}", e.number, e.title)).color(TEXT()));
                let mut bits = Vec::new();
                bits.extend(e.aired.clone());
                bits.extend(e.duration.map(fmt_runtime));
                ui.label(mono(bits.join("   ·   "), 10.0).color(MUTED()));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled_ui(!cx.busy, |ui| small_button(ui, "Fetch still"))
                    .inner
                    .clicked()
                {
                    let id = id.clone();
                    cx.run(move |c| {
                        c.admin_call(
                            "POST",
                            &format!("episodes/{}/fetch-image", encode_segment(&id)),
                            None,
                        )?;
                        Ok(Done {
                            msg: "Fetched the episode image.".into(),
                            library: true,
                            ..Default::default()
                        })
                    });
                }
                if small_button(ui, "Edit").clicked() {
                    self.extras.edit = Some(EpisodeForm {
                        id: id.clone(),
                        title: e.title.clone(),
                        aired: e.aired.clone().unwrap_or_default(),
                        runtime: e
                            .duration
                            .map(|d| format!("{:.0}", d / 60.0))
                            .unwrap_or_default(),
                        overview: e.overview.clone(),
                    });
                }
            });
        });
        let Some(f) = self.extras.edit.as_mut().filter(|f| f.id == id) else {
            return;
        };
        let mut close = false;
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(8, 8))
            .show(ui, |ui| {
                field(ui, "Title", &mut f.title, "");
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(160.0);
                        field(ui, "Aired", &mut f.aired, "YYYY-MM-DD");
                    });
                    ui.vertical(|ui| {
                        ui.set_width(120.0);
                        field(ui, "Runtime (min)", &mut f.runtime, "42");
                    });
                });
                ui.label(mono("Overview", 10.5).color(MUTED()));
                ui.add(
                    egui::TextEdit::multiline(&mut f.overview)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "Save episode", !cx.busy).clicked() {
                        let minutes = f.runtime.trim().parse::<f64>().ok();
                        let aired = f.aired.trim();
                        let body = json!({
                            "title": f.title.trim(),
                            "aired": if aired.is_empty() { Value::Null } else { json!(aired) },
                            "duration": minutes.map(|m| m * 60.0),
                            "overview": f.overview.trim(),
                        });
                        let id = f.id.clone();
                        cx.run(move |c| {
                            c.admin_call(
                                "PATCH",
                                &format!("episodes/{}", encode_segment(&id)),
                                Some(&body),
                            )?;
                            Ok(Done {
                                msg: "Saved the episode.".into(),
                                library: true,
                                ..Default::default()
                            })
                        });
                        close = true;
                    }
                    if small_button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
            });
        if close {
            self.extras.edit = None;
        }
    }

    /// Details tab, under the fields: pick the right TMDB entry, or delete the title.
    pub(super) fn match_and_delete(&mut self, ui: &mut Ui, cx: &mut Cx, item: &Item) {
        let path = Client::item_path(item);
        let x = &mut self.extras;
        x.matches.poll(&mut cx.login);
        ui.add_space(20.0);
        panel(ui, |ui| {
            caps(ui, "TMDB match", TEAL());
            ui.add_space(4.0);
            let now = item
                .tmdb_id
                .as_ref()
                .map(|v| v.to_string().trim_matches('"').to_owned());
            ui.label(
                RichText::new(match &now {
                    Some(id) => format!(
                        "Matched to TMDB {id}. Wrong film or show? Search and pick the right one."
                    ),
                    None => "Not matched to TMDB. Search and pick the right entry.".into(),
                })
                .small()
                .color(MUTED()),
            );
            ui.add_space(6.0);
            if x.tmdb_q.is_empty() {
                x.tmdb_q = item.title.clone();
            }
            ui.horizontal(|ui| {
                let w = ui.available_width() - 90.0;
                ui.add_sized(
                    [w.max(120.0), 34.0],
                    egui::TextEdit::singleline(&mut x.tmdb_q)
                        .margin(egui::Margin::symmetric(10, 8)),
                );
                if ui
                    .add_enabled_ui(!x.matches.loading(), |ui| small_button(ui, "Search"))
                    .inner
                    .clicked()
                {
                    let (c, p, q) = (
                        cx.client.clone(),
                        path.clone(),
                        encode_segment(x.tmdb_q.trim()),
                    );
                    x.matches = Remote::default();
                    x.matches.load(&cx.ctx, move || {
                        c.admin_get(&format!("{p}/tmdb-search?q={q}"))
                    });
                }
            });
            if x.matches.missing {
                needs_note(
                    ui,
                    "This server can't search TMDB from the app yet.",
                    Section::Library,
                );
            } else if let Some(e) = &x.matches.error {
                ui.label(RichText::new(e).small().color(ERROR()));
            }
            if let Some(list) = &x.matches.data {
                ui.add_space(6.0);
                if list.is_empty() {
                    ui.label(RichText::new("Nothing found.").color(MUTED()));
                }
                for m in list.iter().take(10) {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(34.0, 51.0), Sense::hover());
                        match m.poster.as_ref().and_then(|u| cx.images.get(u)) {
                            Some(t) => image_cover(ui.painter(), r, &t, 3, Color32::WHITE),
                            None => placeholder(ui.painter(), r, &m.title, 3),
                        }
                        ui.vertical(|ui| {
                            let year = m.year.map(|y| format!(" ({y})")).unwrap_or_default();
                            ui.label(RichText::new(format!("{}{year}", m.title)).color(TEXT()));
                            if let Some(o) = &m.overview {
                                ui.add(
                                    egui::Label::new(RichText::new(o).small().color(MUTED()))
                                        .truncate(),
                                );
                            }
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add_enabled_ui(!cx.busy, |ui| small_button(ui, "Use this"))
                                .inner
                                .clicked()
                            {
                                let (p, id, t) = (path.clone(), m.tmdb_id.clone(), m.title.clone());
                                cx.run(move |c| {
                                    c.admin_call(
                                        "PUT",
                                        &format!("{p}/tmdb"),
                                        Some(&json!({ "tmdb_id": id, "refetch": true })),
                                    )?;
                                    Ok(Done {
                                        msg: format!("Matched to {t} and refreshed its details."),
                                        library: true,
                                        ..Default::default()
                                    })
                                });
                            }
                        });
                    });
                }
            }
        });
        ui.add_space(20.0);
        panel(ui, |ui| {
            caps(ui, "Delete", ERROR());
            ui.add_space(4.0);
            ui.label(
                RichText::new("Removes the title from the library and its watch progress. The video files stay on disk unless the server says otherwise.")
                    .small()
                    .color(MUTED()),
            );
            ui.add_space(6.0);
            if x.confirm_delete {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled_ui(!cx.busy, |ui| {
                            danger_button(ui, &format!("Delete {}", item.title))
                        })
                        .inner
                        .clicked()
                    {
                        let (p, t) = (path.clone(), item.title.clone());
                        cx.run(move |c| {
                            c.admin_call("DELETE", &p, None)?;
                            Ok(Done {
                                msg: format!("Deleted {t}."),
                                library: true,
                                ..Default::default()
                            })
                        });
                        x.confirm_delete = false;
                    }
                    if small_button(ui, "Cancel").clicked() {
                        x.confirm_delete = false;
                    }
                });
            } else if danger_button(ui, "Delete title…").clicked() {
                x.confirm_delete = true;
            }
        });
    }
}

/// The series' seasons: from the server's record, else the library's.
fn seasons_of(item: &Item, cx: &Cx) -> Vec<crate::api::Season> {
    if !item.seasons.is_empty() {
        return item.seasons.clone();
    }
    cx.lib
        .find(&item.id)
        .map(|i| i.seasons.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{episode_code, parse_code, release_score};

    #[test]
    fn episode_codes() {
        assert_eq!(episode_code(1, 2), "S01E02");
        assert_eq!(episode_code(12, 104), "S12E104");
        assert_eq!(parse_code("S01E02"), Some((1, 2)));
        assert_eq!(parse_code(" s3e10 "), Some((3, 10)));
        assert_eq!(parse_code(""), None);
        assert_eq!(parse_code("Pilot"), None);
    }

    #[test]
    fn release_matching() {
        let video = "Parasite.2019.1080p.BluRay.x265-YAWNTiC.mkv";
        let group = release_score(video, "Parasite.2019.1080p.BluRay.x265-YAWNTiC_eng");
        let bluray = release_score(video, "Parasite (2019) Bluray-1080p");
        let rip = release_score(video, "PARASITE.2019.1080p.FHDRip.H264.AAC-NonDRM");
        let bw = release_score(video, "Parasite.2019.BW.PROPER.1080p.BluRay.x264-REGRET");
        assert!(
            group > bluray && bluray > rip && bluray > bw,
            "{group} {bluray} {rip} {bw}"
        );
        assert!(group >= 4 && bluray >= 4 && rip < 4 && bw < 4);
        // a file renamed without its release: only the resolution helps
        let plain = "Parasite.2019.2160p.mkv 2160p";
        assert!(
            release_score(plain, "Parasite 2019 2160p UHD Bluray (4k)")
                > release_score(plain, "PARASITE.2019.1080p.FHDRip.H264.AAC-NonDRM")
        );
        assert_eq!(release_score("", "anything"), 0);
    }
}
