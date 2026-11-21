#!/bin/bash
# stage-components.sh — build the Ragnar core Rust components and
# stage their files into debian/ragnar-core/ (the bundle package).
#
# The 17 session-critical components live in sibling directories of
# this package (monorepo layout); each is a standalone cargo project.
# This script is invoked from debian/rules during the build and
# mirrors exactly what the former per-component .debs shipped —
# the manifest below is derived from the actual package contents,
# not guessed.
#
# Usage: stage-components.sh <ragnar-root> <dest-dir>
set -euo pipefail

ROOT="$1"   # monorepo root (parent of ragnar-desktop/)
DEST="$2"   # staging dir, e.g. debian/ragnar-core

# install_file <src> <dest-path> <mode>
install_file() {
    install -D -m "$3" "$1" "$DEST/$2"
}

# build_component <dir>
# Builds the release binary and compiles every po/*.po into
# target/locale/<lang>/LC_MESSAGES/<crate>.mo — mirroring the old
# per-package build-deb.sh behaviour.
build_component() {
    local dir="$ROOT/$1"
    local name
    name="$(basename "$dir")"
    ( cd "$dir" && cargo build --release --quiet )
    if [ -d "$dir/po" ]; then
        for po in "$dir"/po/*.po; do
            [ -e "$po" ] || continue
            local lang
            lang="$(basename "$po" .po)"
            local out="$dir/target/locale/$lang/LC_MESSAGES"
            mkdir -p "$out"
            msgfmt -o "$out/$name.mo" "$po"
        done
    fi
}

# mo <crate> <lang> — stage a compiled locale file.
mo() {
    install_file "$ROOT/$1/target/locale/$2/LC_MESSAGES/$1.mo" \
        "usr/share/locale/$2/LC_MESSAGES/$1.mo" 644
}

COMPONENTS="ragnar-session ragnar-power-dialog ragnar-taskbar \
ragnar-view ragnar-status-notifier ragnar-screenlock \
ragnar-polkit-agent ragnar-keyring ragnar-display-svc \
ragnar-automount ragnar-brightness-helper ragnar-nightlight \
ragnar-osd ragnar-mpris ragnar-gtk3-sync ragnar-keyboard-layout \
ragnar-run-dialog"

for c in $COMPONENTS; do
    build_component "$c"
done

# ---------------- ragnar-session ----------------
install_file "$ROOT/ragnar-session/target/release/ragnar-session" usr/libexec/ragnar-session 755
install_file "$ROOT/ragnar-session/target/release/ragnar-session-quit" usr/bin/ragnar-session-quit 755
install_file "$ROOT/ragnar-session/target/release/ragnar-session-inhibit" usr/bin/ragnar-session-inhibit 755
install_file "$ROOT/ragnar-session/target/release/ragnar-session-compositor-ready" usr/libexec/ragnar-session-compositor-ready 755
install_file "$ROOT/ragnar-session/data/ragnar-session" usr/bin/ragnar-session 755
install_file "$ROOT/ragnar-session/data/ragnar-session.target" usr/lib/systemd/user/ragnar-session.target 644
install_file "$ROOT/ragnar-session/data/org.lieraos.ragnar.SessionManager.gschema.xml" usr/share/glib-2.0/schemas/org.lieraos.ragnar.SessionManager.gschema.xml 644

# ---------------- ragnar-power-dialog ----------------
install_file "$ROOT/ragnar-power-dialog/target/release/ragnar-power-dialog" usr/bin/ragnar-power-dialog 755
install_file "$ROOT/ragnar-power-dialog/data/org.lieraos.ragnar.session.gschema.xml" usr/share/glib-2.0/schemas/org.lieraos.ragnar.session.gschema.xml 644
install_file "$ROOT/ragnar-power-dialog/data/power-dialog.css" usr/share/ragnar-power-dialog/power-dialog.css 644
mo ragnar-power-dialog en
# Autostart .desktop is shipped by the meson install of
# ragnar-desktop itself (etc/xdg/autostart) — not duplicated here.

# ---------------- ragnar-taskbar ----------------
install_file "$ROOT/ragnar-taskbar/target/release/ragnar-taskbar" usr/bin/ragnar-taskbar 755
install_file "$ROOT/ragnar-taskbar/data/ragnar-taskbar.desktop" etc/xdg/autostart/ragnar-taskbar.desktop 644
mo ragnar-taskbar en

# ---------------- ragnar-view ----------------
install_file "$ROOT/ragnar-view/target/release/ragnar-view" usr/bin/ragnar-view 755
install_file "$ROOT/ragnar-view/data/org.lieraos.ragnar.View.desktop" etc/xdg/autostart/org.lieraos.ragnar.View.desktop 644
install_file "$ROOT/ragnar-view/data/org.lieraos.ragnar.view.gschema.xml" usr/share/glib-2.0/schemas/org.lieraos.ragnar.view.gschema.xml 644
mo ragnar-view de
mo ragnar-view en

# ---------------- ragnar-status-notifier ----------------
install_file "$ROOT/ragnar-status-notifier/target/release/ragnar-status-notifier" usr/bin/ragnar-status-notifier 755
install_file "$ROOT/ragnar-status-notifier/data/org.lieraos.ragnar.StatusNotifier.desktop" etc/xdg/autostart/org.lieraos.ragnar.StatusNotifier.desktop 644

# ---------------- ragnar-screenlock ----------------
install_file "$ROOT/ragnar-screenlock/target/release/ragnar-screenlock" usr/bin/ragnar-screenlock 755
install_file "$ROOT/ragnar-screenlock/data/org.lieraos.ragnar.Screenlock.desktop" etc/xdg/autostart/org.lieraos.ragnar.Screenlock.desktop 644

# ---------------- ragnar-polkit-agent ----------------
install_file "$ROOT/ragnar-polkit-agent/target/release/ragnar-polkit-agent" usr/libexec/ragnar-desktop/ragnar-polkit-agent 755
install_file "$ROOT/ragnar-polkit-agent/data/org.lieraos.ragnar.Polkit.desktop" etc/xdg/autostart/org.lieraos.ragnar.Polkit.desktop 644
mo ragnar-polkit-agent en

# ---------------- ragnar-keyring ----------------
install_file "$ROOT/ragnar-keyring/target/release/ragnar-keyring" usr/libexec/ragnar-keyring 755
install_file "$ROOT/ragnar-keyring/data/org.lieraos.ragnar.Keyring.desktop" etc/xdg/autostart/org.lieraos.ragnar.Keyring.desktop 644

# ---------------- ragnar-display-svc ----------------
install_file "$ROOT/ragnar-display-svc/target/release/ragnar-display-svc" usr/bin/ragnar-display-svc 755
install_file "$ROOT/ragnar-display-svc/data/org.lieraos.ragnar.DisplayServices.desktop" etc/xdg/autostart/org.lieraos.ragnar.DisplayServices.desktop 644

# ---------------- ragnar-automount ----------------
install_file "$ROOT/ragnar-automount/target/release/ragnar-automount" usr/bin/ragnar-automount 755
install_file "$ROOT/ragnar-automount/data/org.lieraos.ragnar.Automount.desktop" etc/xdg/autostart/org.lieraos.ragnar.Automount.desktop 644
install_file "$ROOT/ragnar-automount/data/polkit-1/rules.d/60-ragnar-automount.rules" etc/polkit-1/rules.d/60-ragnar-automount.rules 644

# ---------------- ragnar-brightness-helper ----------------
install_file "$ROOT/ragnar-brightness-helper/target/release/ragnar-brightness-helper" usr/bin/ragnar-brightness-helper 755

# ---------------- ragnar-nightlight ----------------
install_file "$ROOT/ragnar-nightlight/target/release/ragnar-nightlight" usr/bin/ragnar-nightlight 755
install_file "$ROOT/ragnar-nightlight/data/org.lieraos.ragnar.NightLight.desktop" etc/xdg/autostart/org.lieraos.ragnar.NightLight.desktop 644

# ---------------- ragnar-osd ----------------
install_file "$ROOT/ragnar-osd/target/release/ragnar-osd" usr/bin/ragnar-osd 755
install_file "$ROOT/ragnar-osd/data/org.lieraos.ragnar.OSD.desktop" etc/xdg/autostart/org.lieraos.ragnar.OSD.desktop 644

# ---------------- ragnar-mpris ----------------
install_file "$ROOT/ragnar-mpris/target/release/ragnar-mpris" usr/bin/ragnar-mpris 755
install_file "$ROOT/ragnar-mpris/data/org.lieraos.ragnar.MprisController.desktop" etc/xdg/autostart/org.lieraos.ragnar.MprisController.desktop 644

# ---------------- ragnar-gtk3-sync ----------------
install_file "$ROOT/ragnar-gtk3-sync/target/release/ragnar-gtk3-sync" usr/bin/ragnar-gtk3-sync 755
install_file "$ROOT/ragnar-gtk3-sync/data/org.lieraos.ragnar.Gtk3Sync.desktop" etc/xdg/autostart/org.lieraos.ragnar.Gtk3Sync.desktop 644

# ---------------- ragnar-keyboard-layout ----------------
install_file "$ROOT/ragnar-keyboard-layout/target/release/ragnar-keyboard-layout" usr/bin/ragnar-keyboard-layout 755
install_file "$ROOT/ragnar-keyboard-layout/data/org.lieraos.ragnar.KeyboardLayout.desktop" etc/xdg/autostart/org.lieraos.ragnar.KeyboardLayout.desktop 644

# ---------------- ragnar-run-dialog ----------------
install_file "$ROOT/ragnar-run-dialog/target/release/ragnar-run-dialog" usr/bin/ragnar-run-dialog 755
install_file "$ROOT/ragnar-run-dialog/data/run-dialog.css" usr/share/ragnar-run-dialog/run-dialog.css 644
mo ragnar-run-dialog en
