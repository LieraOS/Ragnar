// Workspace applet controls on the bar settings page: how many
// workspaces exist (written to the session compositor's config and
// live-reloaded — labwc does not advertise ext-workspace's
// create_workspace) and how many this bar's buttons show.
use super::bar_page::commit;
use crate::config::BarConfig;
use gtk4::prelude::*;

/// The control row: "Arbeitsflächen" count spinner (compositor
/// config) plus "Anzeigen" spinner (per-bar display cap, 0 = all).
pub fn row(index: usize, cfg: &BarConfig) -> gtk4::Widget {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);

    row.append(&gtk4::Label::new(Some(&crate::i18n::tr("Arbeitsflächen"))));
    let count = gtk4::SpinButton::with_range(1.0, 10.0, 1.0);
    count.set_value(crate::workspace::desktops::count().unwrap_or(1) as f64);
    // Greyed out when no writable compositor config was found —
    // honest state, never a fake control.
    count.set_sensitive(crate::workspace::desktops::available());
    count.set_tooltip_text(Some(
        "Gesamtzahl — schreibt die Compositor-Konfiguration und lädt sie neu",
    ));
    count.connect_value_changed(|s| {
        crate::workspace::desktops::set_count(s.value() as u32);
    });
    row.append(&count);

    row.append(&gtk4::Label::new(Some(&crate::i18n::tr("Anzeigen"))));
    let shown = gtk4::SpinButton::with_range(0.0, 10.0, 1.0);
    shown.set_value(cfg.workspace_max as f64);
    shown.set_tooltip_text(Some(&crate::i18n::tr("0 = alle anzeigen")));
    shown.connect_value_changed(move |s| {
        commit(index, |c| c.workspace_max = s.value() as i32);
    });
    row.append(&shown);
    row.upcast()
}
