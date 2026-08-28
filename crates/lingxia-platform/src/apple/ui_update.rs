use super::app::Platform;
use super::ffi;
use crate::error::PlatformError;
use crate::traits::ui::UIUpdate;

impl UIUpdate for Platform {
    fn host_appearance_dark(&self) -> bool {
        ffi::host_appearance_dark()
    }

    fn notify_home_first_ready(&self) {
        ffi::on_home_first_ready();
    }

    fn apply_lxapp_appearance(&self, appid: &str, dark: bool) -> Result<(), PlatformError> {
        if ffi::apply_appearance(appid, dark) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to apply appearance for appId: {appid}"
            )))
        }
    }

    async fn measure_page_chrome_capsule(
        &self,
        appid: String,
    ) -> Result<Option<String>, PlatformError> {
        // One path for iOS and macOS: on macOS the SDK answers "null" unless a
        // host (the Runner's simulated phone chrome) registered a capsule
        // provider, so plain macOS hosts still report no capsule.
        let payload = crate::rt::native_call(|callback_id| {
            ffi::get_capsule_rect(&appid, callback_id);
            Ok(())
        })
        .await?;
        Ok((payload != "null").then_some(payload))
    }

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

    async fn update_tabbar_ui_async(&self, appid: String) -> Result<(), PlatformError> {
        crate::rt::native_call_ui(|callback_id| {
            ffi::update_tabbar_ui_async(&appid, callback_id);
            Ok(())
        })
        .await
    }

    fn update_orientation_ui(&self, appid: String) -> Result<(), PlatformError> {
        let success = ffi::update_orientation_ui(&appid);
        if success {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to update orientation UI for appId: {}",
                appid
            )))
        }
    }
}
