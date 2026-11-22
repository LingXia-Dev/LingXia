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
    (1004, None),
    (1005, None),
    (2000, Some("userCanceled")),
    (2001, Some("userDeclined")),
    (3000, None),
    (3001, Some("cameraPermissionDenied")),
    (3002, Some("locationPermissionDenied")),
    (3003, Some("microphonePermissionDenied")),
    (3004, Some("photosPermissionDenied")),
    (3005, Some("storagePermissionDenied")),
    (3006, None),
    (3007, None),
    (3008, Some("clipboardPermissionDenied")),
    (4000, None),
    (4001, Some("locationServicesDisabled")),
    (4002, None),
    (4003, None),
    (4004, Some("refreshNotEnabled")),
    (5000, None),
    (5001, Some("networkUnavailable")),
    (5002, None),
    (5003, Some("serverErrorStatus")),
    (5004, Some("connectionFailed")),
    (6000, None),
    (6001, Some("deviceUnsupported")),
    (6002, Some("hostUpgradeRequired")),
    (12000, None),
    (12001, Some("wifiSystemError")),
    (12002, Some("wifiPasswordWrong")),
    (12003, Some("wifiTimedOut")),
    (12004, None),
    (12005, Some("wifiUnsupported")),
    (12006, Some("wifiPermissionDenied")),
    (12007, Some("wifiPromptUnanswered")),
    (12008, Some("wifiConnectionDeclined")),
    (12009, Some("wifiDisabled")),
    (12010, Some("wifiNetworkNotFound")),
];

/// Reasons raised without a business code, with the code they come with. `LxErrorReason` lists
/// these after the business-code rows.
pub(crate) const STATE_REASONS: &[(&str, &str)] = &[
    ("placementUnsupported", rong::error::E_NOT_SUPPORTED),
    ("appOpenInOtherRole", "E_SURFACE_CONFLICT"),
    ("appUnderMaintenance", rong::error::E_PERMISSION_DENIED),
    ("appSuspended", rong::error::E_PERMISSION_DENIED),
    ("tabAlreadyOpen", rong::error::E_INVALID_ARG),
    ("stackFull", rong::error::E_INVALID_ARG),
    ("revisionOutdated", rong::error::E_INVALID_STATE),
    ("appClosed", rong::error::E_INVALID_STATE),
    ("pageUnloaded", rong::error::E_INVALID_STATE),
    ("tabBarNotDeclared", rong::error::E_INVALID_STATE),
];

/// The table's spelling of a reason another crate raised in `data.reason`.
fn state_reason(reason: &str) -> Option<&'static str> {
    STATE_REASONS
        .iter()
        .find(|(known, _)| *known == reason)
        .map(|(known, _)| *known)
}

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

/// A call the app's or page's state refuses, with the reason that says which state.
pub fn js_state_error(reason: &'static str, detail: impl AsRef<str>) -> RongJSError {
    host_error_from_business_code_with_detail(4000, detail)
        .with_reason(reason)
        .into()
}

/// The server answered a transfer with an error status, carried in `data.status`.
pub fn server_error_status(status: Option<u16>, detail: impl AsRef<str>) -> RongJSError {
    let error = host_error_from_business_code_with_detail(5003, detail);
    match status {
        Some(status) => error.with_data(rong::err_data!({ status: (status) })),
        None => error,
    }
    .into()
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
        let mut reason = None;
        let detail = data.as_object_mut().and_then(|fields| {
            fields.remove("bizCode");
            // A sub-reason is top-level on a Logic error, never in `data`.
            if let Some(Value::String(raised)) = fields.remove("reason") {
                reason = state_reason(&raised);
                if reason.is_none() {
                    log::error!("reason '{raised}' is not in STATE_REASONS");
                }
            }
            match fields.remove("detail") {
                Some(Value::String(detail)) => Some(detail),
                _ => None,
            }
        });
        let mut error = host_error_with_business_meta(
            host_error_kind_for_code(code),
            err_code_message(code),
            code,
            detail.as_deref(),
        );
        if let Some(reason) = reason {
            error = error.with_reason(reason);
        }
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
            .filter_map(|(_, reason)| *reason)
            .chain(STATE_REASONS.iter().map(|(reason, _)| *reason))
            .map(|reason| format!("'{reason}'"))
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
                reason.map(|reason| (reason, host_error_kind_for_code(*code)))
            })
            .chain(STATE_REASONS.iter().copied())
            .map(|(reason, kind)| format!("        /// - `{reason}` with `{kind}`\n"))
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

    /// Source files under `dir` with one of `exts`, test modules cut off.
    fn sources(dir: &std::path::Path, exts: &[&str]) -> Vec<(std::path::PathBuf, String)> {
        let mut found = Vec::new();
        let mut dirs = vec![dir.to_path_buf()];
        while let Some(dir) = dirs.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for path in entries.map(|entry| entry.unwrap().path()) {
                let name = path.file_name().unwrap().to_string_lossy();
                if path.is_dir() {
                    if !matches!(
                        &*name,
                        "tests" | "target" | "build" | "node_modules" | ".build"
                    ) {
                        dirs.push(path);
                    }
                } else if exts.iter().any(|ext| name.ends_with(ext)) {
                    let text = std::fs::read_to_string(&path).unwrap_or_default();
                    let code = text
                        .split("#[cfg(test)]\nmod tests")
                        .next()
                        .unwrap()
                        .to_string();
                    found.push((path, code));
                }
            }
        }
        found
    }

    fn quoted_after(text: &str, marker: &str) -> Vec<String> {
        text.match_indices(marker)
            .filter_map(|(at, _)| {
                let rest = text[at + marker.len()..].trim_start();
                let rest = rest.strip_prefix('"')?;
                Some(rest[..rest.find('"')?].to_string())
            })
            .collect()
    }

    /// The reasons raised by name: here, and the ones `lxapp` puts in its error data.
    fn raised_by_name() -> Vec<String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut raised = Vec::new();
        for (path, code) in sources(&root.join("src"), &[".rs"]) {
            if path.ends_with("i18n.rs") {
                continue;
            }
            raised.extend(quoted_after(&code, ".with_reason("));
            raised.extend(quoted_after(&code, "js_state_error("));
        }
        let surface = std::fs::read_to_string(root.join("src/surface.rs")).unwrap();
        let body = &surface[surface.find("const fn reason(self)").unwrap()..];
        raised.extend(quoted_after(
            &body[..body.find("\n    }").unwrap()],
            "Some(",
        ));
        let lxapp = root.join("../lingxia-lxapp/src/lxapp");
        let navigation = std::fs::read_to_string(lxapp.join("page_instance_host.rs")).unwrap();
        raised.extend(quoted_after(&navigation, "navigation_entry_error("));
        let registry = std::fs::read_to_string(lxapp.join("registry.rs")).unwrap();
        let body = &registry[registry.find("fn unavailable_reason").unwrap()..];
        raised.extend(quoted_after(&body[..body.find("\n}").unwrap()], "=>"));
        raised
    }

    /// A reason raised by name, here or in `lxapp`, is one the table lists.
    #[test]
    fn every_raised_reason_is_in_the_table() {
        let known: Vec<&str> = REASONS
            .iter()
            .filter_map(|(_, reason)| *reason)
            .chain(STATE_REASONS.iter().map(|(reason, _)| *reason))
            .collect();
        let raised = raised_by_name();
        assert!(raised.len() >= 10, "found the raised reasons: {raised:?}");
        for reason in raised {
            assert!(
                known.contains(&reason.as_str()),
                "'{reason}' is raised but not in the table"
            );
        }
    }

    /// Every reason has a producer. A state reason is raised by name; a business code is
    /// written somewhere in the Rust crates or a host SDK (Kotlin, Java, Swift, ArkTS) outside
    /// this table. That is a mention, not proof of a raise: hosts pass codes through callbacks
    /// in too many shapes to parse.
    #[test]
    fn every_reason_has_a_producer() {
        let raised = raised_by_name();
        for (reason, _) in STATE_REASONS {
            assert!(
                raised.iter().any(|name| name == reason),
                "'{reason}' is in the table but nothing raises it"
            );
        }
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut texts: Vec<String> = Vec::new();
        for (path, code) in sources(&repo.join("crates"), &[".rs"]) {
            let name = path.file_name().unwrap().to_string_lossy();
            if !(name == "i18n.rs" && path.parent().unwrap().ends_with("lingxia-logic/src"))
                && name != "i18n_generated.rs"
            {
                texts.push(code);
            }
        }
        let sdk = sources(
            &repo.join("lingxia-sdk"),
            &[".kt", ".java", ".swift", ".ets"],
        );
        assert!(sdk.len() > 20, "the host SDKs were not found");
        texts.extend(sdk.into_iter().map(|(_, code)| code));
        let mentioned = |code: u32| {
            let digits = code.to_string();
            texts.iter().any(|text| {
                text.match_indices(&digits).any(|(at, _)| {
                    let before = text[..at].chars().next_back();
                    let after = text[at + digits.len()..].chars().next();
                    !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '.')
                        && !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '.')
                })
            })
        };
        for (code, reason) in REASONS {
            if let Some(reason) = reason {
                assert!(
                    mentioned(*code),
                    "'{reason}' ({code}) is in the table but nothing raises it"
                );
            }
        }
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
                "reason": "tabAlreadyOpen",
                "operation": "navigateTo",
                "target": "pages/home",
            })),
        };

        let host = host_error_from_lxapp_error(&error);
        let Some(ErrorData::Object(data)) = host.data.clone() else {
            panic!("expected structured error metadata");
        };
        assert!(!data.contains_key("reason"));
        assert_eq!(
            data.get("operation").and_then(ErrorData::as_str),
            Some("navigateTo")
        );
        assert!(!data.contains_key("bizCode") && !data.contains_key("detail"));
        assert_eq!(host.detail(), Some("duplicate route"));
        assert_eq!(host.reason(), Some("tabAlreadyOpen"));
    }
}
