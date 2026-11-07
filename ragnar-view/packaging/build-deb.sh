#!/bin/sh
# Build the .deb: cargo release build → stage pkg tree → dpkg-deb.
set -e
cd "$(dirname "$0")/.."
cargo build --release
cp target/release/ragnar-view pkg/ragnar-view/usr/bin/ragnar-view
for l in de en; do
    mkdir -p "pkg/ragnar-view/usr/share/locale/$l/LC_MESSAGES"
    msgfmt -o "pkg/ragnar-view/usr/share/locale/$l/LC_MESSAGES/ragnar-view.mo" "po/$l.po"
done
mkdir -p pkg/ragnar-view/usr/share/glib-2.0/schemas
glib-compile-schemas --strict --dry-run data
cp data/org.lieraos.ragnar.view.gschema.xml pkg/ragnar-view/usr/share/glib-2.0/schemas/
dpkg-deb --build --root-owner-group pkg/ragnar-view target/debian
