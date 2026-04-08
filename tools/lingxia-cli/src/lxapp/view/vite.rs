use crate::lxapp::framework::PageAction;
use crate::lxapp::framework::{self, ProjectFramework};
use crate::lxapp::options::BuildOptions;
use crate::lxapp::project::{Project, ProjectKind};
use crate::lxapp::view::{
    ViewBuildReport, ViewProgress, bridge_metadata_script, extract_page_actions, page_logic_path,
    page_title, render_page_bridge_import, render_page_bridge_module,
    render_page_bridge_runtime_module, validate_component_view_bindings,
};
use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;

const VITE_CONFIG_TEMPLATE: &str =
    include_str!("../../../templates/builder-frameworks/vite.config.mts");

#[derive(Debug, Clone)]
struct ComponentPageBuild {
    page_path: String,
    page_id: String,
    output_extension: &'static str,
    actions: Vec<PageAction>,
}

pub fn build(
    project: &Project,
    options: &BuildOptions,
    progress: Option<ViewProgress>,
) -> Result<ViewBuildReport> {
    cleanup_legacy_view_artifacts(project.root.as_path())?;
    match project.framework {
        ProjectFramework::React | ProjectFramework::Vue => {
            let install_duration =
                ensure_component_view_tooling(project, project.framework, progress.as_ref())?;
            fs::create_dir_all(&project.output_dir)?;
            copy_static_assets(project)?;
            write_root_manifest(project)?;
            build_component_pages(project, options, install_duration, progress)
        }
        ProjectFramework::Html => {
            let total = project.pages.len();
            let started = Instant::now();
            let install_duration = None;
            fs::create_dir_all(&project.output_dir)?;
            copy_static_assets(project)?;
            write_root_manifest(project)?;
            if let Some(progress) = progress.as_ref() {
                progress.preparing_pages(total, project.framework);
            }
            for page_path in &project.pages {
                copy_html_page(project, page_path)?;
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

fn build_component_pages(
    project: &Project,
    options: &BuildOptions,
    install_duration: Option<Duration>,
    progress: Option<ViewProgress>,
) -> Result<ViewBuildReport> {
    let temp_root = create_temp_build_root(project.root.as_path())?;
    let build_root = temp_root
        .path()
        .join(format!("view-{}", project.framework.as_str()));
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
                 → Prefix with _ to suppress, or remove if unused.",
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
    run_vite_build(project.root.as_path(), &config_path, &project.root)?;
    let bundle_duration = bundle_started.elapsed();

    let finalize_started = Instant::now();
    if let Some(progress) = progress.as_ref() {
        progress.finalizing_pages(total);
    }
    finalize_component_pages(project, &build_root.join("dist"), &pages)?;
    let finalize_duration = finalize_started.elapsed();
    drop(temp_root);
    Ok(ViewBuildReport {
        framework: project.framework,
        page_count: total,
        install_duration,
        prepare_duration,
        bundle_duration,
        finalize_duration,
    })
}

fn copy_html_page(project: &Project, page_path: &str) -> Result<()> {
    let source_path = project.root.join(page_path);
    let output_path = project.output_dir.join(page_path);
    let page_dir = source_path.parent().unwrap_or_else(|| Path::new(""));
    let output_dir = output_path.parent().unwrap_or_else(|| Path::new(""));
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let html = fs::read_to_string(&source_path)
        .with_context(|| format!("Failed to read {}", source_path.display()))?;
    fs::write(&output_path, inject_runtime_script(html))
        .with_context(|| format!("Failed to write {}", output_path.display()))?;

    let base = Path::new(page_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("index");
    let config_path = source_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(format!("{base}.json"));
    if config_path.exists() {
        let output_config = output_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(format!("{base}.json"));
        fs::copy(&config_path, output_config)?;
    }

    copy_html_page_support_files(page_dir, output_dir, base)?;
    Ok(())
}

fn copy_html_page_support_files(source_dir: &Path, output_dir: &Path, base: &str) -> Result<()> {
    if !source_dir.exists() {
        return Ok(());
    }

    let html_name = format!("{base}.html");
    let config_name = format!("{base}.json");
    let logic_ts_name = format!("{base}.ts");
    let logic_js_name = format!("{base}.js");

    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let source_path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        let destination_path = output_dir.join(&file_name);

        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
            continue;
        }

        if file_name_str == html_name
            || file_name_str == config_name
            || file_name_str == logic_ts_name
            || file_name_str == logic_js_name
        {
            continue;
        }

        fs::copy(&source_path, &destination_path).with_context(|| {
            format!(
                "Failed to copy {} -> {}",
                source_path.display(),
                destination_path.display()
            )
        })?;
    }

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
    html = rewrite_entry_script_path(html, &page.page_id);
    html = inject_runtime_script(html);
    html = inject_bridge_metadata(html, &page.actions);
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
        copy_dir_recursive(&entry_path, &project.output_dir.join(entry.file_name()))?;
    }
    Ok(())
}

fn rewrite_entry_script_path(mut html: String, page_id: &str) -> String {
    let expected = format!("src=\"/pages/{page_id}/{page_id}.js\"");
    let expected_rel = format!("src=\"./{page_id}.js\"");
    let expected_plain = format!("src=\"{page_id}.js\"");
    for pattern in [
        expected.as_str(),
        expected_rel.as_str(),
        expected_plain.as_str(),
    ] {
        if html.contains(pattern) {
            html = html.replace(pattern, "src=\"./view.js\"");
        }
    }
    html
}

fn inject_runtime_script(mut html: String) -> String {
    let runtime_script = "<script src=\"lx://assets/runtime.js\"></script>";
    if html.contains(runtime_script) {
        return html;
    }
    if let Some(index) = html.find("</head>") {
        html.insert_str(index, runtime_script);
        return html;
    }
    format!("{runtime_script}{html}")
}

fn inject_bridge_metadata(
    mut html: String,
    actions: &[crate::lxapp::framework::PageAction],
) -> String {
    let metadata = format!("<script>\n{}\n</script>", bridge_metadata_script(actions));
    if html.contains(&metadata) {
        return html;
    }
    if let Some(index) = html.find("</body>") {
        html.insert_str(index, &metadata);
        return html;
    }
    html.push_str(&metadata);
    html
}

fn write_vite_config(
    project: &Project,
    build_dir: &Path,
    options: &BuildOptions,
    framework: ProjectFramework,
    inputs: &BTreeMap<String, PathBuf>,
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

fn create_temp_build_root(project_root: &Path) -> Result<TempDir> {
    tempfile::Builder::new()
        .prefix(".lingxia-view-")
        .tempdir_in(project_root)
        .with_context(|| {
            format!(
                "Failed to create temporary view build directory in {}",
                project_root.display()
            )
        })
}

fn cleanup_legacy_view_artifacts(project_root: &Path) -> Result<()> {
    let lingxia_dir = project_root.join(".lingxia");
    for relative in ["build", "vite"] {
        let path = lingxia_dir.join(relative);
        if path.exists() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("Failed to remove legacy {}", path.display()))?;
        }
    }
    Ok(())
}

fn run_vite_build(project_root: &Path, config_path: &Path, working_dir: &Path) -> Result<()> {
    let vite_bin = project_vite_bin(project_root);
    let status = Command::new("node")
        .arg(&vite_bin)
        .arg("build")
        .arg("--config")
        .arg(config_path)
        .current_dir(working_dir)
        .status()
        .with_context(|| format!("Failed to execute Vite via {}", vite_bin.display()))?;

    if !status.success() {
        return Err(anyhow!("Vite build exited with status {status}"));
    }

    Ok(())
}

fn ensure_component_view_tooling(
    project: &Project,
    framework: ProjectFramework,
    progress: Option<&ViewProgress>,
) -> Result<Option<Duration>> {
    if let Some(progress) = progress {
        progress.ensuring_tooling();
    }

    let install_duration = ensure_project_tooling(project, progress)?;
    let vite_bin = project_vite_bin(project.root.as_path());
    if !vite_bin.exists() {
        return Err(anyhow!(
            "Missing project Vite dependency: {}.\nAdd \"vite\" to devDependencies and run npm install.",
            vite_bin.display()
        ));
    }

    let framework_plugin = match framework {
        ProjectFramework::React => Some((
            "@vitejs/plugin-react",
            project
                .root
                .join("node_modules")
                .join("@vitejs")
                .join("plugin-react")
                .join("package.json"),
        )),
        ProjectFramework::Vue => Some((
            "@vitejs/plugin-vue",
            project
                .root
                .join("node_modules")
                .join("@vitejs")
                .join("plugin-vue")
                .join("package.json"),
        )),
        ProjectFramework::Html => None,
    };

    if let Some((package_name, package_json)) = framework_plugin
        && !package_json.exists()
    {
        return Err(anyhow!(
            "Missing project framework plugin: {}.\nAdd \"{}\" to devDependencies and run npm install.",
            package_json.display(),
            package_name
        ));
    }

    Ok(install_duration)
}

fn ensure_project_tooling(
    project: &Project,
    progress: Option<&ViewProgress>,
) -> Result<Option<Duration>> {
    let package_json = project.root.join("package.json");
    if !package_json.exists() {
        return Ok(None);
    }

    let package_json_value: Value = serde_json::from_str(&fs::read_to_string(&package_json)?)
        .with_context(|| format!("Failed to parse {}", package_json.display()))?;
    let has_declared_deps = package_json_value
        .get("dependencies")
        .and_then(Value::as_object)
        .map(|deps| !deps.is_empty())
        .unwrap_or(false)
        || package_json_value
            .get("devDependencies")
            .and_then(Value::as_object)
            .map(|deps| !deps.is_empty())
            .unwrap_or(false);
    if !has_declared_deps {
        return Ok(None);
    }

    let node_modules_dir = project.root.join("node_modules");
    if node_modules_dir.exists() {
        return Ok(None);
    }

    let use_ci = project.root.join("package-lock.json").exists();
    let mut command = Command::new("npm");
    if use_ci {
        command.arg("ci");
    } else {
        command.arg("install");
    }

    let started = Instant::now();
    if let Some(progress) = progress {
        progress.installing_project_deps();
    }
    let status = command
        .current_dir(&project.root)
        .status()
        .with_context(|| {
            format!(
                "Failed to install project dependencies in {}",
                project.root.display()
            )
        })?;
    if !status.success() {
        return Err(anyhow!(
            "Failed to install project dependencies with npm {}",
            if use_ci { "ci" } else { "install" }
        ));
    }
    if !node_modules_dir.exists() {
        return Err(anyhow!(
            "Project dependency install finished without creating {}",
            node_modules_dir.display()
        ));
    }
    Ok(Some(started.elapsed()))
}

fn write_root_manifest(project: &Project) -> Result<()> {
    match project.kind {
        ProjectKind::LxApp => {
            let source = project.root.join("lxapp.json");
            let mut value: Value = serde_json::from_str(&fs::read_to_string(&source)?)?;
            let page_map = project
                .pages
                .iter()
                .map(|page| (strip_ext(page), page.clone()))
                .collect::<std::collections::HashMap<_, _>>();

            if let Some(pages) = value.get_mut("pages").and_then(Value::as_array_mut) {
                for page in pages.iter_mut() {
                    if let Some(raw) = page.as_str() {
                        if let Some(resolved) = page_map.get(&strip_ext(raw)) {
                            *page = Value::String(resolved.clone());
                        }
                    }
                }
            }

            if let Some(list) = value
                .get_mut("tabBar")
                .and_then(|value| value.get_mut("list"))
                .and_then(Value::as_array_mut)
            {
                for item in list.iter_mut() {
                    if let Some(page_path) = item.get("pagePath").and_then(Value::as_str) {
                        if let Some(resolved) = page_map.get(&strip_ext(page_path)) {
                            if let Some(object) = item.as_object_mut() {
                                object.insert(
                                    "pagePath".to_string(),
                                    Value::String(resolved.clone()),
                                );
                            }
                        }
                    }
                }
            }

            fs::write(
                project.output_dir.join("lxapp.json"),
                serde_json::to_string_pretty(&value)?,
            )?;
        }
        ProjectKind::LxPlugin => {
            fs::copy(
                project.root.join("lxplugin.json"),
                project.output_dir.join("lxplugin.json"),
            )?;
        }
    }
    Ok(())
}

fn copy_static_assets(project: &Project) -> Result<()> {
    let public_dir = project.root.join("public");
    if public_dir.exists() {
        copy_dir_recursive(&public_dir, &project.output_dir.join("public"))?;
    }

    let pages_dir = project.root.join("pages");
    if pages_dir.exists() {
        for page_dir in fs::read_dir(&pages_dir)? {
            let page_dir = page_dir?;
            if !page_dir.file_type()?.is_dir() {
                continue;
            }
            let images_dir = page_dir.path().join("images");
            if images_dir.exists() {
                copy_dir_recursive(
                    &images_dir,
                    &project
                        .output_dir
                        .join("pages")
                        .join(page_dir.file_name())
                        .join("images"),
                )?;
            }
        }
    }
    Ok(())
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "Failed to copy {} -> {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
    }
    Ok(())
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

fn strip_ext(path: &str) -> String {
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

fn project_vite_bin(project_root: &Path) -> PathBuf {
    project_root
        .join("node_modules")
        .join("vite")
        .join("bin")
        .join("vite.js")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_file(root: &Path, relative: &str, content: &str) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn make_project(root: &Path, framework: ProjectFramework) -> Project {
        Project {
            root: root.to_path_buf(),
            kind: ProjectKind::LxApp,
            framework,
            output_dir: root.join("dist"),
            pages: vec!["pages/home/index".to_string()],
            logic_entry: Some("logic.js".to_string()),
            plugin_id: None,
            package_name: Some("demo".to_string()),
            version: "1.0.0".to_string(),
        }
    }

    #[test]
    fn project_vite_bin_points_to_project_node_modules() {
        let temp = tempdir().unwrap();
        assert_eq!(
            project_vite_bin(temp.path()),
            temp.path()
                .join("node_modules")
                .join("vite")
                .join("bin")
                .join("vite.js")
        );
    }

    #[test]
    fn react_view_tooling_requires_project_vite_dependency() {
        let temp = tempdir().unwrap();
        write_file(temp.path(), "package.json", r#"{ "name": "demo" }"#);
        fs::create_dir_all(temp.path().join("node_modules")).unwrap();

        let error = ensure_component_view_tooling(
            &make_project(temp.path(), ProjectFramework::React),
            ProjectFramework::React,
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("Missing project Vite dependency"));
    }

    #[test]
    fn vue_view_tooling_requires_framework_plugin() {
        let temp = tempdir().unwrap();
        write_file(temp.path(), "package.json", r#"{ "name": "demo" }"#);
        write_file(
            temp.path(),
            "node_modules/vite/bin/vite.js",
            "console.log('vite');",
        );

        let error = ensure_component_view_tooling(
            &make_project(temp.path(), ProjectFramework::Vue),
            ProjectFramework::Vue,
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("Missing project framework plugin"));
        assert!(error.contains("@vitejs/plugin-vue"));
    }

    #[test]
    fn react_view_tooling_accepts_project_scoped_vite_setup() {
        let temp = tempdir().unwrap();
        write_file(temp.path(), "package.json", r#"{ "name": "demo" }"#);
        write_file(
            temp.path(),
            "node_modules/vite/bin/vite.js",
            "console.log('vite');",
        );
        write_file(
            temp.path(),
            "node_modules/@vitejs/plugin-react/package.json",
            r#"{ "name": "@vitejs/plugin-react" }"#,
        );

        let install_duration = ensure_component_view_tooling(
            &make_project(temp.path(), ProjectFramework::React),
            ProjectFramework::React,
            None,
        )
        .unwrap();

        assert_eq!(install_duration, None);
    }

    #[test]
    fn html_page_copy_injects_runtime_script() {
        let temp = tempdir().unwrap();
        let project = Project {
            root: temp.path().to_path_buf(),
            kind: ProjectKind::LxApp,
            framework: ProjectFramework::Html,
            output_dir: temp.path().join("dist"),
            pages: vec!["pages/settings/index.html".to_string()],
            logic_entry: None,
            plugin_id: None,
            package_name: Some("demo".to_string()),
            version: "1.0.0".to_string(),
        };
        write_file(
            temp.path(),
            "pages/settings/index.html",
            "<!DOCTYPE html><html><head><title>Settings</title></head><body></body></html>",
        );

        copy_html_page(&project, "pages/settings/index.html").unwrap();

        let output =
            fs::read_to_string(project.output_dir.join("pages/settings/index.html")).unwrap();
        assert!(output.contains("lx://assets/runtime.js"));
    }

    #[test]
    fn html_page_copy_preserves_support_files() {
        let temp = tempdir().unwrap();
        let project = Project {
            root: temp.path().to_path_buf(),
            kind: ProjectKind::LxApp,
            framework: ProjectFramework::Html,
            output_dir: temp.path().join("dist"),
            pages: vec!["pages/downloads/index.html".to_string()],
            logic_entry: None,
            plugin_id: None,
            package_name: Some("demo".to_string()),
            version: "1.0.0".to_string(),
        };
        write_file(
            temp.path(),
            "pages/downloads/index.html",
            "<!DOCTYPE html><html><body><script src=\"./download.js\"></script></body></html>",
        );
        write_file(
            temp.path(),
            "pages/downloads/download.js",
            "console.log('download helper');",
        );
        write_file(temp.path(), "pages/downloads/index.ts", "Page({});");

        copy_html_page(&project, "pages/downloads/index.html").unwrap();

        assert!(
            project
                .output_dir
                .join("pages/downloads/download.js")
                .exists()
        );
        assert!(!project.output_dir.join("pages/downloads/index.ts").exists());
    }
}
