# Valo v0.11.2

A new Images page for admins, to follow image upgrades and see how sharp the library's artwork is.

### New
- **Admin › Images:** run **Upgrade images** and watch its progress, see every image the last run replaced (old size → new size) or couldn't, earlier runs, and how many of the library's posters, backdrops, thumbnails and logos are 4K, 1440p, 1080p, 720p or missing. Click a title to open it in the Library editor.
- **Images upgraded on the Dashboard:** a tile with the count, and the last run's totals under the Upgrade images button.

These need the matching server update, which also fixes Upgrade images running out of memory partway through a big library.

### Downloads
| File | For |
|---|---|
| `valo-v0.11.2-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.11.2-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.11.2-windows-x86_64.zip` | Windows, portable |
| `valo-0.11.2-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.11.2-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.11.2-x86_64.flatpak` | any distro with Flatpak (updates itself and through `flatpak update`; added a few minutes after the others) |
| `valo_0.11.2-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.11.2-linux-x86_64.tar.gz` | manual install |
| `valo-v0.11.2-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.11.2-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage, the Flatpak or the TV app, click **Update** to get this version.

Checksums are in `SHA256SUMS.txt`, and for the Flatpak in `SHA256SUMS-flatpak.txt`, which arrives with it.
