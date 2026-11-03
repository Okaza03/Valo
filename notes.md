# Valo v0.9.2

Admins can now lock a title while its files move from one disk to another, so nobody plays half-copied files.

### Admin › Moving (desktop)
- **A new section:** tick the series or movies you're about to move. Ticked titles are pinned under *Unavailable now*, and a search box finds the rest. Untick a title once it plays from its new place. Your server needs to support it; see `docs/api/admin-moving.md`.

### Everyone (desktop, TV and phone)
- **MOVING band:** a title being moved has a red *MOVING* band across every poster, thumbnail, banner and episode still, so everyone can see why it won't play.
- **Can't be played:** its Play button reads *Unavailable — being moved*. Continue watching, episodes, next episode and casting open the title's page instead of playing. A series being moved covers all its episodes.

### Bundled server
- **Moving lock:** marked titles are kept in `.valo/moving.json`. Their streams are refused until they're unmarked, while artwork and subtitles keep loading.

### Downloads
| File | For |
|---|---|
| `valo-v0.9.2-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.9.2-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.9.2-windows-x86_64.zip` | Windows, portable |
| `valo-0.9.2-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.9.2-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.9.2-x86_64.flatpak` | any distro with Flatpak (added a few minutes after the others) |
| `valo_0.9.2-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.9.2-linux-x86_64.tar.gz` | manual install |
| `valo-v0.9.2-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.9.2-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage or the TV app, click **Update** to get this version.

Checksums for every file except the Flatpak are in `SHA256SUMS.txt`.
