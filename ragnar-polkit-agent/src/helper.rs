// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-polkit-agent — polkit-agent-helper-1 protocol
//
// polkit authentication agents never touch PAM themselves: they
// spawn the setuid helper `/usr/lib/polkit-1/polkit-agent-helper-1`
// and talk a simple line protocol over its pipes:
//   agent → helper: "<cookie>\n"            (first write, always)
//   helper → agent: "PAM_PROMPT_ECHO_OFF <prompt>" → agent answers
//                   on stdin with "<response>\n"
//                   "PAM_PROMPT_ECHO_ON <prompt>"  same, but echo
//                   "PAM_ERROR_MSG <text>" / "PAM_TEXT_INFO <text>"
//                   informational lines only
//                   "SUCCESS" / "FAILURE"          final result
// gio::Subprocess + async reads keep everything on the GTK main
// loop — no worker threads, no channels. A FAILURE may be retried
// by spawning a fresh session (the dialog stays open for that).

use gtk4::prelude::*;
use gtk4::{gio, glib};
use std::ffi::OsStr;
use std::rc::Rc;

/// Helper binary shipped by Debian's libpolkit-agent-1-0.
const HELPER: &str = "/usr/lib/polkit-1/polkit-agent-helper-1";

/// Events the helper reports while a conversation runs.
pub enum Event {
    /// A PAM prompt arrived; bool = echo_off (password-style).
    Prompt(String, bool),
    /// Informational/error text from PAM.
    Info(String),
    /// Authentication succeeded.
    Success,
    /// Authentication failed or the helper died unexpectedly.
    Failure,
}

/// One live helper session: stdin writer + kill handle.
pub struct Session {
    child: gio::Subprocess,
    stdin: gio::OutputStream,
}

impl Session {
    /// Send a response line to the current PAM prompt.
    pub fn respond(&mut self, response: &str) {
        let _ = self.stdin.write_all(
            format!("{response}\n").as_bytes(),
            None::<&gio::Cancellable>,
        );
    }

    /// Abort the conversation — kills the helper if still alive.
    pub fn abort(&mut self) {
        self.child.force_exit();
    }
}

/// Spawn a helper for `user` + `cookie` and start the async read
/// loop; every protocol line arrives as an `Event` on the main
/// loop via `on_event`.
pub fn spawn(user: &str, cookie: &str, on_event: impl Fn(Event) + 'static) -> Option<Session> {
    let argv = [OsStr::new(HELPER), OsStr::new(user)];
    let child = gio::Subprocess::newv(
        &argv,
        gio::SubprocessFlags::STDIN_PIPE | gio::SubprocessFlags::STDOUT_PIPE,
    )
    .ok()?;
    let stdin = child.stdin_pipe()?;
    let stdout = gio::DataInputStream::new(&child.stdout_pipe()?);

    // The cookie must be the first line on the helper's stdin.
    let _ = stdin.write_all(format!("{cookie}\n").as_bytes(), None::<&gio::Cancellable>);

    read_next(stdout, Rc::new(on_event));
    Some(Session { child, stdin })
}

/// Read one line asynchronously, dispatch it, and reschedule until
/// the helper's stdout closes (EOF = failure, no SUCCESS seen).
fn read_next(stdout: gio::DataInputStream, on_event: Rc<impl Fn(Event) + 'static>) {
    // The receiver borrow must end before the closure moves the
    // stream for rescheduling — call on a cheap clone instead.
    let s = stdout.clone();
    s.read_line_utf8_async(
        glib::Priority::DEFAULT,
        None::<&gio::Cancellable>,
        move |res| {
            let ev = match res {
                Ok(Some(line)) => parse(&line),
                // EOF or read error without SUCCESS → failure.
                _ => Some(Event::Failure),
            };
            match ev {
                Some(Event::Failure) | Some(Event::Success) => {
                    if let Some(e) = ev {
                        (on_event)(e);
                    }
                }
                Some(e) => {
                    (on_event)(e);
                    read_next(stdout.clone(), on_event.clone());
                }
                None => read_next(stdout.clone(), on_event.clone()),
            }
        },
    );
}

/// One helper stdout line → event (None = uninteresting line).
fn parse(line: &str) -> Option<Event> {
    let line = line.trim_end_matches('\n');
    if let Some(p) = line.strip_prefix("PAM_PROMPT_ECHO_OFF ") {
        Some(Event::Prompt(p.to_string(), true))
    } else if let Some(p) = line.strip_prefix("PAM_PROMPT_ECHO_ON ") {
        Some(Event::Prompt(p.to_string(), false))
    } else if let Some(t) = line.strip_prefix("PAM_ERROR_MSG ") {
        Some(Event::Info(t.to_string()))
    } else if let Some(t) = line.strip_prefix("PAM_TEXT_INFO ") {
        Some(Event::Info(t.to_string()))
    } else if line == "SUCCESS" {
        Some(Event::Success)
    } else if line == "FAILURE" {
        Some(Event::Failure)
    } else {
        None
    }
}
