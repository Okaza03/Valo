# Valo v0.12.0

Admins can upload movies and episodes to the server straight from the app.

### New
- **Admin › Upload:** the server's movie and series folders on every disk, side by side with their free space. Go into a folder or make one (e.g. `Heat.1995`, or a show and `Season 01`), pick files, and follow each one's progress, speed and time left. Uploads carry on while you use the rest of the app, and the sidebar counts the ones still going. A dropped connection is retried, and if Valo closes, picking the same file into the same folder again carries on where it stopped. When they're done, the server adds them to the library.

This needs the matching server update. Its Docker image has to be rebuilt, and an nginx in front of it must allow 64 MB requests (`client_max_body_size 64m;`).

### Downloads
| File | For |
|---|---|
| `valo-v0.12.0-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.12.0-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.12.0-windows-x86_64.zip` | Windows, portable |
| `valo-0.12.0-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.12.0-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.12.0-x86_64.flatpak` | any distro with Flatpak (updates itself and through `flatpak update`; added a few minutes after the others) |
| `valo_0.12.0-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.12.0-linux-x86_64.tar.gz` | manual install |
| `valo-v0.12.0-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.12.0-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage, the Flatpak or the TV app, click **Update** to get this version.

Checksums are in `SHA256SUMS.txt`, and for the Flatpak in `SHA256SUMS-flatpak.txt`, which arrives with it.
