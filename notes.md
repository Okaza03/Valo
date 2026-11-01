# Valo v0.8.15

Valo is on Android phones and tablets, with casting to your TV. Addons now come from one shared list, and movies that share a name no longer get mixed up.

### Android phones and tablets
- **New app:** everything the TV app has, made for touch: Home, Movies, TV Shows, Streaming, search, title and people pages, requests, My list and your profile. It installs next to the TV app, not over it. Admin isn't in the phone app yet.
- **Casting:** the Cast button plays on a Chromecast or Google TV, and carries on from where you were. The phone stays the remote, and the next episode plays by itself. This release is the first time casting is out in the open, so please report what you find.
- **What casting needs:** Google Play services, and the phone on the same Wi-Fi as the TV, because the video goes through the phone. Chromecasts play H.264 video (4K models HEVC too) with AAC or MP3 sound. Valo doesn't convert files, so anything else can only be watched on the phone. Subtitles inside the video file don't show on the TV.

### Desktop and TV
- **Remakes:** movies with the same name from different years stay separate, so *The Running Man* from 1987 can be requested when the 2025 film is on the server.
- **Streaming links (desktop):** on a title opened from the Streaming tab, the service in *Streaming on* is a link that searches for the title on Netflix, HBO Max, Disney+, Prime Video or Apple TV+.

### Servers
- **Addons from one list:** the Admin page has a list of approved addons with a checkbox on each, instead of a box to paste links in. To get one added, use *Request an addon*, which opens a GitHub issue. Addons you already have stay on, marked *Not on the list*.
- **Bundled server:** an addon keeps its place on the home page when it's turned off and on, and a request only counts as a duplicate with the same IMDb id, or the same title and year.
- **Your own server:** nothing is needed for casting. To keep addon rows in place, base an addon's `id` on its URL ([Addons](https://github.com/Okaza03/Valo/blob/main/docs/api/admin-addons.md)). If your server turns down repeat requests, compare the year as well as the title ([Requests](https://github.com/Okaza03/Valo/blob/main/docs/api/admin-requests.md#for-the-bot)).

### Downloads
| File | For |
|---|---|
| `valo-v0.8.15-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.8.15-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.8.15-windows-x86_64.zip` | Windows, portable |
| `valo-0.8.15-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.8.15-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.8.15-x86_64.flatpak` | any distro with Flatpak (added a few minutes after the others) |
| `valo_0.8.15-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.8.15-linux-x86_64.tar.gz` | manual install |
| `valo-v0.8.15-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.8.15-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage or the TV app, click **Update** to get this version.

Checksums for every file except the Flatpak are in `SHA256SUMS.txt`.
