//! `page.gesture` — real pointer gestures (`lxdev host gesture`): timed paths
//! injected into the host window, so page chrome handles them as a user's.
//! Never a DOM or core shortcut.

use crate::error::{E_UNSUPPORTED, coded};
use crate::resolve::{js_object_to_json, json_to_js, upgrade_authorized};
use crate::{auto_err, require_host_context};
use lingxia_platform::error::PlatformError;
use lingxia_platform::traits::gesture::{
    AppGesture, AppGestureRequest, FlingDirection, GestureEnd, GestureInput, GestureShape,
    GestureTarget,
};
use lxapp::{LxApp, automation as auto};
use rong::{
    JSContext, JSObject, JSResult, JSValue, RongJSError, function::Optional, js_class, js_method,
};
use serde::Deserialize;
use std::sync::{Arc, Weak};

fn illegal_ctor() -> RongJSError {
    rong::HostError::new(rong::error::E_ILLEGAL_CONSTRUCTOR, "Use lx.automation()").into()
}

fn platform_err(err: PlatformError) -> RongJSError {
    match err {
        PlatformError::NotSupported(msg) => coded(E_UNSUPPORTED, msg).into(),
        other => auto_err(other.to_string()),
    }
}

/// The JS request: a shape plus where and how it ends.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GestureOptions {
    shape: ShapeOptions,
    #[serde(default)]
    end: GestureEnd,
    #[serde(default)]
    finger: Option<u64>,
    #[serde(default)]
    input: Option<GestureInput>,
    /// A page name or instance id; absent means the current page.
    #[serde(default)]
    page: Option<String>,
    /// Window points on the host window instead of a page (`true` for the
    /// default window).
    #[serde(default)]
    window: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum ShapeOptions {
    #[serde(rename_all = "camelCase")]
    Line {
        from: [f64; 2],
        to: [f64; 2],
        duration_ms: f64,
        #[serde(default)]
        steps: Option<u32>,
        #[serde(default)]
        hold_ms: f64,
    },
    #[serde(rename_all = "camelCase")]
    Fling {
        from: [f64; 2],
        direction: FlingDirection,
        velocity: f64,
    },
    #[serde(rename_all = "camelCase")]
    Hold { at: [f64; 2], duration_ms: f64 },
}

impl From<ShapeOptions> for GestureShape {
    fn from(shape: ShapeOptions) -> Self {
        match shape {
            ShapeOptions::Line {
                from,
                to,
                duration_ms,
                steps,
                hold_ms,
            } => GestureShape::Line {
                from,
                to,
                duration_ms,
                steps,
                hold_ms,
            },
            ShapeOptions::Fling {
                from,
                direction,
                velocity,
            } => GestureShape::Fling {
                from,
                direction,
                velocity,
            },
            ShapeOptions::Hold { at, duration_ms } => GestureShape::Hold { at, duration_ms },
        }
    }
}

#[derive(Deserialize, Default)]
struct PageOptions {
    #[serde(default)]
    page: Option<String>,
}

#[derive(Deserialize)]
struct OrientOptions {
    orientation: String,
}

#[derive(Deserialize, Default)]
struct ChromeOptions {
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Deserialize, Default)]
struct ViewportOptions {
    #[serde(default)]
    page: Option<String>,
    #[serde(default)]
    window: Option<serde_json::Value>,
}

#[derive(Deserialize, Default)]
struct EdgeBackOptions {
    #[serde(default)]
    edge: Option<FlingDirection>,
    #[serde(default)]
    window: Option<String>,
}

fn parse<T: serde::de::DeserializeOwned>(object: Option<JSObject>, api: &str) -> JSResult<T> {
    let value = match object {
        Some(object) => js_object_to_json(&object)?,
        None => serde_json::Value::Object(Default::default()),
    };
    serde_json::from_value(value).map_err(|e| auto_err(format!("{api}: {e}")))
}

fn page_target(app: &Arc<LxApp>, page: Option<&str>) -> JSResult<GestureTarget> {
    auto::gesture_target(app, page).map_err(auto_err)
}

fn platform() -> JSResult<Arc<lingxia_platform::Platform>> {
    lxapp::get_platform().ok_or_else(|| auto_err("platform is not initialized"))
}

/// A WebView app has no page input where the host reaches GPU pages only.
fn app_inputs(
    inputs: Vec<GestureInput>,
    webview_page_gestures: bool,
    gpu_app: bool,
) -> Vec<GestureInput> {
    if gpu_app || webview_page_gestures {
        inputs
    } else {
        Vec::new()
    }
}

#[js_class(clone)]
pub(crate) struct JSPageGesture {
    lxapp: Weak<LxApp>,
}

impl JSPageGesture {
    pub(crate) fn new(lxapp: Weak<LxApp>) -> Self {
        Self { lxapp }
    }
}

#[js_class(rename = "PageGesture")]
impl JSPageGesture {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    /// Inject one gesture segment and resolve to `{input, delivered, events,
    /// durationMs, finger?}`.
    #[js_method]
    async fn perform(&self, ctx: JSContext, options: JSObject) -> JSResult<JSValue> {
        require_host_context(&ctx)?;
        let options: GestureOptions = parse(Some(options), "gesture.perform")?;
        let target = match options.window {
            Some(serde_json::Value::String(id)) => GestureTarget::Window {
                window_id: Some(id),
            },
            Some(serde_json::Value::Bool(true)) => GestureTarget::Window { window_id: None },
            Some(serde_json::Value::Null) | Some(serde_json::Value::Bool(false)) | None => {
                let app = upgrade_authorized(&ctx, &self.lxapp)?;
                page_target(&app, options.page.as_deref())?
            }
            Some(other) => {
                return Err(auto_err(format!(
                    "window: expected a window id or true, got {other}"
                )));
            }
        };
        let request = AppGestureRequest {
            target,
            input: options.input,
            finger: options.finger,
            shape: options.shape.into(),
            end: options.end,
        };
        let result = platform()?
            .perform_app_gesture(request)
            .await
            .map_err(platform_err)?;
        let json = serde_json::json!({
            "input": result.input,
            "delivered": result.delivered,
            "events": result.events,
            "durationMs": result.duration_ms,
            "finger": result.finger,
        });
        json_to_js(&ctx, &json)
    }

    /// The on-screen part of the page (or `window`) as `{left, top, width, height}`.
    #[js_method]
    async fn viewport(&self, ctx: JSContext, options: Optional<JSObject>) -> JSResult<JSValue> {
        require_host_context(&ctx)?;
        let options: ViewportOptions = parse(options.0, "gesture.viewport")?;
        let target = match options.window {
            Some(serde_json::Value::String(id)) => GestureTarget::Window {
                window_id: Some(id),
            },
            Some(serde_json::Value::Bool(true)) => GestureTarget::Window { window_id: None },
            _ => {
                let app = upgrade_authorized(&ctx, &self.lxapp)?;
                page_target(&app, options.page.as_deref())?
            }
        };
        let [left, top, width, height] = platform()?
            .gesture_viewport(target)
            .await
            .map_err(platform_err)?;
        json_to_js(
            &ctx,
            &serde_json::json!({ "left": left, "top": top, "width": width, "height": height }),
        )
    }

    /// Turn the host window to `orientation` (`landscape` | `portrait`) as a
    /// desktop user resizes it; resolves to its new `{width, height}`.
    #[js_method(rename = "orientWindow")]
    async fn orient_window(&self, ctx: JSContext, options: JSObject) -> JSResult<JSValue> {
        require_host_context(&ctx)?;
        let options: OrientOptions = parse(Some(options), "gesture.orientWindow")?;
        let landscape = match options.orientation.as_str() {
            "landscape" => true,
            "portrait" => false,
            other => {
                return Err(auto_err(format!(
                    "orientation must be landscape or portrait, not {other}"
                )));
            }
        };
        let [width, height] = platform()?
            .orient_window(landscape)
            .await
            .map_err(platform_err)?;
        json_to_js(
            &ctx,
            &serde_json::json!({ "width": width, "height": height }),
        )
    }

    /// Pressable host chrome `[{label, role, scope, selected, bounds}]`, from
    /// the host's own view tree; `bounds` are window points.
    #[js_method]
    async fn chrome(&self, ctx: JSContext, options: Optional<JSObject>) -> JSResult<JSValue> {
        require_host_context(&ctx)?;
        let options: ChromeOptions = parse(options.0, "gesture.chrome")?;
        let elements = platform()?
            .chrome_elements(options.scope)
            .await
            .map_err(platform_err)?;
        json_to_js(&ctx, &serde_json::json!(elements))
    }

    /// The devices this host can imitate (`touch`, `mouse`, `trackpad`) on
    /// this app's pages.
    #[js_method]
    fn inputs(&self, ctx: JSContext) -> JSResult<JSValue> {
        let Some(platform) = lxapp::get_platform() else {
            return json_to_js(&ctx, &serde_json::json!([]));
        };
        let gpu_app = self
            .lxapp
            .upgrade()
            .is_none_or(|app| app.renderer() == lxapp::PageRenderer::Gpu);
        let inputs = app_inputs(
            platform.gesture_inputs(),
            platform.webview_page_gestures(),
            gpu_app,
        );
        json_to_js(&ctx, &serde_json::json!(inputs))
    }

    /// Host automation beyond page gestures (`chrome`, `refreshIndicator`, `hostInput`).
    #[js_method]
    fn capabilities(&self, ctx: JSContext) -> JSResult<JSValue> {
        let capabilities = lxapp::get_platform()
            .map(|platform| platform.host_capabilities())
            .unwrap_or_default();
        json_to_js(&ctx, &serde_json::json!(capabilities))
    }

    /// The page's pull-to-refresh indicator `{phase, offset}`; rejects with
    /// `E_UNSUPPORTED` when the page has none.
    #[js_method(rename = "refreshIndicator")]
    async fn refresh_indicator(
        &self,
        ctx: JSContext,
        options: Optional<JSObject>,
    ) -> JSResult<JSValue> {
        require_host_context(&ctx)?;
        let options: PageOptions = parse(options.0, "gesture.refreshIndicator")?;
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let GestureTarget::Page { app_id, webtag, .. } =
            page_target(&app, options.page.as_deref())?
        else {
            unreachable!("page_target returns a page");
        };
        let state = platform()?
            .pull_refresh_indicator(&app_id, &webtag)
            .await
            .map_err(platform_err)?
            .ok_or_else(|| {
                coded(
                    E_UNSUPPORTED,
                    "this page has no pull-to-refresh indicator (enablePullDownRefresh)",
                )
            })?;
        json_to_js(
            &ctx,
            &serde_json::json!({ "phase": state.phase, "offset": state.offset }),
        )
    }

    /// A system back swipe from the window's left (default) or right edge.
    #[js_method(rename = "edgeBack")]
    async fn edge_back(&self, ctx: JSContext, options: Optional<JSObject>) -> JSResult<JSValue> {
        require_host_context(&ctx)?;
        let options: EdgeBackOptions = parse(options.0, "gesture.edgeBack")?;
        let edge = options.edge.unwrap_or(FlingDirection::Left);
        if !matches!(edge, FlingDirection::Left | FlingDirection::Right) {
            return Err(auto_err("edgeBack: edge must be left or right"));
        }
        let result = platform()?
            .perform_edge_back(options.window, edge)
            .await
            .map_err(platform_err)?;
        let json = serde_json::json!({
            "input": result.input,
            "delivered": result.delivered,
            "events": result.events,
            "durationMs": result.duration_ms,
        });
        json_to_js(&ctx, &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_webview_app_has_no_input_where_only_gpu_pages_take_gestures() {
        let touch = || vec![GestureInput::Touch];
        assert_eq!(app_inputs(touch(), false, true), touch());
        assert!(app_inputs(touch(), false, false).is_empty());
        assert_eq!(app_inputs(touch(), true, false), touch());
    }
}
