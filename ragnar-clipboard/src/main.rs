// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnar-clipboard — Ragnar clipboard manager.
//
// Usage:
//   ragnar-clipboard daemon   watch the clipboard, keep history,
//                             re-provide contents when the source
//                             app exits (run under session autostart)
//   ragnar-clipboard pick     open the history picker window
//   ragnar-clipboard list     print the history to stdout
//   ragnar-clipboard clear    wipe history and clear the clipboard
//
// The daemon owns the clipboard source it installs, so set-entries
// survive this process and the picker exiting.

mod daemon;
mod i18n;
mod picker;
mod store;

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_else(|| "pick".into());
    let code = match arg.as_str() {
        "daemon" => daemon::run(),
        "pick" => picker::run(),
        "list" => {
            for e in store::load() {
                println!("{}\t{}", e.id, e.preview);
            }
            0
        }
        "clear" => match daemon::send("clear") {
            Ok(r) if r == "ok" => 0,
            Ok(r) => {
                eprintln!("ragnar-clipboard: {r}");
                1
            }
            Err(_) => {
                // No daemon running — wipe the store directly.
                store::wipe();
                0
            }
        },
        "--version" | "-V" => {
            println!("ragnar-clipboard {}", env!("CARGO_PKG_VERSION"));
            0
        }
        _ => {
            eprintln!("Usage: ragnar-clipboard [daemon|pick|list|clear]");
            1
        }
    };
    std::process::exit(code);
}
