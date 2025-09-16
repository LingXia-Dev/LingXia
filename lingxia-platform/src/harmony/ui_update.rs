use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::UIUpdate;

impl UIUpdate for Platform {
    fn update_navbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        // Use existing refreshNavBar function via TSFN (it will get current path internally)
        lingxia_webview::tsfn::call_arkts("refreshNavBar", &[&appid]).map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to update NavigationBar UI for appId: {}: {}",
                appid, e
            ))
        })
    }

    fn update_tabbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        // Call ArkTS updateTabBarUI function via TSFN
        lingxia_webview::tsfn::call_arkts("updateTabBarUI", &[&appid]).map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to update TabBar UI for appId: {}: {}",
                appid, e
            ))
        })
    }
}
