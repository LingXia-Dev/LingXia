//! The typed native clients in `.lingxia/`, generated from the Rust routes this lxapp calls.
//!
//! A host project's lxapp calls the crate `lingxia.yaml` names; an lxapp inside a Cargo
//! package (a framework crate's own pages) calls that package's routes.

use crate::config::{HOST_CONFIG_FILE, LingXiaConfig};
use crate::lxapp::framework::{ProjectFramework, is_gpu_lxapp};
use crate::lxapp::project::Project;
use anyhow::{Context, Result};
use colored::Colorize;
use lingxia_native_codegen::ViewClient;
use std::fs;
use std::path::{Path, PathBuf};

pub fn generate(project: &Project) -> Result<()> {
    let out = project.root.join(".lingxia");
    let view = view_client(project);
    let Some(src) = route_source(&project.root)? else {
        return clear(&out);
    };
    let warnings = lingxia_native_codegen::generate_clients(&src, &out, view)
        .with_context(|| format!("native client from {}", src.display()))?;
    for warning in warnings {
        eprintln!("{} {warning}", "warning:".yellow());
    }
    Ok(())
}

/// The WebView client the pages import; a GPU View reaches natives only through its Logic.
fn view_client(project: &Project) -> Option<ViewClient> {
    if is_gpu_lxapp(&project.root) {
        return None;
    }
    Some(match project.framework {
        ProjectFramework::Html => ViewClient::BrowserGlobal,
        ProjectFramework::React => ViewClient::TypeScript,
    })
}

fn clear(out: &Path) -> Result<()> {
    for name in [
        lingxia_native_codegen::LOGIC_CLIENT,
        ViewClient::TypeScript.file_name(),
        ViewClient::BrowserGlobal.file_name(),
    ] {
        match fs::remove_file(out.join(name)) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                return Err(err).with_context(|| format!("remove {}", out.join(name).display()));
            }
            _ => {}
        }
    }
    Ok(())
}

/// The `src/` of the crate whose routes `lxapp_root` calls. The nearest `lingxia.yaml` or
/// `Cargo.toml` above it decides; a workspace-only manifest means no crate.
pub(crate) fn route_source(lxapp_root: &Path) -> Result<Option<PathBuf>> {
    let lxapp_root = fs::canonicalize(lxapp_root)
        .with_context(|| format!("resolve {}", lxapp_root.display()))?;
    for dir in lxapp_root.ancestors() {
        if dir.join(HOST_CONFIG_FILE).is_file() {
            return host_route_source(dir, &lxapp_root);
        }
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() {
            let text = fs::read_to_string(&manifest)
                .with_context(|| format!("read {}", manifest.display()))?;
            let is_package = text.lines().any(|line| line.trim() == "[package]");
            return Ok(is_package
                .then(|| dir.join("src"))
                .filter(|src| src.is_dir()));
        }
    }
    Ok(None)
}

fn host_route_source(project_root: &Path, lxapp_root: &Path) -> Result<Option<PathBuf>> {
    let config = LingXiaConfig::load(project_root)?;
    let bundled = config
        .resources
        .iter()
        .flat_map(|resources| &resources.bundles)
        .filter_map(|bundle| bundle.path.as_deref().map(str::trim))
        .filter(|path| !path.is_empty())
        .any(|path| fs::canonicalize(project_root.join(path)).is_ok_and(|p| p == lxapp_root));
    let src = config
        .get_rust_lib_name()
        .map(|crate_dir| project_root.join(crate_dir).join("src"))
        .filter(|src| src.is_dir());
    Ok(src.filter(|_| bundled))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    const ROUTE: &str =
        "#[lingxia::native(\"demo.ping\")]\npub fn ping() -> HostResult<()> { todo!() }\n";

    #[test]
    fn a_bundled_host_lxapp_calls_the_configured_crate() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        write(
            root,
            HOST_CONFIG_FILE,
            concat!(
                "app:\n  projectName: demo\n  homeAppId: demo\n  rustLibDir: native\n  packageId: com.example.demo\n",
                "  productName: Demo\n  productVersion: 0.0.1\n  platforms: [android]\n",
                "resources:\n  bundles:\n    - type: lxapp\n      appId: demo\n      path: app\n",
            ),
        );
        write(root, "native/src/lib.rs", ROUTE);
        fs::create_dir_all(root.join("app")).unwrap();
        fs::create_dir_all(root.join("other")).unwrap();

        let src = route_source(&root.join("app")).unwrap().unwrap();
        assert!(src.ends_with("native/src"), "{}", src.display());
        assert_eq!(route_source(&root.join("other")).unwrap(), None);
    }

    #[test]
    fn an_lxapp_inside_a_package_calls_that_package_and_a_workspace_means_none() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        write(root, "Cargo.toml", "[workspace]\nmembers = [\"shell\"]\n");
        write(root, "shell/Cargo.toml", "[package]\nname = \"shell\"\n");
        write(root, "shell/src/lib.rs", ROUTE);
        fs::create_dir_all(root.join("shell/webui")).unwrap();
        fs::create_dir_all(root.join("examples/chat")).unwrap();

        let src = route_source(&root.join("shell/webui")).unwrap().unwrap();
        assert!(src.ends_with("shell/src"), "{}", src.display());
        assert_eq!(route_source(&root.join("examples/chat")).unwrap(), None);
    }
}
