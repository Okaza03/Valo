# Valo v0.9.14

A folder sync dry run now shows new episodes and movies you've added.

### Fixed
- **Sync folders finds new files:** the dry run lists files that aren't in the library yet under **New on disk**, and turns on **Sync series** or **Sync movies** to add them. Before, a new episode didn't show up at all.

### New
- **More in the folder scan:** **To rename** lists files whose names don't follow the library's naming, with the server's suggested name after the arrow. **Not edited** lists entries no sync has fetched details for yet. Missing images can also list seasons.

### Downloads
| File | For |
|---|---|
| `valo-v0.9.14-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.9.14-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.9.14-windows-x86_64.zip` | Windows, portable |
| `valo-0.9.14-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.9.14-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.9.14-x86_64.flatpak` | any distro with Flatpak (updates itself and through `flatpak update`; added a few minutes after the others) |
| `valo_0.9.14-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.9.14-linux-x86_64.tar.gz` | manual install |
| `valo-v0.9.14-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.9.14-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage, the Flatpak or the TV app, click **Update** to get this version.

Checksums are in `SHA256SUMS.txt`, and for the Flatpak in `SHA256SUMS-flatpak.txt`, which arrives with it.
