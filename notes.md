# Valo v0.9.13

Posters show up much faster, and series pages say who's been watching each season and episode.

### Fixed
- **Posters load faster:** the posters you're looking at load first, top to bottom, instead of the ones at the bottom of the page. Downloaded artwork is kept for a day, so the next start shows it right away.
- **Hovering a poster:** the top of the poster you point at is no longer cut off under the row's title.

### Changed
- **Who watched what:** "Artur has watched this season" shows on every season where they watched an episode, and "Artur has watched this episode" under every episode they played, even ones they didn't finish. Needs a server that lists the episodes each person played (`viewed` in `/api/watchers/series/<id>`); with an older one, only finished episodes count.

### Downloads
| File | For |
|---|---|
| `valo-v0.9.13-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.9.13-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.9.13-windows-x86_64.zip` | Windows, portable |
| `valo-0.9.13-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.9.13-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.9.13-x86_64.flatpak` | any distro with Flatpak (updates itself and through `flatpak update`; added a few minutes after the others) |
| `valo_0.9.13-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.9.13-linux-x86_64.tar.gz` | manual install |
| `valo-v0.9.13-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.9.13-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage, the Flatpak or the TV app, click **Update** to get this version.

Checksums are in `SHA256SUMS.txt`, and for the Flatpak in `SHA256SUMS-flatpak.txt`, which arrives with it.
