//! Child-process runner. One place decides how external tools are
//! spawned: fixed PATH, arg vectors only, child exit code propagated.

use std::process::Command;

/// Verb result: `Ok(child exit code)` or `Err` for rejected input.
pub type VerbResult = Result<u8, ()>;

/// Standard PATH — pkexec strips the environment, so the helper
/// fixes one itself rather than trusting whatever survives.
const SAFE_PATH: &str = "/usr/sbin:/usr/bin:/sbin:/bin";

/// Spawn `prog` with `args`, wait, return its exit code. Spawn
/// failures surface as an error message + code 1 — the caller sees
/// a non-zero status exactly like a failed tool invocation.
pub fn run(prog: &str, args: &[&str]) -> VerbResult {
    match Command::new(prog)
        .args(args)
        .env("PATH", SAFE_PATH)
        .status()
    {
        Ok(s) => Ok(s.code().unwrap_or(1).clamp(0, 255) as u8),
        Err(e) => {
            eprintln!("ragnar-heimdall-helper: {prog} failed to start: {e}");
            Err(())
        }
    }
}

/// Same but discards output and never fails — the shell helper's
/// `|| true` semantics for best-effort refresh calls.
pub fn run_quiet(prog: &str, args: &[&str]) {
    let _ = Command::new(prog)
        .args(args)
        .env("PATH", SAFE_PATH)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Reject with a message — single funnel for validation failures.
pub fn reject(msg: &str) -> VerbResult {
    eprintln!("ragnar-heimdall-helper: {msg}");
    Err(())
}
