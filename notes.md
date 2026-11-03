# Valo v0.9.0

Casting from the Android phone app now works, and on any Chromecast: files a TV can't play are converted on the phone as they play, like VLC does.

### Android phones and tablets
- **Casting works:** in 0.8.15 the phone couldn't tell the TV where to find the video, so casting failed with *Couldn't find this phone on the TV's network*. Please update.
- **Plays on any Chromecast:** when a TV refuses a file, the phone tries again and makes it playable as it goes. 5.1 sound becomes stereo for Chromecast dongles, MKV files are rewrapped, and video the device can't decode (4K or HEVC on an older Chromecast) becomes 1080p H.264, with HDR brought down to normal range. Nothing is stored and your files aren't changed. TVs with Chromecast built in still get the original whenever they can play it.
- **What to expect:** a converted film takes a few seconds longer to start, and each skip takes 2 to 3 seconds because the stream starts again. Converting video works the phone hard, so keep it plugged in for a long film. The phone has to be able to decode the file itself.
- **Clearer cast errors:** when a TV still won't play something, the message says what happened, with a line of details to send along with a bug report.

### Desktop and TV
- **Streaming page:** a clearer description under each service.

### Downloads
| File | For |
|---|---|
| `valo-v0.9.0-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.9.0-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.9.0-windows-x86_64.zip` | Windows, portable |
| `valo-0.9.0-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.9.0-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.9.0-x86_64.flatpak` | any distro with Flatpak (added a few minutes after the others) |
| `valo_0.9.0-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.9.0-linux-x86_64.tar.gz` | manual install |
| `valo-v0.9.0-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.9.0-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage or the TV app, click **Update** to get this version.

Checksums for every file except the Flatpak are in `SHA256SUMS.txt`.
