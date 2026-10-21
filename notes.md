# Valo v0.8.8

Stremio addons: rows of what's popular and what's on Netflix and the rest, search beyond the server, requests from the app, and more subtitles.

### New
- **Addon catalogs on Home:** when your server's admin adds Stremio addons such as Cinemeta or Streaming Catalogs, Home gets their rows (*Popular · Movies*, *Netflix · TV shows*, …). Titles on the server play as usual; the rest say *not on server*.
- **Streaming page:** a new tab with what's on each streaming service (Netflix, HBO Max, Disney+, Prime Video, Apple TV+, …), movies and series apart. Title pages say which services list them.
- **Request what's missing:** a title that isn't on the server opens a page with its details, cast, trailer, seasons and episodes, and a **Request** button. It shows how your request is doing.
- **Search beyond the server:** search also lists matches from the addons under *Not on the server*, as each addon answers.
- **Everything they've been in:** a person's page lists all their movies and series (from Wikidata), filtered by on the server or not, with their photo. Cast photos come from Wikidata too.
- **Missing details filled in:** posters, overviews, logos, cast, genres, scores and episode names your server doesn't have come from the addons. What the server has always wins.
- **More subtitles:** subtitle addons like OpenSubtitles add tracks to the player, under *From addons*. The server's own subtitle file is still picked first.
- **Subtitle timing:** Z and X (Shift for a whole second), or Earlier and Later in the subtitle menu, move subtitles that run early or late.

### Admin
- **Admin › Addons:** add Stremio addons by their manifest link, for everyone. Catalog, metadata and subtitle addons are taken; stream and torrent addons are refused, since the videos come from your server. Configurable addons have a **Configure** button and **New link…** to change their settings.
- **Requests from the app:** they show up under Admin › Requests, *via App*.
- Servers need `GET /api/addons` and `POST`/`DELETE /api/admin/addons` (see `docs/api/admin-addons.md`), and `GET`/`POST /api/requests` for signed-in users. The reference server has them all. Titles are matched by IMDb id, so libraries that send `imdb_id` get the most out of it.

### Downloads
| File | For |
|---|---|
| `valo-v0.8.8-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.8.8-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.8.8-windows-x86_64.zip` | Windows, portable |
| `valo-0.8.8-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.8.8-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.8.8-x86_64.flatpak` | any distro with Flatpak (added a few minutes after the others) |
| `valo_0.8.8-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.8.8-linux-x86_64.tar.gz` | manual install |
| `valo-v0.8.8-androidtv.apk` | Android TV and Google TV (updates itself) |

If you're using the Windows installer, the AppImage or the TV app, click **Update** to get this version.

Checksums for every file except the Flatpak are in `SHA256SUMS.txt`.
