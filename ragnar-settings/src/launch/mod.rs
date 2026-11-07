//! Launching external LieraOS applications from hub tiles.
//!
//! Spawned detached: the settings hub stays alive and usable while the
//! launched program runs independently.

use anyhow::{Context, Result};
use tracing::info;

/// Spawn `program` detached and return immediately.
///
/// Callers are expected to hide the tile when
/// [`crate::backend::satisfied`] already reports unavailability, so
/// reaching this path with a missing binary is a race, not normal use.
/// The error is returned to the caller for logging/display.
pub fn launch_program(program: &str) -> Result<()> {
    info!("launching external settings app: {program}");
    let mut it = program.split_whitespace();
    let child = it
        .next()
        .map(|bin| std::process::Command::new(bin).args(it).spawn())
        .transpose()
        .with_context(|| format!("failed to launch {program}"))?
        .with_context(|| format!("empty launch command: {program}"))?;
    // Reap the child when it exits so it cannot linger as a zombie;
    // do not hold the settings process open for it.
    std::thread::spawn(move || {
        let _ = child.wait_with_output();
    });
    Ok(())
}
