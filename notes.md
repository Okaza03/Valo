# Valo v0.11.1

A new look for the Valo style's home page, and a TMDB image picker for admins.

### New
- **Featured banner (Valo style):** a rounded banner with the titles before and after it peeking in at the sides, the title's logo and a **Play** / **Resume** button, like Prime Video's.
- **Logos on Continue watching (Valo style):** each card shows the movie's or series' logo on its picture. It steps aside while you hover the card.
- **Choose images by resolution (Admin › Library › Images):** **Choose from TMDB…** lists a title's posters, backdrops, thumbnails or logos with 4K, 1440p, 1080p and 720p filters. Pick one and the server downloads the full image and keeps it.
- **Upgrade images (Admin › Dashboard):** checks every movie's and series' images and replaces the ones TMDB has a larger version of. Images you set by hand are kept.

These two admin features need the matching server update.

### Changed
- The top bar has no dividers on its right side anymore.

### Fixed
- The Flatpak is published again. It was missing from v0.11.0, so Flatpak users get v0.11.0's hardware decoding with this update.

### Downloads
| File | For |
|---|---|
| `valo-v0.11.1-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.11.1-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.11.1-windows-x86_64.zip` | Windows, portable |
| `valo-0.11.1-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.11.1-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.11.1-x86_64.flatpak` | any distro with Flatpak (updates itself and through `flatpak update`; added a few minutes after the others) |
| `valo_0.11.1-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.11.1-linux-x86_64.tar.gz` | manual install |
| `valo-v0.11.1-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.11.1-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage, the Flatpak or the TV app, click **Update** to get this version.

Checksums are in `SHA256SUMS.txt`, and for the Flatpak in `SHA256SUMS-flatpak.txt`, which arrives with it.
