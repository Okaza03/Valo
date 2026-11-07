# Valo v0.9.8

Smoother playback, and fullscreen that plays nicely with the rest of your desktop.

### Changed
- **Drawing:** Valo now draws with Vulkan or DirectX 12 instead of OpenGL, and waits for your screen's refresh before showing each frame, so video doesn't tear.
- **Fullscreen** is a borderless window covering the screen. On Windows it no longer risks turning into exclusive fullscreen, so Alt+Tab, notifications and overlays work while you watch.

### Downloads
| File | For |
|---|---|
| `valo-v0.9.8-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.9.8-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.9.8-windows-x86_64.zip` | Windows, portable |
| `valo-0.9.8-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.9.8-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.9.8-x86_64.flatpak` | any distro with Flatpak (updates itself; added a few minutes after the others) |
| `valo_0.9.8-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.9.8-linux-x86_64.tar.gz` | manual install |
| `valo-v0.9.8-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.9.8-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage, the Flatpak or the TV app, click **Update** to get this version.

Checksums are in `SHA256SUMS.txt`, and for the Flatpak in `SHA256SUMS-flatpak.txt`, which arrives with it.
