use lingxia_service::applink::AppLinkTarget;
use lxapp::{LxAppStartupOptions, Scene};

pub(crate) fn install_handler() {
    lingxia_service::applink::register_handler(open_target);
}

fn open_target(target: AppLinkTarget) -> i32 {
    log::info!(
        "AppLink accepted: appid={}, path={}, releaseType={}",
        target.appid,
        target.path,
        target.release_type
    );

    let appid = target.appid.clone();
    let options = LxAppStartupOptions::new(&target.path)
        .set_query(target.query)
        .set_release_type(target.release_type)
        .set_scene(Scene::AppLink);
    let release_type = target.release_type;

    let _ = rong_rt::RongExecutor::global().spawn(async move {
        if let Err(err) = lxapp::prepare_lxapp_open(&appid, release_type).await {
            log::warn!("AppLink prepare failed for {}: {}", appid, err);
            return;
        }
        if let Err(err) = lxapp::open_lxapp(&appid, options) {
            log::warn!("AppLink open failed for {}: {}", appid, err);
        }
    });
    1
}
