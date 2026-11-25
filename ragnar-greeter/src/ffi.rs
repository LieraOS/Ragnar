// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-greeter — liblightdm-gobject-1 FFI bindings
//
// Hand-declared extern surface — the #[link] attribute resolves
// the versioned soname directly, so no liblightdm-gobject-dev
// symlink is needed at build time (same approach ragnar-sap uses
// for libsane.so.1). Only the greeter API subset this program
// uses: daemon connect, the authenticate/respond/start-session
// flow, user and session lists, hints and the power actions.

use std::ffi::{c_char, c_int, c_void, CStr, CString};

#[repr(C)]
pub struct GList {
    pub data: *mut c_void,
    pub next: *mut GList,
    pub prev: *mut GList,
}

/// LightDM prompt types (lightdm-gobject-1/lightdm/greeter.h).
pub const PROMPT_QUESTION: c_int = 0;
pub const PROMPT_SECRET: c_int = 1;
/// Message types.
pub const MESSAGE_ERROR: c_int = 1;

#[link(name = ":liblightdm-gobject-1.so.0")]
extern "C" {
    fn lightdm_greeter_new(error: *mut *mut c_void) -> *mut c_void;
    fn lightdm_greeter_connect_to_daemon_sync(g: *mut c_void, error: *mut *mut c_void) -> c_int;
    fn lightdm_greeter_authenticate(g: *mut c_void, username: *const c_char) -> c_int;
    fn lightdm_greeter_authenticate_autologin(g: *mut c_void) -> c_int;
    fn lightdm_greeter_respond(g: *mut c_void, response: *const c_char);
    fn lightdm_greeter_cancel_authentication(g: *mut c_void);
    fn lightdm_greeter_get_is_authenticated(g: *mut c_void) -> c_int;
    fn lightdm_greeter_get_default_session_hint(g: *mut c_void) -> *const c_char;
    fn lightdm_greeter_get_hide_users_hint(g: *mut c_void) -> c_int;
    fn lightdm_greeter_get_autologin_timeout_hint(g: *mut c_void) -> c_int;
    fn lightdm_greeter_get_autologin_user_hint(g: *mut c_void) -> *const c_char;
    fn lightdm_greeter_start_session_sync(g: *mut c_void, session: *const c_char, error: *mut *mut c_void) -> c_int;

    fn lightdm_user_list_get_instance() -> *mut c_void;
    fn lightdm_user_list_get_users(list: *mut c_void) -> *mut GList;
    fn lightdm_user_get_name(user: *mut c_void) -> *const c_char;
    fn lightdm_user_get_display_name(user: *mut c_void) -> *const c_char;

    fn lightdm_get_sessions() -> *mut GList;
    fn lightdm_session_get_key(session: *mut c_void) -> *const c_char;
    fn lightdm_session_get_name(session: *mut c_void) -> *const c_char;

    fn lightdm_get_can_suspend() -> c_int;
    fn lightdm_get_can_hibernate() -> c_int;
    fn lightdm_get_can_restart() -> c_int;
    fn lightdm_get_can_shutdown() -> c_int;
    fn lightdm_suspend(error: *mut *mut c_void) -> c_int;
    fn lightdm_hibernate(error: *mut *mut c_void) -> c_int;
    fn lightdm_restart(error: *mut *mut c_void) -> c_int;
    fn lightdm_shutdown(error: *mut *mut c_void) -> c_int;

    fn g_signal_connect_data(instance: *mut c_void, signal: *const c_char,
                             handler: *mut c_void, data: *mut c_void,
                             destroy: *mut c_void, flags: c_int) -> u64;
    fn g_object_unref(object: *mut c_void);
}

/// Opaque greeter handle — owns the GObject, unref'd on drop.
pub struct Greeter(*mut c_void);
impl Drop for Greeter {
    fn drop(&mut self) {
        unsafe { g_object_unref(self.0) }
    }
}

/// Connect a Rust-side handler to a greeter signal. The callback
/// is a plain extern "C" fn receiving (greeter, ..., user_data).
fn connect(obj: *mut c_void, signal: &str, handler: *mut c_void, data: *mut c_void) {
    let name = CString::new(signal).unwrap();
    unsafe {
        g_signal_connect_data(obj, name.as_ptr(), handler, data, std::ptr::null_mut(), 0);
    }
}

/// Signal callback signatures.
pub type PromptCb = extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void);
pub type MsgCb = extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void);
pub type SimpleCb = extern "C" fn(*mut c_void, *mut c_void);

impl Greeter {
    /// Create the greeter object and connect to the daemon.
    /// Returns Err(()) when lightdm is not running (preview mode).
    pub fn connect_daemon() -> Result<Self, ()> {
        unsafe {
            let g = lightdm_greeter_new(std::ptr::null_mut());
            if g.is_null() {
                return Err(());
            }
            if lightdm_greeter_connect_to_daemon_sync(g, std::ptr::null_mut()) == 0 {
                g_object_unref(g);
                return Err(());
            }
            Ok(Self(g))
        }
    }

    /// Wire the daemon signals to plain function pointers.
    /// `data` is passed back unchanged to every callback.
    pub fn on_prompt(&self, cb: PromptCb, data: *mut c_void) {
        connect(self.0, "show-prompt", cb as *mut c_void, data);
    }
    pub fn on_message(&self, cb: MsgCb, data: *mut c_void) {
        connect(self.0, "show-message", cb as *mut c_void, data);
    }
    pub fn on_complete(&self, cb: SimpleCb, data: *mut c_void) {
        connect(self.0, "authentication-complete", cb as *mut c_void, data);
    }

    /// Start authentication for `username` (empty → daemon prompts
    /// for it itself, which also covers hide-users greeters).
    pub fn authenticate(&self, username: &str) {
        if username.is_empty() {
            unsafe { lightdm_greeter_authenticate(self.0, std::ptr::null()) };
            return;
        }
        if let Ok(name) = CString::new(username) {
            unsafe { lightdm_greeter_authenticate(self.0, name.as_ptr()) };
        }
    }

    /// Restart the autologin flow (guest/user hint driven).
    pub fn authenticate_autologin(&self) {
        unsafe { lightdm_greeter_authenticate_autologin(self.0) };
    }

    /// Answer the current prompt.
    pub fn respond(&self, response: &str) {
        if let Ok(r) = CString::new(response) {
            unsafe { lightdm_greeter_respond(self.0, r.as_ptr()) };
        }
    }

    pub fn cancel(&self) {
        unsafe { lightdm_greeter_cancel_authentication(self.0) };
    }

    /// True once authentication-complete left us authenticated.
    pub fn authenticated(&self) -> bool {
        unsafe { lightdm_greeter_get_is_authenticated(self.0) != 0 }
    }

    /// Hand the authenticated user to the session. `session` is a
    /// LightDMSession key (empty → daemon's own default).
    pub fn start_session(&self, session: &str) -> bool {
        let key = CString::new(session).unwrap_or_default();
        unsafe { lightdm_greeter_start_session_sync(self.0, key.as_ptr(), std::ptr::null_mut()) != 0 }
    }

    // ---- hints ---------------------------------------------------
    pub fn default_session(&self) -> String {
        unsafe { cstr(lightdm_greeter_get_default_session_hint(self.0)) }
    }
    pub fn hide_users(&self) -> bool {
        unsafe { lightdm_greeter_get_hide_users_hint(self.0) != 0 }
    }
    pub fn autologin_timeout(&self) -> i32 {
        unsafe { lightdm_greeter_get_autologin_timeout_hint(self.0) as i32 }
    }
    pub fn autologin_user(&self) -> String {
        unsafe { cstr(lightdm_greeter_get_autologin_user_hint(self.0)) }
    }
}

/// (login name, display name) pairs from the user list.
pub fn users() -> Vec<(String, String)> {
    let mut out = Vec::new();
    unsafe {
        let list = lightdm_user_list_get_instance();
        let mut node = lightdm_user_list_get_users(list);
        while !node.is_null() {
            let u = (*node).data;
            out.push((cstr(lightdm_user_get_name(u)),
                      cstr(lightdm_user_get_display_name(u))));
            node = (*node).next;
        }
    }
    out
}

/// (session key, display name) pairs for the session dropdown.
pub fn sessions() -> Vec<(String, String)> {
    let mut out = Vec::new();
    unsafe {
        let mut node = lightdm_get_sessions();
        while !node.is_null() {
            let s = (*node).data;
            out.push((cstr(lightdm_session_get_key(s)),
                      cstr(lightdm_session_get_name(s))));
            node = (*node).next;
        }
    }
    out
}

/// Power actions the daemon advertises plus their triggers.
pub fn can_power() -> (bool, bool, bool, bool) {
    unsafe {
        (lightdm_get_can_suspend() != 0,
         lightdm_get_can_hibernate() != 0,
         lightdm_get_can_restart() != 0,
         lightdm_get_can_shutdown() != 0)
    }
}
pub fn power(action: &str) {
    let mut err = std::ptr::null_mut();
    unsafe {
        match action {
            "suspend" => lightdm_suspend(&mut err),
            "hibernate" => lightdm_hibernate(&mut err),
            "restart" => lightdm_restart(&mut err),
            _ => lightdm_shutdown(&mut err),
        };
    }
}

/// Borrowed C string → owned Rust String (empty on NULL).
unsafe fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        CStr::from_ptr(p).to_string_lossy().to_string()
    }
}
