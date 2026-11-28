mod build;
mod bundle;
mod framework;
mod hardening;
mod i18n;
mod logic;
mod native_client;
mod options;
mod package;
mod page_types;
mod project;
mod renderer;
mod ts_worker;
mod typecheck;
mod view;

use anyhow::Result;
use std::env;
use std::path::Path;

pub(crate) use bundle::is_built_bundle_dir;
pub(crate) use framework::ProjectFramework;
pub(crate) use framework::UI_MODULE_HEADER;
pub(crate) use logic::{MocksBundle, build_mocks};
pub(crate) use project::Project;
pub(crate) use view::stamp_output_html;
pub(crate) use view::{read_view_build_config, view_target_from_dir};

/// What `.lingxia/` holds for editors and `tsc` before any build: page types and native clients.
pub(crate) fn generate_types(project: &Project) -> Result<()> {
    page_types::generate(project)?;
    native_client::generate(project)
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
pub(crate) fn gpu_dev_inputs(dir: &Path, pages: &[String]) -> Result<serde_json::Value> {
    let project = project::Project::discover(dir)?;
    let pages: Vec<&str> = pages.iter().map(String::as_str).collect();
    // Dev inputs serve a session, never a shipped bundle.
    let catalogs = view::gpu::catalogs(&project, &pages, true)?;
    Ok(serde_json::Value::Object(catalogs))
}

pub(crate) use ts_worker::keep_alive as keep_ts_worker_alive;
pub(crate) use typecheck::TYPESCRIPT_RANGE;

pub(crate) fn skip_typecheck() {
    typecheck::skip();
}

pub fn check_in_dir(cwd: &Path, host_problems: Vec<String>) -> Result<()> {
    build::check(cwd, host_problems)
}

/// `lingxia i18n add`: declare `tag` and start its empty catalogue.
pub fn add_language(dir: &Path, tag: &str) -> Result<()> {
    i18n::add_language(dir, tag)
}

/// Dev sessions: the project type-check of the lxapp at `dir`, when it has one; `None` when
/// it does not, else its problems as one report (empty when clean).
pub(crate) fn dev_typecheck(dir: &Path) -> Option<Result<String>> {
    typecheck::applies(dir).then(|| {
        // A saved catalogue changes the message keys pages may name, even before a build lands.
        i18n::refresh_types(dir)?;
        typecheck::run(dir).map(|problems| {
            if problems.is_empty() {
                String::new()
            } else {
                typecheck::report(&problems)
            }
        })
    })
}

/// The editable `src/` of every crate whose routes the lxapp at `dir` calls: its own crate and
/// its path-dependency route crates.
pub(crate) fn native_route_sources(dir: &Path) -> Vec<std::path::PathBuf> {
    let sources = native_client::route_sources(dir).ok().flatten();
    sources
        .into_iter()
        .flat_map(|sources| sources.crates)
        .filter(|route_crate| route_crate.local)
        .map(|route_crate| route_crate.src)
        .collect()
}

/// Rewrites the lxapp's native clients from its crate's routes; true when they changed.
pub(crate) fn regenerate_native_clients(dir: &Path) -> Result<bool> {
    let project = Project::discover(dir)?;
    let out = dir.join(".lingxia");
    let read = || {
        [
            lingxia_native_codegen::LOGIC_CLIENT,
            lingxia_native_codegen::ViewClient::TypeScript.file_name(),
            lingxia_native_codegen::ViewClient::BrowserGlobal.file_name(),
        ]
        .map(|name| std::fs::read(out.join(name)).ok())
    };
    let before = read();
    native_client::generate(&project)?;
    Ok(read() != before)
}

/// Where a session's build of an lxapp records its GPU pages' inputs: beside that build.
pub(crate) fn session_inputs_record(build_dir: &Path) -> std::path::PathBuf {
    let name = build_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    build_dir.with_file_name(format!("{name}.gpu-inputs.json"))
}

/// The compiled i18n a dev build of the lxapp at `dir` last published, for a page recompile.
pub(crate) fn dev_messages(dir: &Path) -> Option<std::path::PathBuf> {
    i18n::compiled_messages(dir, true)
}

pub fn run_in_dir_for_dev(args: &[String], cwd: &Path, output: Option<&Path>) -> Result<()> {
    build::run_for_dev(args, cwd, output)
}

pub(crate) fn package_in_dir(cwd: &Path) -> Result<std::path::PathBuf> {
    let project = project::Project::discover(cwd)?;
    package::package_dist(&project)
}
