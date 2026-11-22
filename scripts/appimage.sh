#!/bin/bash
# Builds the launcher's AppImage: build/The-Aegis-Oppai-XC2S2-<version>-x86_64.AppImage
#
# Needs: Rust (cargo), the Tauri CLI (cargo install tauri-cli --version "^2" --locked), WebKitGTK 4.1 and its headers
# (Arch: webkit2gtk-4.1; Debian/Ubuntu: libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev),
# Python with Pillow for the icon. Tauri's bundler downloads linuxdeploy on its first run.
#
# The AppImage carries the WebKitGTK of the system it is built on: built on a recent rolling distribution it runs on systems
# as new as it, built on Ubuntu 22.04 (the CI workflow) it runs on nearly all of them.
# Nothing of the game, no game art and no NVIDIA or AMD file goes in it.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -1)
[ -f src-tauri/icons/icon.png ] || python3 -I icon/make_icon.py
[ -f build/sdl3/libSDL3.so.0 ] || echo "aviso: sin build/sdl3/libSDL3.so.0 (scripts/build-sdl3.sh): Proton sin SDL3 del sistema no verá mandos"
cd src-tauri
# NO_STRIP: linuxdeploy's own strip is too old for the libraries of current distributions
NO_STRIP=true APPIMAGE_EXTRACT_AND_RUN=1 cargo tauri build --bundles appimage "$@"
OUT=$(ls -t target/release/bundle/appimage/*.AppImage | head -1)
mkdir -p "$ROOT/build"
DEST="$ROOT/build/The-Aegis-Oppai-XC2S2-$VERSION-x86_64.AppImage"
# a new file (not overwritten in place): a launcher that is open keeps running from the old one
rm -f "$DEST"
cp "$OUT" "$DEST"
chmod +x "$DEST"
# check: no Windows DLL (NVIDIA, AMD, the port's) and no picture other than our icon went in
T=$(mktemp -d)
(cd "$T" && "$DEST" --appimage-extract >/dev/null)
BAD=$(find "$T/squashfs-root" -iname '*.dll' -o -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.nup' -o -iname '*.nca' | head -5)
rm -rf "$T"
[ -z "$BAD" ] || { echo "the AppImage carries files it must not: $BAD"; exit 1; }
echo "$DEST: $(du -h "$DEST" | cut -f1)"
