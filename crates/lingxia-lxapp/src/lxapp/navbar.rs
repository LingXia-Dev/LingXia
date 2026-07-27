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
    /// Optional appearance-specific overrides. Missing fields inherit the
    /// common navigationBar* value, then the native semantic default.
    #[serde(default)]
    pub navigation_bar_style: NavigationBarThemeStyle,
    #[serde(default)]
    pub navigation_bar_title_text: String,
    #[serde(default)]
    pub navigation_style: NavigationStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NavigationBarStyle {
    #[serde(default)]
    pub background_color: String,
    #[serde(default)]
    pub text_style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NavigationBarThemeStyle {
    #[serde(default)]
    pub light: NavigationBarStyle,
    #[serde(default)]
    pub dark: NavigationBarStyle,
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

    #[serde(skip)]
    theme_style: NavigationBarThemeStyle,

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
            theme_style: config.navigation_bar_style.clone(),
            show_navbar: config.should_show_navbar(),
            show_back_button: false,
            show_home_button: false,
        }
    }

    /// Whether the page uses a custom (immersive) navigation style: the native
    /// navigation bar is hidden and the page draws its own header, so the host
    /// should let WebView content bleed up under the status bar / navbar strip
    /// instead of reserving a top inset for them.
    pub fn is_custom_navigation(&self) -> bool {
        matches!(self.navigationStyle, NavigationStyle::Custom)
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
        self.theme_style.light.background_color.clear();
        self.theme_style.dark.background_color.clear();
    }

    /// Set text style
    pub fn set_text_style(&mut self, style: String) {
        self.navigationBarTextStyle = style;
        self.theme_style.light.text_style.clear();
        self.theme_style.dark.text_style.clear();
    }

    pub fn resolved(&self, dark: bool) -> Self {
        let mut resolved = self.clone();
        let themed = if dark {
            &self.theme_style.dark
        } else {
            &self.theme_style.light
        };
        if !themed.background_color.is_empty() {
            resolved.navigationBarBackgroundColor = themed.background_color.clone();
        }
        if !themed.text_style.is_empty() {
            resolved.navigationBarTextStyle = themed.text_style.clone();
        }
        resolved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_appearance_style_over_common_values() {
        let config: NavigationBarConfig = serde_json::from_value(serde_json::json!({
            "navigationBarBackgroundColor": "#ffffff",
            "navigationBarTextStyle": "black",
            "navigationBarStyle": {
                "dark": { "backgroundColor": "#111418", "textStyle": "white" }
            }
        }))
        .unwrap();
        let state = NavigationBarState::from_config(&config);
        assert_eq!(
            state.resolved(false).navigationBarBackgroundColor,
            "#ffffff"
        );
        assert_eq!(state.resolved(true).navigationBarBackgroundColor, "#111418");
        assert_eq!(state.resolved(true).navigationBarTextStyle, "white");
    }

    #[test]
    fn imperative_color_override_applies_to_both_appearances() {
        let mut state = NavigationBarState::from_config(&NavigationBarConfig {
            navigation_bar_style: NavigationBarThemeStyle {
                dark: NavigationBarStyle {
                    background_color: "#111418".into(),
                    text_style: "white".into(),
                },
                ..Default::default()
            },
            ..Default::default()
        });
        state.set_background_color("#ff0000".into());
        state.set_text_style("black".into());
        assert_eq!(state.resolved(true).navigationBarBackgroundColor, "#ff0000");
        assert_eq!(state.resolved(true).navigationBarTextStyle, "black");
    }
}
