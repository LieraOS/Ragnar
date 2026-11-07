//! Status page shown for views that are planned but not yet
//! implemented — an honest empty state rather than a dead button.

use libadwaita as adw;

use adw::StatusPage;

/// Build a translated "not available yet" page for `title`.
pub fn build(title: &str) -> StatusPage {
    StatusPage::builder()
        .icon_name("applications-engineering-symbolic")
        .title(title)
        .description(gettextrs::gettext(
            "This section is under development and will be enabled in an upcoming release.",
        ))
        .build()
}
