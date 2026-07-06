//! Windows page-log capture over the Chrome DevTools Protocol.
//!
//! WebView2 exposes the same CDP the DevTools console uses, so instead of
//! injecting a `console.*` override into every page (which only sees explicit
//! `console.log/...` calls, loses object structure to `JSON.stringify`, and
//! misses uncaught exceptions and browser-emitted diagnostics), we subscribe to
//! the CDP `Runtime`/`Log` domains and forward each entry to the webview
//! delegate's `log` hook — the same sink the injected path fed. This captures a
//! strict superset:
//!
//! - `Runtime.consoleAPICalled` — every `console.*` call, with DevTools-style
//!   object previews.
//! - `Runtime.exceptionThrown` — uncaught exceptions / unhandled rejections
//!   (with stack), which the injected override never saw.
//! - `Log.entryAdded` — browser-level messages (failed subresource loads, CSP
//!   and CORS violations, deprecations) that never pass through JS `console`.

use super::*;
use crate::LogLevel;
use serde_json::Value;

/// CDP events we subscribe to for page-log capture.
pub(crate) const CONSOLE_CAPTURE_EVENTS: &[&str] = &[
    "Runtime.consoleAPICalled",
    "Runtime.exceptionThrown",
    "Log.entryAdded",
];

/// Subscribes to the CDP console/log events and forwards each to the webview
/// delegate's `log`. Returns the receivers+tokens to retain (dropping/removing
/// them stops delivery). Subscriptions are set up before [`enable`] so no event
/// races an un-subscribed receiver.
pub(crate) fn subscribe(
    webview: &ICoreWebView2,
    webtag: &WebTag,
) -> StdResult<Vec<(ICoreWebView2DevToolsProtocolEventReceiver, i64)>> {
    let mut receivers = Vec::with_capacity(CONSOLE_CAPTURE_EVENTS.len());
    for event in CONSOLE_CAPTURE_EVENTS {
        let receiver = unsafe {
            let name = CoTaskMemPWSTR::from(*event);
            webview
                .GetDevToolsProtocolEventReceiver(*name.as_ref().as_pcwstr())
                .map_err(|err| {
                    WebViewError::WebView(format!("event receiver for {event} failed: {err}"))
                })?
        };
        let method = (*event).to_string();
        let webtag = webtag.clone();
        let handler =
            DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_webview, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let mut json = PWSTR::null();
                unsafe {
                    args.ParameterObjectAsJson(&mut json)?;
                }
                let params: Value = serde_json::from_str(&CoTaskMemPWSTR::from(json).to_string())
                    .unwrap_or(Value::Null);
                if let Some((level, message)) = decode_cdp_event(&method, &params)
                    && let Some(delegate) = find_webview_delegate(&webtag)
                {
                    delegate.log(level, &message);
                }
                Ok(())
            }));
        let mut token = 0i64;
        unsafe {
            receiver
                .add_DevToolsProtocolEventReceived(&handler, &mut token)
                .map_err(|err| WebViewError::WebView(format!("subscribe {event} failed: {err}")))?;
        }
        receivers.push((receiver, token));
    }
    Ok(receivers)
}

/// Enables the `Runtime` and `Log` CDP domains so the subscribed events start
/// firing. Fire-and-forget: capture is best-effort dev tooling and is enabled
/// before the first navigation is dispatched, so the async completion racing an
/// early log is not worth blocking creation over.
pub(crate) fn enable(webview: &ICoreWebView2) {
    for method in ["Runtime.enable", "Log.enable"] {
        let handler =
            CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |_result, _json| {
                Ok(())
            }));
        let started = unsafe {
            let method_w = CoTaskMemPWSTR::from(method);
            let params = CoTaskMemPWSTR::from("{}");
            webview.CallDevToolsProtocolMethod(
                *method_w.as_ref().as_pcwstr(),
                *params.as_ref().as_pcwstr(),
                &handler,
            )
        };
        if let Err(err) = started {
            log::warn!("{method} dispatch failed: {err}");
        }
    }
}

/// Maps one CDP console/log event to a `(level, message)` pair, or `None` for
/// events that carry no user-visible line (group markers, `console.clear`, ...).
fn decode_cdp_event(method: &str, params: &Value) -> Option<(LogLevel, String)> {
    match method {
        "Runtime.consoleAPICalled" => {
            let call_type = params.get("type").and_then(Value::as_str).unwrap_or("log");
            // Group/timing/table control calls have no meaningful text line.
            if matches!(
                call_type,
                "clear" | "startGroup" | "startGroupCollapsed" | "endGroup"
            ) {
                return None;
            }
            let args = params.get("args").and_then(Value::as_array)?;
            let message = format_console_args(args);
            Some((console_type_level(call_type), message))
        }
        "Runtime.exceptionThrown" => {
            let details = params.get("exceptionDetails")?;
            Some((LogLevel::Error, format_exception(details)))
        }
        "Log.entryAdded" => {
            let entry = params.get("entry")?;
            let message = format_log_entry(entry);
            Some((
                log_entry_level(entry.get("level").and_then(Value::as_str)),
                message,
            ))
        }
        _ => None,
    }
}

/// `console.*` call type -> log level, matching the previous injected mapping
/// (bare `log` is Info) and extending it to the richer CDP call types.
fn console_type_level(call_type: &str) -> LogLevel {
    match call_type {
        "error" | "assert" => LogLevel::Error,
        "warning" => LogLevel::Warn,
        "debug" | "trace" => LogLevel::Debug,
        _ => LogLevel::Info,
    }
}

/// CDP `Log.entryAdded` level -> log level.
fn log_entry_level(level: Option<&str>) -> LogLevel {
    match level {
        Some("error") => LogLevel::Error,
        Some("warning") => LogLevel::Warn,
        Some("verbose") => LogLevel::Debug,
        _ => LogLevel::Info,
    }
}

/// Joins console arguments the way DevTools renders them: space-separated, each
/// argument formatted from its CDP `RemoteObject`.
fn format_console_args(args: &[Value]) -> String {
    args.iter()
        .map(format_remote_object)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Renders a single CDP `RemoteObject` to a readable string, preferring the
/// primitive value, then a DevTools-style object preview, then the engine's
/// description.
fn format_remote_object(obj: &Value) -> String {
    if let Some(value) = obj.get("value") {
        return match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        };
    }
    if let Some(unserializable) = obj.get("unserializableValue").and_then(Value::as_str) {
        return unserializable.to_string();
    }
    match obj.get("type").and_then(Value::as_str) {
        Some("undefined") => return "undefined".to_string(),
        Some("function") => {
            return obj
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| "function".to_string());
        }
        _ => {}
    }
    // Errors carry their full stack in `description`; that reads better than a
    // `{stack: "...", message: "..."}` property preview of the same data.
    if obj.get("subtype").and_then(Value::as_str) == Some("error")
        && let Some(description) = obj.get("description").and_then(Value::as_str)
    {
        return description.to_string();
    }
    if let Some(preview) = obj.get("preview") {
        return format_object_preview(preview);
    }
    obj.get("description")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| obj.get("type").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default()
}

/// Formats a CDP `ObjectPreview` as `[a, b, …]` for arrays or `{k: v, …}` for
/// objects, appending `…` when the preview was truncated (`overflow`).
fn format_object_preview(preview: &Value) -> String {
    let overflow = preview
        .get("overflow")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let properties = preview.get("properties").and_then(Value::as_array);
    let is_array = preview.get("subtype").and_then(Value::as_str) == Some("array");

    let Some(properties) = properties else {
        return preview
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("[object]")
            .to_string();
    };

    if is_array {
        let mut items: Vec<String> = properties.iter().map(preview_property_value).collect();
        if overflow {
            items.push("…".to_string());
        }
        format!("[{}]", items.join(", "))
    } else {
        let mut items: Vec<String> = properties
            .iter()
            .map(|prop| {
                let name = prop.get("name").and_then(Value::as_str).unwrap_or("");
                format!("{name}: {}", preview_property_value(prop))
            })
            .collect();
        if overflow {
            items.push("…".to_string());
        }
        // Keep a class prefix (e.g. `Foo {…}`) but drop the noisy default
        // `Object` so plain objects read as `{k: v}`.
        let prefix = match preview.get("description").and_then(Value::as_str) {
            Some(desc) if !desc.is_empty() && desc != "Object" => format!("{desc} "),
            _ => String::new(),
        };
        format!("{prefix}{{{}}}", items.join(", "))
    }
}

/// Renders one property from an `ObjectPreview`, quoting nested strings so the
/// line reads like a JS literal.
fn preview_property_value(prop: &Value) -> String {
    let value = prop.get("value").and_then(Value::as_str).unwrap_or("");
    match prop.get("type").and_then(Value::as_str) {
        Some("string") => format!("\"{value}\""),
        // Nested objects/arrays are summarized by the engine (e.g. `Array(3)`).
        _ => value.to_string(),
    }
}

/// Formats a CDP `ExceptionDetails` into a single line: the error's full
/// description (which includes the stack for `Error` objects), else the summary
/// text, with the source location appended when known.
fn format_exception(details: &Value) -> String {
    let body = details
        .get("exception")
        .map(format_remote_object)
        .filter(|text| !text.is_empty())
        .or_else(|| {
            details
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Uncaught exception".to_string());

    match location_suffix(
        details.get("url").and_then(Value::as_str),
        details.get("lineNumber").and_then(Value::as_u64),
        details.get("columnNumber").and_then(Value::as_u64),
    ) {
        Some(location) if !body.contains(&location) => format!("{body} ({location})"),
        _ => body,
    }
}

/// Formats a CDP `LogEntry` (browser-level message) as `[source] text (url:line)`.
fn format_log_entry(entry: &Value) -> String {
    let text = entry
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let source = entry.get("source").and_then(Value::as_str);
    let head = match source {
        Some(source) if !source.is_empty() => format!("[{source}] {text}"),
        _ => text.to_string(),
    };
    match location_suffix(
        entry.get("url").and_then(Value::as_str),
        entry.get("lineNumber").and_then(Value::as_u64),
        None,
    ) {
        Some(location) => format!("{head} ({location})"),
        None => head,
    }
}

/// Builds a `url:line:col` suffix from the parts that are present.
fn location_suffix(url: Option<&str>, line: Option<u64>, column: Option<u64>) -> Option<String> {
    let url = url.filter(|value| !value.is_empty())?;
    Some(match (line, column) {
        (Some(line), Some(column)) => format!("{url}:{line}:{column}"),
        (Some(line), None) => format!("{url}:{line}"),
        _ => url.to_string(),
    })
}
