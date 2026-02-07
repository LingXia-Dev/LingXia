use std::fs;
use std::path::{Path, PathBuf};

/// Find the nearest Cargo workspace root for `start`.
///
/// If no parent workspace is found, returns `start` unchanged.
pub fn find_workspace_root_or_self(start: &Path) -> PathBuf {
    let mut current = start.to_path_buf();

    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists()
            && let Ok(content) = fs::read_to_string(&cargo_toml)
            && content.contains("[workspace]")
        {
            return current;
        }

        if !current.pop() {
            return start.to_path_buf();
        }
    }
}
