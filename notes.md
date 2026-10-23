# Valo v0.8.14

My list and Continue watching follow you between devices, a title can only be requested once, and directors' names open their page.

### Desktop and TV
- **My list syncs:** adding or removing a title on one device shows up on your others, and on your server's website if it supports it. What's already on your list is sent the first time you open this version.
- **Continue watching syncs:** taking a title off the row on one device takes it off everywhere. It comes back once you watch it again, as before.
- **One request per title:** once someone has requested a movie or series, everyone sees *Requested by alice on 23 Oct 2026* and its status instead of the Request button. If a request was denied, someone else can still ask again.
- **Directors and creators:** their names open their page wherever they show. On desktop that's the side panel's Director row, the *A film by* label on the poster, a series' Creators row and pages for titles that aren't on the server. On TV, the names in a details page's header can now be selected.

### Servers
- **Bundled server:** keeps each user's My list and Continue watching removals in `.valo/lists.json`, lets signed-in users see everyone's requests (without other people's Discord ids or notes), and answers a request for an already requested title with the existing request.
- **Your own server:** add `GET`/`POST /api/mylist` and `/api/hidden` to sync the lists; [My list and Continue watching](https://github.com/Okaza03/Valo/blob/main/docs/api/lists.md) has the details. To stop duplicate requests, send signed-in users everyone's requests from `GET /api/requests`; see [Requests](https://github.com/Okaza03/Valo/blob/main/docs/api/admin-requests.md#for-the-bot).

### Downloads
| File | For |
|---|---|
| `valo-v0.8.14-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.8.14-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.8.14-windows-x86_64.zip` | Windows, portable |
| `valo-0.8.14-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.8.14-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.8.14-x86_64.flatpak` | any distro with Flatpak (added a few minutes after the others) |
| `valo_0.8.14-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.8.14-linux-x86_64.tar.gz` | manual install |
| `valo-v0.8.14-androidtv.apk` | Android TV and Google TV (updates itself) |

If you're using the Windows installer, the AppImage or the TV app, click **Update** to get this version.

Checksums for every file except the Flatpak are in `SHA256SUMS.txt`.
