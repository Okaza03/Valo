//! Admin › Users: who can sign in, and who can manage the server.
use anyhow::Result;
use iced::alignment::Vertical;
use iced::widget::{checkbox, column, container, mouse_area, row, space, text, text_input, Column};
use iced::{Element, Padding};

use super::super::details::{caps, column_of, gap, panel, NARROW};
use super::super::profile::{field_style, section_title, Remote};
use super::super::{avatar, spin_secs, Msg as LMsg};
use super::{
    danger_button, more_menu, not_set_up, row_frame, Cx, Done, JobFn, Msg as AMsg, Section,
};
use crate::api::{AdminUser, Client};
use crate::iced_ui::spaced::mono;
use crate::iced_ui::style::{self, c, BOLD};
use crate::iced_ui::widgets::{self, Kind as Btn};
use crate::ui::browse::details::TEAL;
use crate::ui::browse::people::ago;

#[derive(Clone, Debug)]
pub enum Msg {
    /// Ask "delete this user?", or stop.
    ConfirmDelete(Option<String>),
    /// Start resetting a password, or stop.
    Reset(Option<String>),
    ResetText(String),
    /// Start changing a display name, or stop.
    Rename(Option<String>),
    RenameText(String),
    NewName(String),
    NewDisplay(String),
    NewPass(String),
    NewAdmin(bool),
    Add,
    SaveName,
    SavePassword,
    SetAdmin(String, bool),
    RequirePassword(String, bool),
    Logout(String),
    Delete(String),
    /// Their page under Analytics.
    Open(String),
}

pub enum Out {
    None,
    Run(JobFn),
    Profile(String),
}

#[derive(Default)]
pub struct State {
    list: Remote<Vec<AdminUser>>,
    confirm_delete: Option<String>,
    reset: Option<(String, String)>,
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

fn run(r: Req) -> Out {
    Out::Run(Box::new(move |c: &Client| r.run(c)))
}

impl State {
    pub fn refresh(&mut self) {
        self.list.retry();
    }

    pub fn missing(&self) -> bool {
        self.list.missing
    }

    pub fn busy(&self) -> bool {
        self.list.loading()
    }

    pub fn list(&self) -> &[AdminUser] {
        self.list.data.as_deref().unwrap_or_default()
    }

    pub fn count(&self) -> Option<usize> {
        self.list.data.as_ref().map(Vec::len)
    }

    /// Keep the list loaded, whichever section is open.
    pub fn work(&mut self, client: &Client, login: &mut bool) {
        if self.list.poll(login) {
            if let Some(u) = self.list.data.as_mut() {
                u.sort_by_key(|u| (!u.admin, u.name.to_lowercase()));
            }
        }
        // fresh enough to tell who's active now
        if self.list.idle() || (self.list.data.is_some() && self.list.stale(60)) {
            let c = client.clone();
            self.list.load(move || c.admin_users());
        }
    }

    pub fn update(&mut self, m: Msg) -> Out {
        match m {
            Msg::ConfirmDelete(n) => self.confirm_delete = n,
            Msg::Reset(n) => self.reset = n.map(|n| (n, String::new())),
            Msg::ResetText(s) => {
                if let Some((_, p)) = self.reset.as_mut() {
                    *p = s;
                }
            }
            Msg::Rename(n) => {
                self.rename = n.map(|n| {
                    let d = self
                        .list()
                        .iter()
                        .find(|u| u.name == n)
                        .and_then(|u| u.display_name.clone())
                        .unwrap_or_else(|| n.clone());
                    (n, d)
                });
            }
            Msg::RenameText(s) => {
                if let Some((_, d)) = self.rename.as_mut() {
                    *d = s;
                }
            }
            Msg::NewName(s) => self.new_name = s,
            Msg::NewDisplay(s) => self.new_display = s,
            Msg::NewPass(s) => self.new_pass = s,
            Msg::NewAdmin(v) => self.new_admin = v,
            Msg::Add => {
                if self.new_name.trim().is_empty() || self.new_pass.is_empty() {
                    return Out::None;
                }
                let r = Req::Add(
                    self.new_name.trim().to_owned(),
                    std::mem::take(&mut self.new_pass),
                    self.new_admin,
                    std::mem::take(&mut self.new_display),
                );
                self.new_name.clear();
                self.new_admin = false;
                return run(r);
            }
            Msg::SaveName => {
                if let Some((n, d)) = self.rename.take().filter(|(_, d)| !d.trim().is_empty()) {
                    return run(Req::Rename(n, d));
                }
            }
            Msg::SavePassword => {
                if let Some((n, p)) = self.reset.take().filter(|(_, p)| !p.is_empty()) {
                    return run(Req::Password(n, p));
                }
            }
            Msg::SetAdmin(n, on) => return run(Req::SetAdmin(n, on)),
            Msg::RequirePassword(n, on) => return run(Req::RequirePassword(n, on)),
            Msg::Logout(n) => return run(Req::Logout(n)),
            Msg::Delete(n) => {
                self.confirm_delete = None;
                return run(Req::Delete(n));
            }
            Msg::Open(n) => return Out::Profile(n),
        }
        Out::None
    }

    pub fn view<'a>(&'a self, cx: &Cx<'a>, w: f32) -> Element<'a, LMsg> {
        let (m, inner) = column_of(w);
        if self.list.missing {
            return container(not_set_up(Section::Users, &cx.client.info.name))
                .padding(Padding::default().left(m))
                .width(m + inner)
                .into();
        }
        let send = |m: Msg| LMsg::Admin(AMsg::Users(m));
        let narrow = inner < NARROW;
        let g = 48.0;
        let lw = if narrow { inner } else { inner * 0.66 };
        let rw = if narrow { inner } else { inner - lw - g };
        let n = self.list.data.as_ref().map_or(0, Vec::len);
        let mut main: Column<'a, LMsg> = column![section_title("Accounts", "Users", n)].width(lw);
        if let Some(e) = &self.list.error {
            main = main.push(
                text(format!("Couldn't load the users: {e}"))
                    .size(14.5)
                    .color(style::ERROR()),
            );
        }
        match self.list.data.as_ref() {
            Some(users) => {
                for u in users {
                    let you = u.name.to_lowercase() == cx.me;
                    main = main.push(self.user_row(u, you, cx)).push(gap(8.0));
                }
            }
            None => {
                if self.list.loading() {
                    main = main.push(crate::iced_ui::icon::spinner(
                        spin_secs(),
                        style::MUTED(),
                        20.0,
                    ));
                }
            }
        }
        let busy = cx.busy;
        let ok = !busy && !self.new_name.trim().is_empty() && !self.new_pass.is_empty();
        let field = |v: &str, secure: bool, on: fn(String) -> Msg, submit: bool| {
            let mut f = text_input("", v)
                .secure(secure)
                .on_input(move |s| LMsg::Admin(AMsg::Users(on(s))))
                .size(15)
                .padding([9, 10])
                .style(field_style);
            if submit {
                f = f.on_submit(LMsg::Admin(AMsg::Users(Msg::Add)));
            }
            f
        };
        let add = panel(column![
            caps("Add a user", c(TEAL())),
            gap(10.0),
            widgets::label("Username", 11.0),
            space().height(8),
            field(&self.new_name, false, Msg::NewName, false),
            gap(4.0),
            widgets::label("Display name (optional)", 11.0),
            space().height(8),
            field(&self.new_display, false, Msg::NewDisplay, false),
            gap(4.0),
            widgets::label("Password", 11.0),
            space().height(8),
            field(&self.new_pass, true, Msg::NewPass, true),
            gap(4.0),
            checkbox(self.new_admin)
                .label("Admin (can manage the server)")
                .on_toggle(|v| LMsg::Admin(AMsg::Users(Msg::NewAdmin(v))))
                .size(16)
                .text_size(14.5),
            gap(10.0),
            widgets::button_wide(Btn::Primary, "Add user", 38.0, ok.then(|| send(Msg::Add))),
        ]);
        let side = container(add).width(rw);
        let body: Element<'a, LMsg> = if narrow {
            column![main, gap(24.0), side].into()
        } else {
            row![main, space().width(g), side].into()
        };
        container(body).padding(Padding::default().left(m)).into()
    }

    /// One user: name, role, last activity, and what can be done to them.
    /// Clicking their name opens their page.
    fn user_row<'a>(&'a self, u: &AdminUser, you: bool, cx: &Cx<'a>) -> Element<'a, LMsg> {
        let busy = cx.busy;
        let send = |m: Msg| LMsg::Admin(AMsg::Users(m));
        let shown = u
            .display_name
            .as_deref()
            .filter(|d| !d.is_empty() && *d != u.name);
        let mut name = row![text(shown.unwrap_or(&u.name).to_owned())
            .font(BOLD)
            .size(15)]
        .spacing(8)
        .align_y(Vertical::Center);
        if shown.is_some() {
            name = name.push(mono(&format!("@{}", u.name), 10.0).color(style::MUTED()));
        }
        if you {
            name = name.push(mono("You", 10.0).color(c(TEAL())));
        }
        if u.active_now() {
            // used Valo in the last five minutes
            let ok = c(crate::ui::theme::OK());
            name = name.push(
                row![super::status_dot(ok), mono("Active now", 10.0).color(ok)]
                    .spacing(5)
                    .align_y(Vertical::Center),
            );
        }
        let who = crate::iced_ui::chrome::tip_below(
            mouse_area(
                row![
                    avatar(&u.name, 17.0),
                    column![name, mono(&user_line(u), 10.5).color(style::MUTED())].spacing(2)
                ]
                .spacing(14)
                .align_y(Vertical::Center),
            )
            .on_press(send(Msg::Open(u.name.clone())))
            .interaction(iced::mouse::Interaction::Pointer),
            &format!("Open {}'s page", u.name),
        );
        let key = format!("user\u{1f}{}", u.name);
        let mut items = vec![
            (
                "Change display name",
                send(Msg::Rename(Some(u.name.clone()))),
            ),
            ("Reset password", send(Msg::Reset(Some(u.name.clone())))),
            (
                if u.must_change_password {
                    "Don't require a new password"
                } else {
                    "Require a new password"
                },
                send(Msg::RequirePassword(
                    u.name.clone(),
                    !u.must_change_password,
                )),
            ),
        ];
        // your own sessions or account: you'd lock yourself out
        if !you {
            items.push(("Sign out on all devices", send(Msg::Logout(u.name.clone()))));
            items.push((
                "Delete user…",
                send(Msg::ConfirmDelete(Some(u.name.clone()))),
            ));
        }
        let mut actions = row![].spacing(8).align_y(Vertical::Center);
        if !you {
            let label = if u.admin {
                "Remove admin"
            } else {
                "Make admin"
            };
            actions = actions.push(widgets::button_with(
                Btn::Small,
                label,
                None,
                (!busy).then(|| send(Msg::SetAdmin(u.name.clone(), !u.admin))),
            ));
        }
        actions = actions.push(more_menu(
            key.clone(),
            cx.menu == Some(key.as_str()),
            items,
            busy,
        ));
        let mut body: Column<'a, LMsg> =
            column![row![who, space::horizontal(), actions].align_y(Vertical::Center)];

        if self.confirm_delete.as_deref() == Some(u.name.as_str()) {
            body = body
                .push(gap(10.0))
                .push(
                    text(format!(
                        "Delete {}? They're signed out everywhere and their watch \
                         progress on the server is removed.",
                        u.name
                    ))
                    .size(14.5),
                )
                .push(gap(6.0))
                .push(
                    row![
                        danger_button("Delete", (!busy).then(|| send(Msg::Delete(u.name.clone())))),
                        widgets::button_with(
                            Btn::Small,
                            "Cancel",
                            None,
                            Some(send(Msg::ConfirmDelete(None)))
                        ),
                    ]
                    .spacing(10),
                );
        }
        if let Some((_, display)) = self.rename.as_ref().filter(|(n, _)| *n == u.name) {
            let ok = !busy && !display.trim().is_empty();
            body = body
                .push(gap(10.0))
                .push(widgets::label("Display name", 11.0))
                .push(space().height(8))
                .push(
                    text_input("", display)
                        .on_input(|s| LMsg::Admin(AMsg::Users(Msg::RenameText(s))))
                        .on_submit(send(Msg::SaveName))
                        .size(15)
                        .padding([9, 10])
                        .style(field_style),
                )
                .push(gap(6.0))
                .push(
                    row![
                        widgets::button_with(
                            Btn::Primary,
                            "Save name",
                            None,
                            ok.then(|| send(Msg::SaveName))
                        ),
                        widgets::button_with(
                            Btn::Small,
                            "Cancel",
                            None,
                            Some(send(Msg::Rename(None)))
                        ),
                    ]
                    .spacing(10),
                );
        }
        if let Some((_, pass)) = self.reset.as_ref().filter(|(n, _)| *n == u.name) {
            let ok = !busy && !pass.is_empty();
            body = body
                .push(gap(10.0))
                .push(widgets::label("New password", 11.0));
            if you {
                body = body.push(
                    text("You'll be signed out everywhere, this device included.")
                        .size(12)
                        .color(style::MUTED()),
                );
            }
            body = body
                .push(space().height(8))
                .push(
                    text_input("", pass)
                        .secure(true)
                        .on_input(|s| LMsg::Admin(AMsg::Users(Msg::ResetText(s))))
                        .on_submit(send(Msg::SavePassword))
                        .size(15)
                        .padding([9, 10])
                        .style(field_style),
                )
                .push(gap(6.0))
                .push(
                    row![
                        widgets::button_with(
                            Btn::Primary,
                            "Save password",
                            None,
                            ok.then(|| send(Msg::SavePassword))
                        ),
                        widgets::button_with(
                            Btn::Small,
                            "Cancel",
                            None,
                            Some(send(Msg::Reset(None)))
                        ),
                    ]
                    .spacing(10),
                );
        }
        row_frame(body)
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
    match u.last_active {
        _ if u.active_now() => {}
        Some(t) => parts.push(format!("active {}", ago(t))),
        None => {}
    }
    parts.join("   ·   ")
}
