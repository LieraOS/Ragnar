# ragnar-nightlight

Rust-Dienst für Night Light im Ragnar-Desktop. Beobachtet
`org.gnome.settings-daemon.plugins.color` (das Standard-Schema, das
Ragnar Settings schreibt), hält `~/.config/gammastep/ragnar_config.ini`
synchron (Temperatur, geoclue2/manuell, Dämmerungszeiten) und
startet/stoppt `gammastep` entsprechend — identischer Vertrag wie der
ersetzte legacy NightLightManager, kein D-Bus.

Rust service for night light in the Ragnar desktop. Watches
`org.gnome.settings-daemon.plugins.color` (the standard schema
Ragnar Settings writes), keeps
`~/.config/gammastep/ragnar_config.ini` in sync (temperature,
geoclue2/manual, dusk/dawn times) and starts/stops `gammastep`
accordingly — same contract as the replaced legacy NightLightManager,
no D-Bus.
