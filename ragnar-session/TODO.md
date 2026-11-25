# TODO — ragnar-session

*Deutsche Version: [TODO_de.md](TODO_de.md)*

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
- [x] `compositor-ready` + `ragnar-session-quit`/`inhibit` ported to Rust
- [ ] `cargo test` suite (registry, parser, inhibitor tests)
- [ ] Actually consume `EndSessionResponse` replies (currently only logged)
- [ ] Purge legacy packages (ragnar-desktop-view, libragnar-*, gir1.2-*, dbgsym)
