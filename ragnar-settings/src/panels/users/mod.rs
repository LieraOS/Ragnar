// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-settings — "User accounts" panel
// Copyright (C) 2026 LieraOS Project
//
// Cinnamon-style own-account management backed by the freedesktop
// AccountsService (org.freedesktop.Accounts) — the same mechanism
// GNOME/Cinnamon settings use. Automatic login is a display-manager
// concern: AccountsService's flag does not drive lightdm, so the
// toggle writes a lightdm.conf.d snippet through pkexec + the
// running ragnar-polkit-dialog agent — the change is real, not a
// stored-but-dead preference.

mod groups;

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4::{Box, Orientation};
use std::process::Command;

/// lightdm drop-in enabling/disabling autologin.
const AUTOLOGIN_CONF: &str = "/etc/lightdm/lightdm.conf.d/50-ragnar-autologin.conf";

/// Build the users panel.
pub fn build() -> gtk4::Widget {
    let page = Box::new(Orientation::Vertical, 18);
    for m in ["top", "bottom", "start", "end"] {
        page.set_property(&format!("margin-{m}"), 24);
    }

    let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
    let uid = Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<u32>()
                .ok()
        })
        .unwrap_or(1000);
    let obj = format!("/org/freedesktop/Accounts/User{uid}");

    // -- User card ---------------------------------------------------------
    let card = Box::new(Orientation::Horizontal, 16);
    // Clickable avatar — picking a file stores it through
    // AccountsService SetIconFile (48px, was a fixed 64px picture).
    let avatar = gtk4::Button::new();
    avatar.set_has_frame(false);
    avatar.set_tooltip_text(Some(&gettext("Change picture…")));
    let avatar_holder = gtk4::Box::new(Orientation::Vertical, 0);
    avatar.set_child(Some(&avatar_holder));
    let render_avatar = |holder: &gtk4::Box, obj: &str| {
        while let Some(c) = holder.first_child() {
            holder.remove(&c);
        }
        let w: gtk4::Widget = match accounts_prop(obj, "IconFile") {
            Some(p) if std::path::Path::new(&p).exists() => {
                let pic = gtk4::Picture::for_filename(&p);
                pic.set_size_request(48, 48);
                pic.set_content_fit(gtk4::ContentFit::Cover);
                pic.upcast()
            }
            _ => {
                let img = gtk4::Image::from_icon_name("avatar-default-symbolic");
                img.set_pixel_size(48);
                img.upcast()
            }
        };
        holder.append(&w);
    };
    render_avatar(&avatar_holder, &obj);
    {
        let (ah, o) = (avatar_holder.clone(), obj.clone());
        avatar.connect_clicked(move |b| {
            let Some(win) = b.root().and_downcast::<gtk4::Window>() else {
                return;
            };
            let (ah, o) = (ah.clone(), o.clone());
            let dialog = gtk4::FileDialog::new();
            dialog.set_title(&gettext("Choose account picture"));
            let f = gtk4::FileFilter::new();
            f.set_name(Some("Images"));
            for m in ["image/png", "image/jpeg"] {
                f.add_mime_type(m);
            }
            let fs = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
            fs.append(&f);
            dialog.set_filters(Some(&fs));
            dialog.open(Some(&win), None::<&gtk4::gio::Cancellable>, move |res| {
                let Ok(file) = res else { return };
                let Some(p) = file.path() else { return };
                if accounts_call(&o, "SetIconFile", &[&p.to_string_lossy()]) {
                    render_avatar(&ah, &o);
                }
            });
        });
    }
    card.append(&avatar);

    let names = Box::new(Orientation::Vertical, 4);
    names.set_valign(gtk4::Align::Center);
    let real = accounts_prop(&obj, "RealName").filter(|r| !r.is_empty());
    let real_l = gtk4::Label::new(Some(&real.clone().unwrap_or_else(|| user.clone())));
    real_l.add_css_class("title-2");
    real_l.set_xalign(0.0);
    names.append(&real_l);
    let uname = gtk4::Label::new(Some(&format!("@{user}")));
    uname.add_css_class("dim-label");
    uname.set_xalign(0.0);
    names.append(&uname);
    let role = if is_admin() {
        gettext("Administrator")
    } else {
        gettext("Standard user")
    };
    let role_l = gtk4::Label::new(Some(&role));
    role_l.set_xalign(0.0);
    role_l.add_css_class("dim-label");
    names.append(&role_l);
    card.append(&names);
    page.append(&card);

    page.append(&gtk4::Separator::new(Orientation::Horizontal));

    // -- Full name (SetRealName via AccountsService) -----------------------
    let nrow = Box::new(Orientation::Horizontal, 10);
    nrow.append(&gtk4::Label::new(Some(&gettext("Full name"))));
    let name_entry = gtk4::Entry::builder()
        .text(real.unwrap_or_default())
        .hexpand(true)
        .build();
    nrow.append(&name_entry);
    let save = gtk4::Button::with_label(&gettext("Save"));
    {
        let (o, ne, rl) = (obj.clone(), name_entry.clone(), real_l.clone());
        save.connect_clicked(move |_| {
            let name = ne.text().trim().to_string();
            if accounts_call(&o, "SetRealName", &[&name]) {
                rl.set_label(&name);
            }
        });
    }
    nrow.append(&save);
    page.append(&nrow);

    // -- Automatic login ----------------------------------------------------
    let arow = Box::new(Orientation::Horizontal, 10);
    arow.append(&gtk4::Label::new(Some(&gettext("Automatic login"))));
    let sw = gtk4::CheckButton::new();
    sw.set_active(autologin_enabled(&user));
    sw.set_halign(gtk4::Align::End);
    sw.set_hexpand(true);
    let u = user.clone();
    sw.connect_toggled(move |b| set_autologin(&u, b.is_active()));
    arow.append(&sw);
    page.append(&arow);
    let ahint = gtk4::Label::new(Some(&gettext(
        "LightDM logs this user in without asking for a password. \
         Requires admin authentication to change.",
    )));
    ahint.add_css_class("dim-label");
    ahint.set_xalign(0.0);
    ahint.set_wrap(true);
    page.append(&ahint);

    page.append(&gtk4::Separator::new(Orientation::Horizontal));

    // -- Password -----------------------------------------------------------
    let prow = Box::new(Orientation::Horizontal, 10);
    prow.append(&gtk4::Label::new(Some(&gettext("Password"))));
    let pw_btn = gtk4::Button::with_label(&gettext("Change password…"));
    pw_btn.set_halign(gtk4::Align::End);
    pw_btn.set_hexpand(true);
    let o = obj.clone();
    pw_btn.connect_clicked(move |b| password_dialog(b, &o));
    prow.append(&pw_btn);
    page.append(&prow);

    // -- Groups: chips + edit dialog ---------------------------------------
    // Group membership editing is admin territory — advanced mode
    // only (viewing chips without the editor would be dead weight
    // for a normal user, so the whole section hides).
    if crate::advanced::enabled() {
        let gh = Box::new(Orientation::Horizontal, 10);
        gh.append(&gtk4::Label::new(Some(&gettext("Groups"))));
        let edit = gtk4::Button::with_label(&gettext("Edit groups…"));
        edit.set_halign(gtk4::Align::End);
        edit.set_hexpand(true);
        gh.append(&edit);
        page.append(&gh);

        let chip_flow = gtk4::FlowBox::new();
        chip_flow.set_selection_mode(gtk4::SelectionMode::None);
        chip_flow.set_max_children_per_line(6);
        groups::chips(&chip_flow, &groups::memberships(&user));
        page.append(&chip_flow);

        {
            let (u, cf) = (user.clone(), chip_flow.clone());
            edit.connect_clicked(move |b| groups::dialog(b, &u, &cf));
        }
    }

    page.upcast()
}

/// Sudo/adm membership decides the displayed account type.
fn is_admin() -> bool {
    Command::new("id")
        .arg("-nG")
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .any(|g| g == "sudo" || g == "adm" || g == "wheel")
        })
        .unwrap_or(false)
}

/// Whether a lightdm.conf.d snippet enables autologin for this user.
fn autologin_enabled(user: &str) -> bool {
    Command::new("sh")
        .args([
            "-c",
            &format!(
                "grep -l '^autologin-user={user}' /etc/lightdm/lightdm.conf.d/*.conf \
                  /etc/lightdm/lightdm.conf 2>/dev/null"
            ),
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Write/remove the autologin snippet via pkexec — the running
/// ragnar-polkit-dialog collects the admin authentication.
fn set_autologin(user: &str, enable: bool) {
    let script = if enable {
        format!(
            "mkdir -p /etc/lightdm/lightdm.conf.d && \
                 printf '[Seat:*]\\nautologin-user={user}\\n' > {AUTOLOGIN_CONF}"
        )
    } else {
        format!("rm -f {AUTOLOGIN_CONF}")
    };
    let _ = Command::new("pkexec")
        .args(["/bin/sh", "-c", &script])
        .status();
}

/// Password dialog: new password + confirmation; the hash is
/// generated locally and stored through AccountsService SetPassword.
fn password_dialog(anchor: &gtk4::Button, obj: &str) {
    let win = gtk4::Window::builder()
        .title(gettext("Change password"))
        .modal(true)
        .default_width(360)
        .build();
    if let Some(t) = anchor.root().and_downcast::<gtk4::Window>() {
        win.set_transient_for(Some(&t));
    }
    let body = Box::new(Orientation::Vertical, 12);
    for m in ["top", "bottom", "start", "end"] {
        body.set_property(&format!("margin-{m}"), 16);
    }
    let p1 = gtk4::PasswordEntry::builder().show_peek_icon(true).build();
    p1.set_property("placeholder-text", gettext("New password"));
    let p2 = gtk4::PasswordEntry::builder().show_peek_icon(true).build();
    p2.set_property("placeholder-text", gettext("Repeat password"));
    let status = gtk4::Label::new(None);
    status.add_css_class("error");
    status.set_xalign(0.0);
    body.append(&p1);
    body.append(&p2);
    body.append(&status);
    let btns = Box::new(Orientation::Horizontal, 8);
    btns.set_halign(gtk4::Align::End);
    let cancel = gtk4::Button::with_label(&gettext("Cancel"));
    let save = gtk4::Button::with_label(&gettext("Change"));
    save.add_css_class("suggested-action");
    btns.append(&cancel);
    btns.append(&save);
    body.append(&btns);
    cancel.connect_clicked({
        let w = win.clone();
        move |_| w.close()
    });
    let (w, st, o) = (win.clone(), status.clone(), obj.to_string());
    save.connect_clicked(move |_| {
        let (a, b) = (p1.text().to_string(), p2.text().to_string());
        if a.len() < 4 {
            st.set_label(&gettext("Password too short"));
            return;
        }
        if a != b {
            st.set_label(&gettext("Passwords do not match"));
            return;
        }
        // AccountsService stores a crypt(3) hash, not plaintext.
        let Ok(out) = Command::new("openssl").args(["passwd", "-6", &a]).output() else {
            return;
        };
        let hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if hash.is_empty() || !accounts_call(&o, "SetPassword", &[&hash, ""]) {
            st.set_label(&gettext("Failed to set password"));
            return;
        }
        w.close();
    });
    win.set_child(Some(&body));
    win.present();
}

/// Read a string property from the AccountsService User object.
fn accounts_prop(obj: &str, prop: &str) -> Option<String> {
    let out = Command::new("gdbus")
        .args([
            "call",
            "--system",
            "--dest",
            "org.freedesktop.Accounts",
            "--object-path",
            obj,
            "--method",
            "org.freedesktop.DBus.Properties.Get",
            "org.freedesktop.Accounts.User",
            prop,
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    // gvariant text form: (<'value'>,) — pull the quoted content.
    let s = String::from_utf8_lossy(&out.stdout).to_string();
    let start = s.find('\'')? + 1;
    let end = s.rfind('\'')?;
    (end > start).then(|| s[start..end].to_string())
}

/// Call a Set* method on the User object; returns success.
fn accounts_call(obj: &str, method: &str, args: &[&str]) -> bool {
    let mut c = Command::new("gdbus");
    c.args([
        "call",
        "--system",
        "--dest",
        "org.freedesktop.Accounts",
        "--object-path",
        obj,
        "--method",
        &format!("org.freedesktop.Accounts.User.{method}"),
    ]);
    for a in args {
        c.arg(a);
    }
    c.output().map(|o| o.status.success()).unwrap_or(false)
}
