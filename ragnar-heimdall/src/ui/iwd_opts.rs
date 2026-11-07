//! iwd Wi-Fi options group: MAC randomization mode and range.
//! Changes go through the pkexec helper which also restarts iwd —
//! the notice under each combo says that plainly.

use libadwaita as adw;

use adw::prelude::*;
use adw::PreferencesGroup;

use crate::backend;

/// Build the Wi-Fi options preferences group.
pub fn wifi_options() -> PreferencesGroup {
    let group = PreferencesGroup::builder()
        .title(gettextrs::gettext("Wi-Fi Options"))
        .description(gettextrs::gettext(
            "Written to /etc/iwd/main.conf; iwd restarts briefly to apply.",
        ))
        .build();

    group.add(&iwd_combo(
        gettextrs::gettext("MAC address randomization"),
        &[
            gettextrs::gettext("Off — use the hardware address"),
            gettextrs::gettext("Once — one random address per boot"),
            gettextrs::gettext("Per network — same random address per SSID"),
        ],
        "AddressRandomization",
        &["disabled", "once", "network"],
        "disabled",
    ));
    group.add(&iwd_combo(
        gettextrs::gettext("Randomization range"),
        &[
            gettextrs::gettext("Last 3 octets only"),
            gettextrs::gettext("Whole address"),
        ],
        "AddressRandomizationRange",
        &["nic", "full"],
        "nic",
    ));
    group
}

/// One ComboRow bound to a whitelisted iwd main.conf key. The
/// worker round-trips through `iwd_set_option`; a failed change
/// leaves the combo where it was — the next refresh re-reads the
/// real file anyway.
fn iwd_combo(
    title: String,
    labels: &[String],
    key: &'static str,
    values: &'static [&'static str],
    default: &str,
) -> adw::ComboRow {
    let model = gtk4::StringList::new(&[]);
    for l in labels {
        model.append(l);
    }
    let current = backend::iwd_option(key).unwrap_or_else(|| default.to_string());
    let selected = values.iter().position(|v| *v == current).unwrap_or(0) as u32;
    let combo = adw::ComboRow::builder()
        .title(&title)
        .model(&model)
        .selected(selected)
        .build();
    combo.connect_selected_notify(move |combo| {
        let Some(value) = values.get(combo.selected() as usize) else {
            return;
        };
        let value = *value;
        std::thread::spawn(move || {
            if let Err(err) = backend::iwd_set_option(key, value) {
                tracing::warn!("iwd option {key}={value} failed: {err:#}");
            }
        });
    });
    combo
}
