# TODO — ragnar-polkit-agent

**Deutsch**

- [x] AuthenticationAgent-Objekt auf dem System-Bus exportieren
- [x] RegisterAuthenticationAgent (unix-session, Fallback unix-user)
- [x] GTK4/libadwaita-Passwortdialog
- [x] polkit-agent-helper-1-Protokoll (gio::Subprocess, async)
- [x] Re-Prompt bei falschem Passwort (Helper-Respawn)
- [x] CancelAuthentication-Handling
- [x] Autostart-Eintrag (X-GNOME-Autostart-Phase=Desktop)
- [ ] Identitäts-Auswahl, wenn polkit mehrere UnixUser liefert
      (derzeit: erster Eintrag)
- [ ] Fingerabdruck/PAM-Module mit eigenen Prompts testen
- [ ] Alten ragnar-polkit-dialog aus ragnar-core-Paket entfernen
- [ ] Verifizieren, dass nur eine Agent-Instanz läuft
      (GApplication-Einzelinstanz ist gesetzt)

**English**

- [x] Export AuthenticationAgent object on the system bus
- [x] RegisterAuthenticationAgent (unix-session, unix-user fallback)
- [x] GTK4/libadwaita password dialog
- [x] polkit-agent-helper-1 protocol (gio::Subprocess, async)
- [x] Re-prompt on wrong password (helper respawn)
- [x] CancelAuthentication handling
- [x] Autostart entry (X-GNOME-Autostart-Phase=Desktop)
- [ ] Identity picker when polkit passes several UnixUser
      identities (currently: first entry)
- [ ] Test fingerprint/other PAM modules with custom prompts
- [ ] Remove legacy ragnar-polkit-dialog from ragnar-core package
- [ ] Verify only one agent instance runs (single-instance app)
