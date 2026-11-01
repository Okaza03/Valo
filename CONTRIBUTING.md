# Contributing to Valo

Thanks for wanting to help. Bug reports, server compatibility reports, docs fixes and code are all welcome.

## Before you start

- **Bugs and ideas**: open an [issue](https://github.com/Okaza03/Valo/issues/new/choose) first, using the matching template.
- **Bigger changes** (new screens, a new server feature, changes to the API in `docs/api/`): open an issue to talk it through before writing code, so your work doesn't go to waste.
- **Security problems**: don't open a public issue. See [SECURITY.md](SECURITY.md).
- **Anything else**: email [Support@jhns.no](mailto:Support@jhns.no).

Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Project layout

| Path                | What's in it                                                      |
| ------------------- | ----------------------------------------------------------------- |
| `apps/valo/`        | The desktop app (Rust, egui, FFmpeg)                              |
| `apps/androidtv/`   | The Android apps: `src/main` is shared, `src/tv` is Android TV / Google TV, `src/phone` is phones and tablets (`./gradlew assembleTvRelease assemblePhoneRelease`) |
| `server/`           | The bundled Python server (standard library only)                 |
| `docs/api/`         | The server API guide                                              |
| `xtask/`            | Build helpers (`cargo xtask ffmpeg`, `dist`, `installer`)         |
| `packaging/`        | Flatpak, Windows installer and other packaging files              |

## Building

```sh
cargo run --release -p valo
```

On Windows, run `cargo xtask ffmpeg` once first and install LLVM (`winget install LLVM.LLVM`). The [README](README.md) has the details.

To test against a server, run the bundled one:

```sh
python3 server/valo_server.py ~/Videos
```

## Making a change

1. Fork the repo and create a branch from `main`.
2. Keep the change focused: one fix or feature per pull request.
3. Before pushing, run:
   ```sh
   cargo fmt --all
   cargo clippy --workspace --all-targets
   cargo test --workspace
   ```
4. If you changed what the server must send or accept, update `docs/api/` and `server/valo_server.py` to match.
5. If you changed something users see, say so in the pull request. Release notes in `notes.md` are written by the maintainer at release time, so you don't need to edit it.

## Commit messages

Use the [Conventional Commits](https://www.conventionalcommits.org/) style, as in the existing history:

```
feat: subtitle search puts the ones made for your file first
fix: picture and subtitles wait for the sound you hear
docs: explain the progress endpoint
```

Write the summary in plain words about what changes for the user.

## Pull requests

- Fill in the pull request template.
- Link the issue it closes (`Closes #123`).
- Add screenshots or a short clip for UI changes.
- Say which platforms you tested on (Linux, Windows, Android TV, Android phone).

## License

Valo is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. Unless you say otherwise, any contribution you submit for inclusion in Valo is dual-licensed the same way, without any additional terms or conditions.
