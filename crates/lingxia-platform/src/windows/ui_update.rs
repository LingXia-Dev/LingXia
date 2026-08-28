use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::Platform;
use crate::error::PlatformError;
use crate::traits::ui::UIUpdate;

type WindowsUiUpdateHandler = Arc<dyn Fn(String) + Send + Sync>;
static WINDOWS_UI_UPDATE_HANDLER: Mutex<Option<WindowsUiUpdateHandler>> = Mutex::new(None);
type WindowsHomeFirstReadyHandler = Arc<dyn Fn() + Send + Sync>;
static WINDOWS_HOME_FIRST_READY_HANDLER: Mutex<Option<WindowsHomeFirstReadyHandler>> =
    Mutex::new(None);

/// Async UI update: the handler receives the appid and a completion closure
/// it must call (with success) once the UI has actually applied the change.
type WindowsUiUpdateAsyncHandler = Arc<dyn Fn(String, Box<dyn FnOnce(bool) + Send>) + Send + Sync>;
static WINDOWS_UI_UPDATE_ASYNC_HANDLER: Mutex<Option<WindowsUiUpdateAsyncHandler>> =
    Mutex::new(None);
static WINDOWS_HOST_APPEARANCE_DARK: AtomicBool = AtomicBool::new(false);

/// Host-registered capsule geometry, answered as the JSON payload the shared
/// Page Chrome pipeline parses (`{width,height,top,right,bottom,left}` in the
/// page's CSS pixel space). A plain Windows host draws no capsule and leaves
/// this empty; the Runner's simulated phone frame registers its floating pill.
type WindowsCapsuleRectProvider = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;
static WINDOWS_CAPSULE_RECT_PROVIDER: Mutex<Option<WindowsCapsuleRectProvider>> = Mutex::new(None);

pub fn set_windows_capsule_rect_provider(provider: WindowsCapsuleRectProvider) {
    if let Ok(mut slot) = WINDOWS_CAPSULE_RECT_PROVIDER.lock() {
        *slot = Some(provider);
    }
}

pub fn set_windows_host_appearance_dark(dark: bool) {
    WINDOWS_HOST_APPEARANCE_DARK.store(dark, Ordering::Release);
}

pub fn set_windows_ui_update_handler(handler: WindowsUiUpdateHandler) {
    if let Ok(mut slot) = WINDOWS_UI_UPDATE_HANDLER.lock() {
        *slot = Some(handler);
    }
}

pub fn set_windows_ui_update_async_handler(handler: WindowsUiUpdateAsyncHandler) {
    if let Ok(mut slot) = WINDOWS_UI_UPDATE_ASYNC_HANDLER.lock() {
        *slot = Some(handler);
    }
}

pub fn set_windows_home_first_ready_handler(handler: WindowsHomeFirstReadyHandler) {
    if let Ok(mut slot) = WINDOWS_HOME_FIRST_READY_HANDLER.lock() {
        *slot = Some(handler);
    }
}

fn invoke_windows_ui_update_handler(appid: String) {
    let handler = WINDOWS_UI_UPDATE_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    if let Some(handler) = handler {
        handler(appid);
    }
}

pub fn sync_windows_ui(appid: &str) {
    invoke_windows_ui_update_handler(appid.to_string());
}

impl UIUpdate for Platform {
    fn notify_home_first_ready(&self) {
        let handler = WINDOWS_HOME_FIRST_READY_HANDLER
            .lock()
            .ok()
            .and_then(|slot| slot.clone());
        if let Some(handler) = handler {
            handler();
        }
    }

    fn update_navbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        invoke_windows_ui_update_handler(appid);
        Ok(())
    }

    async fn measure_page_chrome_capsule(
        &self,
        appid: String,
    ) -> Result<Option<String>, PlatformError> {
        let provider = WINDOWS_CAPSULE_RECT_PROVIDER
            .lock()
            .ok()
            .and_then(|slot| slot.clone());
        Ok(provider.and_then(|provider| provider(&appid)))
    }

    fn update_tabbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        invoke_windows_ui_update_handler(appid);
        Ok(())
    }

    async fn update_tabbar_ui_async(&self, appid: String) -> Result<(), PlatformError> {
        let handler = WINDOWS_UI_UPDATE_ASYNC_HANDLER
            .lock()
            .ok()
            .and_then(|slot| slot.clone());
        let Some(handler) = handler else {
            // No async handler registered (bare host apps): the sync handler
            // applies the update inline, so resolving afterwards is accurate.
            return self.update_tabbar_ui(appid);
        };
        crate::rt::native_call_ui(|callback_id| {
            handler(
                appid.clone(),
                Box::new(move |ok| {
                    let result = if ok { Ok("{}".to_string()) } else { Err(1000) };
                    lingxia_messaging::invoke_callback(callback_id, result);
                }),
            );
            Ok(())
        })
        .await
    }

    fn host_appearance_dark(&self) -> bool {
        // Standard-tier hosts do not wire shell appearance notifications;
        // appearance:auto therefore remains light until shell chrome is enabled.
        WINDOWS_HOST_APPEARANCE_DARK.load(Ordering::Acquire)
    }

    fn apply_lxapp_appearance(&self, appid: &str, dark: bool) -> Result<(), PlatformError> {
        use lingxia_webview::platform::windows::{
            WindowsPreferredColorScheme, find_webview_handler,
            set_windows_lxapp_preferred_color_scheme,
        };
        let scheme = if dark {
            WindowsPreferredColorScheme::Dark
        } else {
            WindowsPreferredColorScheme::Light
        };
        set_windows_lxapp_preferred_color_scheme(appid, scheme);
        for webtag in lingxia_webview::runtime::list_webviews() {
            if webtag.extract_appid() != appid {
                continue;
            }
            if let Some(handler) = find_webview_handler(&webtag)
                && let Err(error) = handler.set_preferred_color_scheme(scheme)
            {
                log::warn!(
                    "failed to apply lxapp appearance to WebView {}: {}",
                    webtag,
                    error
                );
            }
        }
        Ok(())
    }

    fn clear_lxapp_appearance(&self, appid: &str) {
        lingxia_webview::platform::windows::clear_windows_lxapp_preferred_color_scheme(appid);
    }
}
