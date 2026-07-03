//! Host tier (`lx.automation({ host: true })`): cross-lxapp management, browser
//! tabs, and host-window input. Gated by the `host` privilege. macOS is the
//! reference platform where every capability is live.

use crate::auto_err;
use crate::page::png_dimensions;
use crate::resolve::{json_to_js, resolve_lxapp_by_id};
use base64::{Engine as _, engine::general_purpose};
use lingxia_platform::traits::{keyboard, mouse};
use lxapp::{LxApp, LxAppStartupOptions, ReleaseType};
use rong::{
    Class, FromJSObj, HostError, IntoJSObj, JSContext, JSObject, JSResult, JSValue,
    function::Optional, js_class, js_export, js_method,
};
use serde_json::json;
use std::time::Duration;

fn illegal_ctor() -> rong::RongJSError {
    HostError::new(
        rong::error::E_ILLEGAL_CONSTRUCTOR,
        "Use lx.automation({ host: true })",
    )
    .into()
}

fn to_js<T: serde::Serialize>(ctx: &JSContext, value: &T) -> JSResult<JSValue> {
    let json = serde_json::to_value(value).map_err(|err| auto_err(err.to_string()))?;
    json_to_js(ctx, &json)
}

fn release_type(raw: Option<&str>) -> JSResult<ReleaseType> {
    match raw.map(str::trim) {
        None | Some("") | Some("release") => Ok(ReleaseType::Release),
        Some("preview") | Some("trial") => Ok(ReleaseType::Preview),
        Some("developer") | Some("develop") | Some("dev") => Ok(ReleaseType::Developer),
        Some(other) => Err(auto_err(format!(
            "unknown releaseType '{other}' (expected release | preview | developer)"
        ))),
    }
}

/// Self-targeted lifecycle ops run teardown from inside the calling app's own
/// logic runtime (same re-entrancy hazard `eval` guards against). Reject them.
fn reject_self(ctx: &JSContext, target: &LxApp, verb: &str) -> JSResult<()> {
    let caller = LxApp::from_ctx(ctx)?;
    if target.appid == caller.appid {
        return Err(auto_err(format!(
            "cannot {verb} the calling app from its own logic runtime; \
             target a different app (self-exit: lx.app.exit())"
        )));
    }
    Ok(())
}

// ===================== LxAppManager =====================

#[js_export]
pub(crate) struct JSLxAppManager {}

impl JSLxAppManager {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObj, Default)]
struct AppOpt {
    app: Option<String>,
}

#[derive(FromJSObj, Default)]
struct ListOpt {
    // Accepted for API shape; runtime currently returns all instances.
    #[allow(dead_code)]
    all: Option<bool>,
}

#[derive(FromJSObj)]
struct OpenOpt {
    appid: String,
    path: Option<String>,
    #[rename = "releaseType"]
    release_type: Option<String>,
}

#[derive(FromJSObj)]
struct EvalOpt {
    script: String,
    app: Option<String>,
    #[rename = "timeoutMs"]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSOpenResult {
    appid: String,
    path: String,
}

fn app_ref(app: &Option<String>) -> &str {
    app.as_deref().unwrap_or("current")
}

#[js_class(rename = "LxAppManager")]
impl JSLxAppManager {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    #[js_method]
    async fn list(&self, ctx: JSContext, _options: Optional<ListOpt>) -> JSResult<JSValue> {
        to_js(&ctx, &lxapp::list_lxapps())
    }

    #[js_method]
    async fn current(&self, ctx: JSContext) -> JSResult<JSValue> {
        let (appid, path, _) = lxapp::get_current_lxapp();
        to_js(&ctx, &json!({ "appid": appid, "currentPage": path }))
    }

    #[js_method]
    async fn info(&self, ctx: JSContext, options: Optional<AppOpt>) -> JSResult<JSValue> {
        let options = options.0.unwrap_or_default();
        let app = resolve_lxapp_by_id(app_ref(&options.app))?;
        to_js(&ctx, &app.runtime_info())
    }

    #[js_method]
    async fn pages(&self, ctx: JSContext, options: Optional<AppOpt>) -> JSResult<JSValue> {
        let options = options.0.unwrap_or_default();
        let app = resolve_lxapp_by_id(app_ref(&options.app))?;
        to_js(&ctx, &app.runtime_info().page_entries)
    }

    #[js_method]
    async fn open(&self, _ctx: JSContext, options: OpenOpt) -> JSResult<JSOpenResult> {
        let rt = release_type(options.release_type.as_deref())?;
        let app = lxapp::open_lxapp(
            &options.appid,
            LxAppStartupOptions::new(options.path.as_deref().unwrap_or("")).set_release_type(rt),
        )
        .map_err(|err| auto_err(err.to_string()))?;
        Ok(JSOpenResult {
            appid: app.appid.clone(),
            path: app.initial_route(),
        })
    }

    #[js_method]
    async fn close(&self, ctx: JSContext, options: Optional<AppOpt>) -> JSResult<()> {
        let options = options.0.unwrap_or_default();
        let app = resolve_lxapp_by_id(app_ref(&options.app))?;
        reject_self(&ctx, &app, "close")?;
        lxapp::close_lxapp(&app.appid).map_err(|err| auto_err(err.to_string()))
    }

    #[js_method]
    async fn restart(&self, ctx: JSContext, options: Optional<AppOpt>) -> JSResult<()> {
        let options = options.0.unwrap_or_default();
        let app = resolve_lxapp_by_id(app_ref(&options.app))?;
        reject_self(&ctx, &app, "restart")?;
        lxapp::restart_lxapp(&app.appid).map_err(|err| auto_err(err.to_string()))
    }

    #[js_method]
    async fn uninstall(&self, ctx: JSContext, options: Optional<AppOpt>) -> JSResult<()> {
        let options = options.0.unwrap_or_default();
        let app = resolve_lxapp_by_id(app_ref(&options.app))?;
        reject_self(&ctx, &app, "uninstall")?;
        lxapp::uninstall_lxapp(&app.appid).map_err(|err| auto_err(err.to_string()))
    }

    /// Cross-app logic-runtime eval (the host-tier escape hatch). Cannot target
    /// the calling app itself — the logic runtime is single-threaded, so a
    /// re-entrant eval would deadlock; reject that up front.
    #[js_method]
    async fn eval(&self, ctx: JSContext, options: EvalOpt) -> JSResult<JSValue> {
        let app = resolve_lxapp_by_id(app_ref(&options.app))?;
        reject_self(&ctx, &app, "eval")?;
        let timeout = Duration::from_millis(options.timeout_ms.unwrap_or(5000));
        let value = tokio::time::timeout(timeout, app.eval_logic(options.script))
            .await
            .map_err(|_| auto_err("lxapp eval timed out"))?
            .map_err(|err| auto_err(err.to_string()))?;
        json_to_js(&ctx, &value)
    }

    /// Return a base-tier `Automation` handle (`page` / `nav` / `lxapp` self
    /// info) scoped to another app.
    #[js_method]
    fn scope(&self, ctx: JSContext, options: Optional<AppOpt>) -> JSResult<JSObject> {
        let options = options.0.unwrap_or_default();
        let app = resolve_lxapp_by_id(app_ref(&options.app))?;
        Ok(Class::lookup::<crate::JSAutomation>(&ctx)?
            .instance(crate::JSAutomation::new(&app, false)))
    }
}

// ===================== BrowserDriver =====================

#[js_export]
pub(crate) struct JSBrowserDriver {}

impl JSBrowserDriver {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObj)]
struct BrowserOpenOpt {
    url: String,
    tab: Option<String>,
}

#[derive(FromJSObj, Default)]
struct TabOpt {
    tab: Option<String>,
}

#[derive(FromJSObj)]
struct BrowserEvalOpt {
    js: String,
    tab: Option<String>,
    #[rename = "timeoutMs"]
    timeout_ms: Option<u64>,
}

#[derive(FromJSObj)]
struct BrowserQueryOpt {
    css: String,
    tab: Option<String>,
    #[rename = "maxText"]
    max_text: Option<usize>,
}

/// Exactly one condition field must be set.
#[derive(FromJSObj)]
struct BrowserWaitOpt {
    loaded: Option<bool>,
    exists: Option<String>,
    visible: Option<String>,
    hidden: Option<String>,
    editable: Option<String>,
    js: Option<String>,
    url: Option<String>,
    #[rename = "urlContains"]
    url_contains: Option<String>,
    navigation: Option<bool>,
    complete: Option<bool>,
    tab: Option<String>,
    #[rename = "timeoutMs"]
    timeout_ms: Option<u64>,
}

#[derive(FromJSObj)]
struct BrowserSelectorOpt {
    css: String,
    tab: Option<String>,
}

#[derive(FromJSObj)]
struct BrowserTextOpt {
    css: String,
    text: String,
    tab: Option<String>,
}

#[derive(FromJSObj)]
struct BrowserPressOpt {
    key: String,
    tab: Option<String>,
}

#[derive(FromJSObj)]
struct BrowserScrollOpt {
    dx: Option<f64>,
    dy: Option<f64>,
    tab: Option<String>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSTabResult {
    tab: String,
}

const BROWSER_EVAL_DEFAULT_MS: u64 = 5_000;
const BROWSER_WAIT_DEFAULT_MS: u64 = 10_000;
const BROWSER_WAIT_MAX_MS: u64 = 60_000;

/// Resolve "current" (or omitted) to the actual current tab id, mirroring the
/// devtool handler; `lingxia_browser` ops need a concrete id.
fn resolve_tab_id(tab: &Option<String>) -> JSResult<String> {
    let raw = tab.as_deref().unwrap_or("current");
    if raw.trim().eq_ignore_ascii_case("current") {
        lingxia_browser::current_tab()
            .map(|tab| tab.tab_id)
            .ok_or_else(|| auto_err("no current browser tab"))
    } else {
        Ok(raw.trim().to_string())
    }
}

fn browser_wait_condition(o: &BrowserWaitOpt) -> JSResult<lingxia_browser::BrowserWaitCondition> {
    use lingxia_browser::BrowserWaitCondition as C;
    let mut conditions: Vec<C> = Vec::new();
    if o.loaded == Some(true) {
        conditions.push(C::Loaded);
    }
    if let Some(selector) = o.exists.clone() {
        conditions.push(C::SelectorExists { selector });
    }
    if let Some(selector) = o.visible.clone() {
        conditions.push(C::SelectorVisible { selector });
    }
    if let Some(selector) = o.hidden.clone() {
        conditions.push(C::SelectorHidden { selector });
    }
    if let Some(selector) = o.editable.clone() {
        conditions.push(C::SelectorEditable { selector });
    }
    if let Some(js) = o.js.clone() {
        conditions.push(C::JsTrue { js });
    }
    if let Some(url) = o.url.clone() {
        conditions.push(C::UrlEquals { url });
    }
    if let Some(text) = o.url_contains.clone() {
        conditions.push(C::UrlContains { text });
    }
    if o.navigation == Some(true) {
        conditions.push(C::Navigation {
            initial_url: None,
            wait_until_complete: o.complete.unwrap_or(false),
        });
    }
    match conditions.len() {
        1 => Ok(conditions.remove(0)),
        _ => Err(auto_err(
            "wait: pass exactly one condition (loaded | exists | visible | hidden | editable | js | url | urlContains | navigation)",
        )),
    }
}

#[js_class(rename = "BrowserDriver")]
impl JSBrowserDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    #[js_method]
    async fn open(&self, _ctx: JSContext, options: BrowserOpenOpt) -> JSResult<JSTabResult> {
        let tab = lingxia_browser::open(&options.url, options.tab.as_deref())
            .map_err(|err| auto_err(err.to_string()))?;
        Ok(JSTabResult { tab })
    }

    #[js_method]
    async fn tabs(&self, ctx: JSContext) -> JSResult<JSValue> {
        to_js(&ctx, &lingxia_browser::tabs())
    }

    #[js_method]
    async fn current(&self, ctx: JSContext) -> JSResult<JSValue> {
        to_js(&ctx, &lingxia_browser::current_tab())
    }

    #[js_method]
    async fn activate(&self, ctx: JSContext, options: Optional<TabOpt>) -> JSResult<JSValue> {
        let options = options.0.unwrap_or_default();
        let info = lingxia_browser::activate(&resolve_tab_id(&options.tab)?)
            .map_err(|err| auto_err(err.to_string()))?;
        to_js(&ctx, &info)
    }

    #[js_method]
    async fn close(&self, _ctx: JSContext, options: Optional<TabOpt>) -> JSResult<()> {
        let options = options.0.unwrap_or_default();
        lingxia_browser::close(&resolve_tab_id(&options.tab)?)
            .map_err(|err| auto_err(err.to_string()))
    }

    #[js_method]
    async fn reload(&self, _ctx: JSContext, options: Optional<TabOpt>) -> JSResult<()> {
        let options = options.0.unwrap_or_default();
        lingxia_browser::reload(&resolve_tab_id(&options.tab)?)
            .map_err(|err| auto_err(err.to_string()))
    }

    #[js_method]
    async fn back(&self, _ctx: JSContext, options: Optional<TabOpt>) -> JSResult<()> {
        let options = options.0.unwrap_or_default();
        lingxia_browser::go_back(&resolve_tab_id(&options.tab)?)
            .map_err(|err| auto_err(err.to_string()))
    }

    #[js_method]
    async fn forward(&self, _ctx: JSContext, options: Optional<TabOpt>) -> JSResult<()> {
        let options = options.0.unwrap_or_default();
        lingxia_browser::go_forward(&resolve_tab_id(&options.tab)?)
            .map_err(|err| auto_err(err.to_string()))
    }

    /// Evaluate JavaScript in the tab's page.
    #[js_method]
    async fn eval(&self, ctx: JSContext, options: BrowserEvalOpt) -> JSResult<JSValue> {
        let tab = resolve_tab_id(&options.tab)?;
        let timeout = Duration::from_millis(options.timeout_ms.unwrap_or(BROWSER_EVAL_DEFAULT_MS));
        let value = tokio::time::timeout(
            timeout,
            lingxia_browser::evaluate_javascript(&tab, &options.js),
        )
        .await
        .map_err(|_| auto_err("browser eval timed out"))?
        .map_err(|err| auto_err(err.to_string()))?;
        json_to_js(&ctx, &value)
    }

    /// Query element information in the tab's page.
    #[js_method]
    async fn query(&self, ctx: JSContext, options: BrowserQueryOpt) -> JSResult<JSValue> {
        let tab = resolve_tab_id(&options.tab)?;
        let info = lingxia_browser::query_with_max_text(&tab, &options.css, options.max_text)
            .await
            .map_err(|err| auto_err(err.to_string()))?;
        to_js(&ctx, &info)
    }

    /// Wait for a condition: `{ visible: css }`, `{ exists: css }`,
    /// `{ hidden: css }`, `{ editable: css }`, `{ loaded: true }`,
    /// `{ js: expr }`, `{ url }`, `{ urlContains }`, or
    /// `{ navigation: true, complete? }`.
    #[js_method]
    async fn wait(&self, ctx: JSContext, options: BrowserWaitOpt) -> JSResult<JSValue> {
        let tab = resolve_tab_id(&options.tab)?;
        let condition = browser_wait_condition(&options)?;
        let timeout = Duration::from_millis(
            options
                .timeout_ms
                .unwrap_or(BROWSER_WAIT_DEFAULT_MS)
                .min(BROWSER_WAIT_MAX_MS),
        );
        let result = lingxia_browser::wait(&tab, condition, timeout)
            .await
            .map_err(|err| auto_err(err.to_string()))?;
        to_js(&ctx, &result)
    }

    #[js_method]
    async fn click(&self, _ctx: JSContext, options: BrowserSelectorOpt) -> JSResult<()> {
        let tab = resolve_tab_id(&options.tab)?;
        lingxia_browser::click(&tab, &options.css)
            .await
            .map_err(|err| auto_err(err.to_string()))
    }

    #[js_method(rename = "type")]
    async fn type_text(&self, _ctx: JSContext, options: BrowserTextOpt) -> JSResult<()> {
        let tab = resolve_tab_id(&options.tab)?;
        lingxia_browser::type_text(&tab, &options.css, &options.text)
            .await
            .map_err(|err| auto_err(err.to_string()))
    }

    #[js_method]
    async fn fill(&self, _ctx: JSContext, options: BrowserTextOpt) -> JSResult<()> {
        let tab = resolve_tab_id(&options.tab)?;
        lingxia_browser::fill(&tab, &options.css, &options.text)
            .await
            .map_err(|err| auto_err(err.to_string()))
    }

    #[js_method]
    async fn press(&self, _ctx: JSContext, options: BrowserPressOpt) -> JSResult<()> {
        let tab = resolve_tab_id(&options.tab)?;
        lingxia_browser::press(&tab, &options.key)
            .await
            .map_err(|err| auto_err(err.to_string()))
    }

    #[js_method]
    async fn scroll(&self, _ctx: JSContext, options: BrowserScrollOpt) -> JSResult<()> {
        let tab = resolve_tab_id(&options.tab)?;
        lingxia_browser::scroll(&tab, options.dx.unwrap_or(0.0), options.dy.unwrap_or(0.0))
            .await
            .map_err(|err| auto_err(err.to_string()))
    }

    /// Scroll the matching element into view (same verb as `page.scrollTo`).
    #[js_method(rename = "scrollTo")]
    async fn scroll_to(&self, _ctx: JSContext, options: BrowserSelectorOpt) -> JSResult<()> {
        let tab = resolve_tab_id(&options.tab)?;
        lingxia_browser::scroll_to(&tab, &options.css)
            .await
            .map_err(|err| auto_err(err.to_string()))
    }

    /// PNG screenshot of the tab's web content.
    #[js_method]
    async fn screenshot(
        &self,
        _ctx: JSContext,
        options: Optional<TabOpt>,
    ) -> JSResult<JSAppScreenshot> {
        let options = options.0.unwrap_or_default();
        let tab = resolve_tab_id(&options.tab)?;
        let bytes = lingxia_browser::take_screenshot(&tab)
            .await
            .map_err(|err| auto_err(err.to_string()))?;
        let (width, height) = png_dimensions(&bytes).unwrap_or((0, 0));
        Ok(JSAppScreenshot {
            format: "png".to_string(),
            base64: general_purpose::STANDARD.encode(&bytes),
            width,
            height,
        })
    }

    #[js_method(getter, enumerable)]
    fn cookies(&self, ctx: JSContext) -> JSResult<JSObject> {
        Ok(Class::lookup::<JSBrowserCookies>(&ctx)?.instance(JSBrowserCookies::new()))
    }
}

// ---- browser.cookies.* ----

#[js_export]
pub(crate) struct JSBrowserCookies {}

impl JSBrowserCookies {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObj, Default)]
struct CookiesListOpt {
    tab: Option<String>,
    all: Option<bool>,
}

#[derive(FromJSObj)]
struct CookieSetOpt {
    name: String,
    value: String,
    url: Option<String>,
    domain: Option<String>,
    path: Option<String>,
    secure: Option<bool>,
    #[rename = "httpOnly"]
    http_only: Option<bool>,
    #[rename = "expiresUnixMs"]
    expires_unix_ms: Option<i64>,
    #[rename = "sameSite"]
    same_site: Option<String>,
    tab: Option<String>,
}

#[derive(FromJSObj)]
struct CookieDeleteOpt {
    name: String,
    domain: String,
    path: Option<String>,
    tab: Option<String>,
}

#[js_class(rename = "BrowserCookies")]
impl JSBrowserCookies {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    #[js_method]
    async fn list(&self, ctx: JSContext, options: Optional<CookiesListOpt>) -> JSResult<JSValue> {
        let options = options.0.unwrap_or_default();
        let tab = resolve_tab_id(&options.tab)?;
        let cookies = if options.all.unwrap_or(false) {
            lingxia_browser::list_all_cookies(&tab).await
        } else {
            lingxia_browser::list_cookies(&tab).await
        }
        .map_err(|err| auto_err(err.to_string()))?;
        to_js(&ctx, &cookies)
    }

    #[js_method]
    async fn set(&self, _ctx: JSContext, options: CookieSetOpt) -> JSResult<()> {
        use lingxia_webview::{WebViewCookieSameSite, WebViewCookieSetRequest};
        let tab = resolve_tab_id(&options.tab)?;
        let same_site = options
            .same_site
            .as_deref()
            .map(|raw| {
                serde_json::from_value::<WebViewCookieSameSite>(json!(raw))
                    .map_err(|_| auto_err(format!("unknown sameSite '{raw}'")))
            })
            .transpose()?;
        let request = WebViewCookieSetRequest {
            url: options.url.unwrap_or_default(),
            name: options.name,
            value: options.value,
            domain: options.domain,
            path: options.path.unwrap_or_else(|| "/".to_string()),
            secure: options.secure.unwrap_or(false),
            http_only: options.http_only.unwrap_or(false),
            expires_unix_ms: options.expires_unix_ms,
            same_site,
        };
        lingxia_browser::set_cookie(&tab, request)
            .await
            .map_err(|err| auto_err(err.to_string()))
    }

    #[js_method]
    async fn delete(&self, _ctx: JSContext, options: CookieDeleteOpt) -> JSResult<()> {
        let tab = resolve_tab_id(&options.tab)?;
        lingxia_browser::delete_cookie(
            &tab,
            &options.name,
            &options.domain,
            options.path.as_deref().unwrap_or("/"),
        )
        .await
        .map_err(|err| auto_err(err.to_string()))
    }

    #[js_method]
    async fn clear(&self, _ctx: JSContext, options: Optional<TabOpt>) -> JSResult<()> {
        let options = options.0.unwrap_or_default();
        let tab = resolve_tab_id(&options.tab)?;
        lingxia_browser::clear_cookies(&tab)
            .await
            .map_err(|err| auto_err(err.to_string()))
    }
}

// ===================== AppDriver (host window / input) =====================

#[js_export]
pub(crate) struct JSAppDriver {}

impl JSAppDriver {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObj, Default)]
struct WindowOpt {
    window: Option<String>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSAppScreenshot {
    format: String,
    base64: String,
    width: u32,
    height: u32,
}

async fn app_mouse(
    ctx: &JSContext,
    window: Option<String>,
    action: mouse::AppMouseAction,
) -> JSResult<JSValue> {
    use lingxia_platform::traits::mouse::AppMouse;
    let platform = lxapp::get_platform().ok_or_else(|| auto_err("platform is not initialized"))?;
    let result = platform
        .perform_app_mouse(mouse::AppMouseRequest {
            window_id: window,
            action,
        })
        .await
        .map_err(|err| auto_err(err.to_string()))?;
    to_js(ctx, &result)
}

async fn app_keyboard(
    ctx: &JSContext,
    window: Option<String>,
    action: keyboard::AppKeyboardAction,
) -> JSResult<JSValue> {
    use lingxia_platform::traits::keyboard::AppKeyboard;
    let platform = lxapp::get_platform().ok_or_else(|| auto_err("platform is not initialized"))?;
    let result = platform
        .perform_app_keyboard(keyboard::AppKeyboardRequest {
            window_id: window,
            action,
        })
        .await
        .map_err(|err| auto_err(err.to_string()))?;
    to_js(ctx, &result)
}

fn mouse_button(raw: &Option<String>) -> JSResult<mouse::AppMouseButton> {
    match raw.as_deref().map(str::trim) {
        None | Some("") | Some("left") => Ok(mouse::AppMouseButton::Left),
        Some("right") => Ok(mouse::AppMouseButton::Right),
        Some("middle") => Ok(mouse::AppMouseButton::Middle),
        Some(other) => Err(auto_err(format!(
            "unknown mouse button '{other}' (expected left | right | middle)"
        ))),
    }
}

fn keyboard_modifiers(raw: &Option<Vec<String>>) -> JSResult<Vec<keyboard::AppKeyboardModifier>> {
    raw.iter()
        .flatten()
        .map(|value| match value.trim() {
            "command" | "cmd" => Ok(keyboard::AppKeyboardModifier::Command),
            "shift" => Ok(keyboard::AppKeyboardModifier::Shift),
            "option" | "alt" => Ok(keyboard::AppKeyboardModifier::Option),
            "control" | "ctrl" => Ok(keyboard::AppKeyboardModifier::Control),
            other => Err(auto_err(format!(
                "unknown modifier '{other}' (expected command | shift | option | control)"
            ))),
        })
        .collect()
}

#[js_class(rename = "AppDriver")]
impl JSAppDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    #[js_method]
    async fn screenshot(
        &self,
        _ctx: JSContext,
        options: Optional<WindowOpt>,
    ) -> JSResult<JSAppScreenshot> {
        let options = options.0.unwrap_or_default();
        use lingxia_platform::traits::screenshot::AppScreenshot;
        let platform =
            lxapp::get_platform().ok_or_else(|| auto_err("platform is not initialized"))?;
        let bytes = platform
            .take_app_screenshot(options.window.as_deref())
            .await
            .map_err(|err| auto_err(err.to_string()))?;
        let (width, height) = png_dimensions(&bytes).unwrap_or((0, 0));
        Ok(JSAppScreenshot {
            format: "png".to_string(),
            base64: general_purpose::STANDARD.encode(&bytes),
            width,
            height,
        })
    }

    #[js_method]
    async fn windows(&self, ctx: JSContext) -> JSResult<JSValue> {
        use lingxia_platform::traits::screenshot::AppScreenshot;
        let platform =
            lxapp::get_platform().ok_or_else(|| auto_err("platform is not initialized"))?;
        let windows = platform
            .list_app_windows()
            .await
            .map_err(|err| auto_err(err.to_string()))?;
        to_js(&ctx, &windows)
    }

    #[js_method(getter, enumerable)]
    fn mouse(&self, ctx: JSContext) -> JSResult<JSObject> {
        Ok(Class::lookup::<JSAppMouse>(&ctx)?.instance(JSAppMouse::new()))
    }

    #[js_method(getter, enumerable)]
    fn key(&self, ctx: JSContext) -> JSResult<JSObject> {
        Ok(Class::lookup::<JSAppKey>(&ctx)?.instance(JSAppKey::new()))
    }
}

// ---- app.mouse.* ----

#[js_export]
pub(crate) struct JSAppMouse {}

impl JSAppMouse {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObj)]
struct MousePoint {
    x: f64,
    y: f64,
    button: Option<String>,
    window: Option<String>,
}

#[derive(FromJSObj)]
struct MouseDrag {
    #[rename = "fromX"]
    from_x: f64,
    #[rename = "fromY"]
    from_y: f64,
    #[rename = "toX"]
    to_x: f64,
    #[rename = "toY"]
    to_y: f64,
    button: Option<String>,
    window: Option<String>,
}

#[derive(FromJSObj)]
struct MouseScroll {
    x: f64,
    y: f64,
    dx: Option<f64>,
    dy: Option<f64>,
    window: Option<String>,
}

#[js_class(rename = "AppMouse")]
impl JSAppMouse {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    #[js_method(rename = "move")]
    async fn mouse_move(&self, ctx: JSContext, o: MousePoint) -> JSResult<JSValue> {
        app_mouse(
            &ctx,
            o.window,
            mouse::AppMouseAction::Move { x: o.x, y: o.y },
        )
        .await
    }

    #[js_method]
    async fn down(&self, ctx: JSContext, o: MousePoint) -> JSResult<JSValue> {
        let button = mouse_button(&o.button)?;
        app_mouse(
            &ctx,
            o.window,
            mouse::AppMouseAction::Down {
                x: o.x,
                y: o.y,
                button,
            },
        )
        .await
    }

    #[js_method]
    async fn up(&self, ctx: JSContext, o: MousePoint) -> JSResult<JSValue> {
        let button = mouse_button(&o.button)?;
        app_mouse(
            &ctx,
            o.window,
            mouse::AppMouseAction::Up {
                x: o.x,
                y: o.y,
                button,
            },
        )
        .await
    }

    #[js_method]
    async fn click(&self, ctx: JSContext, o: MousePoint) -> JSResult<JSValue> {
        let button = mouse_button(&o.button)?;
        app_mouse(
            &ctx,
            o.window,
            mouse::AppMouseAction::Click {
                x: o.x,
                y: o.y,
                button,
                click_count: 1,
            },
        )
        .await
    }

    #[js_method]
    async fn drag(&self, ctx: JSContext, o: MouseDrag) -> JSResult<JSValue> {
        let button = mouse_button(&o.button)?;
        app_mouse(
            &ctx,
            o.window,
            mouse::AppMouseAction::Drag {
                from_x: o.from_x,
                from_y: o.from_y,
                to_x: o.to_x,
                to_y: o.to_y,
                button,
            },
        )
        .await
    }

    #[js_method]
    async fn scroll(&self, ctx: JSContext, o: MouseScroll) -> JSResult<JSValue> {
        app_mouse(
            &ctx,
            o.window,
            mouse::AppMouseAction::Scroll {
                x: o.x,
                y: o.y,
                dx: o.dx.unwrap_or(0.0),
                dy: o.dy.unwrap_or(0.0),
            },
        )
        .await
    }
}

// ---- app.key.* ----

#[js_export]
pub(crate) struct JSAppKey {}

impl JSAppKey {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObj)]
struct KeyType {
    text: String,
    window: Option<String>,
}

#[derive(FromJSObj)]
struct KeyPress {
    key: String,
    modifiers: Option<Vec<String>>,
    window: Option<String>,
}

#[js_class(rename = "AppKey")]
impl JSAppKey {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    #[js_method(rename = "type")]
    async fn key_type(&self, ctx: JSContext, o: KeyType) -> JSResult<JSValue> {
        app_keyboard(
            &ctx,
            o.window,
            keyboard::AppKeyboardAction::Type { text: o.text },
        )
        .await
    }

    #[js_method]
    async fn press(&self, ctx: JSContext, o: KeyPress) -> JSResult<JSValue> {
        let modifiers = keyboard_modifiers(&o.modifiers)?;
        app_keyboard(
            &ctx,
            o.window,
            keyboard::AppKeyboardAction::Press {
                key: o.key,
                modifiers,
            },
        )
        .await
    }
}
