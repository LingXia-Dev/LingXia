//! `PageDriver` — element-level automation of one selected lxapp's pages.
//! All page/DOM semantics come from the shared `lxapp::automation` lower half,
//! so results match the devtool (`lxdev lxapp page …`) exactly.

use crate::auto_err;
use crate::error::{
    E_AUTOMATION_TIMEOUT, E_EVAL_TIMEOUT, E_PAGE_NOT_ACTIVE, E_PAGE_NOT_READY, code_for,
    eval_code_for, page_action_code, page_error, page_error_with,
};
use crate::resolve::{js_object_to_json, json_to_js, upgrade_authorized};
use base64::{Engine as _, engine::general_purpose};
use lxapp::{LxApp, automation as auto};
use rong::{
    Class, FromJSObject, HostError, IntoJSObject, JSContext, JSObject, JSResult, JSValue,
    RongJSError, function::Optional, js_class, js_method,
};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

const DEFAULT_MAX_TEXT: usize = 4096;
const EVAL_DEFAULT_MS: u64 = 5_000;
const WAIT_POLL_MS: u64 = 100;
// Loaded CI runners stall WebView2/WKWebView first render past 10s (observed
// 10.8-11.5s on Windows), so the default page-readiness budget is 30s.
const WAIT_DEFAULT_MS: u64 = 30_000;
const WAIT_MAX_MS: u64 = 60_000;
/// One page eval long-polls a pending action at most this long, well inside
/// every platform's WebView eval cap, so an action may outlast that cap.
const ACTION_POLL_MS: u64 = 2_000;
/// The page runtime's code for "this document has no action metadata yet".
const PAGE_ACTIONS_NOT_READY: &str = "PAGE_ACTIONS_NOT_READY";

fn is_transient_page_error(error: &str) -> bool {
    error.starts_with("page is not active:")
        || error == "page WebView is not ready"
        // The query path surfaces LxAppError's Display form ("WebView error:
        // No current page"), not the bare lowercase string navigate() emits.
        || error.to_ascii_lowercase().contains("no current page")
        || error.to_ascii_lowercase().contains("0x8007139f")
}

#[js_class(clone)]
pub(crate) struct JSPageDriver {
    lxapp: Weak<LxApp>,
    appid: Arc<str>,
}

impl JSPageDriver {
    pub(crate) fn new(lxapp: Weak<LxApp>, appid: Arc<str>) -> Self {
        Self { lxapp, appid }
    }
}

/// Map a lower-half failure on `page` to its coded, page-annotated error.
fn fail<'a>(app: &'a Arc<LxApp>, page: Option<&'a str>) -> impl Fn(String) -> RongJSError + 'a {
    move |message| page_error(app, page, code_for(&message), message)
}

/// [`fail`] for an evaluation, which separates a throwing script and a
/// timeout from the other failures.
fn eval_fail<'a>(
    app: &'a Arc<LxApp>,
    page: Option<&'a str>,
) -> impl Fn(String) -> RongJSError + 'a {
    move |message| page_error(app, page, eval_code_for(&message), message)
}

#[derive(FromJSObject)]
struct JSEvalOptions {
    script: String,
    page: Option<String>,
    #[js_name = "timeoutMs"]
    timeout_ms: Option<u64>,
}

#[derive(FromJSObject)]
struct JSQueryOptions {
    css: String,
    index: Option<usize>,
    all: Option<bool>,
    #[js_name = "maxText"]
    max_text: Option<usize>,
    /// Return untruncated text/value (ignores `maxText`).
    full: Option<bool>,
    page: Option<String>,
}

#[derive(FromJSObject)]
struct JSClickOptions {
    css: String,
    index: Option<usize>,
    page: Option<String>,
    /// Skip the in-viewport and hit-test checks (attached + enabled only).
    force: Option<bool>,
}

#[derive(FromJSObject)]
struct JSTypeOptions {
    css: String,
    text: String,
    index: Option<usize>,
    page: Option<String>,
    /// `fill` only: skip the in-viewport check.
    force: Option<bool>,
}

#[derive(FromJSObject)]
struct JSPressOptions {
    key: String,
    css: Option<String>,
    index: Option<usize>,
    page: Option<String>,
}

#[derive(FromJSObject)]
struct JSScrollToOptions {
    css: String,
    page: Option<String>,
}

#[derive(FromJSObject, Default)]
struct JSScrollOptions {
    dx: Option<f64>,
    dy: Option<f64>,
    page: Option<String>,
}

#[derive(FromJSObject)]
struct JSWaitForOptions {
    css: String,
    state: Option<String>,
    #[js_name = "timeoutMs"]
    timeout_ms: Option<u64>,
    page: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActionOptions {
    page: Option<String>,
    name: String,
    payload: Option<serde_json::Value>,
    timeout_ms: Option<u64>,
}

/// One page eval of an automation action run. With `start`, it invokes the
/// action through the page runtime's `__lxInvokePageAction` and parks its
/// outcome under `token`; either way it then waits up to `wait_ms` for that
/// outcome. Answers `{state: "not-ready" | "pending" | "lost" | "settled", …}`.
fn action_script(
    token: &str,
    start: Option<(&str, Option<&serde_json::Value>)>,
    wait_ms: u64,
) -> String {
    let token = serde_json::to_string(token).expect("token serializes");
    let start = match start {
        Some((name, payload)) => {
            let name = serde_json::to_string(name).expect("name serializes");
            let payload = payload
                .map(|value| value.to_string())
                .unwrap_or_else(|| "undefined".to_string());
            format!(
                r#"{{
  const invoke = window.__lxInvokePageAction;
  if (typeof invoke !== "function") return {{ state: "not-ready" }};
  if (!window.__lxAutomationActionRuns) {{
    Object.defineProperty(window, "__lxAutomationActionRuns", {{ value: new Map(), configurable: true }});
  }}
  const run = {{}};
  const fail = (error) => ({{
    state: "settled", ok: false,
    error: {{
      code: error && typeof error.code === "string" ? error.code : "",
      message: error && error.message !== undefined ? String(error.message) : String(error),
      data: error && error.data !== undefined ? error.data : null,
    }},
  }});
  run.settled = Promise.resolve()
    .then(() => invoke({name}, {payload}))
    .then(
      (value) => {{ run.result = {{ state: "settled", ok: true, value: value === undefined ? null : value }}; }},
      (error) => {{ run.result = fail(error); }},
    );
  window.__lxAutomationActionRuns.set({token}, run);
  }}"#
            )
        }
        None => String::new(),
    };
    format!(
        r#"(async () => {{
  {start}
  const runs = window.__lxAutomationActionRuns;
  const run = runs && runs.get({token});
  if (!run) return {{ state: "lost" }};
  if (!run.result) await Promise.race([run.settled, new Promise((resolve) => setTimeout(resolve, {wait_ms}))]);
  if (!run.result) return {{ state: "pending" }};
  runs.delete({token});
  return run.result;
}})()"#
    )
}

fn next_action_token() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("action-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

#[derive(FromJSObject, Default)]
struct JSScreenshotOptions {
    page: Option<String>,
}

/// The fields `waitFor` reads off the shared query payload.
#[derive(serde::Deserialize)]
struct WaitProbe {
    exists: bool,
    #[serde(default)]
    visible: bool,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    editable: bool,
}

/// Raw `waitFor` states, evaluated on the first match (the `lxdev lxapp page
/// wait` contract). `visible` means rendered, wherever the match is scrolled.
/// `hidden` needs an existing, non-rendered match, so "no match"
/// satisfies only `detached`. `@lingxia/test` locators layer stricter,
/// uniqueness-aware states on top and treat "no match" as hidden.
fn wait_state_satisfied(state: &str, probe: &WaitProbe) -> bool {
    match state {
        "attached" => probe.exists,
        "detached" => !probe.exists,
        "visible" => probe.exists && probe.visible,
        "hidden" => probe.exists && !probe.visible,
        "enabled" => probe.exists && probe.enabled,
        "editable" => probe.exists && probe.editable,
        _ => false,
    }
}

#[derive(Debug, Clone, IntoJSObject)]
struct JSScreenshot {
    format: String,
    base64: String,
    width: u32,
    height: u32,
}

#[js_class(rename = "PageDriver")]
impl JSPageDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(rong::error::E_ILLEGAL_CONSTRUCTOR, "Use lx.automation()").into())
    }

    /// Evaluate JavaScript in the page WebView.
    #[js_method]
    async fn eval(&self, ctx: JSContext, options: JSEvalOptions) -> JSResult<JSValue> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let timeout = Duration::from_millis(options.timeout_ms.unwrap_or(EVAL_DEFAULT_MS));
        let value = tokio::time::timeout(
            timeout,
            auto::page_eval(&app, options.page.as_deref(), &options.script),
        )
        .await
        .map_err(|_| {
            page_error(
                &app,
                options.page.as_deref(),
                E_EVAL_TIMEOUT,
                "page eval timed out",
            )
        })?
        .map_err(eval_fail(&app, options.page.as_deref()))?;
        json_to_js(&ctx, &value)
    }

    /// Query element information with JavaScript field names.
    #[js_method]
    async fn query(&self, ctx: JSContext, options: JSQueryOptions) -> JSResult<JSValue> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let all = options.all.unwrap_or(false);
        if all && options.index.is_some() {
            return Err(auto_err("pass either all or index, not both"));
        }
        let max_text = if options.full.unwrap_or(false) {
            None
        } else {
            Some(options.max_text.unwrap_or(DEFAULT_MAX_TEXT))
        };
        let value = auto::page_query(
            &app,
            options.page.as_deref(),
            &options.css,
            options.index,
            all,
            max_text,
        )
        .await
        .map_err(fail(&app, options.page.as_deref()))?;
        json_to_js(&ctx, &crate::js_payload::page_query(value))
    }

    #[js_method]
    async fn click(&self, ctx: JSContext, options: JSClickOptions) -> JSResult<()> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        auto::page_click_with(
            &app,
            options.page.as_deref(),
            &options.css,
            options.index,
            options.force.unwrap_or(false),
        )
        .await
        .map_err(fail(&app, options.page.as_deref()))
    }

    #[js_method(rename = "type")]
    async fn type_text(&self, ctx: JSContext, options: JSTypeOptions) -> JSResult<()> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        auto::page_type(
            &app,
            options.page.as_deref(),
            &options.css,
            options.index,
            &options.text,
        )
        .await
        .map_err(fail(&app, options.page.as_deref()))
    }

    #[js_method]
    async fn fill(&self, ctx: JSContext, options: JSTypeOptions) -> JSResult<()> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        auto::page_fill_with(
            &app,
            options.page.as_deref(),
            &options.css,
            options.index,
            &options.text,
            options.force.unwrap_or(false),
        )
        .await
        .map_err(fail(&app, options.page.as_deref()))
    }

    #[js_method]
    async fn press(&self, ctx: JSContext, options: JSPressOptions) -> JSResult<()> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        auto::page_press(
            &app,
            options.page.as_deref(),
            &options.key,
            options.css.as_deref(),
            options.index,
        )
        .await
        .map_err(fail(&app, options.page.as_deref()))
    }

    #[js_method(rename = "scrollTo")]
    async fn scroll_to(&self, ctx: JSContext, options: JSScrollToOptions) -> JSResult<()> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        auto::page_scroll_to(&app, options.page.as_deref(), &options.css)
            .await
            .map_err(fail(&app, options.page.as_deref()))
    }

    #[js_method]
    async fn scroll(&self, ctx: JSContext, options: Optional<JSScrollOptions>) -> JSResult<()> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let options = options.0.unwrap_or_default();
        auto::page_scroll(
            &app,
            options.page.as_deref(),
            options.dx.unwrap_or(0.0),
            options.dy.unwrap_or(0.0),
        )
        .await
        .map_err(fail(&app, options.page.as_deref()))
    }

    /// App-window pointer input at page coordinates (`lxdev lxapp page pointer`).
    #[js_method(getter, enumerable)]
    fn pointer(&self, ctx: JSContext) -> JSResult<JSObject> {
        Ok(Class::lookup::<crate::input::JSPagePointer>(&ctx)?
            .instance(crate::input::JSPagePointer::new(self.appid.clone())))
    }

    /// App-window keyboard input (`lxdev lxapp page key`).
    #[js_method(getter, enumerable)]
    fn key(&self, ctx: JSContext) -> JSResult<JSObject> {
        Ok(
            Class::lookup::<crate::input::JSPageKey>(&ctx)?
                .instance(crate::input::JSPageKey::new()),
        )
    }

    #[js_method(rename = "waitFor")]
    async fn wait_for(&self, ctx: JSContext, options: JSWaitForOptions) -> JSResult<()> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let state = options.state.as_deref().unwrap_or("visible");
        if !matches!(
            state,
            "attached" | "detached" | "visible" | "hidden" | "enabled" | "editable"
        ) {
            return Err(auto_err(format!("waitFor: unknown state '{state}'")));
        }
        // Reject a page name that isn't in the config up front, so a typo'd
        // `page` can't satisfy `detached` below.
        if !auto::page_name_known(&app, options.page.as_deref()) {
            return Err(auto_err(auto::unknown_page_name(
                &app,
                options.page.as_deref().unwrap_or_default(),
            )));
        }
        let timeout = Duration::from_millis(
            options
                .timeout_ms
                .unwrap_or(WAIT_DEFAULT_MS)
                .min(WAIT_MAX_MS),
        );
        let started = Instant::now();
        loop {
            // Navigation returns before the destination WebView is attached.
            // Treat that transient absence like an unsatisfied selector so a
            // targeted wait can also be the page-readiness barrier.
            let probe = match auto::page_query(
                &app,
                options.page.as_deref(),
                &options.css,
                None,
                false,
                Some(0),
            )
            .await
            {
                Ok(value) => serde_json::from_value::<WaitProbe>(value)
                    .map_err(|err| auto_err(format!("waitFor decode: {err}")))?,
                Err(err) if is_transient_page_error(&err) => {
                    if state == "detached" {
                        return Ok(());
                    }
                    if started.elapsed() >= timeout {
                        return Err(fail(&app, options.page.as_deref())(format!(
                            "E_TIMEOUT: waitFor '{}' ({state}): {}",
                            options.css, err
                        )));
                    }
                    tokio::time::sleep(Duration::from_millis(WAIT_POLL_MS)).await;
                    continue;
                }
                Err(err) => return Err(fail(&app, options.page.as_deref())(err)),
            };
            let satisfied = wait_state_satisfied(state, &probe);
            if satisfied {
                return Ok(());
            }
            if started.elapsed() >= timeout {
                return Err(fail(&app, options.page.as_deref())(format!(
                    "E_TIMEOUT: waitFor '{}' ({state})",
                    options.css
                )));
            }
            tokio::time::sleep(Duration::from_millis(WAIT_POLL_MS)).await;
        }
    }

    /// Invoke a unary page action by name, as the View would, and resolve
    /// to its result. The action's own rejection keeps its code and message.
    #[js_method]
    async fn action(&self, ctx: JSContext, options: JSObject) -> JSResult<JSValue> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let options: ActionOptions = serde_json::from_value(js_object_to_json(&options)?)
            .map_err(|err| auto_err(format!("action: invalid options: {err}")))?;
        let page = options.page.as_deref();
        let name = options.name.as_str();
        if name.trim().is_empty() {
            return Err(auto_err("action: name must be a non-empty string"));
        }
        let timeout = Duration::from_millis(options.timeout_ms.unwrap_or(EVAL_DEFAULT_MS));
        let started = Instant::now();
        let remaining = || timeout.saturating_sub(started.elapsed());
        let poll_ms = || (remaining().as_millis() as u64).clamp(1, ACTION_POLL_MS);
        let token = next_action_token();
        let mut started_run = false;
        loop {
            let script = if started_run {
                action_script(&token, None, poll_ms())
            } else {
                action_script(&token, Some((name, options.payload.as_ref())), poll_ms())
            };
            let answer = match auto::page_eval(&app, page, &script).await {
                Ok(answer) => answer,
                // The page is still attaching: like waitFor, readiness is part of the budget.
                Err(err) if !started_run && is_transient_page_error(&err) => {
                    if remaining().is_zero() {
                        return Err(page_error(
                            &app,
                            page,
                            E_PAGE_NOT_READY,
                            format!("page action '{name}' could not start: {err}"),
                        ));
                    }
                    tokio::time::sleep(Duration::from_millis(WAIT_POLL_MS).min(remaining())).await;
                    continue;
                }
                Err(err) => return Err(eval_fail(&app, page)(err)),
            };
            let state = answer.get("state").and_then(|state| state.as_str());
            let not_ready = state == Some("not-ready")
                || (state == Some("settled")
                    && answer.pointer("/error/code").and_then(|code| code.as_str())
                        == Some(PAGE_ACTIONS_NOT_READY));
            match state {
                _ if not_ready => {
                    started_run = false;
                    if remaining().is_zero() {
                        return Err(page_error(
                            &app,
                            page,
                            E_PAGE_NOT_READY,
                            format!(
                                "page action '{name}' could not start: the page's LingXia runtime \
                             and action metadata are not loaded"
                            ),
                        ));
                    }
                    tokio::time::sleep(Duration::from_millis(WAIT_POLL_MS).min(remaining())).await;
                }
                Some("pending") => {
                    started_run = true;
                    if remaining().is_zero() {
                        let _ = auto::page_eval(
                            &app,
                            page,
                            &format!(
                                "void window.__lxAutomationActionRuns?.delete({})",
                                serde_json::to_string(&token).expect("token serializes")
                            ),
                        )
                        .await;
                        return Err(page_error(
                            &app,
                            page,
                            E_AUTOMATION_TIMEOUT,
                            format!(
                                "page action '{name}' did not settle within {}ms",
                                timeout.as_millis()
                            ),
                        ));
                    }
                }
                Some("lost") => {
                    return Err(page_error(
                        &app,
                        page,
                        E_PAGE_NOT_ACTIVE,
                        format!(
                            "page action '{name}' was interrupted: the page document was replaced"
                        ),
                    ));
                }
                Some("settled") if answer.get("ok").and_then(|ok| ok.as_bool()) == Some(true) => {
                    return json_to_js(
                        &ctx,
                        answer.get("value").unwrap_or(&serde_json::Value::Null),
                    );
                }
                Some("settled") => {
                    let error = answer.get("error").cloned().unwrap_or_default();
                    let code = error
                        .get("code")
                        .and_then(|code| code.as_str())
                        .unwrap_or("");
                    let message = error
                        .get("message")
                        .and_then(|message| message.as_str())
                        .filter(|message| !message.is_empty())
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("page action '{name}' failed"));
                    let mut extra = serde_json::Map::new();
                    extra.insert("action".into(), name.into());
                    extra.insert("cause".into(), error.clone());
                    return Err(page_error_with(
                        &app,
                        page,
                        page_action_code(code),
                        message,
                        extra,
                    ));
                }
                _ => {
                    return Err(auto_err(format!(
                        "page action '{name}': unexpected page answer {answer}"
                    )));
                }
            }
        }
    }

    #[js_method]
    async fn screenshot(
        &self,
        ctx: JSContext,
        options: Optional<JSScreenshotOptions>,
    ) -> JSResult<JSScreenshot> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let options = options.0.unwrap_or_default();
        let bytes = auto::page_screenshot(&app, options.page.as_deref())
            .await
            .map_err(fail(&app, options.page.as_deref()))?;
        let (width, height) = png_dimensions(&bytes).unwrap_or((0, 0));
        Ok(JSScreenshot {
            format: "png".to_string(),
            base64: general_purpose::STANDARD.encode(&bytes),
            width,
            height,
        })
    }
}

/// Read width/height from a PNG's IHDR chunk (bytes 16..24, big-endian).
pub(crate) fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::{WaitProbe, is_transient_page_error, wait_state_satisfied};

    #[test]
    fn wait_retries_only_page_readiness_errors() {
        assert!(is_transient_page_error("page is not active: todo"));
        assert!(is_transient_page_error("page WebView is not ready"));
        assert!(is_transient_page_error(
            "The group or resource is not in the correct state (0x8007139F)"
        ));
        assert!(!is_transient_page_error("SyntaxError: invalid selector"));
    }

    #[test]
    fn wait_states_match_the_devtool_element_contract() {
        let visible = WaitProbe {
            exists: true,
            visible: true,
            enabled: true,
            editable: false,
        };
        assert!(wait_state_satisfied("attached", &visible));
        assert!(!wait_state_satisfied("exists", &visible), "not a state");
        assert!(wait_state_satisfied("visible", &visible));
        assert!(wait_state_satisfied("enabled", &visible));
        assert!(!wait_state_satisfied("editable", &visible));
        assert!(!wait_state_satisfied("hidden", &visible));

        let hidden = WaitProbe {
            exists: true,
            visible: false,
            enabled: false,
            editable: false,
        };
        assert!(wait_state_satisfied("hidden", &hidden));
        assert!(!wait_state_satisfied("detached", &hidden));

        let missing = WaitProbe {
            exists: false,
            visible: false,
            enabled: false,
            editable: false,
        };
        assert!(wait_state_satisfied("detached", &missing));
        assert!(!wait_state_satisfied("gone", &missing), "not a state");
        assert!(!wait_state_satisfied("hidden", &missing));
    }
}

#[cfg(all(test, feature = "runtime"))]
mod action_script_tests {
    use super::action_script;
    use rong::{JSResult, Rong, RongJS, Source};
    use serde_json::{Value, json};

    /// Runs `setup`, then each script in order against one fake page whose
    /// `window` is the global, and returns every answer.
    fn run(setup: &str, scripts: Vec<String>) -> Vec<Value> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let setup = format!("globalThis.window = globalThis; {setup}");
        let answers = rt.block_on(async move {
            let pool = Rong::<RongJS>::builder()
                .shared()
                .workers(1)
                .build()
                .unwrap();
            let worker = pool.worker(0).unwrap();
            let handle = worker
                .spawn(
                    async move |js_runtime, _receiver| -> JSResult<Vec<String>> {
                        let ctx = js_runtime.context();
                        rong_modules::init(&ctx, ["timer"])?;
                        ctx.eval::<()>(Source::from_bytes(setup))?;
                        let mut answers = Vec::new();
                        for script in scripts {
                            let script =
                                format!("({script}).then((value) => JSON.stringify(value))");
                            answers
                                .push(ctx.eval_async::<String>(Source::from_bytes(script)).await?);
                        }
                        Ok(answers)
                    },
                )
                .await
                .unwrap();
            handle.join().await.unwrap()
        });
        answers
            .iter()
            .map(|answer| serde_json::from_str(answer).unwrap())
            .collect()
    }

    #[test]
    fn a_missing_runtime_is_not_ready() {
        let answers = run("", vec![action_script("t", Some(("save", None)), 5)]);
        assert_eq!(answers, vec![json!({ "state": "not-ready" })]);
    }

    #[test]
    fn a_slow_action_is_polled_until_it_settles() {
        let payload = json!({ "title": "Draft" });
        let answers = run(
            "window.__lxInvokePageAction = (name, payload) => \
               new Promise((resolve) => { globalThis.finish = () => resolve({ name, payload }); });",
            vec![
                action_script("t", Some(("save", Some(&payload))), 5),
                action_script("t", None, 5),
                "Promise.resolve((globalThis.finish(), true))".to_string(),
                action_script("t", None, 1000),
                action_script("t", None, 5),
            ],
        );
        assert_eq!(answers[0], json!({ "state": "pending" }));
        assert_eq!(answers[1], json!({ "state": "pending" }));
        assert_eq!(
            answers[3],
            json!({ "state": "settled", "ok": true, "value": { "name": "save", "payload": payload } })
        );
        assert_eq!(
            answers[4],
            json!({ "state": "lost" }),
            "a read outcome is released"
        );
    }

    #[test]
    fn a_rejection_keeps_its_code_message_and_data() {
        let answers = run(
            "window.__lxInvokePageAction = () => \
               Promise.reject({ code: 'E_QUOTA', message: 'over quota', data: { limit: 3 } });",
            vec![action_script("t", Some(("save", None)), 1000)],
        );
        assert_eq!(
            answers[0],
            json!({
                "state": "settled",
                "ok": false,
                "error": { "code": "E_QUOTA", "message": "over quota", "data": { "limit": 3 } },
            })
        );
    }
}
