# Valo v0.8.7

Admins can see how each user watches, and ask a user to pick a new password.

### New
- **Change your password in the app:** when an admin asks you for a new password, Valo asks each time you open it until you change it. Type your current password and the new one, or pick Later. Your other devices are signed out; this one stays signed in. On the desktop app.

### Admin
- **A page for each user's watching:** click a user under Admin › Analytics › By user to see their time watched, plays per day, what they watched recently and how far they got, their top titles and favorite genres, the hours and days they watch most, and the devices they're signed in on. The servers you run need the new `GET /api/admin/analytics/users/<name>`; the reference server has it.
- **Require a new password:** a user's menu in Admin › Users has Require a new password. Their row says so until they've changed it. Servers need `must_change_password` and `POST /api/password` (see the sign-in and admin API guides); the reference server has both.

### Downloads
| File | For |
|---|---|
| `valo-v0.8.7-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.8.7-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.8.7-windows-x86_64.zip` | Windows, portable |
| `valo-0.8.7-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.8.7-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.8.7-x86_64.flatpak` | any distro with Flatpak (added a few minutes after the others) |
| `valo_0.8.7-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.8.7-linux-x86_64.tar.gz` | manual install |
| `valo-v0.8.7-androidtv.apk` | Android TV and Google TV (updates itself) |

If you're using the Windows installer, the AppImage or the TV app, click **Update** to get this version.

Checksums for every file except the Flatpak are in `SHA256SUMS.txt`.
