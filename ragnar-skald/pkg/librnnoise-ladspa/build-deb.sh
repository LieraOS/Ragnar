#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Rebuild the librnnoise-ladspa .deb from upstream sources.
#
# The plugin itself is built once from the pinned
# noise-suppression-for-voice commit — the skeleton next to this
# script only assembles the .so into a Debian package. Re-running the
# script reproduces the same package byte-for-byte modulo timestamps.
#
# Usage: pkg/librnnoise-ladspa/build-deb.sh [output.deb]

set -euo pipefail

PKG_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="${1:-$PKG_DIR/../../target/librnnoise-ladspa_0.2-1_amd64.deb}"
COMMIT="9c4e5c28d8950e2cef837d8a0abd36c2fd9b5c2d"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

git clone https://github.com/werman/noise-suppression-for-voice "$WORK/nsv"
git -C "$WORK/nsv" checkout "$COMMIT"

cmake -S "$WORK/nsv" -B "$WORK/nsv/build" \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_VST_PLUGIN=OFF -DBUILD_LV2_PLUGIN=OFF \
    -DBUILD_VST3_PLUGIN=OFF -DBUILD_AU_PLUGIN=OFF
cmake --build "$WORK/nsv/build" --target rnnoise_ladspa -j"$(nproc)"

install -Dm644 \
    "$WORK/nsv/build/bin/ladspa/librnnoise_ladspa.so" \
    "$PKG_DIR/usr/lib/ladspa/librnnoise_ladspa.so"
install -Dm644 "$WORK/nsv/LICENSE" \
    "$PKG_DIR/usr/share/doc/librnnoise-ladspa/copyright"

mkdir -p "$(dirname "$OUT")"
dpkg-deb --root-owner-group --build "$PKG_DIR" "$OUT"
echo "Built: $OUT"
