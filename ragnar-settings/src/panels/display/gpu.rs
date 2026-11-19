// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings-gui — display module: GPU information section
// Copyright (C) 2026 LieraOS Project
//
// One expandable card per detected GPU with live sensor values read
// through the daemon (nvidia-smi for NVIDIA, sysfs hwmon for AMD/Intel).
// Interactive controls are only rendered for capabilities the GPU
// actually reports as writable — everything else stays informational.
// Writes go through pkexec and trigger a polkit auth dialog.

use gettextrs::gettext;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use super::dbus_client;
use ragnar_shared::gpu::{GpuInfo, GpuVendor};

/// Live-updating labels for one GPU card, refreshed by a timer.
struct GpuLabels {
    temperature: gtk4::Label,
    power: gtk4::Label,
    fan: gtk4::Label,
    clocks: gtk4::Label,
    card_index: i32,
}

/// Create the GPU section appended below the monitor controls.
/// Returns an empty box when no GPU can be detected.
pub fn create_gpu_section() -> gtk4::Box {
    let section = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    section.set_margin_top(24);

    let gpus = dbus_client::fetch_gpus();
    if gpus.is_empty() {
        return section;
    }

    let labels: Rc<RefCell<Vec<GpuLabels>>> = Rc::new(RefCell::new(Vec::new()));

    for gpu in &gpus {
        let (expander, live) = gpu_card(gpu);
        section.append(&expander);
        labels.borrow_mut().push(live);
    }

    // Refresh sensor values every 3 seconds while the page lives.
    {
        let labels = labels.clone();
        let section_weak = section.downgrade();
        gtk4::glib::timeout_add_seconds_local(3, move || {
            let Some(section) = section_weak.upgrade() else {
                return gtk4::glib::ControlFlow::Break;
            };
            // Skip work while the page is hidden; the timer stays
            // alive for when the user navigates back.
            if !section.is_mapped() {
                return gtk4::glib::ControlFlow::Continue;
            }
            // fetch_gpus blocks on D-Bus + nvidia-smi — run it off the
            // main thread and only paint the labels on the UI side.
            let labels = labels.clone();
            gtk4::glib::spawn_future_local(async move {
                let gpus = gtk4::gio::spawn_blocking(dbus_client::fetch_gpus)
                    .await
                    .unwrap_or_default();
                for live in labels.borrow().iter() {
                    if let Some(g) = gpus.iter().find(|g| g.card_index == live.card_index) {
                        live.temperature.set_text(&temp_string(g));
                        live.power.set_text(&power_string(g));
                        live.fan.set_text(&fan_string(g));
                        live.clocks.set_text(&clock_string(g));
                    }
                }
            });
            gtk4::glib::ControlFlow::Continue
        });
    }

    section
}

/// Build one expandable GPU card: title = GPU name, collapsed by default.
fn gpu_card(gpu: &GpuInfo) -> (gtk4::Expander, GpuLabels) {
    let expander = gtk4::Expander::new(Some(&gpu.name));
    expander.set_expanded(false);

    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    inner.set_margin_top(6);
    inner.set_margin_bottom(10);
    inner.set_margin_start(12);
    inner.set_margin_end(12);

    let vendor = match gpu.vendor {
        GpuVendor::Nvidia => "NVIDIA".to_string(),
        GpuVendor::Amd => "AMD".to_string(),
        GpuVendor::Intel => "Intel".to_string(),
        GpuVendor::Unknown => gettext("Unknown"),
    };
    inner.append(&info_row(&gettext("Vendor"), &vendor));
    inner.append(&info_row(&gettext("Driver"), &gpu.driver));

    let temperature = value_row(&inner, &gettext("Temperature"), &temp_string(gpu));
    let power = value_row(&inner, &gettext("Power"), &power_string(gpu));
    let fan = value_row(&inner, &gettext("Fan"), &fan_string(gpu));
    let clocks = value_row(&inner, &gettext("Clocks"), &clock_string(gpu));

    // Interactive controls: only where the GPU reports real write support.
    if gpu.supports_power_limit {
        inner.append(&power_limit_row(gpu));
    }
    if gpu.vendor == GpuVendor::Nvidia && gpu.max_graphics_clock_mhz > 0 {
        inner.append(&locked_clock_row(gpu));
    }
    if gpu.supports_fan_control {
        inner.append(&fan_row(gpu));
    }

    expander.set_child(Some(&inner));
    (
        expander,
        GpuLabels {
            temperature,
            power,
            fan,
            clocks,
            card_index: gpu.card_index,
        },
    )
}

/// Static label + value row inside a GPU card.
fn info_row(label: &str, value: &str) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::new(Some(label)));
    let val = gtk4::Label::new(Some(value));
    val.set_halign(gtk4::Align::End);
    val.set_hexpand(true);
    val.add_css_class("dim-label");
    row.append(&val);
    row
}

/// Same as info_row but returns the value label for live updates.
fn value_row(parent: &gtk4::Box, label: &str, value: &str) -> gtk4::Label {
    let row = info_row(label, value);
    // The value label is the second child appended by info_row.
    let val = row
        .last_child()
        .and_then(|w| w.downcast::<gtk4::Label>().ok())
        .unwrap();
    parent.append(&row);
    val
}

/// Slider + apply button for the GPU power limit.
fn power_limit_row(gpu: &GpuInfo) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::new(Some(&gettext("Power Limit"))));

    let scale = gtk4::Scale::with_range(
        gtk4::Orientation::Horizontal,
        gpu.power_limit_min_w,
        gpu.power_limit_max_w,
        1.0,
    );
    scale.set_value(gpu.power_limit_w);
    scale.set_hexpand(true);
    scale.set_digits(0);
    scale.set_value_pos(gtk4::PositionType::Right);
    row.append(&scale);

    let apply = gtk4::Button::with_label(&gettext("Apply"));
    let pci = gpu.pci_address.clone();
    apply.connect_clicked(move |_| {
        let watts = scale.value();
        let reply = dbus_client::send_gpu_power_limit(&pci, watts);
        tracing::info!(
            "ragnar-settings-gui: power limit {}W -> {}",
            watts,
            reply.message
        );
    });
    row.append(&apply);

    row
}

/// Slider + apply/reset for locking the NVIDIA graphics clock.
/// Sliding to 0 and applying unlocks the clock (free boost again).
fn locked_clock_row(gpu: &GpuInfo) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::new(Some(&gettext("Lock GPU Clock"))));

    let scale = gtk4::Scale::with_range(
        gtk4::Orientation::Horizontal,
        0.0,
        gpu.max_graphics_clock_mhz as f64,
        10.0,
    );
    scale.set_value(0.0);
    scale.set_hexpand(true);
    scale.set_digits(0);
    scale.set_value_pos(gtk4::PositionType::Right);
    scale.set_tooltip_text(Some(&gettext("0 = unlocked (free boost)")));
    row.append(&scale);

    let apply = gtk4::Button::with_label(&gettext("Apply"));
    let pci = gpu.pci_address.clone();
    apply.connect_clicked(move |_| {
        let mhz = scale.value() as i32;
        let reply = dbus_client::send_gpu_locked_clock(&pci, mhz);
        tracing::info!(
            "ragnar-settings-gui: lock clock {}MHz -> {}",
            mhz,
            reply.message
        );
    });
    row.append(&apply);

    row
}

/// Slider + apply button for manual fan speed (AMD pwm1).
fn fan_row(gpu: &GpuInfo) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row.append(&gtk4::Label::new(Some(&gettext("Fan Speed"))));

    let scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 100.0, 1.0);
    scale.set_value(gpu.fan_percent.max(0) as f64);
    scale.set_hexpand(true);
    scale.set_digits(0);
    scale.set_value_pos(gtk4::PositionType::Right);
    row.append(&scale);

    let apply = gtk4::Button::with_label(&gettext("Apply"));
    let pci = gpu.pci_address.clone();
    apply.connect_clicked(move |_| {
        let pct = scale.value() as i32;
        let reply = dbus_client::send_gpu_fan(&pci, pct);
        tracing::info!("ragnar-settings-gui: fan {}% -> {}", pct, reply.message);
    });
    row.append(&apply);

    row
}

fn temp_string(g: &GpuInfo) -> String {
    if g.temperature_c >= 0 {
        format!("{} °C", g.temperature_c)
    } else {
        "—".to_string()
    }
}

fn power_string(g: &GpuInfo) -> String {
    if g.power_limit_w > 0.0 {
        format!("{:.0} W / {:.0} W", g.power_draw_w, g.power_limit_w)
    } else {
        format!("{:.0} W", g.power_draw_w)
    }
}

fn fan_string(g: &GpuInfo) -> String {
    if g.fan_percent >= 0 {
        format!("{} %", g.fan_percent)
    } else {
        "—".to_string()
    }
}

fn clock_string(g: &GpuInfo) -> String {
    format!("{} / {} MHz", g.core_clock_mhz, g.mem_clock_mhz)
}
