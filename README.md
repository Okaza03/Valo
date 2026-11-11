<div align="center">

<img src="packaging/valo.svg" alt="Valo" width="96">

**A small, fast video player for your own media library.**

[![CI](https://github.com/Okaza03/Valo/actions/workflows/ci.yml/badge.svg)](https://github.com/Okaza03/Valo/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/Okaza03/Valo?sort=semver&color=brightgreen)](https://github.com/Okaza03/Valo/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/Okaza03/Valo/total)](https://github.com/Okaza03/Valo/releases)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

</div>

A desktop video player and media-library client (Rust, Iced, FFmpeg).

1. **Setup**: enter your media server's address (`192.168.1.20`, `nas.local:8080`, …). It's remembered for next time.
2. **Server check**: the first time you add a server, Valo checks every part of its API one by one (reachability, `/api/info`, sign-in, library, streaming, artwork) and shows how many titles have each piece of metadata. Problems that stop the app from working are errors and block the server. Missing extras, or a server without a sign-in, are warnings you can accept with _I know this is missing, continue_. _Copy report_ gives you the results to send to the server's admin.
3. **Sign in**: only shown if the server says it needs a login. If you're an admin, the library checks run after it; everyone else goes straight to the library.
4. **Library**: Prime Video–style home page (featured banner, Continue watching, Recently added, Movies, TV shows, rows per genre), Movies/TV grids with sorting and genre filter, search (Ctrl+F), and detail pages with seasons and episodes.
5. **Player**: streams straight from the server. It remembers where you stopped and plays the next episode automatically.

You can still play local files: pass them on the command line, drag them onto the window, or use _Open local file…_.

```sh
cargo run --release -p valo
```

On Windows, first run `cargo xtask ffmpeg` once. It downloads a prebuilt LGPL FFmpeg into `target/ffmpeg`, which Windows builds link against (Linux builds compile FFmpeg from source). You also need LLVM (`winget install LLVM.LLVM`) for bindgen.

Settings and resume points are stored in `~/.config/valo/` (`%APPDATA%\valo\` on Windows).

## Installing

Each [release](https://github.com/Okaza03/Valo/releases) has:

- **AppImage**: `chmod +x` it and run it. The first launch adds Valo to your app menu (you can turn this off in Settings). Updates are one click: the app downloads the new AppImage, verifies it against `SHA256SUMS.txt`, replaces itself and restarts.
- **Flatpak**: from Valo's own Flatpak repository, in one command:

  ```
  flatpak install --user https://okaza03.github.io/Valo/valo.flatpakref
  ```

  That adds the repository as `valo` (the runtime comes from Flathub), so from then on `flatpak install io.github.okaza03.Valo` and `flatpak update` work like for any other app. To add only the repository, add Flathub for the runtime too: `flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo`, then `flatpak remote-add --user --if-not-exists valo https://okaza03.github.io/Valo/valo.flatpakrepo`. Each release also attaches the bundle (`flatpak install --user valo-vX.Y.Z-x86_64.flatpak`), which updates from the same repository. Inside the app, **Update now** installs the new version through the host's `flatpak` (the manifest grants `org.freedesktop.Flatpak` for this). The manifest and the repository script are in `packaging/flatpak/`.
- **.deb** and **.tar.gz**: the app tells you when an update is out and opens the release page.
- **Windows installer** (`valo-vX.Y.Z-x86_64-setup.exe`, or `-aarch64-setup.exe` for ARM64 PCs such as Snapdragon laptops): installs for your user only, so no admin prompt. Adds a Start menu entry and _Open with → Valo_ for video files. Updates are one click: the app downloads the next installer, verifies it against `SHA256SUMS.txt`, and runs it, which restarts Valo. Windows SmartScreen may warn on the first run because the installer isn't code-signed.
- **Windows zip**: portable. Unzip and run `valo.exe`, keeping the FFmpeg DLLs next to it. Updates open the release page.
- **Android TV / Google TV** (`valo-vX.Y.Z-androidtv.apk`): a separate app for the TV, made for the remote, that plays with the TV's own hardware decoders. On the TV, install the free *Downloader* app from the Play Store, allow it to install unknown apps when asked, and enter the APK's download link. To update, install the newer APK the same way; your sign-in and progress are kept. The source is in `apps/androidtv/` (`./gradlew assembleTvRelease`).
- **Android phones and tablets** (`valo-vX.Y.Z-android.apk`): everything the TV app has, made for touch, and a Cast button to play on a Chromecast or Google TV. Download the APK on the phone and allow your browser to install apps when asked; updates are one click from the Profile page. It installs next to the TV app, not over it. Casting needs Google Play services and the phone on the same Wi-Fi as the TV, because the video goes through the phone. Chromecasts play H.264 video (4K models HEVC too) with AAC or MP3 sound, and many dongles only take stereo. When a file won't play as it is, the phone converts it as it plays, like VLC does: the sound to stereo, and the video to H.264 only if the device can't decode it (HEVC on an older Chromecast, say). So 5.1 films and MKV files cast too, and nothing on your server changes. See [Casting](docs/api/streams.md#casting-from-the-phone-app).

Windows builds are made with `cargo xtask dist [arch]` (zip) and `cargo xtask installer [arch]` (needs [Inno Setup 6](https://jrsoftware.org/isinfo.php)), where `arch` is `x86_64` (default) or `aarch64`. The installer script is `packaging/windows/valo.iss`. Build ARM64 with the native ARM64 Rust toolchain and LLVM 19 or newer: older libclang can't parse MSVC's ARM64 headers.

## Releasing

1. Bump `version` in `Cargo.toml`.
2. Rewrite `notes.md` for the new version. Its first line must name it (`# Valo vX.Y.Z`). It becomes the release description.
3. Add `<release version="X.Y.Z" …/>` to `packaging/flatpak/io.github.okaza03.Valo.metainfo.xml`.
4. Open a pull request with these changes from a branch named `release-X.Y.Z`. Merging it tags the merge as `vX.Y.Z` and starts the release (`.github/workflows/tag-release.yml`). The branch name has to match the version in `Cargo.toml`.

The release workflow refuses to publish if any of these are out of sync with the tag. It also publishes the Flatpak repository to GitHub Pages; the one-time setup for that is in [packaging/flatpak/README.md](packaging/flatpak/README.md).

## Supported servers

**[docs/api/](docs/api/README.md) is the full guide for server admins**, one page per topic: [setup](docs/api/setup.md), [the server check](docs/api/server-check.md), [sign-in](docs/api/auth.md), [the library JSON](docs/api/library.md), [watch progress](docs/api/progress.md), [streams and images](docs/api/streams.md), the [admin API](docs/api/admin.md), [examples](docs/api/examples.md) for Laravel, Express and Flask, [curl tests](docs/api/testing.md) and [troubleshooting](docs/api/troubleshooting.md).

In short, there are three options:

1. **The bundled server.** `server/valo_server.py` implements the whole API with only the Python standard library. Metadata comes from `info.json` files and images next to the videos.
   ```sh
   python3 server/valo_server.py ~/Videos                 # Movies/ and Series/ inside
   python3 server/valo_server.py ~/Videos --add-user user # turn on sign-in
   python3 server/valo_server.py ~/Videos --add-user user --admin  # ...as an admin
   python3 server/valo_server.py /mnt/disk1 /mnt/disk2    # a library over two disks
   ```
2. **Your own app with the Valo API.** It needs four routes:

   | Route              | Needed                | Returns                                                                  |
   | ------------------ | --------------------- | ------------------------------------------------------------------------ |
   | `GET /api/info`    | always, no sign-in    | `{"name": "…", "auth": true, "version": 1}`                              |
   | `POST /api/login`  | when `auth` is `true` | `{"token": "…", "name": "…", "admin": false}`, or `401`                  |
   | `POST /api/logout` | optional              | invalidates the token                                                    |
   | `GET /api/library` | always                | `{"movies": […], "series": […]}` with `stream` URLs that support `Range` |

   After signing in, every request (including videos and images) carries `Authorization: Bearer <token>`. Only `title` plus `stream` (movies) or `seasons` (series) are required. Everything else (artwork, descriptions, genres, cast, reviews, file info, …) is optional and fills in the details pages. The server check shows which fields are missing.

3. **Any plain directory listing** (nginx `autoindex on`, Apache, Caddy `file_server browse`). Valo crawls it and guesses movies and series from folder and file names (`Show/Season 1/Show.S01E02.Title.mkv`, `Movies/Title (2010)/…`). It also picks up `poster.jpg` and `fanart.jpg`. The server must support HTTP range requests for seeking to work; nginx, Apache and Caddy do, but `python -m http.server` doesn't.

Only `http://` is supported for streaming. The bundled FFmpeg is built without TLS.
