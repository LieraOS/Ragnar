# ragnar-mpris

Rust-Dienst für Media-Key-Weiterleitung im Ragnar-Desktop.
Besitzt `org.lieraos.ragnar.MprisController` am Session-Bus und reicht
`PlayPause`, `Play`, `Pause`, `Next`, `Previous`, `Stop` an den zuletzt
aktiven `org.mpris.MediaPlayer2.*`-Player weiter — identischer Vertrag wie
der ersetzte legacy Dienst, daher bleiben bestehende Keybinds unverändert.
Autostart per `/etc/xdg/autostart`; kein GSettings, kein UI.

Rust service for media-key forwarding in the Ragnar desktop.
Owns `org.lieraos.ragnar.MprisController` on the session bus and forwards
`PlayPause`, `Play`, `Pause`, `Next`, `Previous`, `Stop` to the most
recently active `org.mpris.MediaPlayer2.*` player — the same contract as
the replaced legacy service, so existing keybinds keep working. Autostart
via `/etc/xdg/autostart`; no GSettings, no UI.
