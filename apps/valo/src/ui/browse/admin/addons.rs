//! Admin › Addons: Stremio addons for every user. Catalogs become rows on
//! the home page, metadata fills in what titles are missing, and subtitles
//! show up in the player. Stream (torrent) addons aren't taken; the videos
//! come from the server.
use eframe::egui::{self, Align, Layout, RichText, Stroke, Ui};

use super::super::details::{caps, panel, TEAL};
use super::super::people::{ago, columns_with, section_title};
use super::users::danger_button;
use super::{not_set_up, Cx, Done, Remote, Section};
use crate::api::Addon;
use crate::ui::{bold, mono, primary_button, small_button, text_field};
use crate::ui::{ERROR, LINE, MUTED, TEXT};

#[derive(Default)]
pub(in super::super) struct State {
    pub list: Remote<Vec<Addon>>,
    /// The manifest URL being added.
    url: String,
    confirm_delete: Option<String>,
}

impl State {
    pub fn refresh(&mut self) {
        self.list.retry();
    }

    pub fn missing(&self) -> bool {
        self.list.missing
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        self.list.poll(&mut cx.login);
        if self.list.due() {
            let c = cx.client.clone();
            self.list.load(&cx.ctx, move || c.admin_addons());
        }
        if self.list.missing {
            super::super::details::padded(ui, |ui| {
                if not_set_up(ui, Section::Addons, &cx.client.info.name) {
                    self.list.retry();
                }
            });
            return;
        }
        let busy = cx.busy;
        let mut add = None;
        let mut delete = None;
        let State {
            list,
            url,
            confirm_delete,
        } = self;
        columns_with(
            ui,
            &mut (&mut add, &mut delete),
            |ui, (_, delete)| {
                let n = list.data.as_ref().map_or(0, Vec::len);
                section_title(ui, "Stremio", "Addons", n);
                if let Some(e) = list.error.as_ref() {
                    ui.label(
                        RichText::new(format!("Couldn't load the addons: {e}")).color(ERROR()),
                    );
                }
                let Some(addons) = list.data.as_ref() else {
                    if list.loading() {
                        ui.add(egui::Spinner::new().size(20.0).color(MUTED()));
                    }
                    return;
                };
                if addons.is_empty() {
                    ui.label(mono("No addons yet", 12.0).color(MUTED()));
                }
                for a in addons {
                    if addon_row(ui, a, busy, confirm_delete) {
                        **delete = Some((a.id.clone(), a.name.clone()));
                    }
                    ui.add_space(8.0);
                }
            },
            |ui, (add, _)| {
                panel(ui, |ui| {
                    caps(ui, "Add an addon", TEAL());
                    ui.add_space(10.0);
                    ui.label(mono("Manifest URL", 11.0).color(MUTED()));
                    let f = text_field(ui, url, "https://…/manifest.json", false);
                    ui.add_space(10.0);
                    let ok = !busy && !url.trim().is_empty();
                    let enter = f.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let b = ui.add_sized([ui.available_width(), 38.0], |ui: &mut Ui| {
                        primary_button(ui, "Add addon", ok)
                    });
                    if ok && (b.clicked() || enter) {
                        **add = Some(url.trim().to_owned());
                    }
                    ui.add_space(14.0);
                    for line in [
                        "Paste the link from the addon's install page. For a configurable \
                         addon, configure it first and paste the link it gives you.",
                        "Catalogs become rows on everyone's home page. Titles that aren't \
                         on the server can be requested; they show up under Requests.",
                        "Metadata addons fill in what your titles are missing (posters, \
                         overviews, cast, …), and subtitle addons add subtitles to the player.",
                        "Stream and torrent addons are refused, since the videos come from \
                         this server.",
                        "Titles are matched by IMDb id (Library › Fetch from IMDb). Everyone \
                         signed in can see the addon links.",
                    ] {
                        ui.label(RichText::new(line).color(MUTED()));
                        ui.add_space(6.0);
                    }
                });
            },
        );
        if let Some(u) = add {
            let url = &mut self.url;
            url.clear();
            cx.run(move |c| {
                let a = c.admin_add_addon(&u)?;
                Ok(Done::msg(format!(
                    "Added {}. It's on for everyone now.",
                    if a.name.is_empty() {
                        "the addon"
                    } else {
                        &a.name
                    }
                )))
            });
        }
        if let Some((id, name)) = delete {
            cx.run(move |c| {
                c.admin_delete_addon(&id)?;
                Ok(Done::msg(format!("Removed {name}.")))
            });
        }
    }
}

/// One addon: name, what it is, its link. True when the admin removes it.
fn addon_row(ui: &mut Ui, a: &Addon, busy: bool, confirm: &mut Option<String>) -> bool {
    let mut remove = false;
    egui::Frame::new()
        .stroke(Stroke::new(1.0f32, LINE()))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    ui.horizontal(|ui| {
                        ui.label(bold(&a.name, 16.0));
                        if let Some(v) = a.version.as_ref().filter(|v| !v.is_empty()) {
                            ui.label(mono(format!("v{v}"), 9.5).color(MUTED()));
                        }
                    });
                    let offers = a.offers();
                    if !offers.is_empty() {
                        ui.label(mono(offers, 10.0).color(TEAL()));
                    }
                    if !a.catalogs.is_empty() {
                        let names: Vec<String> = a
                            .catalogs
                            .iter()
                            .map(|c| {
                                let what = if c.kind == "movie" { "movies" } else { "TV" };
                                format!("{} ({what})", c.name)
                            })
                            .collect();
                        ui.label(
                            RichText::new(format!("Rows: {}", names.join(", ")))
                                .small()
                                .color(MUTED()),
                        );
                    }
                    if let Some(d) = a.description.as_ref().filter(|d| !d.trim().is_empty()) {
                        ui.label(RichText::new(d.trim()).color(TEXT()));
                    }
                    let mut meta = Vec::new();
                    if let Some(b) = &a.added_by {
                        meta.push(format!("added by {b}"));
                    }
                    if let Some(t) = a.added {
                        meta.push(ago(t));
                    }
                    if !meta.is_empty() {
                        ui.label(mono(meta.join("   ·   "), 10.0).color(MUTED()));
                    }
                    ui.add(egui::Label::new(mono(&a.url, 10.0).color(MUTED())).truncate());
                });
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    if confirm.is_none()
                        && ui
                            .add_enabled_ui(!busy, |ui| small_button(ui, "Remove…"))
                            .inner
                            .clicked()
                    {
                        *confirm = Some(a.id.clone());
                    }
                });
            });
            if confirm.as_deref() == Some(a.id.as_str()) {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(format!("Remove {}? It goes away for everyone.", a.name))
                        .color(TEXT()),
                );
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled_ui(!busy, |ui| danger_button(ui, "Remove"))
                        .inner
                        .clicked()
                    {
                        remove = true;
                        *confirm = None;
                    }
                    if small_button(ui, "Cancel").clicked() {
                        *confirm = None;
                    }
                });
            }
        });
    remove
}
