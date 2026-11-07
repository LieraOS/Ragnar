// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-polkit-agent — AuthenticationAgent on the system bus
//
// A polkit authentication agent exports one object implementing
// org.freedesktop.PolicyKit1.AuthenticationAgent on the SYSTEM bus
// (polkitd calls back on the registering connection's unique name)
// and announces it via Authority.RegisterAuthenticationAgent with
// subject ('unix-session', {'session-id': <XDG_SESSION_ID>}).
//
// BeginAuthentication keeps its D-Bus call open while the user
// types — answered with () on success or the Cancelled error when
// the dialog is dismissed. Each request spawns a
// polkit-agent-helper-1 session (helper.rs); FAILURE respawns the
// helper so a mistyped password re-prompts inside the same call.

use crate::dialog::{AuthDialog, Request};
use crate::{helper, i18n::tr};
use gtk4::prelude::*;
use gtk4::{gio, glib};
use libadwaita as adw;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Object path our AuthenticationAgent is exported at.
const OBJECT_PATH: &str = "/org/lieraos/ragnar/PolkitAgent";

/// Introspection XML for the agent interface (polkit spec).
const IFACE_XML: &str = r#"
<node>
  <interface name="org.freedesktop.PolicyKit1.AuthenticationAgent">
    <method name="BeginAuthentication">
      <arg type="s"     name="action_id"  direction="in"/>
      <arg type="s"     name="message"    direction="in"/>
      <arg type="s"     name="icon_name"  direction="in"/>
      <arg type="a{ss}" name="details"    direction="in"/>
      <arg type="s"     name="cookie"     direction="in"/>
      <arg type="a(sa{sv})" name="identities" direction="in"/>
    </method>
    <method name="CancelAuthentication">
      <arg type="s" name="cookie" direction="in"/>
    </method>
  </interface>
</node>"#;

/// One in-flight authentication.
struct Active {
    dialog: Rc<AuthDialog>,
    session: Option<helper::Session>,
    action: String,
}

/// Shared agent state (GTK thread only).
struct Agent {
    app: adw::Application,
    sessions: HashMap<String, Rc<RefCell<Active>>>,
}

thread_local! {
    /// Whether the object was already registered with polkitd.
    static REGISTERED: RefCell<bool> = const { RefCell::new(false) };
    /// cookie → pending BeginAuthentication invocation (answered
    /// exactly once, then removed).
    static PENDING: RefCell<HashMap<String, gio::DBusMethodInvocation>> =
        RefCell::new(HashMap::new());
    /// action_id → when the user last cancelled it. polkitd resends
    /// BeginAuthentication for every queued operation — answering a
    /// resend instantly avoids a fresh dialog per retry.
    static LAST_CANCEL: RefCell<Option<(String, std::time::Instant)>> =
        const { RefCell::new(None) };
}

/// Export the object, register it, and keep a retry timer alive so
/// an early-session start before polkitd is up still succeeds.
pub fn register(app: adw::Application) {
    eprintln!("[polkit-agent] registering AuthenticationAgent");
    let agent = Rc::new(RefCell::new(Agent {
        app,
        sessions: HashMap::new(),
    }));
    try_register(agent.clone());
    glib::timeout_add_seconds_local(5, move || {
        if !REGISTERED.with(|r| *r.borrow()) {
            try_register(agent.clone());
        }
        glib::ControlFlow::Continue
    });
}

/// One registration attempt: connect, export, announce.
fn try_register(agent: Rc<RefCell<Agent>>) {
    let Ok(conn) = gio::bus_get_sync(gio::BusType::System, None::<&gio::Cancellable>) else {
        eprintln!("[polkit-agent] cannot connect to system bus");
        return;
    };
    // polkitd restart watchdog: when the authority name reappears
    // on the bus our registration is gone — drop the flag so the
    // retry timer announces the agent again.
    conn.signal_subscribe(
        None,
        Some("org.freedesktop.DBus"),
        Some("NameOwnerChanged"),
        Some("/org/freedesktop/DBus"),
        Some("org.freedesktop.PolicyKit1"),
        gio::DBusSignalFlags::NONE,
        move |_c, _s, _p, _i, _sig, params| {
            let third = params.child_value(2);
            let new_owner = third.str().unwrap_or_default();
            if !new_owner.is_empty() {
                eprintln!("[polkit-agent] polkitd (re)appeared — will re-register");
                REGISTERED.with(|r| *r.borrow_mut() = false);
            }
        },
    );

    let Ok(node) = gio::DBusNodeInfo::for_xml(IFACE_XML) else {
        eprintln!("[polkit-agent] interface XML failed to parse");
        return;
    };
    let Some(iface) = node.interfaces().iter().next() else {
        eprintln!("[polkit-agent] no interfaces in XML");
        return;
    };

    let a = agent.clone();
    // A registration error is fine — it just means the object path
    // is already exported (retry path). The announce below still
    // has to run in that case.
    let registration = conn
        .register_object(OBJECT_PATH, iface)
        .method_call(
            move |_conn: gio::DBusConnection,
                  _sender: Option<&str>,
                  _path: &str,
                  _iface: Option<&str>,
                  method: &str,
                  params: glib::Variant,
                  invocation: gio::DBusMethodInvocation| {
                eprintln!("[polkit-agent] method call: {method}");
                match method {
                    "BeginAuthentication" => begin(&a, &params, invocation),
                    "CancelAuthentication" => cancel(&a, &params, invocation),
                    _ => {
                        invocation
                            .return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method);
                    }
                }
            },
        )
        .build();
    match &registration {
        Ok(_id) => eprintln!("[polkit-agent] object exported"),
        Err(e) => eprintln!("[polkit-agent] OBJECT EXPORT FAILED: {e}"),
    }
    let Some(subject) = session_subject() else {
        eprintln!("[polkit-agent] no session id — not registering yet");
        return;
    };
    let locale = std::env::var("LANG").unwrap_or_default();
    let params = (subject.clone(), locale, OBJECT_PATH).to_variant();
    let ok = conn
        .call_sync(
            Some("org.freedesktop.PolicyKit1"),
            "/org/freedesktop/PolicyKit1/Authority",
            "org.freedesktop.PolicyKit1.Authority",
            "RegisterAuthenticationAgent",
            Some(&params),
            None,
            gio::DBusCallFlags::NONE,
            5000,
            None::<&gio::Cancellable>,
        )
        .is_ok();
    if ok {
        eprintln!("[polkit-agent] registered for subject: {subject:?}");
        REGISTERED.with(|r| *r.borrow_mut() = true);
    } else {
        eprintln!("[polkit-agent] registration failed for: {subject:?}");
    }
}

/// Our own logind session id. Read from /proc/self/sessionid —
/// XDG_SESSION_ID is only set by some display managers (LightDM
/// does not set it). Returns None for processes outside a session
/// (e.g. a pts shell in the 'manager' scope): a session agent
/// without a session simply retries instead of registering a
/// bogus subject.
/// The graphical logind session to authenticate for. Running under
/// the systemd user manager puts us in the 'manager' scope, where
/// /proc/self/sessionid returns the WRONG session id (the manager
/// session, not the seat's user session) — polkitd then never routes
/// pkexec prompts to us. Resolve the real 'user'-class session via
/// logind's ListSessions instead.
fn session_id() -> Option<String> {
    let conn = gio::bus_get_sync(gio::BusType::System, None::<&gio::Cancellable>).ok()?;
    let res = conn
        .call_sync(
            Some("org.freedesktop.login1"),
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
            "ListSessions",
            None,
            None,
            gio::DBusCallFlags::NONE,
            5000,
            None::<&gio::Cancellable>,
        )
        .ok()?;
    let arr = res.child_value(0);
    for i in 0..arr.n_children() {
        let e = arr.child_value(i);
        let sid_v = e.child_value(0);
        let Some(sid) = sid_v.str() else {
            continue;
        };
        let seat_v = e.child_value(3);
        let seat = seat_v.str().unwrap_or_default();
        let path_v = e.child_value(4);
        let Some(path) = path_v.str() else {
            continue;
        };
        // 'user' class on a real seat — that is the graphical session.
        let class = conn
            .call_sync(
                Some("org.freedesktop.login1"),
                path,
                "org.freedesktop.DBus.Properties",
                "Get",
                Some(&("org.freedesktop.login1.Session", "Class").to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                5000,
                None::<&gio::Cancellable>,
            )
            .ok()
            .and_then(|v| v.child_value(0).child_value(0).str().map(|s| s.to_string()))
            .unwrap_or_default();
        if class == "user" && !seat.is_empty() {
            return Some(sid.to_string());
        }
    }
    None
}

/// ('unix-session', {'session-id': <id>}) — None when there is no
/// logind session for this process.
fn session_subject() -> Option<(String, HashMap<String, glib::Variant>)> {
    let mut details = HashMap::new();
    details.insert("session-id".to_string(), session_id()?.to_variant());
    Some(("unix-session".to_string(), details))
}

/// polkitd asks us to authenticate — build the request, show the
/// dialog, spawn the first helper session.
fn begin(agent: &Rc<RefCell<Agent>>, params: &glib::Variant, inv: gio::DBusMethodInvocation) {
    eprintln!("[polkit-agent] BeginAuthentication: {params:?}");
    let v = |i: usize| params.child_value(i);
    let mut message = v(1).str().unwrap_or_default().to_string();
    let icon = v(2).str().unwrap_or_default().to_string();
    let cookie = v(4).str().unwrap_or_default().to_string();
    let action = v(0).str().unwrap_or_default().to_string();

    // A resend for an action the user just cancelled is refused
    // instantly instead of stacking another dialog on screen.
    if LAST_CANCEL
        .with(|c| matches!(&*c.borrow(), Some((a, t)) if a == &action && t.elapsed().as_secs() < 3))
    {
        inv.return_dbus_error(
            "org.freedesktop.PolicyKit1.Error.Cancelled",
            "The dialog was dismissed",
        );
        return;
    }

    // a{ss} details → keep every line (usually just "action-id").
    let mut details = Vec::new();
    let d = v(3);
    for i in 0..d.n_children() {
        let e = d.child_value(i);
        if let (Some(k), Some(val)) = (e.child_value(0).str(), e.child_value(1).str()) {
            details.push((k.to_string(), val.to_string()));
        }
    }

    // pkexec passes the whole `env VAR=… /path` cmdline — strip the
    // env noise and keep just the program for a readable dialog.
    if action == "org.freedesktop.policykit.exec" {
        // The full `env VAR=… /path` line sits inside polkit's own
        // message — rebuild a short message from the program alone.
        let prog = clean_program(&message);
        message = format!(
            "{} '{prog}'",
            crate::i18n::tr("Administrator rights required for")
        );
        for (k, val) in details.iter_mut() {
            if k == "command" || k == "command-line" {
                *val = prog.clone();
            }
        }
    }

    // identities ao → first UnixUser path → uid → login name.
    let Some(user) = first_identity(&v(5)) else {
        inv.return_dbus_error(
            "org.freedesktop.PolicyKit1.Error.Failed",
            "no usable identity",
        );
        return;
    };

    let submit = {
        let agent2 = agent.clone();
        let cookie2 = cookie.clone();
        move |response: String| respond(&agent2, &cookie2, &response)
    };
    let cancel_cb = {
        let agent3 = agent.clone();
        let cookie3 = cookie.clone();
        move || finish_cancel(&agent3, &cookie3)
    };

    let dialog = AuthDialog::new(
        &agent.borrow().app,
        &Request {
            message,
            icon,
            details,
            user: user.clone(),
        },
        submit,
        cancel_cb,
    );

    agent.borrow_mut().sessions.insert(
        cookie.clone(),
        Rc::new(RefCell::new(Active {
            dialog: dialog.clone(),
            session: None,
            action,
        })),
    );
    PENDING.with(|p| p.borrow_mut().insert(cookie.clone(), inv));

    dialog.present();
    start_helper(agent, &cookie, &user);
}

/// Spawn (or respawn after FAILURE) the helper for a session.
fn start_helper(agent: &Rc<RefCell<Agent>>, cookie: &str, user: &str) {
    let agent2 = agent.clone();
    let cookie2 = cookie.to_string();
    let user2 = user.to_string();
    let on_event = move |ev: helper::Event| {
        let Some(a) = agent2.borrow().sessions.get(&cookie2).cloned() else {
            return;
        };
        let dlg = a.borrow().dialog.clone();
        match ev {
            helper::Event::Prompt(text, echo_off) => dlg.prompt(&text, echo_off),
            helper::Event::Info(text) => dlg.info(&text),
            helper::Event::Success => finish_ok(&agent2, &cookie2),
            helper::Event::Failure => {
                dlg.info(&tr("Authentifizierung fehlgeschlagen"));
                // Respawn inside the same BeginAuthentication call —
                // polkitd only ever sees success or cancel.
                let agent3 = agent2.clone();
                let c = cookie2.clone();
                let u = user2.clone();
                glib::idle_add_local_once(move || start_helper(&agent3, &c, &u));
            }
        }
    };
    if let Some(sess) = helper::spawn(user, cookie, on_event) {
        if let Some(a) = agent.borrow().sessions.get(cookie) {
            a.borrow_mut().session = Some(sess);
        }
    }
}

/// Dialog submitted a response — forward to the helper's stdin.
fn respond(agent: &Rc<RefCell<Agent>>, cookie: &str, response: &str) {
    if let Some(a) = agent.borrow().sessions.get(cookie) {
        if let Some(s) = a.borrow_mut().session.as_mut() {
            s.respond(response);
        }
    }
}

/// Helper reported SUCCESS — answer the still-open D-Bus call and
/// tear the session down.
fn finish_ok(agent: &Rc<RefCell<Agent>>, cookie: &str) {
    if let Some(inv) = PENDING.with(|p| p.borrow_mut().remove(cookie)) {
        inv.return_value(None);
    }
    // Bind the removal result first: an `if let` scrutinee keeps the
    // borrow_mut alive through the whole block, and `dialog.close()`
    // re-enters this code via close-request → double-borrow panic.
    let entry = agent.borrow_mut().sessions.remove(cookie);
    if let Some(a) = entry {
        let dialog = a.borrow().dialog.clone();
        dialog.close();
    }
}

/// User cancelled — kill the helper, answer with the Cancelled
/// error, drop the session.
fn finish_cancel(agent: &Rc<RefCell<Agent>>, cookie: &str) {
    let inv = PENDING.with(|p| p.borrow_mut().remove(cookie));
    let entry = agent.borrow_mut().sessions.remove(cookie);
    if let Some(a) = entry {
        LAST_CANCEL.with(|c| {
            *c.borrow_mut() = Some((a.borrow().action.clone(), std::time::Instant::now()));
        });
        if let Some(mut s) = a.borrow_mut().session.take() {
            s.abort();
        }
        let dialog = a.borrow().dialog.clone();
        dialog.close();
    }
    if let Some(inv) = inv {
        inv.return_dbus_error(
            "org.freedesktop.PolicyKit1.Error.Cancelled",
            "The dialog was dismissed",
        );
    }
}

/// polkitd tells us an ongoing auth is dead (caller went away) —
/// tear the dialog down without touching the answered call.
fn cancel(agent: &Rc<RefCell<Agent>>, params: &glib::Variant, inv: gio::DBusMethodInvocation) {
    let cookie = params.child_value(0).str().unwrap_or_default().to_string();
    let entry = agent.borrow_mut().sessions.remove(&cookie);
    if let Some(a) = entry {
        if let Some(mut s) = a.borrow_mut().session.take() {
            s.abort();
        }
        let dialog = a.borrow().dialog.clone();
        dialog.close();
    }
    PENDING.with(|p| p.borrow_mut().remove(&cookie));
    inv.return_value(None);
}

/// First `unix-user` identity → login name via /etc/passwd.
/// polkitd serializes identities as a(sa{sv}) — subject tuples
/// like ('unix-user', {'uid': <1001>}), NOT object paths.
fn first_identity(identities: &glib::Variant) -> Option<String> {
    for i in 0..identities.n_children() {
        let entry = identities.child_value(i);
        let kind_v = entry.child_value(0);
        let kind = kind_v.str().unwrap_or_default();
        if kind != "unix-user" {
            continue;
        }
        let details = entry.child_value(1);
        for j in 0..details.n_children() {
            let kv = details.child_value(j);
            if kv.child_value(0).str() == Some("uid") {
                let val = kv.child_value(1);
                let uid = val.get::<glib::Variant>().and_then(|v| v.get::<u32>());
                if let Some(uid) = uid {
                    if let Some(name) = user_name(uid) {
                        return Some(name);
                    }
                }
            }
        }
    }
    // Fallback: the current user (standard single-user session).
    std::env::var("USER").ok().filter(|u| !u.is_empty())
}

/// Resolve a uid to a login name by scanning /etc/passwd.
fn user_name(uid: u32) -> Option<String> {
    std::fs::read_to_string("/etc/passwd")
        .ok()?
        .lines()
        .find_map(|l| {
            let mut f = l.split(':');
            let name = f.next()?;
            (f.nth(1)?.parse::<u32>().ok()? == uid).then(|| name.to_string())
        })
}

/// Extract the real program out of a pkexec-style command line —
/// drops the `env` binary and every `KEY=VALUE` argument.
fn clean_program(cmd: &str) -> String {
    let mut it = cmd.split_whitespace().peekable();
    while let Some(tok) = it.peek() {
        if tok.contains('=') || *tok == "env" || tok.ends_with("/env") {
            it.next();
        } else {
            break;
        }
    }
    it.next()
        .and_then(|p| p.rsplit('/').next())
        .unwrap_or(cmd)
        .to_string()
}
