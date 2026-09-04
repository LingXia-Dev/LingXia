use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error,
    js_error_from_platform_error, js_invalid_parameter_error, js_service_unavailable_error,
};
use lingxia_app_context::{app_config, env_version};
use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::LxApp;
use lxapp::{
    DISPLAY_LANGUAGE_CHANGE_EVENT, DISPLAY_LANGUAGE_STATE_CHANGE_EVENT, register_app_handler,
    unregister_app_handler_token,
};
use rong::{IntoJSObject, JSContext, JSFunc, JSObject, JSResult, JSValue};
use std::cell::Cell;

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod autostart;
mod screenshot;
mod update;

/// Host app base information.
#[derive(Debug, Clone, IntoJSObject)]
struct AppBaseInfo {
    /// Raw system locale, unaffected by a saved in-app language override.
    /// For the language the UI should actually render in, use
    /// `display_language` instead.
    locale: String,
    /// Effective display language after Runner session override, persisted
    /// preference, and system locale resolution. Native chrome and `lx.*`
    /// i18n strings follow this value.
    #[js_name = "displayLanguage"]
    display_language: String,
    /// Platform family: `"iOS"` / `"macOS"` / `"Android"` / `"Windows"` /
    /// `"Harmony"`. Matches the View-side `usePlatform().os` value.
    os: String,
    #[js_name = "productName"]
    product_name: String,
    #[js_name = "version"]
    version: String,
    #[js_name = "SDKVersion"]
    sdk_version: String,
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct JsDisplayLanguageState {
    preference: String,
    effective: String,
    #[js_name = "effectiveSource"]
    effective_source: String,
}

fn js_display_language_state() -> JsDisplayLanguageState {
    let state = lxapp::display_language_state();
    let effective_source = match state.effective_source {
        lxapp::DisplayLanguageEffectiveSource::System => "system",
        lxapp::DisplayLanguageEffectiveSource::Preference => "preference",
        lxapp::DisplayLanguageEffectiveSource::SessionOverride => "sessionOverride",
    };
    JsDisplayLanguageState {
        preference: state.preference.to_string(),
        effective: state.effective.to_string(),
        effective_source: effective_source.to_string(),
    }
}

/// Read the host app's identity: locale, display language, OS, product name,
/// product version, and SDK runtime version.
fn get_app_base_info(ctx: JSContext) -> JSResult<AppBaseInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let locale = lxapp.runtime.get_system_locale();
    let app_cfg =
        app_config().ok_or_else(|| js_service_unavailable_error("app config not available"))?;
    Ok(AppBaseInfo {
        locale: locale.to_string(),
        display_language: lxapp::display_language(),
        os: lingxia_platform::os_label().to_string(),
        product_name: app_cfg.product_name.clone(),
        version: app_cfg.product_version.clone(),
        sdk_version: lxapp::SDK_RUNTIME_VERSION.to_string(),
    })
}

/// Exit the host app immediately without a confirmation dialog.
///
/// If the user should confirm first, call `lx.showModal(...)` and invoke this
/// only after confirmation.
fn exit_app(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .exit()
        .map_err(|e| js_error_from_platform_error(&e))
}

/// Set the app-icon badge, for example an unread count.
///
/// This targets the dock on macOS, taskbar on Windows, and home/launcher icon
/// on mobile. Null or an empty string clears it. Unsupported platforms treat
/// the call as a no-op.
fn set_app_badge(ctx: JSContext, value: JSValue) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let text = badge_text(value, "lx.app.setBadge")?;
    lxapp
        .runtime
        .set_app_badge(&text)
        .map_err(|e| js_error_from_platform_error(&e))
}

/// A badge is `string | number | null`. Coercing anything else would paint
/// `[object Object]` on the dock, so reject it at the boundary instead.
pub(crate) fn badge_text(value: JSValue, api: &str) -> JSResult<String> {
    if value.is_undefined() || value.is_null() {
        return Ok(String::new());
    }
    if value.is_string() || value.is_number() {
        return value.to_rust::<String>();
    }
    Err(rong::HostError::new(
        rong::error::E_INVALID_ARG,
        format!(
            "{api} value must be a string, a number, or null (received {})",
            value.type_of()
        ),
    )
    .into())
}

/// Guard for host-app-level APIs (`checkUpdate`, `screenshot`, `autostart`).
///
/// Admission follows the native-assigned Control session class, never an appid
/// or bundle/source property supplied by the lxapp.
pub(crate) fn ensure_control_caller(lxapp: &LxApp, api_name: &str) -> JSResult<()> {
    ensure_control_classification(lxapp.is_control_app(), api_name)
}

fn ensure_control_classification(is_control: bool, api_name: &str) -> JSResult<()> {
    if is_control {
        return Ok(());
    }

    Err(js_error_from_business_code_with_detail(
        3000,
        format!("{api_name} is only available in the Control app"),
    ))
}

/// The native host app around this lxapp — its identity, updates, and window.
fn app_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("app") {
        Ok(obj) => Ok(obj),
        Err(_) => {
            let obj = JSObject::new(ctx);
            lx.set("app", obj.clone())?;
            Ok(obj)
        }
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let app = app_namespace(ctx)?;
    init_base(ctx)?;
    register_app_controls(ctx)?;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    autostart::init(ctx, &app)?;
    screenshot::init(ctx)?;
    update::init(ctx)?;

    Ok(())
}

/// Register read-only host identity for every lxapp context, including focused
/// system apps that intentionally do not receive the broader `lx.*` surface.
pub(crate) fn init_base(ctx: &JSContext) -> JSResult<()> {
    register_app_property(ctx)?;
    register_app_base_api(ctx)
}

rong::js_api! {
    fn register_app_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const app: "HostAppApi" = app_namespace(ctx)?;
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_control_classification;

    #[test]
    fn host_display_language_controls_require_native_control_classification() {
        for api in [
            "lx.app.getDisplayLanguageState",
            "lx.app.setDisplayLanguagePreference",
            "lx.app.onDisplayLanguageStateChange",
        ] {
            assert!(ensure_control_classification(true, api).is_ok());
            assert!(ensure_control_classification(false, api).is_err());
        }
    }
}

fn get_display_language_state(ctx: JSContext) -> JSResult<JsDisplayLanguageState> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_control_caller(&lxapp, "lx.app.getDisplayLanguageState")?;
    Ok(js_display_language_state())
}

/// Persist an arbitrary BCP-47 preference, or `"auto"` to follow the system.
fn set_js_display_language_preference(ctx: JSContext, preference: String) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_control_caller(&lxapp, "lx.app.setDisplayLanguagePreference")?;
    let preference = preference
        .parse::<lxapp::DisplayLanguagePreference>()
        .map_err(js_invalid_parameter_error)?;
    lxapp::set_display_language_preference(preference)
        .map_err(|error| js_error_from_lxapp_error(&error))
}

/// Follow the host's effective display language.
///
/// `getBaseInfo().displayLanguage` answers what it is now; this answers when it
/// changes. Logic needs both because the strings it hands to native chrome —
/// navigation bar titles, tab bar labels, modal and action-sheet text — are the
/// app's own, and nothing re-renders them on its behalf.
fn on_display_language_change(ctx: JSContext, callback: JSFunc) -> JSResult<JSFunc> {
    // Invoke immediately with the current value, like every other `on*`
    // subscription, so a caller never needs a separate read to get started.
    let _ = callback.call::<_, ()>(None, (lxapp::display_language(),));
    let token = register_app_handler(&ctx, DISPLAY_LANGUAGE_CHANGE_EVENT, callback)?;
    let off_ctx = ctx.clone();
    let unsubscribed = Cell::new(false);
    JSFunc::new(&ctx, move || {
        if unsubscribed.get() {
            return;
        }
        unregister_app_handler_token(&off_ctx, DISPLAY_LANGUAGE_CHANGE_EVENT, token);
        unsubscribed.set(true);
    })
}

fn on_display_language_state_change(ctx: JSContext, callback: JSFunc) -> JSResult<JSFunc> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_control_caller(&lxapp, "lx.app.onDisplayLanguageStateChange")?;
    let _ = callback.call::<_, ()>(None, (js_display_language_state(),));
    let token = register_app_handler(&ctx, DISPLAY_LANGUAGE_STATE_CHANGE_EVENT, callback)?;
    let off_ctx = ctx.clone();
    let unsubscribed = Cell::new(false);
    JSFunc::new(&ctx, move || {
        if unsubscribed.get() {
            return;
        }
        unregister_app_handler_token(&off_ctx, DISPLAY_LANGUAGE_STATE_CHANGE_EVENT, token);
        unsubscribed.set(true);
    })
}

rong::js_api! {
    fn register_app_base_api(ctx) {
        namespace HostAppApi = app_namespace(ctx)?;
        const envVersion: "HostAppEnvVersion" = env_version().as_str();
        fn getBaseInfo = get_app_base_info;
        fn onDisplayLanguageChange(
            ts_params = "callback: (language: string) => void",
            ts_return = "() => void"
        ) = on_display_language_change;
    }
}

rong::js_api! {
    fn register_app_controls(ctx) {
        namespace HostAppApi = app_namespace(ctx)?;
        fn exit = exit_app;
        fn getDisplayLanguageState(
            ts_return = "DisplayLanguageState"
        ) = get_display_language_state;
        fn onDisplayLanguageStateChange(
            ts_params = "callback: (state: DisplayLanguageState) => void",
            ts_return = "() => void"
        ) = on_display_language_state_change;
        fn setBadge(ts_params = "value: string | number | null") = set_app_badge;
        fn setDisplayLanguagePreference(
            ts_params = "preference: DisplayLanguagePreference"
        ) = set_js_display_language_preference;
    }
}
