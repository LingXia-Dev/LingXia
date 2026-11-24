//! The project type-check: `tsc -b` at the lxapp root, through the project's own TypeScript,
//! or TypeScript 7's native `tsgo` binary when installed.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

static SKIPPED: AtomicBool = AtomicBool::new(false);

/// `lingxia build --no-typecheck`: every lxapp this process builds, a host's bundles included.
pub(crate) fn skip() {
    SKIPPED.store(true, Ordering::Relaxed);
}

pub(crate) fn skipped() -> bool {
    SKIPPED.load(Ordering::Relaxed)
}

/// A project type-checks when it has a root `tsconfig.json`; the templates' is a solution file.
pub(crate) fn applies(root: &Path) -> bool {
    root.join("tsconfig.json").is_file()
}

/// Every type error as `file:line:col: TSnnnn: message`; empty when the project checks clean.
pub(crate) fn run(root: &Path) -> Result<Vec<String>> {
    if let Some(tsgo) = resolve_tsgo(root) {
        announce(&tsgo, "tsgo", || {
            Command::new(&tsgo)
                .arg("--version")
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| {
                    let text = String::from_utf8_lossy(&output.stdout);
                    text.trim().trim_start_matches("Version ").to_owned()
                })
                .unwrap_or_else(|| "of unknown version".to_owned())
        });
        return run_tsgo(root, &tsgo);
    }
    let tsc = resolve_tsc(root)?;
    let typescript = supported_typescript(&tsc)?;
    announce(typescript, "tsc", || package_version(typescript));
    super::ts_worker::typecheck(root, typescript)
}

/// Names the checker the first time this process uses it, so an installed `tsgo` never runs
/// unannounced.
fn announce(checker: &Path, name: &str, version: impl FnOnce() -> String) {
    static SEEN: std::sync::Mutex<std::collections::BTreeSet<PathBuf>> =
        std::sync::Mutex::new(std::collections::BTreeSet::new());
    let first = SEEN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(checker.to_path_buf());
    if first {
        println!(
            "{} {name} {} {}",
            console::style("Type check").cyan(),
            version(),
            console::style(checker.display()).dim()
        );
    }
}

fn package_version(package: &Path) -> String {
    std::fs::read(package.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|manifest| manifest["version"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// The problems under one heading, in the shape of the build's other diagnostics.
pub(crate) fn report(problems: &[String]) -> String {
    let count = problems.len();
    format!(
        "{count} type error{}\n{}",
        if count == 1 { "" } else { "s" },
        problems.join("\n")
    )
}

/// The TypeScript range the CLI drives through its JS API; `lingxia new` writes it into
/// `package.json`. TypeScript 7 (`tsgo`) has no JS compiler API and is driven as a native binary
/// when installed.
pub(crate) const TYPESCRIPT_RANGE: &str = "^5.9.3";
const TYPESCRIPT_MIN: (u64, u64, u64) = (5, 9, 3);

/// The package directory of the TypeScript that `tsc` belongs to, once its version is in
/// [`TYPESCRIPT_RANGE`].
pub(crate) fn supported_typescript(tsc: &Path) -> Result<&Path> {
    let package = tsc
        .parent()
        .and_then(Path::parent)
        .context("TypeScript install has no package directory")?;
    let version = package_version(package);
    if !in_range(&version) {
        bail!(
            "TypeScript {} is installed here; LingXia supports {TYPESCRIPT_RANGE}: run `npm i -D typescript@{TYPESCRIPT_RANGE}`",
            if version.is_empty() {
                "of unknown version"
            } else {
                &version
            }
        );
    }
    Ok(package)
}

fn in_range(version: &str) -> bool {
    let core = version.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<u64> = core
        .split('.')
        .filter_map(|part| part.parse().ok())
        .collect();
    let [major, minor, patch] = parts[..] else {
        return false;
    };
    major == TYPESCRIPT_MIN.0 && (minor, patch) >= (TYPESCRIPT_MIN.1, TYPESCRIPT_MIN.2)
}

pub(crate) fn resolve_tsc(root: &Path) -> Result<PathBuf> {
    root.ancestors()
        .map(|dir| dir.join("node_modules/typescript/bin/tsc"))
        .find(|tsc| tsc.is_file())
        .with_context(|| {
            format!(
                "Type check: TypeScript is not installed for {}; run `npm install` there (or pass --no-typecheck)",
                root.display()
            )
        })
}

/// Locates a native TypeScript 7 (`tsgo`) binary: `LINGXIA_TSGO` names one (`0`, `false` or
/// `off` keeps the project's `tsc`), else one installed in the project's `node_modules`.
pub(crate) fn resolve_tsgo(root: &Path) -> Option<PathBuf> {
    if let Ok(value) = std::env::var("LINGXIA_TSGO") {
        let trimmed = value.trim();
        if matches!(trimmed, "0" | "false" | "off") {
            return None;
        }
        let path = PathBuf::from(trimmed);
        if !trimmed.is_empty() && (path.is_file() || !trimmed.contains(['/', '\\'])) {
            return Some(path);
        }
    }
    let candidates: &[&str] = if cfg!(windows) {
        &[
            "node_modules/@typescript/native-preview/bin/tsgo.exe",
            "node_modules/.bin/tsgo.exe",
            "node_modules/.bin/tsgo.cmd",
        ]
    } else {
        &[
            "node_modules/@typescript/native-preview/bin/tsgo",
            "node_modules/.bin/tsgo",
        ]
    };
    root.ancestors()
        .flat_map(|dir| candidates.iter().map(move |rel| dir.join(rel)))
        .find(|candidate| candidate.is_file())
}

fn write_if_changed(path: &Path, content: &str) -> Result<()> {
    if let Ok(existing) = std::fs::read_to_string(path)
        && existing == content
    {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content).with_context(|| format!("write {}", path.display()))
}

fn resolve_ref_config(base_dir: &Path, ref_path: &str) -> PathBuf {
    let candidate = if Path::new(ref_path).is_absolute() {
        PathBuf::from(ref_path)
    } else {
        base_dir.join(ref_path)
    };
    if candidate.is_dir()
        || !candidate
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
    {
        candidate.join("tsconfig.json")
    } else {
        candidate
    }
}

/// A tsconfig's `extends` target: a relative or absolute file, with or without `.json`, or a
/// package's file found in `node_modules` above `dir`.
fn resolve_extends(dir: &Path, spec: &str) -> Option<PathBuf> {
    let file = |base: PathBuf| {
        [
            base.clone(),
            {
                let mut json = base.clone().into_os_string();
                json.push(".json");
                PathBuf::from(json)
            },
            base.join("tsconfig.json"),
        ]
        .into_iter()
        .find(|candidate| candidate.is_file())
    };
    if spec.starts_with('.') || Path::new(spec).is_absolute() {
        return file(dir.join(spec));
    }
    dir.ancestors()
        .find_map(|ancestor| file(ancestor.join("node_modules").join(spec)))
}

/// The options a wrapper must respect, as `config` gets them through its `extends` chain.
#[derive(Default)]
struct Inherited {
    composite: Option<bool>,
    no_emit: Option<bool>,
    root_dir: bool,
}

fn inherited(config: &Path, depth: usize) -> Inherited {
    let parsed: serde_json::Value = std::fs::read_to_string(config)
        .ok()
        .and_then(|raw| json5::from_str(&raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let mut merged = Inherited::default();
    let bases: Vec<&str> = match parsed.get("extends") {
        Some(serde_json::Value::String(base)) => vec![base.as_str()],
        Some(serde_json::Value::Array(bases)) => {
            bases.iter().filter_map(serde_json::Value::as_str).collect()
        }
        _ => Vec::new(),
    };
    let dir = config.parent().unwrap_or(Path::new("."));
    // Later bases override earlier ones; a cycle stops at the depth limit.
    for base in bases.into_iter().filter(|_| depth < 16) {
        if let Some(path) = resolve_extends(dir, base) {
            let parent = inherited(&path, depth + 1);
            merged.composite = parent.composite.or(merged.composite);
            merged.no_emit = parent.no_emit.or(merged.no_emit);
            merged.root_dir |= parent.root_dir;
        }
    }
    let options = parsed.get("compilerOptions");
    let flag = |name: &str| options.and_then(|o| o.get(name)).and_then(|v| v.as_bool());
    merged.composite = flag("composite").or(merged.composite);
    merged.no_emit = flag("noEmit").or(merged.no_emit);
    merged.root_dir |= options.and_then(|o| o.get("rootDir")).is_some();
    merged
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Recursively writes `.lingxia/typecheck/` wrapper configs matching `ts_worker.rs`:
/// preserves nested `references`, keeps `rootDir` on `composite` configs, avoids `noEmit: true`
/// on composite emit projects, and prunes stale wrapper files.
fn write_typecheck_wrappers(root: &Path) -> Result<PathBuf> {
    let dir = root.join(".lingxia").join("typecheck");
    std::fs::create_dir_all(&dir)?;
    let mut wrapped: std::collections::HashMap<String, PathBuf> = std::collections::HashMap::new();
    let mut kept: std::collections::HashSet<String> = std::collections::HashSet::new();

    fn wrap_one(
        config_path: &Path,
        dir: &Path,
        is_root: bool,
        wrapped: &mut std::collections::HashMap<String, PathBuf>,
        kept: &mut std::collections::HashSet<String>,
    ) -> Result<PathBuf> {
        let canonical =
            std::fs::canonicalize(config_path).unwrap_or_else(|_| config_path.to_path_buf());
        let key = if cfg!(windows) {
            slash_path(&canonical).to_ascii_lowercase()
        } else {
            slash_path(&canonical)
        };
        if let Some(existing) = wrapped.get(&key) {
            return Ok(existing.clone());
        }
        let name = if is_root {
            "solution.json".to_string()
        } else {
            format!("ref-{}.json", wrapped.len())
        };
        let target = dir.join(&name);
        wrapped.insert(key, target.clone());
        kept.insert(name.clone());

        let parsed: serde_json::Value = std::fs::read_to_string(config_path)
            .ok()
            .and_then(|raw| json5::from_str(&raw).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        let base_dir = config_path.parent().unwrap_or(dir);
        let options = inherited(config_path, 0);
        let has_no_emit = options.no_emit.unwrap_or(false);
        let is_composite = options.composite.unwrap_or(false);
        let has_root_dir = options.root_dir;

        let mut wrapper_refs = Vec::new();
        if let Some(refs) = parsed.get("references").and_then(|v| v.as_array()) {
            for entry in refs {
                if let Some(ref_path) = entry.get("path").and_then(|v| v.as_str()) {
                    let child_cfg = resolve_ref_config(base_dir, ref_path);
                    let child_wrapper = wrap_one(&child_cfg, dir, false, wrapped, kept)?;
                    let child_name = child_wrapper
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("ref.json");
                    wrapper_refs.push(serde_json::json!({ "path": format!("./{child_name}") }));
                }
            }
        }

        let stem = name.trim_end_matches(".json");
        let mut compiler_options = serde_json::Map::new();
        compiler_options.insert("incremental".into(), serde_json::json!(true));
        compiler_options.insert(
            "tsBuildInfoFile".into(),
            serde_json::json!(format!("./{stem}.tsbuildinfo")),
        );
        if is_composite && !has_root_dir {
            compiler_options.insert("rootDir".into(), serde_json::json!(slash_path(base_dir)));
        }
        if !has_no_emit {
            let out_dir = format!("./.out/{stem}");
            compiler_options.insert("outDir".into(), serde_json::json!(out_dir));
            compiler_options.insert("declarationDir".into(), serde_json::json!(out_dir));
        }

        let mut wrapper = serde_json::Map::new();
        wrapper.insert("extends".into(), serde_json::json!(slash_path(config_path)));
        wrapper.insert(
            "compilerOptions".into(),
            serde_json::Value::Object(compiler_options),
        );
        if !wrapper_refs.is_empty() {
            wrapper.insert("references".into(), serde_json::Value::Array(wrapper_refs));
        }
        write_if_changed(
            &target,
            &serde_json::to_string_pretty(&serde_json::Value::Object(wrapper))?,
        )?;
        Ok(target)
    }

    let solution = wrap_one(
        &root.join("tsconfig.json"),
        &dir,
        true,
        &mut wrapped,
        &mut kept,
    )?;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if name.ends_with(".json") && !kept.contains(name.as_ref()) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    Ok(solution)
}

fn run_tsgo(root: &Path, tsgo: &Path) -> Result<Vec<String>> {
    let solution = write_typecheck_wrappers(root)?;
    let output = Command::new(tsgo)
        .args(["-b", &solution.to_string_lossy(), "--pretty", "false"])
        .current_dir(root)
        .output()
        .with_context(|| format!("run native TypeScript compiler {}", tsgo.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut problems = parse(&stdout);
    problems.extend(parse(&stderr));
    if !output.status.success() && problems.is_empty() {
        let detail = if !stderr.trim().is_empty() {
            stderr.trim()
        } else {
            stdout.trim()
        };
        bail!(
            "TypeScript native compiler ({}) failed: {detail}",
            tsgo.display()
        );
    }
    Ok(problems)
}

fn parse(text: &str) -> Vec<String> {
    let mut problems: Vec<String> = Vec::new();
    for line in text.lines() {
        if let Some(problem) = located(line) {
            problems.push(problem);
        } else if let Some(rest) = line.strip_prefix("error ") {
            problems.push(rest.to_string());
        } else if line.starts_with("  ")
            && let Some(last) = problems.last_mut()
        {
            // A message tsc wraps (an elaborated assignability error) continues indented.
            last.push('\n');
            last.push_str(line);
        }
    }
    problems
}

/// `file(line,col): error TSnnnn: message` as `file:line:col: TSnnnn: message`.
fn located(line: &str) -> Option<String> {
    let (head, message) = line.split_once("): error ")?;
    let (file, position) = head.rsplit_once('(')?;
    let (row, column) = position.split_once(',')?;
    if file.is_empty() || row.parse::<u32>().is_err() || column.parse::<u32>().is_err() {
        return None;
    }
    Some(format!("{file}:{row}:{column}: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tsc_output_reads_as_build_diagnostics() {
        let text = "pages/home/index.tsx(12,7): error TS2322: Type 'number' is not assignable to type 'string'.\n\
pages/home/index.ts(3,1): error TS2345: Argument of type '{ a: number; }' is not assignable.\n  \
Object literal may only specify known properties.\n\
error TS6053: File '.lingxia/run-lingxia-gen-types.d.ts' not found.\n\
Found 3 errors.\n";
        assert_eq!(
            parse(text),
            [
                "pages/home/index.tsx:12:7: TS2322: Type 'number' is not assignable to type 'string'.",
                "pages/home/index.ts:3:1: TS2345: Argument of type '{ a: number; }' is not assignable.\n  Object literal may only specify known properties.",
                "TS6053: File '.lingxia/run-lingxia-gen-types.d.ts' not found.",
            ]
        );
        assert!(parse("").is_empty());
    }

    #[test]
    fn a_missing_typescript_names_the_fix() {
        let temp = tempfile::tempdir().unwrap();
        let error = resolve_tsc(temp.path()).unwrap_err().to_string();
        assert!(error.contains("npm install"), "{error}");
        assert!(error.contains("--no-typecheck"), "{error}");
    }

    #[test]
    fn only_typescript_in_the_supported_range_is_driven() {
        for version in ["5.9.3", "5.9.10", "5.12.0"] {
            assert!(super::in_range(version), "{version}");
        }
        for version in ["5.9.2", "5.8.3", "6.0.0", "7.0.2", "", "5.9"] {
            assert!(!super::in_range(version), "{version}");
        }
    }

    #[test]
    fn tsgo_in_node_modules_runs_natively_without_node_and_parses_errors() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::write(
            root.join("tsconfig.json"),
            r#"{"files":[],"references":[{"path":"./tsconfig.view.json"}]}"#,
        )
        .unwrap();
        let bin = root.join("node_modules").join(".bin");
        std::fs::create_dir_all(&bin).unwrap();
        #[cfg(windows)]
        let tsgo = {
            let p = bin.join("tsgo.cmd");
            std::fs::write(
                &p,
                "@echo off\r\necho pages/home/index.tsx(4,9): error TS2322: Type 'number' is not assignable to type 'string'.\r\nexit /b 2\r\n",
            )
            .unwrap();
            p
        };
        #[cfg(not(windows))]
        let tsgo = {
            use std::os::unix::fs::PermissionsExt;
            let p = bin.join("tsgo");
            std::fs::write(
                &p,
                "#!/bin/sh\necho \"pages/home/index.tsx(4,9): error TS2322: Type 'number' is not assignable to type 'string'.\"\nexit 2\n",
            )
            .unwrap();
            let mut perms = std::fs::metadata(&p).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&p, perms).unwrap();
            p
        };
        assert_eq!(resolve_tsgo(root), Some(tsgo));
        let problems = run(root).unwrap();
        assert_eq!(
            problems,
            vec![
                "pages/home/index.tsx:4:9: TS2322: Type 'number' is not assignable to type 'string'."
            ]
        );
        // Wrappers were generated cleanly under .lingxia/typecheck/.
        assert!(root.join(".lingxia/typecheck/solution.json").is_file());
        assert!(root.join(".lingxia/typecheck/ref-1.json").is_file());
    }

    #[test]
    fn tsgo_wrappers_read_composite_noemit_and_rootdir_through_extends() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let write = |path: &str, text: &str| {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write(
            "tsconfig.json",
            r#"{"files":[],"references":[{"path":"./tsconfig.view.json"},{"path":"./tsconfig.logic.json"},{"path":"./tsconfig.rooted.json"}]}"#,
        );
        write(
            "configs/base.json",
            r#"{"compilerOptions":{"composite":true,"noEmit":true}}"#,
        );
        write(
            "node_modules/@acme/tsconfig/emit.json",
            r#"{"compilerOptions":{"composite":true}}"#,
        );
        write(
            "configs/rooted.json",
            r#"{"extends":"./base","compilerOptions":{"rootDir":"../src"}}"#,
        );
        // Inherits composite and noEmit from a relative base named without `.json`.
        write("tsconfig.view.json", r#"{"extends":"./configs/base"}"#);
        // Inherits composite from a package; the later base's noEmit=false wins.
        write(
            "tsconfig.logic.json",
            r#"{"extends":["./configs/base.json","@acme/tsconfig/emit.json"],"compilerOptions":{"noEmit":false}}"#,
        );
        write(
            "tsconfig.rooted.json",
            r#"{"extends":"./configs/rooted.json"}"#,
        );

        write_typecheck_wrappers(root).unwrap();
        let wrapper = |name: &str| -> serde_json::Value {
            serde_json::from_str(
                &std::fs::read_to_string(root.join(".lingxia/typecheck").join(name)).unwrap(),
            )
            .unwrap()
        };
        let view = wrapper("ref-1.json");
        assert!(view["compilerOptions"]["rootDir"].is_string(), "{view}");
        assert!(view["compilerOptions"].get("outDir").is_none(), "{view}");
        let logic = wrapper("ref-2.json");
        assert!(logic["compilerOptions"]["rootDir"].is_string(), "{logic}");
        assert!(logic["compilerOptions"]["outDir"].is_string(), "{logic}");
        let rooted = wrapper("ref-3.json");
        assert!(
            rooted["compilerOptions"].get("rootDir").is_none(),
            "an inherited rootDir stays the base's: {rooted}"
        );
    }

    #[test]
    fn tsgo_wrappers_preserve_composite_rootdir_nested_references_and_prune_stale() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::write(
            root.join("tsconfig.json"),
            r#"{"files":[],"references":[{"path":"./tsconfig.logic.json"},{"path":"./tsconfig.view.json"}]}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("tsconfig.logic.json"),
            r#"{"compilerOptions":{"composite":true}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("tsconfig.view.json"),
            r#"{"compilerOptions":{"composite":true},"references":[{"path":"./tsconfig.logic.json"}]}"#,
        )
        .unwrap();
        let stale = root.join(".lingxia/typecheck/ref-99.json");
        std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        std::fs::write(&stale, "{}").unwrap();

        let solution = write_typecheck_wrappers(root).unwrap();
        assert!(solution.is_file());
        assert!(!stale.exists(), "stale wrapper must be pruned");
        let view_wrapper: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(".lingxia/typecheck/ref-2.json")).unwrap(),
        )
        .unwrap();
        assert!(
            view_wrapper["compilerOptions"]["rootDir"].is_string(),
            "composite config without rootDir must receive explicit rootDir"
        );
        assert!(
            view_wrapper["compilerOptions"].get("noEmit").is_none(),
            "composite emit config must not force noEmit: true"
        );
        assert_eq!(
            view_wrapper["references"][0]["path"].as_str(),
            Some("./ref-1.json"),
            "nested reference must point to the wrapped logic config"
        );
    }
}
