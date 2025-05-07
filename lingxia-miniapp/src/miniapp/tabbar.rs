use serde::{Deserialize, Serialize};
use std::path::Path;

/// Example TabBar Configuration
/// ```json
/// {
///     "backgroundColor": "#ffffff",
///     "selectedColor": "#1677ff",
///     "color": "#666666",
///     "borderStyle": "#f0f0f0",
///     "list": [
///         {
///             "text": "首页",
///             "pagePath": "pages/home/index.html",
///             "iconPath": "imags/home.png",
///             "selectedIconPath": "imags/home_selected.png",
///             "selected": true
///         },
///         {
///             "text": "消息",
///             "pagePath": "pages/message/index.html",
///             "iconPath": "imags/message.png",
///             "selectedIconPath": "imags/message_selected.png"
///         },
///         {
///             "text": "我的",
///             "pagePath": "pages/profile/index.html",
///             "iconPath": "imags/profile.png",
///             "selectedIconPath": "imags/profile_selected.png"
///         }
///     ]
/// }
/// ```
///
/// ## Tab Item Requirements
/// The `list` field must contain:
/// - At least 2 items (minimum)
/// - At most 5 items (maximum)
///
/// If these requirements are not met, the tabbar will not be displayed.
///
/// ## Icon Path Format
/// The `iconPath` and `selectedIconPath` fields in TabItem should always be relative paths
/// in the configuration file, relative to the miniapp's own directory. For example:
///
/// - `imags/tabbar/home.png` - Icon in the imags directory of the miniapp
///
/// The framework automatically converts these relative paths to absolute paths when needed
/// by prepending the miniapp's directory path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub(crate) struct TabBar {
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

    /// Whether the tab bar is on a transparent background
    #[serde(default)]
    pub custom: bool,

    /// TabBar visibility, default true
    #[serde(default = "default_visible")]
    pub visible: bool,

    /// Height in dp, default is platform specific
    #[serde(default)]
    pub height: Option<i32>,
}

fn default_background_color() -> String {
    "#ffffff".to_string()
}

fn default_visible() -> bool {
    true
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
}

/// Tab item in the tab bar
///
/// Each tab item represents a button in the tabbar with text and icons.
///
/// ## Icon Paths
/// Both `iconPath` and `selectedIconPath` should be relative paths in the configuration file:
///
/// - All paths are relative to the miniapp's own directory
/// - The framework will automatically convert these to absolute paths when needed
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct TabItem {
    /// Page path for this tab
    pub pagePath: String,

    /// Text displayed under the icon
    pub text: String,

    /// Path to the icon when not selected
    pub iconPath: Option<String>,

    /// Path to the icon when selected
    pub selectedIconPath: Option<String>,

    /// Whether this tab is selected by default
    #[serde(default)]
    pub selected: bool,

    /// Whether this tab is visible
    #[serde(default = "default_visible")]
    pub visible: bool,
}

impl TabBar {
    /// Minimum number of tab items required by WeChat
    pub const MIN_ITEMS: usize = 2;

    /// Maximum number of tab items allowed by WeChat
    pub const MAX_ITEMS: usize = 5;

    /// Check if this TabBar is valid (has enough items to be displayed)
    /// and doesn't exceed the maximum number of allowed items
    pub fn is_valid(&self) -> bool {
        let count = self.list.len();
        count >= Self::MIN_ITEMS && count <= Self::MAX_ITEMS
    }

    /// Check if a path is a tab page
    pub fn is_tab_page(&self, path: &str) -> bool {
        self.list.iter().any(|item| item.pagePath == path)
    }

    /// Get all tab page paths
    pub fn get_tab_pages(&self) -> Vec<String> {
        self.list.iter().map(|item| item.pagePath.clone()).collect()
    }

    /// Get a tab item by path
    pub fn get_tab_by_path(&self, path: &str) -> Option<&TabItem> {
        self.list.iter().find(|item| item.pagePath == path)
    }

    /// Parse a tabbar configuration from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Convert all icon paths in the tabbar to absolute paths
    ///
    /// This method converts all icon paths in the list items to absolute paths
    /// by prepending the provided base_path. All paths in the configuration file
    /// are expected to be relative to the miniapp's own directory.
    ///
    /// # Arguments
    /// * `base_path` - The miniapp's directory path to prepend to relative paths
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
            }

            // Process selectedIconPath if it exists
            if let Some(selected_icon_path) = &item.selectedIconPath {
                // Convert relative path to absolute
                let abs_path = base_path.join(selected_icon_path);
                item.selectedIconPath = Some(abs_path.to_string_lossy().to_string());
            }
        }

        result
    }

    /// Convert to a JSON string with absolute paths
    ///
    /// # Arguments
    /// * `base_path` - Base path for resolving relative paths
    ///
    /// # Returns
    /// JSON string representation with absolute paths
    pub fn to_json_with_absolute_paths(
        &self,
        base_path: &Path,
    ) -> Result<String, serde_json::Error> {
        let tab_bar_with_abs_paths = self.with_absolute_paths(base_path);
        serde_json::to_string(&tab_bar_with_abs_paths)
    }
}
