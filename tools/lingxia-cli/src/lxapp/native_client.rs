//! The typed native clients in `.lingxia/`, generated from the Rust routes this lxapp calls.
//!
//! A first-party lxapp calls the framework crate its appId names; a host project's lxapp calls
//! the crate `lingxia.yaml` names; an lxapp inside a Cargo package calls that package's routes.

use crate::config::{HOST_CONFIG_FILE, LingXiaConfig};
use crate::lxapp::framework::{ProjectFramework, is_gpu_lxapp};
use crate::lxapp::project::Project;
use anyhow::{Context, Result, bail};
use colored::Colorize;
use lingxia_native_codegen::ViewClient;
use std::fs;
use std::path::{Path, PathBuf};

pub fn generate(project: &Project) -> Result<()> {
    let out = project.root.join(".lingxia");
    let view = view_client(project);
    let Some(src) = route_source(&project.root)? else {
        if let Some(file) = native_caller(&project.root)? {
            bail!(
                "{} calls natives, but no Rust crate serves this lxapp: bundle it from a host \
                 project's `lingxia.yaml`, or keep it inside the crate that defines its routes",
                file.display()
            );
        }
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

/// First-party lxapps and the framework crate serving their routes, relative to the repo root.
/// These win over the walk: a showcase bundles them under its own `lingxia.yaml`.
const FIRST_PARTY_ROUTES: &[(&str, &str)] = &[(
    crate::host_assets::BROWSER_WEBUI.app_id,
    "crates/lingxia-browser-shell",
)];

/// The `src/` of the crate whose routes `lxapp_root` calls. A first-party appId decides first,
/// then the nearest `lingxia.yaml` or `Cargo.toml` above it; a workspace-only manifest means
/// no crate.
pub(crate) fn route_source(lxapp_root: &Path) -> Result<Option<PathBuf>> {
    let lxapp_root = fs::canonicalize(lxapp_root)
        .with_context(|| format!("resolve {}", lxapp_root.display()))?;
    if let Some(src) = first_party_route_source(&lxapp_root)? {
        return Ok(Some(src));
    }
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

fn first_party_route_source(lxapp_root: &Path) -> Result<Option<PathBuf>> {
    let manifest = lxapp_root.join("lxapp.json");
    let Ok(text) = fs::read_to_string(&manifest) else {
        return Ok(None);
    };
    let json: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", manifest.display()))?;
    let Some(app_id) = json.get("appId").and_then(|id| id.as_str()) else {
        return Ok(None);
    };
    let Some((_, crate_dir)) = FIRST_PARTY_ROUTES.iter().find(|(id, _)| *id == app_id) else {
        return Ok(None);
    };
    Ok(lxapp_root
        .ancestors()
        .map(|dir| dir.join(crate_dir))
        .find(|dir| dir.join("Cargo.toml").is_file())
        .map(|dir| dir.join("src"))
        .filter(|src| src.is_dir()))
}

/// The first source file that reaches for a native client, if any.
fn native_caller(root: &Path) -> Result<Option<PathBuf>> {
    const SKIP: [&str; 3] = ["node_modules", "dist", "target"];
    const MARKERS: [&str; 3] = ["page.native", "@lingxia/native", ".lingxia/native"];
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let entries = fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))?;
        for entry in entries {
            let path = entry?.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if path.is_dir() {
                if !name.starts_with('.') && !SKIP.contains(&name) {
                    dirs.push(path);
                }
                continue;
            }
            let source = matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("ts" | "tsx" | "js" | "jsx" | "mjs" | "html")
            );
            if source
                && fs::read_to_string(&path)
                    .is_ok_and(|text| MARKERS.iter().any(|marker| text.contains(marker)))
            {
                return Ok(Some(path));
            }
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
        "#[lingxia::native(\"demo.ping\")]\npub fn ping() -> lingxia::Result<()> { todo!() }\n";

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

    #[test]
    fn a_first_party_lxapp_calls_its_framework_crate_from_inside_a_host_project() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        write(
            root,
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/*\"]\n",
        );
        write(
            root,
            "crates/lingxia-browser-shell/Cargo.toml",
            "[package]\nname = \"lingxia-browser-shell\"\n",
        );
        write(root, "crates/lingxia-browser-shell/src/lib.rs", ROUTE);
        write(
            root,
            "examples/showcase/lingxia.yaml",
            "app:\n  projectName: showcase\n  rustLibDir: native\n",
        );
        write(root, "examples/showcase/native/src/lib.rs", ROUTE);
        let browser = format!(
            "{{\"appId\": \"{}\"}}",
            crate::host_assets::BROWSER_WEBUI.app_id
        );
        write(root, "examples/showcase/browser/lxapp.json", &browser);

        let src = route_source(&root.join("examples/showcase/browser"))
            .unwrap()
            .unwrap();
        assert!(
            src.ends_with("crates/lingxia-browser-shell/src"),
            "{}",
            src.display()
        );
    }

    #[test]
    fn an_lxapp_that_calls_natives_without_a_crate_is_an_error() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        write(root, "Cargo.toml", "[workspace]\n");
        write(root, "chat/lxapp.json", "{\"appId\": \"chat\"}");
        write(root, "chat/node_modules/x/index.ts", "page.native.x()");
        write(root, "chat/pages/home/index.ts", "export const a = 1;\n");
        assert_eq!(native_caller(&root.join("chat")).unwrap(), None);

        write(
            root,
            "chat/pages/home/index.ts",
            "import type { Rename } from '@lingxia/native';\n",
        );
        let file = native_caller(&root.join("chat")).unwrap().unwrap();
        assert!(file.ends_with("pages/home/index.ts"), "{}", file.display());
    }
}
