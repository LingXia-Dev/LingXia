use crate::LxApp;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TabBarPosition {
    #[serde(rename = "bottom")]
    #[default]
    Bottom,
    #[serde(rename = "left")]
    Left,
    #[serde(rename = "right")]
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TabItemGroup {
    #[serde(rename = "start")]
    Start,
    #[serde(rename = "end")]
    End,
}

impl TabBarPosition {
    pub fn to_i32(&self) -> i32 {
        match self {
            TabBarPosition::Bottom => 0,
            TabBarPosition::Left => 1,
            TabBarPosition::Right => 2,
        }
    }
}

/// TabBar (unified config and runtime state)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct TabBar {
    #[serde(default = "default_unselected_color")]
    pub color: String,
    #[serde(default = "default_selected_color")]
    pub selectedColor: String,
    #[serde(default = "default_background_color")]
    pub backgroundColor: String,
    #[serde(default = "default_border_color")]
    pub borderStyle: String,
    pub list: Vec<TabBarItem>,
    #[serde(default)]
    pub position: TabBarPosition,
    #[serde(default = "default_dimension")]
    pub dimension: i32,

    // Runtime state (not from JSON)
    #[serde(skip)]
    pub is_visible: bool,
    #[serde(skip)]
    pub selected_index: i32,
}

/// Tab item (unified config and runtime state)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct TabBarItem {
    pub pagePath: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub iconPath: Option<String>,
    #[serde(default)]
    pub selectedIconPath: Option<String>,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub group: Option<TabItemGroup>,

    // Runtime state (not from JSON)
    #[serde(skip)]
    pub badge: Option<String>,
    #[serde(skip)]
    pub has_red_dot: bool,
}

impl TabBar {
    pub const MIN_ITEMS: usize = 2;
    pub const MAX_ITEMS: usize = 5;

    /// Check if TabBar is valid
    pub fn is_valid(&self) -> bool {
        let count = self.list.len();
        (Self::MIN_ITEMS..=Self::MAX_ITEMS).contains(&count)
    }

    /// Convert all icon paths to absolute paths
    pub(crate) fn with_absolute_paths(&self, base_path: &Path) -> Self {
        let mut result = self.clone();

        // Initialize runtime state
        result.is_visible = true;
        result.selected_index = 0; // Default to first tab

        for item in &mut result.list {
            // Process iconPath
            if let Some(icon_path) = &item.iconPath {
                let abs_path = base_path.join(icon_path);
                item.iconPath = Some(abs_path.to_string_lossy().to_string());
            } else {
                item.iconPath = Some("".to_string());
            }

            // Process selectedIconPath
            if let Some(selected_icon_path) = &item.selectedIconPath {
                let abs_path = base_path.join(selected_icon_path);
                item.selectedIconPath = Some(abs_path.to_string_lossy().to_string());
            } else {
                item.selectedIconPath = item.iconPath.clone();
            }

            // Initialize runtime state
            item.badge = None;
            item.has_red_dot = false;
        }

        result
    }

    /// Get specific TabBar item
    pub fn get_item(&self, index: i32) -> Option<&TabBarItem> {
        self.list.get(index as usize)
    }

    /// Set TabBar visibility
    pub fn set_visible(&mut self, visible: bool) {
        self.is_visible = visible;
    }

    /// Set badge for a specific tab
    pub fn set_badge(&mut self, index: i32, text: &str) -> bool {
        if let Some(item) = self.list.get_mut(index as usize) {
            if text.is_empty() {
                item.badge = None;
            } else {
                item.badge = Some(text.to_string());
            }
            true
        } else {
            false
        }
    }

    /// Remove badge from a specific tab
    pub fn remove_badge(&mut self, index: i32) -> bool {
        if let Some(item) = self.list.get_mut(index as usize) {
            item.badge = None;
            true
        } else {
            false
        }
    }

    /// Set red dot for a specific tab
    pub fn set_red_dot(&mut self, index: i32, show: bool) -> bool {
        if let Some(item) = self.list.get_mut(index as usize) {
            item.has_red_dot = show;
            true
        } else {
            false
        }
    }

    /// Set TabBar text color (chainable)
    pub fn set_color(&mut self, color: &str) -> &mut Self {
        self.color = color.to_string();
        self
    }

    /// Set TabBar selected text color (chainable)
    pub fn set_selected_color(&mut self, color: &str) -> &mut Self {
        self.selectedColor = color.to_string();
        self
    }

    /// Set TabBar background color (chainable)
    pub fn set_background_color(&mut self, color: &str) -> &mut Self {
        self.backgroundColor = color.to_string();
        self
    }

    /// Set TabBar border style (chainable)
    pub fn set_border_style(&mut self, style: &str) -> &mut Self {
        self.borderStyle = style.to_string();
        self
    }

    /// Set TabBar position (chainable)
    pub fn set_position(&mut self, position: TabBarPosition) -> &mut Self {
        self.position = position;
        self
    }

    /// Set TabBar dimension (chainable)
    pub fn set_dimension(&mut self, dimension: i32) -> &mut Self {
        self.dimension = dimension;
        self
    }

    /// Get mutable reference to item by index
    pub fn get_item_mut(&mut self, index: i32) -> Option<&mut TabBarItem> {
        self.list.get_mut(index as usize)
    }

    /// Get reference to item by index
    pub fn get_item_ref(&self, index: i32) -> Option<&TabBarItem> {
        self.list.get(index as usize)
    }

    /// Set item text by index (chainable)
    pub fn set_item_text(&mut self, index: i32, text: &str) -> &mut Self {
        if let Some(item) = self.get_item_mut(index) {
            item.text = Some(text.to_string());
        }
        self
    }

    /// Set item icon by index (chainable)
    pub fn set_item_icon(&mut self, index: i32, icon_path: &str) -> &mut Self {
        if let Some(item) = self.get_item_mut(index) {
            item.iconPath = Some(icon_path.to_string());
        }
        self
    }

    /// Set item selected icon by index (chainable)
    pub fn set_item_selected_icon(&mut self, index: i32, icon_path: &str) -> &mut Self {
        if let Some(item) = self.get_item_mut(index) {
            item.selectedIconPath = Some(icon_path.to_string());
        }
        self
    }

    /// Set item badge by index (chainable)
    pub fn set_item_badge(&mut self, index: i32, badge: &str) -> &mut Self {
        if let Some(item) = self.get_item_mut(index) {
            item.badge = Some(badge.to_string());
        }
        self
    }

    /// Clear item badge by index (chainable)
    pub fn clear_item_badge(&mut self, index: i32) -> &mut Self {
        if let Some(item) = self.get_item_mut(index) {
            item.badge = None;
        }
        self
    }

    /// Set item red dot by index (chainable)
    pub fn set_item_red_dot(&mut self, index: i32, show: bool) -> &mut Self {
        if let Some(item) = self.get_item_mut(index) {
            item.has_red_dot = show;
        }
        self
    }

    /// Get selected index
    pub fn get_selected_index(&self) -> i32 {
        self.selected_index
    }

    /// Set selected index (chainable)
    pub fn set_selected_index(&mut self, index: i32) -> &mut Self {
        if index >= 0 && (index as usize) < self.list.len() {
            self.selected_index = index;
        }
        self
    }

    /// Find tab index by page path
    pub fn find_index_by_path(&self, path: &str) -> Option<i32> {
        self.list
            .iter()
            .position(|item| item.pagePath == path)
            .map(|i| i as i32)
    }

    /// Check if a path is a tabbar item
    pub fn is_tabbar_page(&self, path: &str) -> bool {
        self.list.iter().any(|item| item.pagePath == path)
    }

    /// Get all tabBar page paths
    pub fn get_tabbar_pages(&self) -> Vec<String> {
        self.list.iter().map(|item| item.pagePath.clone()).collect()
    }
}

// Default functions for serde
fn default_selected_color() -> String {
    "#1677FF".to_string()
}
fn default_unselected_color() -> String {
    "#666666".to_string()
}
fn default_background_color() -> String {
    "#ffffff".to_string()
}
fn default_border_color() -> String {
    "#F0F0F0".to_string()
}
fn default_dimension() -> i32 {
    64
}

impl LxApp {
    /// Get TabBar state
    /// Returns None if TabBar is not configured or invalid
    pub fn get_tabbar(&self) -> Option<TabBar> {
        let state = self.state.lock().unwrap();
        state
            .tabbar
            .as_ref()
            .filter(|tabbar| tabbar.is_valid())
            .cloned()
    }

    /// Get specific TabBar item
    /// Returns None if TabBar is not configured, invalid, or index is out of bounds
    pub fn get_tabbar_item(&self, index: i32) -> Option<TabBarItem> {
        let state = self.state.lock().unwrap();
        let tabbar = state.tabbar.as_ref().filter(|tabbar| tabbar.is_valid())?;
        tabbar.get_item(index).cloned()
    }

    /// Execute operation on TabBar with mutable access
    /// Returns None if TabBar is not configured or invalid
    pub fn with_tabbar_mut<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut TabBar) -> R,
    {
        let mut state = self.state.lock().unwrap();
        if let Some(ref mut tabbar) = state.tabbar {
            if tabbar.is_valid() {
                Some(f(tabbar))
            } else {
                None
            }
        } else {
            None
        }
    }
}
