//! Cross-platform website favicon disk cache.

use lingxia_transfer::network::{BrowserNetwork, explicit_proxy_client};
use lingxia_transfer::store::FetchRequest;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
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
/// is missing or stale, over the browser's `network`. `on_ready` runs after a new file is committed.
pub fn cached_or_request(
    cache_root: &Path,
    page_url: &str,
    network: BrowserNetwork,
    on_ready: Arc<dyn Fn() + Send + Sync>,
) -> Option<PathBuf> {
    let origin = normalized_origin(page_url)?;
    let key = cache_key(&origin);
    let cached = find_cached(cache_root, &key);
    // Blocked asks nothing and records no failure, so the icon comes once the proxy is back.
    if network != BrowserNetwork::Blocked && should_request(&origin, cached.as_deref()) {
        let cache_root = cache_root.to_path_buf();
        let request_origin = origin.clone();
        std::mem::drop(rong_rt::RongExecutor::global().spawn(async move {
            let fetched = lingxia_transfer::scope_host_network_owner(
                "favicon",
                fetch_and_store(&cache_root, &request_origin, &key, network),
            );
            let succeeded = match fetched.await {
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
    store_file(cache_root, &key, extension, bytes).ok()?;
    find_cached(cache_root, &key)
}

const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const ACCEPT_IMAGES: &str = "image/png,image/x-icon,image/*;q=0.8,*/*;q=0.1";

/// Fetches `origin`'s icon the way the browser's downloads go: refused while Blocked, only through
/// the given proxy (or direct) when Explicit, and over the shared store otherwise.
async fn fetch_and_store(
    cache_root: &Path,
    origin: &str,
    key: &str,
    network: BrowserNetwork,
) -> Result<(), String> {
    let url = format!("{origin}/favicon.ico");
    let (bytes, content_type) = match network {
        BrowserNetwork::Blocked => {
            return Err("browser proxy is disconnected".to_string());
        }
        BrowserNetwork::Explicit(proxy) => fetch_explicit(&url, proxy.as_deref()).await?,
        BrowserNetwork::Runtime => fetch_shared(&url).await?,
    };
    if bytes.is_empty() {
        return Err("empty image".to_string());
    }
    let extension = image_extension(&content_type.to_ascii_lowercase(), &bytes)
        .ok_or_else(|| "response is not a supported image".to_string())?;
    store_file(cache_root, key, extension, &bytes)
}

async fn fetch_shared(url: &str) -> Result<(Vec<u8>, String), String> {
    let store = lingxia_transfer::store::shared().ok_or("HTTP store unavailable")?;
    // Shared across apps on purpose: a site's icon is the same for every caller.
    let mut request = FetchRequest::new(url, "host:favicon");
    request.headers = vec![(http::header::ACCEPT.to_string(), ACCEPT_IMAGES.to_string())];
    request.max_bytes = MAX_FAVICON_BYTES as u64;
    request.timeout = FETCH_TIMEOUT;
    request.connect_timeout = Some(CONNECT_TIMEOUT);
    // The origin is the page's own; an http page has an http icon.
    request.allow_http = true;
    let mut bytes = Vec::new();
    let meta = store
        .fetch_into(request, &mut bytes)
        .await
        .map_err(|error| error.to_string())?;
    Ok((bytes, meta.content_type.unwrap_or_default()))
}

async fn fetch_explicit(url: &str, proxy: Option<&str>) -> Result<(Vec<u8>, String), String> {
    let client = explicit_proxy_client(proxy, Some(CONNECT_TIMEOUT), FETCH_TIMEOUT)?;
    let response = tokio::time::timeout(
        FETCH_TIMEOUT,
        client
            .get(url)
            .header(http::header::ACCEPT, ACCEPT_IMAGES)
            .send(),
    )
    .await
    .map_err(|_| "favicon request timed out".to_string())?
    .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("http status {}", response.status().as_u16()));
    }
    let content_type = response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let bytes = tokio::time::timeout(FETCH_TIMEOUT, response.bytes())
        .await
        .map_err(|_| "favicon body timed out".to_string())?
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_FAVICON_BYTES {
        return Err("favicon is too large".to_string());
    }
    Ok((bytes.to_vec(), content_type))
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

fn store_file(cache_root: &Path, key: &str, extension: &str, bytes: &[u8]) -> Result<(), String> {
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

/// Read only: a scheme request must not trigger network access to a supplied URL.
pub fn cached(cache_root: &Path, page_url: &str) -> Option<PathBuf> {
    find_cached(cache_root, &cache_key(&normalized_origin(page_url)?))
}

pub fn clear_since(cache_root: &Path, since_ms: Option<u64>) -> std::io::Result<()> {
    let directory = cache_directory(cache_root);
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        let modified = metadata
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        if since_ms.is_none_or(|since| modified >= u128::from(since)) {
            std::fs::remove_file(entry.path())?;
        }
    }
    if let Ok(mut state) = request_state().lock() {
        state.failed_at.clear();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-request HTTP server that answers with a PNG and reports the request line it got.
    fn serve_png_once() -> (std::net::SocketAddr, std::sync::mpsc::Receiver<String>) {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 2 {
                line.clear();
            }
            let body = b"\x89PNG\r\n\x1a\nicon";
            let mut stream = stream;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
            tx.send(first.trim_end().to_string()).unwrap();
        });
        (addr, rx)
    }

    #[tokio::test]
    async fn a_blocked_browser_network_refuses_the_fetch() {
        let cache = tempfile::tempdir().unwrap();
        let key = cache_key("http://favicon.invalid");
        let result = fetch_and_store(
            cache.path(),
            "http://favicon.invalid",
            &key,
            BrowserNetwork::Blocked,
        )
        .await;
        assert!(result.is_err());
        assert!(find_cached(cache.path(), &key).is_none());
    }

    #[test]
    fn a_blocked_browser_network_schedules_nothing() {
        let cache = tempfile::tempdir().unwrap();
        let on_ready: Arc<dyn Fn() + Send + Sync> = Arc::new(|| panic!("nothing was fetched"));
        let page = "http://blocked-favicon.invalid/";
        assert!(cached_or_request(cache.path(), page, BrowserNetwork::Blocked, on_ready).is_none());
        let origin = normalized_origin(page).unwrap();
        let state = request_state().lock().unwrap();
        assert!(!state.in_flight.contains(&origin) && !state.failed_at.contains_key(&origin));
    }

    #[tokio::test]
    async fn an_explicit_proxy_carries_the_fetch() {
        let (proxy, request_line) = serve_png_once();
        let cache = tempfile::tempdir().unwrap();
        let key = cache_key("http://favicon.invalid");
        let network = BrowserNetwork::Explicit(Some(format!("http://{proxy}")));
        fetch_and_store(cache.path(), "http://favicon.invalid", &key, network)
            .await
            .unwrap();
        // An absolute-form request line is a proxy request; the origin itself does not resolve.
        assert_eq!(
            request_line.recv().unwrap(),
            "GET http://favicon.invalid/favicon.ico HTTP/1.1"
        );
        assert!(
            find_cached(cache.path(), &key)
                .is_some_and(|path| path.extension() == Some("png".as_ref()))
        );
    }

    #[tokio::test]
    async fn an_explicit_direct_network_goes_straight_to_the_origin() {
        let (origin, request_line) = serve_png_once();
        let cache = tempfile::tempdir().unwrap();
        let origin = format!("http://{origin}");
        let key = cache_key(&origin);
        fetch_and_store(cache.path(), &origin, &key, BrowserNetwork::Explicit(None))
            .await
            .unwrap();
        assert_eq!(request_line.recv().unwrap(), "GET /favicon.ico HTTP/1.1");
        assert!(find_cached(cache.path(), &key).is_some());
    }

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
        store_file(directory.path(), "key", "ico", &[0, 0, 1, 0]).unwrap();
        assert!(
            find_cached(directory.path(), "key")
                .unwrap()
                .ends_with("key.ico")
        );
        store_file(directory.path(), "key", "png", b"png").unwrap();
        let cached = find_cached(directory.path(), "key").unwrap();
        assert!(cached.ends_with("key.png"));
        assert!(!cache_directory(directory.path()).join("key.ico").exists());
    }

    #[test]
    fn clear_respects_time_range_and_preserves_other_caches() {
        let directory = tempfile::tempdir().unwrap();
        let png = b"\x89PNG\r\n\x1a\ncontent";
        store_for_url(directory.path(), "https://example.com", png).unwrap();
        std::fs::write(directory.path().join("other-cache"), b"keep").unwrap();
        clear_since(directory.path(), Some(u64::MAX)).unwrap();
        assert!(cached(directory.path(), "https://example.com").is_some());
        clear_since(directory.path(), None).unwrap();
        assert!(cached(directory.path(), "https://example.com").is_none());
        assert!(directory.path().join("other-cache").exists());
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

    #[tokio::test]
    async fn a_site_icon_is_fetched_through_the_host_store() {
        use std::io::{Read, Write};
        let directory = tempfile::tempdir().unwrap();
        lingxia_transfer::store::install(
            lingxia_transfer::store::Store::open(directory.path().join("http"), 1 << 20).unwrap(),
        );
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let read = stream.read(&mut request).unwrap();
            let png = b"\x89PNG\r\n\x1a\nicon";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                png.len()
            )
            .unwrap();
            stream.write_all(png).unwrap();
            String::from_utf8_lossy(&request[..read]).to_string()
        });
        fetch_and_store(directory.path(), &origin, "site")
            .await
            .unwrap();
        assert!(server.join().unwrap().starts_with("GET /favicon.ico "));
        assert!(
            find_cached(directory.path(), "site")
                .unwrap()
                .ends_with("site.png")
        );
    }
}
