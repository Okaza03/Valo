# Valo v0.8.9

Admins choose which rows everyone sees on the home page, and in what order.

### Admin
- **Admin › Layout:** tick the rows that show on everyone's home page, from Continue watching and Recently added to each genre and addon catalog. The Movies, TV shows and Streaming pages still list everything.
- **Row order:** drag a row by its handle, or use the arrows, to move it. The banner and genre tiles always stay at the top. Everyone gets the new home page the next time their library loads.
- Servers need `GET /api/layout` and `GET`/`PATCH /api/admin/layout` (see `docs/api/admin-layout.md`). The reference server has them. Without them, Layout says *Not set up* and the home page shows every row.

### Downloads
| File | For |
|---|---|
| `valo-v0.8.9-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.8.9-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.8.9-windows-x86_64.zip` | Windows, portable |
| `valo-0.8.9-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.8.9-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.8.9-x86_64.flatpak` | any distro with Flatpak (added a few minutes after the others) |
| `valo_0.8.9-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.8.9-linux-x86_64.tar.gz` | manual install |
| `valo-v0.8.9-androidtv.apk` | Android TV and Google TV (updates itself) |

If you're using the Windows installer, the AppImage or the TV app, click **Update** to get this version.

Checksums for every file except the Flatpak are in `SHA256SUMS.txt`.
