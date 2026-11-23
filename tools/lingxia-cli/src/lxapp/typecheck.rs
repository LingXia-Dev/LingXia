//! The project type-check: `tsc -b` at the lxapp root, through the project's own TypeScript.

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

/// Mirrors each project of the root's reference graph as a config under `.lingxia/typecheck/` that
/// extends it, so build info and any emit land there: a write in the project tree would be an edit
/// to a dev session's watcher, and the check would loop.
const WRAP_SCRIPT: &str = r#"
const { createRequire } = await import('node:module');
const path = await import('node:path');
const fs = await import('node:fs');
const [root, typescript] = process.argv.slice(1);
const ts = createRequire(path.join(typescript, 'package.json'))('typescript');
const out = path.join(root, '.lingxia', 'typecheck');
fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });
const rel = file => { const r = path.relative(out, file).split(path.sep).join('/'); return r.startsWith('.') ? r : `./${r}`; };
const wrappers = new Map();
function wrap(config) {
  if (wrappers.has(config)) return wrappers.get(config);
  const name = `${wrappers.size}-${path.basename(config, '.json')}`;
  const wrapper = path.join(out, `${name}.json`);
  wrappers.set(config, wrapper);
  const read = ts.readConfigFile(config, ts.sys.readFile);
  if (read.error) throw Error(ts.flattenDiagnosticMessageText(read.error.messageText, ' '));
  const parsed = ts.parseJsonConfigFileContent(read.config, ts.sys, path.dirname(config), undefined, config);
  const options = parsed.options;
  const compilerOptions = { tsBuildInfoFile: `./${name}.tsbuildinfo` };
  // Only where tsc has a root: a composite project's defaults to its config directory, which the
  // wrapper's location would otherwise move. Sources linked in from outside must stay legal.
  const rootDir = options.rootDir ?? (options.composite ? path.dirname(config) : undefined);
  if (rootDir) compilerOptions.rootDir = rel(rootDir);
  if (!options.noEmit) {
    compilerOptions.outDir = `./${name}.out`;
    if (options.declarationDir) compilerOptions.declarationDir = `./${name}.out`;
  }
  const references = (parsed.projectReferences ?? []).map(reference => ({ path: rel(wrap(ts.resolveProjectReferencePath(reference))) }));
  fs.writeFileSync(wrapper, JSON.stringify({ extends: rel(config), compilerOptions, references }, null, 1));
  return wrapper;
}
process.stdout.write(wrap(path.join(root, 'tsconfig.json')));
"#;

/// Every type error as `file:line:col: TSnnnn: message`; empty when the project checks clean.
pub(crate) fn run(root: &Path) -> Result<Vec<String>> {
    let tsc = resolve_tsc(root)?;
    let typescript = supported_typescript(&tsc)?;
    let wrapped = Command::new("node")
        .args(["--input-type=module", "--eval", WRAP_SCRIPT])
        .arg(root)
        .arg(typescript)
        .current_dir(root)
        .output()
        .context("Failed to start Node.js for the type check")?;
    if !wrapped.status.success() {
        bail!(
            "Type check: cannot read the project's tsconfig:\n{}",
            String::from_utf8_lossy(&wrapped.stderr).trim()
        );
    }
    let entry = String::from_utf8_lossy(&wrapped.stdout).into_owned();
    let output = Command::new("node")
        .arg(&tsc)
        .args(["-b", &entry, "--pretty", "false"])
        .current_dir(root)
        .output()
        .context("Failed to start the TypeScript compiler; is Node.js installed?")?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let problems = parse(&text);
    if !output.status.success() && problems.is_empty() {
        bail!("Type check failed:\n{}", text.trim());
    }
    Ok(problems)
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
/// `package.json`. TypeScript 7 has no JS compiler API.
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
}
