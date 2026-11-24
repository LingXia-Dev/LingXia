//! Shared TypeScript worker process for `typecheck` and `action_types`.
//!
//! Loading `typescript` in Node costs hundreds of milliseconds per cold start. This module keeps
//! one worker per `(project root, typescript package)` warm while a [`SessionGuard`] is live
//! (across a `lingxia dev` session or a single `lingxia build`), and runs one-shot calls in a
//! single Node process otherwise. Wrapper configs under `.lingxia/typecheck/` are updated only
//! when their content changes so incremental `.tsbuildinfo` survives across runs.

use anyhow::{anyhow, bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Newline-delimited JSON worker over stdin/stdout. Loads `typescript` once at startup and serves
/// both `typecheck` (wrapping tsconfigs + `ts.createSolutionBuilder`) and `action_types` requests.
const WORKER_SCRIPT: &str = r#"
const { createRequire } = await import('node:module');
const path = await import('node:path');
const fs = await import('node:fs');
const { createInterface } = await import('node:readline');

const typescriptDir = process.argv[1];
const ts = createRequire(path.join(typescriptDir, 'package.json'))('typescript');

function formatChain(msg, indent = '') {
  if (typeof msg === 'string') return indent + msg;
  let out = indent + msg.messageText;
  for (const next of msg.next ?? []) out += '\n' + formatChain(next, indent + '  ');
  return out;
}

function formatDiagnostic(d, root) {
  const body = `TS${d.code}: ${formatChain(d.messageText)}`;
  if (d.file && d.start !== undefined) {
    const { line, character } = d.file.getLineAndCharacterOfPosition(d.start);
    const file = path.relative(root, d.file.fileName).split(path.sep).join('/');
    return `${file}:${line + 1}:${character + 1}: ${body}`;
  }
  return body;
}

function writeIfChanged(file, content) {
  try {
    if (fs.readFileSync(file, 'utf8') === content) return;
  } catch {}
  fs.writeFileSync(file, content);
}

function handleTypecheck(root) {
  const out = path.join(root, '.lingxia', 'typecheck');
  fs.mkdirSync(out, { recursive: true });
  const rel = file => {
    const r = path.relative(out, file).split(path.sep).join('/');
    return r.startsWith('.') ? r : `./${r}`;
  };
  const wrappers = new Map();
  const kept = new Set();
  function wrap(config) {
    if (wrappers.has(config)) return wrappers.get(config);
    const name = `${wrappers.size}-${path.basename(config, '.json')}`;
    const wrapper = path.join(out, `${name}.json`);
    wrappers.set(config, wrapper);
    kept.add(`${name}.json`);
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
    const references = (parsed.projectReferences ?? []).map(reference => ({
      path: rel(wrap(ts.resolveProjectReferencePath(reference))),
    }));
    const text = JSON.stringify({ extends: rel(config), compilerOptions, references }, null, 1);
    writeIfChanged(wrapper, text);
    return wrapper;
  }
  const entry = wrap(path.join(root, 'tsconfig.json'));
  // Remove obsolete wrapper configs so removed references do not linger, while preserving
  // `.tsbuildinfo` and `.out` for incremental builds.
  for (const existing of fs.readdirSync(out)) {
    if (existing.endsWith('.json') && !kept.has(existing)) {
      fs.rmSync(path.join(out, existing), { force: true });
    }
  }
  const problems = [];
  const host = ts.createSolutionBuilderHost(
    ts.sys,
    undefined,
    diagnostic => {
      if (diagnostic.category === ts.DiagnosticCategory.Error) {
        problems.push(formatDiagnostic(diagnostic, root));
      }
    },
    () => {},
  );
  const builder = ts.createSolutionBuilder(host, [entry], { pretty: false });
  const status = builder.build();
  if (status !== ts.ExitStatus.Success && problems.length === 0) {
    throw Error(`TypeScript exited with status ${status}`);
  }
  return { ok: true, problems };
}

function handleActionTypes(root, logics) {
  const key = file => path.resolve(file).toLowerCase();
  const seen = new Set();
  function project(config, logic) {
    if (seen.has(config + logic) || !ts.sys.fileExists(config)) return null;
    seen.add(config + logic);
    const read = ts.readConfigFile(config, ts.sys.readFile);
    if (read.error) throw Error(ts.flattenDiagnosticMessageText(read.error.messageText, ' '));
    const parsed = ts.parseJsonConfigFileContent(read.config, ts.sys, path.dirname(config), undefined, config);
    if (parsed.fileNames.some(file => key(file) === key(logic))) return parsed;
    for (const reference of parsed.projectReferences ?? []) {
      const found = project(ts.resolveProjectReferencePath(reference), logic);
      if (found) return found;
    }
    return null;
  }
  const pages = {};
  const groups = new Map();
  for (const logic of logics) {
    const parsed = project(path.join(root, 'tsconfig.json'), logic);
    if (!parsed) { pages[logic] = { error: 'unprojected', files: [] }; continue; }
    const id = JSON.stringify(parsed.options) + parsed.fileNames.filter(f => f.endsWith('.d.ts')).join();
    if (!groups.has(id)) groups.set(id, { parsed, logics: [] });
    groups.get(id).logics.push(logic);
  }
  for (const { parsed, logics: groupLogics } of groups.values()) {
    // The modules and the project's ambient declarations; their imports resolve from there.
    const ambient = parsed.fileNames.filter(file => file.endsWith('.d.ts'));
    const options = { ...parsed.options, noEmit: true, composite: false, incremental: false, declaration: false, emitDeclarationOnly: false };
    const program = ts.createProgram({ rootNames: [...groupLogics, ...ambient], options });
    const checker = program.getTypeChecker();
    const byKey = new Map(program.getSourceFiles().map(file => [key(file.fileName), file]));
    const own = file => !file.fileName.split(/[\\/]/).includes('node_modules') && !program.isSourceFileDefaultLibrary(file);
    function graph(start) {
      const files = new Set(ambient.map(file => path.resolve(file)));
      const stack = [start];
      while (stack.length) {
        const file = stack.pop();
        if (!file || !own(file) || (files.has(path.resolve(file.fileName)) && file !== start)) continue;
        files.add(path.resolve(file.fileName));
        for (const specifier of file.imports ?? []) {
          const resolved = program.getResolvedModule?.(file, specifier.text, program.getModeForUsageLocation(file, specifier))?.resolvedModule;
          if (resolved) stack.push(byKey.get(key(resolved.resolvedFileName)));
        }
      }
      return [...files];
    }
    for (const logic of groupLogics) {
      const source = byKey.get(key(logic));
      const files = source ? graph(source) : [];
      const module = source && checker.getSymbolAtLocation(source);
      const exported = module && checker.tryGetMemberInModuleExports('default', module);
      const page = exported && checker.getTypeOfSymbol(exported).getProperty('__lxPage');
      const declared = page && checker.getTypeOfSymbol(page).getProperty('actions');
      if (!declared) { pages[logic] = { error: 'undefined', files }; continue; }
      const actions = checker.getTypeOfSymbol(declared);
      if (actions.flags & (ts.TypeFlags.Any | ts.TypeFlags.Unknown) || checker.getIndexInfosOfType(actions).length) {
        pages[logic] = { error: 'unknown', files };
        continue;
      }
      const found = [];
      let bad;
      for (const property of checker.getPropertiesOfType(actions)) {
        const signatures = checker.getSignaturesOfType(checker.getTypeOfSymbol(property), ts.SignatureKind.Call);
        if (!signatures.length) { bad = property.name; break; }
        // As `View<typeof page>` types it: an action returning an async iterable streams.
        const stream = signatures.some(signature => checker.getReturnTypeOfSignature(signature).getProperties()
          .some(member => String(member.escapedName).startsWith('__@asyncIterator')));
        found.push({ name: property.name, stream });
      }
      pages[logic] = bad ? { error: 'unknown', name: bad, files } : { actions: found, files };
    }
  }
  return { ok: true, pages };
}

for await (const line of createInterface({ input: process.stdin })) {
  if (!line.trim()) continue;
  let req;
  try {
    req = JSON.parse(line);
  } catch (error) {
    process.stdout.write(JSON.stringify({ ok: false, error: String(error?.message ?? error) }) + '\n');
    continue;
  }
  try {
    let reply;
    if (req.op === 'typecheck') {
      reply = handleTypecheck(req.root);
    } else if (req.op === 'action_types') {
      reply = handleActionTypes(req.root, req.logics);
    } else {
      reply = { ok: false, error: `unknown op: ${req.op}` };
    }
    process.stdout.write(JSON.stringify(reply) + '\n');
  } catch (error) {
    process.stdout.write(JSON.stringify({ ok: false, error: String(error?.message ?? error) }) + '\n');
  }
}
"#;

struct Worker {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Worker {
    fn spawn(root: &Path, typescript: &Path) -> Result<Self> {
        let mut child = Command::new("node")
            .args(["--input-type=module", "--eval", WORKER_SCRIPT])
            .arg(typescript)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("Failed to start Node.js TypeScript worker; is Node.js installed?")?;
        let stdin = child.stdin.take().context("ts worker stdin")?;
        let stdout = BufReader::new(child.stdout.take().context("ts worker stdout")?);
        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }

    fn request<Req: Serialize, Res: DeserializeOwned>(&mut self, req: &Req) -> Result<Res> {
        serde_json::to_writer(&mut self.stdin, req)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        let mut line = String::new();
        let bytes = self.stdout.read_line(&mut line)?;
        if bytes == 0 {
            bail!("TypeScript worker exited unexpectedly");
        }
        serde_json::from_str(&line).context("unreadable TypeScript worker reply")
    }

    fn shutdown(mut self) {
        drop(self.stdin);
        let _ = self.child.wait();
    }
}

type WorkerKey = (PathBuf, PathBuf);
static SESSIONS: AtomicUsize = AtomicUsize::new(0);
static POOL: Mutex<BTreeMap<WorkerKey, Arc<Mutex<Option<Worker>>>>> = Mutex::new(BTreeMap::new());

/// Keeps TypeScript workers alive across calls until every active guard is dropped.
pub(crate) struct SessionGuard;

pub(crate) fn keep_alive() -> SessionGuard {
    SESSIONS.fetch_add(1, Ordering::AcqRel);
    SessionGuard
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        if SESSIONS.fetch_sub(1, Ordering::AcqRel) == 1 {
            shutdown_all();
        }
    }
}

pub(crate) fn shutdown_all() {
    let entries: Vec<Arc<Mutex<Option<Worker>>>> = {
        let mut pool = POOL.lock().unwrap_or_else(|p| p.into_inner());
        std::mem::take(&mut *pool).into_values().collect()
    };
    for slot in entries {
        if let Some(worker) = slot.lock().unwrap_or_else(|p| p.into_inner()).take() {
            worker.shutdown();
        }
    }
}

fn canonical(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn call<Req: Serialize, Res: DeserializeOwned>(
    root: &Path,
    typescript: &Path,
    req: &Req,
) -> Result<Res> {
    if SESSIONS.load(Ordering::Acquire) == 0 {
        let mut worker = Worker::spawn(root, typescript)?;
        let res = worker.request(req);
        worker.shutdown();
        return res;
    }
    let key = (canonical(root), canonical(typescript));
    let slot = {
        let mut pool = POOL.lock().unwrap_or_else(|p| p.into_inner());
        pool.entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(None)))
            .clone()
    };
    let mut guard = slot.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(worker) = guard.as_mut() {
        match worker.request(req) {
            Ok(res) => return Ok(res),
            Err(_) => {
                if let Some(dead) = guard.take() {
                    dead.shutdown();
                }
            }
        }
    }
    let mut worker = Worker::spawn(root, typescript)?;
    let res = worker.request(req);
    if res.is_ok() {
        *guard = Some(worker);
    } else {
        worker.shutdown();
    }
    res
}

#[derive(Serialize)]
struct TypecheckReq<'a> {
    op: &'static str,
    root: &'a Path,
}

#[derive(Deserialize)]
struct TypecheckRes {
    ok: bool,
    #[serde(default)]
    problems: Vec<String>,
    #[serde(default)]
    error: Option<String>,
}

/// Runs `tsc -b` via the shared TypeScript worker, preserving `.tsbuildinfo` when wrapper configs
/// are unchanged.
pub(crate) fn typecheck(root: &Path, typescript: &Path) -> Result<Vec<String>> {
    let reply: TypecheckRes = call(
        root,
        typescript,
        &TypecheckReq {
            op: "typecheck",
            root,
        },
    )?;
    if !reply.ok {
        let message = reply.error.unwrap_or_else(|| "unknown error".into());
        bail!("Type check failed:\n{message}");
    }
    Ok(reply.problems)
}

#[derive(Serialize)]
struct ActionTypesReq<'a> {
    op: &'static str,
    root: &'a Path,
    logics: &'a [&'a Path],
}

#[derive(Deserialize)]
struct ActionTypesRes {
    ok: bool,
    #[serde(default)]
    pages: serde_json::Value,
    #[serde(default)]
    error: Option<String>,
}

/// Reads composed page actions through the shared TypeScript worker.
pub(crate) fn action_types(
    root: &Path,
    typescript: &Path,
    logics: &[&Path],
) -> Result<serde_json::Value> {
    let reply: ActionTypesRes = call(
        root,
        typescript,
        &ActionTypesReq {
            op: "action_types",
            root,
            logics,
        },
    )?;
    if !reply.ok {
        let message = reply.error.unwrap_or_else(|| "unknown error".into());
        return Err(anyhow!(message));
    }
    Ok(reply.pages)
}

#[cfg(test)]
fn active_worker_pid(root: &Path, typescript: &Path) -> Option<u32> {
    let key = (canonical(root), canonical(typescript));
    let slot = POOL.lock().unwrap().get(&key)?.clone();
    let guard = slot.lock().unwrap();
    guard.as_ref().map(|worker| worker.child.id())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_typescript() -> Option<PathBuf> {
        let dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/node_modules/typescript");
        dir.is_dir().then_some(dir)
    }

    #[test]
    fn resident_worker_reuses_process_preserves_tsbuildinfo_and_reports_persistent_errors() {
        let Some(ts_dir) = repo_typescript() else {
            return;
        };
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::write(
            root.join("tsconfig.json"),
            r#"{ "compilerOptions": { "strict": true, "noEmit": true, "incremental": true, "target": "ES2020", "module": "ESNext" }, "include": ["src/**/*.ts"] }"#,
        )
        .unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        let file = root.join("src/a.ts");
        std::fs::write(&file, "export const n: number = 'bad';\n").unwrap();

        let guard = keep_alive();
        let first = typecheck(root, &ts_dir).unwrap();
        assert_eq!(first.len(), 1, "{first:?}");
        assert!(first[0].contains("TS2322"), "{first:?}");
        let pid1 = active_worker_pid(root, &ts_dir).expect("resident worker should stay alive");
        let build_info = root.join(".lingxia/typecheck/0-tsconfig.tsbuildinfo");
        assert!(build_info.is_file(), "expected incremental tsbuildinfo");

        // Second run without editing must reuse the same worker PID and still report the error.
        let second = typecheck(root, &ts_dir).unwrap();
        assert_eq!(second.len(), 1, "{second:?}");
        assert!(second[0].contains("TS2322"), "{second:?}");
        let pid2 = active_worker_pid(root, &ts_dir).expect("resident worker should stay alive");
        assert_eq!(pid1, pid2, "resident worker should reuse the same Node PID");

        // Fixing the file clears the error on the same worker.
        std::fs::write(&file, "export const n: number = 42;\n").unwrap();
        let third = typecheck(root, &ts_dir).unwrap();
        assert!(third.is_empty(), "{third:?}");
        assert_eq!(active_worker_pid(root, &ts_dir), Some(pid1));

        drop(guard);
        assert_eq!(active_worker_pid(root, &ts_dir), None);
    }
}
