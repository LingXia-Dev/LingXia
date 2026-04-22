use anyhow::{Context, Result};
use sha2::Digest;
use std::fs;
use std::path::Path;

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hex_lower(&hasher.finalize())
}

pub(super) fn hash_tree(root: &Path, ignore_dir_names: &[&str]) -> Result<String> {
    let mut hasher = sha2::Sha256::new();
    hash_tree_inner(root, root, &mut hasher, ignore_dir_names)?;
    Ok(hex_lower(&hasher.finalize()))
}

fn hash_tree_inner(
    root: &Path,
    current: &Path,
    hasher: &mut sha2::Sha256,
    ignore_dir_names: &[&str],
) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(current)
        .with_context(|| format!("Failed to read {}", current.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let file_name_str: &str = file_name.as_ref();
        if path.is_dir() {
            if ignore_dir_names.contains(&file_name_str) {
                continue;
            }
            hash_tree_inner(root, &path, hasher, ignore_dir_names)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");

            hasher.update(rel.as_bytes());
            hasher.update([0]);

            let data =
                fs::read(&path).with_context(|| format!("Failed to read {}", path.display()))?;
            hasher.update((data.len() as u64).to_le_bytes());
            hasher.update([0]);
            hasher.update(&data);
            hasher.update([0]);
        }
    }

    Ok(())
}

pub(super) fn path_key(path: &Path) -> String {
    match path.canonicalize() {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(_) => path.to_string_lossy().to_string(),
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", b);
    }
    out
}
