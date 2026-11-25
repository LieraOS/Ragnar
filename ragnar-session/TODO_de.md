# TODO — ragnar-session

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
- [x] `compositor-ready` + `ragnar-session-quit`/`inhibit` nach Rust portiert
- [ ] `cargo test`-Suite (Registry-, Parser-, Inhibit-Tests)
- [ ] `EndSessionResponse`-Responses tatsächlich auswerten (derzeit nur geloggt)
- [ ] Alt-Pakete purgen (ragnar-desktop-view, libragnar-*, gir1.2-*, dbgsym)
