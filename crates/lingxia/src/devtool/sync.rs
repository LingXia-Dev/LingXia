use lingxia_platform::traits::app_runtime::AppRuntime;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const LINGXIA_DIR: &str = "lingxia";
const DEV_LXAPPS_DIR: &str = "dev-lxapps";
const LOCAL_MANIFEST_FILE: &str = ".lingxia-dev-manifest.json";
const HTTP_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DevManifest {
    app_id: String,
    version: String,
    dist_hash: String,
    files: Vec<DevManifestFile>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DevManifestFile {
    path: String,
    hash: String,
    size: u64,
}

pub(crate) fn sync_dev_home_bundle(runtime: Arc<lingxia_platform::Platform>) -> Result<(), String> {
    let Some(config) = lingxia_app_context::app_config() else {
        return Ok(());
    };
    let Some(base_url) = config
        .dev_bundle_base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };

    let app_id = config.home_app_id.trim();
    if app_id.is_empty() {
        return Ok(());
    }

    let manifest_url = format!(
        "{}/lxapp/{}/manifest.json",
        base_url.trim_end_matches('/'),
        encode_path_component(app_id)
    );
    let manifest_bytes = http_get(&manifest_url)?;
    let manifest: DevManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|err| format!("invalid dev manifest from {}: {}", manifest_url, err))?;
    if manifest.app_id != app_id {
        return Err(format!(
            "dev manifest appId '{}' does not match home appId '{}'",
            manifest.app_id, app_id
        ));
    }
    validate_manifest(&manifest)?;

    let root = runtime
        .app_data_dir()
        .join(LINGXIA_DIR)
        .join(DEV_LXAPPS_DIR)
        .join(sanitize_component(app_id));
    let current_dir = root.join("current");
    let local_manifest = read_local_manifest(&current_dir);
    if local_manifest
        .as_ref()
        .is_some_and(|local| local.dist_hash == manifest.dist_hash)
        && current_dir.join("lxapp.json").is_file()
    {
        lxapp::register_dev_bundle_source(app_id.to_string(), current_dir);
        log::info!("Using cached dev lxapp bundle for {}", app_id);
        return Ok(());
    }

    fs::create_dir_all(&root)
        .map_err(|err| format!("failed to create {}: {}", root.display(), err))?;
    let staging_dir = root.join(format!("staging-{}", manifest.dist_hash));
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir)
            .map_err(|err| format!("failed to clear {}: {}", staging_dir.display(), err))?;
    }
    fs::create_dir_all(&staging_dir)
        .map_err(|err| format!("failed to create {}: {}", staging_dir.display(), err))?;

    let local_files = local_manifest
        .as_ref()
        .map(files_by_path)
        .unwrap_or_default();
    for file in &manifest.files {
        let target = staging_dir.join(&file.path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {}", parent.display(), err))?;
        }
        let copied = local_files
            .get(&file.path)
            .is_some_and(|local| local.hash == file.hash && local.size == file.size)
            && copy_existing_file(&current_dir, &file.path, &target)?;
        if copied {
            verify_file_size(&target, file)?;
            continue;
        }

        let url = format!(
            "{}/lxapp/{}/files/{}",
            base_url.trim_end_matches('/'),
            encode_path_component(app_id),
            encode_path_component(&file.path)
        );
        let bytes = http_get(&url)?;
        if bytes.len() as u64 != file.size {
            return Err(format!(
                "dev file size mismatch for {}: expected {}, got {}",
                file.path,
                file.size,
                bytes.len()
            ));
        }
        fs::write(&target, bytes)
            .map_err(|err| format!("failed to write {}: {}", target.display(), err))?;
    }

    write_local_manifest(&staging_dir, &manifest)?;
    let old_dir = root.join("previous");
    if old_dir.exists() {
        let _ = fs::remove_dir_all(&old_dir);
    }
    if current_dir.exists() {
        fs::rename(&current_dir, &old_dir).map_err(|err| {
            format!(
                "failed to move {} to {}: {}",
                current_dir.display(),
                old_dir.display(),
                err
            )
        })?;
    }
    fs::rename(&staging_dir, &current_dir).map_err(|err| {
        format!(
            "failed to publish {} to {}: {}",
            staging_dir.display(),
            current_dir.display(),
            err
        )
    })?;
    let _ = fs::remove_dir_all(&old_dir);

    lxapp::register_dev_bundle_source(app_id.to_string(), current_dir);
    log::info!(
        "Synced dev lxapp bundle for {} ({})",
        app_id,
        manifest.dist_hash
    );
    Ok(())
}

fn validate_manifest(manifest: &DevManifest) -> Result<(), String> {
    if manifest.version.trim().is_empty() || manifest.dist_hash.trim().is_empty() {
        return Err("dev manifest missing version or distHash".to_string());
    }
    let mut seen = BTreeSet::new();
    for file in &manifest.files {
        validate_relative_path(&file.path)?;
        if file.hash.trim().is_empty() {
            return Err(format!("dev manifest file missing hash: {}", file.path));
        }
        if !seen.insert(file.path.as_str()) {
            return Err(format!("duplicate dev manifest file: {}", file.path));
        }
    }
    if !seen.contains("lxapp.json") {
        return Err("dev manifest does not include lxapp.json".to_string());
    }
    Ok(())
}

fn read_local_manifest(current_dir: &Path) -> Option<DevManifest> {
    let bytes = fs::read(current_dir.join(LOCAL_MANIFEST_FILE)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_local_manifest(dir: &Path, manifest: &DevManifest) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|err| err.to_string())?;
    fs::write(dir.join(LOCAL_MANIFEST_FILE), bytes)
        .map_err(|err| format!("failed to write local dev manifest: {}", err))
}

fn files_by_path(manifest: &DevManifest) -> BTreeMap<String, DevManifestFile> {
    manifest
        .files
        .iter()
        .map(|file| (file.path.clone(), file.clone()))
        .collect()
}

fn copy_existing_file(
    current_dir: &Path,
    relative_path: &str,
    target: &Path,
) -> Result<bool, String> {
    let source = current_dir.join(relative_path);
    if !source.is_file() {
        return Ok(false);
    }
    fs::copy(&source, target).map_err(|err| {
        format!(
            "failed to copy cached dev file {} to {}: {}",
            source.display(),
            target.display(),
            err
        )
    })?;
    Ok(true)
}

fn verify_file_size(path: &Path, file: &DevManifestFile) -> Result<(), String> {
    let len = fs::metadata(path)
        .map_err(|err| format!("failed to inspect {}: {}", path.display(), err))?
        .len();
    if len != file.size {
        return Err(format!(
            "cached dev file size mismatch for {}: expected {}, got {}",
            file.path,
            file.size,
            len
        ));
    }
    Ok(())
}

fn http_get(url: &str) -> Result<Vec<u8>, String> {
    let parsed = ParsedHttpUrl::parse(url)?;
    let mut addrs = (parsed.host.as_str(), parsed.port)
        .to_socket_addrs()
        .map_err(|err| format!("failed to resolve {}: {}", parsed.host, err))?;
    let addr = addrs
        .next()
        .ok_or_else(|| format!("no address resolved for {}", parsed.host))?;
    let mut stream = TcpStream::connect_timeout(&addr, HTTP_TIMEOUT)
        .map_err(|err| format!("failed to connect {}: {}", url, err))?;
    let _ = stream.set_read_timeout(Some(HTTP_TIMEOUT));
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        parsed.path, parsed.host_header
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|err| format!("failed to send request {}: {}", url, err))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|err| format!("failed to read response {}: {}", url, err))?;
    parse_http_response(url, &response)
}

fn parse_http_response(url: &str, response: &[u8]) -> Result<Vec<u8>, String> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| format!("invalid HTTP response from {}", url))?;
    let headers = std::str::from_utf8(&response[..header_end])
        .map_err(|err| format!("invalid HTTP headers from {}: {}", url, err))?;
    let status_line = headers
        .lines()
        .next()
        .ok_or_else(|| format!("missing HTTP status from {}", url))?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| format!("invalid HTTP status from {}", url))?;
    if status != 200 {
        return Err(format!("GET {} returned HTTP {}", url, status));
    }
    Ok(response[header_end + 4..].to_vec())
}

struct ParsedHttpUrl {
    host: String,
    host_header: String,
    port: u16,
    path: String,
}

impl ParsedHttpUrl {
    fn parse(url: &str) -> Result<Self, String> {
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| format!("dev bundle URL must use http://, got {}", url))?;
        let (authority, path) = rest
            .split_once('/')
            .map(|(authority, path)| (authority, format!("/{}", path)))
            .unwrap_or((rest, "/".to_string()));
        if authority.is_empty() {
            return Err(format!("missing host in URL {}", url));
        }
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) if !host.is_empty() => {
                let parsed_port = port
                    .parse::<u16>()
                    .map_err(|_| format!("invalid port in URL {}", url))?;
                (host.to_string(), parsed_port)
            }
            _ => (authority.to_string(), 80),
        };
        Ok(Self {
            host,
            host_header: authority.to_string(),
            port,
            path,
        })
    }
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(format!("invalid relative dev file path: {}", path));
    }
    Ok(())
}

fn encode_path_component(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' => out.push(byte as char),
            _ => {
                use std::fmt::Write as _;
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

fn sanitize_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '_',
        })
        .collect()
}
