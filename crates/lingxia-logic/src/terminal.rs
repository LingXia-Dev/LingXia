//! Trusted terminal-settings API.

use crate::authorization::{self, LogicRoute};
use lingxia_platform::traits::ui::UIUpdate;
use lingxia_terminal::TerminalTheme;
use lingxia_terminal_config::runtime::{MutationError, ThemePreviewLease};
use lingxia_terminal_config::{
    ConfigError, SETTINGS_APP_ID, TerminalConfig, ThemeMode, ThemeStore,
};
use lxapp::LxApp;
use rong::{
    FromJSObject, HostError, JSContext, JSContextService, JSFunc, JSObject, JSResult, JSValue,
    JsonToJSValue, Promise, RongJSError,
};
use serde::Serialize;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Weak};
use std::time::Duration;

const REVISION_CONFLICT: &str = "E_TERMINAL_REVISION_CONFLICT";

type ChangeListeners = Rc<RefCell<Vec<Option<JSFunc>>>>;

#[derive(FromJSObject)]
#[ts_skip]
struct RevisionOptions {
    #[js_name = "ifRevision"]
    if_revision: u64,
}

#[derive(FromJSObject)]
#[ts_skip]
struct ResetOptions {
    #[js_name = "ifRevision"]
    if_revision: u64,
    scope: Option<String>,
}

#[derive(FromJSObject)]
#[ts_skip]
struct ImportOptions {
    text: String,
    name: Option<String>,
    overwrite: Option<bool>,
}

#[cfg(target_os = "windows")]
#[derive(FromJSObject)]
#[ts_skip]
struct InstallConptyOptions {
    path: String,
}

#[cfg(target_os = "windows")]
#[derive(FromJSObject)]
#[ts_skip]
struct SetConptyEnabledOptions {
    enabled: bool,
}

struct TerminalContextService {
    app: Weak<LxApp>,
    app_data_dir: PathBuf,
    active: Rc<Cell<bool>>,
    listeners: ChangeListeners,
    previews: Rc<RefCell<HashSet<ThemePreviewLease>>>,
    change_pump: RefCell<Option<Promise>>,
}

impl JSContextService for TerminalContextService {
    fn on_shutdown(&self) {
        self.active.set(false);
        self.listeners.borrow_mut().clear();
        self.change_pump.borrow_mut().take();
        let system_is_dark = self
            .app
            .upgrade()
            .map(|app| app.runtime.host_appearance_dark())
            .unwrap_or(false);
        for lease in self.previews.borrow_mut().drain() {
            lingxia_terminal_config::runtime::end_theme_preview_for_request(
                lingxia_terminal_config::runtime::create_theme_preview_request(lease),
                &self.app_data_dir,
                system_is_dark,
            );
            lingxia_terminal_config::runtime::retire_theme_preview_lease(lease);
        }
    }
}

pub(crate) fn eligible(app: &Arc<LxApp>) -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
        && lingxia_app_context::terminal_enabled()
        // Presence is presentation only. Every call below independently uses
        // the central invocation authorization path.
        && app.app_session_class() == lxapp::AppSessionClass::ControlApp
}

pub(crate) fn owns_context(ctx: &JSContext) -> JSResult<bool> {
    let invocation = authorization::invocation_from_context(ctx)?;
    let app = invocation.lxapp();
    // This chooses the focused settings runtime profile; it is not an
    // authorization decision. Every API call below separately requires the
    // native-assigned ControlApp session.
    Ok(eligible(&app)
        && authorization::authorize(&invocation, LogicRoute::TerminalSettingsGet).is_ok()
        && app.appid == SETTINGS_APP_ID
        && app.is_host_bundled())
}

fn require_access(ctx: &JSContext, route: LogicRoute) -> JSResult<Arc<LxApp>> {
    let invocation = authorization::invocation_from_context(ctx)?;
    authorization::authorize(&invocation, route).map_err(|denied| {
        HostError::new(
            rong::error::E_PERMISSION_DENIED,
            format!(
                "{} requires a live native-assigned ControlApp session",
                denied.route().name()
            ),
        )
    })?;
    if !cfg!(any(target_os = "macos", target_os = "windows"))
        || !lingxia_app_context::terminal_enabled()
    {
        return Err(HostError::new(
            rong::error::E_PERMISSION_DENIED,
            "lx.terminal requires native host terminal support",
        )
        .into());
    }
    Ok(invocation.lxapp())
}

fn terminal_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("terminal") {
        Ok(value) => Ok(value),
        Err(_) => {
            let value = JSObject::new(ctx);
            lx.set("terminal", value.clone())?;
            Ok(value)
        }
    }
}

fn child_namespace(parent: &JSObject, ctx: &JSContext, name: &str) -> JSResult<JSObject> {
    match parent.get::<_, JSObject>(name) {
        Ok(value) => Ok(value),
        Err(_) => {
            let value = JSObject::new(ctx);
            parent.set(name, value.clone())?;
            Ok(value)
        }
    }
}

fn context(ctx: &JSContext, route: LogicRoute) -> JSResult<(Arc<LxApp>, PathBuf, bool)> {
    let app = require_access(ctx, route)?;
    let data_dir = app.app_data_dir();
    let system_is_dark = app.runtime.host_appearance_dark();
    Ok((app, data_dir, system_is_dark))
}

fn to_js<T: Serialize>(ctx: &JSContext, value: &T) -> JSResult<JSValue> {
    let json = serde_json::to_string(value).map_err(|error| {
        HostError::new(
            rong::error::E_INTERNAL,
            format!("failed to serialize terminal API result: {error}"),
        )
    })?;
    json.as_str().json_to_js_value(ctx)
}

fn object_json(object: &JSObject, label: &str) -> JSResult<serde_json::Value> {
    let json = object.to_json_string().map_err(|error| {
        HostError::new(
            rong::error::E_INVALID_ARG,
            format!("{label} must be a JSON object: {error}"),
        )
    })?;
    serde_json::from_str(&json).map_err(|error| {
        HostError::new(
            rong::error::E_INVALID_ARG,
            format!("{label} is not valid JSON: {error}"),
        )
        .into()
    })
}

fn config_error(error: ConfigError) -> RongJSError {
    let (code, message) = match error {
        ConfigError::Io(error) => (
            rong::error::E_IO,
            format!("terminal settings could not be stored: {error}"),
        ),
        ConfigError::Invalid { reason, .. } => (rong::error::E_INVALID_ARG, reason),
    };
    HostError::new(code, message).into()
}

fn mutation_error(error: MutationError) -> RongJSError {
    match error {
        MutationError::Config(error) => config_error(error),
        MutationError::RevisionConflict { expected, actual } => HostError::new(
            REVISION_CONFLICT,
            "terminal settings changed since they were read",
        )
        .with_data(rong::err_data!({
            expectedRevision: (expected),
            actualRevision: (actual)
        }))
        .into(),
    }
}

fn effective_is_dark(config: &TerminalConfig, system_is_dark: bool) -> bool {
    match config.theme.mode {
        ThemeMode::System => system_is_dark,
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
    }
}

fn snapshot_value(data_dir: &Path, system_is_dark: bool) -> serde_json::Value {
    let state = lingxia_terminal_config::runtime::settings_snapshot(data_dir, system_is_dark);
    let effective_dark = effective_is_dark(&state.value, system_is_dark);
    let selected = state.value.theme.selected(system_is_dark);
    let scheme_exists = ThemeStore::new(data_dir).get(selected).is_some();
    let mut warnings = Vec::new();
    if let Some(error) = state.warning {
        let message = match error {
            ConfigError::Invalid { reason, .. } => {
                format!("User terminal settings are invalid: {reason}")
            }
            ConfigError::Io(error) => format!("User terminal settings could not be read: {error}"),
        };
        warnings.push(serde_json::json!({
            "code": "invalidUserFile",
            "message": message,
        }));
    }
    if !scheme_exists {
        warnings.push(serde_json::json!({
            "code": "missingColorScheme",
            "message": format!("Color scheme '{selected}' is not installed"),
        }));
    }
    let resolved = lingxia_terminal_config::resolve_font(
        &state.value.font,
        &lingxia_terminal_config::runtime::installed_fonts(),
    );
    serde_json::json!({
        "revision": state.revision,
        "defaults": state.defaults,
        "overrides": state.overrides,
        "value": state.value,
        "effective": {
            "systemAppearance": if system_is_dark { "dark" } else { "light" },
            "appearance": if effective_dark { "dark" } else { "light" },
            "colorScheme": scheme_exists.then_some(selected),
            "font": resolved,
        },
        "warnings": warnings,
    })
}

fn snapshot_to_js(ctx: &JSContext, route: LogicRoute) -> JSResult<JSValue> {
    let (_, data_dir, system_is_dark) = context(ctx, route)?;
    to_js(ctx, &snapshot_value(&data_dir, system_is_dark))
}

async fn settings_get(ctx: JSContext) -> JSResult<JSValue> {
    snapshot_to_js(&ctx, LogicRoute::TerminalSettingsGet)
}

async fn settings_update(ctx: JSContext, patch: JSValue, options: JSValue) -> JSResult<JSValue> {
    let (_, data_dir, system_is_dark) = context(&ctx, LogicRoute::TerminalSettingsUpdate)?;
    let patch = patch.into_object().ok_or_else(|| {
        RongJSError::from(HostError::new(
            rong::error::E_INVALID_ARG,
            "terminal settings patch must be an object",
        ))
    })?;
    let options = options.to_rust::<RevisionOptions>()?;
    let patch = object_json(&patch, "terminal settings patch")?;
    lingxia_terminal_config::runtime::apply_config_if_revision(
        &data_dir,
        &patch,
        options.if_revision,
        system_is_dark,
    )
    .map_err(mutation_error)?;
    to_js(&ctx, &snapshot_value(&data_dir, system_is_dark))
}

async fn settings_reset(ctx: JSContext, options: JSValue) -> JSResult<JSValue> {
    let (_, data_dir, system_is_dark) = context(&ctx, LogicRoute::TerminalSettingsReset)?;
    let options = options.to_rust::<ResetOptions>()?;
    lingxia_terminal_config::runtime::reset_config_if_revision(
        &data_dir,
        options.scope.as_deref(),
        options.if_revision,
        system_is_dark,
    )
    .map_err(mutation_error)?;
    to_js(&ctx, &snapshot_value(&data_dir, system_is_dark))
}

async fn schemes_list(ctx: JSContext) -> JSResult<JSValue> {
    let (_, data_dir, _) = context(&ctx, LogicRoute::TerminalSchemesList)?;
    to_js(&ctx, &ThemeStore::new(&data_dir).list_with_schemes())
}

async fn schemes_import(ctx: JSContext, options: JSValue) -> JSResult<JSValue> {
    let (app, data_dir, _) = context(&ctx, LogicRoute::TerminalSchemesImport)?;
    let options = options.to_rust::<ImportOptions>()?;
    let scheme = lingxia_terminal_config::parse_scheme(&options.text).map_err(|error| {
        HostError::new(
            rong::error::E_INVALID_DATA,
            format!("invalid terminal color scheme: {error}"),
        )
    })?;
    scheme.to_colors().map_err(|error| {
        HostError::new(
            rong::error::E_INVALID_DATA,
            format!("invalid terminal color scheme: {error}"),
        )
    })?;
    let name = options
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| scheme.name.clone())
        .unwrap_or_else(|| "imported".to_string());
    let details = lingxia_terminal_config::runtime::import_theme(
        &data_dir,
        &name,
        &scheme,
        options.overwrite.unwrap_or(false),
        app.runtime.host_appearance_dark(),
    )
    .map_err(|error| match error {
        lingxia_terminal_config::runtime::ThemeImportError::AlreadyExists(_) => {
            HostError::new(rong::error::E_ALREADY_EXISTS, error.to_string())
        }
        lingxia_terminal_config::runtime::ThemeImportError::Io(_) => HostError::new(
            rong::error::E_IO,
            format!("failed to import terminal color scheme: {error}"),
        ),
    })?;
    to_js(&ctx, &details)
}

async fn fonts_list(ctx: JSContext) -> JSResult<JSValue> {
    require_access(&ctx, LogicRoute::TerminalFontsList)?;
    to_js(&ctx, &lingxia_terminal_config::runtime::installed_fonts())
}

#[cfg(target_os = "windows")]
async fn conpty_status(ctx: JSContext) -> JSResult<JSValue> {
    let (_, data_dir, _) = context(&ctx, LogicRoute::TerminalWindowsStatus)?;
    to_js(&ctx, &lingxia_terminal_config::windows::status(&data_dir))
}

#[cfg(target_os = "windows")]
async fn conpty_install(ctx: JSContext, options: JSValue) -> JSResult<JSValue> {
    let (app, data_dir, _) = context(&ctx, LogicRoute::TerminalWindowsInstall)?;
    let options = options.to_rust::<InstallConptyOptions>()?;
    let logical_path = options.path.trim();
    if !logical_path.starts_with("lx://temp/") {
        return Err(HostError::new(
            rong::error::E_PERMISSION_DENIED,
            "ConPTY package must be an lxapp-owned temp file",
        )
        .into());
    }
    let package_path = app.resolve_accessible_path(logical_path).map_err(|_| {
        HostError::new(
            rong::error::E_PERMISSION_DENIED,
            "ConPTY package must be an lxapp-owned temp file",
        )
    })?;
    let temp_dir = std::fs::canonicalize(&app.temp_dir).unwrap_or_else(|_| app.temp_dir.clone());
    if !package_path.starts_with(temp_dir) || !package_path.is_file() {
        return Err(HostError::new(
            rong::error::E_PERMISSION_DENIED,
            "ConPTY package must be an lxapp-owned temp file",
        )
        .into());
    }
    let status = lingxia_terminal_config::windows::install(&data_dir, &package_path)
        .map_err(|error| HostError::new(rong::error::E_INVALID_DATA, error.to_string()))?;
    to_js(&ctx, &status)
}

#[cfg(target_os = "windows")]
async fn conpty_set_enabled(ctx: JSContext, options: JSValue) -> JSResult<JSValue> {
    let (_, data_dir, _) = context(&ctx, LogicRoute::TerminalWindowsSetEnabled)?;
    let options = options.to_rust::<SetConptyEnabledOptions>()?;
    let status = lingxia_terminal_config::windows::set_enabled(&data_dir, options.enabled)
        .map_err(|error| HostError::new(rong::error::E_INVALID_STATE, error.to_string()))?;
    to_js(&ctx, &status)
}

fn preview_theme(input: JSValue, data_dir: &Path) -> JSResult<TerminalTheme> {
    let theme = if input.is_string() {
        let name = input.to_rust::<String>().map_err(|_| {
            HostError::new(
                rong::error::E_INVALID_ARG,
                "terminal preview name must be a string",
            )
        })?;
        ThemeStore::new(data_dir).get(name.trim()).ok_or_else(|| {
            HostError::new(
                rong::error::E_NOT_FOUND,
                format!("no terminal color scheme named '{}'", name.trim()),
            )
        })?
    } else {
        let object = input.into_object().ok_or_else(|| {
            HostError::new(
                rong::error::E_INVALID_ARG,
                "terminal preview expects a color-scheme name or object",
            )
        })?;
        let json = object.to_json_string()?;
        serde_json::from_str::<TerminalTheme>(&json).map_err(|error| {
            HostError::new(
                rong::error::E_INVALID_DATA,
                format!("invalid terminal preview scheme: {error}"),
            )
        })?
    };
    theme.to_colors().map_err(|error| {
        RongJSError::from(HostError::new(
            rong::error::E_INVALID_DATA,
            format!("invalid terminal preview scheme: {error}"),
        ))
    })?;
    Ok(theme)
}

fn promise_from_result(ctx: &JSContext, result: JSResult<()>) -> JSResult<Promise> {
    Promise::from_future(ctx, None, async move { result })
}

fn create_preview(ctx: JSContext) -> JSResult<JSObject> {
    let (_, data_dir, _) = context(&ctx, LogicRoute::TerminalPreviewCreate)?;
    let lease = lingxia_terminal_config::runtime::create_theme_preview_lease();
    let service = ctx
        .get_service::<TerminalContextService>()
        .expect("terminal context service is installed with lx.terminal");
    service.previews.borrow_mut().insert(lease);
    let leases = service.previews.clone();
    let closed = Rc::new(Cell::new(false));
    let handle = JSObject::new(&ctx);

    let show_data_dir = data_dir.clone();
    let show_closed = closed.clone();
    handle.set(
        "show",
        JSFunc::new(
            &ctx,
            move |ctx: JSContext, input: JSValue| -> JSResult<Promise> {
                let result = (|| {
                    require_access(&ctx, LogicRoute::TerminalPreviewShow)?;
                    if show_closed.get() {
                        return Err(HostError::new(
                            rong::error::E_INVALID_STATE,
                            "terminal preview controller is closed",
                        )
                        .into());
                    }
                    let theme = preview_theme(input, &show_data_dir)?;
                    lingxia_terminal_config::runtime::preview_theme_for_request(
                        lingxia_terminal_config::runtime::create_theme_preview_request(lease),
                        &theme,
                    )
                    .map_err(|error| {
                        HostError::new(
                            rong::error::E_INVALID_DATA,
                            format!("terminal preview failed: {error}"),
                        )
                        .into()
                    })
                })();
                promise_from_result(&ctx, result)
            },
        )?
        .name("show")?,
    )?;

    let clear_data_dir = data_dir.clone();
    let clear_closed = closed.clone();
    handle.set(
        "clear",
        JSFunc::new(&ctx, move |ctx: JSContext| -> JSResult<Promise> {
            let result = (|| {
                let app = require_access(&ctx, LogicRoute::TerminalPreviewClear)?;
                if clear_closed.get() {
                    return Ok(());
                }
                lingxia_terminal_config::runtime::end_theme_preview_for_request(
                    lingxia_terminal_config::runtime::create_theme_preview_request(lease),
                    &clear_data_dir,
                    app.runtime.host_appearance_dark(),
                );
                Ok(())
            })();
            promise_from_result(&ctx, result)
        })?
        .name("clear")?,
    )?;

    let close_data_dir = data_dir;
    handle.set(
        "close",
        JSFunc::new(&ctx, move |ctx: JSContext| -> JSResult<Promise> {
            let result = (|| {
                let app = require_access(&ctx, LogicRoute::TerminalPreviewClose)?;
                if closed.replace(true) {
                    return Ok(());
                }
                lingxia_terminal_config::runtime::end_theme_preview_for_request(
                    lingxia_terminal_config::runtime::create_theme_preview_request(lease),
                    &close_data_dir,
                    app.runtime.host_appearance_dark(),
                );
                lingxia_terminal_config::runtime::retire_theme_preview_lease(lease);
                leases.borrow_mut().remove(&lease);
                Ok(())
            })();
            promise_from_result(&ctx, result)
        })?
        .name("close")?,
    )?;
    Ok(handle)
}

fn install_on_change(
    ctx: &JSContext,
    settings: &JSObject,
    listeners: &ChangeListeners,
) -> JSResult<()> {
    let listeners = listeners.clone();
    let on_change = JSFunc::new(
        ctx,
        move |ctx: JSContext, listener: JSValue| -> JSResult<JSFunc> {
            require_access(&ctx, LogicRoute::TerminalSettingsOnChange)?;
            let listener = listener.to_rust::<JSFunc>()?;
            let slot = {
                let mut slots = listeners.borrow_mut();
                slots.push(Some(listener));
                slots.len() - 1
            };
            let listeners = listeners.clone();
            JSFunc::new(&ctx, move || -> JSResult<()> {
                if let Some(entry) = listeners.borrow_mut().get_mut(slot) {
                    *entry = None;
                }
                Ok(())
            })
        },
    )?
    .name("onChange")?;
    settings.set("onChange", on_change)?;
    Ok(())
}

fn install_change_pump(ctx: &JSContext) -> JSResult<()> {
    let service = ctx
        .get_service::<TerminalContextService>()
        .expect("terminal context service is installed with lx.terminal");
    let active = service.active.clone();
    let listeners = service.listeners.clone();
    let ctx_for_pump = ctx.clone();
    let app = service.app.clone();
    let pump = Promise::from_future(&ctx.clone(), None, async move {
        let mut seen_revision = lingxia_terminal_config::runtime::generation();
        let mut seen_fonts = lingxia_terminal_config::runtime::font_generation();
        let mut seen_dark = app
            .upgrade()
            .map(|app| app.runtime.host_appearance_dark())
            .unwrap_or(false);
        while active.get() {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if !active.get() || listeners.borrow().iter().all(Option::is_none) {
                continue;
            }
            let revision = lingxia_terminal_config::runtime::generation();
            let fonts = lingxia_terminal_config::runtime::font_generation();
            let system_is_dark = app
                .upgrade()
                .map(|app| app.runtime.host_appearance_dark())
                .unwrap_or(false);
            if (revision, fonts, system_is_dark) == (seen_revision, seen_fonts, seen_dark) {
                continue;
            }
            seen_revision = revision;
            seen_fonts = fonts;
            seen_dark = system_is_dark;
            let value = snapshot_to_js(&ctx_for_pump, LogicRoute::TerminalSettingsOnChange)?;
            let callbacks: Vec<JSFunc> = listeners.borrow().iter().flatten().cloned().collect();
            for callback in callbacks {
                let _ = callback.call::<_, JSValue>(None, (value.clone(),));
            }
        }
        Ok::<(), RongJSError>(())
    })?;
    service.change_pump.borrow_mut().replace(pump);
    Ok(())
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let invocation = authorization::invocation_from_context(ctx)?;
    let app = invocation.lxapp();
    if !eligible(&app)
        || authorization::authorize(&invocation, LogicRoute::TerminalSettingsGet).is_err()
    {
        return Ok(());
    }

    let active = Rc::new(Cell::new(true));
    let listeners = Rc::new(RefCell::new(Vec::new()));
    let previews = Rc::new(RefCell::new(HashSet::new()));
    ctx.set_service(TerminalContextService {
        app: Arc::downgrade(&app),
        app_data_dir: app.app_data_dir(),
        active,
        listeners: listeners.clone(),
        previews,
        change_pump: RefCell::new(None),
    });

    let terminal = terminal_namespace(ctx)?;
    let settings = child_namespace(&terminal, ctx, "settings")?;
    let schemes = child_namespace(&terminal, ctx, "colorSchemes")?;
    let fonts = child_namespace(&terminal, ctx, "fonts")?;
    #[cfg(target_os = "windows")]
    let windows = child_namespace(&terminal, ctx, "windows")?;

    settings.set("get", JSFunc::new(ctx, settings_get)?.name("get")?)?;
    settings.set("update", JSFunc::new(ctx, settings_update)?.name("update")?)?;
    settings.set("reset", JSFunc::new(ctx, settings_reset)?.name("reset")?)?;
    install_on_change(ctx, &settings, &listeners)?;

    schemes.set("list", JSFunc::new(ctx, schemes_list)?.name("list")?)?;
    schemes.set("import", JSFunc::new(ctx, schemes_import)?.name("import")?)?;
    schemes.set(
        "createPreview",
        JSFunc::new(ctx, create_preview)?.name("createPreview")?,
    )?;

    fonts.set("list", JSFunc::new(ctx, fonts_list)?.name("list")?)?;
    #[cfg(target_os = "windows")]
    {
        windows.set("status", JSFunc::new(ctx, conpty_status)?.name("status")?)?;
        windows.set(
            "install",
            JSFunc::new(ctx, conpty_install)?.name("install")?,
        )?;
        windows.set(
            "setEnabled",
            JSFunc::new(ctx, conpty_set_enabled)?.name("setEnabled")?,
        )?;
    }
    install_change_pump(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_appearance_honors_pinned_mode() {
        let mut config = TerminalConfig::default();
        config.theme.mode = ThemeMode::Light;
        assert!(!effective_is_dark(&config, true));
        config.theme.mode = ThemeMode::Dark;
        assert!(effective_is_dark(&config, false));
        config.theme.mode = ThemeMode::System;
        assert!(effective_is_dark(&config, true));
    }

    #[test]
    fn every_terminal_api_is_in_the_central_control_inventory() {
        for route in LogicRoute::ALL
            .iter()
            .filter(|route| route.name().starts_with("lx.terminal."))
        {
            assert_eq!(
                authorization::logic_route_inventory()[route.name()]
                    .policy()
                    .audience(),
                lxapp::host::RouteAudience::ControlAppOnly
            );
        }
    }
}
