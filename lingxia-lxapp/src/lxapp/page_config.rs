use crate::lxapp::LxApp;
use crate::lxapp::navbar::{NavigationBarConfig, NavigationBarState};
use crate::warn;
use serde::{Deserialize, Deserializer, Serialize};

/// Page orientation configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PageOrientation {
    /// Portrait orientation (vertical)
    Portrait,
    /// Landscape orientation (horizontal)
    Landscape,
    /// Auto - follow device orientation
    Auto,
}

impl Default for PageOrientation {
    fn default() -> Self {
        Self::Portrait
    }
}

/// App-level orientation configuration with optional 180-degree rotation.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OrientationConfig {
    #[serde(default)]
    pub mode: PageOrientation,
    #[serde(default)]
    pub rotation: u16,
}

impl Default for OrientationConfig {
    fn default() -> Self {
        Self {
            mode: PageOrientation::Portrait,
            rotation: 0,
        }
    }
}

impl OrientationConfig {
    pub fn normalize(mode: PageOrientation, rotation: u16) -> Self {
        let rotation = match rotation {
            0 | 180 => rotation,
            _ => 0,
        };
        let rotation = if matches!(mode, PageOrientation::Auto) {
            0
        } else {
            rotation
        };
        Self { mode, rotation }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label.trim().to_lowercase().as_str() {
            "auto" => Some(Self::normalize(PageOrientation::Auto, 0)),
            "portrait" => Some(Self::normalize(PageOrientation::Portrait, 0)),
            "landscape" => Some(Self::normalize(PageOrientation::Landscape, 0)),
            "reverse-portrait" => Some(Self::normalize(PageOrientation::Portrait, 180)),
            "reverse-landscape" => Some(Self::normalize(PageOrientation::Landscape, 180)),
            _ => None,
        }
    }

    pub fn to_label(self) -> &'static str {
        match (self.mode, self.rotation) {
            (PageOrientation::Auto, _) => "auto",
            (PageOrientation::Portrait, 180) => "reverse-portrait",
            (PageOrientation::Portrait, _) => "portrait",
            (PageOrientation::Landscape, 180) => "reverse-landscape",
            (PageOrientation::Landscape, _) => "landscape",
        }
    }
}

/// Page-level orientation overrides. Missing fields inherit from app defaults.
#[derive(Debug, Clone, Copy, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OrientationOverride {
    #[serde(default)]
    pub mode: Option<PageOrientation>,
    #[serde(default)]
    pub rotation: Option<u16>,
}

impl OrientationOverride {
    pub fn apply(self, base: OrientationConfig) -> OrientationConfig {
        let mode = self.mode.unwrap_or(base.mode);
        let rotation = self.rotation.unwrap_or(base.rotation);
        OrientationConfig::normalize(mode, rotation)
    }
}

// Shared deserialization helper for orientation types
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrientationObject {
    #[serde(default)]
    mode: Option<PageOrientation>,
    #[serde(default)]
    rotation: Option<u16>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OrientationInput {
    Label(String),
    Object(OrientationObject),
}

fn deserialize_orientation<'de, D>(
    deserializer: D,
) -> Result<(Option<PageOrientation>, Option<u16>), D::Error>
where
    D: Deserializer<'de>,
{
    let input = OrientationInput::deserialize(deserializer)?;
    match input {
        OrientationInput::Label(label) => {
            let config = OrientationConfig::from_label(&label).ok_or_else(|| {
                serde::de::Error::custom(format!("invalid orientation: {}", label))
            })?;
            Ok((Some(config.mode), Some(config.rotation)))
        }
        OrientationInput::Object(obj) => Ok((obj.mode, obj.rotation)),
    }
}

impl<'de> Deserialize<'de> for OrientationConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (mode, rotation) = deserialize_orientation(deserializer)?;
        Ok(Self::normalize(
            mode.unwrap_or_default(),
            rotation.unwrap_or_default(),
        ))
    }
}

impl<'de> Deserialize<'de> for OrientationOverride {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (mode, rotation) = deserialize_orientation(deserializer)?;
        Ok(Self { mode, rotation })
    }
}

/// Page configuration loaded from page.json (immutable)
/// This is the single source of truth for page configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PageConfig {
    /// Navigation bar configuration
    #[serde(flatten)]
    pub navigation_bar: NavigationBarConfig,

    /// Enable pull-to-refresh
    #[serde(default)]
    pub enable_pull_down_refresh: bool,

    /// Legacy page orientation
    #[serde(default)]
    pub page_orientation: Option<PageOrientation>,

    /// Orientation overrides
    #[serde(default)]
    pub orientation: OrientationOverride,
}

impl PageConfig {
    /// Create PageConfig from JSON config file path
    /// This is the single entry point for loading page configuration.
    pub fn from_json(lxapp: &LxApp, path: &str) -> Self {
        let json_path = path_to_json_path(path);
        match lxapp.read_json(&json_path) {
            Ok(json_value) => match serde_json::from_value::<PageConfig>(json_value) {
                Ok(config) => config,
                Err(e) => {
                    warn!("Failed to parse page config for {}: {}", path, e);
                    Self::default()
                }
            },
            Err(e) => {
                warn!(
                    "Page config read failed for {} ({}); falling back to default",
                    path, e
                );
                // No page config file or read error - use default (navbar enabled, no pull-to-refresh)
                Self::default()
            }
        }
    }

    /// Create NavigationBarState from this config
    /// This converts immutable config to mutable runtime state.
    pub fn create_navbar_state(&self) -> NavigationBarState {
        NavigationBarState::from_config(&self.navigation_bar)
    }

    /// Check if pull-to-refresh is enabled
    pub fn is_pull_down_refresh_enabled(&self) -> bool {
        self.enable_pull_down_refresh
    }

    /// Get page-level orientation overrides
    pub fn get_orientation_override(&self) -> OrientationOverride {
        OrientationOverride {
            mode: self.orientation.mode.or(self.page_orientation),
            rotation: self.orientation.rotation,
        }
    }
}

/// Convert a page path to its corresponding JSON config path
fn path_to_json_path(path: &str) -> String {
    if path.is_empty() || path == "/" {
        return "pages/index/index.json".to_string();
    }

    let mut trimmed = path.trim_start_matches('/').to_string();
    if trimmed.is_empty() {
        return "pages/index/index.json".to_string();
    }

    // Remove any extension on the last path segment
    if let Some(dot_pos) = trimmed.rfind('.') {
        let last_slash = trimmed.rfind('/');
        if last_slash.map_or(true, |slash| dot_pos > slash) {
            trimmed.truncate(dot_pos);
        }
    }

    format!("{}.json", trimmed)
}
