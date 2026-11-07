# Flatpak

- `io.github.okaza03.Valo.yml`: the manifest. It packages the binary the release workflow builds on Ubuntu 22.04, instead of compiling inside flatpak-builder.
- `io.github.okaza03.Valo.metainfo.xml`: the app's metadata. Every release adds a `<release>` line here.
- `publish-repo.sh`: turns flatpak-builder's repository into Valo's Flatpak remote, and builds the release's `.flatpak` bundle from it.

## The repository

Each release publishes a signed Flatpak repository to GitHub Pages, at <https://okaza03.github.io/Valo/>:

| File | For |
|---|---|
| `repo/` | the OSTree repository `flatpak` reads (only the latest release) |
| `valo.flatpakref` | `flatpak install --user https://okaza03.github.io/Valo/valo.flatpakref`: adds the remote and installs |
| `valo.flatpakrepo` | `flatpak remote-add --user valo https://okaza03.github.io/Valo/valo.flatpakrepo`, after adding Flathub for the runtime (a `.flatpakrepo` can't name it) |

The bundle attached to each release names the same repository, so copies installed from a bundle also get `flatpak update`. The bundle is attached only after Pages has the new version, so the app's **Update now** never finds a release that `flatpak update` can't install yet.

To try the script locally, serve its output and point it there:

```
VALO_FLATPAK_BASE=http://127.0.0.1:8765 packaging/flatpak/publish-repo.sh repo site valo.flatpak [KEY-ID]
(cd site && python3 -m http.server 8765)
```

## One-time setup

1. **A signing key.** `flatpak` checks the repository against the key in the `.flatpakrepo` and `.flatpakref` files, so the same key has to sign every release. Make one just for this, with no passphrase, in a throwaway GnuPG home:

   ```
   export GNUPGHOME=$(mktemp -d)
   gpg --batch --passphrase '' --quick-gen-key "Valo Flatpak repository" ed25519 sign never
   gpg --armor --export-secret-keys | gh secret set FLATPAK_GPG_KEY --repo Okaza03/Valo
   ```

   Keep a copy of the secret key somewhere safe. If it's lost, a new key means every user has to remove the `valo` remote and add it again.

2. **GitHub Pages, deployed by Actions:**

   ```
   gh api -X POST repos/Okaza03/Valo/pages -f build_type=workflow
   ```

3. **Let release tags deploy.** The `github-pages` environment only accepts the default branch at first:

   ```
   gh api -X POST repos/Okaza03/Valo/environments/github-pages/deployment-branch-policies -f name='v*' -f type=tag
   ```

Without the `FLATPAK_GPG_KEY` secret, a tagged release stops at the Flatpak job and names the missing secret.
