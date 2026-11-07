// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Updates" panel: per-terminal argv conventions
// Copyright (C) 2026 LieraOS Project
//
// "Install updates" must open the user's configured terminal running
// a script — and every emulator parses its command line differently.
// The conventions below mirror ragnar_desktop_view's
// SUPPORTED_TERMINALS handling (file_item): gnome-terminal/kgx
// reject -e entirely ("-e is deprecated and will be removed"), while
// a single `-e "<string>"` breaks alacritty/kitty. Unknown terminals
// get the most portable form, `-e sh -c <script>`.

/// Argv (excluding the terminal binary itself) that makes `term`
/// run `script` inside a shell.
pub fn argv(term: &str, script: &str) -> Vec<String> {
    let sh = |v: &[&str]| {
        let mut a: Vec<String> = v.iter().map(|s| s.to_string()).collect();
        a.extend(["sh".into(), "-c".into(), script.to_string()]);
        a
    };
    match term {
        // GNOME Terminal / Console: no -e, everything after `--` is
        // the command to run.
        "gnome-terminal" | "kgx" => sh(&["--"]),
        // MATE, Xfce and Terminator: -x = "execute the remainder of
        // the command line", arguments stay unjoined.
        "mate-terminal" | "xfce4-terminal" | "terminator" => sh(&["-x"]),
        // Konsole and Alacritty take argv-style -e.
        "konsole" | "alacritty" => sh(&["-e"]),
        // Kitty treats trailing arguments as the command outright.
        "kitty" => sh(&[]),
        // Tilix -e takes ONE string that it re-parses — shell-quote
        // the script so the inner quoting survives the round trip.
        "tilix" => vec![
            "-e".into(),
            format!(
                "sh -c {}",
                gtk4::glib::shell_quote(script).to_string_lossy()
            ),
        ],
        // Unknown emulators get the most common convention.
        _ => sh(&["-e"]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: &str = "echo hi";

    /// Exact argv per supported terminal (mirrors file_item).
    #[test]
    fn argv_per_terminal() {
        let expect = |t: &str, head: &[&str]| {
            let mut e: Vec<String> = head.iter().map(|s| s.to_string()).collect();
            e.extend(["sh".into(), "-c".into(), S.to_string()].iter().cloned());
            assert_eq!(argv(t, S), e, "terminal {t}");
        };
        expect("gnome-terminal", &["--"]);
        expect("kgx", &["--"]);
        expect("mate-terminal", &["-x"]);
        expect("xfce4-terminal", &["-x"]);
        expect("terminator", &["-x"]);
        expect("konsole", &["-e"]);
        expect("alacritty", &["-e"]);
        expect("kitty", &[]);
        expect("unknown-terminal", &["-e"]); // fallback
    }

    /// Tilix gets a single re-parsed string with the script quoted.
    #[test]
    fn argv_tilix_quotes_script() {
        let a = argv("tilix", "apt-get update && apt-get dist-upgrade");
        assert_eq!(a.len(), 2);
        assert_eq!(a[0], "-e");
        assert!(a[1].starts_with("sh -c "));
        // Script embedded as ONE shell word, not spread over argv.
        assert!(a[1].contains("apt-get"));
        assert!(a[1].contains("'") || a[1].contains("\\"));
    }
}
