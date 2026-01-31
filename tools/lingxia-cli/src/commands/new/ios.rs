use super::types::ProjectConfig;
use anyhow::Result;
use std::fs;

pub fn create_ios_placeholder(config: &ProjectConfig) -> Result<()> {
    let ios_dir = config.target_dir.join("ios");
    fs::create_dir_all(&ios_dir)?;
    let readme = ios_dir.join("README.md");
    if !readme.exists() {
        fs::write(
            &readme,
            "iOS template is not yet available. This directory is reserved for future use.\n",
        )?;
    }
    println!("  Created iOS placeholder directory");
    Ok(())
}
