# Valo v0.9.1

The Admin page's syncs work better with libraries spread over more than one disk.

### Admin › Dashboard
- **Sync movies and Sync series wait for a scan:** both start greyed out until you run *Sync folders*, and each turns on only when the scan found something for it: new or removed files, or missing images. Running one greys it out again until the next scan.
- **Missing images:** *Sync folders* now lists titles whose poster, backdrop, thumbnail or logo files are gone, grouped into movies, series and episodes, so you know a sync will bring them back. Your server needs to send them; see `docs/api/admin-dashboard.md`.
- **Clear empties the server's log:** the live log's *Clear* button now empties the log on the server for every admin, not just on your screen. Servers that can't do this still clear it on your screen.

### Bundled server
- **Several disks:** give `valo_server.py` one folder per disk (`valo_server.py /mnt/disk1/media /mnt/disk2/media`). A series split across disks shows up once, and the syncs check every disk. Titles already in your library keep their ids.

### Downloads
| File | For |
|---|---|
| `valo-v0.9.1-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.9.1-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.9.1-windows-x86_64.zip` | Windows, portable |
| `valo-0.9.1-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.9.1-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.9.1-x86_64.flatpak` | any distro with Flatpak (added a few minutes after the others) |
| `valo_0.9.1-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.9.1-linux-x86_64.tar.gz` | manual install |
| `valo-v0.9.1-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.9.1-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage or the TV app, click **Update** to get this version.

Checksums for every file except the Flatpak are in `SHA256SUMS.txt`.
