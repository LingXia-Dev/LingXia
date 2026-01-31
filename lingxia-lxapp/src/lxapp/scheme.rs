use super::uri as lx_uri;
use crate::info;
use crate::plugin;
use crate::warn;
use base64::Engine;
use base64::engine::general_purpose;
use http::{Method, Request, Response, StatusCode, Uri};
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_webview::{SystemPipeReader, WebResourceResponse};
use rong_http as net;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error;
use crate::error::LxAppError;
use crate::lxapp::LxApp;
use crate::page::Page;

impl LxApp {
    /// Handler for lx:// scheme requests to access static app assets (images, CSS, JS, etc.)
    /// HTML files are handled separately through generate_page_html and load_data
    pub(crate) fn lingxia_handler(
        &self,
        page: &Page,
        req: Request<Vec<u8>>,
    ) -> Option<WebResourceResponse> {
        let uri = req.uri();
        match uri.host() {
            Some(lx_uri::HOST_PROXY) => {
                let target_uri = match Self::extract_proxy_target(uri) {
                    Some(target_uri) => target_uri,
                    None => {
                        return Some(self.create_error_response(
                            StatusCode::BAD_REQUEST,
                            "Invalid Proxy URL",
                            "Proxy URL must be lx://proxy/<base64-https-url>.",
                        ));
                    }
                };
                return self.handle_lingxia_proxy(target_uri);
            }
            Some(lx_uri::HOST_ASSETS) => {
                return self.handle_sdk_asset(uri);
            }
            Some(lx_uri::HOST_LXAPP)
            | Some(lx_uri::HOST_PLUGIN)
            | Some(lx_uri::HOST_USER_CACHE)
            | Some(lx_uri::HOST_USER_DATA) => {}
            Some(other) => {
                error!("Unknown lx host: {}", other).with_appid(self.appid.clone());
                return Some(self.create_error_response(
                    StatusCode::BAD_REQUEST,
                    "Unknown Host",
                    &format!("Unknown lx host '{}'.", other),
                ));
            }
            None => {
                error!("lx request missing host: {}", uri).with_appid(self.appid.clone());
                return Some(self.create_error_response(
                    StatusCode::BAD_REQUEST,
                    "Invalid URL",
                    "The URL is missing a host component and cannot be processed.",
                ));
            }
        }

        let asset_path = match self.resolve_lx_uri(page, uri) {
            Ok(path) => path,
            Err(e) => {
                error!("resolve_lx_uri failed for {}: {}", uri.path(), e)
                    .with_appid(self.appid.clone());
                return Some(self.create_error_response(
                    StatusCode::NOT_FOUND,
                    "Asset Not Found",
                    &format!("The requested asset '{}' could not be found.", uri.path()),
                ));
            }
        };

        let metadata = match fs::metadata(&asset_path) {
            Ok(meta) => meta,
            Err(e) => {
                error!(
                    "Failed to read asset metadata: {} - {}",
                    asset_path.display(),
                    e
                )
                .with_appid(self.appid.clone());
                return Some(self.create_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Asset Error",
                    "Failed to read asset metadata.",
                ));
            }
        };

        let file_len = metadata.len();
        let mime_type = Self::infer_mime_type(uri.path());

        let mut builder = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", mime_type)
            .header("Access-Control-Allow-Origin", "null");

        if let Ok(value) = http::HeaderValue::from_str(&file_len.to_string()) {
            builder = builder.header("Content-Length", value);
        }

        let response = builder.body(()).unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(())
                .expect("Failed to build fallback empty response")
        });

        let (parts, _) = response.into_parts();
        Some((parts, asset_path).into())
    }

    fn handle_sdk_asset(&self, uri: &Uri) -> Option<WebResourceResponse> {
        let path = uri.path().trim_start_matches('/');
        if path.is_empty() {
            return Some(self.create_error_response(
                StatusCode::BAD_REQUEST,
                "Invalid Asset Path",
                "Asset path is empty.",
            ));
        }
        if lx_uri::has_invalid_segment(path) {
            return Some(self.create_error_response(
                StatusCode::BAD_REQUEST,
                "Invalid Asset Path",
                "Asset path contains invalid segments.",
            ));
        }

        let mut reader = match self.runtime.read_asset(path) {
            Ok(reader) => reader,
            Err(e) => {
                error!("Failed to read sdk asset {}: {}", path, e).with_appid(self.appid.clone());
                return Some(self.create_error_response(
                    StatusCode::NOT_FOUND,
                    "Asset Not Found",
                    &format!("The requested asset '{}' could not be found.", path),
                ));
            }
        };

        let mut data = Vec::new();
        if let Err(e) = reader.read_to_end(&mut data) {
            error!("Failed to read sdk asset {}: {}", path, e).with_appid(self.appid.clone());
            return Some(self.create_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Asset Read Error",
                "Failed to read asset data.",
            ));
        }

        let mime_type = Self::infer_mime_type(path);
        let mut builder = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", mime_type)
            .header("Access-Control-Allow-Origin", "null");

        if let Ok(value) = http::HeaderValue::from_str(&data.len().to_string()) {
            builder = builder.header("Content-Length", value);
        }

        let response = builder.body(()).unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(())
                .expect("Failed to build fallback empty response")
        });

        let (parts, _) = response.into_parts();
        Some((parts, data).into())
    }

    fn handle_lingxia_proxy(&self, target_uri: Uri) -> Option<WebResourceResponse> {
        let target_str = target_uri.to_string();

        //info!("lingxia_proxy: forwarding request to {}", target_str).with_appid(self.appid.clone());

        if target_uri.scheme_str() != Some("https") {
            return Some(self.create_error_response(
                StatusCode::BAD_REQUEST,
                "Unsupported Scheme",
                "Only https URLs are allowed in proxy requests.",
            ));
        }

        let proxy_request = match Request::builder()
            .method(Method::GET)
            .uri(target_uri)
            .body(Vec::new())
        {
            Ok(req) => req,
            Err(e) => {
                error!(
                    "lingxia_proxy: failed to build proxy request {}: {}",
                    target_str, e
                )
                .with_appid(self.appid.clone());

                return Some(self.create_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Proxy Request Error",
                    &format!("Failed to prepare proxy request: {}", e),
                ));
            }
        };

        self.https_handler(proxy_request)
    }

    fn extract_proxy_target(uri: &Uri) -> Option<Uri> {
        if uri.host() != Some(lx_uri::HOST_PROXY) {
            return None;
        }

        let encoded = uri.path().trim_start_matches('/');
        if encoded.is_empty() {
            return None;
        }

        let decoded = general_purpose::URL_SAFE_NO_PAD
            .decode(encoded)
            .or_else(|_| general_purpose::URL_SAFE.decode(encoded))
            .ok()?;
        let decoded_str = std::str::from_utf8(&decoded).ok()?.trim();
        if !decoded_str.starts_with("https://") {
            return None;
        }

        Uri::from_str(decoded_str).ok()
    }

    fn resolve_lx_uri(&self, page: &Page, uri: &Uri) -> Result<PathBuf, LxAppError> {
        match uri.host() {
            Some(lx_uri::HOST_LXAPP) => self.resolve_lxapp_uri(page, uri),
            Some(lx_uri::HOST_PLUGIN) => self.resolve_plugin_uri(page, uri),
            Some(lx_uri::HOST_USER_CACHE) => self.resolve_user_dir_uri(uri, &self.user_cache_dir),
            Some(lx_uri::HOST_USER_DATA) => self.resolve_user_dir_uri(uri, &self.user_data_dir),
            _ => Err(LxAppError::ResourceNotFound(uri.to_string())),
        }
    }

    fn resolve_user_dir_uri(&self, uri: &Uri, base_dir: &Path) -> Result<PathBuf, LxAppError> {
        let decoded_path = lx_uri::decode_lx_path(uri.path());
        let rel = decoded_path.trim_matches('/');
        if rel.is_empty()
            || lx_uri::has_invalid_segment(rel)
            || rel.contains(':')
            || rel.contains('\\')
        {
            return Err(LxAppError::ResourceNotFound(uri.to_string()));
        }

        let absolute = base_dir.join(rel);
        let absolute_str = absolute.to_string_lossy();
        self.resolve_accessible_path(absolute_str.as_ref())
    }

    fn resolve_lxapp_uri(&self, page: &Page, uri: &Uri) -> Result<PathBuf, LxAppError> {
        let decoded_path = lx_uri::decode_lx_path(uri.path());
        if Path::new(&decoded_path).is_absolute() {
            if let Ok(local_path) = self.resolve_accessible_path(&decoded_path) {
                return Ok(local_path);
            }
        }
        let raw_path = decoded_path.trim_start_matches('/');
        // Support both:
        // - lx://lxapp/<appid>/<path> (explicit)
        // - lx://lxapp/<path>         (implicit appid = current page's appid)
        //
        // The implicit form enables common web patterns like <img src="/public/logo.png">,
        // which resolve (via URL rules) to lx://lxapp/public/logo.png (appid omitted).
        let (first, rest) = raw_path.split_once('/').unwrap_or(("", raw_path));
        let normalized = if first == self.appid.as_str() {
            rest.trim_matches('/')
        } else {
            raw_path.trim_matches('/')
        };
        if normalized.is_empty() {
            return Err(LxAppError::ResourceNotFound(uri.to_string()));
        }
        if lx_uri::has_invalid_segment(normalized) {
            return Err(LxAppError::ResourceNotFound(uri.to_string()));
        }

        if let Ok(local_path) = self.resolve_accessible_path(normalized) {
            return Ok(local_path);
        }

        if let Some(stripped) =
            lx_uri::strip_base_dir(page, normalized, lx_uri::HOST_LXAPP, &self.appid)
        {
            if let Ok(local_path) = self.resolve_accessible_path(&stripped) {
                return Ok(local_path);
            }
        }

        let absolute_path = format!("/{}", normalized);
        if let Ok(local_path) = self.resolve_accessible_path(&absolute_path) {
            return Ok(local_path);
        }

        Err(LxAppError::ResourceNotFound(uri.to_string()))
    }

    fn resolve_plugin_uri(&self, page: &Page, uri: &Uri) -> Result<PathBuf, LxAppError> {
        let decoded_path = lx_uri::decode_lx_path(uri.path());
        let raw_path = decoded_path.trim_start_matches('/');
        let (plugin_name, rest) = raw_path
            .split_once('/')
            .ok_or_else(|| LxAppError::ResourceNotFound(uri.to_string()))?;

        let normalized = rest.trim_matches('/');
        if normalized.is_empty() {
            return Err(LxAppError::ResourceNotFound(uri.to_string()));
        }
        if lx_uri::has_invalid_segment(normalized) {
            return Err(LxAppError::ResourceNotFound(uri.to_string()));
        }

        let mut last_err = match plugin::resolve_plugin_resource_path(
            &self.runtime,
            &self.config.plugins,
            plugin_name,
            normalized,
        ) {
            Ok(local_path) => return Ok(local_path),
            Err(e) => e,
        };

        if let Some(stripped) =
            lx_uri::strip_base_dir(page, normalized, lx_uri::HOST_PLUGIN, plugin_name)
        {
            match plugin::resolve_plugin_resource_path(
                &self.runtime,
                &self.config.plugins,
                plugin_name,
                &stripped,
            ) {
                Ok(local_path) => return Ok(local_path),
                Err(e) => last_err = e,
            }
        }

        Err(last_err)
    }

    /// Handler for HTTPS requests:
    /// - Only accepts GET; other methods are rejected
    /// - All GETs to allowed domains are treated as downloadable resources
    ///   with per-app cache; miss -> stream via system pipe while saving
    pub(crate) fn https_handler(&self, req: Request<Vec<u8>>) -> Option<WebResourceResponse> {
        let uri = req.uri();

        // Check if the domain is allowed
        if let Some(host) = uri.host() {
            // First check domain whitelist
            if !self
                .state
                .lock()
                .unwrap()
                .network_security
                .is_domain_allowed(host)
            {
                return Some(self.create_error_response(
                    StatusCode::FORBIDDEN,
                    "Domain Access Denied",
                    &format!(
                        "Access to domain '{}' is not allowed by the security policy.",
                        host
                    ),
                ));
            }

            // Only accept GET; reject others
            if req.method() != http::Method::GET {
                return Some(self.create_error_response(
                    StatusCode::METHOD_NOT_ALLOWED,
                    "Method Not Allowed",
                    &format!(
                        "Only GET is allowed in WebView: method={} {}",
                        req.method(),
                        uri
                    ),
                ));
            }

            let url_str = uri.to_string();
            // Decide extension from URL or query
            let ext_opt = url_ext_from_uri(uri);
            let ext = ext_opt.as_deref().unwrap_or("bin");
            let cache = match self.cache() {
                Ok(c) => c,
                Err(e) => {
                    return Some(self.create_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Cache Not Ready",
                        &format!("Cache unavailable: {}", e),
                    ));
                }
            };

            match cache.resolve_path_with_ext(&url_str, ext) {
                crate::cache::ResolveResult::Exists(file_path) => {
                    info!(
                        "https cache hit -> url={}, file={}",
                        url_str,
                        file_path.display()
                    )
                    .with_appid(self.appid.clone());
                    // Cached: serve file path
                    let mime_type = ext_opt
                        .as_deref()
                        .map(Self::infer_mime_type_ext)
                        .unwrap_or_else(|| Self::infer_mime_type(uri.path()));
                    let builder = http::Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", mime_type)
                        .header("Access-Control-Allow-Origin", "null");
                    let response = builder.body(()).unwrap_or_else(|_| {
                        http::Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(())
                            .expect("Failed to build cached file response")
                    });
                    let (parts, _) = response.into_parts();
                    return Some((parts, file_path).into());
                }
                crate::cache::ResolveResult::NonExists(dest_path) => {
                    #[cfg(unix)]
                    {
                        // Coordinate in-flight downloads using a file lock next to cache destination.
                        let hash_id = dest_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or_default()
                            .to_string();
                        let lock_path = dest_path
                            .parent()
                            .unwrap_or_else(|| std::path::Path::new("."))
                            .join(format!("{}.lock", hash_id));
                        let part_path = dest_path.with_extension("part");

                        let try_acquire_lock = || {
                            fs::OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .open(&lock_path)
                                .is_ok()
                        };

                        let mut acquired_lock = try_acquire_lock();

                        if !acquired_lock {
                            // Self-heal: stale lock/part can be left behind when the process is killed
                            // mid-download (common during dev). We treat a lock as stale when:
                            // - `.part` doesn't exist OR hasn't been modified recently, and
                            // - the lock itself is older than a small threshold.
                            const STALE_AFTER: Duration = Duration::from_secs(60);
                            let now = SystemTime::now();

                            let part_recent = fs::metadata(&part_path)
                                .and_then(|m| m.modified())
                                .ok()
                                .and_then(|t| now.duration_since(t).ok())
                                .map(|age| age <= STALE_AFTER)
                                .unwrap_or(false);

                            let lock_old = fs::metadata(&lock_path)
                                .and_then(|m| m.modified())
                                .ok()
                                .and_then(|t| now.duration_since(t).ok())
                                .map(|age| age > STALE_AFTER)
                                .unwrap_or(false);

                            if lock_old && !part_recent {
                                let _ = fs::remove_file(&lock_path);
                                let _ = fs::remove_file(&part_path);
                                warn!(
                                    "https cache: removed stale lock/part -> url={}, dest={}",
                                    url_str,
                                    dest_path.display()
                                )
                                .with_appid(self.appid.clone());
                                acquired_lock = try_acquire_lock();
                            }
                        }

                        if !acquired_lock {
                            info!(
                                "https cache: in-flight lock exists, stream without caching -> url={}",
                                url_str
                            )
                            .with_appid(self.appid.clone());
                            // Another download is in progress. Serve existing cache file if present,
                            // otherwise stream a separate download without touching the cache path.
                            if dest_path.exists() {
                                let mime_type = ext_opt
                                    .as_deref()
                                    .map(Self::infer_mime_type_ext)
                                    .unwrap_or_else(|| Self::infer_mime_type(uri.path()));
                                let builder = http::Response::builder()
                                    .status(StatusCode::OK)
                                    .header("Content-Type", mime_type)
                                    .header("Access-Control-Allow-Origin", "null");
                                let response = builder.body(()).unwrap_or_else(|_| {
                                    http::Response::builder()
                                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                                        .body(())
                                        .expect("Failed to build cached file response")
                                });
                                let (parts, _) = response.into_parts();
                                return Some((parts, dest_path).into());
                            }

                            let unique_suffix = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .map(|d| d.as_millis())
                                .unwrap_or(0);
                            let tmp_dest_path = dest_path
                                .parent()
                                .unwrap_or_else(|| std::path::Path::new("."))
                                .join(format!("{}.stream.{}", hash_id, unique_suffix));
                            let tmp_part_path = tmp_dest_path.with_extension("part");

                            match create_pipe_sink() {
                                Ok((reader, sink)) => {
                                    let mime_type = ext_opt
                                        .as_deref()
                                        .map(Self::infer_mime_type_ext)
                                        .unwrap_or_else(|| Self::infer_mime_type(uri.path()));
                                    let builder = http::Response::builder()
                                        .status(StatusCode::OK)
                                        .header("Content-Type", mime_type)
                                        .header("Access-Control-Allow-Origin", "null");

                                    let response = builder.body(()).unwrap_or_else(|_| {
                                        http::Response::builder()
                                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                                            .body(())
                                            .expect("Failed to build pipe response")
                                    });
                                    let (parts, _) = response.into_parts();

                                    match net::request_download(
                                        url_str.clone(),
                                        tmp_dest_path.clone(),
                                        None,
                                        Some(sink),
                                    ) {
                                        Ok(rx) => {
                                            let cleanup_tmp_dest_path = tmp_dest_path.clone();
                                            let cleanup_tmp_part_path = tmp_part_path.clone();
                                            let spawned = rong::bg::spawn(async move {
                                                let _ = rx.await;
                                                let _ = fs::remove_file(&cleanup_tmp_dest_path);
                                                let _ = fs::remove_file(&cleanup_tmp_part_path);
                                            });
                                            if spawned.is_err() {
                                                let _ = fs::remove_file(&tmp_dest_path);
                                                let _ = fs::remove_file(&tmp_part_path);
                                            }
                                            return Some((parts, reader).into());
                                        }
                                        Err(e) => {
                                            error!(
                                                "https cache: failed to start streaming download: url={}, err={}",
                                                url_str, e
                                            )
                                            .with_appid(self.appid.clone());
                                            return Some(self.create_error_response(
                                                StatusCode::BAD_GATEWAY,
                                                "Download Failed",
                                                &format!("Failed to start download: {}", e),
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!(
                                        "https cache: failed to create pipe sink: url={}, err={}",
                                        url_str, e
                                    )
                                    .with_appid(self.appid.clone());
                                    return Some(self.create_error_response(
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        "Pipe Creation Failed",
                                        &e,
                                    ));
                                }
                            }
                        }

                        match create_pipe_sink() {
                            Ok((reader, sink)) => {
                                // info!(
                                //     "https_handler: cache miss, start pipe download -> {}",
                                //     dest_path.display()
                                // )
                                // .with_appid(self.appid.clone());

                                let mime_type = ext_opt
                                    .as_deref()
                                    .map(Self::infer_mime_type_ext)
                                    .unwrap_or_else(|| Self::infer_mime_type(uri.path()));
                                let builder = http::Response::builder()
                                    .status(StatusCode::OK)
                                    .header("Content-Type", mime_type)
                                    .header("Access-Control-Allow-Origin", "null");

                                let response = builder.body(()).unwrap_or_else(|_| {
                                    http::Response::builder()
                                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                                        .body(())
                                        .expect("Failed to build pipe response")
                                });
                                let (parts, _) = response.into_parts();

                                match net::request_download(
                                    url_str.clone(),
                                    dest_path.clone(),
                                    None,
                                    Some(sink),
                                ) {
                                    Ok(rx) => {
                                        let cleanup_lock_path = lock_path.clone();
                                        let cleanup_part_path = part_path.clone();
                                        let spawned = rong::bg::spawn(async move {
                                            let res = rx.await.unwrap_or_else(|_| {
                                                Err("download dropped".to_string())
                                            });
                                            let _ = fs::remove_file(&cleanup_lock_path);
                                            if res.is_err() {
                                                let _ = fs::remove_file(&cleanup_part_path);
                                            }
                                        });
                                        if spawned.is_err() {
                                            let _ = fs::remove_file(&lock_path);
                                            let _ = fs::remove_file(&part_path);
                                        }
                                        info!(
                                            "https cache: streaming via pipe -> url={}, dest={}",
                                            url_str,
                                            dest_path.display()
                                        )
                                        .with_appid(self.appid.clone());
                                        return Some((parts, reader).into());
                                    }
                                    Err(e) => {
                                        let _ = fs::remove_file(&lock_path);
                                        let _ = fs::remove_file(&part_path);
                                        error!(
                                            "https cache: failed to start download: url={}, err={}",
                                            url_str, e
                                        )
                                        .with_appid(self.appid.clone());
                                        return Some(self.create_error_response(
                                            StatusCode::BAD_GATEWAY,
                                            "Download Failed",
                                            &format!("Failed to start download: {}", e),
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = fs::remove_file(&lock_path);
                                error!(
                                    "https cache: failed to create pipe sink: url={}, err={}",
                                    url_str, e
                                )
                                .with_appid(self.appid.clone());
                                return Some(self.create_error_response(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Pipe Creation Failed",
                                    &e,
                                ));
                            }
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        // Fallback: no pipe support here; return 501
                        warn!(
                            "https_handler: pipe unsupported on this platform for {}",
                            uri
                        )
                        .with_appid(self.appid.clone());
                        return Some(self.create_error_response(
                            StatusCode::NOT_IMPLEMENTED,
                            "Pipe Unsupported",
                            "Streaming via pipe is not supported on this platform.",
                        ));
                    }
                }
            }
        }

        // URI doesn't have a host component
        Some(self.create_error_response(
            StatusCode::BAD_REQUEST,
            "Invalid URL",
            "The URL is missing a host component and cannot be processed.",
        ))
    }

    fn infer_mime_type(path: &str) -> &'static str {
        if path.ends_with(".js") {
            "application/javascript"
        } else if path.ends_with(".css") {
            "text/css"
        } else if path.ends_with(".png") {
            "image/png"
        } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
            "image/jpeg"
        } else if path.ends_with(".gif") {
            "image/gif"
        } else if path.ends_with(".svg") {
            "image/svg+xml"
        } else if path.ends_with(".webp") {
            "image/webp"
        } else if path.ends_with(".ico") {
            "image/x-icon"
        } else if path.ends_with(".json") {
            "application/json"
        } else if path.ends_with(".woff") {
            "font/woff"
        } else if path.ends_with(".woff2") {
            "font/woff2"
        } else if path.ends_with(".ttf") {
            "font/ttf"
        } else if path.ends_with(".mp3") {
            "audio/mpeg"
        } else if path.ends_with(".wav") {
            "audio/wav"
        } else if path.ends_with(".mp4") {
            "video/mp4"
        } else {
            "application/octet-stream"
        }
    }

    fn infer_mime_type_ext(ext: &str) -> &'static str {
        match ext.to_ascii_lowercase().as_str() {
            "js" => "application/javascript",
            "css" => "text/css",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "webp" => "image/webp",
            "ico" => "image/x-icon",
            "json" => "application/json",
            "woff" => "font/woff",
            "woff2" => "font/woff2",
            "ttf" => "font/ttf",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "mp4" => "video/mp4",
            _ => "application/octet-stream",
        }
    }

    fn sanitize_for_filename(value: &str) -> String {
        value
            .chars()
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' => c,
                _ => '_',
            })
            .collect()
    }
    /// Create a simple centered error response
    pub(crate) fn create_error_response(
        &self,
        status: StatusCode,
        title: &str,
        message: &str,
    ) -> WebResourceResponse {
        let html_content = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>{}</title>
    <style>
        body {{ font-family: system-ui, sans-serif; margin: 0; padding: 20px; background: #f5f5f5; display: flex; justify-content: center; align-items: center; min-height: 100vh; }}
        .error {{ background: white; border-radius: 8px; padding: 40px; text-align: center; max-width: 500px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
        .code {{ font-size: 48px; font-weight: bold; color: #e74c3c; margin-bottom: 16px; }}
        .title {{ font-size: 24px; font-weight: 600; color: #2c3e50; margin-bottom: 16px; }}
        .message {{ font-size: 16px; color: #7f8c8d; line-height: 1.5; }}
    </style>
</head>
<body>
    <div class="error">
        <div class="code">{}</div>
        <div class="title">{}</div>
        <div class="message">{}</div>
    </div>
</body>
</html>"#,
            title,
            status.as_u16(),
            title,
            message
        );

        let mut target_dir = self.user_cache_dir.join("webview_errors");
        if let Err(e) = fs::create_dir_all(&target_dir) {
            error!("Failed to prepare error directory: {}", e).with_appid(self.appid.clone());
            target_dir = std::env::temp_dir().join("lingxia-webview-errors");
            let _ = fs::create_dir_all(&target_dir);
        }

        let file_name = format!(
            "{}_{}.html",
            status.as_u16(),
            Self::sanitize_for_filename(title)
        );
        let mut file_path = target_dir.join(file_name);

        if let Err(e) = fs::write(&file_path, html_content.as_bytes()) {
            error!(
                "Failed to write error response file ({}): {}",
                file_path.display(),
                e
            )
            .with_appid(self.appid.clone());
            let unique_suffix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            file_path = target_dir.join(format!(
                "{}_{}_{}.html",
                status.as_u16(),
                Self::sanitize_for_filename(title),
                unique_suffix
            ));
            let _ = fs::write(&file_path, html_content.as_bytes());
        }

        let file_len = fs::metadata(&file_path)
            .map(|meta| meta.len())
            .unwrap_or_else(|_| html_content.len() as u64);

        let mut builder = Response::builder()
            .status(status)
            .header("Content-Type", "text/html; charset=utf-8");

        if let Ok(value) = http::HeaderValue::from_str(&file_len.to_string()) {
            builder = builder.header("Content-Length", value);
        }

        let response = builder.body(()).unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(())
                .expect("Failed to build fallback error response")
        });

        let (parts, _) = response.into_parts();
        (parts, file_path).into()
    }
}

fn url_ext_from_uri(uri: &Uri) -> Option<String> {
    // Try path first
    if let Some(ext) = ext_from_segment(uri.path()) {
        return Some(ext.to_string());
    }
    // Then try query (e.g., id=...UHD.jpg)
    if let Some(q) = uri.query() {
        let lower = q.to_ascii_lowercase();
        if let Some(pos) = lower.rfind('.') {
            let tail = &lower[pos + 1..];
            let end: String = tail
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if !end.is_empty() && end.len() <= 8 {
                return Some(end);
            }
        }
    }
    None
}

fn ext_from_segment(path: &str) -> Option<&str> {
    let seg = path.rsplit('/').next().unwrap_or(path);
    let dot = seg.rfind('.')?;
    let ext = &seg[dot + 1..];
    if !ext.is_empty() && ext.len() <= 8 {
        Some(ext)
    } else {
        None
    }
}

#[cfg(unix)]
struct PipeBodySink {
    writer: std::os::unix::net::UnixStream,
}

#[cfg(unix)]
impl net::BodySink for PipeBodySink {
    fn write(&mut self, chunk: &[u8]) -> Result<(), String> {
        use std::io::Write;
        self.writer
            .write_all(chunk)
            .map_err(|e| format!("pipe write: {}", e))
    }

    fn close(&mut self, _result: &Result<(), String>) {
        use std::net::Shutdown;
        let _ = self.writer.shutdown(Shutdown::Write);
    }
}

#[cfg(unix)]
fn create_pipe_sink() -> Result<(SystemPipeReader, Box<dyn net::BodySink + Send>), String> {
    use std::os::fd::IntoRawFd;
    use std::os::unix::net::UnixStream;

    let (read_end, write_end) = UnixStream::pair().map_err(|e| format!("pipe: {}", e))?;
    let read_fd = read_end.into_raw_fd();
    let reader = unsafe { SystemPipeReader::from_raw_fd(read_fd) };
    let sink: Box<dyn net::BodySink + Send> = Box::new(PipeBodySink { writer: write_end });
    Ok((reader, sink))
}
