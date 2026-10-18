//! "What's new", shown once after the app updates itself (or is updated).
//! Only the big things go in [`NOTES`]; the full list is in the release notes.
//! Admin features stay out: they're for the server's admin, who knows about them.
use eframe::egui::{self, RichText};

use super::settings::{dialog, header};
use super::{bold, primary_button, MUTED};
use crate::update::{newer, Release, VERSION};

struct Note {
    title: &'static str,
    text: &'static str,
}

/// Newest last. Add an entry when a release brings something users would
/// want to know about; leave small fixes and admin features out.
const NOTES: &[(&str, &[Note])] = &[
    (
        "0.7.0",
        &[
            Note {
                title: "New arrivals",
                text: "When movies or series are added to your server, Valo shows them \
                   to you when you open the app or the library refreshes.",
            },
            Note {
                title: "Refresh any time",
                text: "Click \"Library updated …\" in the bottom-left corner to reload \
                   the library without restarting.",
            },
            Note {
                title: "A fresh home page",
                text: "The Movies, TV shows and genre rows show a new mix each time you \
                   open Valo, and a Top rated row shows the best scored titles.",
            },
            Note {
                title: "Your devices",
                text: "Your profile lists every device you're signed in on, and you can \
                   sign any of them out.",
            },
            Note {
                title: "Collections",
                text: "Film series like Batman stay together in A to Z, in the order your \
                   server's admin picked.",
            },
        ],
    ),
    (
        "0.8.0",
        &[
            Note {
                title: "Discord status",
                text: "Your Discord profile shows what you're watching and how much is left. \
                       Turn it off in Settings under Playback.",
            },
            Note {
                title: "Time left",
                text: "The player shows how long you've watched on the left and how much \
                       is left on the right.",
            },
        ],
    ),
    (
        "0.8.1",
        &[Note {
            title: "Remembers its window",
            text: "Valo opens the way you closed it: maximized, or at the same size \
                   and place.",
        }],
    ),
    (
        "0.8.2",
        &[
            Note {
                title: "Make it yours",
                text: "Settings > Appearance has app styles that lay Valo out like Netflix, \
                       Prime Video, Disney+, HBO Max, YouTube or Apple TV (home page, cards, \
                       details pages and player), and Light, Black, Dim and Midnight themes.",
            },
            Note {
                title: "Bookmark from anywhere",
                text: "Hover over a movie or series on the home page, Movies or TV shows and \
                       click the bookmark in its corner to add it to My list.",
            },
        ],
    ),
    (
        "0.8.3",
        &[Note {
            title: "Subtitles",
            text: "Pick subtitles from the subtitle button in the player (C turns them on \
                   and off). The language you pick comes on by itself next time.",
        }],
    ),
    (
        "0.8.4",
        &[
            Note {
                title: "Updates show what's new",
                text: "When a new version is out, a popup shows what it brings and updates \
                       Valo in one click. Click Later or outside it to update another time.",
            },
            Note {
                title: "Subtitles in sync",
                text: "With Bluetooth headphones, the picture and subtitles now wait for the \
                       sound instead of running a little ahead of it.",
            },
        ],
    ),
];

/// The notes for versions after `seen`, up to `current`.
fn between(seen: &str, current: &str) -> Vec<&'static Note> {
    NOTES
        .iter()
        .filter(|(v, _)| newer(v, seen) && !newer(v, current))
        .flat_map(|(_, notes)| notes.iter())
        .collect()
}

/// Whether there's anything to show for a user who last saw `seen`
/// (empty: a config from before this existed).
pub fn pending(seen: &str) -> bool {
    // every update gets the dialog: the notes, or at least a pointer to them
    seen != VERSION
}

/// Draw the dialog; true once it's dismissed.
pub fn ui(ctx: &egui::Context, seen: &str) -> bool {
    let notes = between(seen, VERSION);
    let (done, close) = dialog(ctx, "whats-new", 460.0, |ui| {
        header(ui, "What's new", &format!("Valo {VERSION}"));
        ui.add_space(6.0);
        for n in &notes {
            ui.label(bold(n.title, 16.0));
            ui.add_space(2.0);
            ui.label(RichText::new(n.text).size(14.5).color(MUTED()));
            ui.add_space(14.0);
        }
        if notes.is_empty() {
            ui.label(
                RichText::new("This update brings fixes and small improvements.")
                    .size(14.5)
                    .color(MUTED()),
            );
            ui.add_space(14.0);
        }
        if super::ext_link(ui, "Full release notes") {
            super::chrome::open_url(&format!(
                "https://github.com/Okaza03/Valo/releases/tag/v{VERSION}"
            ));
        }
        ui.add_space(10.0);
        let b = ui.add_sized([ui.available_width(), 40.0], |ui: &mut egui::Ui| {
            primary_button(ui, "Got it", true)
        });
        b.clicked()
    });
    done || close
}

/// What the "update available" popup asked for.
pub enum UpdateChoice {
    Update,
    Download,
    /// Closed: "Later", a click outside or Escape.
    Later,
}

/// The popup shown when a newer Valo is out: its release notes and a way to
/// get it. `in_place`: the button installs and restarts; else it downloads.
pub fn update_available(
    ctx: &egui::Context,
    rel: &Release,
    in_place: bool,
    flatpak: bool,
) -> Option<UpdateChoice> {
    let (choice, close) = dialog(ctx, "update-available", 460.0, |ui| {
        header(
            ui,
            "Update available",
            &format!("Valo {}  ·  you have {VERSION}", rel.version),
        );
        ui.add_space(6.0);
        if !rel.summary.is_empty() {
            ui.label(RichText::new(&rel.summary).size(15.0));
            ui.add_space(14.0);
        }
        for (title, text) in &rel.notes {
            if !title.is_empty() {
                ui.label(bold(title, 16.0));
                ui.add_space(2.0);
            }
            ui.label(RichText::new(text).size(14.5).color(MUTED()));
            ui.add_space(14.0);
        }
        if rel.summary.is_empty() && rel.notes.is_empty() {
            ui.label(
                RichText::new("This update brings fixes and small improvements.")
                    .size(14.5)
                    .color(MUTED()),
            );
            ui.add_space(14.0);
        }
        if flatpak {
            ui.label(
                RichText::new("Download the new .flatpak and open it to update.")
                    .size(14.5)
                    .color(MUTED()),
            );
            ui.add_space(14.0);
        }
        if super::ext_link(ui, "Full release notes") {
            super::chrome::open_url(&rel.url);
        }
        ui.add_space(10.0);
        let label = if in_place { "Update now" } else { "Download" };
        let full = [ui.available_width(), 40.0];
        let mut choice = None;
        if ui
            .add_sized(full, |ui: &mut egui::Ui| primary_button(ui, label, true))
            .clicked()
        {
            choice = Some(if in_place {
                UpdateChoice::Update
            } else {
                UpdateChoice::Download
            });
        }
        ui.add_space(6.0);
        if ui
            .add_sized(full, |ui: &mut egui::Ui| super::small_button(ui, "Later"))
            .clicked()
        {
            choice = Some(UpdateChoice::Later);
        }
        choice
    });
    choice.or(close.then_some(UpdateChoice::Later))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_ordered_and_picked_by_version() {
        assert!(NOTES.windows(2).all(|w| newer(w[1].0, w[0].0)));
        // nothing before its release, nothing twice
        let all = between("0.6.1", "0.7.0");
        assert!(!all.is_empty());
        assert!(between("0.6.1", "0.6.1").is_empty());
        assert!(between("0.7.0", "0.7.0").is_empty());
        assert_eq!(between("", "0.7.0").len(), all.len());
        // an update always shows the dialog, even without notes of its own
        assert!(pending("0.0.1") && !pending(VERSION));
    }
}
