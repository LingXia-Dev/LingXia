use super::app::Platform;
use super::ffi;
use crate::error::PlatformError;
use crate::traits::ui::UIUpdate;

impl UIUpdate for Platform {
    fn update_navbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        // Use existing updateNavBarUI API (it will get current path internally)
        let success = ffi::update_navbar_ui(&appid);
        if success {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to update NavigationBar UI for appId: {}",
                appid
            )))
        }
    }

    fn update_tabbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        // Use existing updateTabBarUI API
        let success = ffi::update_tabbar_ui(&appid);
        if success {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to update TabBar UI for appId: {}",
                appid
            )))
        }
    }
}
