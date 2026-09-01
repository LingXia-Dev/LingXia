//! Exact cleanup for launchers written by the pre-product-owned discovery
//! design. Never remove the directory recursively: `app_state` belongs to the
//! host and may contain files LingXia did not create.

use std::path::{Path, PathBuf};

const RECORDED_NAME: &str = ".lingxia-launcher-name";

pub(super) fn cleanup(app_state_dir: &Path) -> std::io::Result<()> {
    let directory = app_state_dir.join("bin");
    let mut first_error = None;
    if let Some(name) = recorded_name(app_state_dir) {
        for file_name in artifact_file_names(&name) {
            remove_file(directory.join(file_name), &mut first_error);
        }
    }
    remove_file(app_state_dir.join(RECORDED_NAME), &mut first_error);
    remove_file(app_state_dir.join("control.sock"), &mut first_error);

    // Succeeds only when the old directory is empty. Unknown host files keep
    // it in place and are never interpreted as ours.
    if let Err(error) = std::fs::remove_dir(&directory)
        && error.kind() != std::io::ErrorKind::NotFound
        && error.kind() != std::io::ErrorKind::DirectoryNotEmpty
        && first_error.is_none()
    {
        first_error = Some(error);
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn recorded_name(app_state_dir: &Path) -> Option<String> {
    let value = std::fs::read_to_string(app_state_dir.join(RECORDED_NAME)).ok()?;
    let value = value.trim();
    (!value.is_empty() && command_name(value) == value).then(|| value.to_string())
}

fn command_name(product_name: &str) -> String {
    let mut name = String::new();
    for character in product_name.chars() {
        if character.is_ascii_alphanumeric() {
            name.push(character.to_ascii_lowercase());
        } else if !name.is_empty() && !name.ends_with('-') {
            name.push('-');
        }
    }
    let name = name.trim_end_matches('-');
    if name.is_empty() {
        "app".to_string()
    } else {
        name.to_string()
    }
}

#[cfg(not(windows))]
fn artifact_file_names(name: &str) -> Vec<String> {
    vec![name.to_string()]
}

#[cfg(windows)]
fn artifact_file_names(name: &str) -> Vec<String> {
    vec![
        format!("{name}.exe"),
        format!("{name}.control"),
        format!("{name}.cmd"),
        format!("{name}.ps1"),
    ]
}

fn remove_file(path: PathBuf, first_error: &mut Option<std::io::Error>) {
    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
        && first_error.is_none()
    {
        *first_error = Some(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_removes_only_recorded_legacy_files() {
        let root = std::env::temp_dir().join(format!(
            "lingxia-legacy-control-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(root.join(RECORDED_NAME), "old-product").unwrap();
        for artifact in artifact_file_names("old-product") {
            std::fs::write(bin.join(artifact), b"legacy").unwrap();
        }
        let host_file = bin.join("host-owned");
        std::fs::write(&host_file, b"keep").unwrap();

        cleanup(&root).unwrap();

        assert!(host_file.exists());
        assert!(!root.join(RECORDED_NAME).exists());
        for artifact in artifact_file_names("old-product") {
            assert!(!bin.join(artifact).exists());
        }
        let _ = std::fs::remove_dir_all(root);
    }
}
