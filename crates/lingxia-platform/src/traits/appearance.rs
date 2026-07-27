use serde::{Deserialize, Serialize};

use crate::PlatformError;

/// User-selected host appearance. `System` follows the operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppearancePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl AppearancePreference {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

impl std::str::FromStr for AppearancePreference {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "system" => Ok(Self::System),
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            other => Err(format!(
                "unknown appearance preference: {other} (expected system|light|dark)"
            )),
        }
    }
}

/// Current host appearance, separating the persisted preference from the
/// concrete scheme native chrome and WebViews render.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceState {
    pub preference: AppearancePreference,
    pub effective_dark: bool,
}

impl Default for AppearanceState {
    fn default() -> Self {
        Self {
            preference: AppearancePreference::System,
            effective_dark: false,
        }
    }
}

impl AppearanceState {
    pub fn effective(self) -> &'static str {
        if self.effective_dark { "dark" } else { "light" }
    }
}

/// Host-owned appearance control. Native platforms apply the preference to
/// their window/view hierarchy so every native component and WebView changes
/// in one transaction.
pub trait Appearance: Send + Sync + 'static {
    fn get_appearance(&self) -> Result<AppearanceState, PlatformError> {
        Err(PlatformError::NotSupported(
            "appearance is not supported on this platform".to_string(),
        ))
    }

    fn set_appearance(
        &self,
        _preference: AppearancePreference,
    ) -> Result<AppearanceState, PlatformError> {
        Err(PlatformError::NotSupported(
            "appearance is not supported on this platform".to_string(),
        ))
    }

    fn add_appearance_change_listener(&self, _callback_id: u64) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "appearance events are not supported on this platform".to_string(),
        ))
    }

    fn remove_appearance_change_listener(&self, _callback_id: u64) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "appearance events are not supported on this platform".to_string(),
        ))
    }
}
