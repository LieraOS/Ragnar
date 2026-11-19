// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-settings — "Updates" panel: terminal argv convention
// Copyright (C) 2026 LieraOS Project
//
// "Install updates" opens the configured terminal running a script.
// Only one convention matters on Ragnar: the shipped terminals
// (gnome-terminal, kgx) reject `-e` entirely ("deprecated and will
// be removed") and take the command after `--`. Every other
// terminal gets the most portable form, `-e sh -c <script>` — no
// per-emulator special cases are maintained.

/// Argv (excluding the terminal binary itself) that makes `term`
/// run `script` inside a shell.
pub fn argv(term: &str, script: &str) -> Vec<String> {
    let flag = match term {
        "gnome-terminal" | "kgx" => "--",
        _ => "-e",
    };
    vec![flag.into(), "sh".into(), "-c".into(), script.to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped terminal family uses `--`; everything else `-e`.
    #[test]
    fn argv_conventions() {
        assert_eq!(
            argv("gnome-terminal", "echo hi"),
            vec!["--", "sh", "-c", "echo hi"]
        );
        assert_eq!(argv("kgx", "echo hi"), vec!["--", "sh", "-c", "echo hi"]);
        assert_eq!(argv("foot", "echo hi"), vec!["-e", "sh", "-c", "echo hi"]);
    }
}
