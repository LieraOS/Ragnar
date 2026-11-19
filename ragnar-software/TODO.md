# ragnar-software — TODO

- [x] Welcome page with background update check
- [x] Updates page: list, checkboxes, install with progress dialog
      (apt via RefreshCache+GetUpdates, Flatpak via remote-ls --updates)
- [x] Dependency preview before installation
- [x] Detail pane: version + changelog (GetUpdateDetail / apt changelog)
- [x] Software catalogue: AppStream categories, search, installed badge
- [x] Install + remove (InstallPackages/RemovePackages)
- [x] polkit auth_admin_keep — one password per session
- [x] **Flatpak**: Flathub remote (user scope), AppStream XML
      catalogue, install/remove via flatpak CLI, source badge
      (Debian/Flathub)
- [x] Source filter in the catalogue (All / Debian / Flathub)
- [x] Flatpak updates on the Updates page (remote-ls + CLI install)
- [x] "History" page (apt history.log + flatpak history)
- [x] Sources management (deb822 editor, pending-changes pkexec)
- [x] i18n de/en via gettext (English source, de.po catalogue)
- [x] Local `.deb` double-click install (InstallFiles + MIME)
- [x] `.flatpakref` / `.flatpakrepo` installation
- [x] App icons in the catalogue + detail pane (flathub PNG cache,
      DEP-11 stock names via icon theme)
- [ ] App detail page: extend the existing pane (screenshots,
      installed version, install/remove action)
- [ ] "Installed" page (all installed apps, remove)
