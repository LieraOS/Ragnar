# ragnar-software

Software management for the Ragnar desktop — GPL-3.0-or-later,
part of the Ragnar family. German version: [README_de.md](README_de.md)

GTK4/libadwaita application with five sidebar pages:

- **Welcome** — background update status
- **Updates** — selectable list, severities, changelog pane,
  dependency preview, progress dialog; covers apt packages
  (PackageKit `RefreshCache` + `GetUpdates`) and Flatpak
  updates (Flathub, user scope)
- **Software** — AppStream catalogue (Debian DEP-11 + Flathub XML),
  categories, search, source filter (All/Debian/Flathub), source
  badge, app icons, app detail pane, install + remove
- **History** — transaction log (apt history.log + flatpak history)
- **Settings** — Flathub remote toggle, sources dialog: editable
  deb822/.list entries, remove, add; pending changes, single
  pkexec on apply
- **i18n** — de/en via gettext; English source strings, German
  catalogue in `po/de.po` (English = msgid fallback)

Plus double-click install of local `.deb`/`.flatpakref` files
(MIME handler, `InstallFiles` / `flatpak install --from`).

Backend: PackageKit over the system D-Bus (zbus) — transactions,
polkit (`auth_admin_keep`, one password per session) and progress
are provided by packagekitd. Flatpak runs through the flatpak CLI
in user scope (no password).

## Build

```bash
cargo fmt -- --check && cargo clippy && cargo build --release
cp target/release/ragnar-software pkg/ragnar-software/usr/bin/
dpkg-deb --build pkg/ragnar-software ragnar-software_0.1.0_amd64.deb
```

## Runtime dependencies

`packagekit`, `flatpak`, `libadwaita-1-0`, `libgtk-4-1`
(AppStream data arrives via `apt update` / `flatpak update --appstream`).

See TODO.md for the roadmap.
