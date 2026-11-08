#!/bin/sh
# Turn flatpak-builder's repository into Valo's Flatpak remote (served by
# GitHub Pages), and build the release's .flatpak bundle from it.
#
#   publish-repo.sh REPO SITE BUNDLE [KEY-ID]
#
# REPO is flatpak-builder's --repo, SITE the folder to publish (the website
# from packaging/site plus the repository), BUNDLE the
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

# `flatpak remote-add valo $BASE/valo.flatpakrepo`. A .flatpakrepo can't name
# the runtime's repository, so the instructions add Flathub before it.
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

# The website: packaging/site, with this release's version and links filled in
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
VERSION=$(grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d'"' -f2)
TAG=${VALO_TAG:-v$VERSION}
RELEASES=https://github.com/Okaza03/Valo/releases
cp -a "$ROOT/packaging/site/." "$SITE/"
sed -i \
    -e "s|@BASE@|$BASE|g" \
    -e "s|@DL@|$RELEASES/download/$TAG|g" \
    -e "s|@TAG@|$TAG|g" \
    -e "s|@VERSION@|${TAG#v}|g" \
    -e "s|@VERSION_LINE@|Latest version: ${TAG#v} · <a href=\"$RELEASES/tag/$TAG\">What's new</a>|g" \
    "$SITE/index.html"
