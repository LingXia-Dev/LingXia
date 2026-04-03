mod build;
mod framework;
mod logic;
mod options;
mod package;
mod project;
mod view;

use anyhow::Result;
use std::env;
use std::path::Path;

pub(crate) use framework::ProjectFramework;

/// Page lifecycle method names that are NOT user-defined action handlers.
/// Shared between logic (binding meta extraction) and view (action mode inference).
pub(crate) const PAGE_LIFECYCLE_NAMES: &[&str] = &[
    "onLoad",
    "onShow",
    "onReady",
    "onHide",
    "onUnload",
    "onPullDownRefresh",
];

pub(crate) fn is_page_lifecycle(name: &str) -> bool {
    PAGE_LIFECYCLE_NAMES.contains(&name)
}

pub fn run(args: &[String]) -> Result<()> {
    let cwd = env::current_dir()?;
    run_in_dir(args, &cwd)
}

pub fn run_in_dir(args: &[String], cwd: &Path) -> Result<()> {
    build::run(args, cwd)
}
