//! "What's new", shown once after the app updates itself (or is updated),
//! with the notes of every version since the one last seen.
//! Only the big things go in [`NOTES`]; the full list is in the release notes.
//! Admin features stay out: they're for the server's admin, who knows about them.
use eframe::egui::{self, RichText};

use super::settings::{dialog, header};
use super::{bold, mono, primary_button, ACCENT, MUTED, TEXT};
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
    (
        "0.8.6",
        &[Note {
            title: "Seasons in new arrivals",
            text: "A new series shows the seasons it came with, and when a new season is \
                   added to a series you already have, it shows up in new arrivals again.",
        }],
    ),
    (
        "0.8.7",
        &[Note {
            title: "Change your password",
            text: "When your server's admin asks you for a new password, Valo asks each \
                   time it opens until you change it, right there in the popup.",
        }],
    ),
    (
        "0.8.8",
        &[
            Note {
                title: "Streaming and requests",
                text: "If your server's admin has added Stremio addons, Home has rows like \
                       Popular and Netflix, and the new Streaming page shows what's on each \
                       service. Titles that aren't on the server can be requested.",
            },
            Note {
                title: "Search beyond the server",
                text: "Search also finds movies and series the server doesn't have, so you \
                       can open them and ask for them.",
            },
            Note {
                title: "Everything they've been in",
                text: "A person's page lists all their movies and series, and which of them \
                       are on the server.",
            },
            Note {
                title: "Subtitle timing",
                text: "Subtitles running early or late? Press Z or X, or use the subtitle \
                       menu, to move them.",
            },
        ],
    ),
    (
        "0.8.9",
        &[Note {
            title: "Your home page, your way",
            text: "Your server's admin can now choose which rows show on Home, and in \
                   what order.",
        }],
    ),
    (
        "0.8.11",
        &[Note {
            title: "Removed titles",
            text: "New arrivals also lists movies and series taken off the server, and \
                   the library reloads when you come back to Valo, so they don't linger.",
        }],
    ),
];

/// The notes for versions after `seen`, up to `current`, newest first.
fn between(seen: &str, current: &str) -> Vec<(&'static str, &'static [Note])> {
    NOTES
        .iter()
        .rev()
        .filter(|(v, _)| newer(v, seen) && !newer(v, current))
        .map(|(v, notes)| (*v, *notes))
        .collect()
}

/// "Valo 0.8.6", over a version's notes when there's more than one version.
fn version_heading(ui: &mut egui::Ui, v: &str) {
    ui.label(mono(format!("VALO {v}"), 11.0).color(ACCENT()));
    ui.add_space(6.0);
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
        for (v, list) in &notes {
            if notes.len() > 1 {
                version_heading(ui, v);
            }
            for n in list.iter() {
                ui.label(bold(n.title, 16.0));
                ui.add_space(2.0);
                ui.label(RichText::new(n.text).size(14.5).color(MUTED()));
                ui.add_space(14.0);
            }
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
        let many = rel.changes.len() > 1;
        if many {
            ui.label(
                RichText::new(format!(
                    "{} updates since your version, newest first.",
                    rel.changes.len()
                ))
                .size(14.5)
                .color(MUTED()),
            );
            ui.add_space(14.0);
        }
        for c in &rel.changes {
            if many {
                version_heading(ui, &c.version);
            }
            if !c.summary.is_empty() {
                ui.label(RichText::new(&c.summary).size(15.0).color(TEXT()));
                ui.add_space(14.0);
            }
            for (title, text) in &c.notes {
                if !title.is_empty() {
                    ui.label(bold(title, 16.0));
                    ui.add_space(2.0);
                }
                ui.label(RichText::new(text).size(14.5).color(MUTED()));
                ui.add_space(14.0);
            }
        }
        if rel
            .changes
            .iter()
            .all(|c| c.summary.is_empty() && c.notes.is_empty())
        {
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
        // several versions behind: every one of them, newest first
        let v: Vec<_> = between("0.8.2", "0.8.7").iter().map(|(v, _)| *v).collect();
        assert_eq!(v, ["0.8.7", "0.8.6", "0.8.4", "0.8.3"]);
        // an update always shows the dialog, even without notes of its own
        assert!(pending("0.0.1") && !pending(VERSION));
    }
}
