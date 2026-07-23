use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::input_helper::{build_async_eval_body, new_eval_token, parse_wrapped_eval_result};
use crate::webview::{
    EffectiveWebViewCreateOptions, ProxyActivation, ProxyApplyReport, ProxyConfig, WebTag,
    WebViewCreateSender, WebViewCreateStage,
};
use crate::{
    LoadDataRequest, UserAgentOverride, WebViewController, WebViewError, WebViewScriptError,
};
use async_trait::async_trait;
use jni::objects::{Global, JObject, JValue};
use jni::{jni_sig, jni_str};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;

// Import JNI environment access from shared utils
use super::jni_env::{get_lingxia_webview_class, with_env};

fn encode_options_token(options: &EffectiveWebViewCreateOptions) -> Result<String, WebViewError> {
    let json = serde_json::to_vec(options).map_err(|e| {
        WebViewError::InvalidCreateOptions(format!("Serialize options failed: {e}"))
    })?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

// Type alias for WebView senders map to reduce complexity
pub(crate) struct PendingWebViewCreation {
    pub sender: WebViewCreateSender,
    pub effective_options: EffectiveWebViewCreateOptions,
}

type WebViewSendersMap = Arc<Mutex<HashMap<String, PendingWebViewCreation>>>;
type PendingEvalRequests = Arc<Mutex<HashMap<u64, PendingEvalEntry>>>;
type PendingScreenshotRequests = Arc<Mutex<HashMap<u64, PendingScreenshotEntry>>>;

enum PendingEvalResponse {
    Success(String),
    Failure(String),
    Destroyed,
}

struct PendingEvalEntry {
    webtag: String,
    token: String,
    sender: oneshot::Sender<PendingEvalResponse>,
}

enum PendingScreenshotResponse {
    Success(Vec<u8>),
    Failure(String),
    Destroyed,
}

struct PendingScreenshotEntry {
    webtag: String,
    sender: oneshot::Sender<PendingScreenshotResponse>,
}

// Global map to store senders for WebView creation
pub(crate) static WEBVIEW_SENDERS: OnceLock<WebViewSendersMap> = OnceLock::new();
static PENDING_EVAL_REQUESTS: OnceLock<PendingEvalRequests> = OnceLock::new();
static PENDING_SCREENSHOT_REQUESTS: OnceLock<PendingScreenshotRequests> = OnceLock::new();
static NEXT_EVAL_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_SCREENSHOT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
const EVAL_TIMEOUT: Duration = Duration::from_secs(10);
const EVAL_PARSE_GUARD_MS: u64 = 1000;
const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(5);

fn pending_eval_requests() -> &'static PendingEvalRequests {
    PENDING_EVAL_REQUESTS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

pub(crate) fn complete_pending_eval_request(
    request_id: u64,
    token: &str,
    result: Result<String, String>,
) {
    if let Ok(mut pending) = pending_eval_requests().lock()
        && pending
            .get(&request_id)
            .is_some_and(|entry| entry.token == token)
        && let Some(entry) = pending.remove(&request_id)
    {
        let message = match result {
            Ok(value) => PendingEvalResponse::Success(value),
            Err(error) => PendingEvalResponse::Failure(error),
        };
        let _ = entry.sender.send(message);
    }
}

fn fail_pending_eval_requests_for_webtag(webtag: &WebTag) {
    if let Ok(mut pending) = pending_eval_requests().lock() {
        let matching = pending
            .iter()
            .filter_map(|(request_id, entry)| {
                (entry.webtag == webtag.as_str()).then_some(*request_id)
            })
            .collect::<Vec<_>>();
        for request_id in matching {
            if let Some(entry) = pending.remove(&request_id) {
                let _ = entry.sender.send(PendingEvalResponse::Destroyed);
            }
        }
    }
}

fn pending_screenshot_requests() -> &'static PendingScreenshotRequests {
    PENDING_SCREENSHOT_REQUESTS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

pub(crate) fn complete_pending_screenshot_request(
    request_id: u64,
    result: Result<Vec<u8>, String>,
) {
    if let Ok(mut pending) = pending_screenshot_requests().lock()
        && let Some(entry) = pending.remove(&request_id)
    {
        let message = match result {
            Ok(bytes) => PendingScreenshotResponse::Success(bytes),
            Err(error) => PendingScreenshotResponse::Failure(error),
        };
        let _ = entry.sender.send(message);
    }
}

fn fail_pending_screenshot_requests_for_webtag(webtag: &WebTag) {
    if let Ok(mut pending) = pending_screenshot_requests().lock() {
        let matching = pending
            .iter()
            .filter_map(|(request_id, entry)| {
                (entry.webtag == webtag.as_str()).then_some(*request_id)
            })
            .collect::<Vec<_>>();
        for request_id in matching {
            if let Some(entry) = pending.remove(&request_id) {
                let _ = entry.sender.send(PendingScreenshotResponse::Destroyed);
            }
        }
    }
}

pub(crate) fn apply_http_proxy(
    config: Option<&ProxyConfig>,
) -> Result<ProxyApplyReport, WebViewError> {
    let host = config.map(|cfg| cfg.host.as_str());
    let port = config.map(|cfg| cfg.port as i32).unwrap_or(0);
    let bypass = config.map(|cfg| cfg.bypass.clone()).unwrap_or_default();

    with_env(
        |env| -> Result<ProxyApplyReport, Box<dyn std::error::Error>> {
            let webview_class =
                get_lingxia_webview_class().ok_or("LingXiaWebView class not cached")?;
            let host_obj = match host {
                Some(value) => JObject::from(env.new_string(value)?),
                None => JObject::null(),
            };

            let bypass_array = env.new_object_array(
                bypass.len() as i32,
                jni_str!("java/lang/String"),
                JObject::null(),
            )?;
            for (idx, rule) in bypass.iter().enumerate() {
                let rule_string = env.new_string(rule)?;
                bypass_array.set_element(env, idx, &rule_string)?;
            }
            let bypass_arg = JObject::from(bypass_array);

            let result = env.call_static_method(
                webview_class,
                jni_str!("applyHttpProxy"),
                jni_sig!("(Ljava/lang/String;I[Ljava/lang/String;)Ljava/lang/String;"),
                &[(&host_obj).into(), port.into(), (&bypass_arg).into()],
            )?;

            let error_obj = result.l()?;
            if !error_obj.is_null() {
                let error_jstring = jni::objects::JString::cast_local(env, error_obj)?;
                let error = error_jstring.try_to_string(env)?;
                if let Some(detail) = error.strip_prefix("UNSUPPORTED:") {
                    return Ok(ProxyApplyReport::unsupported(detail.trim()));
                }
                return Err(format!("Android proxy apply failed: {}", error).into());
            }

            let report = if config.is_some() {
                ProxyApplyReport::applied(ProxyActivation::EffectiveNow)
            } else {
                ProxyApplyReport::cleared(ProxyActivation::EffectiveNow)
            };
            Ok(report)
        },
    )
    .map_err(|e| WebViewError::WebView(format!("Failed to apply Android proxy: {:?}", e)))
}

#[derive(Debug)]
pub struct WebViewInner {
    java_webview: Option<Global<JObject<'static>>>,
    pub(crate) webtag: WebTag,
}

impl WebViewInner {
    /// Create a new WebView asynchronously by calling Java static method and storing sender
    pub(crate) fn create(
        appid: &str,
        path: &str,
        session_id: Option<u64>,
        effective_options: EffectiveWebViewCreateOptions,
        sender: WebViewCreateSender,
    ) {
        // Store sender in global map for callback
        let webtag = WebTag::new(appid, path, session_id);
        let senders = WEBVIEW_SENDERS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));

        if let Ok(mut senders_map) = senders.lock() {
            senders_map.insert(
                webtag.to_string(),
                PendingWebViewCreation {
                    sender,
                    effective_options: effective_options.clone(),
                },
            );
        }

        // Helper function to remove sender and send error
        let remove_and_send_error = |error_msg: String| {
            if let Ok(mut senders_map) = senders.lock()
                && let Some(pending) = senders_map.remove(&webtag.to_string())
            {
                pending.sender.fail(
                    WebViewCreateStage::Requested,
                    WebViewError::WebView(error_msg),
                );
            }
        };

        let appid_owned = appid.to_string();
        let path_owned = path.to_string();
        let options_token = match encode_options_token(&effective_options) {
            Ok(token) => token,
            Err(e) => {
                remove_and_send_error(format!("Failed to encode create options token: {e}"));
                return;
            }
        };

        // Get JNI environment via closure
        let result = with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            // Get WebView class reference
            let webview_class =
                get_lingxia_webview_class().ok_or("LingXiaWebView class not cached")?;

            // Create Java strings
            let appid_jstring = env.new_string(&appid_owned)?;
            let path_jstring = env.new_string(&path_owned)?;
            let session = session_id.unwrap_or_default() as i64;
            let options_jstring = env.new_string(&options_token)?;

            // Require the new API with options token.
            // If this fails, Java/Rust artifacts are mismatched and must be rebuilt together.
            env.call_static_method(
                webview_class,
                jni_str!("requestWebView"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;JLjava/lang/String;)V"),
                &[
                    (&appid_jstring).into(),
                    (&path_jstring).into(),
                    session.into(),
                    (&options_jstring).into(),
                ],
            )?;

            log::info!(
                "Successfully requested WebView creation for {}-{}",
                appid_owned,
                path_owned
            );
            Ok(())
        });

        if let Err(e) = result {
            log::error!("Failed to request WebView creation: {:?}", e);
            remove_and_send_error(format!("Failed to request WebView creation: {:?}", e));
        }
    }

    /// Create WebViewInner from existing Java WebView object (called from onWebViewReady)
    pub(crate) fn from_java_object(java_webview: Global<JObject<'static>>, webtag: WebTag) -> Self {
        WebViewInner {
            java_webview: Some(java_webview),
            webtag,
        }
    }

    pub fn get_java_webview(&self) -> &Global<JObject<'static>> {
        self.java_webview
            .as_ref()
            .expect("Android WebView global reference is missing")
    }
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        fail_pending_eval_requests_for_webtag(&self.webtag);
        fail_pending_screenshot_requests_for_webtag(&self.webtag);
        let Some(java_webview) = self.java_webview.take() else {
            return;
        };
        let _ = with_env(move |env| -> Result<(), Box<dyn std::error::Error>> {
            let _ = env.call_method(&*java_webview, jni_str!("destroy"), jni_sig!("()V"), &[]);
            drop(java_webview);
            Ok(())
        });
        log::info!(
            "[WebViewInner] Android WebViewInner dropped and destroyed ({})",
            self.webtag.as_str()
        );
    }
}

#[derive(Debug, serde::Deserialize)]
struct AndroidClickQueryResult {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    cx: f64,
    #[serde(default)]
    cy: f64,
    #[serde(default)]
    dpr: f64,
    #[serde(default)]
    visible: bool,
    #[serde(default)]
    enabled: bool,
}

impl WebViewInner {
    /// Native-dispatch a click on `selector` (nth match) via Chromium's own
    /// touch pipeline. Steps:
    ///  1. Run a CSP-safe expression that locates the element, scrolls it
    ///     into view, and returns its viewport-relative center + DPR.
    ///  2. Call Java `LingXiaWebView.dispatchClickAt(x, y)` which builds a
    ///     real `MotionEvent` (ACTION_DOWN/UP) — Chromium treats it as a
    ///     genuine touch (fires touchstart/touchend → click, focuses inputs,
    ///     surfaces the IME).
    pub(crate) async fn click_inner(
        &self,
        selector: &str,
        options: crate::traits::ClickOptions,
    ) -> Result<(), crate::WebViewInputError> {
        let selector_json = serde_json::to_string(selector).map_err(|err| {
            crate::WebViewInputError::Platform(format!("Invalid selector: {err}"))
        })?;
        let index = options.index.unwrap_or(0);
        let expr = format!(
            "((sel, idx) => {{ const els = document.querySelectorAll(sel); const el = els[idx]; \
             if (!el) return {{ ok:false, error:'no match', count:els.length }}; \
             if (typeof el.scrollIntoView === 'function') {{ try {{ el.scrollIntoView({{block:'center', inline:'center'}}); }} catch(_e){{}} }} \
             const r = el.getBoundingClientRect(); \
             const s = window.getComputedStyle(el); \
             const disabled = !!el.disabled || el.getAttribute('aria-disabled') === 'true'; \
             return {{ ok:true, cx:r.left + r.width/2, cy:r.top + r.height/2, dpr:window.devicePixelRatio || 1, \
                       enabled: !disabled, \
                       visible: r.width > 0 && r.height > 0 && r.bottom > 0 && r.right > 0 && \
                                r.top < window.innerHeight && r.left < window.innerWidth && \
                                s.visibility !== 'hidden' && s.display !== 'none' && Number(s.opacity || '1') !== 0 }}; \
             }})({selector_json}, {index})"
        );
        let value = self
            .eval_js(&expr)
            .await
            .map_err(crate::WebViewInputError::Script)?;
        let result: AndroidClickQueryResult = serde_json::from_value(value).map_err(|err| {
            crate::WebViewInputError::Platform(format!(
                "Failed to decode click query result: {err}"
            ))
        })?;
        if !result.ok {
            return Err(crate::WebViewInputError::ElementNotFound(
                result.error.unwrap_or_else(|| selector.to_string()),
            ));
        }
        if !result.visible {
            return Err(crate::WebViewInputError::ElementNotInteractable(format!(
                "Element not visible: {selector}"
            )));
        }
        if !result.enabled {
            return Err(crate::WebViewInputError::ElementNotInteractable(format!(
                "Element not enabled: {selector}"
            )));
        }
        let dpr = if result.dpr > 0.0 { result.dpr } else { 1.0 };
        let device_x = (result.cx * dpr) as f32;
        let device_y = (result.cy * dpr) as f32;
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            env.call_method(
                &*self.get_java_webview(),
                jni_str!("dispatchClickAt"),
                jni_sig!("(FF)V"),
                &[device_x.into(), device_y.into()],
            )?;
            Ok(())
        })
        .map_err(|err| {
            crate::WebViewInputError::Platform(format!("dispatchClickAt failed: {:?}", err))
        })
    }

    /// Scroll page content by `(dx, dy)` CSS pixels. Android scrolls page-level
    /// content in the native View layer (the DOM document has no scroll extent),
    /// so this drives `WebView.scrollBy` after converting CSS px → device px.
    pub(crate) async fn scroll_inner(
        &self,
        dx: f64,
        dy: f64,
        _options: crate::traits::ScrollOptions,
    ) -> Result<(), crate::WebViewInputError> {
        let dpr = self
            .eval_js("(window.devicePixelRatio || 1)")
            .await
            .map_err(crate::WebViewInputError::Script)?
            .as_f64()
            .filter(|v| *v > 0.0)
            .unwrap_or(1.0);
        let device_dx = (dx * dpr).round() as i32;
        let device_dy = (dy * dpr).round() as i32;
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            env.call_method(
                &*self.get_java_webview(),
                jni_str!("scrollByPixels"),
                jni_sig!("(II)V"),
                &[device_dx.into(), device_dy.into()],
            )?;
            Ok(())
        })
        .map_err(|err| {
            crate::WebViewInputError::Platform(format!("scrollByPixels failed: {:?}", err))
        })
    }

    /// Scroll the first matching element into view. Each swipe is capped to one
    /// gesture, so repeatedly read the element's viewport-relative top and swipe
    /// toward it until it sits ~40 CSS px below the top edge (or it stops moving).
    pub(crate) async fn scroll_to_inner(
        &self,
        selector: &str,
        _options: crate::traits::ScrollOptions,
    ) -> Result<(), crate::WebViewInputError> {
        let selector_json = serde_json::to_string(selector).map_err(|err| {
            crate::WebViewInputError::Platform(format!("Invalid selector: {err}"))
        })?;
        let expr = format!(
            "((sel) => {{ const el = document.querySelector(sel); \
             if (!el) return {{ ok:false }}; const r = el.getBoundingClientRect(); \
             return {{ ok:true, top:r.top, dpr:window.devicePixelRatio || 1 }}; }})({selector_json})"
        );
        let mut last_top = f64::INFINITY;
        for _ in 0..10 {
            let value = self
                .eval_js(&expr)
                .await
                .map_err(crate::WebViewInputError::Script)?;
            if value.get("ok").and_then(|v| v.as_bool()) != Some(true) {
                return Err(crate::WebViewInputError::ElementNotFound(
                    selector.to_string(),
                ));
            }
            let top = value.get("top").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let dpr = value
                .get("dpr")
                .and_then(|v| v.as_f64())
                .filter(|v| *v > 0.0)
                .unwrap_or(1.0);
            // Close enough, or the last swipe made no further progress (clamped
            // at a scroll boundary) — stop.
            if top.abs() <= 48.0 || (last_top - top).abs() < 4.0 {
                break;
            }
            last_top = top;
            let device_dy = ((top - 40.0) * dpr).round() as i32;
            with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
                env.call_method(
                    &*self.get_java_webview(),
                    jni_str!("scrollByPixels"),
                    jni_sig!("(II)V"),
                    &[0i32.into(), device_dy.into()],
                )?;
                Ok(())
            })
            .map_err(|err| {
                crate::WebViewInputError::Platform(format!("scrollByPixels failed: {:?}", err))
            })?;
        }
        Ok(())
    }
}

#[async_trait]
impl WebViewController for WebViewInner {
    fn load_url(&self, url: &str) -> Result<(), WebViewError> {
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            let url_string = env.new_string(&url)?;
            env.call_method(
                &*self.get_java_webview(),
                jni_str!("loadUrl"),
                jni_sig!("(Ljava/lang/String;)V"),
                &[(&url_string).into()],
            )?;
            Ok(())
        })
        .map_err(|e| WebViewError::WebView(format!("Failed to load URL: {:?}", e)))
    }

    fn load_data(&self, request: LoadDataRequest<'_>) -> Result<(), WebViewError> {
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            let data_string = env.new_string(request.data)?;
            let base_url_string = env.new_string(request.base_url)?;
            let history_url_string = match request.history_url {
                Some(url) => env.new_string(&url)?,
                None => env.new_string(request.base_url)?,
            };

            env.call_method(
                &*self.get_java_webview(),
                jni_str!("loadHtmlData"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V"),
                &[
                    (&data_string).into(),
                    (&base_url_string).into(),
                    (&history_url_string).into(),
                ],
            )?;
            Ok(())
        })
        .map_err(|e| WebViewError::WebView(format!("Failed to load data: {:?}", e)))
    }

    fn exec_js(&self, js: &str) -> Result<(), WebViewError> {
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            let script_string = env.new_string(js)?;
            env.call_method(
                &*self.get_java_webview(),
                jni_str!("evaluateJavascript"),
                jni_sig!("(Ljava/lang/String;Landroid/webkit/ValueCallback;)V"),
                &[(&script_string).into(), (&JObject::null()).into()],
            )?;
            Ok(())
        })
        .map_err(|e| WebViewError::WebView(format!("JavaScript execution failed: {:?}", e)))
    }

    async fn eval_js(&self, js: &str) -> Result<serde_json::Value, WebViewScriptError> {
        // CSP-safe + await-aware: the user script is wrapped in an async IIFE
        // that awaits the result, builds a `{ok, value | error}` envelope, then
        // ferries it back via `LingXiaProxy.resolveEval(reqId, envelope)`.
        // Chromium's `WebView.evaluateJavascript` ValueCallback fires on Promise
        // *creation* (not on resolution), so we deliberately ignore it and rely
        // on the JS bridge round-trip — which gives us native `await` support
        // without touching `eval()` (CSP `'unsafe-eval'` stays unneeded).
        let request_id = NEXT_EVAL_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let token = new_eval_token(request_id);
        let (tx, rx) = oneshot::channel();

        pending_eval_requests()
            .lock()
            .map_err(|_| {
                WebViewScriptError::Platform("Android pending eval_js map poisoned".to_string())
            })?
            .insert(
                request_id,
                PendingEvalEntry {
                    webtag: self.webtag.to_string(),
                    token: token.clone(),
                    sender: tx,
                },
            );

        let request_id_json = serde_json::to_string(&request_id.to_string()).map_err(|err| {
            WebViewScriptError::Platform(format!("Failed to encode eval request id: {err}"))
        })?;
        let token_json = serde_json::to_string(&token).map_err(|err| {
            WebViewScriptError::Platform(format!("Failed to encode eval token: {err}"))
        })?;
        let resolve_expr =
            format!("LingXiaProxy.resolveEval({request_id_json}, {token_json}, __lxR)");
        let body = build_async_eval_body(js, Some(&resolve_expr));
        let parse_guard_script = format!(
            "(function(){{ \
               const id={request_id_json}; const token={token_json}; \
               const timers=window.__LingXiaEvalParseTimers||(window.__LingXiaEvalParseTimers=Object.create(null)); \
               if (timers[id]) clearTimeout(timers[id]); \
               timers[id]=setTimeout(function(){{ \
                 try {{ LingXiaProxy.resolveEval(id, token, JSON.stringify({{ok:false, error:'JavaScript evaluation failed to start; source may contain a syntax error'}})); }} catch(_){{}} \
               }}, {EVAL_PARSE_GUARD_MS}); \
             }})()"
        );
        let clear_parse_guard = format!(
            "try {{ \
               const __lxTimers=window.__LingXiaEvalParseTimers; \
               if (__lxTimers) {{ clearTimeout(__lxTimers[{request_id_json}]); delete __lxTimers[{request_id_json}]; }} \
             }} catch(_){{}}"
        );
        // Defensive outer .catch — `build_async_eval_body` already wraps user
        // code in try/catch, but if the async IIFE itself fails (e.g. a JS
        // engine bug, OOM), still resolve the pending request so callers
        // don't hang until timeout.
        let script = format!(
            "(async () => {{ {clear_parse_guard} {body} }})().catch(e => {{ \
               try {{ LingXiaProxy.resolveEval({request_id_json}, {token_json}, JSON.stringify({{ok:false, error: String(e)}})); }} catch(_){{}} \
             }})"
        );

        {
            let dispatch_result = with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
                let parse_guard_string = env.new_string(&parse_guard_script)?;
                env.call_method(
                    &*self.get_java_webview(),
                    jni_str!("evaluateJavascript"),
                    jni_sig!("(Ljava/lang/String;Landroid/webkit/ValueCallback;)V"),
                    &[(&parse_guard_string).into(), (&JObject::null()).into()],
                )?;
                let script_string = env.new_string(&script)?;
                env.call_method(
                    &*self.get_java_webview(),
                    jni_str!("evaluateJavascript"),
                    jni_sig!("(Ljava/lang/String;Landroid/webkit/ValueCallback;)V"),
                    &[(&script_string).into(), (&JObject::null()).into()],
                )?;
                Ok(())
            });

            if let Err(err) = dispatch_result {
                if let Ok(mut pending) = pending_eval_requests().lock() {
                    pending.remove(&request_id);
                }
                return Err(WebViewScriptError::Platform(format!(
                    "Failed to dispatch Android JavaScript evaluation: {:?}",
                    err
                )));
            }
        }

        match timeout(EVAL_TIMEOUT, rx).await {
            Ok(Ok(PendingEvalResponse::Success(envelope))) => parse_wrapped_eval_result(&envelope),
            Ok(Ok(PendingEvalResponse::Failure(err))) => Err(WebViewScriptError::Platform(err)),
            Ok(Ok(PendingEvalResponse::Destroyed)) => Err(WebViewScriptError::Destroyed),
            Ok(Err(_)) => Err(WebViewScriptError::Destroyed),
            Err(_) => {
                if let Ok(mut pending) = pending_eval_requests().lock() {
                    pending.remove(&request_id);
                }
                Err(WebViewScriptError::Timeout)
            }
        }
    }

    fn clear_browsing_data(&self) -> Result<(), WebViewError> {
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            env.call_method(
                &*self.get_java_webview(),
                jni_str!("clearBrowsingData"),
                jni_sig!("()V"),
                &[],
            )?;
            Ok(())
        })
        .map_err(|e| WebViewError::WebView(format!("Failed to clear browsing data: {:?}", e)))
    }

    fn post_message(&self, message: &str) -> Result<(), WebViewError> {
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            let msg_string = env.new_string(&message)?;

            env.call_method(
                &*self.get_java_webview(),
                jni_str!("postMessageToWebView"),
                jni_sig!("(Ljava/lang/String;)V"),
                &[(&msg_string).into()],
            )?;
            Ok(())
        })
        .map_err(|e| WebViewError::WebView(format!("Failed to post message: {:?}", e)))
    }

    fn set_user_agent_override(&self, user_agent: UserAgentOverride) -> Result<(), WebViewError> {
        with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
            let (use_default, user_agent) = match user_agent {
                UserAgentOverride::Default => (true, String::new()),
                UserAgentOverride::Custom(user_agent) => (false, user_agent),
            };
            let user_agent = env.new_string(user_agent)?;

            env.call_method(
                &*self.get_java_webview(),
                jni_str!("setUserAgentOverride"),
                jni_sig!("(ZLjava/lang/String;)V"),
                &[JValue::Bool(use_default), (&user_agent).into()],
            )?;
            Ok(())
        })
        .map_err(|e| WebViewError::WebView(format!("Failed to override user agent: {:?}", e)))
    }

    async fn take_screenshot(&self) -> Result<Vec<u8>, WebViewError> {
        let request_id = NEXT_SCREENSHOT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();

        pending_screenshot_requests()
            .lock()
            .map_err(|_| {
                WebViewError::WebView("Android pending screenshot map poisoned".to_string())
            })?
            .insert(
                request_id,
                PendingScreenshotEntry {
                    webtag: self.webtag.to_string(),
                    sender: tx,
                },
            );

        {
            let dispatch_result = with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
                env.call_method(
                    &*self.get_java_webview(),
                    jni_str!("captureScreenshot"),
                    jni_sig!("(J)V"),
                    &[(request_id as i64).into()],
                )?;
                Ok(())
            });

            if let Err(err) = dispatch_result {
                if let Ok(mut pending) = pending_screenshot_requests().lock() {
                    pending.remove(&request_id);
                }
                return Err(WebViewError::WebView(format!(
                    "Failed to dispatch Android captureScreenshot: {:?}",
                    err
                )));
            }
        }

        match timeout(SCREENSHOT_TIMEOUT, rx).await {
            Ok(Ok(PendingScreenshotResponse::Success(bytes))) => Ok(bytes),
            Ok(Ok(PendingScreenshotResponse::Failure(err))) => Err(WebViewError::WebView(err)),
            Ok(Ok(PendingScreenshotResponse::Destroyed)) => Err(WebViewError::WebView(
                "WebView was destroyed before screenshot completed".to_string(),
            )),
            Ok(Err(_)) => Err(WebViewError::WebView(
                "screenshot request was canceled".to_string(),
            )),
            Err(_) => {
                if let Ok(mut pending) = pending_screenshot_requests().lock() {
                    pending.remove(&request_id);
                }
                Err(WebViewError::WebView("screenshot timed out".to_string()))
            }
        }
    }
}
