use crate::lxapp::framework::PageAction;
use crate::lxapp::framework::{self, ProjectFramework};
use crate::lxapp::options::BuildOptions;
use crate::lxapp::project::Project;
use crate::lxapp::view::{
    ViewBuildReport, ViewProgress, extract_page_actions, page_logic_path, page_title,
    render_page_bridge_import, render_page_bridge_module, render_page_bridge_runtime_module,
    validate_component_view_bindings,
};
use anyhow::{Context, Result, anyhow};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const VITE_CONFIG_TEMPLATE: &str =
    include_str!("../../../templates/builder-frameworks/vite.config.mts");

#[derive(Debug, Clone)]
struct ComponentPageBuild {
    page_path: String,
    page_id: String,
    output_extension: &'static str,
    actions: Vec<PageAction>,
}

#[derive(Debug, Clone, Copy)]
enum HtmlBundleMode {
    Modern,
    LegacyEs5,
}

pub fn build(
    project: &Project,
    options: &BuildOptions,
    progress: Option<ViewProgress>,
    install_duration_hint: Option<Duration>,
) -> Result<ViewBuildReport> {
    super::vite_tooling::cleanup_legacy_view_artifacts(project.root.as_path())?;
    match project.framework {
        ProjectFramework::React | ProjectFramework::Vue => {
            let install_duration = super::vite_tooling::ensure_component_view_tooling(
                project,
                project.framework,
                options,
                progress.as_ref(),
                install_duration_hint,
            )?;
            fs::create_dir_all(&project.output_dir)?;
            super::vite_assets::copy_static_assets(project)?;
            super::vite_assets::write_root_manifest(project)?;
            build_component_pages(project, options, install_duration, progress)
        }
        ProjectFramework::Html => {
            if super::vite_html::html_pages_require_bundling(project)? {
                let install_duration = super::vite_tooling::ensure_component_view_tooling(
                    project,
                    ProjectFramework::Html,
                    options,
                    progress.as_ref(),
                    install_duration_hint,
                )?;
                fs::create_dir_all(&project.output_dir)?;
                super::vite_assets::copy_static_assets(project)?;
                super::vite_assets::write_root_manifest(project)?;
                return build_html_pages(project, options, install_duration, progress);
            }

            let total = project.pages.len();
            let started = Instant::now();
            let install_duration = None;
            fs::create_dir_all(&project.output_dir)?;
            super::vite_assets::copy_static_assets(project)?;
            super::vite_assets::write_root_manifest(project)?;
            if let Some(progress) = progress.as_ref() {
                progress.preparing_pages(total, project.framework);
            }
            for page_path in &project.pages {
                super::vite_html::copy_html_page(project, page_path)?;
            }
            Ok(ViewBuildReport {
                framework: project.framework,
                page_count: total,
                install_duration,
                prepare_duration: started.elapsed(),
                bundle_duration: Duration::ZERO,
                finalize_duration: Duration::ZERO,
            })
        }
    }
}

pub fn prepare_tooling(
    project: &Project,
    options: &BuildOptions,
    progress: Option<ViewProgress>,
) -> Result<Option<Duration>> {
    match project.framework {
        ProjectFramework::React | ProjectFramework::Vue => {
            super::vite_tooling::ensure_project_tooling(project, options, progress.as_ref())
        }
        ProjectFramework::Html => {
            if super::vite_html::html_pages_require_bundling(project)? {
                super::vite_tooling::ensure_component_view_tooling(
                    project,
                    ProjectFramework::Html,
                    options,
                    progress.as_ref(),
                    None,
                )
            } else {
                Ok(None)
            }
        }
    }
}

fn build_component_pages(
    project: &Project,
    options: &BuildOptions,
    install_duration: Option<Duration>,
    progress: Option<ViewProgress>,
) -> Result<ViewBuildReport> {
    let build_root =
        super::vite_tooling::prepare_view_build_root(project.root.as_path(), project.framework)?;
    fs::create_dir_all(&build_root)?;
    fs::write(
        build_root.join("__page_bridge_runtime__.js"),
        render_page_bridge_runtime_module(),
    )?;

    let total = project.pages.len();
    let mut inputs = BTreeMap::new();
    let mut pages = Vec::with_capacity(total);
    let prepare_started = Instant::now();
    if let Some(progress) = progress.as_ref() {
        progress.preparing_pages(total, project.framework);
    }

    for page_path in &project.pages {
        let page_title = page_title(project, page_path)?;
        let logic_path = page_logic_path(project, page_path)?;
        let actions = extract_page_actions(logic_path.as_deref())?;
        let usage_audit = validate_component_view_bindings(project, page_path, &actions)?;
        let unused_actions = actions
            .iter()
            .map(|action| action.name.as_str())
            .filter(|name| !name.starts_with('_') && !usage_audit.used_actions.contains(*name))
            .collect::<Vec<_>>();
        if !unused_actions.is_empty() {
            eprintln!(
                "Warning: view {} does not reference Page(...) actions: {}\n  \
                 → Move logic-only helpers outside Page(...), prefix intentional private actions with _, or remove unused actions.",
                page_path,
                unused_actions.join(", ")
            );
        }
        let page_id = sanitize_page_id(&strip_ext(page_path));
        let build_dir = build_root.join("pages").join(&page_id);
        fs::create_dir_all(&build_dir)?;

        let source_path = project.root.join(page_path);
        let app_import = format!(
            "import App from '{}';",
            relative_import_path(&build_dir, &source_path)?
        );
        let scaffold = framework::scaffold(
            project.framework,
            &page_title,
            &app_import,
            &render_page_bridge_import(),
        );

        fs::write(build_dir.join("index.html"), scaffold.index_html)?;
        fs::write(
            build_dir.join(scaffold.main_entry_filename),
            scaffold.main_entry,
        )?;
        let bridge_runtime_import =
            relative_import_path(&build_dir, &build_root.join("__page_bridge_runtime__.js"))?;
        fs::write(
            build_dir.join("__page_bridge__.js"),
            render_page_bridge_module(&actions, &bridge_runtime_import),
        )?;

        inputs.insert(page_id.clone(), build_dir.join("index.html"));
        pages.push(ComponentPageBuild {
            page_path: page_path.clone(),
            page_id,
            output_extension: scaffold.output_extension,
            actions,
        });
    }
    let prepare_duration = prepare_started.elapsed();

    let bundle_started = Instant::now();
    let config_path = write_vite_config(project, &build_root, options, project.framework, &inputs)?;
    if let Some(progress) = progress.as_ref() {
        progress.bundling_pages(total, project.framework);
    }
    super::vite_tooling::run_vite_build(project.root.as_path(), &config_path, &project.root)?;
    let bundle_duration = bundle_started.elapsed();

    let finalize_started = Instant::now();
    if let Some(progress) = progress.as_ref() {
        progress.finalizing_pages(total);
    }
    finalize_component_pages(project, &build_root.join("dist"), &pages)?;
    let finalize_duration = finalize_started.elapsed();
    Ok(ViewBuildReport {
        framework: project.framework,
        page_count: total,
        install_duration,
        prepare_duration,
        bundle_duration,
        finalize_duration,
    })
}

fn build_html_pages(
    project: &Project,
    options: &BuildOptions,
    install_duration: Option<Duration>,
    progress: Option<ViewProgress>,
) -> Result<ViewBuildReport> {
    let mode = match super::vite_html::html_view_target(project)?.as_deref() {
        Some(target) if target.eq_ignore_ascii_case("es5") => HtmlBundleMode::LegacyEs5,
        _ => HtmlBundleMode::Modern,
    };
    if matches!(mode, HtmlBundleMode::LegacyEs5) {
        return build_html_pages_legacy(project, options, install_duration, progress);
    }

    let build_root =
        super::vite_tooling::prepare_view_build_root(project.root.as_path(), project.framework)?;
    fs::create_dir_all(&build_root)?;

    let total = project.pages.len();
    let mut inputs = BTreeMap::new();
    let mut pages = Vec::with_capacity(total);
    let prepare_started = Instant::now();
    if let Some(progress) = progress.as_ref() {
        progress.preparing_pages(total, project.framework);
    }

    for page_path in &project.pages {
        let actions = extract_page_actions(page_logic_path(project, page_path)?.as_deref())?;
        let page_id = sanitize_page_id(&strip_ext(page_path));
        let build_dir = build_root.join("pages").join(&page_id);
        fs::create_dir_all(&build_dir)?;

        let source_path = project.root.join(page_path);
        let base = Path::new(page_path)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("index");
        fs::copy(&source_path, build_dir.join("index.html")).with_context(|| {
            format!(
                "Failed to copy {} -> {}",
                source_path.display(),
                build_dir.join("index.html").display()
            )
        })?;
        super::vite_html::copy_html_page_support_files(
            source_path.parent().unwrap_or_else(|| Path::new("")),
            &build_dir,
            base,
        )?;

        inputs.insert(page_id.clone(), build_dir.join("index.html"));
        pages.push(ComponentPageBuild {
            page_path: page_path.clone(),
            page_id,
            output_extension: ".html",
            actions,
        });
    }
    let prepare_duration = prepare_started.elapsed();

    let bundle_started = Instant::now();
    let config_path = write_vite_config(project, &build_root, options, project.framework, &inputs)?;
    if let Some(progress) = progress.as_ref() {
        progress.bundling_pages(total, project.framework);
    }
    super::vite_tooling::run_vite_build(project.root.as_path(), &config_path, &project.root)?;
    let bundle_duration = bundle_started.elapsed();

    let finalize_started = Instant::now();
    if let Some(progress) = progress.as_ref() {
        progress.finalizing_pages(total);
    }
    finalize_component_pages(project, &build_root.join("dist"), &pages)?;
    let finalize_duration = finalize_started.elapsed();
    Ok(ViewBuildReport {
        framework: project.framework,
        page_count: total,
        install_duration,
        prepare_duration,
        bundle_duration,
        finalize_duration,
    })
}

fn build_html_pages_legacy(
    project: &Project,
    options: &BuildOptions,
    install_duration: Option<Duration>,
    progress: Option<ViewProgress>,
) -> Result<ViewBuildReport> {
    let build_root =
        super::vite_tooling::prepare_view_build_root(project.root.as_path(), project.framework)?;
    fs::create_dir_all(&build_root)?;

    let total = project.pages.len();
    let mut pages = Vec::with_capacity(total);
    let prepare_started = Instant::now();
    if let Some(progress) = progress.as_ref() {
        progress.preparing_pages(total, project.framework);
    }

    for page_path in &project.pages {
        let actions = extract_page_actions(page_logic_path(project, page_path)?.as_deref())?;
        let page_id = sanitize_page_id(&strip_ext(page_path));
        let source_path = project.root.join(page_path);
        let html = fs::read_to_string(&source_path)
            .with_context(|| format!("Failed to read {}", source_path.display()))?;
        let entry_src = super::vite_html::extract_html_module_entry_script_path(&html).ok_or_else(
            || anyhow!("HTML page {page_path} is configured for ES5 bundling but has no module entry script"),
        )?;
        pages.push(HtmlLegacyPageBuild {
            page_path: page_path.clone(),
            page_id,
            entry_src,
            actions,
        });
    }
    let prepare_duration = prepare_started.elapsed();

    let bundle_started = Instant::now();
    if let Some(progress) = progress.as_ref() {
        progress.bundling_pages(total, project.framework);
    }
    for page in &pages {
        bundle_html_legacy_page(project, options, &build_root, page)?;
    }
    let bundle_duration = bundle_started.elapsed();

    let finalize_started = Instant::now();
    if let Some(progress) = progress.as_ref() {
        progress.finalizing_pages(total);
    }
    for page in &pages {
        finalize_html_legacy_page(project, &build_root, page)?;
    }
    let finalize_duration = finalize_started.elapsed();

    Ok(ViewBuildReport {
        framework: project.framework,
        page_count: total,
        install_duration,
        prepare_duration,
        bundle_duration,
        finalize_duration,
    })
}

#[derive(Debug, Clone)]
struct HtmlLegacyPageBuild {
    page_path: String,
    page_id: String,
    entry_src: String,
    actions: Vec<PageAction>,
}

fn bundle_html_legacy_page(
    project: &Project,
    options: &BuildOptions,
    build_root: &Path,
    page: &HtmlLegacyPageBuild,
) -> Result<()> {
    let page_root = build_root.join("legacy").join(&page.page_id);
    fs::create_dir_all(&page_root)?;

    let page_dir = project.root.join(
        Path::new(&page.page_path)
            .parent()
            .unwrap_or_else(|| Path::new("")),
    );
    let entry_path = page_dir.join(&page.entry_src);
    if !entry_path.exists() {
        return Err(anyhow!(
            "Missing HTML module entry for {}: {}",
            page.page_path,
            entry_path.display()
        ));
    }

    let mut inputs = BTreeMap::new();
    inputs.insert(page.page_id.clone(), entry_path);
    let config_path = write_vite_config_with_target(
        project,
        &page_root,
        options,
        project.framework,
        &inputs,
        "es2015",
        Some("iife"),
        true,
        false,
    )?;
    super::vite_tooling::run_vite_build(project.root.as_path(), &config_path, &project.root)
}

fn finalize_html_legacy_page(
    project: &Project,
    build_root: &Path,
    page: &HtmlLegacyPageBuild,
) -> Result<()> {
    let dist_dir = build_root.join("legacy").join(&page.page_id).join("dist");
    if !dist_dir.exists() {
        return Err(anyhow!("Missing Vite dist output: {}", dist_dir.display()));
    }

    copy_shared_vite_assets(project, &dist_dir)?;

    let page_output_dir = project.output_dir.join(
        Path::new(&page.page_path)
            .parent()
            .unwrap_or_else(|| Path::new("")),
    );
    fs::create_dir_all(&page_output_dir)?;

    let page_dist_dir = dist_dir.join("pages").join(&page.page_id);
    let view_js = page_dist_dir.join(format!("{}.js", page.page_id));
    super::vite_tooling::transpile_file_to_es5(project.root.as_path(), &view_js)?;
    fs::copy(&view_js, page_output_dir.join("view.js"))
        .with_context(|| format!("Failed to copy {}", view_js.display()))?;

    let source_path = project.root.join(&page.page_path);
    let mut html = fs::read_to_string(&source_path)
        .with_context(|| format!("Failed to read {}", source_path.display()))?;
    html = super::vite_html::rewrite_module_entry_script(html, &page.entry_src, "./view.js");
    // Legacy pipeline — emitted because the user opted into ES5 — needs the
    // polyfills script before bridge-runtime so old Android WebView gets
    // Object.assign / Promise.finally / etc.
    html = super::vite_html::inject_runtime_script(html, true);
    html = super::vite_html::inject_bridge_metadata(html, &page.actions);

    let base = Path::new(&page.page_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("index");
    let page_file = page_output_dir.join(format!("{base}.html"));
    fs::write(&page_file, html)
        .with_context(|| format!("Failed to write {}", page_file.display()))?;

    let config_path = source_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(format!("{base}.json"));
    if config_path.exists() {
        fs::copy(&config_path, page_output_dir.join(format!("{base}.json")))?;
    }

    super::vite_html::copy_html_page_support_files(
        source_path.parent().unwrap_or_else(|| Path::new("")),
        &page_output_dir,
        base,
    )?;
    Ok(())
}

fn finalize_component_pages(
    project: &Project,
    dist_dir: &Path,
    pages: &[ComponentPageBuild],
) -> Result<()> {
    if !dist_dir.exists() {
        return Err(anyhow!("Missing Vite dist output: {}", dist_dir.display()));
    }

    copy_shared_vite_assets(project, dist_dir)?;

    for page in pages {
        finalize_component_page(project, dist_dir, page)?;
    }
    Ok(())
}

fn finalize_component_page(
    project: &Project,
    dist_dir: &Path,
    page: &ComponentPageBuild,
) -> Result<()> {
    let page_output_dir = project.output_dir.join(
        Path::new(&page.page_path)
            .parent()
            .unwrap_or_else(|| Path::new("")),
    );
    fs::create_dir_all(&page_output_dir)?;

    let page_dist_dir = dist_dir.join("pages").join(&page.page_id);
    let view_js = page_dist_dir.join(format!("{}.js", page.page_id));
    if view_js.exists() {
        fs::copy(&view_js, page_output_dir.join("view.js"))
            .with_context(|| format!("Failed to copy {}", view_js.display()))?;
    }

    let base = Path::new(&page.page_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("index");
    let page_json = project
        .root
        .join(
            Path::new(&page.page_path)
                .parent()
                .unwrap_or_else(|| Path::new("")),
        )
        .join(format!("{base}.json"));
    if page_json.exists() {
        fs::copy(&page_json, page_output_dir.join(format!("{base}.json")))?;
    }

    let html_path = page_dist_dir.join("index.html");
    let mut html = fs::read_to_string(&html_path)
        .with_context(|| format!("Failed to read {}", html_path.display()))?;
    html = super::vite_html::rewrite_entry_script_path(html, &page.page_id);
    // Modern pipeline targets Chromium >= 51 (Android 7+); no polyfills.
    html = super::vite_html::inject_runtime_script(html, false);
    html = super::vite_html::inject_bridge_metadata(html, &page.actions);
    let page_file = page_output_dir.join(format!("{base}{}", page.output_extension));
    fs::write(&page_file, html)
        .with_context(|| format!("Failed to write {}", page_file.display()))?;
    Ok(())
}

fn copy_shared_vite_assets(project: &Project, dist_dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dist_dir)? {
        let entry = entry?;
        let entry_path = entry.path();
        let file_type = entry.file_type()?;
        if !file_type.is_dir() {
            continue;
        }
        if entry.file_name() == "pages" {
            continue;
        }
        super::vite_assets::copy_dir_recursive(
            &entry_path,
            &project.output_dir.join(entry.file_name()),
        )?;
    }
    Ok(())
}

fn write_vite_config(
    project: &Project,
    build_dir: &Path,
    options: &BuildOptions,
    framework: ProjectFramework,
    inputs: &BTreeMap<String, PathBuf>,
) -> Result<PathBuf> {
    write_vite_config_with_target(
        project, build_dir, options, framework, inputs, "esnext", None, false, true,
    )
}

fn write_vite_config_with_target(
    project: &Project,
    build_dir: &Path,
    options: &BuildOptions,
    framework: ProjectFramework,
    inputs: &BTreeMap<String, PathBuf>,
    build_target: &str,
    output_format: Option<&str>,
    inline_dynamic_imports: bool,
    module_preload: bool,
) -> Result<PathBuf> {
    let config_path = build_dir.join("vite.config.mts");

    let plugin_import = match framework {
        ProjectFramework::React => {
            "import react from '@vitejs/plugin-react';\nconst frameworkPlugins = [react()];"
        }
        ProjectFramework::Vue => {
            "import vue from '@vitejs/plugin-vue';\nconst frameworkPlugins = [vue()];"
        }
        ProjectFramework::Html => "const frameworkPlugins = [];",
    };

    let lxapp_config_path = project.root.join("lxapp.config.ts");
    let maybe_config_import = if lxapp_config_path.exists() {
        format!(
            "let projectConfig = {{}};\ntry {{\n  const mod = await import({});\n  projectConfig = mod.default ?? mod ?? {{}};\n}} catch {{\n  projectConfig = {{}};\n}}\n",
            serde_json::to_string(&lxapp_config_path.to_string_lossy())
                .unwrap_or_else(|_| "\"\"".to_string())
        )
    } else {
        "const projectConfig = {};\n".to_string()
    };
    let input_json = serde_json::to_string(
        &inputs
            .iter()
            .map(|(name, path)| (name.clone(), path.to_string_lossy().to_string()))
            .collect::<BTreeMap<_, _>>(),
    )?;

    let config = VITE_CONFIG_TEMPLATE
        .replace("__PLUGIN_IMPORT__", plugin_import)
        .replace(
            "__PROJECT_ROOT_JSON__",
            &serde_json::to_string(&project.root.to_string_lossy())
                .unwrap_or_else(|_| "\"\"".to_string()),
        )
        .replace(
            "__BUILD_DIR_JSON__",
            &serde_json::to_string(&build_dir.to_string_lossy())
                .unwrap_or_else(|_| "\"\"".to_string()),
        )
        .replace("__INPUT_ENTRIES_JSON__", &input_json)
        .replace("__MAYBE_CONFIG_IMPORT__", &maybe_config_import)
        .replace(
            "__BUILD_TARGET_JSON__",
            &serde_json::to_string(build_target).unwrap_or_else(|_| "\"esnext\"".to_string()),
        )
        .replace(
            "__OUTPUT_FORMAT_JSON__",
            &serde_json::to_string(&output_format).unwrap_or_else(|_| "null".to_string()),
        )
        .replace(
            "__INLINE_DYNAMIC_IMPORTS__",
            if inline_dynamic_imports {
                "true"
            } else {
                "false"
            },
        )
        .replace(
            "__MODULE_PRELOAD__",
            if module_preload { "true" } else { "false" },
        )
        .replace("__SOURCEMAP__", "false")
        .replace(
            "__MINIFY__",
            if options.release { "'oxc'" } else { "false" },
        )
        .replace(
            "__CSS_MINIFY__",
            if options.release {
                "'lightningcss'"
            } else {
                "false"
            },
        );
    fs::write(&config_path, config)?;
    Ok(config_path)
}

fn relative_import_path(from_dir: &Path, target: &Path) -> Result<String> {
    let from_components = from_dir.components().collect::<Vec<_>>();
    let target_components = target.components().collect::<Vec<_>>();
    let mut shared = 0usize;
    while shared < from_components.len()
        && shared < target_components.len()
        && from_components[shared] == target_components[shared]
    {
        shared += 1;
    }

    let mut relative = PathBuf::new();
    for _ in shared..from_components.len() {
        relative.push("..");
    }
    for component in target_components.iter().skip(shared) {
        relative.push(component.as_os_str());
    }

    let path = relative.to_string_lossy().replace('\\', "/");
    Ok(if path.starts_with('.') {
        path
    } else {
        format!("./{path}")
    })
}

pub(super) fn strip_ext(path: &str) -> String {
    let path_obj = Path::new(path);
    if path_obj.extension().is_none() {
        return path.replace('\\', "/");
    }
    path_obj
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/")
}

fn sanitize_page_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '_',
        })
        .collect()
}
