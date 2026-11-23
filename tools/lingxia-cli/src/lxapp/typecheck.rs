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
    let typescript = tsc
        .parent()
        .and_then(Path::parent)
        .context("TypeScript install has no package directory")?;
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
}
