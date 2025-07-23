use serde::{Deserialize, Serialize};
use std::path::Path;

/// TabBar configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct TabBarConfig {
    /// Text color (color value)
    #[serde(default)]
    pub color: String,

    /// Text color when selected (color value)
    #[serde(default)]
    pub selectedColor: String,

    /// Background color (color value)
    #[serde(default = "default_background_color")]
    pub backgroundColor: String,

    /// Border color of the top of the tab bar (color value)
    #[serde(default)]
    pub borderStyle: String,

    /// List of tab items
    pub list: Vec<TabItem>,

    /// Position of the tab bar, can be "bottom" or "top"
    #[serde(default)]
    pub position: TabBarPosition,

    /// Dimension in dp (height for bottom/top, width for left/right)
    #[serde(default = "default_dimension")]
    pub dimension: i32,
}

fn default_background_color() -> String {
    "#ffffff".to_string()
}

fn default_dimension() -> i32 {
    64 // Default height/width in dp
}

/// Position of the tab bar
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TabBarPosition {
    /// Tab bar at the bottom (default)
    #[serde(rename = "bottom")]
    #[default]
    Bottom,

    /// Tab bar at the top
    #[serde(rename = "top")]
    Top,

    /// Tab bar at the left
    #[serde(rename = "left")]
    Left,

    /// Tab bar at the right
    #[serde(rename = "right")]
    Right,
}

/// Tab item in the tab bar
///
/// Each tab item represents a button in the tabbar with text and icons.
///
/// ## Icon Paths
/// Both `iconPath` and `selectedIconPath` should be relative paths in the configuration file:
///
/// - All paths are relative to the lxapp's own directory
/// - The framework will automatically convert these to absolute paths when needed
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct TabItem {
    /// Page path for this tab
    pub pagePath: String,

    /// Text displayed under the icon (optional, if not provided, no text will be shown)
    #[serde(default)]
    pub text: Option<String>,

    /// Path to the icon when not selected
    #[serde(default)]
    pub iconPath: Option<String>,

    /// Path to the icon when selected
    #[serde(default)]
    pub selectedIconPath: Option<String>,

    /// Whether this tab is selected by default
    #[serde(default)]
    pub selected: bool,
}

impl TabBarConfig {
    /// Minimum number of tab items required by WeChat
    pub const MIN_ITEMS: usize = 2;

    /// Maximum number of tab items allowed by WeChat
    pub const MAX_ITEMS: usize = 5;

    /// Check if this TabBar is valid (has enough items to be displayed)
    /// and doesn't exceed the maximum number of allowed items
    pub fn is_valid(&self) -> bool {
        let count = self.list.len();
        (Self::MIN_ITEMS..=Self::MAX_ITEMS).contains(&count)
    }

    /// Convert all icon paths in the tabbar to absolute paths
    ///
    /// This method converts all icon paths in the list items to absolute paths
    /// by prepending the provided base_path. All paths in the configuration file
    /// are expected to be relative to the lxapp's own directory.
    ///
    /// # Arguments
    /// * `base_path` - The lxapp's directory path to prepend to relative paths
    ///
    /// # Returns
    /// A new TabBar instance with all paths converted to absolute paths
    pub fn with_absolute_paths(&self, base_path: &Path) -> Self {
        let mut result = self.clone();

        for item in &mut result.list {
            // Process iconPath if it exists
            if let Some(icon_path) = &item.iconPath {
                // Convert relative path to absolute
                let abs_path = base_path.join(icon_path);
                item.iconPath = Some(abs_path.to_string_lossy().to_string());
            } else {
                // If iconPath is None, set it to empty string
                item.iconPath = Some("".to_string());
            }

            // Process selectedIconPath if it exists
            if let Some(selected_icon_path) = &item.selectedIconPath {
                // Convert relative path to absolute
                let abs_path = base_path.join(selected_icon_path);
                item.selectedIconPath = Some(abs_path.to_string_lossy().to_string());
            } else {
                // If selectedIconPath is None, copy the iconPath
                item.selectedIconPath = item.iconPath.clone();
            }
        }

        result
    }
}
