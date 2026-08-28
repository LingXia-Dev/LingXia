//! Per-user CLI state root: `~/.lingxia`, or `$LINGXIA_HOME` when set.
//!
//! `LINGXIA_HOME` replaces the whole state tree (credentials, caches, config)
//! so CI and per-client direnv setups can isolate everything at once.

use anyhow::{Context, Result};
use std::path::PathBuf;

const LINGXIA_HOME_ENV: &str = "LINGXIA_HOME";

/// The `LINGXIA_HOME` override, if set to a non-empty value.
pub fn lingxia_home_override() -> Option<PathBuf> {
    let value = std::env::var_os(LINGXIA_HOME_ENV)?;
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

/// Root of all persistent per-user CLI state.
pub fn lingxia_dir() -> Result<PathBuf> {
    if let Some(root) = lingxia_home_override() {
        return Ok(root);
    }
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".lingxia"))
}
