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
pub(crate) use project::Project;
pub(crate) use view::native_client_output_path;
pub(crate) use view::view_target_from_dir;

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

pub(crate) fn parse_framework_override(
    framework: Option<&str>,
) -> Result<Option<ProjectFramework>> {
    match framework {
        Some("react") => Ok(Some(ProjectFramework::React)),
        Some("vue") => Ok(Some(ProjectFramework::Vue)),
        Some("html") => Ok(Some(ProjectFramework::Html)),
        Some(other) => Err(anyhow::anyhow!("Unsupported framework: {other}")),
        None => Ok(None),
    }
}

pub(crate) fn package_in_dir(cwd: &Path, framework: Option<&str>) -> Result<std::path::PathBuf> {
    let framework_override = parse_framework_override(framework)?;
    let project = project::Project::discover(cwd, framework_override)?;
    package::package_dist(&project)
}
