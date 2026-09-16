use crate::dismissal::{USER_DISMISSED, canceled, completed};
use crate::i18n::{
    js_error_from_lxapp_error, js_error_from_platform_error, js_internal_error,
    js_invalid_parameter_error,
};
use crate::share::is_platform_file_reference;
use lingxia_platform::error::PlatformError;
use lingxia_platform::traits::clipboard::{
    ClipboardContents, ClipboardKind, ClipboardReadRequest, ClipboardService, ClipboardWrite,
};
use lxapp::LxApp;
use rong::function::Optional;
use rong::{JSContext, JSObject, JSResult, JSValue};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const TEXT_MAX_BYTES: usize = 1024 * 1024;

/// System clipboard. Text is available on every host; other item types reject
/// when this host cannot represent them. The runtime never presents a toast on
/// write — call `lx.showToast` if the product wants one (Android 13+ shows the
/// system's own "copied" notice, which no app can suppress).
fn namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("clipboard") {
        Ok(namespace) => Ok(namespace),
        Err(_) => {
            let namespace = JSObject::new(ctx);
            lx.set("clipboard", namespace.clone())?;
            Ok(namespace)
        }
    }
}

/// Replace the clipboard with Unicode text.
///
/// Empty string is a valid payload (it is not `clear()`). The runtime does not
/// present a toast — call `lx.showToast` if the product wants one. Rejects
/// `E_INVALID_ARG` above 1 MiB.
async fn write_text(ctx: JSContext, text: String) -> JSResult<()> {
    write_item(&ctx, ClipboardWrite::Text(validate_text(&text)?)).await
}

/// Read Unicode text.
///
/// Resolves `{ canceled: true }` only when the user dismisses the OS paste
/// prompt (iOS 16+, macOS 15.4+). No text representation (empty clipboard, or
/// image-only) resolves `{ canceled: false, empty: true }`. A copied empty
/// string resolves `{ canceled: false, empty: false, text: '' }`. Rejects 3008
/// when the host denies clipboard access outright: a macOS "never allow"
/// setting, or HarmonyOS without `ohos.permission.READ_PASTEBOARD`.
async fn read_text(ctx: JSContext) -> JSResult<JSObject> {
    let contents = read_contents(&ctx, Some(ClipboardKind::Text)).await?;
    if contents.canceled {
        return canceled(&ctx);
    }
    let result = completed(&ctx)?;
    match contents.text {
        Some(text) => {
            result.set("empty", false)?;
            result.set("text", text)?;
        }
        None => {
            result.set("empty", true)?;
        }
    }
    Ok(result)
}

/// Replace the clipboard with a typed item.
///
/// `type: 'text'` is universal. Other types reject when unsupported.
async fn write(ctx: JSContext, item: JSValue) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let item = parse_write_item(&lxapp, item)?;
    write_item(&ctx, item).await
}

/// Read the clipboard.
///
/// Omit `type` to receive every representation this host can surface.
/// Pass `type` to request one; if that representation is absent, the
/// completed result is `{ empty: true }` rather than a mismatch error.
/// Images arrive as a temporary PNG under `lx://temp`. Dismissal and
/// permission behave as in `readText`.
async fn read(ctx: JSContext, options: Optional<JSValue>) -> JSResult<JSObject> {
    let kind = parse_read_kind(options.0)?;
    let contents = read_contents(&ctx, kind).await?;
    contents_to_read_result(&ctx, contents)
}

/// Remove every representation.
async fn clear(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .clipboard_clear()
        .await
        .map_err(map_platform_error)
}

/// Which representations are present, without reading payloads.
///
/// Never shows the OS paste prompt: every host can peek types without
/// reading. `canceled` is reserved for hosts that cannot, so branch on it
/// anyway. The answer is a hint — content may change before you read it.
async fn types(ctx: JSContext) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let peeked = match lxapp.runtime.clipboard_types().await {
        Ok(peeked) => peeked,
        Err(PlatformError::BusinessError(USER_DISMISSED)) => return canceled(&ctx),
        Err(error) => return Err(map_platform_error(error)),
    };
    if peeked.canceled {
        return canceled(&ctx);
    }
    let result = completed(&ctx)?;
    let list = rong::JSArray::new(&ctx)?;
    for kind in peeked.kinds {
        list.push(kind.as_str())?;
    }
    result.set("types", list)?;
    Ok(result)
}

async fn write_item(ctx: &JSContext, item: ClipboardWrite) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(ctx)?;
    lxapp
        .runtime
        .clipboard_write(item)
        .await
        .map_err(map_platform_error)
}

async fn read_contents(
    ctx: &JSContext,
    kind: Option<ClipboardKind>,
) -> JSResult<ClipboardContents> {
    let lxapp = LxApp::from_ctx(ctx)?;
    let image_output_path = if kind != Some(ClipboardKind::Text) {
        Some(allocate_image_output(&lxapp.temp_dir)?)
    } else {
        None
    };
    match lxapp
        .runtime
        .clipboard_read(ClipboardReadRequest {
            kind,
            image_output_path,
        })
        .await
    {
        Ok(contents) => Ok(contents),
        Err(PlatformError::BusinessError(USER_DISMISSED)) => Ok(ClipboardContents::canceled()),
        Err(error) => Err(map_platform_error(error)),
    }
}

fn contents_to_read_result(ctx: &JSContext, contents: ClipboardContents) -> JSResult<JSObject> {
    if contents.canceled {
        return canceled(ctx);
    }
    let result = completed(ctx)?;
    if contents.is_empty() {
        result.set("empty", true)?;
        return Ok(result);
    }
    result.set("empty", false)?;
    let items = rong::JSArray::new(ctx)?;
    if let Some(text) = contents.text {
        let item = JSObject::new(ctx);
        item.set("type", "text")?;
        item.set("text", text)?;
        items.push(item)?;
    }
    if let Some(image_path) = contents.image_path {
        let lxapp = LxApp::from_ctx(ctx)?;
        let uri = to_managed_uri(&lxapp, Path::new(&image_path))?;
        let item = JSObject::new(ctx);
        item.set("type", "image")?;
        item.set("filePath", uri)?;
        if let Some(mime) = contents.image_mime.filter(|value| !value.is_empty()) {
            item.set("mimeType", mime)?;
        }
        items.push(item)?;
    }
    result.set("items", items)?;
    Ok(result)
}

fn parse_write_item(lxapp: &LxApp, value: JSValue) -> JSResult<ClipboardWrite> {
    let Some(obj) = value.into_object() else {
        return Err(js_invalid_parameter_error(
            "clipboard.write item must be an object",
        ));
    };
    let kind = obj.get::<_, String>("type").map_err(|_| {
        js_invalid_parameter_error("clipboard.write item requires type: 'text' | 'image'")
    })?;
    match kind.as_str() {
        "text" => {
            let text = obj.get::<_, String>("text").map_err(|_| {
                js_invalid_parameter_error("clipboard.write text item requires string `text`")
            })?;
            Ok(ClipboardWrite::Text(validate_text(&text)?))
        }
        "image" => {
            let file_path = obj.get::<_, String>("filePath").map_err(|_| {
                js_invalid_parameter_error("clipboard.write image item requires string `filePath`")
            })?;
            Ok(ClipboardWrite::Image {
                path: resolve_share_like_file(lxapp, &file_path)?,
            })
        }
        other => Err(js_invalid_parameter_error(format!(
            "unknown clipboard type '{other}'; expected text, image"
        ))),
    }
}

fn parse_read_kind(options: Option<JSValue>) -> JSResult<Option<ClipboardKind>> {
    let Some(value) = options else {
        return Ok(None);
    };
    let Some(obj) = value.into_object() else {
        return Err(js_invalid_parameter_error(
            "clipboard.read options must be an object",
        ));
    };
    let Some(kind) = obj.get_opt::<_, String>("type")? else {
        return Ok(None);
    };
    ClipboardKind::parse(&kind)
        .ok_or_else(|| {
            js_invalid_parameter_error(format!(
                "unknown clipboard type '{kind}'; expected text, image"
            ))
        })
        .map(Some)
}

fn validate_text(text: &str) -> JSResult<String> {
    if text.len() > TEXT_MAX_BYTES {
        return Err(js_invalid_parameter_error(format!(
            "clipboard text exceeds {TEXT_MAX_BYTES} bytes"
        )));
    }
    Ok(text.to_string())
}

fn resolve_share_like_file(lxapp: &LxApp, file: &str) -> JSResult<String> {
    let path = file.trim();
    if path.is_empty() {
        return Err(js_invalid_parameter_error(
            "clipboard image filePath is required",
        ));
    }
    if is_platform_file_reference(path) && lxapp.has_transient_file_reference(path) {
        return Ok(path.to_string());
    }
    if is_platform_file_reference(path) || !path.starts_with("lx://") {
        return Err(js_invalid_parameter_error(
            "clipboard image filePath must be an lx:// path or a file returned by lx.chooseFile/lx.chooseMedia",
        ));
    }
    let resolved = lxapp
        .resolve_accessible_path(path)
        .map_err(|e| js_error_from_lxapp_error(&e))?;
    let metadata =
        fs::metadata(&resolved).map_err(|e| js_invalid_parameter_error(e.to_string()))?;
    if !metadata.is_file() {
        return Err(js_invalid_parameter_error(format!(
            "clipboard image filePath is not a file: {}",
            resolved.display()
        )));
    }
    Ok(resolved.to_string_lossy().to_string())
}

fn allocate_image_output(temp_dir: &Path) -> JSResult<String> {
    let dir = temp_dir.join("clipboard");
    fs::create_dir_all(&dir)
        .map_err(|e| js_internal_error(format!("Failed to prepare clipboard temp dir: {e}")))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    Ok(dir
        .join(format!("lx_{stamp}.png"))
        .to_string_lossy()
        .into_owned())
}

fn to_managed_uri(lxapp: &LxApp, path: &Path) -> JSResult<String> {
    lxapp
        .to_uri(path)
        .ok_or_else(|| js_internal_error("clipboard failed to convert image path to lx:// uri"))
        .map(|uri| uri.into_string())
}

fn map_platform_error(error: PlatformError) -> rong::RongJSError {
    match error {
        // Reads map dismissal to `{ canceled: true }` before reaching here.
        // A write/clear has no prompt to dismiss, so a host sending 2000 for
        // it is a real failure — never let it read as "the user said no".
        PlatformError::BusinessError(USER_DISMISSED) => js_error_from_platform_error(
            &PlatformError::Platform("clipboard host reported dismissal for a write".into()),
        ),
        other => js_error_from_platform_error(&other),
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_property(ctx)?;
    register_api(ctx)
}

rong::js_api! {
    fn register_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const clipboard: "ClipboardApi" = namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_api(ctx) {
        namespace ClipboardApi = namespace(ctx)?;
        fn writeText(ts_params = "text: string") = write_text;
        fn readText(ts_return = "Promise<ClipboardTextResult>") = read_text;
        fn write(ts_params = "item: ClipboardWriteItem") = write;
        fn read(
            ts_params = "options?: ClipboardReadOptions",
            ts_return = "Promise<ClipboardReadResult>"
        ) = read;
        fn clear() = clear;
        fn types(ts_return = "Promise<ClipboardTypesResult>") = types;
    }
}

#[cfg(test)]
mod tests {
    use super::{TEXT_MAX_BYTES, is_platform_file_reference, validate_text};

    #[test]
    fn text_limit_rejects_oversized_payload() {
        let oversized = "x".repeat(TEXT_MAX_BYTES + 1);
        assert!(validate_text(&oversized).is_err());
        assert!(validate_text("ok").is_ok());
        assert!(validate_text("").is_ok());
    }

    #[test]
    fn platform_file_references_match_share() {
        assert!(is_platform_file_reference("content://media/1"));
        assert!(is_platform_file_reference("file:///tmp/a.png"));
        assert!(is_platform_file_reference("datashare://media/1"));
        assert!(!is_platform_file_reference("lx://userdata/a.png"));
        assert!(!is_platform_file_reference("/tmp/a.png"));
    }
}
