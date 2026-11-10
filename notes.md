# Valo v0.9.11

Smoother playback, and Valo keeps better track of what you've watched.

### Fixed
- **Fullscreen:** the picture no longer freezes the first time a video goes fullscreen, and leaving fullscreen is instant again.
- **Stuttering:** 4K and other heavy videos no longer stutter in busy scenes. Video now decodes on all of your processor's cores.

### Changed
- **Watched:** a movie counts as watched once you're 90% of the way through, an episode at 95%, or as soon as you click **Next Episode**.
- **Finished series:** once you've watched the last episode, the series leaves Continue watching (it used to come back as its first episode), gets the WATCHED sash and plays again from the start.

### New
- **Series page:** each episode you've watched says so, and the page tells you when you've watched the whole season or the whole series.
- **Admin › Users:** anyone who used Valo in the last five minutes shows as **Active now**.

### Downloads
| File | For |
|---|---|
| `valo-v0.9.11-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.9.11-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.9.11-windows-x86_64.zip` | Windows, portable |
| `valo-0.9.11-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.9.11-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.9.11-x86_64.flatpak` | any distro with Flatpak (updates itself and through `flatpak update`; added a few minutes after the others) |
| `valo_0.9.11-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.9.11-linux-x86_64.tar.gz` | manual install |
| `valo-v0.9.11-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.9.11-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage, the Flatpak or the TV app, click **Update** to get this version.

Checksums are in `SHA256SUMS.txt`, and for the Flatpak in `SHA256SUMS-flatpak.txt`, which arrives with it.
