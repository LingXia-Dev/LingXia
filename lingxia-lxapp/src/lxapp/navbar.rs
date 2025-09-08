use crate::lxapp::LxApp;
use serde::{Deserialize, Serialize};

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

/// NavigationBar state for a specific page
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct NavigationBarState {
    // Configuration properties (loaded from JSON)
    #[serde(default)]
    pub navigationBarBackgroundColor: String,
    #[serde(default)]
    pub navigationBarTextStyle: String,
    #[serde(default)]
    pub navigationBarTitleText: String,
    #[serde(default)]
    pub navigationStyle: NavigationStyle,

    // Runtime state (not serialized)
    #[serde(skip)]
    pub show_navbar: bool,
    #[serde(skip)]
    pub show_back_button: bool,
    #[serde(skip)]
    pub show_home_button: bool,
}

impl Default for NavigationBarState {
    fn default() -> Self {
        Self {
            navigationBarBackgroundColor: String::new(),
            navigationBarTextStyle: String::new(),
            navigationBarTitleText: String::new(),
            navigationStyle: NavigationStyle::Default,
            show_navbar: true,
            show_back_button: false,
            show_home_button: false,
        }
    }
}

impl NavigationBarState {
    /// Set back button visibility
    pub fn set_back_button_visibility(&mut self, show: bool) {
        self.show_back_button = show;
    }

    /// Set home button visibility
    pub fn set_home_button_visibility(&mut self, show: bool) {
        self.show_home_button = show;
    }

    /// Set navbar visibility
    pub fn set_navbar_visibility(&mut self, show: bool) {
        self.show_navbar = show;
    }

    /// Set title
    pub fn set_title(&mut self, title: String) {
        self.navigationBarTitleText = title;
    }

    /// Set background color
    pub fn set_background_color(&mut self, color: String) {
        self.navigationBarBackgroundColor = color;
    }

    /// Set text style
    pub fn set_text_style(&mut self, style: String) {
        self.navigationBarTextStyle = style;
    }
}

impl NavigationBarState {
    /// Create NavigationBarState from JSON config file path
    pub fn from_json(lxapp: &LxApp, path: &str) -> Self {
        let json_path = path_to_json_path(path);
        match lxapp.read_json(&json_path) {
            Ok(json_value) => {
                match serde_json::from_value::<NavigationBarState>(json_value) {
                    Ok(mut state) => {
                        // Set navbar visibility based on navigationStyle
                        state.show_navbar = match state.navigationStyle {
                            NavigationStyle::Default => true,
                            NavigationStyle::Custom => false,
                        };

                        // Initialize button state (will be updated dynamically)
                        state.show_back_button = false;
                        state.show_home_button = false;
                        state
                    }
                    Err(_) => Self::default(),
                }
            }
            Err(_) => Self::default(),
        }
    }
}

/// Convert page path to JSON configuration path (visible within crate)
fn path_to_json_path(path: &str) -> String {
    if path.contains('.') {
        // Has extension: replace it with .json
        let pos = path.rfind('.').unwrap();
        format!("{}.json", &path[0..pos])
    } else {
        // No extension: append .json
        format!("{}.json", path)
    }
}

/// Extension methods for LxApp to handle NavigationBar state
impl LxApp {
    /// Get NavigationBar state for a specific page
    pub fn get_navbar_state(&self, path: &str) -> NavigationBarState {
        let state = self.state.lock().unwrap();
        if let Some(page) = state.pages.lock().unwrap().get(path) {
            // Always read from page state (initialized in create_page)
            page.get_navbar_state().unwrap_or_default()
        } else {
            // No page exists - fallback to JSON
            NavigationBarState::from_json(self, path)
        }
    }

    /// Update navbar state for a specific page
    pub fn update_navbar_state<F>(&self, path: &str, f: F) -> bool
    where
        F: FnOnce(&mut NavigationBarState),
    {
        let state = self.state.lock().unwrap();
        if let Some(page) = state.pages.lock().unwrap().get(path) {
            page.get_navbar_state_mut(f).is_some()
        } else {
            false
        }
    }
}
