use crate::lxapp::{framework::is_gpu_page, project::Project};
use anyhow::{Context, Result, bail};
use std::process::{Command, Stdio};

/// Each page's action catalog, read from its Logic.
pub(crate) fn catalogs(
    project: &Project,
    pages: &[&str],
    cached: bool,
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let actions = super::page_actions(project, pages, cached)?;
    Ok(pages
        .iter()
        .map(|page| ((*page).to_owned(), super::bridge_metadata(&actions[*page])))
        .collect())
}

/// `dev` keeps a source map in each page module so runtime errors name the author's line.
pub(super) fn build(project: &Project, dev: bool) -> Result<()> {
    let pages: Vec<&str> = project
        .pages
        .iter()
        .filter(|page| is_gpu_page(&project.root, page))
        .map(String::as_str)
        .collect();
    if pages.is_empty() {
        return Ok(());
    }
    // A pack the lxapp ships must prove its contrast before any page builds against it; it
    // ships completed, so the runtime resolves it without the defaults it was written against.
    let pack = project.root.join(crate::r#gen::tokens::PACK_PATH);
    if pack.exists() {
        let pack = crate::r#gen::tokens::load(Some(&pack))?;
        let output = project.output_dir.join(crate::r#gen::tokens::PACK_PATH);
        std::fs::create_dir_all(output.parent().context("theme pack output")?)?;
        std::fs::write(&output, serde_json::to_string(&pack)?)
            .with_context(|| format!("write {}", output.display()))?;
    }
    let catalogs = catalogs(project, &pages, dev)?;
    let script = "import { buildPages } from '@lingxia/view/compiler'; await buildPages(process.cwd(), JSON.parse(process.argv[1]), process.argv[2], JSON.parse(process.argv[3]), { dev: process.argv[4] === 'dev' });";
    let result = Command::new("node")
        .args(["--input-type=module", "--eval", script])
        .arg(serde_json::to_string(&pages)?)
        .arg(&project.output_dir)
        .arg(serde_json::to_string(&catalogs)?)
        .arg(if dev { "dev" } else { "release" })
        .current_dir(&project.root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to start GPU page compiler; install @lingxia/view in the lxapp")?;
    // Carried in the error so a caller in another terminal (`dev listen`) shows it too.
    let stderr = String::from_utf8_lossy(&result.stderr);
    if !result.status.success() {
        bail!("GPU page compilation failed:\n{}", diagnostic(&stderr));
    }
    eprint!("{stderr}");
    Ok(())
}

/// The compiler's message without Node's source excerpt and stack; all of stderr when it
/// threw nothing recognisable.
fn diagnostic(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().collect();
    let thrown = lines.iter().position(|line| {
        line.split_once(": ")
            .is_some_and(|(name, _)| name.ends_with("Error") && !name.contains(char::is_whitespace))
    });
    let Some(start) = thrown else {
        return stderr.trim().to_string();
    };
    let end = lines[start..]
        .iter()
        .position(|line| line.trim_start().starts_with("at ") && line.starts_with(' '))
        .map_or(lines.len(), |n| start + n);
    let message = lines[start..end].join("\n");
    let message = message.strip_prefix("Error: ").unwrap_or(&message);
    message.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_compile_failure_carries_the_authoring_problems_without_the_stack() {
        let stderr = "file:///repo/packages/lingxia-view/compiler/index.mjs:473
      throw Error(`${page}: ${lines.count} authoring problem`);
            ^

Error: pages/home/index.tsx: 2 authoring problems
pages/home/index.tsx:8:13, 9:13: UI_PROP: testId is not a property of <text>
pages/home/index.tsx:10:15: UI_PROP: testId is not a property of <button>
    at buildPages (file:///repo/packages/lingxia-view/compiler/index.mjs:473:13)
    at async file:///app/[eval1]:1:54

Node.js v25.8.1
";
        assert_eq!(
            super::diagnostic(stderr),
            "pages/home/index.tsx: 2 authoring problems
pages/home/index.tsx:8:13, 9:13: UI_PROP: testId is not a property of <text>
pages/home/index.tsx:10:15: UI_PROP: testId is not a property of <button>"
        );
        assert_eq!(
            super::diagnostic("node: bad option\n"),
            "node: bad option",
            "anything else is shown whole"
        );
        assert!(
            super::diagnostic("SyntaxError: Unexpected token\n    at x\n")
                == "SyntaxError: Unexpected token"
        );
    }
}
