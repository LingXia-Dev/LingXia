mod build;
mod bundle;
mod framework;
mod hardening;
mod logic;
mod media;
mod options;
mod package;
mod page_types;
mod project;
mod view;

use anyhow::Result;
use std::env;
use std::path::Path;

pub(crate) use bundle::is_built_bundle_dir;
pub(crate) use framework::ProjectFramework;
pub(crate) use logic::{MocksBundle, build_mocks};
pub(crate) use media::audit_output_media;
pub(crate) use project::Project;
pub(crate) use view::native_client_output_path;
pub(crate) use view::stamp_output_html;
pub(crate) use view::{read_view_build_config, view_target_from_dir};

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

/// Dev sessions: the build output directory of the lxapp at `dir` and the action
/// catalogs of the GPU `pages` (`pages/x/index.tsx`) it is about to recompile.
pub(crate) fn gpu_dev_inputs(
    dir: &Path,
    pages: &[String],
) -> Result<(std::path::PathBuf, serde_json::Value)> {
    let project = project::Project::discover(dir)?;
    let pages: Vec<&str> = pages.iter().map(String::as_str).collect();
    let catalogs = view::gpu::catalogs(&project, &pages)?;
    Ok((
        project.output_dir.clone(),
        serde_json::Value::Object(catalogs),
    ))
}

pub fn run_in_dir_for_dev(args: &[String], cwd: &Path) -> Result<()> {
    build::run_for_dev(args, cwd)
}

pub(crate) fn package_in_dir(cwd: &Path) -> Result<std::path::PathBuf> {
    let project = project::Project::discover(cwd)?;
    package::package_dist(&project)
}
