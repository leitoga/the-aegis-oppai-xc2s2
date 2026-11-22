#!/bin/bash
# The AppImage for every distribution: built inside an Ubuntu 22.04 container (podman or docker), so it carries Ubuntu 22.04's
# WebKitGTK and needs nothing newer than its glibc 2.35. The same file then runs on Kubuntu/Ubuntu 22.04 and newer, Mint, Pop!_OS,
# Arch, CachyOS, Manjaro, EndeavourOS, Fedora, openSUSE...
#
#   scripts/appimage-ubuntu.sh   -> build/The-Aegis-Oppai-XC2S2-<version>-x86_64.AppImage
#
# Needs podman (Arch/CachyOS: sudo pacman -S podman) or docker. The container's cargo and apt caches stay in build/.ubuntu-cache,
# so the second build is quick.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
ENGINE=$(command -v podman || command -v docker || true)
[ -n "$ENGINE" ] || { echo "podman o docker hace falta (Arch/CachyOS: sudo pacman -S podman)"; exit 1; }
mkdir -p build/.ubuntu-cache/cargo build/.ubuntu-cache/target
# half the threads and low priority: the PC stays usable while it builds (all of them froze a 16 GB PC with a game open)
JOBS=${JOBS:-$(( $(nproc) / 2 ))}
exec nice -n 15 ionice -c3 "$ENGINE" run --rm -i \
    -e CARGO_BUILD_JOBS="$JOBS" \
    -v "$ROOT:/src:Z" \
    -v "$ROOT/build/.ubuntu-cache/cargo:/root/.cargo:Z" \
    -v "$ROOT/build/.ubuntu-cache/target:/src/src-tauri/target:Z" \
    -e CARGO_HOME=/root/.cargo -e APPIMAGE_EXTRACT_AND_RUN=1 -e XC2L_REPO="${XC2L_REPO:-}" \
    docker.io/library/ubuntu:22.04 bash -euo pipefail -c '
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends ca-certificates curl build-essential pkg-config file xz-utils python3 python3-pil \
    libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev libssl-dev cmake libudev-dev >/dev/null
if [ ! -x /root/.cargo/bin/cargo ]; then
    curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path
fi
export PATH=/root/.cargo/bin:$PATH
rustup default stable >/dev/null
[ -x /root/.cargo/bin/cargo-tauri ] || cargo install tauri-cli --version "^2" --locked
cd /src
rm -rf build/sdl3
scripts/build-sdl3.sh
(cd src-tauri && cargo test --quiet)
scripts/appimage.sh
ldd --version | sed -n 1p
'
