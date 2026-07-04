//! Windows network capture over the Chrome DevTools Protocol.
//!
//! Enabling capture subscribes to the CDP `Network` domain events via
//! `GetDevToolsProtocolEventReceiver` and records each request/response into a
//! bounded per-webview ring buffer. Response bodies are fetched with
//! `Network.getResponseBody` right after `loadingFinished` (the window before
//! the engine evicts them), capped by size. Dev-tooling only — nothing runs
//! until `Network.enable` is dispatched by a caller.

use super::*;
use crate::traits::{NetworkBody, NetworkCaptureSnapshot, NetworkEntry};
use serde_json::Value;
use std::collections::VecDeque;

/// Max entries retained; older ones are dropped (counted in `dropped`).
const NETWORK_CAPTURE_MAX_ENTRIES: usize = 500;
/// Response bodies larger than this are recorded as `Skipped` to bound memory.
const NETWORK_CAPTURE_MAX_BODY_BYTES: usize = 512 * 1024;
/// Ceiling for the CDP backend's own per-resource / total resource buffers, so
/// it never holds a huge (e.g. compressed-then-inflated) body in memory before
/// we even fetch it. Passed to `Network.enable`.
const NETWORK_CAPTURE_MAX_TOTAL_BUFFER_BYTES: usize = 64 * 1024 * 1024;

/// CDP `Network` events we subscribe to.
pub(crate) const NETWORK_CAPTURE_EVENTS: &[&str] = &[
    "Network.requestWillBeSent",
    "Network.responseReceived",
    "Network.loadingFinished",
    "Network.loadingFailed",
];

#[derive(Default)]
pub(crate) struct NetworkLog {
    entries: VecDeque<NetworkEntry>,
    dropped: u64,
}

impl NetworkLog {
    pub(crate) fn snapshot(&self) -> NetworkCaptureSnapshot {
        NetworkCaptureSnapshot {
            entries: self.entries.iter().cloned().collect(),
            dropped: self.dropped,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.dropped = 0;
    }

    fn index_of(&self, request_id: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| entry.request_id == request_id)
    }

    fn push(&mut self, entry: NetworkEntry) {
        if self.entries.len() >= NETWORK_CAPTURE_MAX_ENTRIES {
            self.entries.pop_front();
            self.dropped += 1;
        }
        self.entries.push_back(entry);
    }

    fn with_entry(&mut self, request_id: &str, update: impl FnOnce(&mut NetworkEntry)) {
        if let Some(index) = self.index_of(request_id)
            && let Some(entry) = self.entries.get_mut(index)
        {
            update(entry);
        }
    }
}

fn json_headers(value: Option<&Value>) -> Vec<(String, String)> {
    let Some(Value::Object(map)) = value else {
        return Vec::new();
    };
    map.iter()
        .map(|(key, val)| {
            let val = match val {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            (key.clone(), val)
        })
        .collect()
}

/// Follow-up CDP fetch a captured event asks the caller to issue (payloads
/// are pulled on demand rather than delivered inline by the event).
pub(crate) enum FollowUp {
    /// The request declared a POST body not delivered inline; pull it with
    /// `Network.getRequestPostData`.
    RequestBody(String),
    /// The response finished; pull its body with `Network.getResponseBody`.
    ResponseBody(String),
}

/// Applies one CDP `Network.*` event (parsed params) to the log, returning a
/// follow-up body fetch when the event warrants one.
pub(crate) fn apply_network_event(
    log: &Mutex<NetworkLog>,
    method: &str,
    params: &Value,
) -> Option<FollowUp> {
    let mut log = log.lock().ok()?;
    let request_id = params.get("requestId")?.as_str()?.to_string();
    match method {
        "Network.requestWillBeSent" => {
            let request = params.get("request")?;
            let inline_body = request
                .get("postData")
                .and_then(Value::as_str)
                .map(str::to_string);
            let has_post_data = request
                .get("hasPostData")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let entry = NetworkEntry {
                request_id: request_id.clone(),
                url: request
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                method: request
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                resource_type: params
                    .get("type")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                request_headers: json_headers(request.get("headers")),
                request_body: inline_body.clone(),
                status: None,
                response_headers: Vec::new(),
                mime_type: None,
                response_body: NetworkBody::None,
                from_cache: false,
                failed: None,
                wall_time: params.get("wallTime").and_then(Value::as_f64),
                started: params
                    .get("timestamp")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0),
                finished: None,
            };
            // A redirect reuses the request id; keep the latest hop.
            if let Some(index) = log.index_of(&request_id) {
                if let Some(existing) = log.entries.get_mut(index) {
                    *existing = entry;
                }
            } else {
                log.push(entry);
            }
            // A POST body the engine didn't inline (large payloads) is pulled
            // separately so agents still see the full request payload.
            (has_post_data && inline_body.is_none()).then_some(FollowUp::RequestBody(request_id))
        }
        "Network.responseReceived" => {
            let response = params.get("response")?;
            let status = response
                .get("status")
                .and_then(Value::as_u64)
                .map(|s| s as u16);
            let headers = json_headers(response.get("headers"));
            let mime = response
                .get("mimeType")
                .and_then(Value::as_str)
                .map(str::to_string);
            let from_cache = response
                .get("fromDiskCache")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            log.with_entry(&request_id, |entry| {
                entry.status = status;
                entry.response_headers = headers;
                entry.mime_type = mime;
                entry.from_cache = from_cache;
            });
            None
        }
        "Network.loadingFinished" => {
            let finished = params.get("timestamp").and_then(Value::as_f64);
            let encoded_len = params
                .get("encodedDataLength")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            log.with_entry(&request_id, |entry| entry.finished = finished);
            // Bound memory before fetching: a body whose wire size already
            // exceeds the cap is marked Skipped instead of pulled into memory
            // via getResponseBody. (A gzipped body under the wire cap but over
            // the cap decoded is still caught by the post-fetch check.)
            if encoded_len > NETWORK_CAPTURE_MAX_BODY_BYTES as f64 {
                let reason = format!(
                    "body ~{} wire bytes over {NETWORK_CAPTURE_MAX_BODY_BYTES} cap",
                    encoded_len as u64
                );
                log.with_entry(&request_id, |entry| {
                    entry.response_body = NetworkBody::Skipped { reason }
                });
                None
            } else {
                // Fetch the body outside the lock (see caller).
                Some(FollowUp::ResponseBody(request_id))
            }
        }
        "Network.loadingFailed" => {
            let error = params
                .get("errorText")
                .and_then(Value::as_str)
                .unwrap_or("request failed")
                .to_string();
            let finished = params.get("timestamp").and_then(Value::as_f64);
            log.with_entry(&request_id, |entry| {
                entry.failed = Some(error);
                entry.finished = finished;
            });
            None
        }
        _ => None,
    }
}

/// Records the outcome of a `Network.getResponseBody` call for `request_id`.
fn record_response_body(log: &Mutex<NetworkLog>, request_id: &str, result: &Value) {
    let Ok(mut log) = log.lock() else {
        return;
    };
    let body = response_body_from_result(result);
    log.with_entry(request_id, |entry| entry.response_body = body);
}

/// Records the outcome of a `Network.getRequestPostData` call for `request_id`.
fn record_request_body(log: &Mutex<NetworkLog>, request_id: &str, result: &Value) {
    let Some(post_data) = result.get("postData").and_then(Value::as_str) else {
        return;
    };
    let Ok(mut log) = log.lock() else {
        return;
    };
    let post_data = post_data.to_string();
    log.with_entry(request_id, |entry| entry.request_body = Some(post_data));
}

/// Subscribes to the CDP `Network` events. Returns the receivers+tokens to
/// retain (dropping/removing them stops delivery). The caller must then call
/// [`enable_domain`] to start event emission — subscriptions are set up first
/// so no event races an un-subscribed receiver.
pub(crate) fn subscribe(
    webview: &ICoreWebView2,
    log: &Arc<Mutex<NetworkLog>>,
) -> StdResult<Vec<(ICoreWebView2DevToolsProtocolEventReceiver, i64)>> {
    let mut receivers = Vec::with_capacity(NETWORK_CAPTURE_EVENTS.len());
    for event in NETWORK_CAPTURE_EVENTS {
        let receiver = unsafe {
            let name = CoTaskMemPWSTR::from(*event);
            webview
                .GetDevToolsProtocolEventReceiver(*name.as_ref().as_pcwstr())
                .map_err(|err| {
                    WebViewError::WebView(format!("event receiver for {event} failed: {err}"))
                })?
        };
        let log = Arc::clone(log);
        let method = (*event).to_string();
        let handler =
            DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |webview, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let mut json = PWSTR::null();
                unsafe {
                    args.ParameterObjectAsJson(&mut json)?;
                }
                let params: Value = serde_json::from_str(&CoTaskMemPWSTR::from(json).to_string())
                    .unwrap_or(Value::Null);
                if let Some(follow_up) = apply_network_event(&log, &method, &params)
                    && let Some(webview) = webview
                {
                    match follow_up {
                        FollowUp::ResponseBody(id) => fetch_body(
                            &webview,
                            &log,
                            "Network.getResponseBody",
                            &id,
                            record_response_body,
                        ),
                        FollowUp::RequestBody(id) => fetch_body(
                            &webview,
                            &log,
                            "Network.getRequestPostData",
                            &id,
                            record_request_body,
                        ),
                    }
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

/// Issues `Network.enable` and replies through `resp` only when the CDP call
/// completes, so `start_network_capture` does not report success before the
/// domain is actually recording (an immediate navigation would otherwise miss
/// the first requests). Runs on the WebView UI thread; the completion fires on
/// the same thread's pump while the caller waits on `resp`.
pub(crate) fn enable_domain(webview: &ICoreWebView2, resp: Sender<StdResult<()>>) {
    let handler =
        CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |result, _json| {
            let reply = result
                .map_err(|err| WebViewError::WebView(format!("Network.enable failed: {err}")));
            let _ = resp.send(reply);
            Ok(())
        }));
    // Cap the DevTools backend's own resource buffers so a large body is
    // bounded before we ever call getResponseBody (belt to the per-entry
    // encoded-size skip below).
    let params = serde_json::json!({
        "maxTotalBufferSize": NETWORK_CAPTURE_MAX_TOTAL_BUFFER_BYTES,
        "maxResourceBufferSize": NETWORK_CAPTURE_MAX_BODY_BYTES,
        "maxPostDataSize": NETWORK_CAPTURE_MAX_BODY_BYTES,
    })
    .to_string();
    let started = unsafe {
        let method = CoTaskMemPWSTR::from("Network.enable");
        let params = CoTaskMemPWSTR::from(params.as_str());
        webview.CallDevToolsProtocolMethod(
            *method.as_ref().as_pcwstr(),
            *params.as_ref().as_pcwstr(),
            &handler,
        )
    };
    // On a dispatch failure the completion never fires; the dropped `resp`
    // surfaces to the caller as a disconnect (mapped to a WebView error).
    if let Err(err) = started {
        log::warn!("Network.enable dispatch failed: {err}");
    }
}

/// Removes the subscriptions and disables the Network domain.
pub(crate) fn stop_capture(
    webview: &ICoreWebView2,
    receivers: &mut Vec<(ICoreWebView2DevToolsProtocolEventReceiver, i64)>,
) {
    for (receiver, token) in receivers.drain(..) {
        unsafe {
            let _ = receiver.remove_DevToolsProtocolEventReceived(token);
        }
    }
    let _ = call_cdp(webview, "Network.disable", "{}");
}

/// Fire-and-forget CDP call (no result needed).
fn call_cdp(webview: &ICoreWebView2, method: &str, params_json: &str) -> StdResult<()> {
    let handler =
        CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |_result, _json| Ok(())));
    unsafe {
        let method_w = CoTaskMemPWSTR::from(method);
        let params = CoTaskMemPWSTR::from(params_json);
        webview
            .CallDevToolsProtocolMethod(
                *method_w.as_ref().as_pcwstr(),
                *params.as_ref().as_pcwstr(),
                &handler,
            )
            .map_err(|err| WebViewError::WebView(format!("CDP {method} failed: {err}")))
    }
}

/// Issues a `{requestId}` CDP body fetch and records its JSON result via
/// `record`. Best effort — a failed call (body evicted, request had none)
/// just leaves the entry unchanged.
fn fetch_body(
    webview: &ICoreWebView2,
    log: &Arc<Mutex<NetworkLog>>,
    method: &'static str,
    request_id: &str,
    record: fn(&Mutex<NetworkLog>, &str, &Value),
) {
    let log = Arc::clone(log);
    let id = request_id.to_string();
    let handler =
        CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |result, return_json| {
            if result.is_ok() {
                let value: Value = serde_json::from_str(&return_json).unwrap_or(Value::Null);
                record(&log, &id, &value);
            }
            Ok(())
        }));
    let params = serde_json::json!({ "requestId": request_id }).to_string();
    unsafe {
        let method_w = CoTaskMemPWSTR::from(method);
        let params_w = CoTaskMemPWSTR::from(params.as_str());
        let _ = webview.CallDevToolsProtocolMethod(
            *method_w.as_ref().as_pcwstr(),
            *params_w.as_ref().as_pcwstr(),
            &handler,
        );
    }
}

fn response_body_from_result(result: &Value) -> NetworkBody {
    let body = result.get("body").and_then(Value::as_str).unwrap_or("");
    let base64 = result
        .get("base64Encoded")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let raw_len = if base64 {
        body.len() / 4 * 3
    } else {
        body.len()
    };
    if raw_len > NETWORK_CAPTURE_MAX_BODY_BYTES {
        return NetworkBody::Skipped {
            reason: format!("body {raw_len} bytes over {NETWORK_CAPTURE_MAX_BODY_BYTES} cap"),
        };
    }
    if body.is_empty() {
        return NetworkBody::None;
    }
    if base64 {
        NetworkBody::Base64 {
            base64: body.to_string(),
        }
    } else {
        NetworkBody::Text {
            text: body.to_string(),
        }
    }
}
