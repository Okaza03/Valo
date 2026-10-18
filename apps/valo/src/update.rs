//! "Is there a newer Valo?" — asks GitHub for the latest release, and
//! when running as an AppImage or from the Windows installer, installs it in
//! place and restarts.
use std::fs;
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use eframe::egui::{self, Align, Align2, Layout, RichText};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::ui::{chrome, mono, small_button, BG, LINE, MUTED, TEXT};
use crate::util::Task;

const REPO: &str = "Okaza03/Valo";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct Release {
    pub version: String,
    pub url: String,
    /// (file name, download URL) of this machine's AppImage, or of the
    /// installer on Windows.
    package: Option<(String, String)>,
    /// Download URL of SHA256SUMS.txt.
    sums: Option<String>,
    /// The release notes' one-line summary.
    pub summary: String,
    /// Its "New"/"Fixed" points as (title, text); the title may be empty.
    pub notes: Vec<(String, String)>,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

/// How this copy was installed, which decides how it can be updated.
#[derive(Clone, PartialEq, Eq)]
pub enum Install {
    /// Replace the file and restart.
    AppImage(PathBuf),
    /// Installed by the Windows installer: run the new one, which replaces
    /// this copy and starts it again.
    WindowsSetup,
    /// Read-only sandbox: the user installs the new .flatpak.
    Flatpak,
    /// .deb, tarball, from source: send them to the release page.
    Other,
}

impl Install {
    pub fn detect() -> Self {
        if let Some(p) = crate::integrate::appimage() {
            Install::AppImage(p.into())
        } else if cfg!(windows) && installed_by_setup() {
            Install::WindowsSetup
        } else if std::env::var_os("FLATPAK_ID").is_some() {
            Install::Flatpak
        } else {
            Install::Other
        }
    }
}

/// The installer leaves its uninstaller next to the exe; the portable zip doesn't.
fn installed_by_setup() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join("unins000.exe").is_file()))
        .unwrap_or(false)
}

/// End of the name of the release file this platform updates itself from.
fn package_suffix() -> String {
    let arch = std::env::consts::ARCH;
    if cfg!(windows) {
        format!("-{arch}-setup.exe")
    } else {
        format!("-{arch}.AppImage")
    }
}

/// "v1.2.3-beta" -> [1, 2, 3]
fn parse(v: &str) -> Vec<u64> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let core = v.split(['-', '+']).next().unwrap_or(v);
    core.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

pub fn newer(latest: &str, current: &str) -> bool {
    let (mut a, mut b) = (parse(latest), parse(current));
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a > b
}

fn agent(timeout: Option<Duration>) -> ureq::Agent {
    let mut b = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent(concat!("Valo/", env!("CARGO_PKG_VERSION")));
    if let Some(t) = timeout {
        b = b.timeout(t);
    }
    b.build()
}

/// The latest release if it's newer than this build, None if we're current.
fn latest() -> Result<Option<Release>> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let r: GhRelease = match agent(Some(Duration::from_secs(10)))
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(r) => r.into_json()?,
        Err(ureq::Error::Status(404, _)) => return Err(anyhow!("no releases published yet")),
        Err(ureq::Error::Status(code, _)) => return Err(anyhow!("GitHub answered {code}")),
        Err(e) => return Err(anyhow!("can't reach GitHub ({e})")),
    };
    let version = r.tag_name.trim_start_matches(['v', 'V']).to_owned();
    let suffix = package_suffix();
    let asset = |f: &dyn Fn(&str) -> bool| {
        r.assets
            .iter()
            .find(|a| f(&a.name))
            .map(|a| (a.name.clone(), a.browser_download_url.clone()))
    };
    let package = asset(&|n| n.ends_with(&suffix));
    let sums = asset(&|n| n == "SHA256SUMS.txt").map(|(_, u)| u);
    let (summary, notes) = release_notes(r.body.as_deref().unwrap_or_default());
    Ok(newer(&version, VERSION).then_some(Release {
        version,
        url: r.html_url,
        package,
        sums,
        summary,
        notes,
    }))
}

/// The summary line and the points of a release's notes (notes.md: "# Valo
/// v1.2.3", a summary, "### New" / "### Fixed" lists, "### Downloads").
/// A "### Admin" list is left out: the popup isn't the place for admin features.
fn release_notes(body: &str) -> (String, Vec<(String, String)>) {
    let plain = |s: &str| s.replace("**", "").replace('`', "").trim().to_owned();
    let (mut summary, mut notes) = (String::new(), Vec::new());
    let (mut in_list, mut admin) = (false, false);
    for line in body.lines().map(str::trim) {
        if let Some(h) = line.strip_prefix("### ") {
            if h.trim().eq_ignore_ascii_case("downloads") {
                break;
            }
            in_list = true;
            admin = h.trim().eq_ignore_ascii_case("admin");
        } else if line.starts_with('#') || line.is_empty() || admin {
        } else if let Some(point) = line.strip_prefix("- ").filter(|_| in_list) {
            // "- **Subtitles:** pick subtitles from …"
            let (title, text) = match point.strip_prefix("**").and_then(|p| p.split_once("**")) {
                Some((t, rest)) => (t.trim_end_matches(':'), rest.trim_start_matches(':')),
                None => ("", point),
            };
            notes.push((plain(title), plain(text)));
        } else if !in_list && summary.is_empty() {
            summary = plain(line);
        }
    }
    (summary, notes)
}

/// The hash `sha256sum` printed for `name` (paths in the file may have folders).
fn expected_hash(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (hash, path) = l.split_once(char::is_whitespace)?;
        let file = path.trim().trim_start_matches('*');
        (file.rsplit('/').next() == Some(name)).then(|| hash.to_lowercase())
    })
}

/// Download the release's package into `dir` as `name.part`, check it
/// against SHA256SUMS.txt, and rename it to `name`. `progress` is per-mille.
fn download(rel: &Release, dir: &Path, progress: &AtomicU32) -> Result<PathBuf> {
    let (Some((name, url)), Some(sums_url)) = (&rel.package, &rel.sums) else {
        bail!("this release has nothing to update this copy with");
    };
    let agent = agent(None);
    let sums = agent
        .get(sums_url)
        .call()
        .context("can't download SHA256SUMS.txt")?
        .into_string()?;
    let want = expected_hash(&sums, name)
        .ok_or_else(|| anyhow!("{name} isn't listed in SHA256SUMS.txt"))?;

    let r = agent
        .get(url)
        .call()
        .with_context(|| format!("can't download {name}"))?;
    let total: u64 = r
        .header("Content-Length")
        .and_then(|l| l.parse().ok())
        .unwrap_or(0);
    let tmp = dir.join(format!(".{name}.part"));
    let res = (|| -> Result<()> {
        let mut out =
            fs::File::create(&tmp).with_context(|| format!("can't write to {}", dir.display()))?;
        let mut hasher = Sha256::new();
        let mut reader = r.into_reader();
        let mut buf = vec![0; 64 << 10];
        let mut done = 0u64;
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n])?;
            done += n as u64;
            if let Some(p) = (done * 1000).checked_div(total) {
                progress.store(p.min(1000) as u32, Ordering::Relaxed);
            }
        }
        out.sync_all()?;
        let got: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if got != want {
            bail!("the download is corrupt (checksum mismatch), nothing was changed");
        }
        Ok(())
    })();
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    res.map(|()| tmp)
}

/// Download the new AppImage next to `dest` and swap it in.
/// Where the updated AppImage goes: a name with this version in it gets the
/// new one ("valo-v0.5.0-x86_64.AppImage" -> "valo-v0.5.1-x86_64.AppImage"),
/// any other name is kept.
fn updated_path(dest: &Path, from: &str, to: &str) -> PathBuf {
    match dest.file_name().and_then(|n| n.to_str()) {
        Some(name) if name.contains(from) => dest.with_file_name(name.replace(from, to)),
        _ => dest.to_owned(),
    }
}

/// Returns where the new AppImage is (see [`updated_path`]).
fn install_appimage(rel: &Release, dest: &Path, progress: &AtomicU32) -> Result<PathBuf> {
    // same folder as the AppImage, so the final rename can't cross filesystems
    let tmp = download(rel, dest.parent().unwrap_or(Path::new(".")), progress)?;
    #[cfg(unix)]
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    let new = updated_path(dest, VERSION, &rel.version);
    // the running copy stays mounted from the old file until we exit
    let res = fs::rename(&tmp, &new).with_context(|| format!("can't write {}", new.display()));
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
        return res.map(|()| new);
    }
    if new != dest {
        // the menu entry follows the new name when it starts (integrate::sync)
        let _ = fs::remove_file(dest);
    }
    Ok(new)
}

/// Download the new installer and start it. It closes this copy if it's still
/// running, replaces the files, and starts the new version.
fn run_setup(rel: &Release, progress: &AtomicU32) -> Result<()> {
    let tmp = download(rel, &std::env::temp_dir(), progress)?;
    // Windows only runs it under its .exe name
    let exe = tmp.with_extension("");
    fs::rename(&tmp, &exe).with_context(|| format!("can't write {}", exe.display()))?;
    Command::new(&exe)
        .args([
            "/SILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/CLOSEAPPLICATIONS",
        ])
        .spawn()
        .with_context(|| format!("can't start {}", exe.display()))?;
    Ok(())
}

/// Start the (new) AppImage with our arguments; the caller then closes this window.
fn relaunch(appimage: &Path) -> Result<()> {
    let mut cmd = Command::new(appimage);
    cmd.args(std::env::args_os().skip(1));
    // the runtime sets these afresh for the new process
    for v in ["APPIMAGE", "APPDIR", "ARGV0", "OWD"] {
        cmd.env_remove(v);
    }
    cmd.spawn()
        .with_context(|| format!("can't start {}", appimage.display()))?;
    Ok(())
}

enum Toast {
    UpToDate,
    Failed(String),
    Installing,
    InstallFailed(String),
}

/// What the top-bar update control asked for.
pub enum Click {
    Check,
    ShowUpdate,
}

pub struct Updates {
    task: Option<(bool, Task<Result<Option<Release>>>)>,
    pub available: Option<Release>,
    pub checked_at: Option<Instant>,
    toast: Option<(Toast, Instant)>,
    /// The "update available" popup is up (or waiting for other dialogs).
    popup: bool,
    install: Install,
    installing: Option<(Task<Result<()>>, Arc<AtomicU32>)>,
}

impl Default for Updates {
    fn default() -> Self {
        Self {
            task: None,
            available: None,
            checked_at: None,
            toast: None,
            popup: false,
            install: Install::detect(),
            installing: None,
        }
    }
}

impl Updates {
    /// `manual` checks always report back; automatic ones only when there's an update.
    pub fn check(&mut self, ctx: &egui::Context, manual: bool) {
        if self.task.is_none() {
            self.task = Some((manual, Task::spawn(ctx, latest)));
        }
    }

    pub fn checking(&self) -> bool {
        self.task.is_some()
    }

    /// Bring the "update available" popup back.
    pub fn show(&mut self) {
        if self.available.is_some() && self.installing.is_none() {
            self.popup = true;
        }
    }

    /// Whether the update button installs in place (vs. opening the release page).
    fn in_place(&self, rel: &Release) -> bool {
        matches!(self.install, Install::AppImage(_) | Install::WindowsSetup)
            && rel.package.is_some()
            && rel.sums.is_some()
    }

    fn start_install(&mut self, ctx: &egui::Context) {
        let Some(rel) = self.available.clone() else {
            return;
        };
        let install = self.install.clone();
        let progress = Arc::new(AtomicU32::new(0));
        let p = progress.clone();
        self.installing = Some((
            Task::spawn(ctx, move || match install {
                Install::AppImage(dest) => {
                    let new = install_appimage(&rel, &dest, &p)?;
                    relaunch(&new)
                }
                Install::WindowsSetup => run_setup(&rel, &p),
                Install::Flatpak | Install::Other => bail!("this copy can't update itself"),
            }),
            progress,
        ));
        self.toast = Some((Toast::Installing, Instant::now()));
    }

    /// Poll the check and draw its result: a popup with the release notes
    /// once `quiet` (no splash or other dialog up), notices in the corner.
    pub fn ui(&mut self, ctx: &egui::Context, quiet: bool) {
        if let Some((manual, mut t)) = self.task.take() {
            match t.poll() {
                None => self.task = Some((manual, t)),
                Some(r) => {
                    self.checked_at = Some(Instant::now());
                    let toast = match r {
                        Ok(Some(rel)) => {
                            self.available = Some(rel);
                            self.popup = self.installing.is_none();
                            None
                        }
                        Ok(None) => {
                            self.available = None;
                            manual.then_some(Toast::UpToDate)
                        }
                        Err(e) => manual.then(|| Toast::Failed(format!("{e:#}"))),
                    };
                    if self.installing.is_none() {
                        self.toast = toast.map(|t| (t, Instant::now()));
                    }
                }
            }
        }
        if let Some((t, _)) = self.installing.as_mut() {
            if let Some(r) = t.poll() {
                self.installing = None;
                match r {
                    // the new version is starting; it takes over from here
                    Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(e) => {
                        self.toast = Some((Toast::InstallFailed(format!("{e:#}")), Instant::now()))
                    }
                }
            }
        }

        if self.popup && quiet && self.installing.is_none() {
            if let Some(r) = &self.available {
                let choice = crate::ui::whats_new::update_available(
                    ctx,
                    r,
                    self.in_place(r),
                    self.install == Install::Flatpak,
                );
                match choice {
                    Some(crate::ui::whats_new::UpdateChoice::Update) => {
                        self.popup = false;
                        self.start_install(ctx);
                    }
                    Some(crate::ui::whats_new::UpdateChoice::Download) => {
                        chrome::open_url(&r.url);
                        self.popup = false;
                    }
                    Some(crate::ui::whats_new::UpdateChoice::Later) => self.popup = false,
                    None => {}
                }
            }
        }

        let Some((toast, since)) = &self.toast else {
            return;
        };
        let keep = matches!(toast, Toast::Installing | Toast::InstallFailed(_))
            || since.elapsed() < Duration::from_secs(6);
        if !keep {
            self.toast = None;
            return;
        }
        let mut close = false;
        egui::Area::new(egui::Id::new("update_toast"))
            .anchor(Align2::RIGHT_BOTTOM, [-16.0, -46.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(BG())
                    .stroke(egui::Stroke::new(1.0f32, LINE()))
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.set_max_width(380.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                let v = self.available.as_ref().map_or("", |r| r.version.as_str());
                                let (title, body) = match toast {
                                    Toast::Installing => {
                                        let p = self
                                            .installing
                                            .as_ref()
                                            .map_or(0, |(_, p)| p.load(Ordering::Relaxed));
                                        (
                                            format!("Updating to {v}… {}%", p / 10),
                                            "Valo restarts by itself when it's done.".to_owned(),
                                        )
                                    }
                                    Toast::UpToDate => (
                                        "Up to date".to_owned(),
                                        format!("Valo {VERSION} is the latest version."),
                                    ),
                                    Toast::Failed(e) => {
                                        ("Couldn't check for updates".to_owned(), e.clone())
                                    }
                                    Toast::InstallFailed(e) => {
                                        ("Couldn't update".to_owned(), e.clone())
                                    }
                                };
                                ui.label(mono(&title, 11.0).color(MUTED()));
                                ui.label(RichText::new(body).color(TEXT()));
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if matches!(toast, Toast::Installing) {
                                    ui.spinner();
                                    return;
                                }
                                if crate::ui::small_icon_button(
                                    ui,
                                    crate::ui::chrome::Icon::Close,
                                    "Dismiss",
                                )
                                .clicked()
                                {
                                    close = true;
                                }
                                let Some(r) = &self.available else { return };
                                if matches!(toast, Toast::InstallFailed(_))
                                    && small_button(ui, "Download").clicked()
                                {
                                    chrome::open_url(&r.url);
                                    close = true;
                                }
                            });
                        });
                    });
            });
        if close {
            self.toast = None;
        }
        let every = if self.installing.is_some() { 100 } else { 500 };
        ctx.request_repaint_after(Duration::from_millis(every));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appimage_name_follows_version() {
        let p = |s: &str| PathBuf::from(s);
        assert_eq!(
            updated_path(&p("/home/u/valo-v0.5.0-x86_64.AppImage"), "0.5.0", "0.5.1"),
            p("/home/u/valo-v0.5.1-x86_64.AppImage")
        );
        assert_eq!(
            updated_path(&p("/home/u/Valo 0.5.0.AppImage"), "0.5.0", "0.6.0"),
            p("/home/u/Valo 0.6.0.AppImage")
        );
        // a name without the version is the user's choice
        assert_eq!(
            updated_path(&p("/home/u/valo.AppImage"), "0.5.0", "0.5.1"),
            p("/home/u/valo.AppImage")
        );
        // only the file name changes, not a folder that happens to match
        assert_eq!(
            updated_path(&p("/opt/0.5.0/valo.AppImage"), "0.5.0", "0.5.1"),
            p("/opt/0.5.0/valo.AppImage")
        );
    }

    #[test]
    fn versions() {
        assert!(newer("0.2.0", "0.1.0"));
        assert!(newer("v1.0", "0.9.9"));
        assert!(newer("0.1.10", "0.1.9"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("v0.1.0-beta", "0.1.0"));
        assert!(!newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn notes() {
        let body = "# Valo v0.8.3\n\nSubtitles in the player, and more.\n\n### New\n\
            - **Subtitles:** pick them from the `subtitle` button.\n\
            - **Click the banner:** opens it.\n\n### Admin\n- **Subtitle search:** for admins.\n\n\
            ### Fixed\n- Volume is kept.\n\n\
            ### Downloads\n| File | For |\n- not a note\n";
        let (summary, notes) = release_notes(body);
        assert_eq!(summary, "Subtitles in the player, and more.");
        assert_eq!(
            notes,
            [
                (
                    "Subtitles".into(),
                    "pick them from the subtitle button.".into()
                ),
                ("Click the banner".into(), "opens it.".into()),
                (String::new(), "Volume is kept.".into()),
            ]
        );
        assert_eq!(release_notes(""), (String::new(), vec![]));
    }

    #[test]
    fn sums() {
        let s = "abc123  target/dist/valo-v0.2.0-x86_64.AppImage\n\
                 def456 *valo_0.2.0-1_amd64.deb\n";
        assert_eq!(
            expected_hash(s, "valo-v0.2.0-x86_64.AppImage").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            expected_hash(s, "valo_0.2.0-1_amd64.deb").as_deref(),
            Some("def456")
        );
        assert_eq!(expected_hash(s, "other.AppImage"), None);
    }
}
