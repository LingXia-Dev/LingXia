use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Navigation style enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum NavigationStyle {
    /// Default navigation style (show navigation bar)
    #[serde(rename = "default")]
    #[default]
    Default,

    /// Custom navigation style (hide navigation bar)
    #[serde(rename = "custom")]
    Custom,
}

impl NavigationStyle {
    /// Convert to i32 for FFI
    pub fn to_i32(&self) -> i32 {
        match self {
            NavigationStyle::Default => 0,
            NavigationStyle::Custom => 1,
        }
    }
}

/// NavigationBar configuration for a specific page
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(non_snake_case)]
pub struct NavigationBarConfig {
    /// Navigation bar background color
    #[serde(default)]
    pub navigationBarBackgroundColor: String,

    /// Navigation bar text color, can be "black" or "white"
    #[serde(default)]
    pub navigationBarTextStyle: String,

    /// Navigation bar title
    #[serde(default)]
    pub navigationBarTitleText: String,

    /// Navigation style (default or custom)
    #[serde(default)]
    pub navigationStyle: NavigationStyle,
}

impl NavigationBarConfig {
    /// Create NavigationBarConfig from serde_json::Value
    pub fn from_value(value: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }
}
