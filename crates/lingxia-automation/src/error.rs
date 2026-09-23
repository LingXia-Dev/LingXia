//! Stable JS error codes for automation driver failures.
//!
//! The shared lower half (`lxapp::automation`) reports failures as plain
//! strings, and older clients parse those strings, so the messages never
//! change. Each failure a test can act on also gets its own `code`, and a page
//! failure carries `data` naming the page it concerned. Anything else keeps the
//! `E_AUTOMATION` fallback.

use lxapp::{LxApp, automation as auto};
use rong::{HostError, RongJSError, error::ErrorData};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Fallback for a failure with no more specific code.
pub(crate) const E_AUTOMATION: &str = "E_AUTOMATION";
/// The calling context lacks the automation privilege the call needs.
pub(crate) const E_AUTOMATION_PRIVILEGE: &str = "E_AUTOMATION_PRIVILEGE";
/// The target page is not the active instance (not open, or replaced).
pub(crate) const E_PAGE_NOT_ACTIVE: &str = "E_PAGE_NOT_ACTIVE";
/// The page exists but has no WebView/current page to act on yet.
pub(crate) const E_PAGE_NOT_READY: &str = "E_PAGE_NOT_READY";
/// No element matched the selector at dispatch.
pub(crate) const E_ELEMENT_NOT_FOUND: &str = "E_ELEMENT_NOT_FOUND";
/// The element matched but cannot take the input (disabled, not editable, ...).
pub(crate) const E_ELEMENT_NOT_INTERACTABLE: &str = "E_ELEMENT_NOT_INTERACTABLE";
/// A driver wait ran out of time.
pub(crate) const E_AUTOMATION_TIMEOUT: &str = "E_AUTOMATION_TIMEOUT";
/// The evaluated script threw.
pub(crate) const E_EVAL_SCRIPT: &str = "E_EVAL_SCRIPT";
/// The evaluation did not settle within its timeout.
pub(crate) const E_EVAL_TIMEOUT: &str = "E_EVAL_TIMEOUT";

/// The code for a driver or lower-half failure message.
pub(crate) fn code_for(message: &str) -> &'static str {
    if message.contains("_privilege_required:") {
        E_AUTOMATION_PRIVILEGE
    } else if message.starts_with("page is not active:")
        || (message.starts_with("page instance ")
            && message.contains("was disposed before runtime became ready"))
    {
        E_PAGE_NOT_ACTIVE
    } else if message == "page WebView is not ready"
        || message.to_ascii_lowercase().contains("no current page")
    {
        E_PAGE_NOT_READY
    } else if message.starts_with("Element not found:") {
        E_ELEMENT_NOT_FOUND
    } else if message.starts_with("Element not interactable:") {
        E_ELEMENT_NOT_INTERACTABLE
    } else if message.starts_with("E_TIMEOUT:")
        || (message.starts_with("timed out after ") && message.contains(" waiting for "))
    {
        E_AUTOMATION_TIMEOUT
    } else {
        E_AUTOMATION
    }
}

/// [`code_for`] for an evaluation: a script that threw and a WebView that
/// gave up waiting get their own codes.
pub(crate) fn eval_code_for(message: &str) -> &'static str {
    if message.starts_with("JavaScript error:") {
        E_EVAL_SCRIPT
    } else if message == "JavaScript evaluation timed out" || message.ends_with("eval timed out") {
        E_EVAL_TIMEOUT
    } else {
        code_for(message)
    }
}

/// A coded automation error.
pub(crate) fn coded(code: &'static str, message: impl Into<String>) -> HostError {
    HostError::new(code, message)
}

/// Attach JSON diagnostic data to an error.
pub(crate) fn with_json_data(error: HostError, data: &Value) -> HostError {
    error.with_data(error_data(data))
}

fn error_data(value: &Value) -> ErrorData {
    match value {
        Value::Null => ErrorData::Null,
        Value::Bool(value) => ErrorData::Bool(*value),
        Value::Number(value) => match (value.as_i64(), value.as_u64()) {
            (Some(value), _) => ErrorData::from(value),
            (None, Some(value)) => ErrorData::from(value),
            _ => ErrorData::from(value.as_f64().unwrap_or_default()),
        },
        Value::String(value) => ErrorData::String(value.clone()),
        Value::Array(values) => ErrorData::Array(values.iter().map(error_data).collect()),
        Value::Object(values) => ErrorData::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), error_data(value)))
                .collect::<BTreeMap<_, _>>(),
        ),
    }
}

/// One page instance as error data names it.
pub(crate) struct PageRef {
    pub name: Option<String>,
    pub path: String,
    pub instance_id: String,
}

impl PageRef {
    fn resolve(app: &Arc<LxApp>, page: Option<&str>) -> Option<Self> {
        let (page, name) = auto::resolve_page(app, page).ok()?;
        Some(Self {
            name,
            path: page.path(),
            instance_id: page.instance_id_string(),
        })
    }
}

/// `{ page, instanceId?, current? }`: the page the call targeted (the
/// resolved instance when it still resolves, else what the caller asked
/// for) and the page that was current when it failed.
pub(crate) fn page_data(
    requested: Option<&str>,
    target: Option<&PageRef>,
    current: Option<&PageRef>,
) -> Value {
    let mut data = Map::new();
    let page = target
        .map(|target| target.name.clone().unwrap_or_else(|| target.path.clone()))
        .or_else(|| requested.map(str::to_string));
    if let Some(page) = page {
        data.insert("page".into(), Value::String(page));
    }
    if let Some(target) = target {
        data.insert(
            "instanceId".into(),
            Value::String(target.instance_id.clone()),
        );
    }
    if let Some(current) = current {
        data.insert(
            "current".into(),
            json!({
                "name": current.name,
                "path": current.path,
                "instanceId": current.instance_id,
            }),
        );
    }
    Value::Object(data)
}

/// A page-driver failure: its code, plus `{ page, instanceId?, current? }`
/// read at the moment it failed.
pub(crate) fn page_error(
    app: &Arc<LxApp>,
    requested: Option<&str>,
    code: &'static str,
    message: impl Into<String>,
) -> RongJSError {
    let requested = requested
        .map(str::trim)
        .filter(|page| !page.is_empty() && !page.eq_ignore_ascii_case("current"));
    let target = PageRef::resolve(app, requested);
    let current = PageRef::resolve(app, None);
    with_json_data(
        coded(code, message),
        &page_data(requested, target.as_ref(), current.as_ref()),
    )
    .into()
}

/// A failure concerning one known page instance (for example one the app's
/// own navigation disposed while a call waited on it).
pub(crate) fn instance_error(
    app: &Arc<LxApp>,
    page: &lxapp::PageInstance,
    name: Option<&str>,
    code: &'static str,
    message: impl Into<String>,
) -> RongJSError {
    let target = PageRef {
        name: name.map(str::to_string),
        path: page.path(),
        instance_id: page.instance_id_string(),
    };
    let current = PageRef::resolve(app, None);
    with_json_data(
        coded(code, message),
        &page_data(None, Some(&target), current.as_ref()),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_half_messages_map_to_stable_codes() {
        let cases = [
            (
                "automation_privilege_required: requires a host privilege grant and a sealed native grant",
                E_AUTOMATION_PRIVILEGE,
            ),
            (
                "host_privilege_required: requires a host privilege grant and a sealed native grant",
                E_AUTOMATION_PRIVILEGE,
            ),
            ("page is not active: detail", E_PAGE_NOT_ACTIVE),
            (
                "page instance 4f2a (pages/detail/index) was disposed before runtime became ready; current page is pages/home/index",
                E_PAGE_NOT_ACTIVE,
            ),
            ("page WebView is not ready", E_PAGE_NOT_READY),
            ("WebView error: No current page", E_PAGE_NOT_READY),
            ("no current page", E_PAGE_NOT_READY),
            ("Element not found: #save", E_ELEMENT_NOT_FOUND),
            (
                "Element not interactable: not enabled",
                E_ELEMENT_NOT_INTERACTABLE,
            ),
            ("E_TIMEOUT: waitFor '#save' (visible)", E_AUTOMATION_TIMEOUT),
            (
                "timed out after 15000ms waiting for page 4f2a to become ready",
                E_AUTOMATION_TIMEOUT,
            ),
            (
                "unknown page name: nope (this session knows: home)",
                E_AUTOMATION,
            ),
            ("JavaScript error: boom", E_AUTOMATION),
            ("redirectTo cannot navigate to a tabBar page", E_AUTOMATION),
        ];
        for (message, code) in cases {
            assert_eq!(code_for(message), code, "{message}");
        }
    }

    #[test]
    fn evaluations_separate_a_throwing_script_from_a_timeout() {
        assert_eq!(
            eval_code_for("JavaScript error: ReferenceError: x"),
            E_EVAL_SCRIPT
        );
        assert_eq!(
            eval_code_for("JavaScript evaluation timed out"),
            E_EVAL_TIMEOUT
        );
        assert_eq!(eval_code_for("page eval timed out"), E_EVAL_TIMEOUT);
        assert_eq!(eval_code_for("lxapp eval timed out"), E_EVAL_TIMEOUT);
        assert_eq!(
            eval_code_for("page is not active: detail"),
            E_PAGE_NOT_ACTIVE
        );
        assert_eq!(
            eval_code_for("WebView destroyed during JavaScript evaluation"),
            E_AUTOMATION
        );
    }

    #[test]
    fn page_data_names_the_target_and_the_current_instance() {
        let current = PageRef {
            name: Some("home".into()),
            path: "pages/home/index".into(),
            instance_id: "a1b2".into(),
        };
        assert_eq!(
            page_data(Some("detail"), None, Some(&current)),
            json!({
                "page": "detail",
                "current": { "name": "home", "path": "pages/home/index", "instanceId": "a1b2" },
            })
        );
        let unnamed = PageRef {
            name: None,
            path: "pages/raw/index".into(),
            instance_id: "c3d4".into(),
        };
        assert_eq!(
            page_data(None, Some(&unnamed), None),
            json!({ "page": "pages/raw/index", "instanceId": "c3d4" })
        );
        assert_eq!(page_data(None, None, None), json!({}));
    }

    #[test]
    fn coded_errors_carry_code_message_and_data() {
        let error = with_json_data(
            coded(E_PAGE_NOT_ACTIVE, "page is not active: detail"),
            &json!({ "page": "detail", "n": 3 }),
        );
        assert_eq!(error.code, E_PAGE_NOT_ACTIVE);
        assert_eq!(error.message, "page is not active: detail");
        let Some(ErrorData::Object(fields)) = error.data else {
            panic!("expected object data");
        };
        assert_eq!(
            fields.get("page").and_then(ErrorData::as_str),
            Some("detail")
        );
        assert!(fields.contains_key("n"));
    }
}
