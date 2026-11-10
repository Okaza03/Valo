# Valo v0.9.12

Valo's desktop app has a new engine underneath, and series pages show who watched each season and episode.

### Fixed
- **Play button:** a long label like "Resume S01 E01" stays on one line.

### New
- **Mouse back and forward:** the side buttons on your mouse go back and forward between pages. Back also closes the player or a dialog.
- **Who watched what:** a series page says who else has watched the whole series or the season you're looking at ("Artur has watched this season"), and each episode says who watched it ("Artur watched this episode"). Needs a server that lists the episodes each person watched (`episodes` in `/api/watchers/series/<id>`, by id or as "S01E02"); with an older one, only those who watched the whole series show.

### Changed
- **Watched at 90%:** movies and episodes both count as watched once you're 90% of the way through, on the desktop and TV apps. Clicking **Next Episode** still counts the episode you left.
- **New UI engine:** the desktop app is rebuilt on Iced. It looks and works the same, every app style included, and the Disney+ style's top tabs no longer overlap.

### Downloads
| File | For |
|---|---|
| `valo-v0.9.12-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.9.12-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.9.12-windows-x86_64.zip` | Windows, portable |
| `valo-0.9.12-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.9.12-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.9.12-x86_64.flatpak` | any distro with Flatpak (updates itself and through `flatpak update`; added a few minutes after the others) |
| `valo_0.9.12-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.9.12-linux-x86_64.tar.gz` | manual install |
| `valo-v0.9.12-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.9.12-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage, the Flatpak or the TV app, click **Update** to get this version.

Checksums are in `SHA256SUMS.txt`, and for the Flatpak in `SHA256SUMS-flatpak.txt`, which arrives with it.
