use crate::dismissal::{USER_DISMISSED, canceled, completed};
use crate::i18n::{js_error_from_platform_error, js_internal_error, js_invalid_parameter_error};
use lingxia_platform::error::PlatformError;
use lingxia_service::media::{ScanCodeRequest, ScanType};
use lxapp::LxApp;
use rong::{FromJSObject, JSContext, JSObject, JSResult, function::Optional};
use serde_json::Value;

#[derive(FromJSObject, Clone, Default)]
#[ts_skip]
struct JSScanOptions {
    #[js_name = "onlyFromCamera"]
    only_from_camera: Option<bool>,
    formats: Option<Vec<String>>,
    signal: Option<JSObject>,
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace PickApi = crate::pick::pick_namespace(ctx)?;
        fn code(
            ts_params = "options?: PickCodeOptions",
            ts_return = "Promise<PickCodeResult>"
        ) = scan;
    }
}

/// Scans a barcode or QR code with the camera.
///
/// Resolves `{ status: 'canceled' }` only when the user dismisses the scanner. A
/// completed scan resolves `{ status: 'ok', value, format }`; nothing is opened,
/// so pass an app link to `lx.nav.go` yourself. Rejects when scanning fails or the host returns an invalid payload.
///
/// @throws {E_INVALID_ARG} An unknown entry in `formats`.
/// @throws {E_ABORT} `signal` aborted.
/// @throws {E_INTERNAL} The scanner failed or returned an invalid payload.
async fn scan(ctx: JSContext, options: Optional<JSScanOptions>) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let opts = options.as_ref().cloned().unwrap_or_default();
    let scan_types = parse_scan_types(opts.formats)?;
    let only_from_camera = opts.only_from_camera.unwrap_or(true);

    let request = ScanCodeRequest {
        scan_types,
        only_from_camera,
    };

    let app = lxapp.clone();
    let work = async move { Ok(lingxia_service::media::scan_code(&*app.runtime, request).await) };
    let scanned = crate::pick::abortable(&ctx, opts.signal.clone(), work, |_| {}).await?;
    let data = match scanned {
        Ok(data) => data,
        // Business code 2000 is the platform's "user dismissed the scanner";
        // every other platform error stays a rejection.
        Err(PlatformError::BusinessError(USER_DISMISSED)) => return canceled(&ctx),
        Err(err) => return Err(js_error_from_platform_error(&err)),
    };

    let payload: Value = serde_json::from_str(&data)
        .map_err(|e| js_internal_error(format!("pick.code invalid payload: {}", e)))?;

    let scan_result = payload
        .get("scanResult")
        .and_then(Value::as_str)
        .ok_or_else(|| js_internal_error("pick.code payload missing string `scanResult`"))?
        .to_string();

    let scan_type = payload
        .get("scanType")
        .and_then(Value::as_str)
        .ok_or_else(|| js_internal_error("pick.code payload missing string `scanType`"))?
        .to_string();

    let result = completed(&ctx)?;
    result.set("value", scan_result)?;
    result.set("format", scan_type)?;
    Ok(result)
}

fn parse_scan_types(value: Option<Vec<String>>) -> JSResult<Vec<ScanType>> {
    let mut out: Vec<ScanType> = Vec::new();
    if let Some(list) = value {
        for token in list {
            let t = parse_scan_type_token(token.as_str())
                .ok_or_else(|| js_invalid_parameter_error("invalid code format"))?;
            if !out.contains(&t) {
                out.push(t);
            }
        }
    }
    Ok(out)
}

fn parse_scan_type_token(value: &str) -> Option<ScanType> {
    match value {
        "bar_code" => Some(ScanType::BarCode),
        "qr_code" => Some(ScanType::QrCode),
        "datamatrix" => Some(ScanType::DataMatrix),
        "pdf417" => Some(ScanType::Pdf417),
        _ => None,
    }
}
