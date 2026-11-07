# Valo v0.9.9

Install and update the Flatpak with `flatpak`, like any other app.

### New
- **Flatpak repository:** Valo now has its own Flatpak repository. Install it with one command:

  ```
  flatpak install --user https://okaza03.github.io/Valo/valo.flatpakref
  ```

  That adds the `valo` repository for your user (the runtime comes from Flathub). From then on, `flatpak install io.github.okaza03.Valo` and `flatpak update` work like for any other app. More at [okaza03.github.io/Valo](https://okaza03.github.io/Valo/).
- **Flatpak bundle:** the `.flatpak` file below now points at the same repository, so a copy installed from it also gets updates through `flatpak update`. If you installed an older bundle, install this one once (or click **Update** in the app) to switch over.

### Downloads
| File | For |
|---|---|
| `valo-v0.9.9-x86_64-setup.exe` | Windows 10/11 (updates itself) |
| `valo-v0.9.9-aarch64-setup.exe` | Windows 11 on ARM64 (updates itself) |
| `valo-0.9.9-windows-x86_64.zip` | Windows, portable |
| `valo-0.9.9-windows-aarch64.zip` | Windows on ARM64, portable |
| `valo-v0.9.9-x86_64.AppImage` | any Linux distro (`chmod +x` and run, updates itself) |
| `valo-v0.9.9-x86_64.flatpak` | any distro with Flatpak (updates itself and through `flatpak update`; added a few minutes after the others) |
| `valo_0.9.9-1_amd64.deb` | Debian / Ubuntu / Mint |
| `valo-0.9.9-linux-x86_64.tar.gz` | manual install |
| `valo-v0.9.9-androidtv.apk` | Android TV and Google TV (updates itself) |
| `valo-v0.9.9-android.apk` | Android phones and tablets (updates itself) |

If you're using the Windows installer, the AppImage, the Flatpak or the TV app, click **Update** to get this version.

Checksums are in `SHA256SUMS.txt`, and for the Flatpak in `SHA256SUMS-flatpak.txt`, which arrives with it.
