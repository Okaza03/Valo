# Valo v0.8.13

Watched movies are marked on their posters, and double-clicking the video goes fullscreen without pausing it.

### Desktop and TV
- **Watched sash:** a movie you've watched to the end gets a slanted *Watched* sash across its poster's top right corner, everywhere posters show: the home rows, Movies, My list, search, Top 10, *More like this*, filmographies and New arrivals. On desktop it fades out while you hover, so the My list button can take the corner.

### Desktop
- **Double-click for fullscreen:** double-clicking the video toggles fullscreen and no longer pauses it too. A single click still plays or pauses, just after a short wait in case a second click is coming.
- **Audio:** Valo's sound now plays through cpal 0.18, which has newer Windows audio code underneath. Playback and A/V sync, Bluetooth included, should be the same as before; if sound crackles, cuts out or drifts from the picture, please open an issue.

### Downloads
| File | For |
|---|---|
| `valo-v0.8.13-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.8.13-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.8.13-windows-x86_64.zip` | Windows, portable |
| `valo-0.8.13-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.8.13-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.8.13-x86_64.flatpak` | any distro with Flatpak (added a few minutes after the others) |
| `valo_0.8.13-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.8.13-linux-x86_64.tar.gz` | manual install |
| `valo-v0.8.13-androidtv.apk` | Android TV and Google TV (updates itself) |

If you're using the Windows installer, the AppImage or the TV app, click **Update** to get this version.

Checksums for every file except the Flatpak are in `SHA256SUMS.txt`.
