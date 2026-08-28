use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::ui::UIUpdate;
use std::sync::atomic::{AtomicBool, Ordering};

static HOST_APPEARANCE_DARK: AtomicBool = AtomicBool::new(false);

pub fn set_harmony_host_appearance_dark(dark: bool) {
    HOST_APPEARANCE_DARK.store(dark, Ordering::Release);
}

impl UIUpdate for Platform {
    fn host_appearance_dark(&self) -> bool {
        HOST_APPEARANCE_DARK.load(Ordering::Acquire)
    }

    fn apply_lxapp_appearance(&self, appid: &str, dark: bool) -> Result<(), PlatformError> {
        let dark = if dark { "true" } else { "false" };
        lingxia_webview::platform::harmony::tsfn::call_arkts("applyLxAppAppearance", &[appid, dark])
            .map_err(|error| PlatformError::Platform(error.to_string()))
    }

    fn notify_home_first_ready(&self) {
        let _ = lingxia_webview::platform::harmony::tsfn::call_arkts("onHomeFirstReady", &[]);
    }

    fn show_splash_campaign(&self, image_path: String, duration_ms: u32) {
        let duration = duration_ms.to_string();
        let _ = lingxia_webview::platform::harmony::tsfn::call_arkts(
            "showSplashCampaign",
            &[&image_path, &duration],
        );
    }

    fn set_host_color_mode(&self, dark: Option<bool>) {
        let mode = match dark {
            Some(true) => "dark",
            Some(false) => "light",
            None => "auto",
        };
        let _ = lingxia_webview::platform::harmony::tsfn::call_arkts("setHostColorMode", &[mode]);
    }

    async fn measure_page_chrome_capsule(
        &self,
        appid: String,
    ) -> Result<Option<String>, PlatformError> {
        let payload = crate::rt::native_call(|callback_id| {
            let callback_id = callback_id.to_string();
            lingxia_webview::platform::harmony::tsfn::call_arkts(
                "getCapsuleRect",
                &[&callback_id, &appid],
            )
            .map_err(|error| PlatformError::Platform(error.to_string()))
        })
        .await?;
        Ok((payload != "null").then_some(payload))
    }

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
