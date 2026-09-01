//! Host- and provider-registered control namespaces.
//!
//! Built-in namespaces (`browser`, `app`, `desktop`) live in this crate. A
//! product can ship more without this crate taking a dependency on the
//! provider. Registering a namespace is how the product declares it: the
//! socket allowlists it because a handler exists, not because YAML invented
//! a new capability name.

use serde_json::Value;
use std::sync::{Mutex, OnceLock};

const FRAMEWORK_NAMESPACES: &[&str] = &[
    "app", "browser", "control", "desktop", "echo", "lxapp", "runner", "session",
];

/// One request, one optional JSON result. `None` means this handler does not
/// own the method, so dispatch continues.
pub type ControlNamespaceHandler = fn(&str, Option<Value>) -> Option<Result<Option<Value>, String>>;

struct Registration {
    namespace: &'static str,
    handler: ControlNamespaceHandler,
}

fn registrations() -> &'static Mutex<Vec<Registration>> {
    static REGISTRATIONS: OnceLock<Mutex<Vec<Registration>>> = OnceLock::new();
    REGISTRATIONS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Publish a method prefix on the shared dispatcher.
///
/// `namespace` is the prefix (`plug` for `plug.ping`) and what
/// the local-control allowlist exposes to host-owned integrations.
/// Registering twice for the same prefix replaces the handler.
///
/// Panics when a host tries to shadow a framework-owned namespace. This is a
/// static host configuration error, not a runtime request failure.
pub fn register_control_namespace(namespace: &'static str, handler: ControlNamespaceHandler) {
    assert!(
        !is_framework_namespace(namespace),
        "control namespace `{namespace}` is reserved by LingXia"
    );
    let mut registrations = registrations()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(existing) = registrations
        .iter_mut()
        .find(|registration| registration.namespace == namespace)
    {
        existing.handler = handler;
        return;
    }
    registrations.push(Registration { namespace, handler });
}

pub(crate) fn handle(method: &str, params: Option<Value>) -> Option<Result<Option<Value>, String>> {
    let namespace = method
        .split_once('.')
        .map(|(head, _)| head)
        .unwrap_or(method);
    let handler = {
        let registrations = registrations()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        registrations
            .iter()
            .find(|registration| registration.namespace == namespace)
            .map(|registration| registration.handler)
    };
    handler.and_then(|handler| handler(method, params))
}

#[cfg_attr(not(feature = "local-control"), allow(dead_code))]
pub(crate) fn is_registered(namespace: &str) -> bool {
    registrations()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .any(|registration| registration.namespace == namespace)
}

#[cfg(feature = "local-control")]
pub(crate) fn is_registered_host_namespace(namespace: &str) -> bool {
    !is_framework_namespace(namespace) && is_registered(namespace)
}

fn is_framework_namespace(namespace: &str) -> bool {
    FRAMEWORK_NAMESPACES.contains(&namespace)
}

pub(crate) fn registered_names() -> Vec<&'static str> {
    registrations()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .map(|registration| registration.namespace)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_registered_namespace_is_visible_to_dispatch() {
        register_control_namespace("dispatch_extra_test", |method, _| {
            (method == "dispatch_extra_test.ping")
                .then_some(Ok(Some(serde_json::json!({"ok": true}))))
        });
        assert!(is_registered("dispatch_extra_test"));
        assert!(registered_names().contains(&"dispatch_extra_test"));
        let result = handle("dispatch_extra_test.ping", None).unwrap().unwrap();
        assert_eq!(result, Some(serde_json::json!({"ok": true})));
        assert!(handle("dispatch_extra_test.unknown", None).is_none());
        assert!(handle("other.ping", None).is_none());
    }

    #[test]
    #[should_panic(expected = "reserved by LingXia")]
    fn framework_namespaces_cannot_be_shadowed() {
        register_control_namespace("desktop", |_, _| Some(Ok(None)));
    }

    #[test]
    fn tagged_handler_error_becomes_the_control_code() {
        let response = crate::command_result(
            "1".into(),
            Err("(not_found): Cloud function 'ping' is unavailable.".into()),
        );
        let error = response.error.expect("tagged handler should error");
        assert_eq!(error.code, "not_found");
        assert_eq!(error.message, "Cloud function 'ping' is unavailable.");
    }
}
