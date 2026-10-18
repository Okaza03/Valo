//! Admin › Users: who can sign in, and who can manage the server.
use anyhow::Result;
use eframe::egui::{self, vec2, Align, Color32, Layout, RichText, Sense, Stroke, Ui};

use super::super::details::{caps, panel, TEAL};
use super::super::people::{ago, columns_with, section_title};
use super::{not_set_up, Cx, Done, Remote, Section};
use crate::api::{AdminUser, Client};
use crate::ui::chrome;
use crate::ui::{bold, menu_item, mono, primary_button, small_button, text_field};
use crate::ui::{ERROR, LINE, MUTED, TEXT};

#[derive(Default)]
pub(in super::super) struct State {
    list: Remote<Vec<AdminUser>>,
    /// Asking "delete this user?".
    confirm_delete: Option<String>,
    /// Resetting this user's password, to this.
    reset: Option<(String, String)>,
    /// Changing this user's display name, to this.
    rename: Option<(String, String)>,
    new_name: String,
    new_display: String,
    new_pass: String,
    new_admin: bool,
}

/// A change to a user, run in the background.
enum Req {
    Add(String, String, bool, String),
    Rename(String, String),
    SetAdmin(String, bool),
    Password(String, String),
    RequirePassword(String, bool),
    Logout(String),
    Delete(String),
}

impl Req {
    fn run(self, c: &Client) -> Result<Done> {
        Ok(Done::msg(match self {
            Req::Add(name, pass, admin, display) => {
                c.admin_add_user(&name, &pass, admin, Some(&display))?;
                format!("Added {name}.")
            }
            Req::Rename(name, display) => {
                let body = serde_json::json!({ "display_name": display.trim() });
                c.admin_call(
                    "PATCH",
                    &format!("users/{}", crate::api::encode_segment(&name)),
                    Some(&body),
                )?;
                format!("{name} is now shown as {}.", display.trim())
            }
            Req::SetAdmin(name, admin) => {
                c.admin_update_user(&name, None, Some(admin))?;
                let what = if admin {
                    "is now an admin"
                } else {
                    "is no longer an admin"
                };
                format!("{name} {what}.")
            }
            Req::Password(name, pass) => {
                c.admin_update_user(&name, Some(&pass), None)?;
                format!("Changed {name}'s password.")
            }
            Req::RequirePassword(name, on) => {
                c.admin_require_password_change(&name, on)?;
                if on {
                    format!(
                        "{name} will be asked to change their password when they next open Valo."
                    )
                } else {
                    format!("{name} no longer has to change their password.")
                }
            }
            Req::Logout(name) => {
                c.admin_logout_user(&name)?;
                format!("Signed {name} out on every device.")
            }
            Req::Delete(name) => {
                c.admin_delete_user(&name)?;
                format!("Deleted {name}.")
            }
        }))
    }
}

impl State {
    pub fn refresh(&mut self) {
        self.list.retry();
    }

    pub fn missing(&self) -> bool {
        self.list.missing
    }

    pub fn count(&self) -> Option<usize> {
        self.list.data.as_ref().map(Vec::len)
    }

    /// Keep the list loaded, whichever section is open.
    pub fn poll_and_load(&mut self, ctx: &egui::Context, client: &Client, login: &mut bool) {
        if self.list.poll(login) {
            if let Some(u) = self.list.data.as_mut() {
                u.sort_by_key(|u| (!u.admin, u.name.to_lowercase()));
            }
        }
        if self.list.idle() || (self.list.data.is_some() && self.list.stale(u64::MAX)) {
            let c = client.clone();
            self.list.load(ctx, move || c.admin_users());
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        if self.list.missing {
            super::super::details::padded(ui, |ui| {
                if not_set_up(ui, Section::Users, &cx.client.info.name) {
                    self.list.retry();
                }
            });
            return;
        }
        let busy = cx.busy;
        let mut req = None;
        let State {
            list,
            confirm_delete,
            reset,
            rename,
            new_name,
            new_display,
            new_pass,
            new_admin,
        } = self;
        columns_with(
            ui,
            &mut req,
            |ui, req| {
                let n = list.data.as_ref().map_or(0, Vec::len);
                section_title(ui, "Accounts", "Users", n);
                if let Some(e) = list.error.as_ref() {
                    ui.label(RichText::new(format!("Couldn't load the users: {e}")).color(ERROR()));
                }
                let Some(users) = list.data.as_ref() else {
                    if list.loading() {
                        ui.add(egui::Spinner::new().size(20.0).color(MUTED()));
                    }
                    return;
                };
                for u in users {
                    let you = u.name.to_lowercase() == cx.me;
                    if let Some(r) = user_row(ui, u, you, busy, confirm_delete, reset, rename) {
                        *req = Some(r);
                    }
                    ui.add_space(8.0);
                }
            },
            |ui, req| {
                panel(ui, |ui| {
                    caps(ui, "Add a user", TEAL());
                    ui.add_space(10.0);
                    ui.label(mono("Username", 11.0).color(MUTED()));
                    text_field(ui, new_name, "", false);
                    ui.add_space(4.0);
                    ui.label(mono("Display name (optional)", 11.0).color(MUTED()));
                    text_field(ui, new_display, "", false);
                    ui.add_space(4.0);
                    ui.label(mono("Password", 11.0).color(MUTED()));
                    let p = text_field(ui, new_pass, "", true);
                    ui.add_space(4.0);
                    ui.checkbox(new_admin, "Admin (can manage the server)");
                    ui.add_space(10.0);
                    let ok = !busy && !new_name.trim().is_empty() && !new_pass.is_empty();
                    let enter = p.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let b = ui.add_sized([ui.available_width(), 38.0], |ui: &mut Ui| {
                        primary_button(ui, "Add user", ok)
                    });
                    if ok && (b.clicked() || enter) {
                        *req = Some(Req::Add(
                            new_name.trim().to_owned(),
                            std::mem::take(new_pass),
                            *new_admin,
                            std::mem::take(new_display),
                        ));
                        new_name.clear();
                        *new_admin = false;
                    }
                });
            },
        );
        if let Some(r) = req {
            cx.run(move |c| r.run(c));
        }
    }
}

/// "Admin", "2 devices", "active 3 days ago".
fn user_line(u: &AdminUser) -> String {
    let mut parts = vec![if u.admin { "Admin" } else { "User" }.to_owned()];
    match u.sessions {
        Some(1) => parts.push("1 device".into()),
        Some(n) => parts.push(format!("{n} devices")),
        None => {}
    }
    if u.must_change_password {
        parts.push("must change password".into());
    }
    if let Some(t) = u.last_active {
        parts.push(format!("active {}", ago(t)));
    }
    parts.join("   ·   ")
}

pub(super) fn danger_button(ui: &mut Ui, text: &str) -> egui::Response {
    let b = egui::Button::new(mono(text, 11.5).color(ERROR()))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0f32, ERROR().gamma_multiply(0.6)))
        .min_size(vec2(0.0, 30.0))
        .corner_radius(2);
    ui.add(b)
}

/// One user: name, role, last activity, and what can be done to them.
fn user_row(
    ui: &mut Ui,
    u: &AdminUser,
    you: bool,
    busy: bool,
    confirm_delete: &mut Option<String>,
    reset: &mut Option<(String, String)>,
    rename: &mut Option<(String, String)>,
) -> Option<Req> {
    let mut req = None;
    egui::Frame::new()
        .stroke(Stroke::new(1.0f32, LINE()))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
                chrome::avatar(ui.painter(), r.center(), 17.0, &u.name);
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.horizontal(|ui| {
                        let shown = u
                            .display_name
                            .as_deref()
                            .filter(|d| !d.is_empty() && *d != u.name);
                        ui.label(bold(shown.unwrap_or(&u.name), 15.0));
                        if shown.is_some() {
                            ui.label(mono(format!("@{}", u.name), 10.0).color(MUTED()));
                        }
                        if you {
                            ui.label(mono("YOU", 10.0).color(TEAL()));
                        }
                    });
                    ui.label(mono(user_line(u), 10.5).color(MUTED()));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        let more =
                            crate::ui::small_icon_button(ui, crate::ui::chrome::Icon::More, "More");
                        let id = ui.make_persistent_id(("admin-user-menu", &u.name));
                        if more.clicked() {
                            ui.memory_mut(|m| m.toggle_popup(id));
                        }
                        egui::popup::popup_below_widget(
                            ui,
                            id,
                            &more,
                            egui::PopupCloseBehavior::CloseOnClickOutside,
                            |ui| {
                                ui.set_min_width(220.0);
                                if menu_item(ui, "Change display name") {
                                    let d =
                                        u.display_name.clone().unwrap_or_else(|| u.name.clone());
                                    *rename = Some((u.name.clone(), d));
                                    ui.memory_mut(|m| m.close_popup());
                                }
                                if menu_item(ui, "Reset password") {
                                    *reset = Some((u.name.clone(), String::new()));
                                    ui.memory_mut(|m| m.close_popup());
                                }
                                let label = if u.must_change_password {
                                    "Don't require a new password"
                                } else {
                                    "Require a new password"
                                };
                                if menu_item(ui, label) {
                                    req = Some(Req::RequirePassword(
                                        u.name.clone(),
                                        !u.must_change_password,
                                    ));
                                    ui.memory_mut(|m| m.close_popup());
                                }
                                // your own sessions or account: you'd lock yourself out
                                if !you && menu_item(ui, "Sign out on all devices") {
                                    req = Some(Req::Logout(u.name.clone()));
                                    ui.memory_mut(|m| m.close_popup());
                                }
                                if !you && menu_item(ui, "Delete user…") {
                                    *confirm_delete = Some(u.name.clone());
                                    ui.memory_mut(|m| m.close_popup());
                                }
                            },
                        );
                        if !you {
                            let label = if u.admin {
                                "Remove admin"
                            } else {
                                "Make admin"
                            };
                            if small_button(ui, label).clicked() {
                                req = Some(Req::SetAdmin(u.name.clone(), !u.admin));
                            }
                        }
                    });
                });
            });

            if confirm_delete.as_deref() == Some(u.name.as_str()) {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(format!(
                        "Delete {}? They're signed out everywhere and their watch \
                         progress on the server is removed.",
                        u.name
                    ))
                    .color(TEXT()),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled_ui(!busy, |ui| danger_button(ui, "Delete"))
                        .inner
                        .clicked()
                    {
                        req = Some(Req::Delete(u.name.clone()));
                        *confirm_delete = None;
                    }
                    if small_button(ui, "Cancel").clicked() {
                        *confirm_delete = None;
                    }
                });
            }

            if let Some((name, display)) = rename.as_mut().filter(|(n, _)| *n == u.name) {
                ui.add_space(10.0);
                ui.label(mono("Display name", 11.0).color(MUTED()));
                let f = text_field(ui, display, "", false);
                ui.add_space(6.0);
                let enter = f.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let mut close = false;
                ui.horizontal(|ui| {
                    let ok = !busy && !display.trim().is_empty();
                    if primary_button(ui, "Save name", ok).clicked() || (ok && enter) {
                        req = Some(Req::Rename(name.clone(), display.clone()));
                        close = true;
                    }
                    if small_button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
                if close {
                    *rename = None;
                }
            }

            if let Some((name, pass)) = reset.as_mut().filter(|(n, _)| *n == u.name) {
                ui.add_space(10.0);
                ui.label(mono("New password", 11.0).color(MUTED()));
                if you {
                    ui.label(
                        RichText::new("You'll be signed out everywhere, this device included.")
                            .small()
                            .color(MUTED()),
                    );
                }
                let f = text_field(ui, pass, "", true);
                // read Enter before focusing the field again, or it never counts
                let enter = f.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if !enter && ui.memory(|m| m.focused().is_none()) {
                    f.request_focus();
                }
                ui.add_space(6.0);
                let mut close = false;
                ui.horizontal(|ui| {
                    let ok = !busy && !pass.is_empty();
                    if primary_button(ui, "Save password", ok).clicked() || (ok && enter) {
                        req = Some(Req::Password(name.clone(), std::mem::take(pass)));
                        close = true;
                    }
                    if small_button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
                if close {
                    *reset = None;
                }
            }
        });
    req
}
