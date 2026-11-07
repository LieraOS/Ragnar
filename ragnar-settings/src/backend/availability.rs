//! Runtime availability checks for settings entries.
//!
//! Each [`crate::registry::Entry`] declares what it needs (a program in
//! `PATH`, a GSettings schema, or nothing). The hub hides entries whose
//! requirements are missing, so the same binary shows a reduced set on
//! a development machine and the full set inside a Ragnar session.

/// What an entry needs to be shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Requirement {
    /// Always available.
    Always,
    /// A program must be resolvable in `PATH`.
    Program(&'static str),
    /// A GSettings schema id must exist on the system.
    Schema(&'static str),
}

/// Whether the requirement is satisfied on this machine right now.
pub fn satisfied(req: &Requirement) -> bool {
    match req {
        Requirement::Always => true,
        Requirement::Program(name) => program_in_path(name),
        Requirement::Schema(id) => schema_exists(id),
    }
}

/// Resolve `name` against every directory in `PATH`.
fn program_in_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
        .unwrap_or(false)
}

/// Ask GSettings whether a schema id is installed.
fn schema_exists(id: &str) -> bool {
    gtk4::gio::SettingsSchemaSource::default()
        .map(|src| src.lookup(id, true).is_some())
        .unwrap_or(false)
}
