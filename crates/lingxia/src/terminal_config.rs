//! Terminal configuration as the runtime exposes it.
//!
//! Loading, watching, the command line and the environment a spawned session
//! needs all live in the shared configuration crate, so every platform gets
//! the same behaviour from the same code. This module adds only what needs
//! the runtime: where this app keeps its data.

use std::path::{Path, PathBuf};

pub use lingxia_terminal_config::cli::{run_if_invoked, session_environment};
pub use lingxia_terminal_config::runtime::{
    apply_theme, current_json, generation, load, set_installed_fonts, watched_directory,
};

/// Where this app keeps its data, as the configuration layer wants it.
///
/// Derived from the initialized runtime rather than rebuilt from each
/// platform's conventions: a host that guesses writes a file the app never
/// reads, and the two paths only differ when it matters.
pub fn app_data_dir() -> Option<PathBuf> {
    crate::app::state_dir()
        .ok()
        .and_then(|dir| dir.parent().map(Path::to_path_buf))
}
