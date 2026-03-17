use crate::error;
use crate::lxapp::ReleaseType;
use crate::startup::LxAppStartupOptions;
use crate::update::{UpdateManager, ensure_first_install};
use crate::{LxAppError, lxapp};
use lingxia_platform::traits::app_runtime::LxAppPresentation;

/// Look up the (appId, path) for a panel item by its id.
/// Returns None if panels are not configured or the id is not found.
pub fn panel_item_for_id(panel_id: &str) -> Option<(String, String)> {
    crate::app::app_config()
        .and_then(|c| c.panels.as_ref())
        .and_then(|p| p.items.iter().find(|item| item.id == panel_id))
        .map(|item| {
            (
                item.content.app_id.clone(),
                item.content.path.clone().unwrap_or_default(),
            )
        })
}

pub fn panels_config_json() -> Option<String> {
    crate::app::app_config()
        .and_then(|c| c.panels.as_ref())
        .and_then(|p| serde_json::to_string(p).ok())
}

/// Open a lxapp for use in a panel without pushing it onto the navigation stack.
/// Fast-open policy:
/// - First install is still blocking (package must exist before open).
/// - For already installed apps, open immediately and run update checks in background.
/// Swift intercepts the resulting `openLxApp` callback and routes to the panel container.
pub fn open_lxapp_for_panel(panel_id: &str, appid: &str, path: &str) {
    let panel_id = panel_id.to_string();
    let appid = appid.to_string();
    let path = path.to_string();
    let _ = rong::bg::spawn(async move {
        if let Err(e) = do_open_lxapp_for_panel(&panel_id, &appid, &path).await {
            error!("open_lxapp_for_panel failed for {}: {}", appid, e).with_appid(appid.clone());
        }
    });
}

async fn do_open_lxapp_for_panel(
    panel_id: &str,
    appid: &str,
    path: &str,
) -> Result<(), LxAppError> {
    let home_appid = crate::app::app_config()
        .map(|c| c.home_lxapp_appid.clone())
        .ok_or_else(|| LxAppError::ResourceNotFound("app not initialized".to_string()))?;

    let home_lxapp = lxapp::try_get(&home_appid).ok_or_else(|| {
        LxAppError::ResourceNotFound(format!("home lxapp '{home_appid}' not found"))
    })?;

    // Keep first install as a hard gate; once installed, do not block panel open on update checks.
    ensure_first_install(&home_lxapp, appid, ReleaseType::Release).await?;

    let manager = lxapp::get_lxapps_manager().ok_or_else(|| {
        LxAppError::ResourceNotFound("lxapps manager not initialized".to_string())
    })?;
    let app = manager.ensure_lxapp(appid.to_string(), ReleaseType::Release);
    app.open(
        LxAppStartupOptions::new(path)
            .set_presentation(LxAppPresentation::Panel)
            .set_panel_id(panel_id.to_string()),
    )?;

    UpdateManager::spawn_release_lxapp_update_check(appid.to_string());
    Ok(())
}
