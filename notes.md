# Valo v0.9.12

Valo's desktop app has a new engine underneath, and series pages show who watched each season and episode.

### Fixed
- **Play button:** a long label like "Resume S01 E01" stays on one line.
- **Watched episodes:** an episode you left in its end credits for the next one counts as watched, including ones watched before v0.9.11. Series like that now say "You've watched this season" or "You've watched the whole series", tag every episode you've seen and leave Continue watching.

### New
- **Who watched what:** a series page says who else has watched the whole series or the season you're looking at ("Artur has watched this season"), and each episode lists who watched it. Needs the updated server (`server/valo_server.py`); with an older one, only those who watched the whole series show.

### Changed
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
