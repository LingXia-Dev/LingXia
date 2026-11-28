use crate::authorization;
use crate::authorization::LogicRoute;
use crate::i18n::js_error;
use crate::update;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::host::HostInvocationContext;
use lxapp::{self, Channel, LxApp, LxAppStartupOptions};
use rong::{FromJSObject, JSContext, JSObject, JSResult};
use serde_json::Value;
use std::sync::Arc;

#[derive(FromJSObject)]
#[ts_skip]
pub(crate) struct NavigateToAppOptions {
    #[js_name = "appId"]
    pub(crate) appid: String,
    // Kept only so the forward-only API rejects legacy route input instead of
    // silently ignoring an unknown object field.
    pub(crate) path: Option<String>,
    pub(crate) page: Option<String>,
    pub(crate) query: Option<JSObject>,
    pub(crate) channel: Option<String>,
    #[js_name = "version"]
    pub(crate) target_version: Option<String>,
}

pub(crate) fn build_startup_options(
    target: &LxApp,
    options: &NavigateToAppOptions,
) -> Result<(LxAppStartupOptions, Channel), lingxia_error::Error> {
    let path = resolve_page_target(target, options)?;
    let mut startup_options = LxAppStartupOptions::new(&path);

    let channel = parse_channel(options.channel.as_deref())?;

    if options.channel.is_some() {
        startup_options = startup_options.set_release_type(channel);
    }

    Ok((startup_options, channel))
}

fn parse_channel(channel: Option<&str>) -> Result<Channel, lingxia_error::Error> {
    lxapp::parse_optional_channel(channel).map_err(lingxia_error::Error::invalid_arg)
}

fn resolve_page_target<'a>(
    target: &'a LxApp,
    options: &'a NavigateToAppOptions,
) -> Result<String, lingxia_error::Error> {
    validate_page_selector(options)?;
    let path = if let Some(page) = options.page.as_deref().map(str::trim) {
        target
            .find_page_path_by_name(page)
            .ok_or_else(|| lingxia_error::Error::not_found(format!("page name: {page}")))?
    } else {
        String::new()
    };
    append_query(path, options.query.as_ref())
}

fn validate_page_selector(options: &NavigateToAppOptions) -> Result<(), lingxia_error::Error> {
    if options.path.is_some() {
        return Err(lingxia_error::Error::invalid_arg(
            "path is not supported; pass the configured page name in page".to_string(),
        ));
    }
    if options
        .page
        .as_deref()
        .is_some_and(|page| page.trim().is_empty())
    {
        return Err(lingxia_error::Error::invalid_arg(
            "page must be a non-empty configured page name".to_string(),
        ));
    }
    Ok(())
}

fn append_query(path: String, query: Option<&JSObject>) -> Result<String, lingxia_error::Error> {
    let Some(query) = query else {
        return Ok(path);
    };
    let query_json = query.to_json_string().map_err(lxapp::rong_error)?;
    let query: Value = serde_json::from_str(&query_json)?;
    lxapp::append_page_query(path, &query).map_err(lingxia_error::Error::invalid_arg)
}

fn should_navigate_to_app(
    lxapp: &LxApp,
    options: &NavigateToAppOptions,
) -> Result<bool, lingxia_error::Error> {
    validate_page_selector(options)?;
    if options.appid.is_empty() {
        return Err(lingxia_error::Error::invalid_arg(
            "navigateToApp requires appId".to_string(),
        ));
    }

    if lxapp.appid == options.appid {
        return Ok(false);
    }

    Ok(true)
}

const BROWSER_SYSTEM_APP_ID: &str = "app.lingxia.browser";

fn ensure_ordinary_app_target(appid: &str) -> Result<(), lingxia_error::Error> {
    #[cfg(feature = "terminal")]
    if appid == lingxia_terminal_config::SETTINGS_APP_ID {
        return Err(lingxia_error::Error::not_supported(
            "the host Terminal Settings control session is reserved; use lx.surface.open({ app })"
                .to_string(),
        ));
    }
    if appid == BROWSER_SYSTEM_APP_ID {
        return Err(lingxia_error::Error::not_supported(
            "the host Browser System control session is reserved; use lx.surface.open({ app })"
                .to_string(),
        ));
    }
    Ok(())
}

pub(crate) async fn prepare_app_open(
    lxapp: &Arc<LxApp>,
    options: &NavigateToAppOptions,
    control_invocation: Option<&HostInvocationContext>,
) -> JSResult<(LxAppStartupOptions, Channel)> {
    let target_appid = options.appid.clone();
    let host_control_surface =
        register_host_control_surface_bundle(lxapp, &target_appid, control_invocation)?;
    validate_page_selector(options).map_err(js_error)?;
    let release_type = lxapp::resolve_open_channel(
        &target_appid,
        lxapp::parse_requested_channel(options.channel.as_deref())
            .map_err(|e| js_error(lingxia_error::Error::invalid_arg(e)))?,
    );
    let target_version = options
        .target_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if host_control_surface && target_version.is_some() {
        return Err(js_error(lingxia_error::Error::invalid_arg(
            "host-bundled control surface apps do not support version".to_string(),
        )));
    }

    lxapp::ensure_open_allowed(&target_appid)
        .await
        .map_err(js_error)?;

    if !host_control_surface {
        if let Some(target_version) = target_version {
            lxapp::ensure_target_version_ready(lxapp, &target_appid, release_type, target_version)
                .await
                .map_err(js_error)?;
        } else {
            update::ensure_first_install(lxapp, &target_appid, release_type).await?;
        }
    }

    let target_app = if host_control_surface {
        let authority = crate::native_control_authority().ok_or_else(|| {
            js_error(lingxia_error::Error::not_supported(
                "native control authority is not initialized".to_string(),
            ))
        })?;
        lxapp::ensure_control_surface_lxapp(authority, &target_appid, release_type)
    } else {
        lxapp::ensure_lxapp_ready(&target_appid, release_type).await
    }
    .map_err(js_error)?;
    let (startup_options, _) = build_startup_options(&target_app, options).map_err(js_error)?;
    // Options default to the release channel. The instance above may already
    // be a draft dev bundle; opening with the default asks for a channel switch.
    let release_type = target_app.release_type();
    let startup_options = startup_options.set_release_type(release_type);

    Ok((startup_options, release_type))
}

fn register_host_control_surface_bundle(
    lxapp: &Arc<LxApp>,
    target_appid: &str,
    control_invocation: Option<&HostInvocationContext>,
) -> JSResult<bool> {
    let route_and_label: Option<(LogicRoute, &'static str)> = {
        #[cfg(feature = "terminal")]
        if target_appid == lingxia_terminal_config::SETTINGS_APP_ID {
            Some((
                LogicRoute::ShellOpenHostTerminalSettings,
                "Terminal Settings",
            ))
        } else if target_appid == BROWSER_SYSTEM_APP_ID {
            Some((LogicRoute::ShellOpenHostBrowserSurface, "Browser System"))
        } else {
            None
        }
        #[cfg(not(feature = "terminal"))]
        if target_appid == BROWSER_SYSTEM_APP_ID {
            Some((LogicRoute::ShellOpenHostBrowserSurface, "Browser System"))
        } else {
            None
        }
    };
    let Some((route, label)) = route_and_label else {
        return Ok(false);
    };
    let Some(invocation) = control_invocation else {
        return Err(js_error(lingxia_error::Error::not_supported(format!(
            "the host {label} control session is reserved; use lx.surface.open({{ app }})"
        ))));
    };
    if let Err(denied) = authorization::authorize(invocation, route) {
        // A caller that lacks the audience is denied, not unsupported: every
        // control-plane refusal reaches JS as E_PERMISSION_DENIED.
        return Err(authorization::denial_error(denied));
    }
    let manifest = format!("{target_appid}/lxapp.json");
    lxapp.runtime.read_asset(&manifest).map_err(|_| {
        js_error(lingxia_error::Error::not_found(format!(
            "{label} is not bundled by this host"
        )))
    })?;
    lxapp::register_builtin_asset_bundle(target_appid.to_string());
    Ok(true)
}

async fn do_navigate_to_app(
    invocation: HostInvocationContext,
    options: NavigateToAppOptions,
) -> JSResult<()> {
    let lxapp = invocation.lxapp();
    let target_appid = options.appid.clone();
    let (startup_options, _) = prepare_app_open(&lxapp, &options, None).await?;
    let release_type = startup_options.release_type;

    lxapp
        .navigate_to(target_appid.clone(), startup_options)
        .map_err(js_error)?;

    lxapp::schedule_lxapp_update_check(&target_appid, release_type);
    Ok(())
}

fn do_navigate_back_lxapp(lxapp: &LxApp) -> Result<(), lingxia_error::Error> {
    lxapp.navigate_back()?;
    Ok(())
}

/// Open another lxapp, optionally at one of its pages.
///
/// Navigating to the lxapp already running is a no-op. Rejects with
/// `E_SURFACE_CONFLICT` when the target is currently docked as an aside —
/// close that aside before opening it as a main.
pub(crate) async fn navigate_to_app(ctx: JSContext, options: NavigateToAppOptions) -> JSResult<()> {
    let invocation = authorization::invocation_from_context(&ctx)?;
    let lxapp = invocation.lxapp();

    ensure_ordinary_app_target(&options.appid).map_err(js_error)?;

    if !should_navigate_to_app(&lxapp, &options).map_err(js_error)? {
        return Ok(());
    }

    // One lxapp, one region: navigating (main) to an lxapp that is currently
    // docked as an aside must not silently move it — the caller closes the
    // aside first.
    if lxapp::open_region(&options.appid) == Some(lxapp::LxAppOpenRegion::Aside) {
        return Err(rong::HostError::new(
            "E_SURFACE_CONFLICT",
            format!(
                "lxapp '{}' is already open as an aside; close it before navigating to it as a main",
                options.appid
            ),
        )
        .into());
    }

    do_navigate_to_app(invocation, options).await?;
    Ok(())
}

/// Leave this lxapp and reveal the one that opened it.
pub(crate) async fn navigate_back_app(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    do_navigate_back_lxapp(&lxapp).map_err(js_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(page: Option<&str>, path: Option<&str>) -> NavigateToAppOptions {
        NavigateToAppOptions {
            appid: "target".to_string(),
            path: path.map(str::to_string),
            page: page.map(str::to_string),
            query: None,
            channel: None,
            target_version: None,
        }
    }

    #[test]
    fn app_navigation_rejects_route_paths() {
        let error = validate_page_selector(&options(None, Some("/pages/home/index")))
            .unwrap_err()
            .to_string();
        assert!(error.contains("path is not supported"));
    }

    #[test]
    fn app_navigation_rejects_an_empty_page_name() {
        let error = validate_page_selector(&options(Some("  "), None))
            .unwrap_err()
            .to_string();
        assert!(error.contains("page must be a non-empty configured page name"));
    }

    #[cfg(feature = "terminal")]
    #[test]
    fn ordinary_navigation_cannot_promote_a_settings_app_id() {
        let error = ensure_ordinary_app_target(lingxia_terminal_config::SETTINGS_APP_ID)
            .expect_err("the fixed shell route is the only control entry");
        assert!(error.to_string().contains("lx.surface.open({ app })"));
        assert!(ensure_ordinary_app_target("same.app").is_ok());
    }
}
