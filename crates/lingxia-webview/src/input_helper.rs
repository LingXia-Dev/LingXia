use crate::WebViewScriptError;
use serde::Deserialize;
use serde_json::Value;

#[cfg(any(all(feature = "webview-input", target_os = "macos"), test))]
pub(crate) const INPUT_HELPER_BOOTSTRAP: &str = r#"
(function() {
    if (window.__LingXiaInput) return;

    function findElement(selector, index) {
        if (typeof selector !== 'string' || selector.trim() === '') {
            return { el: null, count: 0 };
        }
        try {
            const nodes = Array.from(document.querySelectorAll(selector));
            const resolvedIndex = Number.isInteger(index) && index >= 0 ? index : 0;
            return { el: nodes[resolvedIndex] || null, count: nodes.length, index: resolvedIndex };
        } catch (_err) {
            return { el: null, count: 0 };
        }
    }

    function isEditable(el) {
        if (!el) return false;
        if (el.isContentEditable) return true;
        const tag = (el.tagName || '').toLowerCase();
        if (tag === 'textarea') {
            return !el.disabled && !el.readOnly;
        }
        if (tag === 'input') {
            const type = (el.type || 'text').toLowerCase();
            const blocked = new Set(['button', 'checkbox', 'color', 'file', 'hidden', 'image', 'radio', 'range', 'reset', 'submit']);
            return !el.disabled && !el.readOnly && !blocked.has(type);
        }
        return false;
    }

    function rectPayload(el) {
        const rect = el.getBoundingClientRect();
        const visible = rect.width > 0 &&
            rect.height > 0 &&
            rect.bottom > 0 &&
            rect.right > 0 &&
            rect.top < window.innerHeight &&
            rect.left < window.innerWidth;
        return {
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height,
            centerX: rect.left + (rect.width / 2),
            centerY: rect.top + (rect.height / 2),
            viewportWidth: window.innerWidth,
            viewportHeight: window.innerHeight,
            visible,
            editable: isEditable(el)
        };
    }

    function elementResult(selector, index) {
        const found = findElement(selector, index);
        const el = found.el;
        if (!el) {
            return { ok: false, error: `Element not found: ${selector}`, count: found.count, index: found.index || 0 };
        }
        return { ok: true, count: found.count, index: found.index || 0, ...rectPayload(el) };
    }

    window.__LingXiaInput = {
        query_box(selector, index) {
            return elementResult(selector, index);
        },
        is_visible(selector, index) {
            const result = elementResult(selector, index);
            return result.ok ? { ok: true, visible: result.visible } : result;
        },
        is_editable(selector, index) {
            const result = elementResult(selector, index);
            return result.ok ? { ok: true, editable: result.editable } : result;
        }
    };
})();
"#;

#[derive(Debug, Deserialize)]
struct EvalEnvelope {
    ok: bool,
    #[serde(default)]
    value: Value,
    #[serde(default)]
    error: Option<String>,
}

pub(crate) fn build_wrapped_eval_script(js: &str) -> Result<String, WebViewScriptError> {
    let quoted = serde_json::to_string(js)
        .map_err(|err| WebViewScriptError::Platform(format!("Failed to encode script: {err}")))?;
    Ok(format!(
        "(function(){{try{{const __lxValue=(0,eval)({quoted});return JSON.stringify({{ok:true,value:__lxValue===undefined?null:__lxValue}});}}catch(e){{return JSON.stringify({{ok:false,error:String(e)}});}}}})()"
    ))
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

#[cfg(all(feature = "webview-input", target_os = "macos"))]
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
    fn wrapped_eval_script_quotes_source() {
        let script = build_wrapped_eval_script("1 + 1").unwrap();
        assert!(script.contains("eval"));
        assert!(script.contains("\"1 + 1\""));
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

    #[test]
    fn helper_invocation_bootstraps_namespace() {
        #[cfg(not(all(feature = "webview-input", target_os = "macos")))]
        return;
        let script = build_helper_invocation("window.__LingXiaInput.query_box(\"#app\")");
        assert!(script.contains("__LingXiaInput"));
        assert!(script.contains("query_box"));
    }
}
