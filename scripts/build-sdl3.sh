#!/bin/bash
# Builds SDL3 (zlib license) from its official source release into build/sdl3/libSDL3.so.0. The launcher carries it and hands
# it to Proton when the system has none: Proton's controller bus loads the system's libSDL3.so.0, and without it the game sees
# no controller at all with DLSS ("Failed loading SDL3 library.").
#
# Built where the AppImage is built (Ubuntu 22.04), so it needs nothing newer than glibc 2.35. Only what a controller bus
# needs (joystick, gamepad, haptic, sensors, events): no video or audio, so it builds anywhere without their headers.
# Needs: cmake, a C compiler, libudev-dev (controllers through udev), curl.
set -euo pipefail
cd "$(dirname "$0")/.."
VER=3.4.20
SHA=f3bb06f55bed9557227a700af2ea62e49497ebdf58037732a731a5fb3ef8581f
OUT=build/sdl3/libSDL3.so.0
STAMP=build/sdl3/version
if [ -f "$OUT" ] && [ "$(cat "$STAMP" 2>/dev/null)" = "$VER" ]; then
    echo "$OUT ($VER) ya está"
    exit 0
fi
W=build/.sdl3
mkdir -p "$W" build/sdl3
TAR="$W/SDL3-$VER.tar.gz"
if ! echo "$SHA  $TAR" | sha256sum -c --status 2>/dev/null; then
    curl -fsSL -o "$TAR" "https://github.com/libsdl-org/SDL/releases/download/release-$VER/SDL3-$VER.tar.gz"
    echo "$SHA  $TAR" | sha256sum -c --status || { echo "SDL3-$VER.tar.gz: SHA-256 no coincide"; exit 1; }
fi
rm -rf "$W/src" "$W/out"
mkdir -p "$W/src"
tar -xzf "$TAR" -C "$W/src" --strip-components=1
cmake -S "$W/src" -B "$W/out" -DCMAKE_BUILD_TYPE=Release -DSDL_SHARED=ON -DSDL_STATIC=OFF -DSDL_TESTS=OFF \
    -DSDL_EXAMPLES=OFF -DSDL_INSTALL=OFF -DSDL_TEST_LIBRARY=OFF \
    -DSDL_UNIX_CONSOLE_BUILD=ON -DSDL_VIDEO=OFF -DSDL_RENDER=OFF -DSDL_GPU=OFF -DSDL_CAMERA=OFF -DSDL_AUDIO=OFF >/dev/null
cmake --build "$W/out" -j "${JOBS:-$(nproc)}" >/dev/null
cp -L "$W/out/libSDL3.so.0" "$OUT"
strip --strip-unneeded "$OUT" 2>/dev/null || true
echo "$VER" > "$STAMP"
rm -rf "$W/src" "$W/out"
echo "$OUT: $(du -h "$OUT" | cut -f1)"
