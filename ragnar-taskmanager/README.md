# Ragnar Task Manager

**Copyright (C) 2025 Jörg Schröder / LieraOS**
**License:** GPL-3.0-or-later — **Website:** https://www.lieraos.com

Native task manager for the Ragnar desktop, written in Rust with
GTK4/libadwaita. Successor of the Qt6-based *Linux Task Manager*
(`lieratask`), fully re-implemented Wayland-native — no X11, no
portals, no helper daemon. All data comes straight from procfs,
sysfs and the systemd D-Bus API.

## Features

- **Processes** — live list sorted by CPU usage, search by name/PID/
  user, details dialog (state, threads, memory, start time, command
  line), terminate (SIGTERM), kill (SIGKILL), nice-priority control.
- **Performance** — per-core CPU bars, 60 s history graph, RAM/swap
  meters, network and disk throughput per device, GPU load and
  temperature (AMD via sysfs, NVIDIA via nvidia-smi).
- **Services** — systemd units via `org.freedesktop.systemd1` D-Bus:
  list, search, start/stop/restart. Polkit authorisation handled by
  systemd, the UI never blocks.
- **Startup** — XDG autostart entries with on/off switches; toggling
  writes a user-local `Hidden=` override, system files stay untouched.
- **System** — hostname, OS, kernel, CPU model, cores, memory,
  uptime and load average.

## Global hotkey

`Ctrl+Shift+Esc` is bound by the **compositor**, not by a daemon —
the Ragnar labwc bridge ships a `C-S-Escape` keybind in `rc.xml`
that executes `ragnar-taskmanager`. Other compositors (Odin etc.)
register the same binding in their own configuration; the app
itself stays compositor-agnostic.

## Build & install

```bash
cargo build --release
msgfmt po/de.po -o /usr/share/locale/de/LC_MESSAGES/ragnar-taskmanager.mo
```

Debian package:

```bash
dpkg-deb --root-owner-group --build pkg/ragnar-taskmanager \
    ragnar-taskmanager_0.1.0-1_amd64.deb
```

## Localisation

Code is English; the UI follows the system language through gettext.
German is included (`po/de.po`); English is the untranslated default.
