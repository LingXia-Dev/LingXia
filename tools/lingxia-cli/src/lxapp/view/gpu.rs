use crate::lxapp::{framework::is_gpu_page, project::Project};
use anyhow::{Context, Result, bail};
use std::process::Command;

/// Each page's action catalog, read from its Logic.
pub(crate) fn catalogs(
    project: &Project,
    pages: &[&str],
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let mut catalogs = serde_json::Map::new();
    for page in pages {
        let logic = super::page_logic_path(project, page)?;
        let actions = super::extract_page_actions(logic.as_deref())?;
        catalogs.insert((*page).to_owned(), super::bridge_metadata(&actions));
    }
    Ok(catalogs)
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
    // A pack the lxapp ships must prove its contrast before any page builds against it.
    let pack = project.root.join(crate::r#gen::tokens::PACK_PATH);
    if pack.exists() {
        crate::r#gen::tokens::load(Some(&pack))?;
    }
    let catalogs = catalogs(project, &pages)?;
    let script = "import { buildPages } from '@lingxia/ui/compiler'; await buildPages(process.cwd(), JSON.parse(process.argv[1]), process.argv[2], JSON.parse(process.argv[3]), { dev: process.argv[4] === 'dev' });";
    let result = Command::new("node")
        .args(["--input-type=module", "--eval", script])
        .arg(serde_json::to_string(&pages)?)
        .arg(&project.output_dir)
        .arg(serde_json::to_string(&catalogs)?)
        .arg(if dev { "dev" } else { "release" })
        .current_dir(&project.root)
        .status()
        .context("Failed to start GPU page compiler; install @lingxia/ui in the lxapp")?;
    if !result.success() {
        bail!("GPU page compilation failed; see the compiler diagnostic above");
    }
    Ok(())
}
