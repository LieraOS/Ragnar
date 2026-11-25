# ragnar-session

Rust replacement for the gnome-session fork (`ragnar-session-binary`) that
managed the Ragnar session. Implements the `org.gnome.SessionManager`
D-Bus API as verified on the live system:

*Deutsche Version: [README_de.md](README_de.md)*

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

## Shipped tools

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
