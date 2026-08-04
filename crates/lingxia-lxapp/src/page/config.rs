use crate::LxAppError;
use crate::lxapp::LxApp;
use crate::lxapp::navbar::{NavigationBarConfig, NavigationBarState, NavigationStyle};
use crate::warn;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// PageInstance orientation configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum PageOrientation {
    /// Portrait orientation (vertical)
    #[default]
    Portrait,
    /// Landscape orientation (horizontal)
    Landscape,
    /// Auto - follow device orientation
    Auto,
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

/// PageInstance-level orientation overrides. Missing fields inherit from app defaults.
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

/// PageInstance configuration loaded from page.json (immutable)
/// This is the single source of truth for page configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageConfig {
    #[serde(default)]
    pub navigation_style: NavigationStyle,

    #[serde(default)]
    pub navigation_bar: NavigationBarConfig,

    /// Enable pull-to-refresh
    #[serde(default)]
    pub enable_pull_down_refresh: bool,

    /// PageInstance orientation override
    #[serde(default)]
    pub page_orientation: Option<PageOrientation>,
}

impl PageConfig {
    fn validate_removed_fields(path: &str, value: &Value) -> Result<(), serde_json::Error> {
        let Some(object) = value.as_object() else {
            return Ok(());
        };
        for (field, replacement) in [
            ("navigationBarTitleText", "navigationBar.title"),
            (
                "navigationBarBackgroundColor",
                "navigationBar.style.backgroundColor",
            ),
            (
                "navigationBarTextStyle",
                "navigationBar.style.foregroundColor",
            ),
        ] {
            if object.contains_key(field) {
                return Err(serde_json::Error::custom(format!(
                    "{path}.{field}: removed; use {replacement}"
                )));
            }
        }
        if object.contains_key("backgroundColor") {
            return Err(serde_json::Error::custom(format!(
                "{path}.backgroundColor: removed; page background is host-owned"
            )));
        }
        Ok(())
    }

    fn parse_page_orientation_value(value: &Value) -> Option<PageOrientation> {
        let raw = value.as_str()?;
        match raw.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(PageOrientation::Auto),
            "portrait" => Some(PageOrientation::Portrait),
            "landscape" => Some(PageOrientation::Landscape),
            _ => None,
        }
    }

    fn sanitize_page_orientation(path: &str, json_value: &mut Value) {
        let Some(obj) = json_value.as_object_mut() else {
            return;
        };

        let Some(raw_orientation) = obj.get("pageOrientation").cloned() else {
            return;
        };

        if Self::parse_page_orientation_value(&raw_orientation).is_none() {
            warn!(
                "Ignoring invalid pageOrientation for {}: {:?}",
                path, raw_orientation
            );
            obj.remove("pageOrientation");
        }
    }

    pub(crate) fn from_value(path: &str, mut value: Value) -> Result<Self, serde_json::Error> {
        Self::validate_removed_fields(path, &value)?;
        Self::sanitize_page_orientation(path, &mut value);
        let config = serde_json::from_value::<PageConfig>(value)?;
        config
            .navigation_bar
            .style
            .validate(&format!("{path}.navigationBar"))
            .map_err(serde_json::Error::custom)?;
        Ok(config)
    }

    /// Create PageConfig from JSON config file path.
    /// Missing files use defaults; malformed files are hard page-load errors.
    pub fn from_json(lxapp: &LxApp, path: &str) -> Result<Self, LxAppError> {
        if path.trim().is_empty() {
            return Ok(Self::default());
        }

        let json_path = path_to_json_path(path);
        match lxapp.read_json(&json_path) {
            Ok(value) => Self::from_value(&json_path, value).map_err(|error| {
                let detail = error.to_string();
                let detail = if detail.starts_with(&json_path) {
                    detail
                } else {
                    format!("{json_path}: {detail}")
                };
                LxAppError::InvalidJsonFile(detail)
            }),
            Err(LxAppError::ResourceNotFound(_)) => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    /// Create NavigationBarState from this config
    /// This converts immutable config to mutable runtime state.
    pub fn create_navbar_state(&self) -> NavigationBarState {
        NavigationBarState::from_config(self.navigation_style, &self.navigation_bar)
    }

    /// Check if pull-to-refresh is enabled
    pub fn is_pull_down_refresh_enabled(&self) -> bool {
        self.enable_pull_down_refresh
    }

    /// Get page-level orientation overrides
    pub fn get_orientation_override(&self) -> OrientationOverride {
        match self.page_orientation {
            Some(mode) => OrientationOverride {
                mode: Some(mode),
                rotation: Some(0),
            },
            None => OrientationOverride::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_removed_page_fields_with_actionable_diagnostics() {
        let value = serde_json::json!({
            "navigationStyle": "custom",
            "navigationBarTitleText": "Legacy"
        });
        let error = PageConfig::from_value("pages/home/index.json", value).unwrap_err();
        assert!(error.to_string().contains(
            "pages/home/index.json.navigationBarTitleText: removed; use navigationBar.title"
        ));

        let error = PageConfig::from_value(
            "pages/home/index.json",
            serde_json::json!({"backgroundColor": "#FFFFFF"}),
        )
        .unwrap_err();
        assert!(error.to_string().contains(
            "pages/home/index.json.backgroundColor: removed; page background is host-owned"
        ));
    }

    #[test]
    fn rejects_translucent_manifest_navigation_colors() {
        let error = PageConfig::from_value(
            "pages/home/index.json",
            serde_json::json!({
                "navigationBar": {"style": {"backgroundColor": "#FFFFFF80"}}
            }),
        )
        .unwrap_err();
        assert!(error.to_string().contains(
            "pages/home/index.json.navigationBar.style.backgroundColor: expected opaque #RRGGBB"
        ));
    }

    #[test]
    fn keeps_custom_navigation_when_valid() {
        let config = PageConfig::from_value(
            "pages/home/index.json",
            serde_json::json!({
                "navigationStyle": "custom",
                "navigationBar": {"title": "Home"}
            }),
        )
        .unwrap();
        assert_eq!(config.navigation_style, NavigationStyle::Custom);
        assert_eq!(config.navigation_bar.title, "Home");
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
        if last_slash.is_none_or(|slash| dot_pos > slash) {
            trimmed.truncate(dot_pos);
        }
    }

    format!("{}.json", trimmed)
}
