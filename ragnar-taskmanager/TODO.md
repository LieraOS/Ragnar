# TODO — Ragnar Task Manager

## Open

- Settings page: refresh interval, default sort column, history
  length (persisted to ~/.config/ragnar-taskmanager/settings.ini)
- Process tree view (parent/child hierarchy)
- Per-process network/disk io from /proc/pid/io + netlink
- Users filter dropdown
- Intel GPU utilisation (needs i915 perf counters or fdinfo)
- Optional elevated kill/renice for other users' processes via
  polkit (currently own processes + EPERM message)
- Odin keybind registration once Odin's keybinding config lands
