//! `lingxia upgrade` inside a project: after the CLI half, compare the
//! project's LingXia line (major.minor of npm / crate / SDK pins) with this
//! CLI. A newer line is printed and offered as a prompt; `--yes` applies
//! without asking. Same-line patch drift is not a new version.
//!
//! Pins the project owns, rewritten string-level so formatting stays put:
//! `@lingxia/*` npm ranges, scaffolded LingXia crate requirements in
//! `native/Cargo.toml`, the `lingxia-windows-sdk` git ref +
//! `lingxia-windows-build` crate req, and the gradle `lingxia.sdkVersion`
//! fallback.
//!
//! SDK packages differ by platform and are fetched (or lock-refreshed) here
//! rather than waiting for the next `lingxia build`:
//! - Android: Maven zip into `~/.lingxia/sdk/android-maven/<ver>/`
//! - Apple: source zip into `~/.lingxia/sdk/apple/<ver>/`, then point
//!   `ios/` and `macos/` `Package.swift` at it
//! - Windows: `cargo update -p lingxia-windows-sdk` (git, not crates.io)
//! - Harmony: HAR into `~/.lingxia/sdk/harmony/<ver>/`
//!
//! In-workspace checkouts already have the SDK as source paths; those are
//! left alone.

use crate::sdk_cache::{self, SdkPlatform};
use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::{Confirm, theme::ColorfulTheme};
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Exit code for `--check` when the project is behind (mirrors the CLI
/// self-upgrade `--check` convention).
const EXIT_UPGRADE_AVAILABLE: i32 = 10;
const EXIT_CONFIRMATION_REQUIRED: i32 = 1;

const NATIVE_LINGXIA_CRATES: &[&str] = &[
    "lingxia",
    "lingxia-control-runtime",
    "lingxia-device-io",
    "lingxia-native-codegen",
];

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
    /// `cargo update -p <name>` in this file's directory after writing.
    cargo_update: Vec<String>,
}

/// Platform SDK work that is not a source-file rewrite.
enum SdkStep {
    Fetch { platform: SdkPlatform, cached: bool },
    InjectApple { dir: PathBuf },
}

struct UpgradePlan {
    edits: Vec<Edit>,
    sdk_steps: Vec<SdkStep>,
    sdk_version: String,
    project_line: Option<(u64, u64)>,
    cli_line: Option<(u64, u64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectConfirmation {
    Accepted,
    Declined,
    Required,
}

impl UpgradePlan {
    /// Newer CLI major.minor than the oldest project pin. Same-line patches
    /// (and a project that is ahead) are not a new version.
    fn line_behind(&self) -> bool {
        is_line_behind(self.project_line, self.cli_line)
    }
}

pub fn execute(root: &Path, check: bool, yes: bool) -> Result<i32> {
    let prepared = build_plan(root)?;
    report_plan(root, &prepared);

    if !prepared.line_behind() {
        println!("  {}", "up to date".green());
        return Ok(0);
    }

    if check {
        println!();
        println!(
            "{} The project is on an older LingXia line; run `lingxia upgrade` and confirm to apply.",
            "!".yellow()
        );
        return Ok(EXIT_UPGRADE_AVAILABLE);
    }

    match confirm_project_upgrade(prepared.project_line, prepared.cli_line, yes) {
        ProjectConfirmation::Accepted => {}
        ProjectConfirmation::Declined => {
            println!("  Skipped project pins and SDK packages.");
            return Ok(0);
        }
        ProjectConfirmation::Required => {
            println!("  Skipped project pins and SDK packages.");
            return Ok(EXIT_CONFIRMATION_REQUIRED);
        }
    }

    apply_plan(root, &prepared)?;
    Ok(0)
}

fn build_plan(root: &Path) -> Result<UpgradePlan> {
    let sdk_version = crate::versions::current_versions().sdk;
    let in_workspace = crate::platform::is_inside_lingxia_workspace(root);
    Ok(UpgradePlan {
        edits: plan(root)?,
        sdk_steps: collect_sdk_steps(root, in_workspace, &sdk_version),
        sdk_version,
        project_line: project_compat_line(root),
        cli_line: cli_compat_line(),
    })
}

fn report_plan(root: &Path, prepared: &UpgradePlan) {
    println!();
    println!("{}", "LingXia project".bold());
    println!("  root       {}", root.display());
    match (prepared.project_line, prepared.cli_line) {
        (Some(project), Some(cli)) => {
            println!("  project    {}.{}", project.0, project.1);
            println!("  CLI        {}.{}", cli.0, cli.1);
        }
        (_, Some(cli)) => {
            println!("  CLI        {}.{}", cli.0, cli.1);
        }
        _ => {}
    }

    if !prepared.line_behind() {
        return;
    }

    if !prepared.edits.is_empty() {
        println!();
        for edit in &prepared.edits {
            println!(
                "  {}",
                edit.path.strip_prefix(root).unwrap_or(&edit.path).display()
            );
            for change in &edit.changes {
                println!("    {change}");
            }
            if !edit.cargo_update.is_empty() {
                println!("    cargo update -p {}", edit.cargo_update.join(" -p "));
            }
        }
    }

    let pending_sdk = prepared
        .sdk_steps
        .iter()
        .any(|step| step.is_pending(&prepared.sdk_version));
    if pending_sdk {
        println!();
        println!("  {}", "SDK packages".bold());
        for step in &prepared.sdk_steps {
            if !step.is_pending(&prepared.sdk_version) {
                continue;
            }
            print_sdk_step(root, step, &prepared.sdk_version);
        }
    }
}

fn confirm_project_upgrade(
    project_line: Option<(u64, u64)>,
    cli_line: Option<(u64, u64)>,
    yes: bool,
) -> ProjectConfirmation {
    let stdin_is_terminal = std::io::stdin().is_terminal();
    if let Some(confirmation) = confirmation_without_prompt(yes, stdin_is_terminal) {
        if confirmation == ProjectConfirmation::Required
            && let (Some(project), Some(cli)) = (project_line, cli_line)
        {
            println!(
                "  {} Re-run in a terminal or pass --yes to upgrade pins and SDKs from {}.{} to {}.{}.",
                "!".yellow(),
                project.0,
                project.1,
                cli.0,
                cli.1
            );
        }
        return confirmation;
    }
    let (Some(project), Some(cli)) = (project_line, cli_line) else {
        return ProjectConfirmation::Declined;
    };
    if Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "Upgrade this project's pins and SDK packages from {}.{} to {}.{}?",
            project.0, project.1, cli.0, cli.1
        ))
        .default(true)
        .interact()
        .unwrap_or(false)
    {
        ProjectConfirmation::Accepted
    } else {
        ProjectConfirmation::Declined
    }
}

fn confirmation_without_prompt(yes: bool, stdin_is_terminal: bool) -> Option<ProjectConfirmation> {
    if yes {
        Some(ProjectConfirmation::Accepted)
    } else if !stdin_is_terminal {
        Some(ProjectConfirmation::Required)
    } else {
        None
    }
}

fn apply_plan(root: &Path, prepared: &UpgradePlan) -> Result<()> {
    let mut npm_dirs = Vec::new();
    let mut cargo_dirs: Vec<(PathBuf, Vec<String>)> = Vec::new();
    let mut refresh_failures = Vec::new();
    for edit in &prepared.edits {
        fs::write(&edit.path, &edit.new_content)
            .with_context(|| format!("write {}", edit.path.display()))?;
        if edit.refresh_npm
            && let Some(dir) = edit.path.parent()
        {
            npm_dirs.push(dir.to_path_buf());
        }
        if !edit.cargo_update.is_empty()
            && let Some(dir) = edit.path.parent()
        {
            cargo_dirs.push((dir.to_path_buf(), edit.cargo_update.clone()));
        }
    }
    if !prepared.edits.is_empty() {
        println!();
        println!("{} Project pins updated.", "✓".green());
    }

    if npm_dirs.is_empty() && root.join("package.json").is_file() {
        npm_dirs.push(root.to_path_buf());
    }

    for dir in npm_dirs {
        println!("  refreshing lockfile in {} …", dir.display());
        let status = Command::new(crate::npm::command())
            .arg("install")
            .current_dir(&dir)
            .status();
        match status {
            Ok(status) if status.success() => {}
            Ok(status) => {
                println!(
                    "  {} `npm install` failed with {status} in {}; run it manually.",
                    "!".yellow(),
                    dir.display()
                );
                refresh_failures.push(format!("npm install in {} ({status})", dir.display()));
            }
            Err(err) => {
                println!(
                    "  {} Could not run `npm install` in {}: {err}",
                    "!".yellow(),
                    dir.display()
                );
                refresh_failures.push(format!("npm install in {} ({err})", dir.display()));
            }
        }
    }

    for (dir, packages) in cargo_dirs {
        let mut cmd = Command::new("cargo");
        cmd.arg("update");
        for package in &packages {
            cmd.arg("-p").arg(package);
        }
        println!(
            "  cargo update -p {} in {} …",
            packages.join(" -p "),
            dir.display()
        );
        match cmd.current_dir(&dir).status() {
            Ok(status) if status.success() => {}
            Ok(status) => {
                println!(
                    "  {} `cargo update` failed with {status} in {}; run it manually.",
                    "!".yellow(),
                    dir.display()
                );
                refresh_failures.push(format!("cargo update in {} ({status})", dir.display()));
            }
            Err(err) => {
                println!(
                    "  {} Could not run `cargo update` in {}: {err}",
                    "!".yellow(),
                    dir.display()
                );
                refresh_failures.push(format!("cargo update in {} ({err})", dir.display()));
            }
        }
    }

    apply_sdk_steps(root, &prepared.sdk_steps, &prepared.sdk_version);
    if !refresh_failures.is_empty() {
        anyhow::bail!(
            "Project manifests changed, but dependency refresh failed: {}",
            refresh_failures.join("; ")
        );
    }
    Ok(())
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
                cargo_update: Vec::new(),
            });
        }
    }

    let native_cargo = root.join("native").join("Cargo.toml");
    if native_cargo.is_file() {
        let mut new_content = fs::read_to_string(&native_cargo)
            .with_context(|| format!("read {}", native_cargo.display()))?;
        let mut changes = Vec::new();
        let mut cargo_update = Vec::new();
        for crate_name in NATIVE_LINGXIA_CRATES {
            let (rewritten, crate_changes) = rewrite_cargo_dep_req(
                &new_content,
                crate_name,
                &crate::versions::cargo_compat_req(),
            );
            if !crate_changes.is_empty() {
                cargo_update.push((*crate_name).to_string());
                changes.extend(crate_changes);
            }
            new_content = rewritten;
        }
        if !changes.is_empty() {
            edits.push(Edit {
                path: native_cargo,
                new_content,
                changes,
                refresh_npm: false,
                cargo_update,
            });
        }
    }

    let windows_cargo = root.join("windows").join("Cargo.toml");
    if windows_cargo.is_file() {
        let content = fs::read_to_string(&windows_cargo)
            .with_context(|| format!("read {}", windows_cargo.display()))?;
        let (content, mut changes) =
            rewrite_windows_sdk_ref(&content, &crate::versions::windows_sdk_git_ref());
        let mut cargo_update = Vec::new();
        if changes.iter().any(|c| c.starts_with("lingxia-windows-sdk")) {
            cargo_update.push("lingxia-windows-sdk".to_string());
        }
        let (new_content, build_changes) = rewrite_cargo_dep_req(
            &content,
            "lingxia-windows-build",
            &crate::versions::cargo_compat_req(),
        );
        if !build_changes.is_empty() {
            cargo_update.push("lingxia-windows-build".to_string());
        }
        changes.extend(build_changes);
        if !changes.is_empty() {
            edits.push(Edit {
                path: windows_cargo,
                new_content,
                changes,
                refresh_npm: false,
                cargo_update,
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
                cargo_update: Vec::new(),
            });
        }
    }

    Ok(edits)
}

fn collect_sdk_steps(root: &Path, in_workspace: bool, version: &str) -> Vec<SdkStep> {
    if in_workspace {
        return Vec::new();
    }
    let mut steps = Vec::new();
    if root
        .join("android")
        .join("app")
        .join("build.gradle.kts")
        .is_file()
    {
        steps.push(SdkStep::Fetch {
            platform: SdkPlatform::Android,
            cached: sdk_cache::sdk_is_cached(SdkPlatform::Android, version),
        });
    }
    let mut apple_dirs = Vec::new();
    for name in ["ios", "macos"] {
        let dir = root.join(name);
        if dir.join("Package.swift").is_file() {
            apple_dirs.push(dir);
        }
    }
    if !apple_dirs.is_empty() {
        steps.push(SdkStep::Fetch {
            platform: SdkPlatform::Apple,
            cached: sdk_cache::sdk_is_cached(SdkPlatform::Apple, version),
        });
        for dir in apple_dirs {
            steps.push(SdkStep::InjectApple { dir });
        }
    }
    if root
        .join("harmony")
        .join("entry")
        .join("oh-package.json5")
        .is_file()
    {
        steps.push(SdkStep::Fetch {
            platform: SdkPlatform::Harmony,
            cached: sdk_cache::sdk_is_cached(SdkPlatform::Harmony, version),
        });
    }
    steps
}

impl SdkStep {
    fn is_pending(&self, sdk_version: &str) -> bool {
        match self {
            SdkStep::Fetch { cached, .. } => !cached,
            SdkStep::InjectApple { dir } => apple_inject_pending(dir, sdk_version),
        }
    }
}

fn apple_inject_pending(dir: &Path, sdk_version: &str) -> bool {
    let Some(sdk_dir) = sdk_cache::cached_sdk_dir(SdkPlatform::Apple, sdk_version) else {
        return true;
    };
    !crate::platform::apple::sdk_package_points_at(dir, &sdk_dir)
}

fn sdk_label(platform: SdkPlatform) -> &'static str {
    match platform {
        SdkPlatform::Android => "Android Maven",
        SdkPlatform::Apple => "Apple source",
        SdkPlatform::Harmony => "Harmony HAR",
    }
}

fn print_sdk_step(root: &Path, step: &SdkStep, sdk_version: &str) {
    match step {
        SdkStep::Fetch { platform, .. } => {
            let asset = platform.asset_name(sdk_version);
            println!(
                "    {} {sdk_version} (download {asset})",
                sdk_label(*platform)
            );
        }
        SdkStep::InjectApple { dir } => {
            let rel = dir.strip_prefix(root).unwrap_or(dir);
            println!(
                "    {}/Package.swift -> cached Apple SDK {sdk_version}",
                rel.display()
            );
        }
    }
}

fn apply_sdk_steps(root: &Path, steps: &[SdkStep], sdk_version: &str) {
    if steps.is_empty() {
        return;
    }
    println!();
    println!("{} Refreshing platform SDKs.", "✓".green());
    for step in steps {
        match step {
            SdkStep::Fetch { platform, .. } => {
                println!("  fetching {} {} …", sdk_label(*platform), sdk_version);
                match sdk_cache::ensure_sdk(*platform, sdk_version) {
                    Ok(path) => println!("    {} {}", "✓".green(), path.display()),
                    Err(err) => println!(
                        "    {} {err}\n      Pins are updated; `lingxia build` will retry the fetch.",
                        "!".yellow()
                    ),
                }
            }
            SdkStep::InjectApple { dir } => {
                let Some(sdk_dir) = sdk_cache::cached_sdk_dir(SdkPlatform::Apple, sdk_version)
                else {
                    continue;
                };
                if !sdk_cache::sdk_is_cached(SdkPlatform::Apple, sdk_version) {
                    println!(
                        "    {} skip {}/Package.swift (Apple SDK not in cache)",
                        "!".yellow(),
                        dir.strip_prefix(root).unwrap_or(dir).display()
                    );
                    continue;
                }
                match crate::platform::apple::inject_sdk_package_dependency(dir, &sdk_dir) {
                    Ok(()) => println!(
                        "    {} {}/Package.swift",
                        "✓".green(),
                        dir.strip_prefix(root).unwrap_or(dir).display()
                    ),
                    Err(err) => println!("    {} {err}", "!".yellow()),
                }
            }
        }
    }
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
    for line in content.split_inclusive('\n') {
        out.push_str(&rewrite_npm_line(line, range, &mut changes));
    }
    (out, changes)
}

fn rewrite_npm_line(line: &str, range: &str, changes: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(line.len());
    let mut copied_through = 0;
    let mut search_from = 0;
    while let Some((name, spec, value_start, value_end)) = next_npm_entry(line, search_from) {
        if is_version_range(spec) && spec != range {
            out.push_str(&line[copied_through..value_start]);
            out.push_str(range);
            copied_through = value_end;
            changes.push(format!("{name}: {spec} -> {range}"));
        }
        search_from = value_end + 1;
    }
    out.push_str(&line[copied_through..]);
    out
}

fn next_npm_entry(line: &str, search_from: usize) -> Option<(&str, &str, usize, usize)> {
    let mut cursor = search_from;
    loop {
        let key_start = line[cursor..].find("\"@lingxia/")? + cursor;
        let key_end = line[key_start + 1..].find('"')? + key_start + 1;
        let after_key = &line[key_end + 1..];
        let delimiter_offset = after_key.find(|c: char| !c.is_whitespace())?;
        let delimiter = key_end + 1 + delimiter_offset;
        if line.as_bytes().get(delimiter) != Some(&b':') {
            cursor = key_end + 1;
            continue;
        }
        let after_colon = &line[delimiter + 1..];
        let value_quote_offset = after_colon.find(|c: char| !c.is_whitespace())?;
        let value_quote = delimiter + 1 + value_quote_offset;
        if line.as_bytes().get(value_quote) != Some(&b'"') {
            cursor = key_end + 1;
            continue;
        }
        let value_start = value_quote + 1;
        let value_end = line[value_start..].find('"')? + value_start;
        return Some((
            &line[key_start + 1..key_end],
            &line[value_start..value_end],
            value_start,
            value_end,
        ));
    }
}

/// Whether an npm dependency spec is a plain version range (as opposed to a
/// path, workspace, or URL spec, which the project owns deliberately).
fn is_version_range(spec: &str) -> bool {
    spec.chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || matches!(c, '^' | '~' | '>' | '<' | '='))
}

fn cargo_table_name(line: &str) -> Option<&str> {
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

fn is_cargo_dependency_table(table: &str) -> bool {
    matches!(
        table,
        "dependencies" | "dev-dependencies" | "build-dependencies"
    ) || table == "workspace.dependencies"
        || (table.starts_with("target.")
            && (table.ends_with(".dependencies")
                || table.ends_with(".dev-dependencies")
                || table.ends_with(".build-dependencies")))
}

/// Rewrite `crate_name = "req"` / `{ version = "req", … }` in a Cargo.toml.
/// Path-only tables have no `version =` and are left untouched.
fn rewrite_cargo_dep_req(content: &str, crate_name: &str, req: &str) -> (String, Vec<String>) {
    let mut changes = Vec::new();
    let mut out = String::with_capacity(content.len());
    let prefix = format!("{crate_name} =");
    let prefix_nospace = format!("{crate_name}=");
    let mut in_dependencies = false;
    for line in content.split_inclusive('\n') {
        if let Some(table) = cargo_table_name(line) {
            in_dependencies = is_cargo_dependency_table(table);
            out.push_str(line);
            continue;
        }
        let trimmed = line.trim_start();
        let is_dep = in_dependencies
            && (trimmed.starts_with(&prefix) || trimmed.starts_with(&prefix_nospace));
        if !is_dep {
            out.push_str(line);
            continue;
        }
        let rewritten = if trimmed.contains('{') {
            rewrite_quoted_after(line, "version = ", req)
                .or_else(|| rewrite_quoted_after(line, "version=", req))
        } else {
            rewrite_quoted_after(line, &format!("{crate_name} = "), req)
                .or_else(|| rewrite_quoted_after(line, &format!("{crate_name}="), req))
        };
        match rewritten {
            Some((new_line, old)) if old != req => {
                changes.push(format!("{crate_name}: {old} -> {req}"));
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
    let mut in_dependencies = false;
    for line in content.split_inclusive('\n') {
        if let Some(table) = cargo_table_name(line) {
            in_dependencies = is_cargo_dependency_table(table);
            out.push_str(line);
            continue;
        }
        if !in_dependencies || !line.trim_start().starts_with("lingxia-windows-sdk") {
            out.push_str(line);
            continue;
        }
        let old_ref = ["rev = \"", "tag = \"", "rev=\"", "tag=\""]
            .iter()
            .find_map(|marker| {
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

fn cli_compat_line() -> Option<(u64, u64)> {
    major_minor(env!("LINGXIA_RUST_CRATE_VERSION"))
}

/// Oldest major.minor among the project's LingXia pins (installed npm, npm
/// ranges, native crate, Android SDK fallback, Windows crates tag / build crate).
fn project_compat_line(root: &Path) -> Option<(u64, u64)> {
    collect_project_versions(root)
        .into_iter()
        .filter_map(|v| major_minor(&v))
        .min()
}

fn is_line_behind(project: Option<(u64, u64)>, cli: Option<(u64, u64)>) -> bool {
    matches!((project, cli), (Some(p), Some(c)) if p < c)
}

fn collect_project_versions(root: &Path) -> Vec<String> {
    let mut versions = Vec::new();
    if let Some(version) = installed_lingxia_npm_version(root) {
        versions.push(version);
    }
    for package_json in find_package_jsons(root) {
        if let Ok(content) = fs::read_to_string(&package_json) {
            versions.extend(npm_range_specs(&content));
        }
    }
    versions.extend(pinned_native_crate_reqs(root));
    if let Some(version) = pinned_gradle_sdk(root) {
        versions.push(version);
    }
    if let Some(version) = pinned_windows_sdk_version(root) {
        versions.push(version);
    }
    if let Some(version) = pinned_windows_build_req(root) {
        versions.push(version);
    }
    versions
}

fn npm_range_specs(content: &str) -> Vec<String> {
    let mut specs = Vec::new();
    for line in content.lines() {
        let mut search_from = 0;
        while let Some((_, spec, _, value_end)) = next_npm_entry(line, search_from) {
            if is_version_range(spec) {
                specs.push(spec.to_string());
            }
            search_from = value_end + 1;
        }
    }
    specs
}

fn pinned_gradle_sdk(root: &Path) -> Option<String> {
    let content =
        fs::read_to_string(root.join("android").join("app").join("build.gradle.kts")).ok()?;
    for line in content.lines() {
        if line.contains("lingxia.sdkVersion")
            && line.contains("?:")
            && let Some((_, old)) = rewrite_quoted_after(line, "?: ", "")
        {
            return Some(old);
        }
    }
    None
}

fn pinned_windows_sdk_version(root: &Path) -> Option<String> {
    let content = fs::read_to_string(root.join("windows").join("Cargo.toml")).ok()?;
    let mut in_dependencies = false;
    for line in content.lines() {
        if let Some(table) = cargo_table_name(line) {
            in_dependencies = is_cargo_dependency_table(table);
            continue;
        }
        if !in_dependencies || !line.trim_start().starts_with("lingxia-windows-sdk") {
            continue;
        }
        for prefix in ["tag = \"lingxia-crates-v", "tag=\"lingxia-crates-v"] {
            if let Some(start) = line.find(prefix).map(|index| index + prefix.len()) {
                let end = line[start..].find('"')? + start;
                return Some(line[start..end].to_string());
            }
        }
    }
    None
}

fn pinned_windows_build_req(root: &Path) -> Option<String> {
    let content = fs::read_to_string(root.join("windows").join("Cargo.toml")).ok()?;
    cargo_dependency_req(&content, "lingxia-windows-build")
}

fn cargo_dependency_req(content: &str, crate_name: &str) -> Option<String> {
    let prefix = format!("{crate_name} =");
    let prefix_nospace = format!("{crate_name}=");
    let mut in_dependencies = false;
    for line in content.lines() {
        if let Some(table) = cargo_table_name(line) {
            in_dependencies = is_cargo_dependency_table(table);
            continue;
        }
        let trimmed = line.trim_start();
        if !in_dependencies
            || !(trimmed.starts_with(&prefix) || trimmed.starts_with(&prefix_nospace))
        {
            continue;
        }
        let markers = if trimmed.contains('{') {
            ["version = ", "version="]
        } else {
            [prefix.as_str(), prefix_nospace.as_str()]
        };
        if let Some(old) = markers
            .iter()
            .find_map(|marker| rewrite_quoted_after(line, marker, "").map(|(_, old)| old))
        {
            return Some(old);
        }
    }
    None
}

/// One-line drift warning at build time: the project's installed/pinned
/// LingXia line is older than this CLI's. Never fails the build; silent when
/// nothing is resolvable.
pub fn warn_if_behind(project_root: &Path) {
    if let (Some(cli), Some(project)) = (cli_compat_line(), project_compat_line(project_root))
        && project < cli
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

/// Version requirements for scaffolded LingXia crates in `native/Cargo.toml`.
fn pinned_native_crate_reqs(project_root: &Path) -> Vec<String> {
    let Ok(content) = fs::read_to_string(project_root.join("native").join("Cargo.toml")) else {
        return Vec::new();
    };
    NATIVE_LINGXIA_CRATES
        .iter()
        .filter_map(|crate_name| cargo_dependency_req(&content, crate_name))
        .collect()
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
    fn minified_npm_manifest_rewrites_and_reads_every_lingxia_range() {
        let content = r#"{"dependencies":{"@lingxia/react":"~0.11.0","@lingxia/types":"^0.11.2","@lingxia/bridge":"file:../bridge"}}"#;
        let (out, changes) = rewrite_npm_ranges(content, "~0.13.0");
        assert_eq!(changes.len(), 2);
        assert!(out.contains(r#""@lingxia/react":"~0.13.0""#));
        assert!(out.contains(r#""@lingxia/types":"~0.13.0""#));
        assert!(out.contains(r#""@lingxia/bridge":"file:../bridge""#));
        assert_eq!(
            npm_range_specs(content),
            vec!["~0.11.0".to_string(), "^0.11.2".to_string()]
        );
    }

    #[test]
    fn npm_scanner_requires_an_object_key() {
        let content = r#"{"keywords":["@lingxia/react","0.11.0"],"dependencies":{"@lingxia/react":"0.11.0"}}"#;
        let (out, changes) = rewrite_npm_ranges(content, "~0.13.0");
        assert_eq!(changes, vec!["@lingxia/react: 0.11.0 -> ~0.13.0"]);
        assert!(out.contains(r#""keywords":["@lingxia/react","0.11.0"]"#));
        assert_eq!(npm_range_specs(content), vec!["0.11.0".to_string()]);
    }

    #[test]
    fn cargo_req_is_rewritten_in_both_forms() {
        let content = "[dependencies]\nlingxia = { version = \"0.11.2\", default-features = false, features = [\"standard\"] }\nserde = \"1\"\n";
        let (out, changes) = rewrite_cargo_dep_req(content, "lingxia", "~0.12.0");
        assert_eq!(changes, vec!["lingxia: 0.11.2 -> ~0.12.0"]);
        assert!(out.contains("lingxia = { version = \"~0.12.0\", default-features = false"));
        assert!(out.contains("serde = \"1\""));

        let simple = "[dependencies]\nlingxia = \"0.11\"\n";
        let (out, changes) = rewrite_cargo_dep_req(simple, "lingxia", "~0.12.0");
        assert_eq!(changes.len(), 1);
        assert_eq!(out, "[dependencies]\nlingxia = \"~0.12.0\"\n");

        let path_only = "[dependencies]\nlingxia = { path = \"../../../crates/lingxia\" }\n";
        let (out, changes) = rewrite_cargo_dep_req(path_only, "lingxia", "~0.12.0");
        assert!(changes.is_empty());
        assert_eq!(out, path_only);

        let compact = "[dependencies]\nlingxia={version=\"0.11.2\",default-features=false}\n";
        let (out, changes) = rewrite_cargo_dep_req(compact, "lingxia", "~0.13.0");
        assert_eq!(changes, vec!["lingxia: 0.11.2 -> ~0.13.0"]);
        assert_eq!(
            out,
            "[dependencies]\nlingxia={version=\"~0.13.0\",default-features=false}\n"
        );

        let compact_simple = "[dependencies]\nlingxia=\"0.11\"\n";
        let (out, changes) = rewrite_cargo_dep_req(compact_simple, "lingxia", "~0.13.0");
        assert_eq!(changes, vec!["lingxia: 0.11 -> ~0.13.0"]);
        assert_eq!(out, "[dependencies]\nlingxia=\"~0.13.0\"\n");
    }

    #[test]
    fn cargo_rewrites_only_dependency_tables() {
        let content = "[features]\nlingxia = [\"dep:lingxia\"]\n\n[package.metadata.dependencies]\nlingxia = \"metadata\"\n\n[target.'cfg(windows)'.dependencies]\nlingxia = { version = \"0.11.2\" }\n\n[[example]]\nlingxia = [\"metadata\"]\n";
        let (out, changes) = rewrite_cargo_dep_req(content, "lingxia", "~0.13.0");
        assert_eq!(changes, vec!["lingxia: 0.11.2 -> ~0.13.0"]);
        assert!(out.contains("[features]\nlingxia = [\"dep:lingxia\"]"));
        assert!(out.contains("[package.metadata.dependencies]\nlingxia = \"metadata\""));
        assert!(out.contains("[[example]]\nlingxia = [\"metadata\"]"));
        assert!(out.contains("lingxia = { version = \"~0.13.0\" }"));
        assert_eq!(
            cargo_dependency_req(content, "lingxia").as_deref(),
            Some("0.11.2")
        );
    }

    #[test]
    fn native_plan_updates_and_diagnoses_every_scaffolded_crate() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("native")).unwrap();
        fs::write(
            root.join("native/Cargo.toml"),
            "[features]\nlingxia = [\"dep:lingxia\"]\n\n[dependencies]\nlingxia = { version = \"0.11.2\" }\nlingxia-control-runtime = { version = \"0.10.0\", optional = true }\nlingxia-device-io = { version = \"0.12.0\", optional = true }\n\n[build-dependencies]\nlingxia-native-codegen = { version = \"0.11.0\" }\n",
        )
        .unwrap();

        let edits = plan(root).unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].changes.len(), NATIVE_LINGXIA_CRATES.len());
        assert_eq!(
            edits[0].cargo_update,
            NATIVE_LINGXIA_CRATES
                .iter()
                .map(|name| (*name).to_string())
                .collect::<Vec<_>>()
        );
        assert!(
            edits[0]
                .new_content
                .contains("[features]\nlingxia = [\"dep:lingxia\"]")
        );
        assert_eq!(
            pinned_native_crate_reqs(root),
            vec!["0.11.2", "0.10.0", "0.12.0", "0.11.0"]
        );
        assert_eq!(project_compat_line(root), Some((0, 10)));
    }

    #[test]
    fn non_interactive_upgrade_requires_yes() {
        assert_eq!(
            confirmation_without_prompt(false, false),
            Some(ProjectConfirmation::Required)
        );
        assert_eq!(
            confirmation_without_prompt(true, false),
            Some(ProjectConfirmation::Accepted)
        );
        assert_eq!(confirmation_without_prompt(false, true), None);
    }

    #[test]
    fn windows_build_crate_req_is_rewritten() {
        let content = "[build-dependencies]\nlingxia-windows-build = { version = \"0.11.2\" }\n";
        let (out, changes) = rewrite_cargo_dep_req(content, "lingxia-windows-build", "~0.12.0");
        assert_eq!(changes, vec!["lingxia-windows-build: 0.11.2 -> ~0.12.0"]);
        assert_eq!(
            out,
            "[build-dependencies]\nlingxia-windows-build = { version = \"~0.12.0\" }\n"
        );
    }

    #[test]
    fn windows_git_ref_is_rewritten() {
        let content = "[dependencies]\nlingxia-windows-sdk = { git = \"https://github.com/LingXia-Dev/LingXia.git\", tag = \"lingxia-crates-v0.11.2\", package = \"lingxia-windows-sdk\", default-features = false }\n";
        let (out, changes) = rewrite_windows_sdk_ref(content, "rev = \"abc1234\"");
        assert_eq!(changes.len(), 1);
        assert!(out.contains("rev = \"abc1234\", package"));
        assert!(!out.contains("tag ="));

        // Same ref again: no change reported.
        let (again, changes) = rewrite_windows_sdk_ref(&out, "rev = \"abc1234\"");
        assert!(changes.is_empty());
        assert_eq!(again, out);

        let compact =
            "[dependencies]\nlingxia-windows-sdk={git=\"repo\",tag=\"lingxia-crates-v0.11.2\"}\n";
        let (out, changes) = rewrite_windows_sdk_ref(compact, "rev = \"abc1234\"");
        assert_eq!(changes.len(), 1);
        assert!(out.contains("rev = \"abc1234\""));
        assert!(!out.contains("tag="));
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

    #[test]
    fn sdk_steps_cover_each_platform_and_skip_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("android/app")).unwrap();
        fs::write(root.join("android/app/build.gradle.kts"), "").unwrap();
        fs::create_dir_all(root.join("ios")).unwrap();
        fs::write(
            root.join("ios/Package.swift"),
            "// swift-tools-version: 6.0\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("macos")).unwrap();
        fs::write(
            root.join("macos/Package.swift"),
            "// swift-tools-version: 6.0\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("harmony/entry")).unwrap();
        fs::write(root.join("harmony/entry/oh-package.json5"), "{}\n").unwrap();

        let steps = collect_sdk_steps(root, false, "0.12.0");
        let fetches: Vec<_> = steps
            .iter()
            .filter_map(|s| match s {
                SdkStep::Fetch { platform, .. } => Some(*platform),
                _ => None,
            })
            .collect();
        assert_eq!(
            fetches,
            vec![
                SdkPlatform::Android,
                SdkPlatform::Apple,
                SdkPlatform::Harmony
            ]
        );
        let injects = steps
            .iter()
            .filter(|s| matches!(s, SdkStep::InjectApple { .. }))
            .count();
        assert_eq!(injects, 2);

        assert!(collect_sdk_steps(root, true, "0.12.0").is_empty());
    }

    #[test]
    fn windows_git_ref_and_build_crate_compose() {
        let content = "[dependencies]\nlingxia-windows-sdk = { git = \"https://github.com/LingXia-Dev/LingXia.git\", tag = \"lingxia-crates-v0.11.2\", package = \"lingxia-windows-sdk\", default-features = false }\n\n[build-dependencies]\nlingxia-windows-build = { version = \"0.11.2\" }\n";
        let (mid, sdk_changes) = rewrite_windows_sdk_ref(content, "rev = \"abc1234\"");
        let (out, build_changes) = rewrite_cargo_dep_req(&mid, "lingxia-windows-build", "~0.12.0");
        assert_eq!(sdk_changes.len(), 1);
        assert_eq!(build_changes.len(), 1);
        assert!(out.contains("rev = \"abc1234\""));
        assert!(out.contains("lingxia-windows-build = { version = \"~0.12.0\" }"));
    }

    #[test]
    fn major_minor_line_is_the_safe_behind_check() {
        assert!(is_line_behind(Some((0, 11)), Some((0, 12))));
        assert!(!is_line_behind(Some((0, 12)), Some((0, 12))));
        assert!(!is_line_behind(Some((0, 13)), Some((0, 12))));
        assert!(!is_line_behind(None, Some((0, 12))));
        assert!(!is_line_behind(Some((0, 11)), None));
    }

    #[test]
    fn npm_range_specs_skip_path_deps() {
        let content = r#"{
  "dependencies": {
    "@lingxia/react": "~0.11.0",
    "@lingxia/bridge": "file:../../../packages/lingxia-bridge"
  }
}
"#;
        assert_eq!(npm_range_specs(content), vec!["~0.11.0".to_string()]);
    }

    #[test]
    fn oldest_pin_wins_the_project_line() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(
            root.join("package.json"),
            r#"{ "dependencies": { "@lingxia/react": "~0.12.0" } }"#,
        )
        .unwrap();
        fs::create_dir_all(root.join("native")).unwrap();
        fs::write(
            root.join("native/Cargo.toml"),
            "[dependencies]\nlingxia = { version = \"0.11.2\" }\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("android/app")).unwrap();
        fs::write(
            root.join("android/app/build.gradle.kts"),
            "val lingxiaSdkVersion = (findProperty(\"lingxia.sdkVersion\") as String?) ?: \"0.12.0\"\n",
        )
        .unwrap();
        assert_eq!(project_compat_line(root), Some((0, 11)));
    }

    #[test]
    fn windows_crates_tag_parses_as_a_version() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("windows")).unwrap();
        fs::write(
            root.join("windows/Cargo.toml"),
            "[dependencies]\nlingxia-windows-sdk = { git = \"https://github.com/LingXia-Dev/LingXia.git\", tag = \"lingxia-crates-v0.11.2\", package = \"lingxia-windows-sdk\" }\n",
        )
        .unwrap();
        assert_eq!(pinned_windows_sdk_version(root).as_deref(), Some("0.11.2"));
        assert_eq!(project_compat_line(root), Some((0, 11)));
    }
}
