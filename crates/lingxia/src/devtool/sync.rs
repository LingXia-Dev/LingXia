use lingxia_platform::traits::app_runtime::AppRuntime;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

const LINGXIA_DIR: &str = "lingxia";
const DEV_LXAPPS_DIR: &str = "dev-lxapps";
const LOCAL_MANIFEST_FILE: &str = ".lingxia-dev-manifest.json";
const HTTP_TIMEOUT: Duration = Duration::from_secs(3);
/// More changed files than this come as one archive instead of one request each.
const PER_FILE_MAX: usize = 4;
/// The largest zstd window a dev bundle archive may use (the dev server's).
const BUNDLE_WINDOW_LOG: u32 = 24;

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

/// Result of pulling one app id from the dev bundle server.
#[derive(Debug, Clone)]
pub(crate) enum DevBundleSync {
    /// No dev bundle server is configured, or the app id is empty.
    Idle,
    /// The server has no manifest for this app id (not a local bundle).
    NotPublished,
    /// Files are in `dev-lxapps/<appId>/current` and the source is registered.
    Ready { root: std::path::PathBuf },
}

/// Pull `app_id` when this process has a dev bundle server, giving up at
/// `deadline`. A missing manifest is [`DevBundleSync::NotPublished`]:
/// downloaded guests are not published here.
pub(crate) fn sync_dev_bundle(
    runtime: Arc<lingxia_platform::Platform>,
    app_id: &str,
    deadline: Instant,
) -> Result<DevBundleSync, String> {
    let Some(config) = lingxia_app_context::app_config() else {
        return Ok(DevBundleSync::Idle);
    };
    sync_from(
        runtime,
        config.dev_bundle_base_url.as_deref(),
        app_id,
        deadline,
    )
}

/// [`sync_dev_bundle`] against an explicit server, for callers that run before
/// the app config is installed.
pub(crate) fn sync_from(
    runtime: Arc<lingxia_platform::Platform>,
    base_url: Option<&str>,
    app_id: &str,
    deadline: Instant,
) -> Result<DevBundleSync, String> {
    let Some(base_url) = base_url.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(DevBundleSync::Idle);
    };

    let app_id = app_id.trim();
    if app_id.is_empty() {
        return Ok(DevBundleSync::Idle);
    }

    let manifest_url = dev_bundle_url(
        base_url,
        &format!("lxapp/{}/manifest.json", encode_path_component(app_id)),
    );
    let manifest_bytes = match http_get(&manifest_url, deadline)? {
        (200, body) => body,
        (404, _) => return Ok(DevBundleSync::NotPublished),
        (status, _) => return Err(format!("GET {manifest_url} returned HTTP {status}")),
    };
    let manifest: DevManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|err| format!("invalid dev manifest from {}: {}", manifest_url, err))?;
    if manifest.app_id != app_id {
        return Err(format!(
            "dev manifest appId '{}' does not match requested appId '{}'",
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
        lxapp::register_dev_bundle_source(app_id.to_string(), current_dir.clone());
        log::info!("Using cached dev lxapp bundle for {}", app_id);
        return Ok(DevBundleSync::Ready { root: current_dir });
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
    let mut missing = Vec::new();
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
        } else {
            missing.push(file);
        }
    }

    // A whole bundle is one request; a hot-swap delta stays per file.
    if missing.len() > PER_FILE_MAX {
        let url = dev_bundle_url(
            base_url,
            &format!(
                "lxapp/{}/bundle/{}.tar.zst",
                encode_path_component(app_id),
                encode_path_component(&manifest.dist_hash)
            ),
        );
        let started = Instant::now();
        match http_get(&url, deadline)? {
            (200, body) => {
                unpack_bundle(&body, &missing, &staging_dir)?;
                log::debug!(
                    "Dev bundle {app_id}: {} files as one archive, {} bytes in {} ms",
                    missing.len(),
                    body.len(),
                    started.elapsed().as_millis()
                );
                missing.clear();
            }
            // An older dev server has no archive endpoint.
            (404, _) => {}
            (status, _) => return Err(format!("GET {url} returned HTTP {status}")),
        }
    }

    for file in missing {
        let target = staging_dir.join(&file.path);
        let url = dev_bundle_url(
            base_url,
            &format!(
                "lxapp/{}/files/{}",
                encode_path_component(app_id),
                encode_path_component(&file.path)
            ),
        );
        let bytes = match http_get(&url, deadline)? {
            (200, body) => body,
            (status, _) => return Err(format!("GET {url} returned HTTP {status}")),
        };
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

    lxapp::register_dev_bundle_source(app_id.to_string(), current_dir.clone());
    log::info!(
        "Synced dev lxapp bundle for {} ({})",
        app_id,
        manifest.dist_hash
    );
    Ok(DevBundleSync::Ready { root: current_dir })
}

/// Write the zstd tar's copies of `wanted` into `staging_dir`. Every wanted file
/// must be in it at its manifest size; anything else is ignored.
fn unpack_bundle(
    archive: &[u8],
    wanted: &[&DevManifestFile],
    staging_dir: &Path,
) -> Result<(), String> {
    let mut wanted: BTreeMap<&str, &DevManifestFile> = wanted
        .iter()
        .map(|file| (file.path.as_str(), *file))
        .collect();
    let mut decoder = zstd::stream::read::Decoder::new(archive)
        .map_err(|err| format!("invalid dev bundle archive: {err}"))?;
    decoder
        .window_log_max(BUNDLE_WINDOW_LOG)
        .map_err(|err| format!("invalid dev bundle archive: {err}"))?;
    let mut entries = tar::Archive::new(decoder);
    for entry in entries
        .entries()
        .map_err(|err| format!("invalid dev bundle archive: {err}"))?
    {
        let mut entry = entry.map_err(|err| format!("invalid dev bundle archive: {err}"))?;
        let path = entry
            .path()
            .map_err(|err| format!("invalid dev bundle entry: {err}"))?
            .to_string_lossy()
            .into_owned();
        let Some(file) = wanted.remove(path.as_str()) else {
            continue;
        };
        let mut bytes = Vec::with_capacity(file.size as usize);
        entry
            .read_to_end(&mut bytes)
            .map_err(|err| format!("failed to read {path} from the dev bundle: {err}"))?;
        if bytes.len() as u64 != file.size {
            return Err(format!(
                "dev file size mismatch for {path}: expected {}, got {}",
                file.size,
                bytes.len()
            ));
        }
        let target = staging_dir.join(&file.path);
        fs::write(&target, bytes)
            .map_err(|err| format!("failed to write {}: {}", target.display(), err))?;
    }
    match wanted.keys().next() {
        Some(path) => Err(format!("dev bundle archive is missing {path}")),
        None => Ok(()),
    }
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
            file.path, file.size, len
        ));
    }
    Ok(())
}

/// Status and body of a GET, within `deadline`. Only a transport or framing
/// failure is an `Err`. The dev server closes every connection, so each file
/// is its own request.
fn http_get(url: &str, deadline: Instant) -> Result<(u16, Vec<u8>), String> {
    let parsed = ParsedHttpUrl::parse(url)?;
    let timeout = request_timeout(deadline)
        .ok_or_else(|| format!("dev bundle sync ran out of time before {url}"))?;
    let mut stream = super::net::connect_dev_server(url, timeout)
        .map_err(|err| format!("failed to connect {}: {}", url, err))?;
    let _ = stream.set_read_timeout(Some(timeout));
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

/// One request's timeout: the per-request bound, cut to what the deadline leaves.
fn request_timeout(deadline: Instant) -> Option<Duration> {
    let left = deadline.checked_duration_since(Instant::now())?;
    (!left.is_zero()).then(|| left.min(HTTP_TIMEOUT))
}

fn parse_http_response(url: &str, response: &[u8]) -> Result<(u16, Vec<u8>), String> {
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
    Ok((status, response[header_end + 4..].to_vec()))
}

/// The parts of a dev bundle URL the request line needs; the connection
/// itself comes from [`super::net::connect_dev_server`].
struct ParsedHttpUrl {
    host_header: String,
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
        Ok(Self {
            host_header: authority.to_string(),
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

fn dev_bundle_url(base_url: &str, relative_path: &str) -> String {
    let (base, query) = base_url
        .split_once('?')
        .map(|(base, query)| (base, Some(query)))
        .unwrap_or((base_url, None));
    let mut url = format!(
        "{}/{}",
        base.trim_end_matches('/'),
        relative_path.trim_start_matches('/')
    );
    if let Some(query) = query.filter(|query| !query.is_empty()) {
        url.push('?');
        url.push_str(query);
    }
    url
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

#[cfg(test)]
mod tests {
    use super::{
        DevManifestFile, Duration, HTTP_TIMEOUT, Instant, ParsedHttpUrl, dev_bundle_url,
        parse_http_response, request_timeout, unpack_bundle,
    };

    fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (path, body) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            builder.append_data(&mut header, path, *body).unwrap();
        }
        zstd::encode_all(builder.into_inner().unwrap().as_slice(), 3).unwrap()
    }

    fn file(path: &str, size: u64) -> DevManifestFile {
        DevManifestFile {
            path: path.into(),
            hash: "h".into(),
            size,
        }
    }

    #[test]
    fn a_bundle_archive_fills_only_the_wanted_files_at_their_sizes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("pages")).unwrap();
        let bytes = archive(&[
            ("lxapp.json", b"{}"),
            ("pages/a.js", b"abc"),
            ("extra.js", b"x"),
        ]);
        let (json, page) = (file("lxapp.json", 2), file("pages/a.js", 3));
        unpack_bundle(&bytes, &[&json, &page], dir.path()).unwrap();
        assert_eq!(
            std::fs::read(dir.path().join("pages/a.js")).unwrap(),
            b"abc"
        );
        assert!(!dir.path().join("extra.js").exists());

        let short = file("pages/a.js", 4);
        assert!(
            unpack_bundle(&bytes, &[&short], dir.path())
                .unwrap_err()
                .contains("size mismatch")
        );
        let absent = file("pages/b.js", 1);
        assert!(
            unpack_bundle(&bytes, &[&absent], dir.path())
                .unwrap_err()
                .contains("missing pages/b.js")
        );
    }

    #[test]
    fn a_request_never_outlives_the_sync_deadline() {
        let now = Instant::now();
        assert_eq!(
            request_timeout(now + Duration::from_secs(60)),
            Some(HTTP_TIMEOUT)
        );
        let short = request_timeout(now + Duration::from_millis(500)).unwrap();
        assert!(short <= Duration::from_millis(500) && !short.is_zero());
        assert_eq!(request_timeout(now), None);
        assert_eq!(request_timeout(now - Duration::from_secs(1)), None);
    }

    #[test]
    fn http_404_means_the_bundle_is_not_published() {
        let (status, _) =
            parse_http_response("http://127.0.0.1:9", b"HTTP/1.1 404 Not Found\r\n\r\n").unwrap();
        assert_eq!(status, 404);
        let (status, body) =
            parse_http_response("http://127.0.0.1:9", b"HTTP/1.1 200 OK\r\n\r\n{}").unwrap();
        assert_eq!((status, body.as_slice()), (200, b"{}".as_slice()));
    }

    #[test]
    fn dev_bundle_paths_keep_auth_query_at_the_end() {
        let url = dev_bundle_url(
            "http://192.168.1.20:39142/__lingxia/dev?token=abc",
            "lxapp/demo/manifest.json",
        );

        assert_eq!(
            url,
            "http://192.168.1.20:39142/__lingxia/dev/lxapp/demo/manifest.json?token=abc"
        );
        assert_eq!(
            ParsedHttpUrl::parse(&url).unwrap().path,
            "/__lingxia/dev/lxapp/demo/manifest.json?token=abc"
        );
    }
}
