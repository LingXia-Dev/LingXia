use std::sync::{Arc, OnceLock};

use serde_json::Value;

/// In-process receiver of embedded native-component messages.
///
/// On iOS/Android/HarmonyOS the page's component messages
/// (`component.mount`, `component.update`, `component.unmount`, ...) reach
/// the platform component manager through platform channels
/// (WKScriptMessageHandler, JavascriptInterface, embed proxy) without
/// passing through this crate. Platforms whose component manager is plain
/// in-process Rust (currently Windows) register a host here instead; the
/// page delegate then forwards every component message it receives from
/// the view, keeping this crate platform-agnostic.
pub trait NativeComponentHost: Send + Sync {
    /// Handles one raw component message (JSON text) sent by `page`'s view.
    fn on_component_message(&self, page: &crate::PageInstance, message_json: &str);

    /// Tears down every component mounted by the page identified by
    /// `page_key` (the page's webview-tag key); called when the page
    /// instance is destroyed.
    fn on_page_destroyed(&self, page_key: &str);
}

static NATIVE_COMPONENT_HOST: OnceLock<Arc<dyn NativeComponentHost>> = OnceLock::new();

/// Registers the process-wide native-component host. Returns `false` when a
/// host was already registered (the first registration wins).
pub fn register_native_component_host(host: Arc<dyn NativeComponentHost>) -> bool {
    NATIVE_COMPONENT_HOST.set(host).is_ok()
}

/// Routes a component message from a page view to the registered host;
/// pages on platforms without an in-process host drop the message (their
/// component traffic never reaches this path).
pub(crate) fn dispatch_component_message(page: &crate::PageInstance, message_json: &str) {
    match NATIVE_COMPONENT_HOST.get() {
        Some(host) => host.on_component_message(page, message_json),
        None => {
            crate::debug!("no native-component host registered; dropping component message");
        }
    }
}

/// Notifies the registered host (if any) that a page instance is gone.
pub(crate) fn notify_page_destroyed(page_key: &str) {
    if let Some(host) = NATIVE_COMPONENT_HOST.get() {
        host.on_page_destroyed(page_key);
    }
}

fn normalize_event_name(event_name: &str) -> Option<String> {
    let normalized = event_name.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }
    Some(normalized)
}

fn resolve_function_name(bindings_json: &str, event_name: &str) -> Option<String> {
    let bindings: Value = serde_json::from_str(bindings_json).ok()?;
    let object = bindings.as_object()?;

    if let Some(value) = object.get(event_name).and_then(Value::as_str) {
        let function_name = value.trim();
        if !function_name.is_empty() {
            return Some(function_name.to_string());
        }
    }

    for (raw_key, raw_value) in object {
        let key = raw_key.trim().to_lowercase();
        if key != event_name {
            continue;
        }
        if let Some(value) = raw_value.as_str() {
            let function_name = value.trim();
            if !function_name.is_empty() {
                return Some(function_name.to_string());
            }
        }
    }

    None
}

pub fn on_native_component_event(
    appid: &str,
    path: &str,
    _component_id: &str,
    event_name: &str,
    payload_json: &str,
    bindings_json: &str,
) -> bool {
    if appid.is_empty() || path.is_empty() || payload_json.is_empty() || bindings_json.is_empty() {
        return false;
    }

    let Some(normalized_event_name) = normalize_event_name(event_name) else {
        return false;
    };
    let Some(function_name) = resolve_function_name(bindings_json, &normalized_event_name) else {
        return false;
    };

    crate::try_get(appid)
        .and_then(|lxapp| lxapp.get_page(path))
        .map(|page| page.call_js(function_name, payload_json.to_string()))
        .is_some_and(|result| result.is_ok())
}

#[cfg(test)]
mod tests {
    use super::{normalize_event_name, resolve_function_name};

    #[test]
    fn normalize_event_name_keeps_original_name() {
        assert_eq!(
            normalize_event_name("buffering").as_deref(),
            Some("buffering")
        );
    }

    #[test]
    fn normalize_event_name_trims_and_lowercases() {
        assert_eq!(
            normalize_event_name("  PlayRequest  ").as_deref(),
            Some("playrequest")
        );
    }

    #[test]
    fn resolve_function_name_from_exact_key() {
        let bindings = r#"{"timeupdate":"onTimeUpdate"}"#;
        assert_eq!(
            resolve_function_name(bindings, "timeupdate").as_deref(),
            Some("onTimeUpdate")
        );
    }

    #[test]
    fn resolve_function_name_from_case_insensitive_key() {
        let bindings = r#"{"TimeUpdate":"onTimeUpdate"}"#;
        assert_eq!(
            resolve_function_name(bindings, "timeupdate").as_deref(),
            Some("onTimeUpdate")
        );
    }

    #[test]
    fn resolve_function_name_ignores_non_string_values() {
        let bindings = r#"{"timeupdate":123}"#;
        assert!(resolve_function_name(bindings, "timeupdate").is_none());
    }
}
