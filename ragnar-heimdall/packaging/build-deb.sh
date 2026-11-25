#!/usr/bin/env bash
# Build the ragnar-heimdall .deb package: release binary plus the desktop
# entry. cargo-deb stages everything declared in the
# [package.metadata.deb] assets table of Cargo.toml and resolves the
# shared-library dependencies itself ($auto -> dpkg-shlibdeps).
#
# Output: target/debian/ragnar-heimdall_<version>-<rev>_amd64.deb
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release

# Compile every gettext catalog in po/ into a staged .mo — the
# assets table picks them up from target/locale/<lang>/.
for po in po/*.po; do
    [ -e "$po" ] || continue
    lang="$(basename "$po" .po)"
    dest="target/locale/$lang/LC_MESSAGES"
    mkdir -p "$dest"
    msgfmt -o "$dest/ragnar-heimdall.mo" "$po"
done

# --no-build: the release binary above is fresh; cargo-deb would
# rebuild it otherwise.
cargo deb --no-build
