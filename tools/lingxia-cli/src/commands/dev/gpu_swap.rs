//! Dev sessions: an edit that touches only GPU page View code recompiles those
//! pages and swaps them into the running app, keeping page and Logic state.
//! Anything else (Logic, config, a file no GPU page imports) rebuilds and restarts.

use anyhow::{Context, Result, anyhow};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// The GPU pages to recompile for `changed`, or `None` when the edit needs a full rebuild.
/// Inputs come from the last dev build (`.lingxia/gpu-inputs.json`).
pub(super) fn plan(dir: &Path, changed: &BTreeSet<PathBuf>) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(dir.join(".lingxia").join("gpu-inputs.json")).ok()?;
    let inputs: BTreeMap<String, Vec<String>> = serde_json::from_str(&text).ok()?;
    let dir = canonical(dir);
    let mut pages = BTreeSet::new();
    for path in changed {
        // Editors save atomically: a temp file written, then renamed over the real one.
        // The temp path is gone by now and the rename's target is in `changed` too.
        let exists = path.exists();
        let path = canonical(path);
        let relative = path
            .strip_prefix(&dir)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/");
        // Configuration and Logic can change what the app is; those reopen it.
        if relative.ends_with(".json") {
            return None;
        }
        let touched: Vec<&String> = inputs
            .iter()
            .filter(|(_, files)| files.contains(&relative))
            .map(|(page, _)| page)
            .collect();
        if touched.is_empty() {
            if !exists {
                continue;
            }
            return None;
        }
        pages.extend(touched.into_iter().cloned());
    }
    (!pages.is_empty()).then(|| pages.into_iter().collect())
}

/// `path` with symlinks resolved; a vanished file resolves through its directory.
fn canonical(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| match (path.parent(), path.file_name()) {
            (Some(parent), Some(name)) => canonical(parent).join(name),
            _ => path.to_path_buf(),
        })
}

const COMPILER: &str = "import { buildPages } from '@lingxia/ui/compiler';
import { createInterface } from 'node:readline';
for await (const line of createInterface({ input: process.stdin })) {
  const { pages, output, catalogs } = JSON.parse(line);
  try { await buildPages(process.cwd(), pages, output, catalogs, { dev: true }); console.log(JSON.stringify({ ok: true })); }
  catch (error) { console.log(JSON.stringify({ ok: false, error: String(error?.message ?? error) })); }
}";

/// A page compiler kept warm for the session: loading it costs seconds, a page does not.
pub(super) struct Compiler {
    dir: PathBuf,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Compiler {
    pub(super) fn spawn(dir: &Path) -> Result<Self> {
        let mut child = Command::new("node")
            .args(["--input-type=module", "--eval", COMPILER])
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("Failed to start the GPU page compiler")?;
        let stdin = child.stdin.take().context("compiler stdin")?;
        let stdout = BufReader::new(child.stdout.take().context("compiler stdout")?);
        Ok(Self {
            dir: dir.to_path_buf(),
            child,
            stdin,
            stdout,
        })
    }

    pub(super) fn dir(&self) -> &Path {
        &self.dir
    }

    /// Recompile `pages` (`pages/x/index.tsx`) into the lxapp's build output.
    pub(super) fn compile(&mut self, pages: &[String]) -> Result<()> {
        let (output, catalogs) = crate::lxapp::gpu_dev_inputs(&self.dir, pages)?;
        let request = serde_json::json!({ "pages": pages, "output": output, "catalogs": catalogs });
        writeln!(self.stdin, "{request}").context("GPU page compiler exited")?;
        let mut line = String::new();
        self.stdout.read_line(&mut line)?;
        let reply: serde_json::Value =
            serde_json::from_str(&line).map_err(|_| anyhow!("GPU page compiler exited"))?;
        if reply["ok"] == true {
            Ok(())
        } else {
            Err(anyhow!(
                "{}",
                reply["error"].as_str().unwrap_or("compile failed")
            ))
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

    #[test]
    fn view_edits_name_their_pages_and_anything_else_rebuilds() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".lingxia")).unwrap();
        std::fs::write(
            dir.path().join(".lingxia/gpu-inputs.json"),
            r#"{"pages/a/index.tsx":["pages/a/index.tsx","shared/ui.tsx","theme/index.css"],"pages/b/index.tsx":["pages/b/index.tsx","shared/ui.tsx"]}"#,
        )
        .unwrap();
        let changed = |paths: &[&str]| -> BTreeSet<PathBuf> {
            paths.iter().map(|path| dir.path().join(path)).collect()
        };
        assert_eq!(
            plan(dir.path(), &changed(&["pages/a/index.tsx"])),
            Some(vec!["pages/a/index.tsx".to_owned()])
        );
        assert_eq!(
            plan(dir.path(), &changed(&["shared/ui.tsx"])),
            Some(vec![
                "pages/a/index.tsx".to_owned(),
                "pages/b/index.tsx".to_owned()
            ])
        );
        assert_eq!(
            plan(
                dir.path(),
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
            assert_eq!(plan(dir.path(), &changed(&[other])), None, "{other}");
        }
        // An atomic save: the vanished temp file is skipped, the renamed-to page swaps.
        std::fs::create_dir_all(dir.path().join("pages/a")).unwrap();
        std::fs::write(dir.path().join("pages/a/index.tsx"), "").unwrap();
        assert_eq!(
            plan(
                dir.path(),
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
                &changed(&["pages/a/index.tsx"])
            ),
            None
        );
    }
}
