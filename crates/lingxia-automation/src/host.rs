//! Host-only cross-lxapp management, browser tabs, and app-window capture.
//! The `Automation` root checks the `host` privilege before exposing these
//! unforgeable drivers.
//! macOS is the reference platform where every capability is live.

use crate::auto_err;
use crate::page::png_dimensions;
use crate::resolve::{json_to_js, resolve_lxapp_by_id};
use base64::{Engine as _, engine::general_purpose};
use lxapp::{LxApp, LxAppStartupOptions, ReleaseType};
use rong::{
    Class, FromJSObject, HostError, IntoJSObject, JSContext, JSObject, JSResult, JSValue,
    function::Optional, js_class, js_method,
};
use serde_json::json;
use std::time::Duration;

fn illegal_ctor() -> rong::RongJSError {
    HostError::new(rong::error::E_ILLEGAL_CONSTRUCTOR, "Use lx.automation()").into()
}

fn to_js<T: serde::Serialize>(ctx: &JSContext, value: &T) -> JSResult<JSValue> {
    let json = serde_json::to_value(value).map_err(|err| auto_err(err.to_string()))?;
    json_to_js(ctx, &json)
}

fn release_type(raw: Option<&str>) -> JSResult<ReleaseType> {
    match raw.map(str::trim) {
        None | Some("") | Some("release") => Ok(ReleaseType::Release),
        Some("preview") => Ok(ReleaseType::Preview),
        Some("developer") => Ok(ReleaseType::Developer),
        Some(other) => Err(auto_err(format!(
            "unknown releaseType '{other}' (expected release | preview | developer)"
        ))),
    }
}

/// Self-targeted lifecycle ops run teardown from inside the calling app's own
/// logic runtime (same re-entrancy hazard `eval` guards against). Reject them.
/// A host automation context has no self lxapp and runs on its own worker, so
/// nothing is re-entrant there.
pub(crate) fn reject_self(ctx: &JSContext, target: &LxApp, verb: &str) -> JSResult<()> {
    let caller = match LxApp::from_ctx(ctx) {
        Ok(caller) => caller,
        Err(_) if crate::host_automation_authority(ctx).is_some() => return Ok(()),
        Err(err) => return Err(err),
    };
    if target.appid == caller.appid {
        return Err(auto_err(format!(
            "cannot {verb} the calling app from its own logic runtime; \
             target a different app (self-exit: lx.app.exit())"
        )));
    }
    Ok(())
}

// ===================== LxAppManager =====================

#[js_class(clone)]
pub(crate) struct JSLxAppManager {}

impl JSLxAppManager {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject, Default)]
struct AppOpt {
    app: Option<String>,
}

#[derive(FromJSObject)]
struct OpenOpt {
    appid: String,
    path: Option<String>,
    #[js_name = "releaseType"]
    release_type: Option<String>,
}

#[derive(Debug, Clone, IntoJSObject)]
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
    async fn list(&self, ctx: JSContext) -> JSResult<JSValue> {
        to_js(&ctx, &lxapp::list_lxapps())
    }

    #[js_method]
    async fn current(&self, ctx: JSContext) -> JSResult<JSValue> {
        let (appid, path, _) = lxapp::get_current_lxapp();
        to_js(&ctx, &json!({ "appid": appid, "currentPage": path }))
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

    /// Enumerate the host app's windows (`lxdev lxapp windows`).
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

    /// PNG screenshot of a host app window (`lxdev lxapp screenshot`);
    /// defaults to the session's focused/main window.
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
}

// ===================== DeviceDriver =====================

#[js_class(clone)]
pub(crate) struct JSDeviceDriver {}

impl JSDeviceDriver {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject)]
struct DeviceSetOpt {
    /// Device preset id (see `list()`).
    id: String,
    /// Force landscape (`true`) or portrait (`false`); omit to use the
    /// runner's normal device-selection behavior.
    landscape: Option<bool>,
}

#[js_class(rename = "DeviceDriver")]
impl JSDeviceDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    /// The device presets the host runner offers.
    #[js_method]
    async fn list(&self, ctx: JSContext) -> JSResult<JSValue> {
        let entries = lxapp::device::device_list().map_err(auto_err)?;
        to_js(&ctx, &entries)
    }

    /// The currently selected device and orientation.
    #[js_method]
    async fn get(&self, ctx: JSContext) -> JSResult<JSValue> {
        let state = lxapp::device::device_get().map_err(auto_err)?;
        to_js(&ctx, &state)
    }

    /// Switch the simulated device by preset id and/or orientation.
    #[js_method]
    async fn set(&self, ctx: JSContext, options: DeviceSetOpt) -> JSResult<JSValue> {
        let state = lxapp::device::device_set(&options.id, options.landscape).map_err(auto_err)?;
        to_js(&ctx, &state)
    }
}

// ===================== BrowserDriver =====================

#[js_class(clone)]
pub(crate) struct JSBrowserDriver {}

impl JSBrowserDriver {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject)]
struct BrowserOpenOpt {
    url: String,
    tab: Option<String>,
}

#[derive(FromJSObject, Default)]
struct TabOpt {
    tab: Option<String>,
}

#[derive(FromJSObject)]
struct BrowserEvalOpt {
    js: String,
    tab: Option<String>,
    /// After the eval, wait for a navigation it triggers.
    #[js_name = "waitNavigation"]
    wait_navigation: Option<bool>,
    /// With `waitNavigation`, wait until the load completes.
    complete: Option<bool>,
    #[js_name = "timeoutMs"]
    timeout_ms: Option<u64>,
}

#[derive(FromJSObject)]
struct BrowserQueryOpt {
    css: String,
    tab: Option<String>,
    #[js_name = "maxText"]
    max_text: Option<usize>,
    /// Return untruncated text/value (ignores `maxText`).
    full: Option<bool>,
}

/// Exactly one condition field must be set.
#[derive(FromJSObject)]
struct BrowserWaitOpt {
    loaded: Option<bool>,
    exists: Option<String>,
    visible: Option<String>,
    hidden: Option<String>,
    editable: Option<String>,
    js: Option<String>,
    url: Option<String>,
    #[js_name = "urlContains"]
    url_contains: Option<String>,
    navigation: Option<bool>,
    /// With `navigation`: baseline URL to detect a change from (default: the
    /// current URL is not pinned, so any navigation satisfies it).
    #[js_name = "fromUrl"]
    from_url: Option<String>,
    complete: Option<bool>,
    tab: Option<String>,
    #[js_name = "timeoutMs"]
    timeout_ms: Option<u64>,
}

#[derive(FromJSObject)]
struct BrowserSelectorOpt {
    css: String,
    tab: Option<String>,
}

/// `click` / `press` also carry the navigation-sync flags.
#[derive(FromJSObject)]
struct BrowserClickOpt {
    css: String,
    tab: Option<String>,
    #[js_name = "waitNavigation"]
    wait_navigation: Option<bool>,
    complete: Option<bool>,
    #[js_name = "timeoutMs"]
    timeout_ms: Option<u64>,
}

#[derive(FromJSObject)]
struct BrowserTextOpt {
    css: String,
    text: String,
    tab: Option<String>,
}

#[derive(FromJSObject)]
struct BrowserPressOpt {
    key: String,
    tab: Option<String>,
    #[js_name = "waitNavigation"]
    wait_navigation: Option<bool>,
    complete: Option<bool>,
    #[js_name = "timeoutMs"]
    timeout_ms: Option<u64>,
}

#[derive(FromJSObject)]
struct BrowserScrollOpt {
    dx: Option<f64>,
    dy: Option<f64>,
    tab: Option<String>,
}

#[derive(Debug, Clone, IntoJSObject)]
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
            initial_url: o.from_url.clone(),
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

/// Capture the pre-action URL when `wait_navigation` is set, so a following
/// [`browser_wait_after_action`] can detect the navigation the action triggers.
/// Mirrors the devtool browser handlers' arming order.
async fn browser_arm_navigation(
    tab: &str,
    wait_navigation: bool,
) -> JSResult<Option<Option<String>>> {
    if wait_navigation {
        Ok(Some(
            lingxia_browser::current_url(tab)
                .await
                .map_err(|err| auto_err(err.to_string()))?,
        ))
    } else {
        Ok(None)
    }
}

/// Wait for the armed navigation after the action ran. `None` armed value means
/// `wait_navigation` was off → returns `None` (no navigation payload).
async fn browser_wait_after_action(
    tab: &str,
    armed: Option<Option<String>>,
    complete: bool,
    timeout: Duration,
) -> JSResult<Option<serde_json::Value>> {
    let Some(initial_url) = armed else {
        return Ok(None);
    };
    let result = lingxia_browser::wait(
        tab,
        lingxia_browser::BrowserWaitCondition::Navigation {
            initial_url,
            wait_until_complete: complete,
        },
        timeout,
    )
    .await
    .map_err(|err| auto_err(err.to_string()))?;
    serde_json::to_value(&result)
        .map(Some)
        .map_err(|err| auto_err(err.to_string()))
}

fn browser_wait_duration(timeout_ms: Option<u64>) -> Duration {
    Duration::from_millis(
        timeout_ms
            .unwrap_or(BROWSER_WAIT_DEFAULT_MS)
            .min(BROWSER_WAIT_MAX_MS),
    )
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

    /// Evaluate JavaScript in the tab's page. With `waitNavigation`, awaits a
    /// navigation the script triggers and resolves to `{ value, navigation }`.
    #[js_method]
    async fn eval(&self, ctx: JSContext, options: BrowserEvalOpt) -> JSResult<JSValue> {
        let tab = resolve_tab_id(&options.tab)?;
        let wait_navigation = options.wait_navigation.unwrap_or(false);
        let timeout = Duration::from_millis(options.timeout_ms.unwrap_or(BROWSER_EVAL_DEFAULT_MS));
        let armed = browser_arm_navigation(&tab, wait_navigation).await?;
        let value = tokio::time::timeout(
            timeout,
            lingxia_browser::evaluate_javascript(&tab, &options.js),
        )
        .await
        .map_err(|_| auto_err("browser eval timed out"))?
        .map_err(|err| auto_err(err.to_string()))?;
        match browser_wait_after_action(&tab, armed, options.complete.unwrap_or(false), timeout)
            .await?
        {
            Some(navigation) => {
                json_to_js(&ctx, &json!({ "value": value, "navigation": navigation }))
            }
            None => json_to_js(&ctx, &value),
        }
    }

    /// Query element information in the tab's page. `full` returns untruncated
    /// text/value (ignoring `maxText`).
    #[js_method]
    async fn query(&self, ctx: JSContext, options: BrowserQueryOpt) -> JSResult<JSValue> {
        let tab = resolve_tab_id(&options.tab)?;
        let max_text = if options.full.unwrap_or(false) {
            None
        } else {
            options.max_text
        };
        let info = lingxia_browser::query_with_max_text(&tab, &options.css, max_text)
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

    /// Click an element. With `waitNavigation`, awaits a navigation the click
    /// triggers and resolves to the navigation payload (else `null`).
    #[js_method]
    async fn click(&self, ctx: JSContext, options: BrowserClickOpt) -> JSResult<JSValue> {
        let tab = resolve_tab_id(&options.tab)?;
        let wait_navigation = options.wait_navigation.unwrap_or(false);
        let timeout = browser_wait_duration(options.timeout_ms);
        let armed = browser_arm_navigation(&tab, wait_navigation).await?;
        lingxia_browser::click(&tab, &options.css)
            .await
            .map_err(|err| auto_err(err.to_string()))?;
        let navigation =
            browser_wait_after_action(&tab, armed, options.complete.unwrap_or(false), timeout)
                .await?;
        json_to_js(&ctx, &navigation.unwrap_or(serde_json::Value::Null))
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

    /// Press a key. With `waitNavigation`, awaits a navigation it triggers and
    /// resolves to the navigation payload (else `null`).
    #[js_method]
    async fn press(&self, ctx: JSContext, options: BrowserPressOpt) -> JSResult<JSValue> {
        let tab = resolve_tab_id(&options.tab)?;
        let wait_navigation = options.wait_navigation.unwrap_or(false);
        let timeout = browser_wait_duration(options.timeout_ms);
        let armed = browser_arm_navigation(&tab, wait_navigation).await?;
        lingxia_browser::press(&tab, &options.key)
            .await
            .map_err(|err| auto_err(err.to_string()))?;
        let navigation =
            browser_wait_after_action(&tab, armed, options.complete.unwrap_or(false), timeout)
                .await?;
        json_to_js(&ctx, &navigation.unwrap_or(serde_json::Value::Null))
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

#[js_class(clone)]
pub(crate) struct JSBrowserCookies {}

impl JSBrowserCookies {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject, Default)]
struct CookiesListOpt {
    tab: Option<String>,
    all: Option<bool>,
}

#[derive(FromJSObject)]
struct CookieSetOpt {
    name: String,
    value: String,
    url: Option<String>,
    domain: Option<String>,
    path: Option<String>,
    secure: Option<bool>,
    #[js_name = "httpOnly"]
    http_only: Option<bool>,
    #[js_name = "expiresUnixMs"]
    expires_unix_ms: Option<i64>,
    #[js_name = "sameSite"]
    same_site: Option<String>,
    tab: Option<String>,
}

#[derive(FromJSObject)]
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
// ===================== shared window option / screenshot payload =====================

#[derive(FromJSObject, Default)]
struct WindowOpt {
    window: Option<String>,
}

#[derive(Debug, Clone, IntoJSObject)]
struct JSAppScreenshot {
    format: String,
    base64: String,
    width: u32,
    height: u32,
}
