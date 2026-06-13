use lingxia_platform::traits::app_runtime::LxAppOpenMode;
use lxapp::startup::LxAppStartupOptions;
use lxapp::{LxAppError, ReleaseType};

pub fn panel_item_for_id(panel_id: &str) -> Option<(String, String)> {
    lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref())
        .and_then(|panels| panels.items.iter().find(|item| item.id == panel_id))
        .map(|item| {
            (
                item.content.app_id.clone(),
                item.content.path.clone().unwrap_or_default(),
            )
        })
}

pub fn panels_config_json() -> Option<String> {
    lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref())
        .and_then(|panels| serde_json::to_string(panels).ok())
}

pub fn open_panel_lxapp(panel_id: &str, appid: &str, path: &str) {
    let panel_id = panel_id.to_string();
    let appid = appid.to_string();
    let path = path.to_string();

    std::mem::drop(rong::RongExecutor::global().spawn(async move {
        if let Err(err) = do_open_panel_lxapp(&panel_id, &appid, &path).await {
            log::error!("open_panel_lxapp failed for {}: {}", appid, err);
        }
    }));
}

async fn do_open_panel_lxapp(panel_id: &str, appid: &str, path: &str) -> Result<(), LxAppError> {
    lxapp::prepare_lxapp_open(appid, ReleaseType::Release).await?;

    let _ = lxapp::open_lxapp(
        appid,
        LxAppStartupOptions::new(path)
            .set_open_mode(LxAppOpenMode::Panel)
            .set_panel_id(panel_id.to_string()),
    )?;

    lxapp::schedule_lxapp_update_check(appid, ReleaseType::Release);
    Ok(())
}
