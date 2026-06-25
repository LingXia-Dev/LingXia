use super::apple::create_apple_project;
use super::types::ProjectConfig;
use anyhow::Result;

pub fn create_macos_project(config: &ProjectConfig) -> Result<()> {
    create_apple_project(config, "macos", "macos", "macOS")
}
