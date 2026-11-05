# Valo v0.9.7

Sync folders finds every new episode, even ones that aren't named S01E02.

### Fixed
- **Sync folders:** new episodes now show in the dry run even when the file has no `S01E02` in its name, like `Season 2/Episode 3.mkv` or `Season 1/04 - Pilot.mkv`. They're numbered from their season folder.
- **Sync folders:** files added while the server was off now count as new.

### Admins (desktop)
- **Sync folders:** the report starts with a summary, like *1 new series, 1 new season, 4 new episodes*. A new *Not renamed* list shows the files without `S01E02` in the name, the episode each one was given, and the ones that couldn't be placed, so you know what to rename.

### For server owners
Update `valo_server.py` and restart it. It now keeps the last sync in `.valo/synced.json`. Other servers can send `unnamed` in `POST /api/admin/sync/folders` (see [Dashboard](https://github.com/Okaza03/Valo/blob/main/docs/api/admin-dashboard.md#post-apiadminsyncwhat)).

### Downloads
| File | For |
|---|---|
| `valo-v0.9.7-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.9.7-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.9.7-windows-x86_64.zip` | Windows, portable |
| `valo-0.9.7-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.9.7-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.9.7-x86_64.flatpak` | any distro with Flatpak (updates itself; added a few minutes after the others) |
| `valo_0.9.7-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.9.7-linux-x86_64.tar.gz` | manual install |
| `valo-v0.9.7-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.9.7-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage, the Flatpak or the TV app, click **Update** to get this version.

Checksums are in `SHA256SUMS.txt`, and for the Flatpak in `SHA256SUMS-flatpak.txt`, which arrives with it.
