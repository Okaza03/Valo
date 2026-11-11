// no console window behind the app on Windows (debug builds keep it for logs)
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod addons;
mod api;
mod audio;
mod config;
mod filmography;
mod fullscreen;
mod iced_ui;
mod inhibit;
mod integrate;
mod net;
mod player;
mod presence;
mod specs;
mod subs;
mod sync;
mod ui;
mod update;
mod util;

use std::path::PathBuf;

fn main() {
    // video files on the command line play right away
    let paths: Vec<PathBuf> = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .collect();
    if let Err(e) = iced_ui::run(paths) {
        eprintln!("valo: {e}");
        std::process::exit(1);
    }
}

/// Names the .desktop file the window's icon comes from: the Flatpak's renamed
/// copy of valo.desktop, the AppImage's own entry, or valo.desktop.
fn app_id() -> String {
    if let Ok(id) = std::env::var("FLATPAK_ID") {
        id
    } else if integrate::appimage().is_some() {
        integrate::APP_ID.into()
    } else {
        "valo".into()
    }
}

/// Ask for video files to play, starting in the Videos folder.
pub fn pick_files() -> Option<Vec<PathBuf>> {
    let home = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let start = std::env::var_os(home).map(|h| PathBuf::from(h).join("Videos"));
    let mut dlg = rfd::FileDialog::new()
        .add_filter("Video", api::VIDEO_EXTS)
        .add_filter("All files", &["*"]);
    if let Some(s) = start.filter(|s| s.is_dir()) {
        dlg = dlg.set_directory(s);
    }
    dlg.pick_files()
}
