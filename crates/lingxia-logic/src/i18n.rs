use crate::I18nKey;
use lingxia_platform::error::{PRESENTER_UNAVAILABLE_CODE, PlatformError};
use lxapp::LxAppError;
use rong::{HostError, RongJSError, error::ErrorData};
use serde_json::Value;
use std::collections::BTreeMap;

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
    // The host has narrowed it to a product language; one the SDK lacks reads in English.
    let language = lxapp::display_language();
    if let Some(text) = lingxia_app_context::app_config()
        .and_then(|config| config.sdk_text.get(&language))
        .and_then(|words| words.get(key.name()))
    {
        return text.clone();
    }
    key.get(&language).to_string()
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

/// The `reason` a Logic error carries for a business code: `None` where `code` already says it all.
/// Every code in `i18n/error` has a row; `LxErrorReason` in `public_types.rs` is the same list.
pub(crate) const REASONS: &[(u32, Option<&str>)] = &[
    (1000, None),
    (1001, None),
    (1002, None),
    (1003, None),
    (1004, Some("initializationFailed")),
    (1005, None),
    (2000, Some("userCancelled")),
    (2001, Some("userDeclined")),
    (3000, None),
    (3001, Some("cameraPermissionDenied")),
    (3002, Some("locationPermissionDenied")),
    (3003, Some("microphonePermissionDenied")),
    (3004, Some("photosPermissionDenied")),
    (3005, Some("storagePermissionDenied")),
    (3006, Some("notificationsPermissionDenied")),
    (3007, Some("bluetoothPermissionDenied")),
    (3008, Some("clipboardPermissionDenied")),
    (4000, Some("serviceUnavailable")),
    (4001, Some("locationServicesDisabled")),
    (4002, Some("bluetoothDisabled")),
    (4003, Some("nfcDisabled")),
    (4004, Some("featureDisabled")),
    (5000, None),
    (5001, Some("networkUnavailable")),
    (5002, None),
    (5003, Some("serverFailed")),
    (5004, Some("connectionFailed")),
    (6000, None),
    (6001, Some("deviceUnsupported")),
    (6002, Some("hostUpgradeRequired")),
    (12000, Some("wifiUninitialized")),
    (12001, Some("wifiFailed")),
    (12002, Some("wifiPasswordWrong")),
    (12003, Some("wifiTimedOut")),
    (12004, Some("wifiRequestDuplicated")),
    (12005, Some("wifiUnsupported")),
    (12006, Some("wifiPermissionDenied")),
    (12007, Some("wifiPromptUnanswered")),
    (12008, Some("wifiConnectionDeclined")),
    (12009, Some("wifiDisabled")),
    (12010, Some("wifiNetworkNotFound")),
];

fn reason_for_code(code: u32) -> Option<&'static str> {
    REASONS
        .iter()
        .find(|(known, _)| *known == code)
        .and_then(|(_, reason)| *reason)
}

fn host_error_with_business_meta(
    host_code: &'static str,
    message: String,
    biz_code: u32,
    detail: Option<&str>,
) -> HostError {
    let mut error = HostError::new(host_code, message);
    if let Some(reason) = reason_for_code(biz_code) {
        error = error.with_reason(reason);
    }
    if let Some(detail) = detail.map(str::trim).filter(|value| !value.is_empty()) {
        error = error.with_detail(detail);
    }
    error
}

fn error_data_from_json(value: &Value) -> ErrorData {
    match value {
        Value::Null => ErrorData::Null,
        Value::Bool(value) => ErrorData::Bool(*value),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                ErrorData::from(value)
            } else if let Some(value) = value.as_u64() {
                ErrorData::from(value)
            } else {
                ErrorData::from(value.as_f64().unwrap_or_default())
            }
        }
        Value::String(value) => ErrorData::String(value.clone()),
        Value::Array(values) => ErrorData::Array(values.iter().map(error_data_from_json).collect()),
        Value::Object(values) => ErrorData::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), error_data_from_json(value)))
                .collect::<BTreeMap<_, _>>(),
        ),
    }
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
        LxAppError::ResourceExhausted(_) => 1002,
        LxAppError::ResourceNotFound(_) | LxAppError::PluginNotConfigured(_) => 1003,
        LxAppError::UnsupportedOperation(_) | LxAppError::SurfaceRoleLocked(_) => 6000,
        LxAppError::IoError(_) => 1001,
        LxAppError::RongJSHost { code, data, .. } => code
            .parse::<u32>()
            .ok()
            .or_else(|| biz_code_from_data(data))
            .unwrap_or(1005),
        LxAppError::WebView(_)
        | LxAppError::InvalidJsonFile(_)
        | LxAppError::Runtime(_)
        | LxAppError::SurfaceConflict(_)
        | LxAppError::ChannelError(_)
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
        | LxAppError::SurfaceConflict(detail)
        | LxAppError::SurfaceRoleLocked(detail)
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
    if let LxAppError::RongJSHost {
        data: Some(data), ..
    } = error
    {
        // `bizCode` is the carrier a View reply reads; a Logic error says it as `reason`.
        let mut data = data.clone();
        let detail = data.as_object_mut().and_then(|fields| {
            fields.remove("bizCode");
            match fields.remove("detail") {
                Some(Value::String(detail)) => Some(detail),
                _ => None,
            }
        });
        let error = host_error_with_business_meta(
            host_error_kind_for_code(code),
            err_code_message(code),
            code,
            detail.as_deref(),
        );
        return match data.as_object() {
            Some(fields) if fields.is_empty() => error,
            _ => error.with_data(error_data_from_json(&data)),
        };
    }
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
        PlatformError::PresenterUnavailable => PRESENTER_UNAVAILABLE_CODE,
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
        PlatformError::PresenterUnavailable => "page chrome presenter not mounted",
    }
}

pub fn host_error_from_platform_error(error: &PlatformError) -> HostError {
    let code = business_code_from_platform_error(error);
    host_error_from_business_code_with_detail(code, detail_from_platform_error(error))
}

pub fn js_error_from_platform_error(error: &PlatformError) -> RongJSError {
    host_error_from_platform_error(error).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table, the generated `LxErrorReason` and the catalogue's codes are one list.
    #[test]
    fn the_reason_table_matches_the_union_and_the_error_codes() {
        let union = REASONS
            .iter()
            .filter_map(|(_, reason)| reason.map(|reason| format!("'{reason}'")))
            .collect::<Vec<_>>()
            .join(" | ");
        let declared = format!("type LxErrorReason = r###\"{union}\"###;");
        assert!(
            include_str!("public_types.rs").contains(&declared),
            "LxErrorReason in public_types.rs must read: {declared}"
        );
        let listed: String = REASONS
            .iter()
            .filter_map(|(code, reason)| {
                let kind = host_error_kind_for_code(*code);
                reason.map(|reason| format!("        /// - `{reason}` with `{kind}`\n"))
            })
            .collect();
        assert!(
            include_str!("public_types.rs").contains(&listed),
            "LxErrorReason's doc must list each reason with its code:\n{listed}"
        );
        let catalogue = include_str!("../../../i18n/error/en-US.yaml");
        let codes: Vec<u32> = catalogue
            .lines()
            .filter_map(|line| line.trim().split_once(':')?.0.parse().ok())
            .collect();
        let table: Vec<u32> = REASONS.iter().map(|(code, _)| *code).collect();
        assert_eq!(table, codes, "every i18n error code needs a REASONS row");
    }

    #[test]
    fn a_business_code_becomes_reason_and_detail() {
        let error = host_error_from_business_code_with_detail(3002, "  denied in settings ");
        assert_eq!(error.reason(), Some("locationPermissionDenied"));
        assert_eq!(error.detail(), Some("denied in settings"));
        assert!(error.data.is_none());
        assert_eq!(host_error_from_business_code(1002).reason(), None);
    }

    #[test]
    fn lxapp_host_errors_keep_structured_navigation_metadata() {
        let error = LxAppError::RongJSHost {
            code: "E_INVALID_ARG".to_string(),
            message: "duplicate route".to_string(),
            data: Some(serde_json::json!({
                "bizCode": 1002,
                "detail": "duplicate route",
                "reason": "duplicate_route",
                "operation": "navigateTo",
                "target": "pages/home",
            })),
        };

        let host = host_error_from_lxapp_error(&error);
        let Some(ErrorData::Object(data)) = host.data.clone() else {
            panic!("expected structured error metadata");
        };
        assert_eq!(
            data.get("reason").and_then(ErrorData::as_str),
            Some("duplicate_route")
        );
        assert_eq!(
            data.get("operation").and_then(ErrorData::as_str),
            Some("navigateTo")
        );
        assert!(!data.contains_key("bizCode") && !data.contains_key("detail"));
        assert_eq!(host.detail(), Some("duplicate route"));
        assert_eq!(host.reason(), None);
    }
}
