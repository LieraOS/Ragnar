# TODO — ragnar-session

## Deutsch
- [x] Crate-Grundgerüst (zbus 5, tokio)
- [x] `.session`-Parser + `.desktop`-Resolver
- [x] Component-Spawn + AutoRestart-Supervision
- [x] `org.gnome.SessionManager`-Iface komplett
- [x] Client/ClientPrivate-Objekte zur Laufzeit servieren
- [x] Inhibit-Registry
- [x] EndSession-Orchestrierung (EndSessionDialog-Vertrag)
- [x] Standalone-Test via `dbus-run-session` (echter power-dialog registrierte sich)
- [x] Erste Live-Session: Session kam hoch, Logout-Handshake funktionierte
- [x] `ragnar-session.target`: zieht graphical-session{,-pre} +
      xdg-desktop-autostart per Dependency (RefuseManualStart!);
      KEIN `Before=graphical-session.target` → Ordering-Cycle
- [ ] `cargo test`-Suite (Registry-, Parser-, Inhibit-Tests)
- [ ] `EndSessionResponse`-Responses tatsächlich auswerten (derzeit nur geloggt)
- [ ] `compositor-ready` + `ragnar-session-quit`/`inhibit` in Rust portieren
- [ ] Alt-Pakete purgen (ragnar-desktop-view, libragnar-*, gir1.2-*, dbgsym)

## English
- [x] Crate skeleton (zbus 5, tokio)
- [x] `.session` parser + `.desktop` resolver
- [x] Component spawn + AutoRestart supervision
- [x] Full `org.gnome.SessionManager` interface
- [x] Runtime-served Client/ClientPrivate objects
- [x] Inhibitor registry
- [x] End-session orchestration (EndSessionDialog contract)
- [x] Standalone test via `dbus-run-session` (real power-dialog registered)
- [x] First live session: session came up, logout handshake worked
- [x] `ragnar-session.target`: pulls graphical-session{,-pre} +
      xdg-desktop-autostart via dependencies (RefuseManualStart!);
      NO `Before=graphical-session.target` → ordering cycle
- [ ] `cargo test` suite (registry, parser, inhibitor tests)
- [ ] Actually consume `EndSessionResponse` replies (currently only logged)
- [ ] Port `compositor-ready` + `ragnar-session-quit`/`inhibit` to Rust
- [ ] Purge legacy packages (ragnar-desktop-view, libragnar-*, gir1.2-*, dbgsym)
