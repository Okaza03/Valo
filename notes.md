# Valo v0.9.4

The Flatpak now updates itself, like the AppImage and the Windows installer.

### Desktop (Linux Flatpak)
- **Updates:** when a new version is out, *Update now* downloads it, checks it, installs it and restarts Valo. Your sign-in, settings and progress are kept. To do this, the Flatpak now asks for permission to talk to Flatpak on your computer (`org.freedesktop.Flatpak`). **Install this version by hand** (download the `.flatpak` and open it, or run `flatpak install --user valo-v0.9.4-x86_64.flatpak`), since older copies can't update themselves. From here on, updates are one click.

### Fixed
- **Security:** updated rustls, the library behind Valo's secure connections, to 0.23.46 for its security fixes.

### Downloads
| File | For |
|---|---|
| `valo-v0.9.4-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.9.4-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.9.4-windows-x86_64.zip` | Windows, portable |
| `valo-0.9.4-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.9.4-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.9.4-x86_64.flatpak` | any distro with Flatpak (updates itself from now on; added a few minutes after the others) |
| `valo_0.9.4-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.9.4-linux-x86_64.tar.gz` | manual install |
| `valo-v0.9.4-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.9.4-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage or the TV app, click **Update** to get this version. Flatpak users: install this one by hand, as above.

Checksums are in `SHA256SUMS.txt`, and for the Flatpak in `SHA256SUMS-flatpak.txt`, which arrives with it.
