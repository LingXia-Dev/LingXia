use anyhow::{Context, Result, bail};
use colored::Colorize;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::config::{EnvVersion, HOST_CONFIG_FILE, LingXiaConfig, has_host_config};
use crate::http_client;
use crate::lxapp;
use crate::platform::detector::PlatformType;

pub struct PublishOptions {
    pub token: String,
    pub lingxia_server: Option<String>,
    pub package: Option<String>,
    pub platform: Option<String>,
    pub channel: Option<String>,
    pub framework: Option<String>,
    pub progress: Option<String>,
}

#[derive(Debug)]
struct PackageMeta {
    target: String,
    target_id: String,
    version: String,
    channel: Option<String>,
}

struct ResolvedPackage {
    path: PathBuf,
    platform: Option<String>,
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

    let mut meta = resolve_meta(&cwd, opts.channel.as_deref())?;
    let package = resolve_package_for_publish(
        &cwd,
        &meta,
        opts.package,
        opts.platform,
        opts.framework,
        opts.progress,
    )?;
    let package_path = &package.path;
    if meta.target == "app" {
        let metadata = read_app_package_metadata(package_path).with_context(|| {
            format!(
                "Failed to read app package metadata from {}",
                package_path.display()
            )
        })?;
        meta.channel = Some(metadata.env_version);
        // The runtime uses the suffixed lingxiaId from the baked-in app.json
        // for update checks. Mirror that on the server side so id matches,
        // otherwise developer/preview packages would upload to the base id but
        // clients would poll for the suffixed one.
        if let Some(id) = metadata.lingxia_id {
            meta.target_id = id;
        }
    } else if meta.channel.is_none() {
        meta.channel = Some("release".to_string());
    }
    // Resolve server *after* channel is known so we can prefer the per-env
    // server from app.environments.<channel>.lingxiaServer.
    let lingxia_server =
        resolve_lingxia_server(&cwd, meta.channel.as_deref(), opts.lingxia_server)?;
    let lingxia_server = lingxia_server.trim_end_matches('/').to_string();
    let file_name = package_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "package".to_string());

    let channel_label = meta
        .channel
        .as_deref()
        .map(|c| format!(" ({c})"))
        .unwrap_or_default();
    println!(
        "{}  Publishing {} {} v{}{} …",
        "→".cyan(),
        meta.target,
        meta.target_id.bold(),
        meta.version.bold(),
        channel_label,
    );
    println!("   Package: {}", package_path.display());

    let file_data = fs::read(&package_path)
        .with_context(|| format!("Failed to read package: {}", package_path.display()))?;
    let sha256 = sha256_hex(&file_data);
    println!("   SHA256:  {sha256}");

    let upload_url = format!("{lingxia_server}/api/v1/package/upload");
    println!("   Upload → {upload_url}");

    let mut fields: Vec<(&str, String)> = vec![
        ("kind", meta.target.clone()),
        ("id", meta.target_id.clone()),
        ("version", meta.version.clone()),
        ("sha256", sha256.clone()),
    ];
    if let Some(ch) = &meta.channel {
        fields.push(("channel", ch.clone()));
    }
    if let Some(platform) = package.platform.as_deref() {
        fields.push(("platform", platform.to_string()));
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
        .map_err(|err| upload_transport_error(&upload_url, file_data.len(), err))?;

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
    platform: Option<String>,
    framework: Option<String>,
    progress: Option<String>,
) -> Result<ResolvedPackage> {
    match meta.target.as_str() {
        "lxapp" | "lxplugin" => {
            if platform.is_some() {
                bail!("--platform is only supported when publishing target=app.");
            }
            if explicit.is_some() {
                bail!(
                    "--package-path is not supported for {}. lingxia publish always packages the current project first.",
                    meta.target
                );
            }
            package_current_project(cwd, framework, progress)
        }
        _ => {
            let platform = resolve_publish_platform(cwd, &meta.target, platform.as_deref())?;
            Ok(ResolvedPackage {
                path: find_or_resolve_package(cwd, &meta.target, explicit, platform.as_deref())?,
                platform,
                cleanup_after_publish: false,
            })
        }
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
        platform: None,
        cleanup_after_publish: true,
    })
}

fn resolve_meta(cwd: &Path, channel_arg: Option<&str>) -> Result<PackageMeta> {
    let target = detect_target(cwd)?;

    match target.as_str() {
        "lxapp" => {
            let (id, version) = read_lxapp_json(cwd)?;
            let channel = channel_arg.map(normalize_channel).transpose()?;
            Ok(PackageMeta {
                target,
                target_id: id,
                version,
                channel: Some(channel.unwrap_or_else(|| "release".to_string())),
            })
        }
        "lxplugin" => {
            let (id, version) = read_lxplugin_json(cwd)?;
            let channel = channel_arg.map(normalize_channel).transpose()?;
            Ok(PackageMeta {
                target,
                target_id: id,
                version,
                channel: Some(channel.unwrap_or_else(|| "release".to_string())),
            })
        }
        "app" => {
            if channel_arg.is_some() {
                bail!(
                    "--env/--channel is not supported when publishing target=app; app channel is read from the packaged app.json envVersion"
                );
            }
            let (id, version) = read_app_config(cwd)?;
            Ok(PackageMeta {
                target,
                target_id: id,
                version,
                channel: None,
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
        "Could not detect project type. No lxapp.json, lxplugin.json, or {} found.\nRun publish from an lxapp, lxplugin, or host app project root.",
        HOST_CONFIG_FILE
    );
}

fn normalize_channel(s: &str) -> Result<String> {
    match s.to_lowercase().as_str() {
        "release" => Ok("release".to_string()),
        "preview" | "trial" => Ok("preview".to_string()),
        "developer" | "develop" => Ok("developer".to_string()),
        _ => bail!("Invalid envVersion '{s}'. Must be one of: release, preview, developer"),
    }
}

struct AppPackageMetadata {
    env_version: String,
    /// Suffixed lingxiaId baked into the package, if any. Authoritative for
    /// publish because the runtime resolves updates against this exact id.
    lingxia_id: Option<String>,
}

fn read_app_package_metadata(path: &Path) -> Result<AppPackageMetadata> {
    let app_json = if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("apk"))
    {
        read_zip_entry(path, &["assets/app.json", "app/src/main/assets/app.json"])?
    } else if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("-macos.zip"))
    {
        read_macos_zip_app_json(path)?
    } else {
        bail!("unsupported app package type; expected Android .apk or macOS *-macos.zip");
    };
    let value: serde_json::Value =
        serde_json::from_slice(&app_json).context("Failed to parse app.json in package")?;
    let env_version = value
        .get("envVersion")
        .and_then(|value| value.as_str())
        .context("app.json in package is missing envVersion; rebuild the app with a newer CLI")?;
    let env_version = normalize_channel(env_version)?;
    let lingxia_id = value
        .get("lingxiaId")
        .and_then(|value| value.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Ok(AppPackageMetadata {
        env_version,
        lingxia_id,
    })
}

fn read_zip_entry(path: &Path, names: &[&str]) -> Result<Vec<u8>> {
    let file =
        fs::File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file)
        .with_context(|| format!("Failed to read zip archive {}", path.display()))?;
    for name in names {
        if let Ok(mut entry) = zip.by_name(name) {
            let mut data = Vec::new();
            entry
                .read_to_end(&mut data)
                .with_context(|| format!("Failed to read {name} from {}", path.display()))?;
            return Ok(data);
        }
    }
    bail!(
        "app.json not found in {}; looked for {}",
        path.display(),
        names.join(", ")
    )
}

fn read_macos_zip_app_json(path: &Path) -> Result<Vec<u8>> {
    let file =
        fs::File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut outer = zip::ZipArchive::new(file)
        .with_context(|| format!("Failed to read zip archive {}", path.display()))?;
    // A single unreadable entry (corrupt header, ZIP64 edge, etc.) shouldn't
    // abort the whole publish — skip it and keep scanning.
    for index in 0..outer.len() {
        let mut entry = match outer.by_index(index) {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let name = entry.name().to_string();
        if name.ends_with(".app/Contents/Resources/app.json") {
            let mut data = Vec::new();
            entry
                .read_to_end(&mut data)
                .with_context(|| format!("Failed to read {name} from {}", path.display()))?;
            return Ok(data);
        }
    }
    bail!("app.json not found in macOS app package {}", path.display())
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

fn resolve_lingxia_server(
    cwd: &Path,
    channel: Option<&str>,
    lingxia_server_arg: Option<String>,
) -> Result<String> {
    if let Some(s) = lingxia_server_arg {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            bail!("--lingxia-server cannot be empty");
        }
        return Ok(trimmed.to_string());
    }
    let config_path = cwd.join(HOST_CONFIG_FILE);
    if !config_path.exists() {
        bail!("Use --lingxia-server to specify the package upload server URL.");
    }
    let Ok(cfg) = LingXiaConfig::load(cwd) else {
        bail!("Use --lingxia-server to specify the package upload server URL.");
    };

    // Prefer the env-specific server when a channel is known and the project
    // declares per-env overrides. Falls through to the top-level
    // `app.lingxiaServer` for projects that haven't migrated.
    if let Some(channel) = channel
        && let Ok(env_version) = EnvVersion::parse_cli(channel)
        && let Ok(resolved) = cfg.resolve_env(env_version)
        && !resolved.lingxia_server.is_empty()
    {
        return Ok(resolved.lingxia_server);
    }

    if let Some(url) = cfg.app.and_then(|a| a.lingxia_server) {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    bail!("Use --lingxia-server to specify the package upload server URL.");
}

fn find_or_resolve_package(
    cwd: &Path,
    target: &str,
    explicit: Option<String>,
    platform: Option<&str>,
) -> Result<PathBuf> {
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
    collect_matching_packages(cwd, &dist_dir, target, platform, &mut candidates, 0);
    collect_matching_packages(
        &dist_dir,
        &dist_dir,
        target,
        platform,
        &mut candidates,
        MAX_PACKAGE_SEARCH_DEPTH,
    );
    if candidates.is_empty() {
        collect_common_build_packages(cwd, target, platform, &mut candidates);
    }
    candidates.sort();
    candidates.dedup();

    match candidates.len() {
        0 => bail!("{}", missing_package_message(target, platform)),
        1 => Ok(candidates.remove(0)),
        _ => {
            let list = candidates
                .iter()
                .map(|p| format!("  {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n");
            bail!(
                "Multiple packages found. Use --platform <platform> or --package-path <PATH> to specify one:\n{list}"
            )
        }
    }
}

const MAX_PACKAGE_SEARCH_DEPTH: u32 = 3;

fn resolve_publish_platform(
    cwd: &Path,
    target: &str,
    platform: Option<&str>,
) -> Result<Option<String>> {
    if target != "app" {
        if platform.is_some() {
            bail!("--platform is only supported when publishing target=app.");
        }
        return Ok(None);
    }

    if let Some(platform) = platform {
        return normalize_platform(platform).map(Some);
    }

    let Ok(config) = LingXiaConfig::load(cwd) else {
        return Ok(None);
    };
    let Some(app) = config.app.as_ref() else {
        return Ok(None);
    };

    let mut platforms = app
        .platforms
        .iter()
        .map(|platform| normalize_config_platform(platform))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    platforms.sort();
    platforms.dedup();

    match platforms.as_slice() {
        [only] => Ok(Some(only.clone())),
        [] => bail!(
            "Host app publishing supports Android and macOS. iOS uses App Store; Harmony uses app marketplace."
        ),
        _ => {
            let list = platforms.join(", ");
            bail!(
                "Multiple app platforms are configured: {list}\n\
                 Pass `--platform <platform>` when publishing, or use `--package-path <PATH>`."
            )
        }
    }
}

fn normalize_platform(value: &str) -> Result<String> {
    let platform: PlatformType = value.parse()?;
    match platform {
        PlatformType::Android | PlatformType::MacOs => Ok(platform.as_str().to_string()),
        PlatformType::Ios => bail!("iOS host app publishing uses App Store."),
        PlatformType::Harmony => bail!("Harmony host app publishing uses app marketplace."),
    }
}

fn normalize_config_platform(value: &str) -> Result<Option<String>> {
    let platform: PlatformType = value.parse()?;
    Ok(match platform {
        PlatformType::Android | PlatformType::MacOs => Some(platform.as_str().to_string()),
        PlatformType::Ios | PlatformType::Harmony => None,
    })
}

fn collect_common_build_packages(
    cwd: &Path,
    target: &str,
    platform: Option<&str>,
    out: &mut Vec<PathBuf>,
) {
    if target != "app" {
        return;
    }

    let rels: &[(&str, &str)] = &[
        (
            "android",
            "android/app/build/outputs/apk/release/app-release.apk",
        ),
        ("android", "app/build/outputs/apk/release/app-release.apk"),
    ];

    for (candidate_platform, rel) in rels {
        if platform.is_some_and(|platform| platform != *candidate_platform) {
            continue;
        }
        let path = cwd.join(rel);
        if path.is_file() {
            out.push(path);
        }
    }
}

fn missing_package_message(target: &str, platform: Option<&str>) -> String {
    if target == "app" {
        let platform_suffix = platform
            .map(|platform| format!(" for platform '{platform}'"))
            .unwrap_or_default();
        return format!(
            "No package found for target '{target}'{platform_suffix}.\n\
             Run `lingxia package --platform <platform>` first, then retry `lingxia publish`.\n\
             Searched ./dist for Android APKs and macOS update zips, plus common Android release outputs.\n\
             If the package is elsewhere, pass `--package-path <PATH>`."
        );
    }

    format!(
        "No package found for target '{target}'. Run `lingxia package` first, or use --package-path <PATH>."
    )
}

fn collect_matching_packages(
    dir: &Path,
    dist_dir: &Path,
    target: &str,
    platform: Option<&str>,
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
                collect_matching_packages(&path, dist_dir, target, platform, out, max_depth - 1);
            }
            continue;
        }
        if !path.is_file() {
            continue;
        }

        if package_matches(target, &path, dist_dir, platform) {
            out.push(path);
        }
    }
}

fn package_matches(target: &str, path: &Path, dist_dir: &Path, platform: Option<&str>) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    match target {
        "app" => package_platform(file_name, path, dist_dir).is_some_and(|candidate| {
            platform
                .map(|platform| candidate == platform)
                .unwrap_or(true)
        }),
        _ => false,
    }
}

fn package_platform(file_name: &str, path: &Path, dist_dir: &Path) -> Option<&'static str> {
    if file_name.ends_with(".apk") {
        return Some("android");
    }
    if file_name.ends_with("-macos.zip") && path.starts_with(dist_dir.join("macos")) {
        return Some("macos");
    }
    None
}

fn sha256_hex(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

fn upload_transport_error(url: &str, package_bytes: usize, err: ureq::Error) -> anyhow::Error {
    let message = err.to_string();
    let lower = message.to_ascii_lowercase();
    let size_mib = package_bytes as f64 / 1024.0 / 1024.0;

    if lower.contains("broken pipe")
        || lower.contains("connection reset")
        || lower.contains("connection reset by peer")
    {
        return anyhow::anyhow!(
            "HTTP request failed: {url}\n\
             Transport error: {message}\n\
             The server closed the connection while receiving a {size_mib:.1} MiB package.\n\
             Check the upload server or gateway request-body limit, then retry. For cloud-mockd, restart the updated mock server."
        );
    }

    anyhow::anyhow!("HTTP request failed: {url}\nTransport error: {message}")
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
    use super::{
        build_multipart, find_or_resolve_package, normalize_platform, package_matches,
        read_app_package_metadata, resolve_meta, resolve_publish_platform,
    };
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;
    use zip::write::SimpleFileOptions;

    #[test]
    fn app_package_match_accepts_cli_generated_macos_zip_only_in_dist_macos() {
        let temp = TempDir::new().unwrap();
        let dist_dir = temp.path().join("dist");
        let allowed = dist_dir.join("macos").join("Demo-1.0.0-macos.zip");
        let rejected = temp.path().join("Demo-1.0.0-macos.zip");

        fs::create_dir_all(allowed.parent().unwrap()).unwrap();
        fs::write(&allowed, b"zip").unwrap();
        fs::write(&rejected, b"zip").unwrap();

        assert!(package_matches("app", &allowed, &dist_dir, None));
        assert!(!package_matches("app", &rejected, &dist_dir, None));
    }

    #[test]
    fn find_or_resolve_package_ignores_unrelated_root_zip_for_app_publish() {
        let temp = TempDir::new().unwrap();
        let dist_macos = temp.path().join("dist").join("macos");
        fs::create_dir_all(&dist_macos).unwrap();
        fs::write(temp.path().join("notes.zip"), b"zip").unwrap();

        let expected = dist_macos.join("Demo-1.0.0-macos.zip");
        fs::write(&expected, b"zip").unwrap();

        let resolved = find_or_resolve_package(temp.path(), "app", None, None).unwrap();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn find_or_resolve_package_falls_back_to_android_release_output() {
        let temp = TempDir::new().unwrap();
        let expected = temp
            .path()
            .join("android/app/build/outputs/apk/release/app-release.apk");
        fs::create_dir_all(expected.parent().unwrap()).unwrap();
        fs::write(&expected, b"apk").unwrap();

        let resolved = find_or_resolve_package(temp.path(), "app", None, None).unwrap();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn find_or_resolve_package_prefers_dist_over_common_build_output() {
        let temp = TempDir::new().unwrap();
        let dist = temp.path().join("dist/android/app-release.apk");
        let build = temp
            .path()
            .join("android/app/build/outputs/apk/release/app-release.apk");
        fs::create_dir_all(dist.parent().unwrap()).unwrap();
        fs::create_dir_all(build.parent().unwrap()).unwrap();
        fs::write(&dist, b"dist-apk").unwrap();
        fs::write(&build, b"build-apk").unwrap();

        let resolved = find_or_resolve_package(temp.path(), "app", None, None).unwrap();
        assert_eq!(resolved, dist);
    }

    #[test]
    fn find_or_resolve_package_filters_by_platform() {
        let temp = TempDir::new().unwrap();
        let android = temp.path().join("dist/android/app-release.apk");
        let macos = temp.path().join("dist/macos/Demo-1.0.0-macos.zip");
        fs::create_dir_all(android.parent().unwrap()).unwrap();
        fs::create_dir_all(macos.parent().unwrap()).unwrap();
        fs::write(&android, b"apk").unwrap();
        fs::write(&macos, b"zip").unwrap();

        let resolved = find_or_resolve_package(temp.path(), "app", None, Some("android")).unwrap();
        assert_eq!(resolved, android);
    }

    #[test]
    fn resolve_publish_platform_requires_platform_for_multi_platform_config() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("lingxia.yaml"),
            r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 1.0.0
  lingxiaId: demo
  environments:
    release:
      lingxiaServer: https://api.example.com
  platforms:
    - android
    - macos
  homeAppId: demo.home
android:
  packageId: app.example.demo
macos:
  bundleId: app.example.demo
ui:
  launch:
    initialSurface: main
  surfaces:
    - id: main
      presentation:
        kind: window
      content:
        kind: lxapp
        appId: demo.home
  activators:
    - id: main
      kind: titlebarItem
      hostSurface: main
      action:
        kind: focusSurface
        surface: main
"#,
        )
        .unwrap();

        let error = resolve_publish_platform(temp.path(), "app", None).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Multiple app platforms are configured")
        );
    }

    #[test]
    fn resolve_publish_platform_rejects_harmony_host_publish() {
        let error = normalize_platform("harmony").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Harmony host app publishing uses app marketplace")
        );
    }

    #[test]
    fn resolve_publish_platform_ignores_store_only_config_platforms() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("lingxia.yaml"),
            r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 1.0.0
  lingxiaId: demo
  environments:
    release:
      lingxiaServer: https://api.example.com
  platforms:
    - android
    - harmony
  homeAppId: demo.home
android:
  packageId: app.example.demo
harmony:
  bundleName: app.example.demo
"#,
        )
        .unwrap();

        let platform = resolve_publish_platform(temp.path(), "app", None).unwrap();
        assert_eq!(platform.as_deref(), Some("android"));
    }

    #[test]
    fn build_multipart_includes_app_platform_field() {
        let body = build_multipart(
            "boundary",
            &[("kind", "app"), ("platform", "android")],
            "app-release.apk",
            b"apk",
        );
        let body = String::from_utf8(body).unwrap();
        assert!(body.contains("name=\"platform\"\r\n\r\nandroid"));
    }

    #[test]
    fn lxapp_publish_channel_can_be_selected() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("lxapp.json"),
            br#"{"appId":"demo","version":"1.0.0","pages":["index.html"]}"#,
        )
        .unwrap();

        let meta = resolve_meta(temp.path(), Some("preview")).unwrap();

        assert_eq!(meta.target, "lxapp");
        assert_eq!(meta.channel.as_deref(), Some("preview"));
    }

    #[test]
    fn app_publish_rejects_explicit_channel() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("lingxia.yaml"),
            r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 1.0.0
  lingxiaId: demo
  platforms:
    - android
  homeAppId: demo.home
android:
  packageId: app.example.demo
"#,
        )
        .unwrap();

        let err = resolve_meta(temp.path(), Some("developer"))
            .unwrap_err()
            .to_string();

        assert!(err.contains("not supported when publishing target=app"));
    }

    #[test]
    fn publish_reads_channel_from_android_app_json_env_version() {
        let temp = TempDir::new().unwrap();
        let apk = temp.path().join("app-preview.apk");
        write_zip(
            &apk,
            &[(
                "assets/app.json",
                br#"{"productName":"Demo","productVersion":"1.0.0","homeAppId":"demo","homeAppVersion":"1.0.0","envVersion":"preview"}"#,
            )],
        );

        let metadata = read_app_package_metadata(&apk).unwrap();

        assert_eq!(metadata.env_version, "preview");
        assert!(metadata.lingxia_id.is_none());
    }

    #[test]
    fn publish_reads_channel_from_macos_app_json_env_version() {
        let temp = TempDir::new().unwrap();
        let zip = temp.path().join("Demo-1.0.0-macos.zip");
        write_zip(
            &zip,
            &[(
                "Demo.app/Contents/Resources/app.json",
                br#"{"productName":"Demo","productVersion":"1.0.0","homeAppId":"demo","homeAppVersion":"1.0.0","envVersion":"developer"}"#,
            )],
        );

        let metadata = read_app_package_metadata(&zip).unwrap();

        assert_eq!(metadata.env_version, "developer");
    }

    #[test]
    fn publish_picks_up_suffixed_lingxia_id_from_app_package() {
        // dev/preview builds bake a suffixed id into app.json. publish must
        // upload that exact id so update checks line up.
        let temp = TempDir::new().unwrap();
        let apk = temp.path().join("app-dev.apk");
        write_zip(
            &apk,
            &[(
                "assets/app.json",
                br#"{"productName":"Demo","productVersion":"1.0.0","homeAppId":"demo","homeAppVersion":"1.0.0","envVersion":"developer","lingxiaId":"demo.dev"}"#,
            )],
        );

        let metadata = read_app_package_metadata(&apk).unwrap();

        assert_eq!(metadata.env_version, "developer");
        assert_eq!(metadata.lingxia_id.as_deref(), Some("demo.dev"));
    }

    fn write_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (name, data) in entries {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
}
