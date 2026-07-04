use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_platform_error, js_internal_error,
    js_service_unavailable_error,
};
use lingxia_platform::traits::screenshot::AppScreenshot;
use lingxia_service::storage;
use lxapp::LxApp;
use rong::function::Optional;
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSObject, JSResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default, FromJSObj)]
struct JSAppScreenshotOptions {
    /// Platform-specific window id (desktop only). Omitted: the platform
    /// captures the key/main window (desktop) or the sole window (mobile).
    #[rename = "windowId"]
    window_id: Option<String>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSAppScreenshotResult {
    #[rename = "tempFilePath"]
    temp_file_path: String,
    width: Option<u32>,
    height: Option<u32>,
}

pub(super) fn init(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    let screenshot = JSFunc::new(ctx, app_screenshot)?.name("screenshot")?;
    app.set("screenshot", screenshot)?;
    Ok(())
}

/// `lx.app.screenshot(options?)` — capture the host app's window as a PNG.
///
/// App-level semantics, one level above any page/WebView capture: the image
/// is what the user sees of the whole app — host-drawn navigation chrome,
/// native overlays, and every composited WebView, not just this lxapp's web
/// content. Because that view can include other lxapps' UI, the API is
/// restricted to the home lxapp, like the other host-level APIs on `lx.app`.
async fn app_screenshot(
    ctx: JSContext,
    options: Optional<JSAppScreenshotOptions>,
) -> JSResult<JSAppScreenshotResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    super::ensure_home_lxapp(&lxapp, "lx.app.screenshot")?;

    let window_id = options.as_ref().and_then(|o| o.window_id.clone());
    let platform = lxapp::get_platform()
        .ok_or_else(|| js_service_unavailable_error("platform is not initialized"))?;
    let bytes = platform
        .take_app_screenshot(window_id.as_deref())
        .await
        .map_err(|e| js_error_from_platform_error(&e))?;

    let (width, height) = match png_dimensions(&bytes) {
        Some((w, h)) => (Some(w), Some(h)),
        None => (None, None),
    };

    let path = generate_output_path(&lxapp.temp_dir)?;
    fs::write(&path, &bytes).map_err(|err| {
        js_internal_error(format!(
            "screenshot failed to write {}: {}",
            path.display(),
            err
        ))
    })?;
    ensure_temp_output_quota(&lxapp, &path)?;

    let uri = lxapp
        .to_uri(&path)
        .ok_or_else(|| js_internal_error("screenshot failed to convert path to lx:// uri"))?
        .into_string();

    Ok(JSAppScreenshotResult {
        temp_file_path: uri,
        width,
        height,
    })
}

fn generate_output_path(cache_root: &Path) -> JSResult<PathBuf> {
    let base_dir = cache_root.join("app-screenshot");
    fs::create_dir_all(&base_dir).map_err(|err| {
        js_internal_error(format!(
            "Failed to prepare directory {}: {}",
            base_dir.display(),
            err
        ))
    })?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    Ok(base_dir.join(format!("lx_{timestamp}.png")))
}

fn ensure_temp_output_quota(lxapp: &LxApp, path: &Path) -> JSResult<()> {
    let size = storage::path_size(path);
    storage::ensure_temp_quota(&lxapp.temp_dir, path, size)
        .map_err(|err| js_error_from_business_code_with_detail(1002, err.detail()))
}

/// Read width/height from the PNG IHDR header (always the first chunk).
fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[0..8] != PNG_SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

#[cfg(test)]
mod tests {
    use super::png_dimensions;

    #[test]
    fn png_dimensions_reads_ihdr() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&800u32.to_be_bytes());
        bytes.extend_from_slice(&600u32.to_be_bytes());
        assert_eq!(png_dimensions(&bytes), Some((800, 600)));
    }

    #[test]
    fn png_dimensions_rejects_non_png() {
        assert_eq!(png_dimensions(b"not a png"), None);
        assert_eq!(png_dimensions(&[]), None);
    }
}
