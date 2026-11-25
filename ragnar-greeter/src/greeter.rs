// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-greeter — authentication wiring
//
// Bridges the liblightdm signal callbacks (which arrive on the
// GTK main thread via the daemon's gmainloop integration) to the
// login panel. Flow: authenticate(user) → prompts arrive one by
// one → each respond() answers the current prompt →
// authentication-complete decides between start_session_sync and
// an error + retry.

use std::ffi::{c_char, c_int, c_void, CStr};
use std::rc::Rc;

use gtk4::prelude::*;

use crate::ffi;
use crate::i18n::tr;
use crate::login::LoginPanel;

/// UI state shared with the FFI callbacks. Leaked as a stable
/// pointer — the process lives as long as the greeter session.
pub struct Auth {
    panel: Rc<LoginPanel>,
    greeter: Option<ffi::Greeter>,
    busy: std::cell::Cell<bool>,
}

impl Auth {
    /// Wire daemon signals (when connected) and the login button.
    pub fn new(panel: Rc<LoginPanel>, greeter: Option<ffi::Greeter>) -> &'static Self {
        // &'static Self is Copy — the signal userdata and every
        // GTK closure can hold the same reference.
        let auth: &'static Self = Box::leak(Box::new(Self {
            panel: panel.clone(),
            greeter,
            busy: std::cell::Cell::new(false),
        }));
        let data = auth as *const Self as *mut c_void;

        if let Some(g) = &auth.greeter {
            g.on_prompt(on_prompt, data);
            g.on_message(on_message, data);
            g.on_complete(on_complete, data);
        }

        auth.panel.button.connect_clicked(move |_| auth.sign_in());
        auth.panel.password.connect_activate(move |_| auth.sign_in());
        auth
    }

    /// Honour the daemon's timed autologin: start its auth flow so
    /// authentication-complete arrives on its own and the session
    /// begins unless the user signs in first.
    pub fn maybe_autologin(&'static self) {
        let Some(g) = &self.greeter else { return };
        if g.autologin_timeout() <= 0 {
            return;
        }
        let user = g.autologin_user();
        self.panel.set_message(
            &format!("{} — {}", tr("Automatische Anmeldung"), user),
            false,
        );
        g.authenticate_autologin();
    }

    /// Login click / Enter in the password field.
    fn sign_in(&'static self) {
        if self.busy.get() {
            return;
        }
        match &self.greeter {
            Some(g) => {
                self.busy.set(true);
                self.panel.set_busy(true);
                self.panel.set_message(&tr("Anmeldung läuft…"), false);
                g.authenticate(&self.panel.username());
            }
            None => {
                // Preview mode — no daemon, just prove the UI works.
                self.panel.set_message(
                    &tr("Vorschau: Anmeldung würde an LightDM gehen"),
                    false,
                );
            }
        }
    }

    /// Every answered prompt; secrets come from the entry, the
    /// open question gets the chosen username.
    fn answer_prompt(&self, _text: &str, kind: c_int) {
        let Some(g) = &self.greeter else { return };
        if kind == ffi::PROMPT_SECRET {
            let pw = self.panel.password.text();
            g.respond(&pw);
            self.panel.password.set_text("");
        } else if kind == ffi::PROMPT_QUESTION {
            g.respond(&self.panel.username());
        }
    }

    /// Daemon message (info / error lines go under the button).
    fn show_message(&self, text: &str, kind: c_int) {
        self.panel
            .set_message(text, kind == ffi::MESSAGE_ERROR);
    }

    /// The last prompt was answered — daemon decided.
    fn completed(&'static self) {
        let Some(g) = &self.greeter else { return };
        if g.authenticated() && g.start_session(&self.panel.session_key()) {
            return; // session takes over, the greeter exits
        }
        self.busy.set(false);
        self.panel.set_busy(false);
        self.panel
            .set_message(&tr("Anmeldung fehlgeschlagen"), true);
        g.cancel();
    }
}

// ---- extern "C" signal entry points ------------------------------------
// The callbacks arrive on the main thread (same gmainloop GTK
// owns), so touching widgets directly is safe.

extern "C" fn on_prompt(_g: *mut c_void, text: *const c_char, kind: c_int, data: *mut c_void) {
    let auth = unsafe { &*(data as *const Auth) };
    let text = unsafe {
        if text.is_null() { String::new() } else { CStr::from_ptr(text).to_string_lossy().to_string() }
    };
    auth.answer_prompt(&text, kind);
}

extern "C" fn on_message(_g: *mut c_void, text: *const c_char, kind: c_int, data: *mut c_void) {
    let auth = unsafe { &*(data as *const Auth) };
    let text = unsafe {
        if text.is_null() { String::new() } else { CStr::from_ptr(text).to_string_lossy().to_string() }
    };
    auth.show_message(&text, kind);
}

extern "C" fn on_complete(_g: *mut c_void, data: *mut c_void) {
    let auth = unsafe { &*(data as *const Auth) };
    auth.completed();
}
