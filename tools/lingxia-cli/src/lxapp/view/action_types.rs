//! Page actions from the type checker, for actions a page's source does not spell out.
//!
//! One program answers every page asked in a build. Outside a release build each answer is
//! kept in `.lingxia/action-types.json` with a content hash of the module graph it was read
//! from, so a dev save re-asks only the pages whose graph changed.

use super::super::typecheck;
use crate::lxapp::framework::{PageAction, PageActionMode, page_base};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Reads `__lxPage.actions` of each Logic module's default export through the project's own
/// TypeScript, in the tsconfig project that includes the modules.
const SCRIPT: &str = r#"
const { createRequire } = await import('node:module');
const path = await import('node:path');
const [root, typescript, logicJson] = process.argv.slice(1);
const ts = createRequire(path.join(typescript, 'package.json'))('typescript');
const logics = JSON.parse(logicJson);
const key = file => path.resolve(file).toLowerCase();
const seen = new Set();
function project(config, logic) {
  if (seen.has(config + logic)) return null;
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
for (const { parsed, logics } of groups.values()) {
  // The modules and the project's ambient declarations; their imports resolve from there.
  const ambient = parsed.fileNames.filter(file => file.endsWith('.d.ts'));
  const options = { ...parsed.options, noEmit: true, composite: false, incremental: false, declaration: false, emitDeclarationOnly: false };
  const program = ts.createProgram({ rootNames: [...logics, ...ambient], options });
  const checker = program.getTypeChecker();
  const byKey = new Map(program.getSourceFiles().map(file => [key(file.fileName), file]));
  const own = file => !file.fileName.split(/[\\/]/).includes('node_modules') && !program.isSourceFileDefaultLibrary(file);
  // The project's own files a module reaches; a change to any of them can change its actions.
  function graph(start) {
    const files = new Set(ambient.map(file => path.resolve(file)));
    const stack = [start];
    while (stack.length) {
      const file = stack.pop();
      if (!file || !own(file) || files.has(path.resolve(file.fileName)) && file !== start) continue;
      files.add(path.resolve(file.fileName));
      for (const specifier of file.imports ?? []) {
        const resolved = program.getResolvedModule?.(file, specifier.text, program.getModeForUsageLocation(file, specifier))?.resolvedModule;
        if (resolved) stack.push(byKey.get(key(resolved.resolvedFileName)));
      }
    }
    return [...files];
  }
  for (const logic of logics) {
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
process.stdout.write(JSON.stringify(pages));
"#;

const CACHE: &str = "action-types.json";

#[derive(Clone, Serialize, Deserialize)]
struct Reply {
    #[serde(default)]
    actions: Vec<Found>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    name: Option<String>,
    files: Vec<PathBuf>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Found {
    name: String,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct Cached {
    /// Each file of the graph and the SHA-256 of its content.
    hashes: BTreeMap<PathBuf, String>,
    reply: Reply,
}

#[derive(Default, Serialize, Deserialize)]
struct Cache {
    /// The CLI and TypeScript versions the answers came from; any other reads as empty.
    version: String,
    pages: BTreeMap<PathBuf, Cached>,
}

/// A page whose actions the type checker must read.
pub(crate) struct Composed<'a> {
    pub page: &'a str,
    pub logic: PathBuf,
    /// What the page wrote for its actions, quoted when they cannot be known.
    pub expression: String,
}

fn hash(file: &Path) -> Option<String> {
    let bytes = std::fs::read(file).ok()?;
    let digest = sha2::Sha256::digest(&bytes);
    Some(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// The tsconfigs that decide how the graph resolves.
fn configs(root: &Path) -> Vec<PathBuf> {
    [root.to_path_buf(), root.join(".lingxia")]
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("tsconfig") && name.ends_with(".json"))
        })
        .collect()
}

fn fresh(cached: &Cached) -> bool {
    !cached.hashes.is_empty()
        && cached
            .hashes
            .iter()
            .all(|(file, digest)| hash(file).as_deref() == Some(digest.as_str()))
}

/// The actions of each page, by page path, asking TypeScript once for every page not answered.
/// `cached` lets an answer whose module graph is unchanged stand; a release build passes false.
pub(crate) fn actions(
    root: &Path,
    composed: &[Composed<'_>],
    cached: bool,
) -> Result<HashMap<String, Vec<PageAction>>> {
    if composed.is_empty() {
        return Ok(HashMap::new());
    }
    let first_page = page_base(composed[0].page);
    let tsc = typecheck::resolve_tsc(root).with_context(|| {
        format!(
            "{first_page}: its actions are composed (`{}`), so the build reads them with TypeScript",
            composed[0].expression
        )
    })?;
    let typescript = tsc
        .parent()
        .and_then(Path::parent)
        .context("TypeScript install has no package directory")?;
    let ts_version = std::fs::read(typescript.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|package| package["version"].as_str().map(str::to_owned))
        .unwrap_or_default();
    let version = format!("{}+typescript-{ts_version}", env!("CARGO_PKG_VERSION"));
    let cache_path = root.join(".lingxia").join(CACHE);
    let mut cache: Cache = std::fs::read(&cache_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Cache>(&bytes).ok())
        .filter(|cache| cached && cache.version == version)
        .unwrap_or_default();
    cache.version = version;
    let stale: Vec<&Composed<'_>> = composed
        .iter()
        .filter(|page| !cache.pages.get(&page.logic).is_some_and(fresh))
        .collect();
    let mut answers: HashMap<PathBuf, Reply> = HashMap::new();
    if !stale.is_empty() {
        let logics: Vec<&Path> = stale.iter().map(|page| page.logic.as_path()).collect();
        let output = Command::new("node")
            .args(["--input-type=module", "--eval", SCRIPT])
            .arg(root)
            .arg(typescript)
            .arg(serde_json::to_string(&logics)?)
            .current_dir(root)
            .output()
            .context("Failed to start Node.js to read page actions")?;
        if !output.status.success() {
            bail!(
                "{first_page}: cannot read page actions with TypeScript:\n{}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        answers = serde_json::from_slice(&output.stdout).context("unreadable page action types")?;
        let configs = configs(root);
        for page in &stale {
            let Some(reply) = answers.get(&page.logic) else {
                continue;
            };
            let hashes = reply
                .files
                .iter()
                .chain(&configs)
                .filter_map(|file| Some((file.clone(), hash(file)?)))
                .collect();
            cache.pages.insert(
                page.logic.clone(),
                Cached {
                    hashes,
                    reply: reply.clone(),
                },
            );
        }
        if let Some(dir) = cache_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&cache_path, serde_json::to_vec(&cache)?);
    }
    composed
        .iter()
        .map(|page| {
            let reply = answers
                .remove(&page.logic)
                .or_else(|| {
                    cache
                        .pages
                        .get(&page.logic)
                        .map(|cached| cached.reply.clone())
                })
                .with_context(|| {
                    format!(
                        "{}: TypeScript gave no answer for its actions",
                        page_base(page.page)
                    )
                })?;
            Ok((page.page.to_string(), answer(page, reply)?))
        })
        .collect()
}

fn answer(page: &Composed<'_>, reply: Reply) -> Result<Vec<PageAction>> {
    let (name, expression) = (page_base(page.page), &page.expression);
    match reply.error.as_deref() {
        None => Ok(reply
            .actions
            .into_iter()
            .map(|found| PageAction {
                name: found.name,
                mode: if found.stream {
                    PageActionMode::Stream
                } else {
                    PageActionMode::Call
                },
            })
            .collect()),
        Some("unknown") => bail!(
            "{name}: actions must be statically known: write them in the object, or type the helper's return ({})",
            match reply.name {
                Some(action) => format!("`{expression}`: `{action}` is not a function"),
                None => format!("`{expression}` has no fixed names"),
            }
        ),
        Some("unprojected") => bail!(
            "{name}: its actions are composed (`{expression}`), but no tsconfig project includes {}; add it to the Logic tsconfig's `include`",
            page.logic.display()
        ),
        Some(_) => bail!(
            "{name}: the Logic module must `export default definePage({{ … }})` from '@lingxia/logic'"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_stands_only_while_its_graph_has_the_same_content() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("shared.ts");
        std::fs::write(&file, "export const a = 1;").unwrap();
        let when = std::fs::metadata(&file).unwrap().modified().unwrap();
        let cached = Cached {
            hashes: BTreeMap::from([(file.clone(), hash(&file).unwrap())]),
            reply: Reply {
                actions: Vec::new(),
                error: None,
                name: None,
                files: vec![file.clone()],
            },
        };
        assert!(fresh(&cached));
        // Same length, same modification time, different content.
        std::fs::write(&file, "export const b = 1;").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(when)
            .unwrap();
        assert!(!fresh(&cached));
        std::fs::remove_file(&file).unwrap();
        assert!(!fresh(&cached));
    }
}
