#!/bin/sh
# Turn flatpak-builder's repository into Valo's Flatpak remote (served by
# GitHub Pages), and build the release's .flatpak bundle from it.
#
#   publish-repo.sh REPO SITE BUNDLE [KEY-ID]
#
# REPO is flatpak-builder's --repo, SITE the folder to publish, BUNDLE the
# bundle to write. With KEY-ID (a key in the default GnuPG home), the
# repository's summary is signed and the key goes into the .flatpakrepo and
# .flatpakref files and the bundle, so flatpak can check what it downloads.
set -eu

REPO=$1
SITE=$2
BUNDLE=$3
KEY=${4:-}

APP=io.github.okaza03.Valo
BRANCH=master # flatpak-builder's default; the bundles have always used it
# where GitHub Pages serves the site (overridable to test the script locally)
BASE=${VALO_FLATPAK_BASE:-https://okaza03.github.io/Valo}
URL=$BASE/repo/
RUNTIME_REPO=https://dl.flathub.org/repo/flathub.flatpakrepo
TITLE="Valo"
COMMENT="A small, fast video player and media library client"
HOMEPAGE=https://github.com/Okaza03/Valo
ICON=https://raw.githubusercontent.com/Okaza03/Valo/main/packaging/valo.svg

SIGN=""
GPGKEY_LINE=""
if [ -n "$KEY" ]; then
    SIGN="--gpg-sign=$KEY"
    PUBKEY=$(mktemp)
    trap 'rm -f "$PUBKEY"' EXIT
    gpg --export "$KEY" > "$PUBKEY"
    GPGKEY_LINE="GPGKey=$(base64 -w0 < "$PUBKEY")"
fi

# The site holds only this release: a client that has an older one just
# pulls the new commit. Static deltas make that one download.
# shellcheck disable=SC2086 # $SIGN is empty or one word
flatpak build-update-repo $SIGN \
    --title="$TITLE" --comment="$COMMENT" --homepage="$HOMEPAGE" --icon="$ICON" \
    --default-branch="$BRANCH" --generate-static-deltas --prune "$REPO"

# The bundle names the remote, so a copy installed from it also gets
# `flatpak update` (and the key, so those updates are checked).
# shellcheck disable=SC2086
flatpak build-bundle --repo-url="$URL" --runtime-repo="$RUNTIME_REPO" \
    ${KEY:+--gpg-keys="$PUBKEY"} "$REPO" "$BUNDLE" "$APP" "$BRANCH"

rm -rf "$SITE"
mkdir -p "$SITE"
cp -a "$REPO" "$SITE/repo"
rm -rf "$SITE/repo/tmp"

# `flatpak remote-add valo $BASE/valo.flatpakrepo`
cat > "$SITE/valo.flatpakrepo" <<EOF
[Flatpak Repo]
Title=$TITLE
Url=$URL
Homepage=$HOMEPAGE
Comment=$COMMENT
Icon=$ICON
$GPGKEY_LINE
EOF

# `flatpak install $BASE/valo.flatpakref`: adds the remote and installs, in one go
cat > "$SITE/valo.flatpakref" <<EOF
[Flatpak Ref]
Name=$APP
Branch=$BRANCH
Title=$TITLE
Url=$URL
Homepage=$HOMEPAGE
Comment=$COMMENT
Icon=$ICON
RuntimeRepo=$RUNTIME_REPO
SuggestRemoteName=valo
IsRuntime=false
$GPGKEY_LINE
EOF

cat > "$SITE/index.html" <<EOF
<!doctype html>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Valo for Flatpak</title>
<style>
  body { font: 16px/1.5 system-ui, sans-serif; max-width: 42rem; margin: 3rem auto; padding: 0 1rem; color: #ddd; background: #18181b; }
  code, pre { background: #26262b; border-radius: 6px; }
  pre { padding: .75rem 1rem; overflow-x: auto; }
  a { color: #8ab4ff; }
</style>
<h1>Valo for Flatpak</h1>
<p>Install Valo and keep it up to date with <code>flatpak</code>:</p>
<pre>flatpak install --user $BASE/valo.flatpakref</pre>
<p>Or add the repository once, then install by name:</p>
<pre>flatpak remote-add --user --if-not-exists valo $BASE/valo.flatpakrepo
flatpak install --user io.github.okaza03.Valo</pre>
<p><code>flatpak update</code> gets new versions. The runtime comes from Flathub.</p>
<p><a href="$HOMEPAGE">Valo on GitHub</a></p>
EOF
