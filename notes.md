# Valo v0.10.0

Skip the intro and the recap, go on to the next episode at the credits, a redesigned Admin page, and new icons throughout.

### New
- **Skip Intro and Skip Recap:** while an episode plays, a button above the timeline's right corner skips the intro or the recap (or press **S**).
- **Next Episode at the credits:** when the credits start, **Next episode in 5…1** counts down and plays the next episode, while *Play the next episode automatically* is on. **Watch Credits** stops it. Without known credits, **Next Episode** shows in the last 20 seconds.
- **Where the times come from:**
  - chapter names in the file ("Opening", "Previously on", "End Credits")
  - a server that detects them, by matching the theme song and credits music between episodes
  - times an admin sets by hand
- **Admin › Skip markers:** pick series and let the server find their times. Series with times are listed apart, where each episode's recap, intro and credits can be seen and fixed by hand.
- **A new look for the Admin page:**
  - a sidebar with icons, cards and stat cards, and tabs
  - an area chart for plays per day
  - a user's analytics page that opens with a profile card
  - the mouse's back and forward buttons move between Admin sections
- **Check for Updates** sits at the bottom right of the window, and becomes **Download Updates** when a new version is out.
- **Lucide icons** throughout the app.

### Fixed
- A click below the timeline, between the player's buttons, no longer pauses.
- The profile menu closes when the pointer moves away from it.
- Analytics, Library and Collections in the Admin page no longer have a wide gap on the left in a big window.
- A user's **Recently watched** in Admin › Analytics shows each title's picture.
- The status line separates its parts with | instead of dots.

### Downloads
| File | For |
|---|---|
| `valo-v0.10.0-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.10.0-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.10.0-windows-x86_64.zip` | Windows, portable |
| `valo-0.10.0-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.10.0-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.10.0-x86_64.flatpak` | any distro with Flatpak (updates itself and through `flatpak update`; added a few minutes after the others) |
| `valo_0.10.0-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.10.0-linux-x86_64.tar.gz` | manual install |
| `valo-v0.10.0-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.10.0-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage, the Flatpak or the TV app, click **Update** to get this version.

Checksums are in `SHA256SUMS.txt`, and for the Flatpak in `SHA256SUMS-flatpak.txt`, which arrives with it.
