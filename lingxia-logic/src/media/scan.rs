use crate::i18n::{
    js_error_from_business_code, js_error_from_platform_error, js_internal_error,
    js_invalid_parameter_error, js_timeout_error,
};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::traits::media_interaction::{MediaInteraction, ScanCodeRequest, ScanType};
use lxapp::{LxApp, lx};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, function::Optional};
use serde_json::Value;

#[derive(FromJSObj, Clone, Default)]
struct JSScanOptions {
    #[rename = "onlyFromCamera"]
    only_from_camera: Option<bool>,
    #[rename = "scanType"]
    scan_type: Option<Vec<String>>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct ScanResultObj {
    #[rename = "scanResult"]
    scan_result: String,
    #[rename = "scanType"]
    scan_type: String,
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let scan_func = JSFunc::new(ctx, scan)?;
    lx::register_js_api(ctx, "scanCode", scan_func)?;
    Ok(())
}

async fn scan(ctx: JSContext, options: Optional<JSScanOptions>) -> JSResult<ScanResultObj> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let opts = options.as_ref().cloned().unwrap_or_default();
    let scan_types = parse_scan_types(opts.scan_type)?;
    let only_from_camera = opts.only_from_camera.unwrap_or(true);

    let (callback_id, receiver) = get_callback();

    let request = ScanCodeRequest {
        scan_types,
        only_from_camera,
        callback_id,
    };

    lxapp
        .runtime
        .scan_code(request)
        .map_err(|e| js_error_from_platform_error(&e))?;

    let result = receiver
        .await
        .map_err(|_| js_timeout_error("scanCode callback timed out"))?;

    let data = match result {
        CallbackResult::Success(data) => data,
        CallbackResult::Error(code) => return Err(js_error_from_business_code(code)),
    };

    let payload: Value = serde_json::from_str(&data)
        .map_err(|e| js_internal_error(format!("scanCode invalid payload: {}", e)))?;

    let scan_result = payload
        .get("scanResult")
        .and_then(Value::as_str)
        .ok_or_else(|| js_internal_error("scanCode payload missing string `scanResult`"))?
        .to_string();

    let scan_type = payload
        .get("scanType")
        .and_then(Value::as_str)
        .ok_or_else(|| js_internal_error("scanCode payload missing string `scanType`"))?
        .to_string();

    Ok(ScanResultObj {
        scan_result,
        scan_type,
    })
}

fn parse_scan_types(value: Option<Vec<String>>) -> JSResult<Vec<ScanType>> {
    let mut out: Vec<ScanType> = Vec::new();
    if let Some(list) = value {
        for token in list {
            let t = parse_scan_type_token(token.as_str())
                .ok_or_else(|| js_invalid_parameter_error("invalid scanType token"))?;
            if !out.contains(&t) {
                out.push(t);
            }
        }
    }
    Ok(out)
}

fn parse_scan_type_token(value: &str) -> Option<ScanType> {
    match value {
        "barCode" => Some(ScanType::BarCode),
        "qrCode" => Some(ScanType::QrCode),
        "datamatrix" => Some(ScanType::DataMatrix),
        "pdf417" => Some(ScanType::Pdf417),
        _ => None,
    }
}
