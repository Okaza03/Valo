# Valo v0.11.0

Videos are decoded on the graphics card, and a fix for the Admin page's folder scan.

### New
- **Hardware decoding:** the desktop app decodes video on the graphics card when it can, which takes much less CPU and power (about 60% less CPU on a 4K 10-bit HEVC file in testing). Linux uses Vulkan Video (NVIDIA, AMD and Intel), Windows uses Direct3D 11 and macOS uses VideoToolbox. When the graphics card can't decode a file, Valo switches to software decoding by itself.
- **A setting for it:** Settings › Playback › **Use hardware video decoding**, on by default. Turn it off if a video shows glitches. It applies from the next video.
- **See how a video is decoded:** the player's Info panel shows **Decoding: Hardware (Vulkan)**, or **Software** and why.

### Fixed
- **Admin › Dashboard:** after a folder scan that found removed titles, **Sync movies** and **Sync series** stayed off and the scan said nothing had changed. They now turn on for whichever kind was added or removed, and the removed titles are listed again.

### Downloads
| File | For |
|---|---|
| `valo-v0.11.0-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.11.0-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.11.0-windows-x86_64.zip` | Windows, portable |
| `valo-0.11.0-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.11.0-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.11.0-x86_64.flatpak` | any distro with Flatpak (updates itself and through `flatpak update`; added a few minutes after the others) |
| `valo_0.11.0-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.11.0-linux-x86_64.tar.gz` | manual install |
| `valo-v0.11.0-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.11.0-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage, the Flatpak or the TV app, click **Update** to get this version.

Checksums are in `SHA256SUMS.txt`, and for the Flatpak in `SHA256SUMS-flatpak.txt`, which arrives with it.
