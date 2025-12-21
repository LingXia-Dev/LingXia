use lingxia_messaging::{CallbackResult, get_callback};
use crate::i18n::err_code_message;
use lingxia_platform::{
    MediaInteraction, ScanCodeRequest, ScanType, ToastIcon, ToastOptions, ToastPosition,
    UserFeedback,
};
use lxapp::{LxApp, lx};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError, function::Optional};
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
        .map_err(|e| RongJSError::Error(format!("scan failed to start: {}", e)))?;

    let result = receiver
        .await
        .map_err(|_| RongJSError::Error("scan cancelled or failed".to_string()))?;

    let data = match result {
        CallbackResult::Success(data) => data,
        CallbackResult::Error(code) => {
            // 2000 = user cancelled, return empty result
            if code == 2000 {
                return Ok(ScanResultObj {
                    scan_result: String::new(),
                    scan_type: String::new(),
                });
            }

            let message = err_code_message(code)
                .unwrap_or_else(|| format!("Scan error: {}", code));
            let _ = lxapp.runtime.show_toast(ToastOptions {
                title: message.clone(),
                icon: ToastIcon::Error,
                image: None,
                duration: 2.0,
                mask: false,
                position: ToastPosition::Center,
            });
            return Err(RongJSError::Error(message));
        }
    };

    let payload: Value = serde_json::from_str(&data).unwrap_or(Value::Null);

    let scan_result = payload
        .get("scanResult")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_default();

    let scan_type = payload
        .get("scanType")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_default();

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
                .ok_or_else(|| RongJSError::Error("invalid scanType token".to_string()))?;
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
