//! The typed native clients in `.lingxia/`, generated from the Rust routes this lxapp calls.
//!
//! A host project's lxapp calls the crate `lingxia.yaml` names; an lxapp inside a Cargo package
//! calls that package's routes. Either way the client also carries every library crate in that
//! crate's dependency graph that declares `lingxia::routes!()`, found offline from its lock file
//! for every platform `lingxia.yaml` enables. A route crate `lingxia` itself installs serves only
//! the system lxapp that names it, and a client leaves out routes the lxapp's class may not call.

use crate::config::{HOST_CONFIG_FILE, LingXiaConfig};
use crate::host_assets::SYSTEM_LXAPPS;
use crate::lxapp::framework::{ProjectFramework, is_gpu_lxapp};
use crate::lxapp::project::Project;
use anyhow::{Context, Result, bail};
use colored::Colorize;
use lingxia_native_codegen::{CallerClass, ViewClient};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The crates whose routes one lxapp's client carries.
#[derive(Debug)]
pub(crate) struct RouteSources {
    /// The lxapp's own crate's `src/` first, then each library route crate's.
    pub(crate) crates: Vec<RouteCrate>,
    /// Library route crates the host's `host!` does not list: the host installs none of their
    /// routes, so the client leaves them out.
    pub(crate) unlisted: Vec<String>,
    /// The platforms whose dependency graph could not be resolved, and why.
    pub(crate) skipped: Vec<String>,
    pub(crate) caller: CallerClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RouteCrate {
    pub(crate) src: PathBuf,
    /// Source a developer edits, not a registry or git checkout.
    pub(crate) local: bool,
}

pub fn generate(project: &Project) -> Result<()> {
    let out = project.root.join(".lingxia");
    let view = view_client(project);
    let Some(sources) = route_sources(&project.root)? else {
        if let Some(file) = native_caller(&project.root)? {
            bail!(
                "{} calls natives, but no Rust crate serves this lxapp: bundle it from a host \
                 project's `lingxia.yaml`, or keep it inside the crate that defines its routes",
                file.display()
            );
        }
        return clear(&out);
    };
    for reason in &sources.skipped {
        eprintln!(
            "{} the native client may miss route crates: {reason}",
            "warning:".yellow()
        );
    }
    for name in &sources.unlisted {
        eprintln!(
            "{} {name} has routes but host! does not list it; add it to `lingxia::host!(…, routes = [..])`",
            "warning:".yellow()
        );
    }
    let srcs: Vec<PathBuf> = sources.crates.iter().map(|c| c.src.clone()).collect();
    let warnings = lingxia_native_codegen::generate_clients(&srcs, &out, view, sources.caller)
        .with_context(|| format!("native client from {}", srcs[0].display()))?;
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

/// The crates whose routes `lxapp_root` calls: the nearest `lingxia.yaml`'s crate, when it
/// bundles this lxapp, or the nearest package; a workspace-only manifest means none.
pub(crate) fn route_sources(lxapp_root: &Path) -> Result<Option<RouteSources>> {
    let lxapp_root = fs::canonicalize(lxapp_root)
        .with_context(|| format!("resolve {}", lxapp_root.display()))?;
    let mut own = None;
    for dir in lxapp_root.ancestors() {
        if dir.join(HOST_CONFIG_FILE).is_file() {
            own = host_route_source(dir, &lxapp_root)?;
            break;
        }
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() {
            let text = fs::read_to_string(&manifest)
                .with_context(|| format!("read {}", manifest.display()))?;
            let is_package = text.lines().any(|line| line.trim() == "[package]");
            own = is_package
                .then(|| dir.join("src"))
                .filter(|src| src.is_dir())
                .map(|src| HostCrate {
                    src,
                    platforms: Vec::new(),
                    framework_routes: None,
                    caller: package_caller(&lxapp_root),
                });
            break;
        }
    }
    let Some(HostCrate {
        src,
        platforms,
        framework_routes,
        caller,
    }) = own
    else {
        return Ok(None);
    };
    let mut sources = RouteSources {
        crates: vec![RouteCrate {
            src: src.clone(),
            local: true,
        }],
        unlisted: Vec::new(),
        skipped: Vec::new(),
        caller,
    };
    let crate_dir = src.parent().unwrap_or(&src);
    // A package outside a host project has no platforms of its own: this machine's stands in.
    let targets: Vec<(String, &str, Vec<String>)> = if platforms.is_empty() {
        vec![("this machine".to_string(), "host-tuple", Vec::new())]
    } else {
        platforms
            .into_iter()
            .filter_map(|(platform, features)| {
                platform_target(&platform).map(|target| (platform, target, features))
            })
            .collect()
    };
    let resolved: Vec<(String, Result<Vec<LibraryRoutes>>)> = std::thread::scope(|scope| {
        let runs: Vec<_> = targets
            .iter()
            .map(|(platform, target, features)| {
                let run = scope.spawn(move || library_route_crates(crate_dir, target, features));
                (platform.clone(), run)
            })
            .collect();
        runs.into_iter()
            .map(|(platform, run)| {
                let result = run
                    .join()
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("cargo metadata panicked")));
                (platform, result)
            })
            .collect()
    });
    let mut libraries: Vec<LibraryRoutes> = Vec::new();
    for (platform, result) in resolved {
        match result {
            Ok(found) => {
                for library in found {
                    if !libraries
                        .iter()
                        .any(|known| known.package == library.package)
                    {
                        libraries.push(library);
                    }
                }
            }
            Err(err) => sources.skipped.push(format!("{platform}: {err:#}")),
        }
    }
    libraries.sort_by(|a, b| a.package.cmp(&b.package));
    let listed = lingxia_native_codegen::host_route_crates(&src.join("lib.rs"))
        .ok()
        .flatten();
    for library in libraries {
        if library.framework && framework_routes != Some(library.package.as_str()) {
            continue;
        }
        if let Some(listed) = &listed
            && !library.framework
            && !listed.contains(&library.lib_name)
        {
            sources.unlisted.push(library.package);
            continue;
        }
        sources.crates.push(library.source);
    }
    Ok(Some(sources))
}

/// The target a platform's host library is built for. Only its OS decides which crates resolve.
fn platform_target(platform: &str) -> Option<&'static str> {
    Some(match platform {
        "android" => "aarch64-linux-android",
        "ios" => "aarch64-apple-ios",
        "macos" => "aarch64-apple-darwin",
        "harmony" => "aarch64-unknown-linux-ohos",
        "windows" => "x86_64-pc-windows-msvc",
        _ => return None,
    })
}

/// An lxapp inside a package has no host config: a system lxapp's appId still makes it a surface.
fn package_caller(lxapp_root: &Path) -> CallerClass {
    let app_id = fs::read_to_string(lxapp_root.join("lxapp.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|manifest| manifest["appId"].as_str().map(str::to_string));
    match app_id.as_deref().and_then(crate::host_assets::system_lxapp) {
        Some(_) => CallerClass::ControlSurface,
        None => CallerClass::StandardApp,
    }
}

struct HostCrate {
    src: PathBuf,
    /// Each platform `lingxia.yaml` enables, with the cargo features it turns on there.
    platforms: Vec<(String, Vec<String>)>,
    /// The framework route crate the lxapp calls, when it is a system lxapp.
    framework_routes: Option<&'static str>,
    caller: CallerClass,
}

struct LibraryRoutes {
    package: String,
    lib_name: String,
    source: RouteCrate,
    /// Reached through `lingxia` itself, which installs it; a host does not list it.
    framework: bool,
}

/// Every crate in `crate_dir`'s normal dependency graph for `target` whose root declares
/// `lingxia::routes!()`. `cargo metadata --offline` reads the lock file; no build runs, and a
/// target whose crates were never fetched fails rather than resolving short.
fn library_route_crates(
    crate_dir: &Path,
    target: &str,
    features: &[String],
) -> Result<Vec<LibraryRoutes>> {
    let manifest = crate_dir.join("Cargo.toml");
    let declared = declared_features(&manifest)?;
    let features: Vec<&str> = features
        .iter()
        .map(String::as_str)
        .filter(|feature| declared.contains(*feature))
        .collect();
    let mut command = Command::new("cargo");
    command
        .args([
            "metadata",
            "--format-version",
            "1",
            "--offline",
            "--filter-platform",
            target,
            "--manifest-path",
        ])
        .arg(&manifest);
    if !features.is_empty() {
        command.arg("--features").arg(features.join(","));
    }
    let output = command.output().context("run cargo metadata")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "cargo metadata --offline failed: {}",
            stderr
                .lines()
                .find(|line| line.starts_with("error"))
                .unwrap_or("")
                .trim_start_matches("error: ")
        );
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("parse cargo metadata")?;
    let packages = metadata["packages"].as_array().cloned().unwrap_or_default();
    let manifest = fs::canonicalize(&manifest).unwrap_or(manifest);
    let Some(root) = packages.iter().find(|package| {
        package["manifest_path"]
            .as_str()
            .and_then(|path| fs::canonicalize(path).ok())
            .is_some_and(|path| path == manifest)
    }) else {
        bail!("cargo metadata does not list {}", manifest.display());
    };
    let mut deps: HashMap<&str, Vec<&str>> = HashMap::new();
    for node in metadata["resolve"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let normal = node["deps"].as_array().into_iter().flatten().filter(|dep| {
            dep["dep_kinds"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind["kind"].is_null()))
        });
        deps.insert(
            node["id"].as_str().unwrap_or_default(),
            normal.filter_map(|dep| dep["pkg"].as_str()).collect(),
        );
    }
    let reach = |from: &str| {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([from]);
        while let Some(id) = queue.pop_front() {
            for dep in deps.get(id).into_iter().flatten() {
                if seen.insert(*dep) {
                    queue.push_back(dep);
                }
            }
        }
        seen
    };
    let root_id = root["id"].as_str().unwrap_or_default();
    let reachable = reach(root_id);
    let framework = packages
        .iter()
        .find(|package| package["name"] == "lingxia")
        .and_then(|package| package["id"].as_str())
        .map(reach)
        .unwrap_or_default();
    let mut found = Vec::new();
    for package in &packages {
        let id = package["id"].as_str().unwrap_or_default();
        if id == root_id || !reachable.contains(id) {
            continue;
        }
        let uses_routes = package["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|dep| dep["name"] == "lingxia" || dep["name"] == "lingxia-native-macros");
        let lib = package["targets"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|target| {
                target["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind == "lib" || kind == "rlib"))
            });
        let (true, Some(lib)) = (uses_routes, lib) else {
            continue;
        };
        let Some(lib_rs) = lib["src_path"].as_str().map(PathBuf::from) else {
            continue;
        };
        if !lingxia_native_codegen::declares_routes(&lib_rs)? {
            continue;
        }
        let Some(src) = lib_rs.parent() else { continue };
        found.push(LibraryRoutes {
            package: package["name"].as_str().unwrap_or_default().to_string(),
            lib_name: lib["name"].as_str().unwrap_or_default().replace('-', "_"),
            source: RouteCrate {
                src: src.to_path_buf(),
                local: package["source"].is_null(),
            },
            framework: framework.contains(id),
        });
    }
    Ok(found)
}

/// The `[features]` a manifest declares; `--features` naming another fails the whole call.
fn declared_features(manifest: &Path) -> Result<BTreeSet<String>> {
    let text =
        fs::read_to_string(manifest).with_context(|| format!("read {}", manifest.display()))?;
    let table: toml::Table =
        toml::from_str(&text).with_context(|| format!("parse {}", manifest.display()))?;
    Ok(table
        .get("features")
        .and_then(toml::Value::as_table)
        .map(|features| features.keys().cloned().collect())
        .unwrap_or_default())
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

/// The host crate, when the host ships this lxapp: a bundle, or a system lxapp.
fn host_route_source(project_root: &Path, lxapp_root: &Path) -> Result<Option<HostCrate>> {
    let config = LingXiaConfig::load(project_root)?;
    let is_this =
        |path: &str| fs::canonicalize(project_root.join(path)).is_ok_and(|p| p == lxapp_root);
    let system = SYSTEM_LXAPPS
        .into_iter()
        .find(|system| system.configured_path(&config).is_some_and(is_this));
    let bundle_id = config
        .resources
        .iter()
        .flat_map(|resources| &resources.bundles)
        .find(|bundle| {
            bundle
                .path
                .as_deref()
                .map(str::trim)
                .is_some_and(|path| !path.is_empty() && is_this(path))
        })
        .map(|bundle| bundle.app_id.as_str());
    let bundled = system.is_some() || bundle_id.is_some();
    let src = config
        .get_rust_lib_name()
        .map(|crate_dir| project_root.join(crate_dir).join("src"))
        .filter(|src| src.is_dir());
    let platforms = config
        .app
        .iter()
        .flat_map(|app| &app.platforms)
        .map(|platform| {
            let features = config.native_features_for_platform(platform);
            (platform.clone(), features)
        })
        .collect();
    let home = config
        .app
        .as_ref()
        .and_then(|app| app.home_app_id.as_deref());
    let caller = if system.is_some() {
        CallerClass::ControlSurface
    } else if home.is_some_and(|home| bundle_id == Some(home)) {
        CallerClass::ControlApp
    } else {
        CallerClass::StandardApp
    };
    Ok(src.filter(|_| bundled).map(|src| HostCrate {
        src,
        platforms,
        framework_routes: system.and_then(|system| system.route_crate),
        caller,
    }))
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

    fn own_src(lxapp: &Path) -> Option<PathBuf> {
        route_sources(lxapp)
            .unwrap()
            .map(|sources| sources.crates[0].src.clone())
    }

    fn package(root: &Path, dir: &str, name: &str, deps: &[&str], lib: &str) {
        let deps: String = deps
            .iter()
            .map(|dep| format!("{dep} = {{ path = \"../{dep}\" }}\n"))
            .collect();
        write(
            root,
            &format!("{dir}/Cargo.toml"),
            &format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{deps}"
            ),
        );
        write(root, &format!("{dir}/src/lib.rs"), lib);
    }

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

        let src = own_src(&root.join("app")).unwrap();
        assert!(src.ends_with("native/src"), "{}", src.display());
        assert_eq!(own_src(&root.join("other")), None);
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

        let src = own_src(&root.join("shell/webui")).unwrap();
        assert!(src.ends_with("shell/src"), "{}", src.display());
        assert_eq!(own_src(&root.join("examples/chat")), None);
    }

    #[test]
    fn library_route_crates_join_the_client_and_framework_ones_serve_the_system_lxapp_naming_them()
    {
        let temp = tempdir().unwrap();
        let root = temp.path();
        write(
            root,
            HOST_CONFIG_FILE,
            concat!(
                "app:\n  projectName: demo\n  homeAppId: demo\n  rustLibDir: native\n  packageId: com.example.demo\n",
                "  productName: Demo\n  productVersion: 0.0.1\n  platforms: [android, ios]\n",
                "browser:\n  webui:\n    path: browser\n",
                "resources:\n  bundles:\n    - type: lxapp\n      appId: demo\n      path: app\n",
            ),
        );
        let routes = "lingxia::routes!();\n";
        package(
            root,
            "lingxia",
            "lingxia",
            &["lingxia-browser-shell", "term"],
            "",
        );
        package(
            root,
            "lingxia-browser-shell",
            "lingxia-browser-shell",
            &["lingxia-native-macros"],
            routes,
        );
        package(root, "term", "term", &["lingxia-native-macros"], routes);
        package(
            root,
            "lingxia-native-macros",
            "lingxia-native-macros",
            &[],
            "",
        );
        package(root, "notes-core", "notes-core", &["lingxia"], routes);
        package(root, "plain", "plain", &["lingxia"], "");
        let host = |listed: &str| {
            let lib = format!("lingxia::host!(App{listed});\n{ROUTE}");
            package(
                root,
                "native",
                "demo",
                &["lingxia", "notes-core", "plain"],
                &lib,
            );
            // Linked on iOS only: the union over enabled platforms still finds it.
            let manifest = root.join("native/Cargo.toml");
            let mut text = fs::read_to_string(&manifest).unwrap();
            text.push_str(
                "\n[target.'cfg(target_os = \"ios\")'.dependencies]\nios-notes = { path = \"../ios-notes\" }\n",
            );
            fs::write(manifest, text).unwrap();
        };
        package(root, "ios-notes", "ios-notes", &["lingxia"], routes);
        fs::create_dir_all(root.join("app")).unwrap();
        fs::create_dir_all(root.join("browser")).unwrap();
        let crates = |lxapp: &str| -> (Vec<String>, Vec<String>) {
            let sources = route_sources(&root.join(lxapp)).unwrap().unwrap();
            assert!(sources.skipped.is_empty(), "{:?}", sources.skipped);
            let names = sources.crates.iter().map(|c| {
                let dir = c.src.parent().unwrap().file_name().unwrap();
                dir.to_string_lossy().into_owned()
            });
            (names.collect(), sources.unlisted)
        };

        host(", routes = [notes_core, ios_notes]");
        let class = |lxapp: &str| route_sources(&root.join(lxapp)).unwrap().unwrap().caller;
        assert_eq!(class("app"), CallerClass::ControlApp);
        assert_eq!(class("browser"), CallerClass::ControlSurface);
        assert_eq!(
            crates("app"),
            (
                vec!["native".into(), "ios-notes".into(), "notes-core".into()],
                vec![]
            )
        );
        assert_eq!(
            crates("browser"),
            (
                vec![
                    "native".into(),
                    "ios-notes".into(),
                    "lingxia-browser-shell".into(),
                    "notes-core".into()
                ],
                vec![]
            )
        );

        host("");
        assert_eq!(
            crates("app"),
            (
                vec!["native".into()],
                vec!["ios-notes".into(), "notes-core".into()]
            )
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
