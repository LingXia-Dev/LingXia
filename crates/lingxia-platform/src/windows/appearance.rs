use std::collections::HashSet;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use lingxia_webview::platform::windows::WindowsPreferredColorScheme;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::core::w;

use crate::error::PlatformError;
use crate::traits::appearance::{Appearance, AppearancePreference, AppearanceState};

use super::Platform;

type AppearanceHandler = Arc<dyn Fn(AppearancePreference) + Send + Sync>;

static PREFERENCE: AtomicU8 = AtomicU8::new(0);
static APPEARANCE_HANDLERS: Mutex<Vec<AppearanceHandler>> = Mutex::new(Vec::new());
static LISTENERS: LazyLock<Mutex<HashSet<u64>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

pub fn set_windows_appearance_handler(handler: AppearanceHandler) {
    if let Ok(mut handlers) = APPEARANCE_HANDLERS.lock() {
        handlers.push(handler);
    }
}

fn current_preference() -> AppearancePreference {
    match PREFERENCE.load(Ordering::Acquire) {
        1 => AppearancePreference::Light,
        2 => AppearancePreference::Dark,
        _ => AppearancePreference::System,
    }
}

fn system_dark() -> bool {
    let mut value = 1_u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast::<c_void>()),
            Some(&mut size),
        )
    };
    status == ERROR_SUCCESS && value == 0
}

fn state() -> AppearanceState {
    let preference = current_preference();
    AppearanceState {
        preference,
        effective_dark: match preference {
            AppearancePreference::System => system_dark(),
            AppearancePreference::Light => false,
            AppearancePreference::Dark => true,
        },
    }
}

fn apply_webviews(preference: AppearancePreference) {
    let scheme = match preference {
        AppearancePreference::System => WindowsPreferredColorScheme::Auto,
        AppearancePreference::Light => WindowsPreferredColorScheme::Light,
        AppearancePreference::Dark => WindowsPreferredColorScheme::Dark,
    };
    lingxia_webview::platform::windows::set_windows_preferred_color_scheme_for_new_webviews(scheme);
    let mut failures = Vec::new();
    for webtag in lingxia_webview::runtime::list_webviews() {
        let Some(handler) = lingxia_webview::platform::windows::find_webview_handler(&webtag)
        else {
            continue;
        };
        if let Err(error) = handler.set_preferred_color_scheme(scheme) {
            failures.push(format!("{}: {error}", webtag.key()));
        }
    }
    if !failures.is_empty() {
        log::warn!(
            "failed to apply the preferred color scheme to existing WebViews: {}",
            failures.join("; ")
        );
    }
}

fn emit(state: AppearanceState) {
    let payload = serde_json::json!({
        "preference": state.preference.as_str(),
        "effective": state.effective(),
    })
    .to_string();
    let listeners = LISTENERS
        .lock()
        .map(|listeners| listeners.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    for callback_id in listeners {
        let _ = lingxia_messaging::invoke_callback(callback_id, Ok(payload.clone()));
    }
}

/// Called by the Windows message loop after a system appearance change.
/// Returns the new effective state when the process is following the OS.
pub fn notify_windows_system_appearance_changed() -> Option<AppearanceState> {
    (current_preference() == AppearancePreference::System).then(|| {
        let current = state();
        emit(current);
        current
    })
}

impl Appearance for Platform {
    fn get_appearance(&self) -> Result<AppearanceState, PlatformError> {
        Ok(state())
    }

    fn set_appearance(
        &self,
        preference: AppearancePreference,
    ) -> Result<AppearanceState, PlatformError> {
        PREFERENCE.store(
            match preference {
                AppearancePreference::System => 0,
                AppearancePreference::Light => 1,
                AppearancePreference::Dark => 2,
            },
            Ordering::Release,
        );
        // The process preference is authoritative. A stale or closing WebView
        // must not leave native chrome and future WebViews on the old theme.
        apply_webviews(preference);
        let handlers = APPEARANCE_HANDLERS
            .lock()
            .map(|handlers| handlers.clone())
            .unwrap_or_default();
        for handler in handlers {
            handler(preference);
        }
        let current = state();
        emit(current);
        Ok(current)
    }

    fn add_appearance_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        LISTENERS
            .lock()
            .map_err(|_| PlatformError::Platform("appearance listener lock poisoned".into()))?
            .insert(callback_id);
        emit(state());
        Ok(())
    }

    fn remove_appearance_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        LISTENERS
            .lock()
            .map_err(|_| PlatformError::Platform("appearance listener lock poisoned".into()))?
            .remove(&callback_id);
        Ok(())
    }
}
