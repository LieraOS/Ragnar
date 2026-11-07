# ragnar-automount TODO

## Done / Erledigt

- [x] Session automounter via `gio::VolumeMonitor` (start + hot-plug).
- [x] Dedicated GSettings schema `org.lieraos.ragnar.automount` with
      key `enabled` (shipped in `data/`, installed by the
      `ragnar-settings` package); read per event so the Settings
      toggle applies live.
      Dediziertes GSettings-Schema `org.lieraos.ragnar.automount` mit
      Key `enabled`; wird pro Ereignis gelesen, der Settings-Schalter
      greift sofort.

## Open / Offen

- [ ] Ship schema via a dedicated `ragnar-automount` package instead
      of `ragnar-settings` once the daemon gets its own packaging.
      Schema über ein eigenes `ragnar-automount`-Paket ausliefern,
      sobald der Daemon eine eigene Paketierung bekommt.
