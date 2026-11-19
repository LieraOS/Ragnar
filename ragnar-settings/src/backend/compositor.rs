// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — compositor detection & capability map
// Copyright (C) 2026 LieraOS Project
//
// Which compositor is running and what it can do. Deep settings
// rows declare the capability they need: controls the running
// compositor supports stay editable; everything else is shown
// locked with a "requires the Ragnar compositor" note. When the
// native compositor takes over, those rows unlock by themselves —
// the capability map is the only place that ever needs updating.

/// The compositor detected for this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compositor {
    /// Wayland compositor writing ~/.config/ragnar-desktop/labwc/rc.xml.
    Labwc,
    /// The native Ragnar compositor — full feature surface.
    Ragnar,
    /// Nothing detected (e.g. running under X11 or a foreign session).
    Unknown,
}

/// A deep compositor feature a settings row can require.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cap {
    /// Titlebar/corner/shadow styling.
    Decoration,
    /// Inner/outer window gaps and work-area margins.
    Gaps,
    /// Focus-follows-mouse behaviour, raise delays.
    Focus,
    /// Edge snapping distances and maximize-on-top behaviour.
    Snapping,
    /// Workspace count, names and popup timing.
    Workspaces,
    /// Per-application window rules.
    WindowRules,
    /// New-window placement policy.
    Placement,
    /// Per-device libinput sections.
    InputDevices,
    /// Tearing for fullscreen applications.
    Tearing,
    // --- requires the native Ragnar compositor -------------------
    /// Backdrop blur behind windows.
    Blur,
    /// Configurable window/workspace animations.
    Animations,
    /// Tiling layout engines (master/stack, splits).
    Layouts,
    /// Touchpad workspace gestures.
    Gestures,
    /// Grouped/tabbed windows.
    WindowGroups,
    /// Scratchpad overlay workspace.
    Scratchpad,
    /// Mirror one output onto another.
    Mirror,
}

/// Detect the running compositor. Order: an odin process wins over
/// a labwc process; the rc.xml on disk counts as labwc evidence so
/// file-backed editing still works on dev machines inside a Ragnar
/// session where detection is only about capabilities, not paths.
pub fn detect() -> Compositor {
    if proc_running("ragnar-odin") || proc_running("odin") {
        return Compositor::Ragnar;
    }
    if proc_running("labwc") || crate::panels::rcxml::rc_path().exists() {
        return Compositor::Labwc;
    }
    Compositor::Unknown
}

/// Whether the running compositor supports `cap`.
pub fn supports(cap: Cap) -> bool {
    match detect() {
        // The native compositor exposes the full surface — every
        // capability including the labwc-incompatible depth.
        Compositor::Ragnar => true,
        Compositor::Unknown => false,
        Compositor::Labwc => !matches!(
            cap,
            Cap::Blur
                | Cap::Animations
                | Cap::Layouts
                | Cap::Gestures
                | Cap::WindowGroups
                | Cap::Scratchpad
                | Cap::Mirror
        ),
    }
}

/// Scan /proc/*/comm for an exact process name.
fn proc_running(name: &str) -> bool {
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return false;
    };
    rd.flatten().any(|e| {
        e.file_name()
            .to_str()
            .map(|n| n.bytes().all(|b| b.is_ascii_digit()))
            .unwrap_or(false)
            && std::fs::read_to_string(e.path().join("comm"))
                .map(|s| s.trim() == name)
                .unwrap_or(false)
    })
}
