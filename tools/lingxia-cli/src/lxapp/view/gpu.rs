use crate::lxapp::{framework::is_gpu_page, project::Project};
use anyhow::{Context, Result, bail};
use std::process::Command;

pub(super) fn build(project: &Project) -> Result<()> {
    let pages: Vec<&str> = project
        .pages
        .iter()
        .filter(|page| is_gpu_page(page))
        .map(String::as_str)
        .collect();
    if pages.is_empty() {
        return Ok(());
    }
    let mut catalogs = serde_json::Map::new();
    for page in &pages {
        let logic = super::page_logic_path(project, page)?;
        let actions = super::extract_page_actions(logic.as_deref())?;
        catalogs.insert((*page).to_owned(), super::bridge_metadata(&actions));
    }
    let script = "import { buildPages } from '@lingxia/ui/compiler'; await buildPages(process.cwd(), JSON.parse(process.argv[1]), process.argv[2], JSON.parse(process.argv[3]));";
    let result = Command::new("node")
        .args(["--input-type=module", "--eval", script])
        .arg(serde_json::to_string(&pages)?)
        .arg(&project.output_dir)
        .arg(serde_json::to_string(&catalogs)?)
        .current_dir(&project.root)
        .status()
        .context("Failed to start GPU page compiler; install @lingxia/ui in the lxapp")?;
    if !result.success() {
        bail!("GPU page compilation failed; see the compiler diagnostic above");
    }
    Ok(())
}
