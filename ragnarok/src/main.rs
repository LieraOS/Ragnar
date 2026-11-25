// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// ragnarok — every error the system recorded, in one report.
//
// Easter egg with real teeth: it merges journald error records with
// the classic /var/log and per-user Xorg logs that journald never
// sees, groups them by source and sorts by error count.

mod args;
mod entry;
mod journal;
mod logfiles;
mod report;

use std::io::IsTerminal;

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let args = match args::parse(&argv) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(if msg.starts_with("ragnarok") { 0 } else { 2 });
        }
    };

    // Merge both logging worlds: journald plus plain log files.
    let mut entries = journal::collect(args.boot);
    entries.extend(logfiles::collect());

    let colour = std::io::stdout().is_terminal() && args.output.is_none();
    let text = report::render(&entries, args.filter.as_deref(), colour);
    print!("{text}");

    if let Some(path) = &args.output {
        // The file copy is plain text — no ANSI escapes on disk.
        let plain = report::render(&entries, args.filter.as_deref(), false);
        if let Err(e) = std::fs::write(path, plain) {
            eprintln!("ragnarok: cannot write {path}: {e}");
            std::process::exit(1);
        }
        eprintln!("report written to {path}");
    }
}
