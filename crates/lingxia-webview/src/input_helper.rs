#![cfg_attr(
    not(any(
        target_os = "android",
        target_os = "ios",
        target_os = "macos",
        all(target_os = "linux", target_env = "ohos")
    )),
    allow(dead_code)
)]

use crate::WebViewScriptError;
use serde::Deserialize;
use serde_json::Value;
#[cfg(any(
    target_os = "android",
    all(target_os = "linux", target_env = "ohos"),
    test
))]
use std::collections::hash_map::RandomState;
#[cfg(any(
    target_os = "android",
    all(target_os = "linux", target_env = "ohos"),
    test
))]
use std::hash::{BuildHasher, Hash, Hasher};

#[cfg(all(
    feature = "webview-input",
    any(target_os = "macos", target_os = "windows")
))]
pub(crate) const INPUT_HELPER_BOOTSTRAP: &str = include_str!("input_helper.js");

#[derive(Debug, Deserialize)]
struct EvalEnvelope {
    ok: bool,
    #[serde(default)]
    value: Value,
    #[serde(default)]
    error: Option<String>,
}

/// Heuristic: does `src` look like a statement body rather than an
/// expression suitable for `await (src)`?
///
/// WebView eval is expression-first because internal callers pass IIFEs and
/// browser/lxapp page eval both need CSP-safe execution. The statement-body
/// path is only a compatibility convenience for simple leading statements.
fn looks_like_function_body(src: &str) -> bool {
    // Match on leading statement keywords only. We deliberately do NOT use
    // ";"-as-anywhere as a signal — expression IIFEs commonly embed `;` inside
    // their body (e.g. `((sel, idx) => { const r = ...; return ...; })(sel, idx)`)
    // and we would mis-classify them as function bodies, dropping the value.
    let trimmed = src.trim_start();
    trimmed.starts_with("const ")
        || trimmed.starts_with("let ")
        || trimmed.starts_with("var ")
        || trimmed.starts_with("if ")
        || trimmed.starts_with("for ")
        || trimmed.starts_with("while ")
        || trimmed.starts_with("try ")
        || trimmed.starts_with("return ")
        || trimmed.starts_with("return;")
        || trimmed == "return"
        || trimmed.starts_with("function ")
}

#[cfg(any(
    target_os = "android",
    all(target_os = "linux", target_env = "ohos"),
    test
))]
pub(crate) fn new_eval_token(request_id: u64) -> String {
    fn hash_with_random_state(request_id: u64, domain: u8) -> u64 {
        let mut hasher = RandomState::new().build_hasher();
        domain.hash(&mut hasher);
        request_id.hash(&mut hasher);
        hasher.finish()
    }

    let hi = hash_with_random_state(request_id, 1);
    let lo = hash_with_random_state(request_id, 2);
    format!("{hi:016x}{lo:016x}")
}

/// Build a CSP-safe, await-aware eval body. Returned source produces a
/// JSON envelope string `{"ok":true,"value":...}` / `{"ok":false,"error":"..."}`
/// once awaited.
///
/// `resolve_call` is an optional JS expression evaluated with the envelope
/// string available as `__lxR` once the body finishes. Platforms whose native
/// API does not natively await Promises (Android, Harmony) inject a bridge
/// call here to route the result back to native code; platforms that natively
/// await (iOS/macOS via `callAsyncJavaScript:`) pass `None` and read the
/// envelope from the function's return value.
pub(crate) fn build_async_eval_body(src: &str, resolve_call: Option<&str>) -> String {
    let inner_await = if looks_like_function_body(src) {
        format!("await (async () => {{ {src} }})()")
    } else {
        format!("await ({src})")
    };
    let envelope_then_finish = match resolve_call {
        Some(call) => format!("{call}; return;"),
        None => "return __lxR;".to_string(),
    };
    format!(
        "let __lxR; \
         try {{ \
           const __lxV = {inner_await}; \
           __lxR = JSON.stringify({{ok:true, value: __lxV === undefined ? null : __lxV}}); \
         }} catch (e) {{ \
           __lxR = JSON.stringify({{ok:false, error: String(e && e.stack ? e.stack : e)}}); \
         }} \
         {envelope_then_finish}"
    )
}

pub(crate) fn parse_wrapped_eval_result(raw: &str) -> Result<Value, WebViewScriptError> {
    let envelope: EvalEnvelope = serde_json::from_str(raw).map_err(|err| {
        WebViewScriptError::Platform(format!(
            "Failed to decode JavaScript result envelope: {err}"
        ))
    })?;
    if envelope.ok {
        Ok(envelope.value)
    } else {
        Err(WebViewScriptError::Js(envelope.error.unwrap_or_else(
            || "JavaScript evaluation failed".to_string(),
        )))
    }
}

#[cfg(all(
    feature = "webview-input",
    any(target_os = "macos", target_os = "windows")
))]
pub(crate) fn build_helper_invocation(expr: &str) -> String {
    format!(
        "(() => {{ {} return {}; }})()",
        INPUT_HELPER_BOOTSTRAP, expr
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn async_eval_body_expression_uses_await() {
        let body = build_async_eval_body("1 + 1", None);
        assert!(body.contains("await (1 + 1)"));
        assert!(body.contains("return __lxR;"));
    }

    #[test]
    fn async_eval_body_function_body_wraps_with_iife() {
        let body = build_async_eval_body("const x = 1; return x;", None);
        assert!(body.contains("await (async () => { const x = 1; return x; })()"));
    }

    #[test]
    fn async_eval_body_iife_expression_with_semicolons_is_expression() {
        // Regression: `((sel, idx) => { ...; ...; return ...; })(sel, idx)`
        // is an EXPRESSION (the IIFE returns a value), not a function body.
        // The old heuristic flagged any source containing `;` as body and
        // turned this into `await (async () => { <expr> })()` which discards
        // the IIFE's return value.
        let body = build_async_eval_body(
            "((sel, idx) => { const els = []; return {ok:true}; })('x', 0)",
            None,
        );
        assert!(body.contains("await (((sel, idx) =>"));
        assert!(!body.contains("await (async () =>"));
    }

    #[test]
    fn async_eval_body_with_resolve_call_replaces_return() {
        let body = build_async_eval_body("await lx.foo()", Some("Bridge.resolve('r0', __lxR)"));
        assert!(body.contains("Bridge.resolve('r0', __lxR);"));
        assert!(!body.contains("return __lxR;"));
    }

    #[test]
    fn eval_token_is_not_request_id() {
        let token = new_eval_token(42);
        assert_eq!(token.len(), 32);
        assert_ne!(token, "42");
    }

    #[test]
    fn parse_wrapped_eval_result_decodes_success() {
        let value = parse_wrapped_eval_result(r#"{"ok":true,"value":{"answer":42}}"#).unwrap();
        assert_eq!(value["answer"], 42);
    }

    #[test]
    fn parse_wrapped_eval_result_maps_js_error() {
        let err = parse_wrapped_eval_result(r#"{"ok":false,"error":"boom"}"#).unwrap_err();
        assert!(matches!(err, WebViewScriptError::Js(message) if message == "boom"));
    }

    #[cfg(all(feature = "webview-input", target_os = "macos"))]
    #[test]
    fn helper_invocation_bootstraps_namespace() {
        let script = build_helper_invocation("window.__LingXiaInput.query_box(\"#app\")");
        assert!(script.contains("__LingXiaInput"));
        assert!(script.contains("query_box"));
    }
}
