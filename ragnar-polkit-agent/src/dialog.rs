// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-polkit-agent — the authentication dialog
//
// Modal adwaita window shown while a polkit authentication runs:
// requestor icon + message, optional detail/action line, identity
// row, password entry and Authenticate/Cancel buttons. The window
// itself is dumb — the session logic in dbus.rs feeds it prompts
// and reads answers through the two callbacks.

use crate::i18n::tr;
use gtk4::glib;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Live authentication dialog plus the widgets the session pokes.
pub struct AuthDialog {
    pub window: adw::ApplicationWindow,
    entry: gtk4::PasswordEntry,
    prompt_label: gtk4::Label,
    info_label: gtk4::Label,
    /// Written each time the user submits a response.
    pub on_submit: Rc<dyn Fn(String)>,
    /// Written when the user cancels (button, Esc, window close).
    pub on_cancel: Rc<dyn Fn()>,
    cancelled: Rc<RefCell<bool>>,
}

/// Everything the dialog needs to know about the request.
pub struct Request {
    /// Human-readable message from the polkit action file.
    pub message: String,
    /// Icon name (may be empty → fallback lock icon).
    pub icon: String,
    /// "action-id: org.foo.bar" style detail lines.
    pub details: Vec<(String, String)>,
    /// Display name of the identity being authenticated.
    pub user: String,
}

impl AuthDialog {
    /// Build the dialog; callbacks fire on the GTK main loop.
    pub fn new(
        app: &adw::Application,
        req: &Request,
        on_submit: impl Fn(String) + 'static,
        on_cancel: impl Fn() + 'static,
    ) -> Rc<Self> {
        let cancelled = Rc::new(RefCell::new(false));
        let on_submit = Rc::new(on_submit) as Rc<dyn Fn(String)>;
        let on_cancel = Rc::new(on_cancel) as Rc<dyn Fn()>;

        let win = adw::ApplicationWindow::builder()
            .application(app)
            .title(tr("Authentifizierung erforderlich"))
            .default_width(420)
            .resizable(false)
            .modal(true)
            .build();

        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        for m in ["top", "bottom", "start", "end"] {
            content.set_property(&format!("margin-{m}"), 20);
        }

        // Icon + message row.
        let head = gtk4::Box::new(gtk4::Orientation::Horizontal, 14);
        let icon_name = if req.icon.is_empty() {
            "dialog-password-symbolic"
        } else {
            req.icon.as_str()
        };
        let icon = gtk4::Image::from_icon_name(icon_name);
        icon.set_pixel_size(48);
        icon.set_valign(gtk4::Align::Start);
        head.append(&icon);
        let texts = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        let msg = gtk4::Label::new(Some(&req.message));
        msg.set_wrap(true);
        msg.set_xalign(0.0);
        msg.add_css_class("title-4");
        texts.append(&msg);
        for (k, v) in &req.details {
            let d = gtk4::Label::new(Some(&format!("{k}: {v}")));
            d.set_xalign(0.0);
            d.add_css_class("dim-label");
            d.add_css_class("caption");
            texts.append(&d);
        }
        let who = gtk4::Label::new(Some(&format!("{}: {}", tr("Benutzer"), req.user)));
        who.set_xalign(0.0);
        who.add_css_class("dim-label");
        texts.append(&who);
        head.append(&texts);
        content.append(&head);

        // Prompt + entry.
        let prompt_label = gtk4::Label::new(Some(&tr("Passwort:")));
        prompt_label.set_xalign(0.0);
        content.append(&prompt_label);
        let entry = gtk4::PasswordEntry::new();
        entry.set_show_peek_icon(true);
        content.append(&entry);

        // Info/error line (PAM_ERROR_MSG, failures).
        let info_label = gtk4::Label::new(None);
        info_label.set_xalign(0.0);
        info_label.add_css_class("error");
        info_label.set_wrap(true);
        info_label.set_visible(false);
        content.append(&info_label);

        // Buttons.
        let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        buttons.set_halign(gtk4::Align::End);
        let cancel = gtk4::Button::with_label(&tr("Abbrechen"));
        let ok = gtk4::Button::with_label(&tr("Authentifizieren"));
        ok.add_css_class("suggested-action");
        buttons.append(&cancel);
        buttons.append(&ok);
        content.append(&buttons);

        win.set_content(Some(&content));

        let me = Rc::new(Self {
            window: win.clone(),
            entry: entry.clone(),
            prompt_label,
            info_label,
            on_submit: on_submit.clone(),
            on_cancel: on_cancel.clone(),
            cancelled: cancelled.clone(),
        });

        // Submit on Enter or button click — each closure gets its
        // own Rc clone of the dialog handle.
        {
            let me2 = me.clone();
            entry.connect_activate(move |e| me2.submit(e.text().to_string()));
            let me2 = me.clone();
            ok.connect_clicked(move |_| me2.submit(me2.entry.text().to_string()));
        }
        // Cancel paths — button, Esc, close request all funnel into
        // the same once-only callback.
        {
            let me2 = me.clone();
            cancel.connect_clicked(move |_| me2.cancel());
            let me2 = me.clone();
            win.connect_close_request(move |_| {
                me2.cancel();
                glib::Propagation::Proceed
            });
        }

        me
    }

    /// Present the window and focus the password field.
    pub fn present(&self) {
        self.window.present();
        self.entry.grab_focus();
    }

    /// New PAM prompt arrived — update label, clear the entry.
    pub fn prompt(&self, text: &str, _echo_off: bool) {
        self.prompt_label.set_text(text);
        self.entry.set_text("");
        self.entry.grab_focus();
    }

    /// Show an informational/error line (also "wrong password").
    pub fn info(&self, text: &str) {
        self.info_label.set_text(text);
        self.info_label.set_visible(true);
    }

    /// Tear the window down after a result.
    pub fn close(&self) {
        self.window.close();
    }

    /// Deliver the user's response to the session.
    fn submit(&self, response: String) {
        if response.is_empty() {
            return;
        }
        (self.on_submit)(response);
    }

    /// Fire the cancel callback exactly once.
    fn cancel(&self) {
        let mut c = self.cancelled.borrow_mut();
        if !*c {
            *c = true;
            drop(c);
            (self.on_cancel)();
        }
    }
}
