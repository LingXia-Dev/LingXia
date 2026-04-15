use anyhow::{Context, Result, bail};
use colored::Colorize;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{HOST_CONFIG_FILE, LingXiaConfig, has_host_config};
use crate::http_client;
use crate::lxapp;

pub struct PublishOptions {
    pub token: String,
    pub api_server: Option<String>,
    pub target: Option<String>,
    pub package: Option<String>,
    pub release_type: Option<String>,
    pub framework: Option<String>,
    pub progress: Option<String>,
}

struct PackageMeta {
    target: String,
    target_id: String,
    version: String,
    release_type: Option<String>, // Some only for lxapp
}

struct ResolvedPackage {
    path: PathBuf,
    cleanup_after_publish: bool,
}

impl Drop for ResolvedPackage {
    fn drop(&mut self) {
        if self.cleanup_after_publish {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub fn execute(opts: PublishOptions) -> Result<()> {
    let cwd = env::current_dir()?;

    let meta = resolve_meta(&cwd, opts.target, opts.release_type)?;
    let api_server = resolve_api_server(&cwd, opts.api_server)?;
    let api_server = api_server.trim_end_matches('/').to_string();

    let package =
        resolve_package_for_publish(&cwd, &meta, opts.package, opts.framework, opts.progress)?;
    let package_path = &package.path;
    let file_name = package_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "package".to_string());

    let release_label = meta
        .release_type
        .as_deref()
        .map(|r| format!(" ({r})"))
        .unwrap_or_default();
    println!(
        "{}  Publishing {} {} v{}{} …",
        "→".cyan(),
        meta.target,
        meta.target_id.bold(),
        meta.version.bold(),
        release_label,
    );
    println!("   Package: {}", package_path.display());

    let file_data = fs::read(&package_path)
        .with_context(|| format!("Failed to read package: {}", package_path.display()))?;
    let sha256 = sha256_hex(&file_data);
    println!("   SHA256:  {sha256}");

    let upload_url = format!("{api_server}/api/v1/package/upload");
    println!("   Upload → {upload_url}");

    let mut fields: Vec<(&str, String)> = vec![
        ("target", meta.target.clone()),
        ("targetId", meta.target_id.clone()),
        ("version", meta.version.clone()),
        ("sha256", sha256.clone()),
    ];
    if let Some(rt) = &meta.release_type {
        fields.push(("releaseType", rt.clone()));
    }
    let field_refs: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let boundary = format!("----LingXiaBoundary{}", rand_hex());
    let body = build_multipart(&boundary, &field_refs, &file_name, &file_data);
    let content_type = format!("multipart/form-data; boundary={boundary}");

    let agent = http_client::create_agent(120);
    let mut resp = agent
        .post(&upload_url)
        .header("Authorization", &format!("Bearer {}", opts.token))
        .header("Content-Type", &content_type)
        .send(body.as_slice())
        .with_context(|| format!("HTTP request failed: {upload_url}"))?;

    let status = resp.status().as_u16();
    let body_str = resp
        .body_mut()
        .read_to_string()
        .unwrap_or_else(|_| "<unreadable>".to_string());

    if status == 200 {
        println!("{} Published successfully.", "✓".green().bold());
        Ok(())
    } else {
        bail!("Upload failed (HTTP {status}): {body_str}");
    }
}

fn resolve_package_for_publish(
    cwd: &Path,
    meta: &PackageMeta,
    explicit: Option<String>,
    framework: Option<String>,
    progress: Option<String>,
) -> Result<ResolvedPackage> {
    match meta.target.as_str() {
        "lxapp" | "lxplugin" => {
            if explicit.is_some() {
                bail!(
                    "--package-path is not supported for {}. lingxia publish always packages the current project first.",
                    meta.target
                );
            }
            package_current_project(cwd, framework, progress)
        }
        _ => Ok(ResolvedPackage {
            path: find_or_resolve_package(cwd, &meta.target, explicit)?,
            cleanup_after_publish: false,
        }),
    }
}

fn package_current_project(
    cwd: &Path,
    framework: Option<String>,
    progress: Option<String>,
) -> Result<ResolvedPackage> {
    let mut args = vec!["build".to_string(), "--release".to_string()];
    if let Some(framework) = framework.as_deref() {
        args.push("--framework".to_string());
        args.push(framework.to_string());
    }
    if let Some(progress) = progress.as_deref() {
        args.push("--progress".to_string());
        args.push(progress.to_string());
    }
    lxapp::run_in_dir(&args, cwd)?;
    Ok(ResolvedPackage {
        path: lxapp::package_in_dir(cwd, framework.as_deref())?,
        cleanup_after_publish: true,
    })
}

fn resolve_meta(
    cwd: &Path,
    target_arg: Option<String>,
    release_type_arg: Option<String>,
) -> Result<PackageMeta> {
    let target = match target_arg.as_deref() {
        Some(t) => normalize_target(t)?,
        None => detect_target(cwd)?,
    };

    match target.as_str() {
        "lxapp" => {
            let (id, version) = read_lxapp_json(cwd)?;
            let release_type = release_type_arg
                .as_deref()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "--release-type is required when publishing target=lxapp. Must be one of: release, preview, developer"
                    )
                })
                .and_then(normalize_release_type)?;
            Ok(PackageMeta {
                target,
                target_id: id,
                version,
                release_type: Some(release_type),
            })
        }
        "lxplugin" => {
            let (id, version) = read_lxplugin_json(cwd)?;
            Ok(PackageMeta {
                target,
                target_id: id,
                version,
                release_type: None,
            })
        }
        "app" => {
            let (id, version) = read_app_config(cwd)?;
            Ok(PackageMeta {
                target,
                target_id: id,
                version,
                release_type: None,
            })
        }
        _ => bail!("Unknown target: {target}"),
    }
}

fn detect_target(cwd: &Path) -> Result<String> {
    if cwd.join("lxapp.json").exists() {
        return Ok("lxapp".to_string());
    }
    if cwd.join("lxplugin.json").exists() {
        return Ok("lxplugin".to_string());
    }
    if has_host_config(cwd) {
        return Ok("app".to_string());
    }
    bail!(
        "Could not detect project type. No lxapp.json, lxplugin.json, or {} found.\nUse --target to specify: lxapp, lxplugin, or app.",
        HOST_CONFIG_FILE
    );
}

fn normalize_target(s: &str) -> Result<String> {
    match s.to_lowercase().as_str() {
        "lxapp" => Ok("lxapp".to_string()),
        "lxplugin" | "plugin" => Ok("lxplugin".to_string()),
        "app" => Ok("app".to_string()),
        _ => bail!("Invalid --target '{s}'. Must be one of: lxapp, lxplugin, app"),
    }
}

fn normalize_release_type(s: &str) -> Result<String> {
    match s.to_lowercase().as_str() {
        "release" => Ok("release".to_string()),
        "preview" | "trial" => Ok("preview".to_string()),
        "developer" | "develop" => Ok("developer".to_string()),
        _ => bail!("Invalid --release-type '{s}'. Must be one of: release, preview, developer"),
    }
}

fn read_lxapp_json(cwd: &Path) -> Result<(String, String)> {
    let path = cwd.join("lxapp.json");
    if !path.exists() {
        bail!("lxapp.json not found in {}", cwd.display());
    }
    let val: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path)?).context("Failed to parse lxapp.json")?;
    let id = non_empty_str(&val["appId"], "appId in lxapp.json")?;
    let version = non_empty_str(&val["version"], "version in lxapp.json")?;
    Ok((id, version))
}

fn read_lxplugin_json(cwd: &Path) -> Result<(String, String)> {
    let path = cwd.join("lxplugin.json");
    if !path.exists() {
        bail!("lxplugin.json not found in {}", cwd.display());
    }
    let val: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path)?)
        .context("Failed to parse lxplugin.json")?;
    let id = non_empty_str(&val["lxPluginId"], "lxPluginId in lxplugin.json")?;
    let version = non_empty_str(&val["version"], "version in lxplugin.json")?;
    Ok((id, version))
}

fn read_app_config(cwd: &Path) -> Result<(String, String)> {
    let cfg = LingXiaConfig::load(cwd)?;
    let app = cfg.app.context("app section missing in lingxia.yaml")?;

    let target_id = app
        .lingxia_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .context("app.lingxiaId is required in lingxia.yaml when publishing target=app")?;

    let version = app.product_version;
    if version.trim().is_empty() {
        bail!("productVersion is empty in lingxia.yaml");
    }

    Ok((target_id, version))
}

fn non_empty_str(val: &serde_json::Value, label: &str) -> Result<String> {
    let s = val.as_str().unwrap_or("").trim().to_string();
    if s.is_empty() {
        bail!("{label} is missing or empty");
    }
    Ok(s)
}

fn resolve_api_server(cwd: &Path, api_server_arg: Option<String>) -> Result<String> {
    if let Some(s) = api_server_arg {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            bail!("--api-server cannot be empty");
        }
        return Ok(trimmed.to_string());
    }
    let config_path = cwd.join(HOST_CONFIG_FILE);
    if config_path.exists() {
        if let Ok(cfg) = LingXiaConfig::load(cwd) {
            if let Some(url) = cfg.app.and_then(|a| a.api_server) {
                let trimmed = url.trim();
                if !trimmed.is_empty() {
                    return Ok(trimmed.to_string());
                }
            }
        }
    }
    bail!("Use --api-server to specify the package upload server URL.");
}

fn find_or_resolve_package(cwd: &Path, target: &str, explicit: Option<String>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        let path = if Path::new(&p).is_absolute() {
            PathBuf::from(p)
        } else {
            cwd.join(p)
        };
        if !path.exists() {
            bail!("Package not found: {}", path.display());
        }
        if !path.is_file() {
            bail!("Package is not a file: {}", path.display());
        }
        return Ok(path);
    }

    let mut candidates = Vec::new();
    let dist_dir = cwd.join("dist");
    collect_matching_packages(cwd, &dist_dir, target, &mut candidates, 0);
    collect_matching_packages(
        &dist_dir,
        &dist_dir,
        target,
        &mut candidates,
        MAX_PACKAGE_SEARCH_DEPTH,
    );
    candidates.sort();
    candidates.dedup();

    match candidates.len() {
        0 => bail!(
            "No package found for target '{target}'. Run 'lingxia package' first, or use --package-path <PATH>."
        ),
        1 => Ok(candidates.remove(0)),
        _ => {
            let list = candidates
                .iter()
                .map(|p| format!("  {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n");
            bail!("Multiple packages found. Use --package-path <PATH> to specify one:\n{list}")
        }
    }
}

const MAX_PACKAGE_SEARCH_DEPTH: u32 = 3;

fn collect_matching_packages(
    dir: &Path,
    dist_dir: &Path,
    target: &str,
    out: &mut Vec<PathBuf>,
    max_depth: u32,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if max_depth > 0 {
                collect_matching_packages(&path, dist_dir, target, out, max_depth - 1);
            }
            continue;
        }
        if !path.is_file() {
            continue;
        }

        if package_matches(target, &path, dist_dir) {
            out.push(path);
        }
    }
}

fn package_matches(target: &str, path: &Path, dist_dir: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    match target {
        "app" => {
            if file_name.ends_with(".apk")
                || file_name.ends_with(".ipa")
                || file_name.ends_with(".hap")
            {
                return true;
            }

            file_name.ends_with("-macos.zip") && path.starts_with(dist_dir.join("macos"))
        }
        _ => false,
    }
}

fn sha256_hex(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

fn rand_hex() -> String {
    use rand::RngExt;
    let bytes: [u8; 8] = rand::rng().random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn build_multipart(
    boundary: &str,
    fields: &[(&str, &str)],
    file_name: &str,
    file_data: &[u8],
) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"package\"; filename=\"{file_name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(file_data);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

#[cfg(test)]
mod tests {
    use super::{find_or_resolve_package, package_matches};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn app_package_match_accepts_cli_generated_macos_zip_only_in_dist_macos() {
        let temp = TempDir::new().unwrap();
        let dist_dir = temp.path().join("dist");
        let allowed = dist_dir.join("macos").join("Demo-1.0.0-macos.zip");
        let rejected = temp.path().join("Demo-1.0.0-macos.zip");

        fs::create_dir_all(allowed.parent().unwrap()).unwrap();
        fs::write(&allowed, b"zip").unwrap();
        fs::write(&rejected, b"zip").unwrap();

        assert!(package_matches("app", &allowed, &dist_dir));
        assert!(!package_matches("app", &rejected, &dist_dir));
    }

    #[test]
    fn find_or_resolve_package_ignores_unrelated_root_zip_for_app_publish() {
        let temp = TempDir::new().unwrap();
        let dist_macos = temp.path().join("dist").join("macos");
        fs::create_dir_all(&dist_macos).unwrap();
        fs::write(temp.path().join("notes.zip"), b"zip").unwrap();

        let expected = dist_macos.join("Demo-1.0.0-macos.zip");
        fs::write(&expected, b"zip").unwrap();

        let resolved = find_or_resolve_package(temp.path(), "app", None).unwrap();
        assert_eq!(resolved, expected);
    }
}
