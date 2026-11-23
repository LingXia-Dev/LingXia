mod build;
mod bundle;
mod framework;
mod hardening;
mod i18n;
mod logic;
mod options;
mod package;
mod page_types;
mod project;
mod renderer;
mod typecheck;
mod view;

use anyhow::Result;
use std::env;
use std::path::Path;

pub(crate) use bundle::is_built_bundle_dir;
pub(crate) use framework::ProjectFramework;
pub(crate) use framework::UI_MODULE_HEADER;
pub(crate) use logic::{MocksBundle, build_mocks};
pub(crate) use page_types::generate as generate_page_types;
pub(crate) use project::Project;
pub(crate) use view::native_client_output_path;
pub(crate) use view::stamp_output_html;
pub(crate) use view::{read_view_build_config, view_target_from_dir};

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
    // Dev inputs serve a session, never a shipped bundle.
    let catalogs = view::gpu::catalogs(&project, &pages, true)?;
    Ok((
        project.output_dir.clone(),
        serde_json::Value::Object(catalogs),
    ))
}

pub(crate) fn skip_typecheck() {
    typecheck::skip();
}

pub fn check_in_dir(cwd: &Path) -> Result<()> {
    build::check(cwd)
}

/// Dev sessions: the project type-check of the lxapp at `dir`, when it has one; `None` when
/// it does not, else its problems as one report (empty when clean).
pub(crate) fn dev_typecheck(dir: &Path) -> Option<Result<String>> {
    typecheck::applies(dir).then(|| {
        typecheck::run(dir).map(|problems| {
            if problems.is_empty() {
                String::new()
            } else {
                typecheck::report(&problems)
            }
        })
    })
}

pub fn run_in_dir_for_dev(args: &[String], cwd: &Path) -> Result<()> {
    build::run_for_dev(args, cwd)
}

pub(crate) fn package_in_dir(cwd: &Path) -> Result<std::path::PathBuf> {
    let project = project::Project::discover(cwd)?;
    package::package_dist(&project)
}
