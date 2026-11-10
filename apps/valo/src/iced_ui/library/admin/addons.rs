//! Admin › Addons: Stremio addons for every user. Catalogs become rows on
//! the home page, metadata fills in what titles are missing, and subtitles
//! show up in the player. The addons come from a list in the repo; admins
//! turn them on and off here, and ask for new ones with an issue.
use iced::alignment::Vertical;
use iced::widget::{checkbox, column, container, row, space, text, Column};
use iced::{Element, Length, Padding};

use super::super::details::{caps, column_of, gap, panel, NARROW};
use super::super::profile::{section_title, Remote};
use super::super::{spin_secs, Msg as LMsg};
use super::{danger_button, not_set_up, row_frame, Asks, Cx, Done, JobFn, Msg as AMsg, Section};
use crate::addons::Listed;
use crate::api::{Addon, Client, ADDON_REQUEST_URL};
use crate::iced_ui::spaced::mono;
use crate::iced_ui::style::{self, c, BOLD};
use crate::iced_ui::widgets::{self, Kind as Btn};
use crate::ui::browse::details::TEAL;
use crate::ui::browse::people::ago;

#[derive(Clone, Debug)]
pub enum Msg {
    /// Turn it on: (manifest URL, name).
    On(String, String),
    /// Turn it off: (id, name).
    Off(String, String),
    /// Ask before turning off an addon that can't be turned back on (or stop asking).
    ConfirmOff(Option<String>),
}

#[derive(Default)]
pub struct State {
    list: Remote<Vec<Addon>>,
    /// What can be turned on.
    offered: Remote<Vec<Listed>>,
    /// The id of an addon that can't be turned back on, being turned off.
    confirm_off: Option<String>,
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

    pub fn busy(&self) -> bool {
        self.list.loading() || self.offered.loading()
    }

    pub fn work(&mut self, client: &Client, asks: &mut Asks) {
        self.list.poll(&mut asks.login);
        self.offered.poll(&mut asks.login);
        if self.list.due() {
            let c = client.clone();
            self.list.load(move || c.admin_addons());
        }
        if self.offered.due() {
            self.offered.load(crate::addons::list);
        }
    }

    pub fn update(&mut self, m: Msg) -> Option<JobFn> {
        match m {
            Msg::ConfirmOff(id) => self.confirm_off = id,
            Msg::On(url, name) => {
                return Some(Box::new(move |c: &Client| {
                    c.admin_add_addon(&url)?;
                    Ok(Done::msg(format!("{name} is on for everyone now.")))
                }));
            }
            Msg::Off(id, name) => {
                self.confirm_off = None;
                return Some(Box::new(move |c: &Client| {
                    c.admin_delete_addon(&id)?;
                    Ok(Done::msg(format!("{name} is off.")))
                }));
            }
        }
        None
    }

    pub fn view<'a>(&'a self, cx: &Cx<'a>, w: f32) -> Element<'a, LMsg> {
        let (m, inner) = column_of(w);
        if self.list.missing {
            return container(not_set_up(Section::Addons, &cx.client.info.name))
                .padding(Padding::default().left(m))
                .width(m + inner)
                .into();
        }
        let narrow = inner < NARROW;
        let g = 48.0;
        let lw = if narrow { inner } else { inner * 0.66 };
        let rw = if narrow { inner } else { inner - lw - g };

        let shown = rows(
            self.offered.data.as_deref().unwrap_or_default(),
            self.list.data.as_deref().unwrap_or_default(),
        );
        let mut main: Column<'a, LMsg> =
            column![section_title("Stremio", "Addons", shown.len())].width(lw);
        if let Some(e) = &self.list.error {
            main = main.push(
                text(format!("Couldn't load the addons: {e}"))
                    .size(14.5)
                    .color(style::ERROR()),
            );
        }
        if let Some(e) = &self.offered.error {
            main = main.push(
                text(format!("Couldn't load the addon list: {e}"))
                    .size(14.5)
                    .color(style::ERROR()),
            );
        }
        let waiting =
            self.list.data.is_none() || (self.offered.loading() && self.offered.data.is_none());
        if waiting {
            if self.busy() {
                main = main.push(crate::iced_ui::icon::spinner(
                    spin_secs(),
                    style::MUTED(),
                    20.0,
                ));
            }
        } else {
            if shown.is_empty() {
                main = main.push(mono("No addons yet", 12.0).color(style::MUTED()));
            }
            // without the list, we can't tell what could be turned back on
            let locked = self.offered.data.is_none();
            for r in &shown {
                main = main.push(self.addon_row(r, cx.busy, locked)).push(gap(8.0));
            }
        }

        let mut help: Column<'a, LMsg> = column![
            caps("Missing an addon?", c(TEAL())),
            gap(10.0),
            widgets::primary(
                "Request an addon",
                Some(LMsg::OpenUrl(ADDON_REQUEST_URL.to_owned()))
            )
            .width(Length::Fill)
            .height(38),
            gap(14.0),
        ];
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
            help = help
                .push(text(line).size(14.5).color(style::MUTED()))
                .push(gap(6.0));
        }
        let side = container(panel(help)).width(rw);
        let body: Element<'a, LMsg> = if narrow {
            column![main, gap(24.0), side].into()
        } else {
            row![main, space().width(g), side].into()
        };
        container(body).padding(Padding::default().left(m)).into()
    }

    /// One addon: a checkbox to turn it on or off, name, and what it is.
    fn addon_row<'a>(&'a self, r: &Row, busy: bool, locked: bool) -> Element<'a, LMsg> {
        let send = |m: Msg| LMsg::Admin(AMsg::Addons(m));
        let can = !busy && (r.listed || !locked);
        let toggle = match r.on {
            None => send(Msg::On(r.url.to_owned(), r.name.to_owned())),
            Some(a) if r.listed => send(Msg::Off(a.id.clone(), r.name.to_owned())),
            Some(a) => send(Msg::ConfirmOff(Some(a.id.clone()))),
        };
        let tip = if r.on.is_some() {
            "Turn off"
        } else {
            "Turn on"
        };
        let check = crate::iced_ui::chrome::tip_below(
            checkbox(r.on.is_some())
                .on_toggle_maybe(can.then_some(move |_| toggle.clone()))
                .size(16),
            tip,
        );

        let mut name = row![text(r.name.to_owned()).font(BOLD).size(16)]
            .spacing(8)
            .align_y(Vertical::Center);
        let version = r.on.and_then(|a| a.version.as_ref());
        if let Some(v) = version.filter(|v| !v.is_empty()) {
            name = name.push(mono(&format!("v{v}"), 9.5).color(style::MUTED()));
        }
        if !r.listed && !locked {
            name = name.push(mono("Not on the list", 9.5).color(style::MUTED()));
        }
        let mut info: Column<'a, LMsg> = column![name].spacing(3);
        if let Some(a) = r.on {
            info = addon_details(info, a);
        }
        if let Some(d) = r.description.filter(|d| !d.trim().is_empty()) {
            info = info.push(text(d.trim().to_owned()).size(14.5));
        }
        let mut body: Column<'a, LMsg> =
            column![row![check, info].spacing(10).align_y(Vertical::Top)];
        let confirming =
            r.on.filter(|a| self.confirm_off.as_deref() == Some(a.id.as_str()));
        if let Some(a) = confirming {
            body = body
                .push(gap(10.0))
                .push(
                    text(format!(
                        "Turn off {}? It isn't on the list, so it can't be turned back on.",
                        r.name
                    ))
                    .size(14.5),
                )
                .push(gap(6.0))
                .push(
                    row![
                        danger_button(
                            "Turn off",
                            (!busy).then(|| send(Msg::Off(a.id.clone(), r.name.to_owned())))
                        ),
                        widgets::button_with(
                            Btn::Small,
                            "Cancel",
                            None,
                            Some(send(Msg::ConfirmOff(None)))
                        ),
                    ]
                    .spacing(8),
                );
        }
        row_frame(body)
    }
}

/// What the server knows about an addon that's on.
fn addon_details<'a>(mut col: Column<'a, LMsg>, a: &Addon) -> Column<'a, LMsg> {
    let offers = a.offers();
    if !offers.is_empty() {
        col = col.push(mono(&offers, 10.0).color(c(TEAL())));
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
        col = col.push(
            text(format!("Rows: {}", names.join(", ")))
                .size(12)
                .color(style::MUTED()),
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
        col = col.push(mono(&meta.join("   ·   "), 10.0).color(style::MUTED()));
    }
    col
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
