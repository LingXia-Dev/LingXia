//! Host display language: product preference, effective tag, session override.
//!
//! Three layers, one direction:
//!
//! 1. **System** — `Platform::get_system_locale`.
//! 2. **Host preference** — [`DisplayLanguage`]: follow the system or pin a
//!    catalog the native chrome ships. This is what settings UI and
//!    `lx.app.setDisplayLanguage` write.
//! 3. **Effective tag** — [`display_language`]: override if pinned, else the
//!    system locale. Every lxapp inherits this; they do not set it.
//!
//! A Runner session may pin a tag outside the host catalogs without persisting
//! it ([`apply_display_language_override`]) so an lxapp can be tested in a
//! language the product picker cannot choose.

use super::runtime_registry::{get_lxapps_manager, get_platform};
use crate::error::LxAppError;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_webview::WebViewController;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};

/// Product display-language preference.
///
/// `"auto"` follows the system locale. `"en-US"` and `"zh-CN"` are the two
/// catalogs native chrome ships. This is the value `lx.app.setDisplayLanguage`
/// and `lingxia::app::set_display_language` accept — not the resolved tag
/// [`display_language`] returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisplayLanguage {
    /// Follow the system locale.
    Auto,
    /// Pin English.
    EnUs,
    /// Pin Simplified Chinese.
    ZhCn,
}

impl DisplayLanguage {
    /// Wire value shared with JS and settings.json (`"auto"`, `"en-US"`, `"zh-CN"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::EnUs => "en-US",
            Self::ZhCn => "zh-CN",
        }
    }

    /// Stored override. `None` means follow the system locale.
    pub fn override_tag(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::EnUs | Self::ZhCn => Some(self.as_str()),
        }
    }
}

impl fmt::Display for DisplayLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DisplayLanguage {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "auto" => Ok(Self::Auto),
            "en-US" => Ok(Self::EnUs),
            "zh-CN" => Ok(Self::ZhCn),
            _ => Err("language must be auto, en-US, or zh-CN".to_string()),
        }
    }
}

/// In-memory override. `None` follows the system locale. May hold a tag the
/// product picker cannot choose (Runner session override).
static OVERRIDE: Mutex<Option<String>> = Mutex::new(None);

/// Keeps the persisted preference and the published in-memory value ordered as
/// one write when native settings, host Rust, and the home lxapp race.
static PREFERENCE_WRITE_LOCK: Mutex<()> = Mutex::new(());

type ChangeListener = Arc<dyn Fn() + Send + Sync>;

fn change_listeners() -> &'static Mutex<Vec<ChangeListener>> {
    static LISTENERS: OnceLock<Mutex<Vec<ChangeListener>>> = OnceLock::new();
    LISTENERS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Native chrome and the browser settings stream register here. Several
/// listeners are expected: they are different consumers of the same write.
pub fn add_display_language_change_listener(listener: Box<dyn Fn() + Send + Sync>) {
    change_listeners()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Arc::from(listener));
}

/// Effective language the UI should render in: a pinned override when set,
/// otherwise the system locale. `"en-US"` if the SDK has not been initialized.
pub fn display_language() -> String {
    if let Some(language) = OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return language;
    }
    get_platform()
        .map(|runtime| runtime.get_system_locale().to_string())
        .unwrap_or_else(|| "en-US".to_string())
}

/// Persist a product preference and publish the resolved tag to every lxapp.
///
/// Host Rust should call `lingxia::app::set_display_language` so the data
/// directory comes from the facade. Logic and the browser settings page call
/// this after the runtime is up.
pub fn set_display_language(language: DisplayLanguage) -> Result<(), LxAppError> {
    let dir = get_platform()
        .ok_or_else(|| LxAppError::Runtime("SDK has not been initialized".to_string()))?
        .app_data_dir();
    set_display_language_in(&dir, language)
}

/// Persist + publish using an explicit data directory. Startup tests use this
/// before a platform exists; product code should prefer [`set_display_language`].
pub fn set_display_language_in(
    app_data_dir: &Path,
    language: DisplayLanguage,
) -> Result<(), LxAppError> {
    let _guard = PREFERENCE_WRITE_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    lingxia_settings::set_display_language(app_data_dir, language.override_tag())
        .map_err(|error| LxAppError::Runtime(error.to_string()))?;
    apply_display_language_override(language.override_tag().map(str::to_string));
    Ok(())
}

/// Pin or clear the in-memory override without writing settings.json.
///
/// Used at process start (saved preference or Runner `--display-language`).
/// `None` follows the system locale. The tag is not restricted to host
/// catalogs, so a Runner session can exercise an lxapp language the product
/// picker cannot pin.
pub fn apply_display_language_override(language: Option<String>) {
    let normalized = language.filter(|value| !value.trim().is_empty());
    *OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) = normalized;
    publish(&display_language());
    let listeners = change_listeners()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    for listener in listeners {
        listener();
    }
}

fn publish(language: &str) {
    let Some(manager) = get_lxapps_manager() else {
        return;
    };
    let quoted = serde_json::to_string(language).unwrap_or_else(|_| "\"en-US\"".to_string());
    let script = format!("var f = globalThis.__lingxiaApplyDisplayLanguage; if (f) f({quoted});");
    // Collect first: `publish_app_event` looks the appid up in this same map,
    // and a re-entrant read while a writer is queued deadlocks the caller.
    let apps: Vec<_> = manager
        .lxapps
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().clone()))
        .collect();
    for (appid, app) in apps {
        for page in app.live_page_instances() {
            if let Some(webview) = page.webview() {
                let _ = webview.exec_js(&script);
            }
        }
        crate::appservice::event_bus::publish_app_event(
            &appid,
            crate::DISPLAY_LANGUAGE_CHANGE_EVENT,
            Some(quoted.clone()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display_language_state_lock() -> &'static Mutex<()> {
        static LOCK: Mutex<()> = Mutex::new(());
        &LOCK
    }

    #[test]
    fn parses_auto_and_host_catalogs() {
        assert_eq!(
            "auto".parse::<DisplayLanguage>().unwrap(),
            DisplayLanguage::Auto
        );
        assert_eq!(
            "en-US".parse::<DisplayLanguage>().unwrap(),
            DisplayLanguage::EnUs
        );
        assert_eq!(
            "zh-CN".parse::<DisplayLanguage>().unwrap(),
            DisplayLanguage::ZhCn
        );
        assert!("".parse::<DisplayLanguage>().is_err());
        assert!("ja-JP".parse::<DisplayLanguage>().is_err());
        assert!("en".parse::<DisplayLanguage>().is_err());
    }

    #[test]
    fn auto_stores_no_override_tag() {
        assert_eq!(DisplayLanguage::Auto.override_tag(), None);
        assert_eq!(DisplayLanguage::EnUs.override_tag(), Some("en-US"));
        assert_eq!(DisplayLanguage::ZhCn.as_str(), "zh-CN");
    }

    #[test]
    fn persist_writes_store_and_effective_value() {
        let _guard = display_language_state_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let dir = tempfile::tempdir().expect("temp app data");
        set_display_language_in(dir.path(), DisplayLanguage::ZhCn).expect("persist zh-CN");
        assert_eq!(display_language(), "zh-CN");
        assert_eq!(
            lingxia_settings::get_display_language(dir.path())
                .expect("read store")
                .as_deref(),
            Some("zh-CN")
        );

        set_display_language_in(dir.path(), DisplayLanguage::Auto).expect("persist auto");
        assert!(
            lingxia_settings::get_display_language(dir.path())
                .expect("read store")
                .is_none()
        );
        apply_display_language_override(None);
    }

    #[test]
    fn session_override_can_pin_a_tag_the_picker_cannot() {
        let _guard = display_language_state_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        apply_display_language_override(Some("ja-JP".to_string()));
        assert_eq!(display_language(), "ja-JP");
        apply_display_language_override(None);
    }
}
