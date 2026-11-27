use crate::I18nKey;
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

/// A numeric code's English text, the message when the caller gave none.
pub fn business_message(code: u32) -> String {
    match crate::i18n_generated::err_code_key(code) {
        Some(key) => key.get("en").to_string(),
        None => format!("error {code}"),
    }
}

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
    ("directoryNotEmpty", rong::error::E_INVALID_STATE),
];

/// Whether a reason is one `LxErrorReason` lists.
fn known_reason(reason: &str) -> bool {
    STATE_REASONS.iter().any(|(known, _)| *known == reason)
        || lingxia_error::BUSINESS_CODES
            .iter()
            .any(|(_, _, known)| *known == Some(reason))
}

/// An error as Logic receives it: its code, English message, reason and data.
pub fn host_error(error: &lingxia_error::Error) -> HostError {
    let mut host = HostError::new(error.static_code(), error.message().to_string());
    if let Some(reason) = error.reason() {
        if !known_reason(reason) {
            log::error!("reason '{reason}' is not in LxErrorReason");
        }
        host = host.with_reason(reason.to_string());
    }
    match error.data() {
        Some(data) => host.with_data(error_data_from_json(data)),
        None => host,
    }
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
    host_error(&lingxia_error::Error::business(
        code,
        business_message(code),
    ))
}

/// `detail` is the message; an empty one reads as the code's own text.
pub fn host_error_from_business_code_with_detail(code: u32, detail: impl AsRef<str>) -> HostError {
    let detail = detail.as_ref().trim();
    let message = if detail.is_empty() {
        business_message(code)
    } else {
        detail.to_string()
    };
    host_error(&lingxia_error::Error::business(code, message))
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

pub fn host_error_of(error: impl Into<lingxia_error::Error>) -> HostError {
    host_error(&error.into())
}

pub fn js_error(error: impl Into<lingxia_error::Error>) -> RongJSError {
    host_error_of(error).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table, the generated `LxErrorReason` and the catalogue's codes are one list.
    #[test]
    fn the_reason_table_matches_the_union_and_the_error_codes() {
        let union = lingxia_error::BUSINESS_CODES
            .iter()
            .filter_map(|(_, _, reason)| *reason)
            .chain(STATE_REASONS.iter().map(|(reason, _)| *reason))
            .map(|reason| format!("'{reason}'"))
            .collect::<Vec<_>>()
            .join(" | ");
        let declared = format!("type LxErrorReason = r###\"{union}\"###;");
        assert!(
            include_str!("public_types.rs").contains(&declared),
            "LxErrorReason in public_types.rs must read: {declared}"
        );
        let listed: String = lingxia_error::BUSINESS_CODES
            .iter()
            .filter_map(|(_, kind, reason)| reason.map(|reason| (reason, *kind)))
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
        let table: Vec<u32> = lingxia_error::BUSINESS_CODES
            .iter()
            .map(|(code, _, _)| *code)
            .collect();
        assert_eq!(
            table, codes,
            "every i18n error code needs a BUSINESS_CODES row"
        );
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
        let known: Vec<&str> = lingxia_error::BUSINESS_CODES
            .iter()
            .filter_map(|(_, _, reason)| *reason)
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
            let parent = path.parent().unwrap();
            if !(name == "i18n.rs" && parent.ends_with("lingxia-logic/src"))
                && !(name == "lib.rs" && parent.ends_with("lingxia-error/src"))
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
        for (code, _, reason) in lingxia_error::BUSINESS_CODES {
            if let Some(reason) = reason {
                assert!(
                    mentioned(*code),
                    "'{reason}' ({code}) is in the table but nothing raises it"
                );
            }
        }
    }

    #[test]
    fn a_business_code_becomes_reason_and_an_english_message() {
        let error = host_error_from_business_code_with_detail(3002, "  denied in settings ");
        assert_eq!(error.code, "E_PERMISSION_DENIED");
        assert_eq!(error.reason(), Some("locationPermissionDenied"));
        assert_eq!(error.message, "denied in settings");
        assert_eq!(error.detail(), None);
        assert!(error.data.is_none());
        assert_eq!(host_error_from_business_code(1002).reason(), None);
    }

    #[test]
    fn lxapp_host_errors_keep_structured_navigation_metadata() {
        let error = lingxia_error::Error::invalid_arg("duplicate route")
            .with_reason("tabAlreadyOpen")
            .with_data(serde_json::json!({ "operation": "navigateTo", "target": "pages/home" }));

        let host = host_error_of(error);
        let Some(ErrorData::Object(data)) = host.data.clone() else {
            panic!("expected structured error metadata");
        };
        assert!(!data.contains_key("reason"));
        assert_eq!(
            data.get("operation").and_then(ErrorData::as_str),
            Some("navigateTo")
        );
        assert!(!data.contains_key("detail"));
        assert_eq!(host.message, "duplicate route");
        assert_eq!(host.detail(), None);
        assert_eq!(host.reason(), Some("tabAlreadyOpen"));
    }
}
