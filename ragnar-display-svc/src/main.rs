// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-display-svc — display management daemon for Ragnar/Wayland
// Copyright (C) 2026 LieraOS Project

mod wayland;
mod config;
mod dbus_service;
mod gpu;
mod gpu_clock;
mod gpu_fan;
mod gpu_nvidia;
mod gpu_power;
mod gpu_sysfs;
mod gsettings;
mod main_loop;
mod xrandr;

fn main() {
    main_loop::run();
}
