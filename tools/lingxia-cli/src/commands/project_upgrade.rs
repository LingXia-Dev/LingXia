//! `lingxia upgrade` inside a project: bump every project-pinned LingXia
//! version to this CLI's compatibility line.
//!
//! Platform SDK binaries (Apple/Android/Harmony) already follow the CLI at
//! build time; what goes stale in a checkout are the pins the project owns:
//! `@lingxia/*` npm ranges, the `lingxia` crate requirement in
//! `native/Cargo.toml`, the `lingxia-windows-sdk` git ref, and the gradle
//! `lingxia.sdkVersion` fallback. This rewrites exactly those, string-level,
//! so file formatting and every other line stay untouched.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

/// Exit code for `--check` when the project is behind (mirrors the CLI
/// self-upgrade `--check` convention).
const EXIT_UPGRADE_AVAILABLE: i32 = 10;

/// Directories never walked when looking for embedded package.json files.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "target",
    ".lingxia",
    ".git",
    ".claude",
    ".build",
];

/// The project root for upgrade purposes: the nearest ancestor holding a
/// `lingxia.yaml` (host app) or `lxapp.json` (standalone lxapp).
pub fn find_project_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join(crate::config::HOST_CONFIG_FILE).exists() || dir.join("lxapp.json").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// One planned file rewrite, with human-readable change lines.
struct Edit {
    path: PathBuf,
    new_content: String,
    changes: Vec<String>,
    /// Re-run `npm install` in this file's directory after writing.
    refresh_npm: bool,
}

pub fn execute(root: &Path, check: bool) -> Result<i32> {
    let edits = plan(root)?;

    println!("{}", "LingXia project".bold());
    println!("  root       {}", root.display());
    println!(
        "  CLI line   npm {} / crate {}",
        crate::versions::npm_compat_range(),
        crate::versions::cargo_compat_req()
    );

    if edits.is_empty() {
        println!("  {}", "up to date".green());
        return Ok(0);
    }

    println!();
    for edit in &edits {
        println!(
            "  {}",
            edit.path.strip_prefix(root).unwrap_or(&edit.path).display()
        );
        for change in &edit.changes {
            println!("    {change}");
        }
    }

    if check {
        println!();
        println!(
            "{} The project is behind this CLI; run `lingxia upgrade` to apply.",
            "!".yellow()
        );
        return Ok(EXIT_UPGRADE_AVAILABLE);
    }

    let mut npm_dirs = Vec::new();
    for edit in &edits {
        fs::write(&edit.path, &edit.new_content)
            .with_context(|| format!("write {}", edit.path.display()))?;
        if edit.refresh_npm
            && let Some(dir) = edit.path.parent()
        {
            npm_dirs.push(dir.to_path_buf());
        }
    }
    println!();
    println!("{} Project pins updated.", "✓".green());

    for dir in npm_dirs {
        println!("  refreshing lockfile in {} …", dir.display());
        let status = std::process::Command::new(crate::npm::command())
            .arg("install")
            .current_dir(&dir)
            .status();
        match status {
            Ok(status) if status.success() => {}
            _ => println!(
                "  {} `npm install` did not complete in {}; run it manually.",
                "!".yellow(),
                dir.display()
            ),
        }
    }
    Ok(0)
}

fn plan(root: &Path) -> Result<Vec<Edit>> {
    let mut edits = Vec::new();

    for package_json in find_package_jsons(root) {
        let content = fs::read_to_string(&package_json)
            .with_context(|| format!("read {}", package_json.display()))?;
        let (new_content, changes) =
            rewrite_npm_ranges(&content, &crate::versions::npm_compat_range());
        if !changes.is_empty() {
            edits.push(Edit {
                path: package_json,
                new_content,
                changes,
                refresh_npm: true,
            });
        }
    }

    let native_cargo = root.join("native").join("Cargo.toml");
    if native_cargo.is_file() {
        let content = fs::read_to_string(&native_cargo)
            .with_context(|| format!("read {}", native_cargo.display()))?;
        let (new_content, changes) =
            rewrite_cargo_lingxia_req(&content, &crate::versions::cargo_compat_req());
        if !changes.is_empty() {
            edits.push(Edit {
                path: native_cargo,
                new_content,
                changes,
                refresh_npm: false,
            });
        }
    }

    let windows_cargo = root.join("windows").join("Cargo.toml");
    if windows_cargo.is_file() {
        let content = fs::read_to_string(&windows_cargo)
            .with_context(|| format!("read {}", windows_cargo.display()))?;
        let (new_content, changes) =
            rewrite_windows_sdk_ref(&content, &crate::versions::windows_sdk_git_ref());
        if !changes.is_empty() {
            edits.push(Edit {
                path: windows_cargo,
                new_content,
                changes,
                refresh_npm: false,
            });
        }
    }

    let gradle = root.join("android").join("app").join("build.gradle.kts");
    if gradle.is_file() {
        let content =
            fs::read_to_string(&gradle).with_context(|| format!("read {}", gradle.display()))?;
        let (new_content, changes) =
            rewrite_gradle_sdk_fallback(&content, &crate::versions::current_versions().sdk);
        if !changes.is_empty() {
            edits.push(Edit {
                path: gradle,
                new_content,
                changes,
                refresh_npm: false,
            });
        }
    }

    Ok(edits)
}

/// All package.json files that belong to the project (root plus embedded
/// lxapps in a host project), skipping build/output directories.
fn find_package_jsons(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(root, 0, &mut found);
    found.sort();
    return found;

    fn walk(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
        if depth > 3 {
            return;
        }
        let candidate = dir.join("package.json");
        if candidate.is_file() {
            found.push(candidate);
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if SKIP_DIRS.contains(&name.as_ref()) {
                continue;
            }
            walk(&path, depth + 1, found);
        }
    }
}

/// Rewrite every `"@lingxia/…": "<range>"` entry to `range`, leaving
/// non-version specs (`file:`, `link:`, `workspace:`, git/URLs) untouched.
fn rewrite_npm_ranges(content: &str, range: &str) -> (String, Vec<String>) {
    let mut changes = Vec::new();
    let mut out = String::with_capacity(content.len());
    for (index, line) in content.split_inclusive('\n').enumerate() {
        out.push_str(&rewrite_npm_line(line, range, index, &mut changes));
    }
    (out, changes)
}

fn rewrite_npm_line(line: &str, range: &str, _index: usize, changes: &mut Vec<String>) -> String {
    let Some(key_start) = line.find("\"@lingxia/") else {
        return line.to_string();
    };
    let Some(key_end) = line[key_start + 1..].find('"').map(|i| key_start + 1 + i) else {
        return line.to_string();
    };
    let name = &line[key_start + 1..key_end];
    let rest = &line[key_end + 1..];
    let Some(value_open) = rest.find('"').map(|i| key_end + 2 + i) else {
        return line.to_string();
    };
    let Some(value_end) = line[value_open..].find('"').map(|i| value_open + i) else {
        return line.to_string();
    };
    let spec = &line[value_open..value_end];
    if !is_version_range(spec) || spec == range {
        return line.to_string();
    }
    changes.push(format!("{name}: {spec} -> {range}"));
    format!("{}{range}{}", &line[..value_open], &line[value_end..])
}

/// Whether an npm dependency spec is a plain version range (as opposed to a
/// path, workspace, or URL spec, which the project owns deliberately).
fn is_version_range(spec: &str) -> bool {
    spec.chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || matches!(c, '^' | '~' | '>' | '<' | '='))
}

/// Rewrite the `lingxia` dependency requirement in a native Cargo.toml,
/// handling both `lingxia = "req"` and `lingxia = { version = "req", … }`.
fn rewrite_cargo_lingxia_req(content: &str, req: &str) -> (String, Vec<String>) {
    let mut changes = Vec::new();
    let mut out = String::with_capacity(content.len());
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let is_dep = trimmed.starts_with("lingxia =") || trimmed.starts_with("lingxia=");
        if !is_dep {
            out.push_str(line);
            continue;
        }
        let rewritten = if trimmed.contains('{') {
            rewrite_quoted_after(line, "version = ", req)
        } else {
            rewrite_quoted_after(line, "lingxia = ", req)
        };
        match rewritten {
            Some((new_line, old)) if old != req => {
                changes.push(format!("lingxia: {old} -> {req}"));
                out.push_str(&new_line);
            }
            _ => out.push_str(line),
        }
    }
    (out, changes)
}

/// Rewrite the git ref (`rev = "…"` or `tag = "…"`) on the
/// `lingxia-windows-sdk` dependency line.
fn rewrite_windows_sdk_ref(content: &str, git_ref: &str) -> (String, Vec<String>) {
    let mut changes = Vec::new();
    let mut out = String::with_capacity(content.len());
    for line in content.split_inclusive('\n') {
        if !line.trim_start().starts_with("lingxia-windows-sdk") {
            out.push_str(line);
            continue;
        }
        let old_ref = ["rev = \"", "tag = \""].iter().find_map(|marker| {
            let start = line.find(marker)? + marker.len();
            let end = line[start..].find('"')? + start;
            Some((line[start..end].to_string(), marker, start, end))
        });
        match old_ref {
            Some((old, marker, start, end)) => {
                // `current`/`git_ref` are full fragments incl. both quotes,
                // e.g. `tag = "lingxia-crates-v0.11.2"`.
                let current = format!("{marker}{old}\"");
                if current == git_ref {
                    out.push_str(line);
                    continue;
                }
                let new_line = format!(
                    "{}{git_ref}{}",
                    &line[..start - marker.len()],
                    &line[end + 1..]
                );
                changes.push(format!("lingxia-windows-sdk: {current} -> {git_ref}"));
                out.push_str(&new_line);
            }
            None => out.push_str(line),
        }
    }
    (out, changes)
}

/// Rewrite the scaffolded `?: "<version>"` fallback on the
/// `lingxia.sdkVersion` gradle property (only used when gradle runs without
/// the CLI, which passes the live value).
fn rewrite_gradle_sdk_fallback(content: &str, sdk_version: &str) -> (String, Vec<String>) {
    let mut changes = Vec::new();
    let mut out = String::with_capacity(content.len());
    for line in content.split_inclusive('\n') {
        if !line.contains("lingxia.sdkVersion") || !line.contains("?:") {
            out.push_str(line);
            continue;
        }
        match rewrite_quoted_after(line, "?: ", sdk_version) {
            Some((new_line, old)) if old != sdk_version => {
                changes.push(format!(
                    "lingxia.sdkVersion fallback: {old} -> {sdk_version}"
                ));
                out.push_str(&new_line);
            }
            _ => out.push_str(line),
        }
    }
    (out, changes)
}

/// Replace the first quoted string after `marker` on `line` with `value`.
/// Returns the new line and the old value.
fn rewrite_quoted_after(line: &str, marker: &str, value: &str) -> Option<(String, String)> {
    let marker_at = line.find(marker)?;
    let open = line[marker_at..].find('"')? + marker_at + 1;
    let close = line[open..].find('"')? + open;
    let old = line[open..close].to_string();
    Some((format!("{}{value}{}", &line[..open], &line[close..]), old))
}

/// The `@lingxia/page-runtime` version actually installed in the project —
/// the code that gets compiled into every page bundle.
pub fn installed_lingxia_npm_version(project_root: &Path) -> Option<String> {
    for name in ["page-runtime", "types"] {
        let manifest = project_root
            .join("node_modules")
            .join("@lingxia")
            .join(name)
            .join("package.json");
        let Ok(content) = fs::read_to_string(&manifest) else {
            continue;
        };
        if let Some(version) = serde_json::from_str::<serde_json::Value>(&content)
            .ok()
            .and_then(|v| {
                v.get("version")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
        {
            return Some(version);
        }
    }
    None
}

fn major_minor(version: &str) -> Option<(u64, u64)> {
    let version = version.trim_start_matches(['^', '~', '=', '>', '<', ' ']);
    let mut parts = version.split('.');
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

/// One-line drift warning at build time: the project's installed/pinned
/// LingXia line differs from this CLI's. Never fails the build; silent when
/// nothing is resolvable.
pub fn warn_if_behind(project_root: &Path) {
    let cli_line = major_minor(env!("LINGXIA_RUST_CRATE_VERSION"));
    let project_line = installed_lingxia_npm_version(project_root)
        .or_else(|| pinned_native_crate_req(project_root))
        .and_then(|v| major_minor(&v));
    if let (Some(cli), Some(project)) = (cli_line, project_line)
        && cli != project
    {
        eprintln!(
            "{} This project is on the LingXia {}.{} line, the CLI on {}.{} — run `lingxia upgrade` in the project to align.",
            "!".yellow(),
            project.0,
            project.1,
            cli.0,
            cli.1
        );
    }
}

/// The `lingxia` crate requirement pinned in `native/Cargo.toml`, if any.
fn pinned_native_crate_req(project_root: &Path) -> Option<String> {
    let content = fs::read_to_string(project_root.join("native").join("Cargo.toml")).ok()?;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("lingxia =") || trimmed.starts_with("lingxia=")) {
            continue;
        }
        let marker = if trimmed.contains('{') {
            "version = "
        } else {
            "lingxia = "
        };
        if let Some((_, old)) = rewrite_quoted_after(line, marker, "") {
            return Some(old);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npm_ranges_are_rewritten_and_paths_kept() {
        let content = r#"{
  "dependencies": {
    "@lingxia/react": "~0.11.0",
    "@lingxia/types": "^0.11.2",
    "@lingxia/bridge": "file:../../../packages/lingxia-bridge",
    "react": "^19.0.0"
  },
  "devDependencies": {
    "@lingxia/test": "~0.11.0"
  }
}
"#;
        let (out, changes) = rewrite_npm_ranges(content, "~0.12.0");
        assert_eq!(changes.len(), 3);
        assert!(out.contains(r#""@lingxia/react": "~0.12.0""#));
        assert!(out.contains(r#""@lingxia/types": "~0.12.0""#));
        assert!(out.contains(r#""@lingxia/test": "~0.12.0""#));
        // Path specs and third-party deps stay untouched.
        assert!(out.contains("file:../../../packages/lingxia-bridge"));
        assert!(out.contains(r#""react": "^19.0.0""#));

        // Idempotent: a second pass changes nothing.
        let (again, changes) = rewrite_npm_ranges(&out, "~0.12.0");
        assert!(changes.is_empty());
        assert_eq!(again, out);
    }

    #[test]
    fn cargo_req_is_rewritten_in_both_forms() {
        let content = "[dependencies]\nlingxia = { version = \"0.11.2\", default-features = false, features = [\"standard\"] }\nserde = \"1\"\n";
        let (out, changes) = rewrite_cargo_lingxia_req(content, "~0.12.0");
        assert_eq!(changes, vec!["lingxia: 0.11.2 -> ~0.12.0"]);
        assert!(out.contains("lingxia = { version = \"~0.12.0\", default-features = false"));
        assert!(out.contains("serde = \"1\""));

        let simple = "lingxia = \"0.11\"\n";
        let (out, changes) = rewrite_cargo_lingxia_req(simple, "~0.12.0");
        assert_eq!(changes.len(), 1);
        assert_eq!(out, "lingxia = \"~0.12.0\"\n");
    }

    #[test]
    fn windows_git_ref_is_rewritten() {
        let content = "lingxia-windows-sdk = { git = \"https://github.com/LingXia-Dev/LingXia.git\", tag = \"lingxia-crates-v0.11.2\", package = \"lingxia-windows-sdk\", default-features = false }\n";
        let (out, changes) = rewrite_windows_sdk_ref(content, "rev = \"abc1234\"");
        assert_eq!(changes.len(), 1);
        assert!(out.contains("rev = \"abc1234\", package"));
        assert!(!out.contains("tag ="));

        // Same ref again: no change reported.
        let (again, changes) = rewrite_windows_sdk_ref(&out, "rev = \"abc1234\"");
        assert!(changes.is_empty());
        assert_eq!(again, out);
    }

    #[test]
    fn gradle_fallback_is_rewritten() {
        let content = "    val lingxiaSdkVersion = (findProperty(\"lingxia.sdkVersion\") as String?) ?: \"0.11.0\"\n";
        let (out, changes) = rewrite_gradle_sdk_fallback(content, "0.12.0");
        assert_eq!(changes.len(), 1);
        assert!(out.contains("?: \"0.12.0\""));
    }

    #[test]
    fn embedded_package_jsons_are_found_and_outputs_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("package.json"), "{}").unwrap();
        fs::create_dir_all(root.join("lxapp")).unwrap();
        fs::write(root.join("lxapp/package.json"), "{}").unwrap();
        fs::create_dir_all(root.join("node_modules/dep")).unwrap();
        fs::write(root.join("node_modules/dep/package.json"), "{}").unwrap();
        fs::create_dir_all(root.join("dist")).unwrap();
        fs::write(root.join("dist/package.json"), "{}").unwrap();

        let found = find_package_jsons(root);
        assert_eq!(found.len(), 2);
        assert!(
            found
                .iter()
                .all(|p| !p.to_string_lossy().contains("node_modules"))
        );
    }

    #[test]
    fn line_comparison_reads_ranges_and_versions() {
        assert_eq!(major_minor("~0.12.0"), Some((0, 12)));
        assert_eq!(major_minor("0.11.2"), Some((0, 11)));
        assert_eq!(major_minor("^1.2.3"), Some((1, 2)));
        assert_eq!(major_minor("garbage"), None);
    }
}
