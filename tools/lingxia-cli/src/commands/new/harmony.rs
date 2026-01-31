use super::types::ProjectConfig;
use anyhow::Result;
use std::fs;

pub fn create_harmony_placeholder(config: &ProjectConfig) -> Result<()> {
    let harmony_dir = config.target_dir.join("harmony");
    fs::create_dir_all(&harmony_dir)?;
    let readme = harmony_dir.join("README.md");
    if !readme.exists() {
        fs::write(
            &readme,
            "HarmonyOS template is not yet available. This directory is reserved for future use.\n",
        )?;
    }
    println!("  Created HarmonyOS placeholder directory");
    Ok(())
}
