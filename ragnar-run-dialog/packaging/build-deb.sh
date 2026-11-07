#!/usr/bin/env bash
# Build the ragnar-run-dialog .deb: release binary, default CSS
# template and gettext catalogs. cargo-deb stages everything
# declared in [package.metadata.deb] of Cargo.toml and resolves
# shared-library dependencies itself ($auto).
#
# Output: target/debian/ragnar-run-dialog_<version>-<rev>_amd64.deb
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
    msgfmt -o "$dest/ragnar-run-dialog.mo" "$po"
done

# --no-build: the release binary above is fresh; cargo-deb would
# rebuild it otherwise.
cargo deb --no-build
