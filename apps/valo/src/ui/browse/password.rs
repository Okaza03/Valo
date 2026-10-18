//! "Change your password", shown each time the app opens while an admin
//! asks for a new one (`must_change_password` from `GET /api/me`). Later
//! closes it until the next start.
use anyhow::Result;
use eframe::egui::{self, Color32, RichText, Stroke};

use super::details::{caps, TEAL};
use super::Browse;
use crate::api;
use crate::ui::{bold, mono, primary_button, small_button, text_field};
use crate::ui::{Action, BG, ERROR, LINE, MUTED, OK};
use crate::util::Task;

#[derive(Default)]
pub(super) struct Prompt {
    /// Showing the dialog.
    pub open: bool,
    current: String,
    new: String,
    again: String,
    task: Option<Task<Result<()>>>,
    error: Option<String>,
    /// The new password is saved: say so, then close.
    changed: bool,
}

impl Browse {
    pub(super) fn password_ui(&mut self, ctx: &egui::Context, allowed: bool, action: &mut Action) {
        let p = &mut self.password;
        if let Some(r) = p.task.as_mut().and_then(|t| t.poll()) {
            p.task = None;
            match r {
                Ok(()) => {
                    p.changed = true;
                    p.error = None;
                    p.current.clear();
                    p.new.clear();
                    p.again.clear();
                }
                Err(e) if e.is::<api::Unauthorized>() => {
                    p.open = false;
                    *action = Action::NeedLogin(self.client.clone());
                }
                Err(e) => p.error = Some(format!("{e:#}")),
            }
        }
        if !p.open || !allowed {
            return;
        }
        let frame = egui::Frame::new()
            .fill(BG())
            .stroke(Stroke::new(1.0f32, LINE()))
            .corner_radius(4)
            .inner_margin(26);
        let server = self.client.info.name.clone();
        let mut close = false;
        let mut submit = None;
        let r = egui::Modal::new(egui::Id::new("change-password"))
            .frame(frame)
            .backdrop_color(Color32::from_black_alpha(160))
            .show(ctx, |ui| {
                ui.set_width(420.0f32.min(ctx.screen_rect().width() - 80.0));
                if p.changed {
                    caps(ui, "Done", OK());
                    ui.add_space(4.0);
                    ui.label(bold("Password changed", 24.0));
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(
                            "Use it the next time you sign in. Your other devices were signed out.",
                        )
                        .color(MUTED()),
                    );
                    ui.add_space(16.0);
                    if primary_button(ui, "Close", true).clicked() {
                        close = true;
                    }
                    return;
                }
                caps(ui, "Your account", TEAL());
                ui.add_space(4.0);
                ui.label(bold("Change your password", 24.0));
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!(
                        "An admin of {server} asked you to pick a new password. \
                         Until you do, this shows each time you open Valo."
                    ))
                    .color(MUTED()),
                );
                ui.add_space(14.0);
                ui.label(mono("Current password", 11.0).color(MUTED()));
                text_field(ui, &mut p.current, "", true);
                ui.add_space(4.0);
                ui.label(mono("New password", 11.0).color(MUTED()));
                text_field(ui, &mut p.new, "", true);
                ui.add_space(4.0);
                ui.label(mono("New password again", 11.0).color(MUTED()));
                let last = text_field(ui, &mut p.again, "", true);
                let mismatch = !p.again.is_empty() && p.new != p.again;
                if mismatch {
                    ui.label(
                        RichText::new("The new passwords don't match.")
                            .small()
                            .color(ERROR()),
                    );
                }
                if let Some(e) = &p.error {
                    ui.add_space(4.0);
                    ui.label(RichText::new(e).color(ERROR()));
                }
                ui.add_space(14.0);
                let busy = p.task.is_some();
                let ok = !busy && !p.current.is_empty() && !p.new.is_empty() && p.new == p.again;
                let enter = last.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    if primary_button(ui, "Change password", ok).clicked() || (ok && enter) {
                        submit = Some((p.current.clone(), p.new.clone()));
                    }
                    if small_button(ui, "Later").clicked() {
                        close = true;
                    }
                    if busy {
                        ui.add(egui::Spinner::new().size(16.0).color(MUTED()));
                    }
                });
            });
        if let Some((current, new)) = submit {
            p.error = None;
            let c = self.client.clone();
            p.task = Some(Task::spawn(ctx, move || c.change_password(&current, &new)));
        }
        // Escape or a click outside counts as Later
        if close || (r.should_close() && p.task.is_none()) {
            p.open = false;
            p.changed = false;
            p.error = None;
        }
    }
}
