//! `Intl` for Logic on QuickJS (Android, Windows): rong's ECMA-402 over the locale data every
//! QuickJS host embeds, with its default locale on the lxapp's language. JavaScriptCore and ArkJS
//! carry their own `Intl`.
use crate::lxapp::LxApp;
use rong::{JSContext, JSResult};
use std::sync::Arc;

#[cfg(not(any(target_os = "ios", target_os = "macos", target_env = "ohos")))]
pub(super) fn install(ctx: &JSContext, lxapp: &Arc<LxApp>) -> JSResult<()> {
    use rong_quickjs::intl;
    thread_local! {
        static DATA: Result<intl::IntlData, String> =
            intl::IntlData::try_from_static_blob(lingxia_i18n::intl::INTL_DATA)
                .map_err(|e| e.to_string());
    }
    let data = DATA.with(Clone::clone).map_err(|error| {
        rong::RongJSError::from(rong::error::HostError::new(
            rong::error::E_INTERNAL,
            format!("the embedded locale data does not load: {error}"),
        ))
    })?;
    intl::configure(ctx, data, settings(lxapp));
    intl::init(ctx)?;
    let app = Arc::downgrade(lxapp);
    let own = ctx.clone();
    let follow = rong::JSFunc::new(ctx, move || {
        if let Some(app) = app.upgrade() {
            intl::set_settings(&own, settings(&app));
        }
    })?;
    super::event_bus::register_app_handler(ctx, crate::DISPLAY_LANGUAGE_CHANGE_EVENT, follow)
        .map(|_| ())
}

#[cfg(any(target_os = "ios", target_os = "macos", target_env = "ohos"))]
pub(super) fn install(_ctx: &JSContext, _lxapp: &Arc<LxApp>) -> JSResult<()> {
    Ok(())
}

#[cfg(not(any(target_os = "ios", target_os = "macos", target_env = "ohos")))]
fn settings(lxapp: &LxApp) -> rong_quickjs::intl::IntlSettings {
    rong_quickjs::intl::IntlSettings {
        locale: Some(lxapp.content_language(&crate::display_language())),
        time_zone: None,
        hour_cycle: None,
    }
}
