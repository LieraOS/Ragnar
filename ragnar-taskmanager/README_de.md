# Ragnar Taskverwaltung

**Copyright (C) 2025 Jörg Schröder / LieraOS**
**Lizenz:** GPL-3.0-or-later — **Website:** https://www.lieraos.com

Native Taskverwaltung für das Ragnar-Desktop, in Rust mit
GTK4/libadwaita geschrieben. Nachfolger des Qt6-basierten
*Linux Task Manager* (`lieratask`), vollständig Wayland-nativ neu
implementiert — kein X11, keine Portale, kein Hilfsdaemon. Alle
Daten kommen direkt aus procfs, sysfs und der systemd-D-Bus-API.

## Funktionen

- **Prozesse** — Live-Liste sortiert nach CPU-Last, Suche nach
  Name/PID/Benutzer, Detaildialog (Status, Threads, Speicher,
  Startzeit, Befehlszeile), Beenden (SIGTERM), Erzwingen (SIGKILL),
  nice-Priorität.
- **Leistung** — CPU-Balken pro Kern, 60-s-Verlaufsgraph, RAM-/
  Swap-Anzeigen, Netzwerk- und Festplattendurchsatz pro Gerät,
  GPU-Last und -Temperatur (AMD via sysfs, NVIDIA via nvidia-smi).
- **Dienste** — systemd-Units über `org.freedesktop.systemd1`
  D-Bus: Auflisten, Suchen, Start/Stop/Neustart. Die polkit-
  Autorisierung übernimmt systemd, die UI blockiert nie.
- **Autostart** — XDG-Autostart-Einträge mit Ein/Aus-Schaltern;
  ein Schalter schreibt ein benutzerlokales `Hidden=`-Override,
  Systemdateien bleiben unberührt.
- **System** — Rechnername, Betriebssystem, Kernel, CPU-Modell,
  Kerne, Speicher, Laufzeit und Lastdurchschnitt.

## Globaler Hotkey

`Strg+Umschalt+Esc` wird vom **Compositor** gebunden, nicht von
einem Daemon — die Ragnar-labwc-Bridge liefert einen
`C-S-Escape`-Keybind in der `rc.xml`, der `ragnar-taskmanager`
ausführt. Andere Compositors (Odin usw.) registrieren dieselbe
Bindung in ihrer eigenen Konfiguration; die Anwendung bleibt
compositor-unabhängig.

## Bauen & Installieren

```bash
cargo build --release
msgfmt po/de.po -o /usr/share/locale/de/LC_MESSAGES/ragnar-taskmanager.mo
```

Debian-Paket:

```bash
dpkg-deb --root-owner-group --build pkg/ragnar-taskmanager \
    ragnar-taskmanager_0.1.0-1_amd64.deb
```

## Lokalisierung

Der Code ist englisch; die Oberfläche folgt der Systemsprache
über gettext. Deutsch liegt bei (`po/de.po`); Englisch ist der
unübersetzte Standard.
