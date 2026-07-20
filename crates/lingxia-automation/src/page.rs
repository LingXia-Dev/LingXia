//! `PageDriver` — element-level automation of one selected lxapp's pages.
//! All page/DOM semantics come from the shared `lxapp::automation` lower half,
//! so results match the devtool (`lxdev lxapp page …`) exactly.

use crate::auto_err;
use crate::resolve::{json_to_js, upgrade};
use base64::{Engine as _, engine::general_purpose};
use lxapp::{LxApp, automation as auto};
use rong::{
    Class, FromJSObject, HostError, IntoJSObject, JSContext, JSObject, JSResult, JSValue,
    function::Optional, js_class, js_method,
};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

const DEFAULT_MAX_TEXT: usize = 4096;
const EVAL_DEFAULT_MS: u64 = 5_000;
const WAIT_POLL_MS: u64 = 100;
const WAIT_DEFAULT_MS: u64 = 10_000;
const WAIT_MAX_MS: u64 = 60_000;

fn is_transient_page_error(error: &str) -> bool {
    error.starts_with("page is not active:")
        || error == "page WebView is not ready"
        || error == "no current page"
        || error.to_ascii_lowercase().contains("0x8007139f")
}

#[js_class(clone)]
pub(crate) struct JSPageDriver {
    lxapp: Weak<LxApp>,
}

impl JSPageDriver {
    pub(crate) fn new(lxapp: &Arc<LxApp>) -> Self {
        Self {
            lxapp: Arc::downgrade(lxapp),
        }
    }
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
}

#[derive(FromJSObject)]
struct JSTypeOptions {
    css: String,
    text: String,
    index: Option<usize>,
    page: Option<String>,
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

#[derive(FromJSObject, Default)]
struct JSScreenshotOptions {
    page: Option<String>,
}

/// The two fields `waitFor` reads off the shared query payload.
#[derive(serde::Deserialize)]
struct WaitProbe {
    exists: bool,
    #[serde(default)]
    visible: bool,
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
        let app = upgrade(&self.lxapp)?;
        let timeout = Duration::from_millis(options.timeout_ms.unwrap_or(EVAL_DEFAULT_MS));
        let value = tokio::time::timeout(
            timeout,
            auto::page_eval(&app, options.page.as_deref(), &options.script),
        )
        .await
        .map_err(|_| auto_err("page eval timed out"))?
        .map_err(auto_err)?;
        json_to_js(&ctx, &value)
    }

    /// Query element information. Same payload shape as `lxdev lxapp page query`.
    #[js_method]
    async fn query(&self, ctx: JSContext, options: JSQueryOptions) -> JSResult<JSValue> {
        let app = upgrade(&self.lxapp)?;
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
        .map_err(auto_err)?;
        json_to_js(&ctx, &value)
    }

    #[js_method]
    async fn click(&self, _ctx: JSContext, options: JSClickOptions) -> JSResult<()> {
        let app = upgrade(&self.lxapp)?;
        auto::page_click(&app, options.page.as_deref(), &options.css, options.index)
            .await
            .map_err(auto_err)
    }

    #[js_method(rename = "type")]
    async fn type_text(&self, _ctx: JSContext, options: JSTypeOptions) -> JSResult<()> {
        let app = upgrade(&self.lxapp)?;
        auto::page_type(
            &app,
            options.page.as_deref(),
            &options.css,
            options.index,
            &options.text,
        )
        .await
        .map_err(auto_err)
    }

    #[js_method]
    async fn fill(&self, _ctx: JSContext, options: JSTypeOptions) -> JSResult<()> {
        let app = upgrade(&self.lxapp)?;
        auto::page_fill(
            &app,
            options.page.as_deref(),
            &options.css,
            options.index,
            &options.text,
        )
        .await
        .map_err(auto_err)
    }

    #[js_method]
    async fn press(&self, _ctx: JSContext, options: JSPressOptions) -> JSResult<()> {
        let app = upgrade(&self.lxapp)?;
        auto::page_press(
            &app,
            options.page.as_deref(),
            &options.key,
            options.css.as_deref(),
            options.index,
        )
        .await
        .map_err(auto_err)
    }

    #[js_method(rename = "scrollTo")]
    async fn scroll_to(&self, _ctx: JSContext, options: JSScrollToOptions) -> JSResult<()> {
        let app = upgrade(&self.lxapp)?;
        auto::page_scroll_to(&app, options.page.as_deref(), &options.css)
            .await
            .map_err(auto_err)
    }

    #[js_method]
    async fn scroll(&self, _ctx: JSContext, options: Optional<JSScrollOptions>) -> JSResult<()> {
        let app = upgrade(&self.lxapp)?;
        let options = options.0.unwrap_or_default();
        auto::page_scroll(
            &app,
            options.page.as_deref(),
            options.dx.unwrap_or(0.0),
            options.dy.unwrap_or(0.0),
        )
        .await
        .map_err(auto_err)
    }

    /// App-window pointer input at page coordinates (`lxdev lxapp page pointer`).
    #[js_method(getter, enumerable)]
    fn pointer(&self, ctx: JSContext) -> JSResult<JSObject> {
        Ok(Class::lookup::<crate::input::JSPagePointer>(&ctx)?
            .instance(crate::input::JSPagePointer::new()))
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
    async fn wait_for(&self, _ctx: JSContext, options: JSWaitForOptions) -> JSResult<()> {
        let app = upgrade(&self.lxapp)?;
        let state = options.state.as_deref().unwrap_or("visible");
        if !matches!(state, "exists" | "visible" | "gone") {
            return Err(auto_err(format!("waitFor: unknown state '{state}'")));
        }
        // Reject a page name that isn't in the config up front, so a typo'd
        // `page` can't satisfy `gone` below.
        if !auto::page_name_known(&app, options.page.as_deref()) {
            return Err(auto_err(format!(
                "unknown page name: {}",
                options.page.as_deref().unwrap_or_default()
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
                    if state == "gone" {
                        return Ok(());
                    }
                    if started.elapsed() >= timeout {
                        return Err(auto_err(format!(
                            "E_TIMEOUT: waitFor '{}' ({state}): {}",
                            options.css, err
                        )));
                    }
                    tokio::time::sleep(Duration::from_millis(WAIT_POLL_MS)).await;
                    continue;
                }
                Err(err) => return Err(auto_err(err)),
            };
            let satisfied = match state {
                "exists" => probe.exists,
                "gone" => !probe.exists,
                _ => probe.exists && probe.visible,
            };
            if satisfied {
                return Ok(());
            }
            if started.elapsed() >= timeout {
                return Err(auto_err(format!(
                    "E_TIMEOUT: waitFor '{}' ({state})",
                    options.css
                )));
            }
            tokio::time::sleep(Duration::from_millis(WAIT_POLL_MS)).await;
        }
    }

    #[js_method]
    async fn screenshot(
        &self,
        _ctx: JSContext,
        options: Optional<JSScreenshotOptions>,
    ) -> JSResult<JSScreenshot> {
        let app = upgrade(&self.lxapp)?;
        let options = options.0.unwrap_or_default();
        let bytes = auto::page_screenshot(&app, options.page.as_deref())
            .await
            .map_err(auto_err)?;
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
    use super::is_transient_page_error;

    #[test]
    fn wait_retries_only_page_readiness_errors() {
        assert!(is_transient_page_error("page is not active: todo"));
        assert!(is_transient_page_error("page WebView is not ready"));
        assert!(is_transient_page_error(
            "The group or resource is not in the correct state (0x8007139F)"
        ));
        assert!(!is_transient_page_error("SyntaxError: invalid selector"));
    }
}
