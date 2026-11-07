# ragnar-osd

Rust-Dienst für das On-Screen-Display des Ragnar-Desktops.
Besitzt `org.lieraos.ragnar.OSD` am Session-Bus und zeigt bei
`Show(a{sv})` (Keys: `icon`, `label`, `level` als double 0..1 oder
int 0..100) ein GTK4-Layer-Shell-Overlay unten mittig — 2,5 s
Auto-Expire, Vertrag identisch zum ersetzten legacy Dienst.
Enthält ausserdem die eingebauten Watcher (vormals OSDKeys):
Lautstärke-Änderungen via `pactl subscribe` (PipeWire-Kompat) und
Helligkeits-Änderungen via `/sys/class/backlight` führen direkt zum
Overlay — die eigentlichen Tasten-Aktionen bleiben beim
media-keys-Plugin der Compositor-Bridge.

Rust service for the Ragnar desktop's on-screen display. Owns
`org.lieraos.ragnar.OSD` on the session bus and shows a GTK4
layer-shell overlay on `Show(a{sv})` (keys: `icon`, `label`, `level`
as double 0..1 or int 0..100) — bottom-centred, 2.5 s auto-expire,
same contract as the replaced legacy service. Also contains the
built-in watchers (formerly OSDKeys): volume changes via
`pactl subscribe` (PipeWire compat) and brightness changes via
`/sys/class/backlight` trigger the overlay directly — the actual key
actions stay with the compositor bridge's media-keys plugin.
