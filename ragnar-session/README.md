# ragnar-session

## Deutsch

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

### Mitgelieferte Werkzeuge

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

## English

Rust replacement for the gnome-session fork (`ragnar-session-binary`) that
managed the Ragnar session. Implements the `org.gnome.SessionManager`
D-Bus API as verified on the live system:

- Parses `.session` files (`RequiredComponents`), launches components via
  `.desktop` Exec and supervises/respawns `X-GNOME-AutoRestart` entries
- `RegisterClient`/`UnregisterClient` with `Client`/`ClientPrivate`
  objects under `/org/gnome/SessionManager/ClientN`
  (`EndSessionResponse` handshake as used by `ragnar-power-dialog`)
- `Logout`/`Reboot`/`Shutdown` via the `EndSessionDialog` contract
  (QueryEndSession → dialog → Confirmed* → EndSession/Stop → SessionOver)
- `Inhibit`/`Uninhibit`/`IsInhibited`, `Setenv`, `GetLocale`,
  `IsSessionRunning`, `GetClients`, properties
- Autostart is handled by `systemd-xdg-autostart-generator` — no custom
  autostart phase

`RAGNAR_SESSION_FILE` overrides the `.session` path (for
`dbus-run-session` tests). Otherwise `--session=<name>` or `Desktop`.

### Shipped tools

- `/usr/bin/ragnar-session` — launcher wrapper (locale setup, execs
  `/usr/libexec/ragnar-session`)
- `ragnar-session-quit` — logout/reboot/shutdown through the
  SessionManager API (used by keybindings; `rc.xml` invokes it)
- `ragnar-session-inhibit` — run a command while session actions
  (logout, suspend, idle, switch-user) are inhibited
- `ragnar-session-compositor-ready` — waits until the Wayland socket
  accepts connections (startup gate for portals/services)
- `ragnar-session.target` (systemd user unit) and the
  `org.lieraos.ragnar.SessionManager` GSettings schema
