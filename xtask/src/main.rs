//! `cargo xtask <install|uninstall|dist|deb|appimage|dmg|ffmpeg|installer|ci>`
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};
use std::{env, fs};

type Res<T = ()> = Result<T, Box<dyn std::error::Error>>;

const NAME: &str = "valo";
/// Prebuilt shared FFmpeg (LGPL) that Windows builds link and ship. Keep its
/// version in step with ffmpeg-sys-next, which builds release/<its major.minor>
/// from source on Linux. `{}` is `win64` or `winarm64`.
const FFMPEG_URL: &str = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-n9.0-latest-{}-lgpl-shared-9.0.zip";

/// A Windows CPU to build for: `x86_64` (default) or `aarch64` (`arm64`).
#[derive(Clone, Copy, PartialEq)]
enum Arch {
    X64,
    Arm64,
}

impl Arch {
    fn parse(a: Option<String>) -> Res<Self> {
        match a.as_deref() {
            None | Some("x86_64" | "x64" | "amd64") => Ok(Arch::X64),
            Some("aarch64" | "arm64") => Ok(Arch::Arm64),
            Some(o) => Err(format!("unknown architecture {o}: use x86_64 or aarch64").into()),
        }
    }

    /// As in std::env::consts::ARCH, which the app's updater matches on.
    fn name(self) -> &'static str {
        match self {
            Arch::X64 => "x86_64",
            Arch::Arm64 => "aarch64",
        }
    }

    fn triple(self) -> &'static str {
        match self {
            Arch::X64 => "x86_64-pc-windows-msvc",
            Arch::Arm64 => "aarch64-pc-windows-msvc",
        }
    }

    fn native(self) -> bool {
        self.name() == env::consts::ARCH
    }

    /// Where its FFmpeg goes. The native one is in target/ffmpeg, which
    /// .cargo/config.toml points FFMPEG_DIR at, so plain `cargo run` works.
    fn ffmpeg_dir(self) -> PathBuf {
        if self.native() {
            root().join("target/ffmpeg")
        } else {
            root().join(format!("target/ffmpeg-{}", self.name()))
        }
    }

    /// Folder cargo puts the release build in.
    fn release_dir(self) -> PathBuf {
        if self.native() {
            root().join("target/release")
        } else {
            root().join("target").join(self.triple()).join("release")
        }
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        exit(1);
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn sh(cmd: &mut Command) -> Res {
    println!("+ {cmd:?}");
    if cmd.status()?.success() {
        Ok(())
    } else {
        Err(format!("command failed: {cmd:?}").into())
    }
}

fn cargo(args: &[&str]) -> Res {
    sh(Command::new("cargo").args(args).current_dir(root()))
}

/// The FFmpeg a Windows build for `arch` links. FFMPEG_DIR overrides it for
/// native builds.
fn ffmpeg_dir(arch: Arch) -> Res<PathBuf> {
    let d = match env::var_os("FFMPEG_DIR") {
        Some(d) if arch.native() => PathBuf::from(d),
        _ => arch.ffmpeg_dir(),
    };
    if !d.join("include").is_dir() {
        return Err(format!(
            "FFmpeg not found in {}: run `cargo xtask ffmpeg {}`",
            d.display(),
            arch.name()
        )
        .into());
    }
    Ok(d)
}

/// Download and unpack FFmpeg for `arch` (Windows).
fn ffmpeg(arch: Arch) -> Res {
    let os = match arch {
        Arch::X64 => "win64",
        Arch::Arm64 => "winarm64",
    };
    let url = env::var("FFMPEG_URL").unwrap_or_else(|_| FFMPEG_URL.replace("{}", os));
    let work = root().join("target/ffmpeg-download");
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work)?;
    sh(Command::new("curl")
        .args(["-fL", "--retry", "3", "-o", "ffmpeg.zip", &url])
        .current_dir(&work))?;
    // Windows' tar is bsdtar, which unpacks zips
    sh(Command::new("tar")
        .args(["-xf", "ffmpeg.zip"])
        .current_dir(&work))?;
    let unpacked = fs::read_dir(&work)?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.join("include").is_dir())
        .ok_or("the FFmpeg archive has no include/ folder")?;
    let dest = arch.ffmpeg_dir();
    let _ = fs::remove_dir_all(&dest);
    fs::rename(unpacked, &dest)?;
    let _ = fs::remove_dir_all(&work);
    println!("FFmpeg is in {}", dest.display());
    Ok(())
}

fn run() -> Res {
    let mut a = env::args().skip(1);
    match a.next().as_deref() {
        Some("install" | "uninstall" | "deb" | "appimage") if cfg!(windows) => {
            Err("that's for Linux; on Windows use `cargo xtask dist` or `installer`".into())
        }
        Some("dmg") if !cfg!(target_os = "macos") => {
            Err("`cargo xtask dmg` runs on macOS (see .github/workflows/macos.yml)".into())
        }
        Some("dmg") => dmg(),
        Some("install") => install(a.next()),
        Some("uninstall") => uninstall(a.next()),
        Some("dist") if cfg!(windows) => dist_windows(Arch::parse(a.next())?).map(|_| ()),
        Some("dist") => dist(),
        Some("deb") => cargo(&["deb", "-p", NAME]),
        Some("appimage") => appimage(),
        Some("ffmpeg") => ffmpeg(Arch::parse(a.next())?),
        Some("installer") => installer(Arch::parse(a.next())?),
        Some("ci") => {
            cargo(&["fmt", "--all", "--check"])?;
            cargo(&[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ])?;
            cargo(&["test", "--workspace"])
        }
        _ => {
            eprintln!("usage: cargo xtask <install [prefix] | uninstall [prefix] | dist | deb | appimage | dmg | ffmpeg [arch] | installer [arch] | ci>\n  arch (Windows): x86_64 (default) or aarch64");
            exit(2);
        }
    }
}

fn put(src: &Path, dst: &Path, mode: u32) -> Res {
    fs::create_dir_all(dst.parent().unwrap())?;
    // unlink first: a running copy can't be overwritten ("Text file busy"),
    // but it keeps running from the old file once it's removed
    let _ = fs::remove_file(dst);
    fs::copy(src, dst)?;
    #[cfg(unix)]
    fs::set_permissions(dst, fs::Permissions::from_mode(mode))?;
    #[cfg(not(unix))]
    let _ = mode;
    Ok(())
}

fn prefix_or_default(p: Option<String>) -> PathBuf {
    p.map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var("HOME").expect("HOME not set")).join(".local"))
}

fn stage(prefix: &Path, absolute_exec: bool) -> Res {
    let r = root();
    let bin = prefix.join("bin").join(NAME);
    put(&r.join("target/release").join(NAME), &bin, 0o755)?;
    put(
        &r.join("packaging/valo.svg"),
        &prefix.join("share/icons/hicolor/scalable/apps/valo.svg"),
        0o644,
    )?;
    let mut desktop = fs::read_to_string(r.join("packaging/valo.desktop"))?;
    if absolute_exec {
        desktop = desktop.replace("Exec=valo", &format!("Exec={}", bin.display()));
    }
    let dst = prefix.join("share/applications/valo.desktop");
    fs::create_dir_all(dst.parent().unwrap())?;
    fs::write(&dst, desktop)?;
    Ok(())
}

fn install(prefix: Option<String>) -> Res {
    let prefix = prefix_or_default(prefix);
    cargo(&["build", "--release", "-p", NAME])?;
    stage(&prefix, true)?;
    // a local build: tell it apart from the released app in menus and the taskbar
    let entry = prefix.join("share/applications/valo.desktop");
    let desktop = fs::read_to_string(&entry)?.replace("\nName=Valo\n", "\nName=Valo Test\n");
    fs::write(&entry, desktop)?;
    let _ = Command::new("update-desktop-database")
        .arg(prefix.join("share/applications"))
        .status();
    println!(
        "Installed to {}. Run `{NAME}` or open it from your app menu.",
        prefix.display()
    );
    Ok(())
}

fn uninstall(prefix: Option<String>) -> Res {
    let prefix = prefix_or_default(prefix);
    for f in [
        "bin/valo",
        "share/applications/valo.desktop",
        "share/icons/hicolor/scalable/apps/valo.svg",
    ] {
        let _ = fs::remove_file(prefix.join(f));
    }
    println!("Removed from {}", prefix.display());
    Ok(())
}

fn dist() -> Res {
    cargo(&["build", "--release", "-p", NAME])?;
    let name = format!(
        "{NAME}-{}-linux-{}",
        env!("CARGO_PKG_VERSION"),
        env::consts::ARCH
    );
    let out = root().join("target/dist");
    let tree = out.join(&name);
    let _ = fs::remove_dir_all(&tree);
    stage(&tree, false)?;
    sh(Command::new("tar")
        .args(["-czf", &format!("{name}.tar.gz"), &name])
        .current_dir(&out))?;
    println!("Built {}", out.join(format!("{name}.tar.gz")).display());
    Ok(())
}

fn appimage() -> Res {
    cargo(&["build", "--release", "-p", NAME])?;
    let r = root();
    let work = r.join("target/appimage");
    fs::create_dir_all(&work)?;
    // needs linuxdeploy on PATH: https://github.com/linuxdeploy/linuxdeploy
    sh(Command::new("linuxdeploy")
        .args(["--appdir", "AppDir"])
        .arg("--executable")
        .arg(r.join("target/release").join(NAME))
        .arg("--desktop-file")
        .arg(r.join("packaging/valo.desktop"))
        .arg("--icon-file")
        .arg(r.join("packaging/valo.svg"))
        .args(["--output", "appimage"])
        .current_dir(&work))
}

/// Windows: valo.exe plus FFmpeg's DLLs in target/dist/<name>/, zipped.
/// Returns the folder and the version.
fn dist_windows(arch: Arch) -> Res<(PathBuf, String)> {
    let ffmpeg = ffmpeg_dir(arch)?;
    let mut build = Command::new("cargo");
    build.args(["build", "--release", "-p", NAME]);
    if !arch.native() {
        build
            .args(["--target", arch.triple()])
            .env("FFMPEG_DIR", &ffmpeg);
    }
    sh(build.current_dir(root()))?;
    let r = root();
    let version = env!("CARGO_PKG_VERSION").to_owned();
    let name = format!("{NAME}-{version}-windows-{}", arch.name());
    let out = r.join("target/dist");
    let tree = out.join(&name);
    let _ = fs::remove_dir_all(&tree);
    fs::create_dir_all(&tree)?;
    fs::copy(
        arch.release_dir().join(format!("{NAME}.exe")),
        tree.join(format!("{NAME}.exe")),
    )?;
    for dll in fs::read_dir(ffmpeg.join("bin"))?
        .flatten()
        .map(|e| e.path())
    {
        if dll
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("dll"))
        {
            fs::copy(&dll, tree.join(dll.file_name().unwrap()))?;
        }
    }
    // FFmpeg is LGPL: its license goes with the DLLs
    if let Ok(l) = fs::read(ffmpeg.join("LICENSE.txt")) {
        fs::write(tree.join("FFMPEG-LICENSE.txt"), l)?;
    }
    let zip = format!("{name}.zip");
    let _ = fs::remove_file(out.join(&zip));
    sh(Command::new("tar")
        .args(["-a", "-cf", &zip, &name])
        .current_dir(&out))?;
    println!("Built {}", out.join(zip).display());
    Ok((tree, version))
}

/// Windows: the Inno Setup installer, target/dist/valo-vX.Y.Z-<arch>-setup.exe.
/// Needs Inno Setup 6 (`iscc` on PATH, or in its default folder).
fn installer(arch: Arch) -> Res {
    let (tree, version) = dist_windows(arch)?;
    // machine-wide install, or winget's per-user one
    let iscc = env::var_os("ISCC").map(PathBuf::from).unwrap_or_else(|| {
        [
            env::var_os("ProgramFiles(x86)").map(PathBuf::from),
            env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Programs")),
        ]
        .into_iter()
        .flatten()
        .map(|d| d.join(r"Inno Setup 6\ISCC.exe"))
        .find(|p| p.is_file())
        .unwrap_or_else(|| "iscc".into())
    });
    let out = root().join("target/dist");
    sh(Command::new(iscc)
        .arg(format!("/DAppVersion={version}"))
        .arg(format!("/DArch={}", arch.name()))
        .arg(format!("/DSourceDir={}", tree.display()))
        .arg(format!("/DOutputDir={}", out.display()))
        .arg(format!(
            "/DOutputName={NAME}-v{version}-{}-setup",
            arch.name()
        ))
        .arg(root().join("packaging/windows/valo.iss")))
}

/// macOS: Valo.app in a disk image, target/dist/valo-vX.Y.Z-macos-<arch>.dmg.
/// Unsigned (ad-hoc signature only), so Gatekeeper asks before the first run.
/// Needs rsvg-convert (`brew install librsvg`) for the icon.
fn dmg() -> Res {
    let r = root();
    // FFmpeg and the app target the same oldest macOS as the Info.plist
    sh(Command::new("cargo")
        .args(["build", "--release", "-p", NAME])
        .env("MACOSX_DEPLOYMENT_TARGET", MACOS_MIN)
        .current_dir(&r))?;
    let version = env!("CARGO_PKG_VERSION");
    let arch = env::consts::ARCH;
    let work = r.join("target/dmg");
    let _ = fs::remove_dir_all(&work);
    let app = work.join("root/Valo.app/Contents");
    put(
        &r.join("target/release").join(NAME),
        &app.join("MacOS").join(NAME),
        0o755,
    )?;
    fs::write(app.join("Info.plist"), info_plist(version))?;

    // icon: every size iconutil wants, drawn from the SVG
    let iconset = work.join("valo.iconset");
    fs::create_dir_all(&iconset)?;
    for px in [16, 32, 128, 256, 512] {
        for (scale, suffix) in [(1, ""), (2, "@2x")] {
            let size = (px * scale).to_string();
            sh(Command::new("rsvg-convert")
                .args(["-w", &size, "-h", &size, "-o"])
                .arg(iconset.join(format!("icon_{px}x{px}{suffix}.png")))
                .arg(r.join("packaging/valo.svg")))?;
        }
    }
    fs::create_dir_all(app.join("Resources"))?;
    sh(Command::new("iconutil")
        .args(["-c", "icns", "-o"])
        .arg(app.join("Resources/valo.icns"))
        .arg(&iconset))?;

    // Apple Silicon won't run unsigned code at all; an ad-hoc signature is enough
    sh(Command::new("codesign")
        .args(["--force", "--deep", "--sign", "-"])
        .arg(work.join("root/Valo.app")))?;

    // drag-to-install: the app next to a link to /Applications
    // (`ln`, not std::os::unix, so this file still builds on Windows)
    sh(Command::new("ln")
        .args(["-s", "/Applications"])
        .arg(work.join("root/Applications")))?;
    let out = r.join("target/dist");
    fs::create_dir_all(&out)?;
    let dmg = out.join(format!("{NAME}-v{version}-macos-{arch}.dmg"));
    let _ = fs::remove_file(&dmg);
    sh(Command::new("hdiutil")
        .args([
            "create",
            "-volname",
            "Valo",
            "-fs",
            "HFS+",
            "-format",
            "UDZO",
            "-srcfolder",
        ])
        .arg(work.join("root"))
        .arg(&dmg))?;
    println!("Built {}", dmg.display());
    Ok(())
}

/// Oldest macOS the app runs on.
const MACOS_MIN: &str = "11.0";

fn info_plist(version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Valo</string>
  <key>CFBundleDisplayName</key><string>Valo</string>
  <key>CFBundleIdentifier</key><string>io.github.okaza03.Valo</string>
  <key>CFBundleExecutable</key><string>{NAME}</string>
  <key>CFBundleIconFile</key><string>valo</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>{version}</string>
  <key>CFBundleVersion</key><string>{version}</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>LSMinimumSystemVersion</key><string>{MACOS_MIN}</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.entertainment</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
"#
    )
}
