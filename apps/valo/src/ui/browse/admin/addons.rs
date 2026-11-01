//! Admin › Addons: Stremio addons for every user. Catalogs become rows on
//! the home page, metadata fills in what titles are missing, and subtitles
//! show up in the player. The addons come from a list in the repo; admins
//! turn them on and off here, and ask for new ones with an issue.
use eframe::egui::{self, Align, Layout, RichText, Stroke, Ui};

use super::super::details::{caps, panel, TEAL};
use super::super::people::{ago, columns_with, section_title};
use super::users::danger_button;
use super::{not_set_up, Cx, Done, Remote, Section};
use crate::addons::Listed;
use crate::api::{Addon, ADDON_REQUEST_URL};
use crate::ui::chrome;
use crate::ui::{bold, mono, primary_button, small_button};
use crate::ui::{ERROR, LINE, MUTED, TEXT};

#[derive(Default)]
pub(in super::super) struct State {
    pub list: Remote<Vec<Addon>>,
    /// What can be turned on.
    offered: Remote<Vec<Listed>>,
    /// The id of an addon that can't be turned back on, being turned off.
    confirm_off: Option<String>,
}

/// What the admin asked for on an addon's row.
enum Ask {
    /// Turn it on: (manifest URL, name).
    On(String, String),
    /// Turn it off: (id, name).
    Off(String, String),
}

/// One addon of the page.
struct Row<'a> {
    name: &'a str,
    url: &'a str,
    description: Option<&'a str>,
    /// Turned on, with what the server knows about it.
    on: Option<&'a Addon>,
    /// It's on the list, so it can be turned back on.
    listed: bool,
}

/// The list, then whatever else the server has: addons that left the list,
/// or that an admin added by hand.
fn rows<'a>(offered: &'a [Listed], on: &'a [Addon]) -> Vec<Row<'a>> {
    let listed = offered.iter().map(|l| Row {
        name: &l.name,
        url: &l.url,
        description: Some(&l.description),
        on: on.iter().find(|a| a.url == l.url),
        listed: true,
    });
    let others = on
        .iter()
        .filter(|a| !offered.iter().any(|l| l.url == a.url))
        .map(|a| Row {
            name: &a.name,
            url: &a.url,
            description: a.description.as_deref(),
            on: Some(a),
            listed: false,
        });
    listed.chain(others).collect()
}

impl State {
    pub fn refresh(&mut self) {
        self.list.retry();
        self.offered.retry();
    }

    pub fn missing(&self) -> bool {
        self.list.missing
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        self.list.poll(&mut cx.login);
        self.offered.poll(&mut cx.login);
        if self.list.due() {
            let c = cx.client.clone();
            self.list.load(&cx.ctx, move || c.admin_addons());
        }
        if self.offered.due() {
            self.offered.load(&cx.ctx, crate::addons::list);
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
        let mut asked = None;
        let State {
            list,
            offered,
            confirm_off,
        } = self;
        columns_with(
            ui,
            &mut asked,
            |ui, asked| {
                let shown = rows(
                    offered.data.as_deref().unwrap_or_default(),
                    list.data.as_deref().unwrap_or_default(),
                );
                section_title(ui, "Stremio", "Addons", shown.len());
                if let Some(e) = list.error.as_ref() {
                    ui.label(
                        RichText::new(format!("Couldn't load the addons: {e}")).color(ERROR()),
                    );
                }
                if let Some(e) = offered.error.as_ref() {
                    ui.label(
                        RichText::new(format!("Couldn't load the addon list: {e}")).color(ERROR()),
                    );
                }
                if list.data.is_none() || offered.loading() && offered.data.is_none() {
                    if list.loading() || offered.loading() {
                        ui.add(egui::Spinner::new().size(20.0).color(MUTED()));
                    }
                    return;
                }
                if shown.is_empty() {
                    ui.label(mono("No addons yet", 12.0).color(MUTED()));
                }
                // without the list, we can't tell what could be turned back on
                let locked = offered.data.is_none();
                for r in &shown {
                    if let Some(a) = addon_row(ui, r, busy, locked, confirm_off) {
                        *asked = Some(a);
                    }
                    ui.add_space(8.0);
                }
            },
            |ui, _| {
                panel(ui, |ui| {
                    caps(ui, "Missing an addon?", TEAL());
                    ui.add_space(10.0);
                    let b = ui.add_sized([ui.available_width(), 38.0], |ui: &mut Ui| {
                        primary_button(ui, "Request an addon", true)
                    });
                    if b.clicked() {
                        chrome::open_url(ADDON_REQUEST_URL);
                    }
                    ui.add_space(14.0);
                    for line in [
                        "Requests are GitHub issues. An addon that's approved shows up in \
                         this list for every server.",
                        "Catalogs become rows on everyone's home page. Titles that aren't \
                         on the server can be requested; they show up under Requests.",
                        "Metadata addons fill in what your titles are missing (posters, \
                         overviews, cast, …), and subtitle addons add subtitles to the player.",
                        "Titles are matched by IMDb id (Library › Fetch from IMDb). Everyone \
                         signed in can see the addon links.",
                    ] {
                        ui.label(RichText::new(line).color(MUTED()));
                        ui.add_space(6.0);
                    }
                });
            },
        );
        match asked {
            Some(Ask::On(url, name)) => cx.run(move |c| {
                c.admin_add_addon(&url)?;
                Ok(Done::msg(format!("{name} is on for everyone now.")))
            }),
            Some(Ask::Off(id, name)) => cx.run(move |c| {
                c.admin_delete_addon(&id)?;
                Ok(Done::msg(format!("{name} is off.")))
            }),
            None => {}
        }
    }
}

/// One addon: a checkbox to turn it on or off, name, and what it is.
fn addon_row(
    ui: &mut Ui,
    r: &Row,
    busy: bool,
    locked: bool,
    confirm: &mut Option<String>,
) -> Option<Ask> {
    let mut ask = None;
    egui::Frame::new()
        .stroke(Stroke::new(1.0f32, LINE()))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let mut on = r.on.is_some();
                let can = !busy && (r.listed || !locked);
                let tip = if on { "Turn off" } else { "Turn on" };
                let changed = ui
                    .add_enabled(can, egui::Checkbox::without_text(&mut on))
                    .on_hover_text(tip)
                    .changed();
                if changed {
                    match r.on {
                        None => ask = Some(Ask::On(r.url.to_owned(), r.name.to_owned())),
                        Some(a) if r.listed => {
                            ask = Some(Ask::Off(a.id.clone(), r.name.to_owned()));
                        }
                        Some(a) => *confirm = Some(a.id.clone()),
                    }
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    ui.horizontal(|ui| {
                        ui.label(bold(r.name, 16.0));
                        let version = r.on.and_then(|a| a.version.as_ref());
                        if let Some(v) = version.filter(|v| !v.is_empty()) {
                            ui.label(mono(format!("v{v}"), 9.5).color(MUTED()));
                        }
                        if !r.listed && !locked {
                            ui.label(mono("Not on the list", 9.5).color(MUTED()));
                        }
                    });
                    if let Some(a) = r.on {
                        addon_details(ui, a);
                    }
                    if let Some(d) = r.description.filter(|d| !d.trim().is_empty()) {
                        ui.label(RichText::new(d.trim()).color(TEXT()));
                    }
                });
            });
            let confirming = r.on.filter(|a| confirm.as_deref() == Some(a.id.as_str()));
            if let Some(a) = confirming {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(format!(
                        "Turn off {}? It isn't on the list, so it can't be turned back on.",
                        r.name
                    ))
                    .color(TEXT()),
                );
                ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                    if ui
                        .add_enabled_ui(!busy, |ui| danger_button(ui, "Turn off"))
                        .inner
                        .clicked()
                    {
                        ask = Some(Ask::Off(a.id.clone(), r.name.to_owned()));
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

/// What the server knows about an addon that's on.
fn addon_details(ui: &mut Ui, a: &Addon) {
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
    let mut meta = Vec::new();
    if let Some(b) = &a.added_by {
        meta.push(format!("turned on by {b}"));
    }
    if let Some(t) = a.added {
        meta.push(ago(t));
    }
    if !meta.is_empty() {
        ui.label(mono(meta.join("   ·   "), 10.0).color(MUTED()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(url: &str) -> Listed {
        Listed {
            name: url.to_owned(),
            url: url.to_owned(),
            description: "About it".to_owned(),
        }
    }

    fn addon(id: &str, url: &str) -> Addon {
        Addon {
            id: id.to_owned(),
            url: url.to_owned(),
            name: url.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn list_first_then_the_servers_others() {
        let offered = [
            listed("https://a/manifest.json"),
            listed("https://b/manifest.json"),
        ];
        let on = [
            addon("x", "https://old/manifest.json"),
            addon("y", "https://b/manifest.json"),
        ];
        let r = rows(&offered, &on);
        let got: Vec<_> = r
            .iter()
            .map(|r| (r.url, r.on.map(|a| a.id.as_str()), r.listed))
            .collect();
        assert_eq!(
            got,
            [
                ("https://a/manifest.json", None, true),
                ("https://b/manifest.json", Some("y"), true),
                ("https://old/manifest.json", Some("x"), false),
            ]
        );
    }
}
