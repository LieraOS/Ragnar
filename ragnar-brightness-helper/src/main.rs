// SPDX-License-Identifier: GPL-3.0-or-later
// ragnar-brightness-helper — backlight control via logind
//
// Rust replacement for the legacy ragnar-brightness-helper. Called by
// the compositor keybinds (XF86MonBrightness*):
//   --up / --down [--step PERCENT]   relative change (default 5 %)
//   --set PERCENT                    absolute value
// Uses org.freedesktop.login1.Session.SetBrightness on the caller's
// own session object — unprivileged, no /sys writes, no polkit.

mod backlight;
mod logind;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let step = parse_opt(&args, "--step").unwrap_or(5);
    let set = parse_opt(&args, "--set");
    let up = args.iter().any(|a| a == "--up");
    let down = args.iter().any(|a| a == "--down");

    if !(1..=100).contains(&step) {
        eprintln!("ragnar-brightness-helper: --step must be between 1 and 100");
        return ExitCode::from(2);
    }
    if up == down && set.is_none() {
        eprintln!("usage: ragnar-brightness-helper --up|--down [--step N] | --set PCT");
        return ExitCode::from(2);
    }

    let Some(dev) = backlight::find() else {
        // No backlight hardware (desktop monitors, VMs) — same
        // behaviour as the legacy helper: report and exit nonzero.
        eprintln!("ragnar-brightness-helper: no backlight device in /sys/class/backlight");
        return ExitCode::from(1);
    };

    let max = backlight::max(&dev);
    if max == 0 {
        eprintln!("ragnar-brightness-helper: {} reports max_brightness 0", dev);
        return ExitCode::from(1);
    }
    let cur = backlight::current(&dev);

    let new = match set {
        Some(pct) if !(0..=100).contains(&pct) => {
            eprintln!("ragnar-brightness-helper: --set must be 0-100");
            return ExitCode::from(2);
        }
        Some(pct) => pct as u64 * max / 100,
        None => {
            let delta = step as u64 * max / 100;
            if up {
                (cur + delta).min(max)
            } else {
                cur.saturating_sub(delta)
            }
        }
    };

    match logind::set_brightness(&dev, new as u32) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ragnar-brightness-helper: SetBrightness failed: {e}");
            ExitCode::from(1)
        }
    }
}

/// Extract the integer value of `--opt N` from the arg list.
fn parse_opt(args: &[String], opt: &str) -> Option<i64> {
    args.iter()
        .position(|a| a == opt)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
}
