use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use super::Platform;
use crate::error::PlatformError;
use crate::traits::ui::UIUpdate;

type WindowsUiUpdateHandler = Arc<dyn Fn(String) + Send + Sync>;
static WINDOWS_UI_UPDATE_HANDLER: Mutex<Option<WindowsUiUpdateHandler>> = Mutex::new(None);
/// The shell repaints its own chrome when the product's light/dark setting
/// moves. A plain host that draws no chrome registers nothing and the call is
/// a no-op, exactly as it was before the setting existed.
type WindowsHostColorModeHandler = Arc<dyn Fn() + Send + Sync>;
static WINDOWS_HOST_COLOR_MODE_HANDLER: Mutex<Option<WindowsHostColorModeHandler>> =
    Mutex::new(None);
type WindowsHomeFirstReadyHandler = Arc<dyn Fn() + Send + Sync>;
static WINDOWS_HOME_FIRST_READY_HANDLER: Mutex<Option<WindowsHomeFirstReadyHandler>> =
    Mutex::new(None);

/// Async UI update: the handler receives the appid and a completion closure
/// it must call (with success) once the UI has actually applied the change.
type WindowsUiUpdateAsyncHandler = Arc<dyn Fn(String, Box<dyn FnOnce(bool) + Send>) + Send + Sync>;
static WINDOWS_UI_UPDATE_ASYNC_HANDLER: Mutex<Option<WindowsUiUpdateAsyncHandler>> =
    Mutex::new(None);
static WINDOWS_HOST_APPEARANCE_DARK: AtomicBool = AtomicBool::new(false);
/// 0 = follow the OS cache, 1 = light, 2 = dark. The Runner pins this the
/// way macOS pins `NSApp.appearance`, so `lx.appearance` Auto tracks the
/// simulated screen rather than the host OS.
static WINDOWS_HOST_APPEARANCE_OVERRIDE: AtomicU8 = AtomicU8::new(0);

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

pub fn set_windows_host_color_mode_handler(handler: WindowsHostColorModeHandler) {
    if let Ok(mut slot) = WINDOWS_HOST_COLOR_MODE_HANDLER.lock() {
        *slot = Some(handler);
    }
}

/// Pin (`Some`) or release (`None`) the value `lx.appearance` Auto resolves
/// against. `None` follows the OS cache written by
/// [`set_windows_host_appearance_dark`].
pub fn set_windows_host_appearance_override(dark: Option<bool>) {
    WINDOWS_HOST_APPEARANCE_OVERRIDE.store(
        match dark {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        },
        Ordering::Release,
    );
}

pub(crate) fn windows_host_appearance_is_dark() -> bool {
    match WINDOWS_HOST_APPEARANCE_OVERRIDE.load(Ordering::Acquire) {
        1 => false,
        2 => true,
        _ => WINDOWS_HOST_APPEARANCE_DARK.load(Ordering::Acquire),
    }
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
        // The system's answer. What the product renders in resolves the user's
        // preference against this, and the shell reads that, not this.
        //
        // Standard-tier hosts do not wire shell appearance notifications;
        // appearance:auto therefore remains light until shell chrome is enabled.
        windows_host_appearance_is_dark()
    }

    fn set_host_color_mode(&self, _dark: Option<bool>) {
        // The scheme itself is read back through `lxapp::host_appearance_dark`
        // at paint time; this only tells the chrome that it moved.
        let handler = WINDOWS_HOST_COLOR_MODE_HANDLER
            .lock()
            .ok()
            .and_then(|slot| slot.clone());
        if let Some(handler) = handler {
            handler();
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn override_wins_over_the_os_cache() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let previous_override = WINDOWS_HOST_APPEARANCE_OVERRIDE.load(Ordering::Acquire);
        let previous_os = WINDOWS_HOST_APPEARANCE_DARK.load(Ordering::Acquire);
        set_windows_host_appearance_dark(false);
        set_windows_host_appearance_override(Some(true));
        assert!(windows_host_appearance_is_dark());
        set_windows_host_appearance_override(Some(false));
        assert!(!windows_host_appearance_is_dark());
        set_windows_host_appearance_override(None);
        set_windows_host_appearance_dark(true);
        assert!(windows_host_appearance_is_dark());
        set_windows_host_appearance_override(None);
        set_windows_host_appearance_dark(false);
        assert!(!windows_host_appearance_is_dark());
        WINDOWS_HOST_APPEARANCE_OVERRIDE.store(previous_override, Ordering::Release);
        WINDOWS_HOST_APPEARANCE_DARK.store(previous_os, Ordering::Release);
    }
}
