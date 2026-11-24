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
        return run_tsgo(root, &tsgo);
    }
    let tsc = resolve_tsc(root)?;
    let typescript = supported_typescript(&tsc)?;
    super::ts_worker::typecheck(root, typescript)
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
    let version = std::fs::read(package.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|manifest| manifest["version"].as_str().map(str::to_owned))
        .unwrap_or_default();
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

/// Locates a native TypeScript 7 (`tsgo`) binary when explicitly configured via `LINGXIA_TSGO`
/// or installed in the project's `node_modules`.
pub(crate) fn resolve_tsgo(root: &Path) -> Option<PathBuf> {
    if let Ok(value) = std::env::var("LINGXIA_TSGO") {
        let trimmed = value.trim();
        if !trimmed.is_empty() && !matches!(trimmed, "0" | "false" | "off") {
            let path = PathBuf::from(trimmed);
            if path.is_file() {
                return Some(path);
            }
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

/// Writes `.lingxia/typecheck/` wrapper configs so `tsgo -b` checks with `noEmit: true` and
/// incremental `.tsbuildinfo` without polluting the source tree.
fn write_typecheck_wrappers(root: &Path) -> Result<PathBuf> {
    let dir = root.join(".lingxia").join("typecheck");
    std::fs::create_dir_all(&dir)?;
    let root_config = root.join("tsconfig.json");
    let raw = std::fs::read_to_string(&root_config)
        .with_context(|| format!("read {}", root_config.display()))?;
    let parsed: serde_json::Value =
        json5::from_str(&raw).with_context(|| format!("parse {}", root_config.display()))?;
    let solution = dir.join("solution.json");
    let refs = parsed
        .get("references")
        .and_then(|v| v.as_array())
        .filter(|arr| !arr.is_empty());
    if let Some(refs) = refs {
        let mut wrapper_refs = Vec::with_capacity(refs.len());
        for (index, entry) in refs.iter().enumerate() {
            let Some(ref_path) = entry.get("path").and_then(|v| v.as_str()) else {
                continue;
            };
            let normalized = ref_path.trim_start_matches("./");
            let extends = format!("../../{normalized}");
            let wrapper_name = format!("ref-{index}.json");
            let wrapper_path = dir.join(&wrapper_name);
            let wrapper = serde_json::json!({
                "extends": extends,
                "compilerOptions": {
                    "noEmit": true,
                    "incremental": true,
                    "tsBuildInfoFile": format!("ref-{index}.tsbuildinfo"),
                    "outDir": ".out"
                }
            });
            write_if_changed(&wrapper_path, &serde_json::to_string_pretty(&wrapper)?)?;
            wrapper_refs.push(serde_json::json!({ "path": format!("./{wrapper_name}") }));
        }
        let content = serde_json::json!({
            "files": [],
            "references": wrapper_refs
        });
        write_if_changed(&solution, &serde_json::to_string_pretty(&content)?)?;
    } else {
        let content = serde_json::json!({
            "extends": "../../tsconfig.json",
            "compilerOptions": {
                "noEmit": true,
                "incremental": true,
                "tsBuildInfoFile": "solution.tsbuildinfo",
                "outDir": ".out"
            }
        });
        write_if_changed(&solution, &serde_json::to_string_pretty(&content)?)?;
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
        assert!(root.join(".lingxia/typecheck/ref-0.json").is_file());
    }
}
