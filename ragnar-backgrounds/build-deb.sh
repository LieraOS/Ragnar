#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) Jörg Schröder
# build-deb.sh — assemble the ragnar-backgrounds .deb.
#
# Every image in data/ is installed to
# /usr/share/backgrounds/ragnar/ and registered in the generated
# gnome-background-properties XML, so dropping a new image into
# data/ and re-running this script is all it takes to extend the
# set. The entry for default.jpg is emitted unconditionally — the
# file itself is shipped by ragnar-core, not by this package.
#
# Usage: build-deb.sh [output.deb]
set -euo pipefail

cd "$(dirname "$0")"
PKG=pkg/ragnar-backgrounds
BG="$PKG/usr/share/backgrounds/ragnar"
PROPS="$PKG/usr/share/gnome-background-properties"
VERSION="$(sed -n 's/^Version: //p' "$PKG/DEBIAN/control")"
OUT="${1:-ragnar-backgrounds_${VERSION}_all.deb}"

rm -rf "$BG" "$PROPS"
mkdir -p "$BG" "$PROPS"
for img in data/*.{jpg,jpeg,png,webp}; do
    [ -e "$img" ] || continue
    install -m644 "$img" "$BG/"
done

# Pretty name from the filename: foo-bar.png -> "Foo Bar".
pretty() {
    basename "$1" | sed 's/\.[^.]*$//; s/[-_]/ /g' | \
        awk '{for(i=1;i<=NF;i++)$i=toupper(substr($i,1,1)) substr($i,2); print}'
}

entry() {
    cat <<EOF
  <wallpaper deleted="false">
    <name>$(pretty "$1")</name>
    <filename>/usr/share/backgrounds/ragnar/$(basename "$1")</filename>
    <options>zoom</options>
    <pcolor>#000000</pcolor>
    <scolor>#000000</scolor>
    <shade_type>solid</shade_type>
  </wallpaper>
EOF
}

{
    echo '<?xml version="1.0" encoding="UTF-8"?>'
    echo '<!DOCTYPE wallpapers SYSTEM "gnome-wp-list.dtd">'
    echo '<wallpapers>'
    entry default.jpg
    for img in data/*.{jpg,jpeg,png,webp}; do
        [ -e "$img" ] || continue
        entry "$img"
    done
    echo '</wallpapers>'
} > "$PROPS/ragnar-backgrounds.xml"

dpkg-deb --root-owner-group --build "$PKG" "$OUT"
echo "built $OUT"
