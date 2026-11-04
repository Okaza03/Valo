# Valo v0.9.5

See what everyone else on your server is watching, and a cleaner Analytics page for admins.

### Desktop
- **Viewed by other users:** a new row on the home page, under *Recently added*, shows what other people on your server watched lately. The latest viewer's name is under each poster (for example *Jo +2*). *See all* opens the full list.
- **Who watched this:** a movie's or series' page shows who else watched it, under the *Play* button. You'll see up to three faces and the two latest names, like *Jo, Sam and 3 others watched this*. Hover to see everyone and when they watched.

### Admins (desktop)
- **Analytics:** *By user* lists every account, not just the people who watched the most. People who didn't watch in the chosen range are listed at the end.
- **Users:** click a user's name to open their page, with what they watched, when, and on which devices. *‹ Users* takes you back.
- **Layout:** *Viewed by other users* can be hidden or moved like the other home page rows.

### Fixed
- **Sync folders:** new and removed files are counted once. Some servers listed each one twice, so 5 new files showed as 10.

### For server owners
The new rows need `GET /api/watchers`, `GET /api/watchers/movie/<id>` and `GET /api/watchers/series/<id>` (see [Watch progress](https://github.com/Okaza03/Valo/blob/main/docs/api/progress.md)). The bundled `valo_server.py` has them. Without them, the app hides these rows. For *Analytics*, send every user who watched, not just the top few.

### Downloads
| File | For |
|---|---|
| `valo-v0.9.5-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.9.5-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.9.5-windows-x86_64.zip` | Windows, portable |
| `valo-0.9.5-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.9.5-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.9.5-x86_64.flatpak` | any distro with Flatpak (updates itself; added a few minutes after the others) |
| `valo_0.9.5-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.9.5-linux-x86_64.tar.gz` | manual install |
| `valo-v0.9.5-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.9.5-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage, the Flatpak or the TV app, click **Update** to get this version.

Checksums are in `SHA256SUMS.txt`, and for the Flatpak in `SHA256SUMS-flatpak.txt`, which arrives with it.
