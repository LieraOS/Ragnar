// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) Jörg Schröder
// Command line parsing for ragnarok.
//
// Usage:
//   ragnarok [FILTER] [-b N|--boot N] [--all] [-o FILE|--output FILE]
//   ragnarok -h|--help

/// Parsed command line options.
pub struct Args {
    /// Substring filter matched against unit/identifier/file names.
    pub filter: Option<String>,
    /// Journalctl -b argument (0 = current boot, -1 previous, ...).
    /// None means the current boot; `--all` clears it for all boots.
    pub boot: Option<i32>,
    /// All recorded boots instead of just one.
    pub all: bool,
    /// Optional file the report is written to (without colours).
    pub output: Option<String>,
}

const HELP: &str = "\
ragnarok — show every error the system recorded.

Spellings: ragnarok, ragnarök, ragnarøk, ragnaroek and the
common misspelling ragnarock all run the same command.

Usage:
  ragnarok [FILTER] [options]

Arguments:
  FILTER              only groups whose unit, program or file name
                      contains FILTER (case-insensitive)

Options:
  -b, --boot N        journald boot offset: 0 current, -1 previous, ...
  --all               all recorded boots instead of the current one
  -o, --output FILE   also write the report (plain text) to FILE
  -h, --help          this help

Sources: journald (priority <= err) plus classic log files under
/var/log and the user's Xorg/Wayland logs. Warnings are excluded.";

/// Parse argv into Args; returns Err(help-or-error) to print and exit.
pub fn parse(argv: &[String]) -> Result<Args, String> {
    let mut args = Args {
        filter: None,
        boot: Some(0),
        all: false,
        output: None,
    };
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "-h" | "--help" => return Err(HELP.to_string()),
            "--all" => args.all = true,
            "-b" | "--boot" => {
                i += 1;
                let v = argv.get(i).ok_or("missing value for --boot")?;
                args.boot = Some(v.parse().map_err(|_| "invalid --boot value")?);
            }
            "-o" | "--output" => {
                i += 1;
                args.output = Some(argv.get(i).ok_or("missing value for -o")?.clone());
            }
            s if s.starts_with("-b") && s.len() > 2 => {
                args.boot = Some(s[2..].parse().map_err(|_| "invalid -b value")?);
            }
            s if s.starts_with('-') => {
                return Err(format!("unknown option {s}\n\n{HELP}"));
            }
            s => {
                if args.filter.is_none() {
                    args.filter = Some(s.to_string());
                } else {
                    return Err(format!("unexpected argument {s}\n\n{HELP}"));
                }
            }
        }
        i += 1;
    }
    if args.all {
        args.boot = None;
    }
    Ok(args)
}
