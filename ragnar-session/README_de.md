# ragnar-session

Rust-Ersatz für den gnome-session-Fork (`ragnar-session-binary`), der bislang
die Ragnar-Session verwaltete. Implementiert die auf dem produktiven System
verifizierte `org.gnome.SessionManager`-D-Bus-API:

- `.session`-Dateien parsen (`RequiredComponents`), Komponenten per
  `.desktop`-Exec starten und bei `X-GNOME-AutoRestart` überwachen/respawnen
- `RegisterClient`/`UnregisterClient` mit `Client`- und
  `ClientPrivate`-Objekten unter `/org/gnome/SessionManager/ClientN`
  (`EndSessionResponse`-Handshake, wie ihn `ragnar-power-dialog` nutzt)
- `Logout`/`Reboot`/`Shutdown` über den `EndSessionDialog`-Vertrag
  (QueryEndSession → Dialog → Confirmed* → EndSession/Stop → SessionOver)
- `Inhibit`/`Uninhibit`/`IsInhibited`, `Setenv`, `GetLocale`,
  `IsSessionRunning`, `GetClients`, Properties
- Autostart liegt bei `systemd-xdg-autostart-generator` — kein eigener
  Autostart-Code

Umgebungsvariable `RAGNAR_SESSION_FILE` überschreibt den `.session`-Pfad
(für `dbus-run-session`-Tests). Sonst `--session=<name>` bzw. `Desktop`.

## Mitgelieferte Werkzeuge

- `/usr/bin/ragnar-session` — Launcher-Wrapper (Locale-Setup, exec auf
  `/usr/libexec/ragnar-session`)
- `ragnar-session-quit` — Logout/Reboot/Shutdown über die SessionManager-API
  (für Keybindings; `rc.xml` ruft es auf)
- `ragnar-session-inhibit` — Befehl ausführen, während Session-Aktionen
  (logout, suspend, idle, switch-user) gehemmt sind
- `ragnar-session-compositor-ready` — wartet, bis der Wayland-Socket
  Verbindungen annimmt (Startup-Gate für Portale/Dienste)
- `ragnar-session.target` (systemd User-Unit) und das
  `org.lieraos.ragnar.SessionManager`-GSettings-Schema
