use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::ui::UIUpdate;

impl UIUpdate for Platform {
    fn update_navbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        // Use existing refreshNavBar function via TSFN (it will get current path internally)
        lingxia_webview::platform::harmony::tsfn::call_arkts("refreshNavBar", &[&appid]).map_err(
            |e| {
                PlatformError::Platform(format!(
                    "Failed to update NavigationBar UI for appId: {}: {}",
                    appid, e
                ))
            },
        )
    }

    fn update_tabbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        // Call ArkTS updateTabBarUI function via TSFN
        lingxia_webview::platform::harmony::tsfn::call_arkts("updateTabBarUI", &[&appid]).map_err(
            |e| {
                PlatformError::Platform(format!(
                    "Failed to update TabBar UI for appId: {}: {}",
                    appid, e
                ))
            },
        )
    }

    async fn update_tabbar_ui_async(&self, appid: String) -> Result<(), PlatformError> {
        crate::rt::native_call_ui(|callback_id| {
            let callback_id = callback_id.to_string();
            lingxia_webview::platform::harmony::tsfn::call_arkts(
                "updateTabBarUIAsync",
                &[&callback_id, &appid],
            )
            .map_err(|e| {
                PlatformError::Platform(format!(
                    "Failed to update TabBar UI for appId: {}: {}",
                    appid, e
                ))
            })
        })
        .await
    }

    fn update_orientation_ui(&self, appid: String) -> Result<(), PlatformError> {
        lingxia_webview::platform::harmony::tsfn::call_arkts("updateOrientationUI", &[&appid])
            .map_err(|e| {
                PlatformError::Platform(format!(
                    "Failed to update orientation UI for appId: {}: {}",
                    appid, e
                ))
            })
    }
}
