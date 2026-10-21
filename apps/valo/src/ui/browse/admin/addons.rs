//! Admin › Addons: Stremio addons for every user. Catalogs become rows on
//! the home page, metadata fills in what titles are missing, and subtitles
//! show up in the player. Stream (torrent) addons aren't taken; the videos
//! come from the server.
use eframe::egui::{self, Align, Layout, RichText, Stroke, Ui};

use super::super::details::{caps, panel, TEAL};
use super::super::people::{ago, columns_with, section_title};
use super::users::danger_button;
use super::{not_set_up, Cx, Done, Remote, Section};
use crate::addons::configure_url;
use crate::api::Addon;
use crate::ui::chrome::{self, Icon};
use crate::ui::{bold, mono, primary_button, small_button, small_button_icon, text_field};
use crate::ui::{ERROR, LINE, MUTED, TEXT};

#[derive(Default)]
pub(in super::super) struct State {
    pub list: Remote<Vec<Addon>>,
    /// The manifest URL being added.
    url: String,
    confirm_delete: Option<String>,
    /// Swapping an addon's link for a newly configured one: (addon id, new link).
    relink: Option<(String, String)>,
}

/// What the admin asked for on an addon's card.
enum Ask {
    Remove,
    /// Use this link instead (new settings).
    Relink(String),
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
            self.list.load(&cx.ctx, move || {
                let mut l = c.admin_addons()?;
                crate::addons::mark_configurable(&mut l);
                Ok(l)
            });
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
        let mut relinked = None;
        let State {
            list,
            url,
            confirm_delete,
            relink,
        } = self;
        columns_with(
            ui,
            &mut (&mut add, &mut delete, &mut relinked),
            |ui, (_, delete, relinked)| {
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
                    match addon_row(ui, a, busy, confirm_delete, relink) {
                        Some(Ask::Remove) => **delete = Some((a.id.clone(), a.name.clone())),
                        Some(Ask::Relink(u)) => {
                            **relinked = Some((a.id.clone(), a.name.clone(), u));
                        }
                        None => {}
                    }
                    ui.add_space(8.0);
                }
            },
            |ui, (add, _, _)| {
                panel(ui, |ui| {
                    caps(ui, "Add an addon", TEAL());
                    ui.add_space(10.0);
                    ui.label(mono("Manifest URL", 11.0).color(MUTED()));
                    let f = text_field(ui, url, "https://…/manifest.json", false);
                    let typed = url.trim();
                    if typed.starts_with("http") || typed.starts_with("stremio://") {
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            if crate::ui::ext_link(ui, RichText::new("Configure it first").small())
                            {
                                chrome::open_url(&configure_url(typed));
                            }
                            ui.label(
                                RichText::new("then paste the link it gives you")
                                    .small()
                                    .color(MUTED()),
                            );
                        });
                    }
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
        if let Some((old, name, u)) = relinked {
            self.relink = None;
            cx.run(move |c| {
                // the new link first, so a bad one leaves the old addon working
                let a = c.admin_add_addon(&u)?;
                c.admin_delete_addon(&old)?;
                let name = if a.name.is_empty() { name } else { a.name };
                Ok(Done::msg(format!(
                    "Updated {name}. Everyone gets the new settings now."
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

/// One addon: name, what it is, its link, and its buttons.
fn addon_row(
    ui: &mut Ui,
    a: &Addon,
    busy: bool,
    confirm: &mut Option<String>,
    relink: &mut Option<(String, String)>,
) -> Option<Ask> {
    let mut ask = None;
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
                                format!("{} ({what})", c.name.trim())
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
                    ui.add_enabled_ui(!busy, |ui| {
                        if confirm.is_none() && small_button(ui, "Remove…").clicked() {
                            *confirm = Some(a.id.clone());
                            *relink = None;
                        }
                        if a.configurable {
                            if relink.is_none() && small_button(ui, "New link…").clicked() {
                                *relink = Some((a.id.clone(), String::new()));
                                *confirm = None;
                            }
                            if small_button_icon(ui, Icon::External, "Configure").clicked() {
                                chrome::open_url(&configure_url(&a.url));
                            }
                        }
                    });
                });
            });
            if let Some((_, new)) = relink.as_mut().filter(|(id, _)| *id == a.id) {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(
                        "Change the settings on its configure page, copy the link it gives \
                         you, and paste it here. Everyone gets the new settings.",
                    )
                    .color(TEXT()),
                );
                ui.add_space(4.0);
                let f = text_field(ui, new, "https://…/manifest.json", false);
                let enter = f.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let mut close = false;
                ui.horizontal(|ui| {
                    let ok = !busy && !new.trim().is_empty() && new.trim() != a.url;
                    if (primary_button(ui, "Use this link", ok).clicked() || enter) && ok {
                        ask = Some(Ask::Relink(new.trim().to_owned()));
                    }
                    if small_button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
                if close {
                    *relink = None;
                }
            }
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
                        ask = Some(Ask::Remove);
                        *confirm = None;
                    }
                    if small_button(ui, "Cancel").clicked() {
                        *confirm = None;
                    }
                });
            }
        });
    ask
}
