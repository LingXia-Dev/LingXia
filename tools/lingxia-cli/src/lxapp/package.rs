use crate::lxapp::project::Project;
use anyhow::{Context, Result, anyhow};
use dirs::cache_dir;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn package_dist(project: &Project) -> Result<PathBuf> {
    if !project.output_dir.exists() {
        return Err(anyhow!(
            "Dist directory not found, cannot package build output: {}",
            project.output_dir.display()
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

    run_tar(
        &[
            "--exclude=._*",
            "--exclude=.DS_Store",
            "--use-compress-program",
            "zstd -T1",
            "-cf",
            archive_path
                .to_str()
                .ok_or_else(|| anyhow!("Invalid archive path"))?,
            ".",
        ],
        &project.output_dir,
    )?;

    Ok(archive_path)
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

fn run_tar(args: &[&str], cwd: &Path) -> Result<()> {
    let status = Command::new("tar")
        .args(args)
        .current_dir(cwd)
        .env("COPYFILE_DISABLE", "1")
        .env("ZSTD_NBTHREADS", "1")
        .env("ZSTD_DEFAULT_NBTHREADS", "1")
        .status()
        .with_context(|| format!("Failed to execute tar in {}", cwd.display()))?;

    if !status.success() {
        return Err(anyhow!("tar exited with status {status}"));
    }

    Ok(())
}
