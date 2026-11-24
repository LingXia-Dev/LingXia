//! Dev sessions: an edit that touches only GPU page View code recompiles those
//! pages and swaps them into the running app, keeping page and Logic state.
//! Anything else (Logic, config, a file no GPU page imports) rebuilds and restarts.

use anyhow::{Context, Result, anyhow};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

type Inputs = BTreeMap<PathBuf, BTreeSet<String>>;
/// Each project's last record: when it was written, its length, and what it resolved to.
type ReadInputs = BTreeMap<PathBuf, (SystemTime, u64, Arc<Inputs>)>;

/// The files the session's last build of `dir` compiled its GPU pages from (`record`), resolved,
/// each with the pages it feeds. A linked package's files sit outside `dir` (`../kit/src/x.tsx`).
/// Resolving them costs a file-system call per input of every page, and the watcher asks on
/// every save: the answer is kept until the build writes the record again.
fn inputs(dir: &Path, record: &Path) -> Option<Arc<Inputs>> {
    static READ: Mutex<ReadInputs> = Mutex::new(BTreeMap::new());
    let written = std::fs::metadata(record)
        .and_then(|record| Ok((record.modified()?, record.len())))
        .ok()?;
    let mut read = READ.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((modified, len, inputs)) = read.get(record)
        && (*modified, *len) == written
    {
        return Some(inputs.clone());
    }
    let text = std::fs::read_to_string(record).ok()?;
    let pages: BTreeMap<String, Vec<String>> = serde_json::from_str(&text).ok()?;
    let mut by_file = Inputs::new();
    for (page, files) in pages {
        for file in files {
            by_file
                .entry(canonical(&dir.join(file)))
                .or_default()
                .insert(page.clone());
        }
    }
    let inputs = Arc::new(by_file);
    read.insert(record.to_path_buf(), (written.0, written.1, inputs.clone()));
    Some(inputs)
}

const CHUNK_KEY: &str = "lx:view";

/// A page deleted since its last build: the record outlives it until the next dev build.
fn built(dir: &Path, page: &str) -> bool {
    page == CHUNK_KEY || dir.join(page).is_file()
}

/// The inputs of `dir`'s GPU pages that lie outside it: linked packages' sources and token files.
pub(super) fn external_inputs(dir: &Path, record: &Path) -> BTreeSet<PathBuf> {
    let root = canonical(dir);
    let Some(inputs) = inputs(dir, record) else {
        return BTreeSet::new();
    };
    let mut pages: BTreeMap<&str, bool> = BTreeMap::new();
    inputs
        .iter()
        .filter(|(file, _)| !file.starts_with(&root))
        .filter(|(_, feeds)| {
            feeds
                .iter()
                .any(|page| *pages.entry(page).or_insert_with(|| built(dir, page)))
        })
        .map(|(file, _)| file.clone())
        .collect()
}

/// The GPU pages to recompile for `changed`, or `None` when the edit needs a full rebuild.
/// Inputs come from the session's last build of `dir` (`record`).
pub(crate) fn plan(dir: &Path, record: &Path, changed: &BTreeSet<PathBuf>) -> Option<Vec<String>> {
    let inputs = inputs(dir, record)?;
    let mut pages = BTreeSet::new();
    for path in changed {
        // Editors save atomically: a temp file written, then renamed over the real one.
        // The temp path is gone by now and the rename's target is in `changed` too.
        let exists = path.exists();
        let path = canonical(path);
        // Configuration and Logic can change what the app is; those reopen it.
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            return None;
        }
        match inputs.get(&path) {
            Some(touched) => {
                if touched.contains(CHUNK_KEY) {
                    // A shared View chunk input changed: rebuild the chunk and swap all built GPU pages.
                    for feeds in inputs.values() {
                        pages.extend(
                            feeds
                                .iter()
                                .filter(|page| page.as_str() != CHUNK_KEY && built(dir, page))
                                .cloned(),
                        );
                    }
                } else {
                    pages.extend(touched.iter().filter(|page| built(dir, page)).cloned());
                }
            }
            None if !exists => continue,
            None => return None,
        }
    }
    (!pages.is_empty()).then(|| pages.into_iter().collect())
}

/// `path` with symlinks resolved; a vanished file resolves through its directory.
pub(super) fn canonical(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| match (path.parent(), path.file_name()) {
            (Some(parent), Some(name)) => canonical(parent).join(name),
            _ => path.to_path_buf(),
        })
}

// A `prime` request compiles into a scratch directory, its record too: it only fills the warm caches.
const COMPILER: &str = "import { buildPages } from '@lingxia/view/compiler';
import { createInterface } from 'node:readline';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
for await (const line of createInterface({ input: process.stdin })) {
  const { pages, output, catalogs, prime, messages, inputsRecord } = JSON.parse(line);
  const target = prime ? await mkdtemp(join(tmpdir(), 'lingxia-prime-')) : output;
  try {
    const { warm, analysed } = await buildPages(process.cwd(), pages, target, catalogs, {
      dev: true, messages, inputsRecord: prime ? join(target, 'gpu-inputs.json') : inputsRecord,
    });
    console.log(JSON.stringify({ ok: true, warm, analysed }));
  } catch (error) { console.log(JSON.stringify({ ok: false, error: String(error?.message ?? error) })); }
  finally { if (prime) await rm(target, { recursive: true, force: true }); }
}";

/// A page compiler kept warm for the session: loading it costs seconds, a page does not.
pub(super) struct Compiler {
    dir: PathBuf,
    output: PathBuf,
    /// The session build's inputs record, which each compile updates.
    record: PathBuf,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    /// Replies to prime requests not read yet; they precede the next compile's reply.
    pending: usize,
}

/// How a compile ran: warm Tailwind, and how many files it analysed afresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Compiled {
    pub warm: bool,
    pub analysed: u64,
}

impl Compiler {
    /// A compiler for the lxapp at `dir` that writes into `output`, the session's build of it.
    pub(super) fn spawn(dir: &Path, output: &Path) -> Result<Self> {
        let record = crate::lxapp::session_inputs_record(output);
        let mut child = Command::new("node")
            .args(["--input-type=module", "--eval", COMPILER])
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("Failed to start the GPU page compiler")?;
        let stdin = child.stdin.take().context("compiler stdin")?;
        let stdout = BufReader::new(child.stdout.take().context("compiler stdout")?);
        let mut compiler = Self {
            dir: dir.to_path_buf(),
            output: output.to_path_buf(),
            record,
            child,
            stdin,
            stdout,
            pending: 0,
        };
        compiler.prime();
        Ok(compiler)
    }

    /// Compile every GPU page of the last dev build into a scratch directory in the background,
    /// so the first save of the session compiles warm.
    fn prime(&mut self) {
        let inputs = std::fs::read_to_string(&self.record)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
        let Some(pages) = inputs.as_ref().and_then(|v| v.as_object()) else {
            return;
        };
        let pages: Vec<String> = pages
            .keys()
            .filter(|page| page.as_str() != CHUNK_KEY)
            .cloned()
            .collect();
        if pages.is_empty() {
            return;
        }
        let Ok(catalogs) = crate::lxapp::gpu_dev_inputs(&self.dir, &pages) else {
            return;
        };
        let request = serde_json::json!({
            "pages": pages,
            "catalogs": catalogs,
            "prime": true,
            "messages": crate::lxapp::dev_messages(&self.dir),
        });
        if writeln!(self.stdin, "{request}").is_ok() {
            self.pending += 1;
        }
    }

    pub(super) fn dir(&self) -> &Path {
        &self.dir
    }

    /// Recompile `pages` (`pages/x/index.tsx`) into the lxapp's build output.
    pub(super) fn compile(&mut self, pages: &[String]) -> Result<Compiled> {
        let catalogs = crate::lxapp::gpu_dev_inputs(&self.dir, pages)?;
        let request = serde_json::json!({
            "pages": pages,
            "output": self.output,
            "catalogs": catalogs,
            "messages": crate::lxapp::dev_messages(&self.dir),
            "inputsRecord": self.record,
        });
        writeln!(self.stdin, "{request}").context("GPU page compiler exited")?;
        let mut line = String::new();
        // A prime still running answers first; its result does not matter.
        while self.pending > 0 {
            line.clear();
            self.stdout.read_line(&mut line)?;
            self.pending -= 1;
        }
        line.clear();
        self.stdout.read_line(&mut line)?;
        let reply: serde_json::Value =
            serde_json::from_str(&line).map_err(|_| anyhow!("GPU page compiler exited"))?;
        if reply["ok"] == true {
            Ok(Compiled {
                warm: reply["warm"] == true,
                analysed: reply["analysed"].as_u64().unwrap_or(0),
            })
        } else {
            Err(anyhow!(
                "{}",
                reply["error"].as_str().unwrap_or("compile failed")
            ))
        }
    }
}

/// What [`Compiler::compile_each`] built.
pub(super) struct Batch {
    /// The last successful compile, if any page built.
    pub compiled: Option<Compiled>,
    pub built: Vec<String>,
    pub failed: Vec<(String, anyhow::Error)>,
}

impl Compiler {
    /// Compile `pages` together; when that fails and there are several, each alone, so one
    /// page that does not build does not hold the others back.
    pub(super) fn compile_each(&mut self, pages: Vec<String>) -> Result<Batch> {
        match self.compile(&pages) {
            Ok(compiled) => Ok(Batch {
                compiled: Some(compiled),
                built: pages,
                failed: Vec::new(),
            }),
            Err(err) if pages.len() < 2 => Err(err),
            Err(_) => {
                let mut batch = Batch {
                    compiled: None,
                    built: Vec::new(),
                    failed: Vec::new(),
                };
                for page in pages {
                    match self.compile(std::slice::from_ref(&page)) {
                        Ok(compiled) => {
                            batch.compiled = Some(compiled);
                            batch.built.push(page);
                        }
                        Err(err) => batch.failed.push((page, err)),
                    }
                }
                Ok(batch)
            }
        }
    }
}

impl Drop for Compiler {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The runtime page path of a page source: `pages/x/index.tsx` is `pages/x/index`.
pub(super) fn page_path(page: &str) -> String {
    page.rsplit_once('.')
        .map_or(page, |(base, _)| base)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An lxapp with one GPU page that resolves `@lingxia/view` from this repository.
    fn gpu_lxapp() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let write = |path: &str, text: &str| {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write(
            "lxapp.json",
            r#"{"appId":"prime-test","appName":"Prime","version":"1.0.0","pages":[{"name":"a","path":"pages/a/index"}]}"#,
        );
        write("package.json", r#"{"name":"prime-test","private":true}"#);
        write(
            "pages/a/index.tsx",
            "export function render() { return <view className=\"p-4 gap-2\"><text className=\"text-sm\">Hello</text></view>; }\n",
        );
        // The session's record of its last build into `out`.
        write(
            "out.gpu-inputs.json",
            r#"{"pages/a/index.tsx":["pages/a/index.tsx"]}"#,
        );
        let packages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/lingxia-view");
        std::fs::create_dir_all(root.join("node_modules/@lingxia")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            packages.canonicalize().unwrap(),
            root.join("node_modules/@lingxia/view"),
        )
        .unwrap();
        dir
    }

    #[cfg(unix)]
    #[test]
    fn a_primed_compiler_compiles_the_first_save_warm() {
        let lxapp = gpu_lxapp();
        let mut compiler = Compiler::spawn(lxapp.path(), &lxapp.path().join("out")).unwrap();
        assert_eq!(
            compiler.pending, 1,
            "spawning primes every GPU page of the last dev build"
        );
        let first = compiler.compile(&["pages/a/index.tsx".to_owned()]).unwrap();
        assert_eq!(
            first,
            Compiled {
                warm: true,
                analysed: 0
            },
            "the first save finds warm caches"
        );
        assert!(
            lxapp.path().join("out/pages/a/index.js").is_file(),
            "the compile still writes its output"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_page_that_does_not_build_leaves_the_others_to_swap() {
        let lxapp = gpu_lxapp();
        std::fs::create_dir_all(lxapp.path().join("pages/b")).unwrap();
        std::fs::write(
            lxapp.path().join("pages/b/index.tsx"),
            "export function render() { return <view>missing</view; }\n",
        )
        .unwrap();
        let mut compiler = Compiler::spawn(lxapp.path(), &lxapp.path().join("out")).unwrap();
        let batch = compiler
            .compile_each(vec![
                "pages/a/index.tsx".to_owned(),
                "pages/b/index.tsx".to_owned(),
            ])
            .unwrap();
        assert!(batch.compiled.is_some());
        assert_eq!(batch.built, ["pages/a/index.tsx"]);
        assert_eq!(batch.failed.len(), 1);
        assert_eq!(batch.failed[0].0, "pages/b/index.tsx");
        assert!(
            format!("{:#}", batch.failed[0].1).contains("pages/b/index.tsx"),
            "{:#}",
            batch.failed[0].1
        );
    }

    #[test]
    fn a_linked_package_edit_swaps_the_pages_that_import_it() {
        let base = tempfile::tempdir().unwrap();
        let app = base.path().join("app");
        let write = |path: &Path| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        };
        for file in [
            "kit/src/chip.tsx",
            "kit/tokens.css",
            "kit/README.md",
            "app/pages/a/index.tsx",
        ] {
            write(&base.path().join(file));
        }
        std::fs::create_dir_all(app.join(".lingxia")).unwrap();
        std::fs::write(
            app.join(".lingxia/gpu-inputs.json"),
            r#"{"pages/a/index.tsx":["pages/a/index.tsx","../kit/src/chip.tsx","../kit/tokens.css"],"pages/b/index.tsx":["pages/b/index.tsx"]}"#,
        )
        .unwrap();
        let kit = canonical(&base.path().join("kit"));
        assert_eq!(
            external_inputs(&app, &app.join(".lingxia/gpu-inputs.json")),
            BTreeSet::from([kit.join("src/chip.tsx"), kit.join("tokens.css")])
        );
        for file in ["kit/src/chip.tsx", "kit/tokens.css"] {
            assert_eq!(
                plan(
                    &app,
                    &app.join(".lingxia/gpu-inputs.json"),
                    &BTreeSet::from([base.path().join(file)])
                ),
                Some(vec!["pages/a/index.tsx".to_owned()]),
                "{file}"
            );
        }
        // Another file of the package is no page input: it rebuilds.
        assert_eq!(
            plan(
                &app,
                &app.join(".lingxia/gpu-inputs.json"),
                &BTreeSet::from([base.path().join("kit/README.md")])
            ),
            None
        );
    }

    #[test]
    fn view_edits_name_their_pages_and_anything_else_rebuilds() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".lingxia")).unwrap();
        std::fs::write(
            dir.path().join(".lingxia/gpu-inputs.json"),
            r#"{"pages/a/index.tsx":["pages/a/index.tsx","shared/ui.tsx","theme/index.css"],"pages/b/index.tsx":["pages/b/index.tsx","shared/ui.tsx"],"pages/gone/index.tsx":["pages/gone/index.tsx","shared/ui.tsx"]}"#,
        )
        .unwrap();
        for page in ["pages/a/index.tsx", "pages/b/index.tsx"] {
            std::fs::create_dir_all(dir.path().join(page).parent().unwrap()).unwrap();
            std::fs::write(dir.path().join(page), "").unwrap();
        }
        let changed = |paths: &[&str]| -> BTreeSet<PathBuf> {
            paths.iter().map(|path| dir.path().join(path)).collect()
        };
        assert_eq!(
            plan(
                dir.path(),
                &dir.path().join(".lingxia/gpu-inputs.json"),
                &changed(&["pages/a/index.tsx"])
            ),
            Some(vec!["pages/a/index.tsx".to_owned()])
        );
        // `pages/gone` was deleted after its last build: a shared edit no longer plans it.
        assert_eq!(
            plan(
                dir.path(),
                &dir.path().join(".lingxia/gpu-inputs.json"),
                &changed(&["shared/ui.tsx"])
            ),
            Some(vec![
                "pages/a/index.tsx".to_owned(),
                "pages/b/index.tsx".to_owned()
            ])
        );
        assert_eq!(
            plan(
                dir.path(),
                &dir.path().join(".lingxia/gpu-inputs.json"),
                &changed(&["theme/index.css", "pages/a/index.tsx"])
            ),
            Some(vec!["pages/a/index.tsx".to_owned()])
        );
        // Logic, config and files no GPU page imports rebuild everything.
        for other in [
            "pages/a/index.ts",
            "pages/a/index.json",
            "lxapp.json",
            "pages/web/index.tsx",
        ] {
            assert_eq!(
                plan(
                    dir.path(),
                    &dir.path().join(".lingxia/gpu-inputs.json"),
                    &changed(&[other])
                ),
                None,
                "{other}"
            );
        }
        // An atomic save: the vanished temp file is skipped, the renamed-to page swaps.
        std::fs::create_dir_all(dir.path().join("pages/a")).unwrap();
        std::fs::write(dir.path().join("pages/a/index.tsx"), "").unwrap();
        assert_eq!(
            plan(
                dir.path(),
                &dir.path().join(".lingxia/gpu-inputs.json"),
                &changed(&[
                    "pages/a/.index.tsx.swp",
                    "pages/a/index.tsx~tmp",
                    "pages/a/index.tsx"
                ])
            ),
            Some(vec!["pages/a/index.tsx".to_owned()])
        );
        assert_eq!(page_path("pages/a/index.tsx"), "pages/a/index");
        assert_eq!(
            plan(
                tempfile::tempdir().unwrap().path(),
                &dir.path().join("none.json"),
                &changed(&["pages/a/index.tsx"])
            ),
            None
        );
    }

    #[test]
    fn asking_again_for_an_unchanged_record_resolves_no_input_twice() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // A gallery's worth: a hundred pages that each compile from the same kit files.
        let kit: Vec<String> = (0..120)
            .map(|i| format!("../kit/src/part{i}.tsx"))
            .collect();
        let mut record = BTreeMap::new();
        for page in 0..100 {
            let source = format!("pages/p{page}/index.tsx");
            std::fs::create_dir_all(root.join(format!("pages/p{page}"))).unwrap();
            std::fs::write(root.join(&source), "").unwrap();
            let mut files = kit.clone();
            files.push(source.clone());
            record.insert(source, files);
        }
        std::fs::create_dir_all(root.join(".lingxia")).unwrap();
        std::fs::write(
            root.join(".lingxia/gpu-inputs.json"),
            serde_json::to_string(&record).unwrap(),
        )
        .unwrap();
        let first = inputs(root, &root.join(".lingxia/gpu-inputs.json")).unwrap();
        let edit = BTreeSet::from([root.join("pages/p7/index.tsx")]);
        for _ in 0..50 {
            assert!(Arc::ptr_eq(
                &first,
                &inputs(root, &root.join(".lingxia/gpu-inputs.json")).unwrap()
            ));
            assert_eq!(
                plan(root, &root.join(".lingxia/gpu-inputs.json"), &edit),
                Some(vec!["pages/p7/index.tsx".to_owned()])
            );
        }
        // The next build writes the record again, and the answer follows it.
        record.remove("pages/p7/index.tsx");
        std::fs::write(
            root.join(".lingxia/gpu-inputs.json"),
            serde_json::to_string(&record).unwrap(),
        )
        .unwrap();
        assert_eq!(
            plan(root, &root.join(".lingxia/gpu-inputs.json"), &edit),
            None
        );
    }

    #[test]
    fn editing_a_shared_view_chunk_input_plans_all_built_gpu_pages() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".lingxia")).unwrap();
        std::fs::create_dir_all(root.join("pages/a")).unwrap();
        std::fs::create_dir_all(root.join("pages/b")).unwrap();
        std::fs::create_dir_all(root.join("kit")).unwrap();
        std::fs::write(root.join("pages/a/index.tsx"), "").unwrap();
        std::fs::write(root.join("pages/b/index.tsx"), "").unwrap();
        std::fs::write(root.join("kit/button.tsx"), "").unwrap();
        std::fs::write(
            root.join(".lingxia/gpu-inputs.json"),
            r#"{"lx:view":["kit/button.tsx"],"pages/a/index.tsx":["pages/a/index.tsx"],"pages/b/index.tsx":["pages/b/index.tsx"]}"#,
        )
        .unwrap();
        let changed = BTreeSet::from([root.join("kit/button.tsx")]);
        assert_eq!(
            plan(root, &root.join(".lingxia/gpu-inputs.json"), &changed),
            Some(vec![
                "pages/a/index.tsx".to_owned(),
                "pages/b/index.tsx".to_owned(),
            ])
        );
    }
}
