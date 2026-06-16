use super::cache::HostAssetsCache;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const GENERATED_HOST_RESOURCE_FILES: &[&str] = &[
    "app.json",
    "ui.json",
    "bridge-runtime.js",
    "polyfills.es5.js",
];

pub(crate) fn clean_configured_host_assets(project_root: &Path) -> Result<Vec<PathBuf>> {
    let cache = HostAssetsCache::load(project_root);
    let mut removed = Vec::new();

    for (dest_key, stamp) in cache.destinations {
        let target_root = PathBuf::from(dest_key);
        if !is_path_under(project_root, &target_root) {
            continue;
        }

        for file in GENERATED_HOST_RESOURCE_FILES {
            remove_generated_file(&target_root.join(file), &mut removed)?;
        }

        for bundle_name in stamp.bundle_hashes.keys() {
            remove_generated_dir(&target_root.join(bundle_name), &mut removed)?;
        }

        for icon_path in stamp.app_ui_icon_hashes.keys() {
            let path = target_root.join(icon_path);
            remove_generated_file(&path, &mut removed)?;
            remove_empty_parent_dirs_until(&target_root, path.parent());
        }
    }

    remove_generated_dir(
        &project_root.join(".lingxia").join("host-assets"),
        &mut removed,
    )?;

    Ok(removed)
}

fn remove_generated_file(path: &Path, removed: &mut Vec<PathBuf>) -> Result<()> {
    if path.is_file() {
        fs::remove_file(path).with_context(|| format!("Failed to remove {}", path.display()))?;
        removed.push(path.to_path_buf());
    }
    Ok(())
}

fn remove_generated_dir(path: &Path, removed: &mut Vec<PathBuf>) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("Failed to remove {}", path.display()))?;
        removed.push(path.to_path_buf());
    }
    Ok(())
}

fn is_path_under(root: &Path, path: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let canonical = path.canonicalize().or_else(|_| {
        path.parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
            .and_then(|parent| parent.canonicalize())
    });
    canonical.is_ok_and(|path| path.starts_with(root))
}

fn remove_empty_parent_dirs_until(root: &Path, start: Option<&Path>) {
    let Ok(root) = root.canonicalize() else {
        return;
    };
    let mut current = start.map(Path::to_path_buf);
    while let Some(dir) = current {
        let Ok(canonical) = dir.canonicalize() else {
            break;
        };
        if canonical == root || !canonical.starts_with(&root) {
            break;
        }
        let is_empty = fs::read_dir(&canonical)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if !is_empty || fs::remove_dir(&canonical).is_err() {
            break;
        }
        current = canonical.parent().map(Path::to_path_buf);
    }
}
