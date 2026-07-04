use crate::lxapp::project::Project;
use anyhow::{Context, Result, anyhow};
use dirs::cache_dir;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn package_dist(project: &Project) -> Result<PathBuf> {
    if !project.output_dir.exists() {
        return Err(anyhow!(
            "Dist directory not found, cannot package build output: {}",
            project.output_dir.display()
        ));
    }
    let integrity_manifest = project
        .output_dir
        .join(crate::lxapp::hardening::INTEGRITY_MANIFEST);
    if !integrity_manifest.is_file() {
        return Err(anyhow!(
            "Release integrity manifest not found: {}.\nRun `lingxia build --release` before packaging.",
            integrity_manifest.display()
        ));
    }

    let default_name = match project.kind {
        crate::lxapp::project::ProjectKind::LxApp => "lingxia-app",
        crate::lxapp::project::ProjectKind::LxPlugin => "lingxia-plugin",
    };
    let base_name = sanitize_name(project.package_name.as_deref(), default_name);
    let version = sanitize_version(&project.version);
    let archive_name = format!("{base_name}-{version}.tar.zst");
    let cache_root = cache_dir()
        .ok_or_else(|| anyhow!("Failed to locate user cache directory"))?
        .join("lingxia")
        .join("packages");
    fs::create_dir_all(&cache_root)
        .with_context(|| format!("Failed to create {}", cache_root.display()))?;
    let archive_path = cache_root.join(archive_name);

    if archive_path.exists() {
        fs::remove_file(&archive_path)
            .with_context(|| format!("Failed to remove {}", archive_path.display()))?;
    }

    write_tar_zst(&project.output_dir, &archive_path)?;

    Ok(archive_path)
}

/// Build `src_dir` into `archive_path` (`.tar.zst`) via the tar + zstd crates —
/// no external `zstd`, which Windows lacks.
fn write_tar_zst(src_dir: &Path, archive_path: &Path) -> Result<()> {
    let file = fs::File::create(archive_path)
        .with_context(|| format!("Failed to create {}", archive_path.display()))?;
    // Level 0 = zstd's default; single-threaded, matching the prior `zstd -T1`.
    let encoder =
        zstd::stream::write::Encoder::new(file, 0).context("Failed to create zstd encoder")?;
    let mut builder = tar::Builder::new(encoder);

    append_dir_filtered(&mut builder, src_dir, "")?;

    let encoder = builder
        .into_inner()
        .context("Failed to finalize tar archive")?;
    encoder.finish().context("Failed to finalize zstd stream")?;
    Ok(())
}

/// Forward-slash entry paths; skips macOS `._*` / `.DS_Store`.
fn append_dir_filtered(
    builder: &mut tar::Builder<impl Write>,
    src_root: &Path,
    rel: &str,
) -> Result<()> {
    let dir = if rel.is_empty() {
        src_root.to_path_buf()
    } else {
        src_root.join(rel)
    };
    let mut entries = fs::read_dir(&dir)
        .with_context(|| format!("Failed to read {}", dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("._") || name == ".DS_Store" {
            continue;
        }
        let entry_rel = if rel.is_empty() {
            name.to_string()
        } else {
            format!("{rel}/{name}")
        };
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            builder
                .append_dir(&entry_rel, &path)
                .with_context(|| format!("Failed to add dir {}", path.display()))?;
            append_dir_filtered(builder, src_root, &entry_rel)?;
        } else if file_type.is_file() {
            let mut file = fs::File::open(&path)
                .with_context(|| format!("Failed to open {}", path.display()))?;
            builder
                .append_file(&entry_rel, &mut file)
                .with_context(|| format!("Failed to add file {}", path.display()))?;
        }
    }
    Ok(())
}

fn sanitize_name(name: Option<&str>, fallback: &str) -> String {
    let Some(name) = name else {
        return fallback.to_string();
    };
    let cleaned = name
        .trim()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '_',
        })
        .collect::<String>();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned
    }
}

fn sanitize_version(version: &str) -> String {
    let cleaned = version
        .trim()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '_',
        })
        .collect::<String>();
    if cleaned.is_empty() {
        "0.0.0".to_string()
    } else {
        cleaned
    }
}
