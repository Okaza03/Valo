# Valo v0.8.12

A series' bonus videos get a season of their own: put them in a Featurettes folder next to the seasons.

### Desktop and TV
- **Featurettes:** a series' featurettes show up as a *Featurettes* season after the real ones, in the season picker and everywhere else a season number showed: *Featurette 2* in place of an episode code, and *Season 3 and Featurettes* in New arrivals. They aren't counted in "2 seasons".
- **Season picker:** the desktop's default details page names the chosen season (*Specials*, *Featurettes*, a server's own titles) instead of always showing a number.

### Servers
- **Bundled server:** videos in a show's `Featurettes` folder are listed as featurettes, titled after their files, instead of being skipped for having no `S01E02` in their names.
- **Your own server:** send them as season `999`. [Featurettes](https://github.com/Okaza03/Valo/blob/main/docs/api/featurettes.md) covers what a sync needs: leave the folder's name alone, number the videos by name, skip online lookups and take thumbnails from the videos.

### Downloads
| File | For |
|---|---|
| `valo-v0.8.12-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.8.12-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.8.12-windows-x86_64.zip` | Windows, portable |
| `valo-0.8.12-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.8.12-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.8.12-x86_64.flatpak` | any distro with Flatpak (added a few minutes after the others) |
| `valo_0.8.12-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.8.12-linux-x86_64.tar.gz` | manual install |
| `valo-v0.8.12-androidtv.apk` | Android TV and Google TV (updates itself) |

If you're using the Windows installer, the AppImage or the TV app, click **Update** to get this version.

Checksums for every file except the Flatpak are in `SHA256SUMS.txt`.
