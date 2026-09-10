use lingxia_service::applink::AppLinkTarget;
use lxapp::{LxAppStartupOptions, Scene};

pub(crate) fn install_handler() {
    lingxia_service::applink::register_handler(open_target);
}

/// Deliver `url` as if the OS posted an App Link.
///
/// Same codes as the platform FFI: `1` delivered, `0` not https or host not
/// configured, `-1` malformed `/lxapp/*` or no handler. Delivery is not
/// navigation complete — `open_lxapp` is spawned after this returns.
#[cfg(feature = "devtool")]
pub fn inject(url: &str) -> i32 {
    lingxia_service::applink::deliver(url)
}

fn open_target(target: AppLinkTarget) -> i32 {
    let Some(appid) = resolve_target_appid(&target) else {
        log::warn!("AppLink missing appId and the host has no homeAppId");
        return -1;
    };
    log::info!(
        "AppLink accepted: appid={}, path={}, releaseType={}",
        appid,
        target.path,
        target.release_type
    );

    let options = LxAppStartupOptions::new(&target.path)
        .set_query(target.query)
        .set_release_type(target.release_type)
        .set_scene(Scene::AppLink)
        .set_link_url(target.url);
    let release_type = target.release_type;

    std::mem::drop(rong_rt::RongExecutor::global().spawn(async move {
        if let Err(err) = lxapp::prepare_lxapp_open(&appid, release_type).await {
            lxapp::notify_lxapp_open_blocked(&err);
            return;
        }
        if let Err(err) = lxapp::open_lxapp(&appid, options) {
            log::warn!("AppLink open failed for {}: {}", appid, err);
        }
    }));
    1
}

fn resolve_target_appid(target: &AppLinkTarget) -> Option<String> {
    let appid = target.appid.trim();
    if !appid.is_empty() {
        return Some(appid.to_string());
    }
    lingxia_app_context::home_app_id().map(str::to_string)
}
