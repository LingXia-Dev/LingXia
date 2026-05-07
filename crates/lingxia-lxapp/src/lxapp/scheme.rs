use super::uri as lx_uri;
use crate::plugin;
use http::{Request, Response, StatusCode, Uri};
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_webview::WebResourceResponse;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::error;
use crate::error::LxAppError;
use crate::lxapp::LxApp;
use crate::page::PageInstance;

enum ResolvedLxAsset {
    FilePath(PathBuf),
    BuiltinAsset(String),
}

impl LxApp {
    /// Handler for lx:// scheme requests to access static app assets (images, CSS, JS, etc.)
    /// HTML files are handled separately through generate_page_html and load_data
    pub fn handle_lingxia_request(
        &self,
        page: &PageInstance,
        req: Request<Vec<u8>>,
    ) -> Option<WebResourceResponse> {
        let uri = req.uri();
        match uri.host() {
            Some(lx_uri::HOST_ASSETS) => {
                return self.handle_sdk_asset(uri);
            }
            Some(lx_uri::HOST_LXAPP)
            | Some(lx_uri::HOST_PLUGIN)
            | Some(lx_uri::HOST_TEMP)
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
            Ok(ResolvedLxAsset::BuiltinAsset(relative)) => {
                return self.handle_builtin_lxapp_asset(&relative);
            }
            Ok(ResolvedLxAsset::FilePath(path)) => path,
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
        self.touch_user_cache_access_time(&asset_path);
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

    fn resolve_lx_uri(
        &self,
        page: &PageInstance,
        uri: &Uri,
    ) -> Result<ResolvedLxAsset, LxAppError> {
        match uri.host() {
            Some(lx_uri::HOST_LXAPP) => {
                if matches!(
                    self.bundle_source,
                    crate::lxapp::LxAppBundleSource::BuiltinAssets { .. }
                ) {
                    self.resolve_lxapp_relative_path(page, uri)
                        .map(ResolvedLxAsset::BuiltinAsset)
                } else {
                    self.resolve_lxapp_uri(page, uri)
                        .map(ResolvedLxAsset::FilePath)
                }
            }
            Some(lx_uri::HOST_PLUGIN) => self
                .resolve_plugin_uri(page, uri)
                .map(ResolvedLxAsset::FilePath),
            Some(lx_uri::HOST_TEMP) => self
                .resolve_lx_path_uri(
                    &lx_uri::LxUri::from_str(&uri.to_string())
                        .map_err(|_| LxAppError::ResourceNotFound(uri.to_string()))?,
                )
                .map(ResolvedLxAsset::FilePath),
            Some(lx_uri::HOST_USER_CACHE) => self
                .resolve_user_dir_uri(uri, &self.user_cache_dir)
                .map(ResolvedLxAsset::FilePath),
            Some(lx_uri::HOST_USER_DATA) => self
                .resolve_user_dir_uri(uri, &self.user_data_dir)
                .map(ResolvedLxAsset::FilePath),
            _ => Err(LxAppError::ResourceNotFound(uri.to_string())),
        }
    }

    fn handle_builtin_lxapp_asset(&self, relative_path: &str) -> Option<WebResourceResponse> {
        let asset_root = self.bundle_source.builtin_asset_root()?;
        let asset_path = format!(
            "{}/{}",
            asset_root.trim_end_matches('/'),
            relative_path.trim_start_matches('/')
        );

        let mut reader = match self.runtime.read_asset(&asset_path) {
            Ok(reader) => reader,
            Err(e) => {
                error!("Failed to read builtin lxapp asset {}: {}", asset_path, e)
                    .with_appid(self.appid.clone());
                return Some(self.create_error_response(
                    StatusCode::NOT_FOUND,
                    "Asset Not Found",
                    &format!(
                        "The requested asset '{}' could not be found.",
                        relative_path
                    ),
                ));
            }
        };

        let mut data = Vec::new();
        if let Err(e) = reader.read_to_end(&mut data) {
            error!("Failed to read builtin lxapp asset {}: {}", asset_path, e)
                .with_appid(self.appid.clone());
            return Some(self.create_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Asset Read Error",
                "Failed to read asset data.",
            ));
        }

        let mime_type = Self::infer_mime_type(relative_path);
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

    fn resolve_lxapp_uri(&self, page: &PageInstance, uri: &Uri) -> Result<PathBuf, LxAppError> {
        let relative = self.resolve_lxapp_relative_path(page, uri)?;
        if let Ok(local_path) = self.resolve_accessible_path(&relative) {
            return Ok(local_path);
        }

        let absolute_path = format!("/{}", relative);
        if let Ok(local_path) = self.resolve_accessible_path(&absolute_path) {
            return Ok(local_path);
        }

        Err(LxAppError::ResourceNotFound(uri.to_string()))
    }

    fn resolve_lxapp_relative_path(
        &self,
        _page: &PageInstance,
        uri: &Uri,
    ) -> Result<String, LxAppError> {
        let decoded_path = lx_uri::decode_lx_path(uri.path());
        if Path::new(&decoded_path).is_absolute() {
            if let Ok(local_path) = self.resolve_accessible_path(&decoded_path) {
                let relative = local_path
                    .strip_prefix(&self.lxapp_dir)
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| decoded_path.trim_start_matches('/').to_string());
                return Ok(relative);
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
        let normalized = normalized.to_string();

        Ok(normalized)
    }

    fn resolve_plugin_uri(&self, _page: &PageInstance, uri: &Uri) -> Result<PathBuf, LxAppError> {
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

        let last_err = match plugin::resolve_plugin_resource_path(
            &self.runtime,
            &self.config.plugins,
            plugin_name,
            normalized,
        ) {
            Ok(local_path) => return Ok(local_path),
            Err(e) => e,
        };

        Err(last_err)
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

    fn touch_user_cache_access_time(&self, path: &Path) {
        if path.starts_with(&self.user_cache_dir) && path.exists() {
            crate::cache::touch_access_time(path);
        }
    }

    /// Create a simple centered error response (returned inline, no disk write).
    pub(crate) fn create_error_response(
        &self,
        status: StatusCode,
        title: &str,
        message: &str,
    ) -> WebResourceResponse {
        let html = format!(
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
        let body = html.into_bytes();

        let mut builder = Response::builder()
            .status(status)
            .header("Content-Type", "text/html; charset=utf-8")
            .header("Access-Control-Allow-Origin", "null");

        if let Ok(value) = http::HeaderValue::from_str(&body.len().to_string()) {
            builder = builder.header("Content-Length", value);
        }

        let response = builder.body(()).unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(())
                .expect("Failed to build fallback error response")
        });

        let (parts, _) = response.into_parts();
        (parts, body).into()
    }
}
