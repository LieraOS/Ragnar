#!/usr/bin/env bash
# Build the ragnar-screenshot .deb via cargo-deb — same pattern as
# the other Ragnar Rust components.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release
for po in po/*.po; do
    lang="$(basename "$po" .po)"
    dest="target/locale/$lang/LC_MESSAGES"
    mkdir -p "$dest"
    msgfmt -o "$dest/ragnar-screenshot.mo" "$po"
done
cargo deb --no-build
