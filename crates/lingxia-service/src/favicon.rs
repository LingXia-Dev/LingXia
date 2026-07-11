//! Cross-platform website favicon disk cache.

use bytes::Bytes;
use http::Request;
use http_body_util::{BodyExt, Empty};
use rong_rt::http as host_http;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::io::Error as IoError;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

const CACHE_DIR: &str = "browser-favicons";
const MAX_FAVICON_BYTES: usize = 512 * 1024;
const REFRESH_AFTER: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const RETRY_AFTER_FAILURE: Duration = Duration::from_secs(5 * 60);
const EXTENSIONS: &[&str] = &["png", "ico", "jpg", "gif", "webp", "svg"];

#[derive(Default)]
struct RequestState {
    in_flight: HashSet<String>,
    failed_at: HashMap<String, Instant>,
}

fn request_state() -> &'static Mutex<RequestState> {
    static STATE: OnceLock<Mutex<RequestState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(RequestState::default()))
}

fn normalized_origin(page_url: &str) -> Option<String> {
    let uri = page_url.trim().parse::<http::Uri>().ok()?;
    let scheme = uri.scheme_str()?.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let authority = uri.authority()?.as_str().to_ascii_lowercase();
    (!authority.is_empty()).then(|| format!("{scheme}://{authority}"))
}

fn cache_key(origin: &str) -> String {
    let digest = Sha256::digest(origin.as_bytes());
    let mut output = String::with_capacity(32);
    for byte in &digest[..16] {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn cache_directory(cache_root: &Path) -> PathBuf {
    cache_root.join(CACHE_DIR)
}

fn find_cached(cache_root: &Path, key: &str) -> Option<PathBuf> {
    let directory = cache_directory(cache_root);
    EXTENSIONS
        .iter()
        .map(|extension| directory.join(format!("{key}.{extension}")))
        .find(|path| path.is_file())
}

fn cache_is_fresh(path: &Path) -> bool {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age < REFRESH_AFTER)
}

fn should_request(origin: &str, cached: Option<&Path>) -> bool {
    if cached.is_some_and(cache_is_fresh) {
        return false;
    }
    let Ok(mut state) = request_state().lock() else {
        return false;
    };
    if state.in_flight.contains(origin) {
        return false;
    }
    if state
        .failed_at
        .get(origin)
        .is_some_and(|failed| failed.elapsed() < RETRY_AFTER_FAILURE)
    {
        return false;
    }
    state.in_flight.insert(origin.to_string());
    true
}

fn finish_request(origin: &str, succeeded: bool) {
    if let Ok(mut state) = request_state().lock() {
        state.in_flight.remove(origin);
        if succeeded {
            state.failed_at.remove(origin);
        } else {
            state.failed_at.insert(origin.to_string(), Instant::now());
        }
    }
}

/// Returns an existing cache file immediately and schedules a refresh when it
/// is missing or stale. `on_ready` runs after a new file is committed.
pub fn cached_or_request(
    cache_root: &Path,
    page_url: &str,
    on_ready: Arc<dyn Fn() + Send + Sync>,
) -> Option<PathBuf> {
    let origin = normalized_origin(page_url)?;
    let key = cache_key(&origin);
    let cached = find_cached(cache_root, &key);
    if should_request(&origin, cached.as_deref()) {
        let cache_root = cache_root.to_path_buf();
        let request_origin = origin.clone();
        std::mem::drop(rong_rt::RongExecutor::global().spawn(async move {
            let succeeded = match fetch_and_store(&cache_root, &request_origin, &key).await {
                Ok(()) => {
                    on_ready();
                    true
                }
                Err(error) => {
                    log::debug!("favicon fetch failed for {request_origin}: {error}");
                    false
                }
            };
            finish_request(&request_origin, succeeded);
        }));
    }
    cached
}

/// Stores favicon bytes already supplied by a platform WebView and returns
/// the cache path. This avoids a second network request when the real page
/// favicon is available at pin time.
pub fn store_for_url(cache_root: &Path, page_url: &str, bytes: &[u8]) -> Option<PathBuf> {
    if bytes.is_empty() || bytes.len() > MAX_FAVICON_BYTES {
        return None;
    }
    let origin = normalized_origin(page_url)?;
    let key = cache_key(&origin);
    let extension = image_extension("", bytes)?;
    store(cache_root, &key, extension, bytes).ok()?;
    find_cached(cache_root, &key)
}

async fn fetch_and_store(cache_root: &Path, origin: &str, key: &str) -> Result<(), String> {
    let url = format!("{origin}/favicon.ico");
    let request = Request::builder()
        .method("GET")
        .uri(&url)
        .header(
            http::header::ACCEPT,
            "image/png,image/x-icon,image/*;q=0.8,*/*;q=0.1",
        )
        .body(
            Empty::<Bytes>::new()
                .map_err(|_| IoError::other("favicon request body error"))
                .boxed(),
        )
        .map_err(|error| format!("build request: {error}"))?;
    let response = host_http::send_with_small_body_limit(
        request,
        MAX_FAVICON_BYTES,
        host_http::RequestOptions::new()
            .with_connect_timeout(Duration::from_secs(5))
            .with_request_timeout(Duration::from_secs(10)),
    )
    .await
    .map_err(|error| format!("request: {error}"))?;
    if !response.status.is_success() {
        return Err(format!("HTTP {}", response.status));
    }
    let content_type = response
        .headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let bytes = host_http::collect_body(response.body)
        .await
        .map_err(|error| format!("read body: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAX_FAVICON_BYTES {
        return Err("empty or oversized image".to_string());
    }
    let extension = image_extension(&content_type, &bytes)
        .ok_or_else(|| "response is not a supported image".to_string())?;
    store(cache_root, key, extension, &bytes)
}

fn image_extension(content_type: &str, bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if bytes.starts_with(&[0, 0, 1, 0]) {
        return Some("ico");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("jpg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("gif");
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Some("webp");
    }
    let trimmed = bytes
        .iter()
        .copied()
        .skip_while(u8::is_ascii_whitespace)
        .take(256)
        .collect::<Vec<_>>();
    if content_type.contains("svg")
        && std::str::from_utf8(&trimmed)
            .ok()
            .is_some_and(|value| value.contains("<svg"))
    {
        return Some("svg");
    }
    None
}

fn store(cache_root: &Path, key: &str, extension: &str, bytes: &[u8]) -> Result<(), String> {
    let directory = cache_directory(cache_root);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("create {}: {error}", directory.display()))?;
    let path = directory.join(format!("{key}.{extension}"));
    // Unique per writer: a background fetch and a pin-time store for the same
    // key must not interleave through one shared temp file.
    static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);
    let temporary = directory.join(format!(
        "{key}.{}.tmp",
        TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("write {}: {error}", temporary.display()))?;
    replace_cache_file(&temporary, &path)?;
    for other in EXTENSIONS
        .iter()
        .filter(|candidate| **candidate != extension)
        .map(|candidate| directory.join(format!("{key}.{candidate}")))
    {
        let _ = std::fs::remove_file(other);
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_cache_file(temporary: &Path, path: &Path) -> Result<(), String> {
    std::fs::rename(temporary, path).map_err(|error| format!("commit {}: {error}", path.display()))
}

#[cfg(windows)]
fn replace_cache_file(temporary: &Path, path: &Path) -> Result<(), String> {
    let backup = path.with_extension("bak");
    let _ = std::fs::remove_file(&backup);
    let had_previous = path.exists();
    if had_previous {
        std::fs::rename(path, &backup)
            .map_err(|error| format!("backup {}: {error}", path.display()))?;
    }
    if let Err(error) = std::fs::rename(temporary, path) {
        if had_previous {
            let _ = std::fs::rename(&backup, path);
        }
        return Err(format!("commit {}: {error}", path.display()));
    }
    if had_previous {
        let _ = std::fs::remove_file(backup);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_is_normalized_and_rejects_non_web_urls() {
        assert_eq!(
            normalized_origin("HTTPS://Example.COM:8443/path?q=1"),
            Some("https://example.com:8443".to_string())
        );
        assert_eq!(normalized_origin("file:///tmp/icon.png"), None);
        assert_eq!(normalized_origin("javascript:alert(1)"), None);
    }

    #[test]
    fn image_format_is_detected_from_bytes() {
        assert_eq!(
            image_extension("text/plain", b"\x89PNG\r\n\x1a\nrest"),
            Some("png")
        );
        assert_eq!(
            image_extension("image/x-icon", &[0, 0, 1, 0, 1]),
            Some("ico")
        );
        assert_eq!(
            image_extension("image/svg+xml", b"  <svg viewBox='0 0 1 1'/>"),
            Some("svg")
        );
        assert_eq!(
            image_extension("text/html", b"<html>not an icon</html>"),
            None
        );
    }

    #[test]
    fn cache_files_are_replaced_atomically_by_format() {
        let directory = tempfile::tempdir().unwrap();
        store(directory.path(), "key", "ico", &[0, 0, 1, 0]).unwrap();
        assert!(
            find_cached(directory.path(), "key")
                .unwrap()
                .ends_with("key.ico")
        );
        store(directory.path(), "key", "png", b"png").unwrap();
        let cached = find_cached(directory.path(), "key").unwrap();
        assert!(cached.ends_with("key.png"));
        assert!(!cache_directory(directory.path()).join("key.ico").exists());
    }

    #[test]
    fn webview_bytes_are_stored_by_normalized_origin() {
        let directory = tempfile::tempdir().unwrap();
        let png = b"\x89PNG\r\n\x1a\ncontent";
        let first = store_for_url(directory.path(), "https://EXAMPLE.com/a", png).unwrap();
        let second = store_for_url(directory.path(), "https://example.com/b", png).unwrap();
        assert_eq!(first, second);
        assert_eq!(std::fs::read(first).unwrap(), png);
    }
}
