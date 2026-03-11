use crate::I18nKey;
use lingxia_platform::error::PlatformError;
use lxapp::LxAppError;
use rong::{HostError, RongJSError};
use serde_json::Value;

/// Normalize locale string to use hyphens instead of underscores
///
/// Converts platform-specific locale formats:
/// - iOS: "zh_CN" -> "zh-CN"
/// - Android: "zh-rCN" -> "zh-CN"
/// - Standard: "zh-CN" -> "zh-CN"
#[inline]
fn normalize_locale(locale: &str) -> String {
    let normalized = locale.replace('_', "-").replace("-r", "-");
    let primary = normalized
        .split('-')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match primary.as_str() {
        "en" => "en-US".to_string(),
        "zh" => "zh-CN".to_string(),
        _ => {
            log::warn!("Unsupported locale `{}`; using en-US.", locale);
            "en-US".to_string()
        }
    }
}

/// Get localized string for a given key
///
/// Automatically retrieves locale from lxapp runtime.
/// This is the recommended way to use i18n in the logic layer.
///
/// # Arguments
/// * `key` - The i18n key to look up
///
/// # Returns
/// The localized string for the given key
///
/// # Example
/// ```ignore
/// let cancel_text = t(I18nKey::CommonCancel);
/// let confirm_text = t(I18nKey::CommonConfirm);
/// ```
#[inline]
pub fn t(key: I18nKey) -> String {
    let locale = lxapp::get_locale();
    let normalized = normalize_locale(&locale);
    key.get(&normalized).to_string()
}

pub fn err_code_message(code: u32) -> String {
    if let Some(key) = crate::i18n_generated::err_code_key(code) {
        return t(key);
    }

    log::error!("Unknown business error code {}.", code);
    format!("Unknown business error code: {}", code)
}

fn host_error_kind_for_code(code: u32) -> &'static str {
    if code == 1002 {
        return rong::error::E_INVALID_ARG;
    }
    if code == 1003 {
        return rong::error::E_NOT_FOUND;
    }
    if code == 12010 {
        return rong::error::E_NOT_FOUND;
    }
    if (2000..3000).contains(&code) {
        return rong::error::E_ABORT;
    }
    if (3000..4000).contains(&code) {
        return rong::error::E_PERMISSION_DENIED;
    }
    if (6000..7000).contains(&code) || code == 12005 {
        return rong::error::E_NOT_SUPPORTED;
    }
    if (4000..5000).contains(&code) || code == 12000 || code == 12009 {
        return rong::error::E_INVALID_STATE;
    }
    if code == 5002 || code == 12003 {
        return rong::error::E_TIMEOUT;
    }
    if (5000..6000).contains(&code) || (12001..12009).contains(&code) {
        return rong::error::E_NETWORK;
    }
    rong::error::E_INTERNAL
}

fn host_error_with_business_meta(
    host_code: &'static str,
    message: String,
    biz_code: u32,
    detail: Option<&str>,
) -> HostError {
    if let Some(detail) = detail.map(str::trim).filter(|value| !value.is_empty()) {
        return HostError::new(host_code, message)
            .with_data(rong::err_data!({ bizCode: (biz_code), detail: (detail) }));
    }

    HostError::new(host_code, message).with_data(rong::err_data!({ bizCode: (biz_code) }))
}

pub fn host_error_from_business_code(code: u32) -> HostError {
    host_error_with_business_meta(
        host_error_kind_for_code(code),
        err_code_message(code),
        code,
        None,
    )
}

pub fn host_error_from_business_code_with_detail(code: u32, detail: impl AsRef<str>) -> HostError {
    host_error_with_business_meta(
        host_error_kind_for_code(code),
        err_code_message(code),
        code,
        Some(detail.as_ref()),
    )
}

pub fn js_error_from_business_code(code: u32) -> RongJSError {
    host_error_from_business_code(code).into()
}

pub fn js_error_from_business_code_with_detail(code: u32, detail: impl AsRef<str>) -> RongJSError {
    host_error_from_business_code_with_detail(code, detail).into()
}

pub fn js_internal_error(detail: impl AsRef<str>) -> RongJSError {
    js_error_from_business_code_with_detail(1005, detail)
}

pub fn js_invalid_parameter_error(detail: impl AsRef<str>) -> RongJSError {
    js_error_from_business_code_with_detail(1002, detail)
}

pub fn js_resource_not_found_error(detail: impl AsRef<str>) -> RongJSError {
    js_error_from_business_code_with_detail(1003, detail)
}

pub fn js_service_unavailable_error(detail: impl AsRef<str>) -> RongJSError {
    js_error_from_business_code_with_detail(4000, detail)
}

pub fn js_timeout_error(detail: impl AsRef<str>) -> RongJSError {
    js_error_from_business_code_with_detail(5002, detail)
}

fn business_code_from_lxapp_error(error: &LxAppError) -> u32 {
    fn code_from_value(value: &Value) -> Option<u32> {
        if let Some(code) = value.as_u64() {
            return u32::try_from(code).ok();
        }
        if let Some(text) = value.as_str()
            && let Ok(parsed) = text.parse::<u32>()
        {
            return Some(parsed);
        }
        None
    }

    fn biz_code_from_data(data: &Option<Value>) -> Option<u32> {
        let obj = data.as_ref()?.as_object()?;
        obj.get("bizCode")
            .and_then(code_from_value)
            .or_else(|| obj.get("code").and_then(code_from_value))
    }

    match error {
        LxAppError::InvalidParameter(_) => 1002,
        LxAppError::ResourceNotFound(_) | LxAppError::PluginNotConfigured(_) => 1003,
        LxAppError::UnsupportedOperation(_) => 6000,
        LxAppError::IoError(_) => 1001,
        LxAppError::RongJSHost { code, data, .. } => code
            .parse::<u32>()
            .ok()
            .or_else(|| biz_code_from_data(data))
            .unwrap_or(1005),
        LxAppError::WebView(_)
        | LxAppError::InvalidJsonFile(_)
        | LxAppError::Runtime(_)
        | LxAppError::ChannelError(_)
        | LxAppError::ResourceExhausted(_)
        | LxAppError::Bridge(_)
        | LxAppError::RongJS(_)
        | LxAppError::PluginDownloadFailed(_) => 1005,
    }
}

fn detail_suffix(error: &LxAppError) -> Option<&str> {
    match error {
        LxAppError::ResourceNotFound(detail)
        | LxAppError::InvalidJsonFile(detail)
        | LxAppError::InvalidParameter(detail)
        | LxAppError::UnsupportedOperation(detail)
        | LxAppError::IoError(detail)
        | LxAppError::Runtime(detail)
        | LxAppError::ChannelError(detail)
        | LxAppError::ResourceExhausted(detail)
        | LxAppError::Bridge(detail)
        | LxAppError::RongJS(detail)
        | LxAppError::PluginNotConfigured(detail)
        | LxAppError::PluginDownloadFailed(detail)
        | LxAppError::WebView(detail) => Some(detail.as_str()),
        LxAppError::RongJSHost { message, .. } => Some(message.as_str()),
    }
}

pub fn host_error_from_lxapp_error(error: &LxAppError) -> HostError {
    let code = business_code_from_lxapp_error(error);

    log::warn!("Mapped LxAppError to business code {}: {}", code, error);
    host_error_with_business_meta(
        host_error_kind_for_code(code),
        err_code_message(code),
        code,
        detail_suffix(error),
    )
}

pub fn js_error_from_lxapp_error(error: &LxAppError) -> RongJSError {
    host_error_from_lxapp_error(error).into()
}

fn business_code_from_platform_error(error: &PlatformError) -> u32 {
    match error {
        PlatformError::NotSupported(_) => 6000,
        PlatformError::InvalidParameter(_) => 1002,
        PlatformError::AssetNotFound(_) => 1003,
        PlatformError::Platform(_) => 1005,
        PlatformError::BusinessError(code) => *code,
        PlatformError::CallbackDropped => 1006,
    }
}

fn detail_from_platform_error(error: &PlatformError) -> &str {
    match error {
        PlatformError::Platform(detail)
        | PlatformError::NotSupported(detail)
        | PlatformError::AssetNotFound(detail)
        | PlatformError::InvalidParameter(detail) => detail,
        PlatformError::BusinessError(_) => "business error",
        PlatformError::CallbackDropped => "callback dropped",
    }
}

pub fn host_error_from_platform_error(error: &PlatformError) -> HostError {
    let code = business_code_from_platform_error(error);
    host_error_from_business_code_with_detail(code, detail_from_platform_error(error))
}

pub fn js_error_from_platform_error(error: &PlatformError) -> RongJSError {
    host_error_from_platform_error(error).into()
}
