// Custom command applets — the user-defined extension point: a
// layout entry `cmd:Name|command|secs` runs `command` every `secs`
// seconds through /bin/sh and shows its stdout (first line) as
// `Name output` in the bar. Real stdout data only — no parsing
// tricks, no fake values. Created via the settings "Eigenes Applet"
// button, removable like every other applet.
use crate::bar::State;
use gtk4::gio;
use gtk4::prelude::*;

/// Parse "cmd:Name|shell command|seconds" -> (name, cmd, secs).
pub fn parse(spec: &str) -> Option<(String, String, u64)> {
    let rest = spec.strip_prefix("cmd:")?;
    let mut it = rest.split('|');
    let name = it.next()?.to_string();
    let cmd = it.next()?.to_string();
    let secs = it.next().and_then(|s| s.parse().ok()).unwrap_or(5).max(1);
    Some((name, cmd, secs))
}

/// Build a custom applet: `Name <output>` label refreshed per spec.
pub fn build(_state: &State, spec: &str) -> gtk4::Widget {
    let Some((name, cmd, secs)) = parse(spec) else {
        return gtk4::Label::new(Some("cmd?")).upcast();
    };
    let label = gtk4::Label::new(Some(&name));
    label.set_tooltip_text(Some(&cmd));
    // The user command runs via gio::Subprocess — asynchronous: a
    // blocking or slow command must never freeze the bar's main loop.
    let run = move |l: &gtk4::Label| {
        let (l2, name2, cmd2) = (l.clone(), name.clone(), cmd.clone());
        gtk4::glib::spawn_future_local(async move {
            let out = run_shell(&cmd2).await.unwrap_or_default();
            l2.set_text(&format!("{name2} {out}"));
        });
    };
    run(&label);
    let l = label.clone();
    gtk4::glib::timeout_add_seconds_local(secs as u32, move || {
        run(&l);
        gtk4::glib::ControlFlow::Continue
    });
    label.upcast()
}

/// Run the applet's shell command off the main loop; returns the
/// first stdout line. Async via gio::Subprocess.
async fn run_shell(cmd: &str) -> Option<String> {
    let p = gio::Subprocess::newv(
        &[
            std::ffi::OsStr::new("/bin/sh"),
            std::ffi::OsStr::new("-c"),
            std::ffi::OsStr::new(cmd),
        ],
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE,
    )
    .ok()?;
    let (stdout, _) = p.communicate_utf8_future(None).await.ok()?;
    Some(stdout?.trim().lines().next().unwrap_or("").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_spec() {
        let (name, cmd, secs) = parse("cmd:Uptime|uptime -p|10").unwrap();
        assert_eq!(name, "Uptime");
        assert_eq!(cmd, "uptime -p");
        assert_eq!(secs, 10);
    }

    #[test]
    fn defaults_interval_and_clamps() {
        assert_eq!(parse("cmd:X|true|").unwrap().2, 5);
        assert_eq!(parse("cmd:X|true|0").unwrap().2, 1);
    }

    #[test]
    fn rejects_malformed() {
        assert!(parse("sensor-cpu").is_none());
        assert!(parse("cmd:X").is_none());
        assert!(parse("cmd:").is_none());
    }
}
