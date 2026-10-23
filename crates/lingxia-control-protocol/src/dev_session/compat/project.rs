//! Reading a project from disk for [`super`]: its SDK pins, bundled lxapps,
//! and installed `@lingxia/*` packages. The `project` feature; the app
//! runtime never compiles this.

use super::{BUILD_TARGET_FLOOR, Component, LxappCheck, SdkPin};
use std::path::{Path, PathBuf};

/// Every crate this repo publishes, all on the workspace version. A project
/// may depend on any of them, and an upgrade that misses one locks two
/// copies of the runtime.
pub const SDK_CRATES: &[&str] = &[
    "lingxia",
    "lingxia-app-context",
    "lingxia-automation",
    "lingxia-browser",
    "lingxia-browser-shell",
    "lingxia-control-commands",
    "lingxia-control-protocol",
    "lingxia-control-runtime",
    "lingxia-device-io",
    "lingxia-log",
    "lingxia-logic",
    "lingxia-lxapp",
    "lingxia-media",
    "lingxia-messaging",
    "lingxia-native-codegen",
    "lingxia-native-macros",
    "lingxia-network-dev",
    "lingxia-platform",
    "lingxia-provider",
    "lingxia-proxy",
    "lingxia-service",
    "lingxia-settings",
    "lingxia-shell",
    "lingxia-surface",
    "lingxia-terminal",
    "lingxia-terminal-config",
    "lingxia-transfer",
    "lingxia-update",
    "lingxia-webview",
    "lingxia-windows-build",
    "lingxia-windows-contract",
    "lingxia-windows-sdk",
];

/// Directories a project walk never enters: dependencies, build output, and
/// tool state. Platform build trees can hold copies of project manifests.
pub const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "oh_modules",
    "dist",
    "target",
    "build",
    "Pods",
    ".gradle",
    ".lingxia",
    ".git",
    ".claude",
    ".build",
];

/// Every `file_name` under `dir`, at most three directories down, outside
/// [`SKIP_DIRS`].
pub fn walk_named(dir: &Path, file_name: &str, found: &mut Vec<PathBuf>) {
    walk(dir, file_name, 0, found);
}

fn walk(dir: &Path, file_name: &str, depth: usize, found: &mut Vec<PathBuf>) {
    if depth > 3 {
        return;
    }
    let candidate = dir.join(file_name);
    if candidate.is_file() {
        found.push(candidate);
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name();
        if SKIP_DIRS.contains(&name.to_string_lossy().as_ref()) {
            continue;
        }
        walk(&path, file_name, depth + 1, found);
    }
}

/// The `@lingxia/*` packages installed in `root/node_modules`.
pub fn installed_packages(root: &Path) -> Vec<Component> {
    let scope = root.join("node_modules").join("@lingxia");
    let Ok(entries) = std::fs::read_dir(&scope) else {
        return Vec::new();
    };
    let mut packages: Vec<Component> = entries
        .flatten()
        .filter_map(|entry| {
            let text = std::fs::read_to_string(entry.path().join("package.json")).ok()?;
            let manifest: serde_json::Value = serde_json::from_str(&text).ok()?;
            let name = manifest.get("name")?.as_str()?.to_string();
            let version = manifest.get("version")?.as_str()?;
            let commit = manifest
                .get("lingxia")
                .and_then(|lingxia| lingxia.get("commit"))
                .and_then(serde_json::Value::as_str);
            Some(Component::new(name, version).with_commit(commit))
        })
        .collect();
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    packages
}

/// The packages `root` imports. Node resolves from the closest
/// `node_modules`, workspace hoists included, so the nearest copy wins.
/// Deduplicate only within one importing root, never across lxapps.
pub fn resolved_packages(root: &Path) -> Vec<Component> {
    let mut seen = std::collections::HashSet::new();
    root.ancestors()
        .flat_map(installed_packages)
        .filter(|package| seen.insert(package.name.clone()))
        .collect()
}

/// Local lxapp directories a host project bundles via
/// `resources.bundles[].path`. Reads the manifest without validating it, so an
/// otherwise-invalid `lingxia.yaml` still has its lxapps checked here and the
/// config error surfaces later in the build.
pub fn host_bundle_dirs(root: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(root.join("lingxia.yaml")) else {
        return Vec::new();
    };
    let Ok(value) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&text) else {
        return Vec::new();
    };
    let Some(bundles) = value
        .get("resources")
        .and_then(|resources| resources.get("bundles"))
        .and_then(serde_yaml_ng::Value::as_sequence)
    else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = Vec::new();
    for dir in bundles
        .iter()
        .filter_map(|bundle| bundle.get("path").and_then(serde_yaml_ng::Value::as_str))
        .map(|path| root.join(path))
        .filter(|dir| dir.join("lxapp.json").is_file())
    {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs
}

/// SDK requirement pins under `root`: the [`SDK_CRATES`] in any Cargo.toml,
/// a Windows SDK git tag where it has no version, and the Android gradle
/// fallback.
pub fn host_sdk_pins(root: &Path) -> Vec<SdkPin> {
    let mut manifests = Vec::new();
    walk_named(root, "Cargo.toml", &mut manifests);
    manifests.sort();
    let mut pins = Vec::new();
    for manifest in manifests {
        let Ok(content) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        let rel = manifest
            .strip_prefix(root)
            .unwrap_or(manifest.as_path())
            .display()
            .to_string();
        for name in SDK_CRATES {
            if let Some(spec) = cargo_dependency_req(&content, name) {
                pins.push(SdkPin {
                    source: format!("{rel} ({name})"),
                    spec,
                });
            }
        }
        let windows = format!("{rel} (lingxia-windows-sdk)");
        if !pins.iter().any(|pin| pin.source == windows)
            && let Some(spec) = windows_sdk_tag(&content)
        {
            pins.push(SdkPin {
                source: windows,
                spec,
            });
        }
    }
    if let Some(spec) = gradle_sdk_fallback(root) {
        pins.push(SdkPin {
            source: "android/app/build.gradle.kts".to_string(),
            spec,
        });
    }
    pins
}

/// The project that owns `entry`: a host ancestor owns any embedded lxapp
/// below it; otherwise the nearest lxapp directory.
pub fn owning_project_root(entry: &Path) -> PathBuf {
    let start = if entry.is_dir() {
        entry
    } else {
        entry.parent().unwrap_or(entry)
    };
    let mut lxapp = None;
    for dir in start.ancestors() {
        if dir.join("lingxia.yaml").is_file() {
            return dir.to_path_buf();
        }
        if lxapp.is_none() && dir.join("lxapp.json").is_file() {
            lxapp = Some(dir.to_path_buf());
        }
    }
    lxapp.unwrap_or_else(|| start.to_path_buf())
}

/// The window check for a project root, one message per problem: a host's
/// pins and each bundled lxapp, or a standalone lxapp. `tool` is the checking
/// tool's own line.
pub fn project_errors(root: &Path, tool: (u64, u64)) -> Vec<String> {
    let floor = BUILD_TARGET_FLOOR;
    let mut errors = Vec::new();
    if root.join("lingxia.yaml").is_file() {
        let pins = host_sdk_pins(root);
        let host_line = match super::agreed_host_line(&pins) {
            Ok(Some((line, shown))) => {
                if let Err(skew) = super::check_target_line(floor, tool, line, "host SDK", shown) {
                    errors.push(skew.to_string());
                }
                Some(line)
            }
            Ok(None) => None,
            Err(skew) => {
                errors.push(skew.to_string());
                None
            }
        };
        for dir in host_bundle_dirs(root) {
            errors.extend(lxapp_error(floor, tool, host_line, &dir));
        }
    } else if root.join("lxapp.json").is_file() {
        errors.extend(lxapp_error(floor, tool, None, root));
    }
    errors
}

/// `minRuntime` from `dir/lxapp.json`, or the one-line reason it has none.
pub fn read_min_runtime(dir: &Path) -> Result<String, String> {
    let name = dir.display();
    let text = std::fs::read_to_string(dir.join("lxapp.json"))
        .map_err(|error| format!("{name}: cannot read lxapp.json: {error}"))?;
    let manifest: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| format!("{name}: lxapp.json is not valid JSON: {error}"))?;
    manifest
        .get("minRuntime")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{name}: lxapp.json minRuntime is required; run `lingxia upgrade`"))
}

fn lxapp_error(
    floor: (u64, u64),
    tool: (u64, u64),
    host_line: Option<(u64, u64)>,
    dir: &Path,
) -> Option<String> {
    let min_runtime = match read_min_runtime(dir) {
        Ok(min_runtime) => min_runtime,
        Err(message) => return Some(message),
    };
    let name = dir.display().to_string();
    let packages = resolved_packages(dir);
    super::check_lxapp(
        floor,
        tool,
        host_line,
        &LxappCheck {
            name: &name,
            min_runtime: &min_runtime,
            packages: &packages,
        },
    )
    .err()
    .map(|skew| skew.to_string())
}

/// The requirement a Cargo.toml gives `crate_name` in any dependency table:
/// `name = "req"`, `name = { version = "req", … }`, or a
/// `[dependencies.name]` table. `None` for path- or git-only dependencies.
pub fn cargo_dependency_req(content: &str, crate_name: &str) -> Option<String> {
    let prefix = format!("{crate_name} =");
    let prefix_nospace = format!("{crate_name}=");
    let mut in_dependencies = false;
    let mut in_crate_table = false;
    for line in content.lines() {
        if let Some(table) = cargo_table_name(line) {
            in_dependencies = is_cargo_dependency_table(table);
            in_crate_table = is_cargo_crate_table(table, crate_name);
            continue;
        }
        let trimmed = line.trim_start();
        if !((in_dependencies
            && (trimmed.starts_with(&prefix) || trimmed.starts_with(&prefix_nospace)))
            || (in_crate_table
                && (trimmed.starts_with("version =") || trimmed.starts_with("version="))))
        {
            continue;
        }
        let markers = if in_crate_table || trimmed.contains('{') {
            ["version =", "version="]
        } else {
            [prefix.as_str(), prefix_nospace.as_str()]
        };
        if let Some(spec) = markers.iter().find_map(|marker| quoted_after(line, marker)) {
            return Some(spec.to_string());
        }
    }
    None
}

/// The version in a `lingxia-windows-sdk` git pin's
/// `tag = "lingxia-crates-v<version>"`.
pub fn windows_sdk_tag(content: &str) -> Option<String> {
    let mut in_dependencies = false;
    for line in content.lines() {
        if let Some(table) = cargo_table_name(line) {
            in_dependencies = is_cargo_dependency_table(table);
            continue;
        }
        if !in_dependencies || !line.trim_start().starts_with("lingxia-windows-sdk") {
            continue;
        }
        for marker in ["tag = \"lingxia-crates-v", "tag=\"lingxia-crates-v"] {
            if let Some(index) = line.find(marker) {
                let rest = &line[index + marker.len()..];
                let end = rest.find('"')?;
                return Some(rest[..end].to_string());
            }
        }
    }
    None
}

/// The scaffolded `?: "<version>"` fallback on the `lingxia.sdkVersion`
/// gradle property.
pub fn gradle_sdk_fallback(root: &Path) -> Option<String> {
    let content = std::fs::read_to_string(root.join("android/app/build.gradle.kts")).ok()?;
    content
        .lines()
        .filter(|line| line.contains("lingxia.sdkVersion"))
        .find_map(|line| quoted_after(line, "?:"))
        .map(str::to_string)
}

/// The quoted string right after `marker` (whitespace between allowed).
fn quoted_after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    let rest = line.get(line.find(marker)? + marker.len()..)?.trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(&rest[..rest.find('"')?])
}

/// `[name]` / `[[name]]` header on `line`, trimmed.
pub fn cargo_table_name(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("[[") {
        let end = rest.find("]]")?;
        return Some(rest[..end].trim());
    }
    let rest = &trimmed[1..];
    let end = rest.find(']')?;
    Some(rest[..end].trim())
}

pub fn is_cargo_dependency_table(table: &str) -> bool {
    matches!(
        table,
        "dependencies" | "dev-dependencies" | "build-dependencies" | "workspace.dependencies"
    ) || (table.starts_with("target.")
        && (table.ends_with(".dependencies")
            || table.ends_with(".dev-dependencies")
            || table.ends_with(".build-dependencies")))
}

/// `[dependencies.<crate_name>]` and its target/dev/build variants.
pub fn is_cargo_crate_table(table: &str, crate_name: &str) -> bool {
    table.rsplit_once('.').is_some_and(|(parent, name)| {
        is_cargo_dependency_table(parent) && name.trim_matches('"') == crate_name
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_crates_are_the_workspace_crates() {
        let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut names: Vec<String> = std::fs::read_dir(&crates_dir)
            .unwrap()
            .filter_map(|entry| {
                let manifest =
                    std::fs::read_to_string(entry.ok()?.path().join("Cargo.toml")).ok()?;
                let line = manifest.lines().find(|line| line.starts_with("name"))?;
                Some(line.split('"').nth(1)?.to_string())
            })
            .collect();
        names.sort();
        let mut listed: Vec<&str> = SDK_CRATES.to_vec();
        listed.sort();
        assert_eq!(names, listed);
    }

    #[test]
    fn host_pins_are_read_from_cargo_and_gradle() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        std::fs::create_dir_all(dir.join("native")).unwrap();
        std::fs::create_dir_all(dir.join("android/app/build")).unwrap();
        std::fs::write(
            dir.join("native/Cargo.toml"),
            "[dependencies]\nlingxia = { version = \"~0.25.2\", features = [\"standard\"] }\n\
             lingxia-device-io =\"~0.25.2\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("android/app/build.gradle.kts"),
            "val lingxiaSdkVersion = (findProperty(\"lingxia.sdkVersion\") as String?) ?: \"0.25.0\"\n",
        )
        .unwrap();
        // Build trees are skipped.
        for skipped in ["target", "android/app/build"] {
            std::fs::create_dir_all(dir.join(skipped)).unwrap();
            std::fs::write(
                dir.join(skipped).join("Cargo.toml"),
                "[dependencies]\nlingxia = \"9.9.9\"\n",
            )
            .unwrap();
        }
        let pins = host_sdk_pins(dir);
        assert_eq!(pins.len(), 3, "{pins:?}");
        assert_eq!(
            super::super::agreed_host_line(&pins)
                .unwrap()
                .map(|(l, _)| l),
            Some((0, 25))
        );
    }

    #[test]
    fn a_windows_git_pin_reads_its_tag() {
        let content = "[dependencies]\nlingxia-windows-sdk = { git = \"https://x\", tag = \"lingxia-crates-v0.25.1\" }\n";
        assert_eq!(cargo_dependency_req(content, "lingxia-windows-sdk"), None);
        assert_eq!(windows_sdk_tag(content).as_deref(), Some("0.25.1"));
        let table = "[target.'cfg(windows)'.dependencies.lingxia]\nversion = \"0.25.0\"\n";
        assert_eq!(
            cargo_dependency_req(table, "lingxia").as_deref(),
            Some("0.25.0")
        );
    }

    #[test]
    fn host_bundle_dirs_read_without_validation() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        std::fs::create_dir_all(dir.join("lxapp")).unwrap();
        // No `surfaces:` and other required host fields: config validation
        // would reject this manifest, but the bundle paths still read.
        std::fs::write(
            dir.join("lingxia.yaml"),
            "app:\n  projectName: demo\nresources:\n  bundles:\n    - type: lxapp\n      appId: demo\n      path: lxapp\n    - type: lxapp\n      appId: pkg\n      package: '@demo/pkg'\n",
        )
        .unwrap();
        std::fs::write(dir.join("lxapp/lxapp.json"), "{}").unwrap();
        assert_eq!(host_bundle_dirs(dir), vec![dir.join("lxapp")]);
    }

    #[test]
    fn installed_packages_are_read_from_node_modules() {
        let dir = tempfile::tempdir().unwrap();
        let scope = dir.path().join("node_modules/@lingxia");
        for (name, manifest) in [
            ("test", r#"{"name":"@lingxia/test","version":"0.25.0"}"#),
            (
                "react",
                r#"{"name":"@lingxia/react","version":"0.25.2","lingxia":{"commit":"abcdef1"}}"#,
            ),
            ("broken", "not json"),
        ] {
            std::fs::create_dir_all(scope.join(name)).unwrap();
            std::fs::write(scope.join(name).join("package.json"), manifest).unwrap();
        }
        let packages = installed_packages(dir.path());
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "@lingxia/react");
        assert_eq!(packages[0].commit.as_deref(), Some("abcdef1"));
        assert!(installed_packages(Path::new("/nonexistent")).is_empty());
    }

    #[test]
    fn a_broken_manifest_says_what_is_wrong() {
        let dir = tempfile::tempdir().unwrap();
        let err = read_min_runtime(dir.path()).unwrap_err();
        assert!(err.contains("cannot read lxapp.json"), "{err}");
        std::fs::write(dir.path().join("lxapp.json"), "{").unwrap();
        let err = read_min_runtime(dir.path()).unwrap_err();
        assert!(err.contains("not valid JSON"), "{err}");
        std::fs::write(dir.path().join("lxapp.json"), "{}").unwrap();
        let err = read_min_runtime(dir.path()).unwrap_err();
        assert!(err.contains("minRuntime is required"), "{err}");
        std::fs::write(dir.path().join("lxapp.json"), r#"{"minRuntime":"0.25.0"}"#).unwrap();
        assert_eq!(read_min_runtime(dir.path()).unwrap(), "0.25.0");
    }

    #[test]
    fn an_embedded_lxapp_belongs_to_its_host() {
        let dir = tempfile::tempdir().unwrap();
        let lxapp = dir.path().join("lxapp/pages");
        std::fs::create_dir_all(&lxapp).unwrap();
        std::fs::write(dir.path().join("lxapp/lxapp.json"), "{}").unwrap();
        assert_eq!(owning_project_root(&lxapp), dir.path().join("lxapp"));
        std::fs::write(dir.path().join("lingxia.yaml"), "").unwrap();
        assert_eq!(owning_project_root(&lxapp), dir.path());
    }
}
