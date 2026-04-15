use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

const DEV_DIR_NAME: &str = ".lingxia";
const DEV_INFO_FILE_NAME: &str = "dev.json";

#[derive(Debug, Clone, Deserialize)]
pub struct DevInfo {
    pub ws_url: Option<String>,
    pub log_file: String,
}

pub fn dev_info_path(project_root: &Path) -> PathBuf {
    project_root.join(DEV_DIR_NAME).join(DEV_INFO_FILE_NAME)
}

pub fn read_dev_info(project_root: &Path) -> Result<DevInfo> {
    let path = dev_info_path(project_root);
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
}
