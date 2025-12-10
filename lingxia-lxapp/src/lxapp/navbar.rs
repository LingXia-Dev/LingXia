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

/// NavigationBar configuration (immutable, from page.json)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NavigationBarConfig {
    #[serde(default)]
    pub navigation_bar_background_color: String,
    #[serde(default)]
    pub navigation_bar_text_style: String,
    #[serde(default)]
    pub navigation_bar_title_text: String,
    #[serde(default)]
    pub navigation_style: NavigationStyle,
}

/// NavigationBar runtime state (mutable, derived from config)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct NavigationBarState {
    // Configuration properties (initialized from NavigationBarConfig)
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

impl NavigationBarConfig {
    /// Check if navbar should be shown based on navigation style
    pub fn should_show_navbar(&self) -> bool {
        matches!(self.navigation_style, NavigationStyle::Default)
    }
}

impl Default for NavigationBarState {
    fn default() -> Self {
        Self::from_config(&NavigationBarConfig::default())
    }
}

impl NavigationBarState {
    /// Create NavigationBarState from NavigationBarConfig
    pub fn from_config(config: &NavigationBarConfig) -> Self {
        Self {
            navigationBarBackgroundColor: config.navigation_bar_background_color.clone(),
            navigationBarTextStyle: config.navigation_bar_text_style.clone(),
            navigationBarTitleText: config.navigation_bar_title_text.clone(),
            navigationStyle: config.navigation_style.clone(),
            show_navbar: config.should_show_navbar(),
            show_back_button: false,
            show_home_button: false,
        }
    }

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
