use super::{ProjectFramework, ViewProgress};
use crate::lxapp::options::BuildOptions;
use crate::lxapp::project::Project;
use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

pub(super) fn prepare_view_build_root(
    project_root: &Path,
    framework: ProjectFramework,
) -> Result<PathBuf> {
    let build_root = project_root
        .join(".lingxia")
        .join("view-build")
        .join(framework.as_str());
    if build_root.exists() {
        fs::remove_dir_all(&build_root)?;
    }
    fs::create_dir_all(&build_root)?;
    Ok(build_root)
}

pub(super) fn cleanup_legacy_view_artifacts(project_root: &Path) -> Result<()> {
    if let Ok(entries) = fs::read_dir(project_root) {
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(".lingxia-view-") {
                fs::remove_dir_all(entry.path())?;
            }
        }
    }

    let lingxia_dir = project_root.join(".lingxia");
    let Ok(entries) = fs::read_dir(&lingxia_dir) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("view-build-") {
            fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

pub(super) fn run_vite_build(
    project_root: &Path,
    config_path: &Path,
    working_dir: &Path,
) -> Result<()> {
    let vite_bin = project_vite_bin(project_root);
    let status = Command::new("node")
        .arg(vite_bin)
        .arg("build")
        .arg("--config")
        .arg(config_path)
        .current_dir(working_dir)
        .status()
        .with_context(|| format!("Failed to start Vite build in {}", working_dir.display()))?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("Vite build failed"))
    }
}

pub(super) fn ensure_component_view_tooling(
    project: &Project,
    framework: ProjectFramework,
    options: &BuildOptions,
    progress: Option<&ViewProgress>,
    install_duration_hint: Option<Duration>,
) -> Result<Option<Duration>> {
    if let Some(progress) = progress {
        progress.ensuring_tooling();
    }

    let install_duration = match install_duration_hint {
        Some(duration) => Some(duration),
        None => ensure_project_tooling(project, options, progress)?,
    };
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

pub(super) fn transpile_file_to_es5(project_root: &Path, file_path: &Path) -> Result<()> {
    let ts_module = project_root
        .join("node_modules")
        .join("typescript")
        .join("lib")
        .join("typescript.js");
    if !ts_module.exists() {
        return Err(anyhow!(
            "Missing project TypeScript dependency: {}.\nAdd \"typescript\" to devDependencies and run npm install.",
            ts_module.display()
        ));
    }

    let transpile_script = format!(
        r#"
import fs from 'node:fs';
import ts from {ts_module};
const filePath = {file_path};
const source = fs.readFileSync(filePath, 'utf8');
const result = ts.transpileModule(source, {{
  compilerOptions: {{
    target: ts.ScriptTarget.ES5,
    module: ts.ModuleKind.None,
    removeComments: false
  }},
  fileName: filePath
}});
fs.writeFileSync(filePath, result.outputText, 'utf8');
"#,
        ts_module = serde_json::to_string(&ts_module.to_string_lossy())
            .unwrap_or_else(|_| "\"\"".to_string()),
        file_path = serde_json::to_string(&file_path.to_string_lossy())
            .unwrap_or_else(|_| "\"\"".to_string()),
    );

    let status = Command::new("node")
        .arg("--input-type=module")
        .arg("--eval")
        .arg(transpile_script)
        .current_dir(project_root)
        .status()
        .with_context(|| format!("Failed to start ES5 transpile for {}", file_path.display()))?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "Failed to transpile {} to ES5",
            file_path.display()
        ))
    }
}

pub(super) fn ensure_project_tooling(
    project: &Project,
    options: &BuildOptions,
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

    let use_ci =
        (options.release || options.package) && project.root.join("package-lock.json").exists();
    let mut command = Command::new("npm");
    if use_ci {
        command.arg("ci");
    } else {
        command.arg("install");
    }
    command.args(["--no-audit", "--no-fund"]);

    let started = Instant::now();
    if let Some(progress) = progress {
        progress.installing_project_deps();
    } else {
        println!(
            "  ▸ installing project dependencies with npm {}",
            if use_ci { "ci" } else { "install" }
        );
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
            "Failed to install project dependencies with npm {}.\n\
Tip: npm ci requires package.json and package-lock.json to be in sync. \
For local debug builds, rerun without --release so LingXia can use npm install.",
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
    use crate::lxapp::project::{Project, ProjectKind};
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

    fn build_options() -> BuildOptions {
        BuildOptions {
            release: false,
            package: false,
            framework: None,
            progress: crate::lxapp::options::ProgressMode::Task,
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
            &build_options(),
            None,
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
            &build_options(),
            None,
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
            &build_options(),
            None,
            None,
        )
        .unwrap();

        assert_eq!(install_duration, None);
    }
}
