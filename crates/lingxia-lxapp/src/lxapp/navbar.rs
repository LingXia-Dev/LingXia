use super::page_chrome::{PageChromeColor, VisibilityPreference};
use lingxia_app_context::ThemeStyle;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum NavigationStyle {
    #[default]
    Default,
    Custom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NavigationBarStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<PageChromeColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground_color: Option<PageChromeColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divider_color: Option<PageChromeColor>,
}

impl NavigationBarStyle {
    pub fn validate(self, path: &str) -> Result<Self, String> {
        for (field, color) in [
            ("backgroundColor", self.background_color),
            ("foregroundColor", self.foreground_color),
        ] {
            if color.is_some_and(|color| !color.is_opaque()) {
                return Err(format!("{path}.style.{field}: expected opaque #RRGGBB"));
            }
        }
        Ok(self)
    }

    pub fn is_empty(self) -> bool {
        self == Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NavigationBarConfig {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub style: NavigationBarStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedNavigationBarStyle {
    pub background_color: PageChromeColor,
    pub foreground_color: PageChromeColor,
    pub divider_color: PageChromeColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedCapsuleStyle {
    pub background_color: PageChromeColor,
    pub foreground_color: PageChromeColor,
    pub divider_color: PageChromeColor,
    pub interaction_color: PageChromeColor,
}

impl ResolvedCapsuleStyle {
    fn resolve(theme: Option<&ThemeStyle>, defaults: Self) -> Self {
        Self {
            background_color: theme
                .and_then(|style| style.surface_background_color)
                .map(theme_color)
                .unwrap_or(defaults.background_color),
            foreground_color: theme
                .and_then(|style| style.foreground_color)
                .map(theme_color)
                .unwrap_or(defaults.foreground_color),
            divider_color: theme
                .and_then(|style| style.separator_color)
                .map(theme_color)
                .unwrap_or(defaults.divider_color),
            interaction_color: theme
                .and_then(|style| style.selection_background_color)
                .map(theme_color)
                .unwrap_or(defaults.interaction_color),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NavigationBarState {
    pub navigation_style: NavigationStyle,
    pub manifest: NavigationBarConfig,
    pub runtime_title: Option<String>,
    pub runtime_style: NavigationBarStyle,
    pub home_button: VisibilityPreference,

    #[serde(skip)]
    pub show_navbar: bool,
    #[serde(skip)]
    pub show_back_button: bool,
    #[serde(skip)]
    pub navigation_home_button: bool,
}

impl Default for NavigationBarState {
    fn default() -> Self {
        Self::from_config(NavigationStyle::Default, &NavigationBarConfig::default())
    }
}

impl NavigationBarState {
    pub fn from_config(style: NavigationStyle, config: &NavigationBarConfig) -> Self {
        Self {
            navigation_style: style,
            manifest: config.clone(),
            runtime_title: None,
            runtime_style: NavigationBarStyle::default(),
            home_button: VisibilityPreference::Auto,
            show_navbar: matches!(style, NavigationStyle::Default),
            show_back_button: false,
            navigation_home_button: false,
        }
    }

    pub fn is_custom_navigation(&self) -> bool {
        matches!(self.navigation_style, NavigationStyle::Custom)
    }

    pub fn title(&self) -> &str {
        self.runtime_title
            .as_deref()
            .unwrap_or(&self.manifest.title)
    }

    pub fn home_button_visible(&self) -> bool {
        self.show_navbar
            && self.home_button == VisibilityPreference::Auto
            && self.navigation_home_button
    }

    pub fn resolved_style(
        &self,
        theme: Option<&ThemeStyle>,
        defaults: ResolvedNavigationBarStyle,
    ) -> ResolvedNavigationBarStyle {
        let theme_background = theme.and_then(|style| style.surface_background_color);
        let theme_foreground = theme.and_then(|style| style.foreground_color);
        let theme_divider = theme.and_then(|style| style.separator_color);
        ResolvedNavigationBarStyle {
            background_color: self
                .runtime_style
                .background_color
                .or(self.manifest.style.background_color)
                .or_else(|| theme_background.map(theme_color))
                .unwrap_or(defaults.background_color),
            foreground_color: self
                .runtime_style
                .foreground_color
                .or(self.manifest.style.foreground_color)
                .or_else(|| theme_foreground.map(theme_color))
                .unwrap_or(defaults.foreground_color),
            divider_color: self
                .runtime_style
                .divider_color
                .or(self.manifest.style.divider_color)
                .or_else(|| theme_divider.map(theme_color))
                .unwrap_or(defaults.divider_color),
        }
    }

    pub fn set_back_button_visibility(&mut self, show: bool) {
        self.show_back_button = show;
    }

    pub fn set_home_button_visibility(&mut self, show: bool) {
        self.navigation_home_button = show;
    }

    pub fn set_navbar_visibility(&mut self, show: bool) {
        self.show_navbar = show;
    }

    pub fn set_runtime_title(&mut self, title: Option<String>) {
        self.runtime_title = title;
    }

    pub fn set_home_button_preference(&mut self, preference: VisibilityPreference) {
        self.home_button = preference;
    }

    pub fn clear_runtime_style(&mut self) {
        self.runtime_style = NavigationBarStyle::default();
    }
}

impl crate::LxApp {
    pub fn resolved_navigation_bar_style(&self, path: &str) -> ResolvedNavigationBarStyle {
        let state = self.get_navbar_state(path);
        let dark = self.appearance_state().resolved.is_dark();
        let theme = lingxia_app_context::theme().and_then(|theme| theme.style(dark));
        let defaults = if dark {
            ResolvedNavigationBarStyle {
                background_color: PageChromeColor::from_rgba(0x1C1C1EFF),
                foreground_color: PageChromeColor::from_rgba(0xFFFFFFFF),
                divider_color: PageChromeColor::from_rgba(0x38383AFF),
            }
        } else {
            ResolvedNavigationBarStyle {
                background_color: PageChromeColor::from_rgba(0xFFFFFFFF),
                foreground_color: PageChromeColor::from_rgba(0x000000FF),
                divider_color: PageChromeColor::from_rgba(0xE5E5EAFF),
            }
        };
        state.resolved_style(theme, defaults)
    }

    pub fn resolved_capsule_style(&self) -> ResolvedCapsuleStyle {
        let dark = self.appearance_state().resolved.is_dark();
        let theme = lingxia_app_context::theme().and_then(|theme| theme.style(dark));
        let defaults = if dark {
            ResolvedCapsuleStyle {
                background_color: PageChromeColor::from_rgba(0x1C1C1EFF),
                foreground_color: PageChromeColor::from_rgba(0xFFFFFFFF),
                divider_color: PageChromeColor::from_rgba(0x545458FF),
                interaction_color: PageChromeColor::from_rgba(0x3A3A3CFF),
            }
        } else {
            ResolvedCapsuleStyle {
                background_color: PageChromeColor::from_rgba(0xFFFFFFFF),
                foreground_color: PageChromeColor::from_rgba(0x000000FF),
                divider_color: PageChromeColor::from_rgba(0xD1D1D6FF),
                interaction_color: PageChromeColor::from_rgba(0xE5E5EAFF),
            }
        };
        ResolvedCapsuleStyle::resolve(theme, defaults)
    }
}

fn theme_color(color: lingxia_app_context::ThemeColor) -> PageChromeColor {
    PageChromeColor::from_rgba((color.rgb() << 8) | 0xff)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(value: &str) -> PageChromeColor {
        PageChromeColor::parse(value).unwrap()
    }

    #[test]
    fn runtime_manifest_theme_default_precedence() {
        let config = NavigationBarConfig {
            title: "Manifest".into(),
            style: NavigationBarStyle {
                foreground_color: Some(color("#222222")),
                ..Default::default()
            },
        };
        let mut state = NavigationBarState::from_config(NavigationStyle::Default, &config);
        state.runtime_style.background_color = Some(color("#111111"));
        let theme: ThemeStyle = serde_json::from_str(
            r##"{"surfaceBackgroundColor":"#AAAAAA","foregroundColor":"#BBBBBB","separatorColor":"#CCCCCC"}"##,
        )
        .unwrap();
        let resolved = state.resolved_style(
            Some(&theme),
            ResolvedNavigationBarStyle {
                background_color: color("#DDDDDD"),
                foreground_color: color("#EEEEEE"),
                divider_color: color("#FFFFFF"),
            },
        );
        assert_eq!(resolved.background_color, color("#111111"));
        assert_eq!(resolved.foreground_color, color("#222222"));
        assert_eq!(resolved.divider_color, color("#CCCCCC"));
    }

    #[test]
    fn hidden_home_button_cannot_be_forced_by_navigation() {
        let mut state = NavigationBarState::default();
        state.set_home_button_visibility(true);
        state.set_home_button_preference(VisibilityPreference::Hidden);
        assert!(!state.home_button_visible());
    }

    #[test]
    fn capsule_uses_semantic_theme_roles() {
        let theme: ThemeStyle = serde_json::from_str(
            r##"{
                "surfaceBackgroundColor":"#111111",
                "foregroundColor":"#222222",
                "separatorColor":"#333333",
                "selectionBackgroundColor":"#444444"
            }"##,
        )
        .unwrap();
        let defaults = ResolvedCapsuleStyle {
            background_color: color("#AAAAAA"),
            foreground_color: color("#BBBBBB"),
            divider_color: color("#CCCCCC"),
            interaction_color: color("#DDDDDD"),
        };
        let resolved = ResolvedCapsuleStyle::resolve(Some(&theme), defaults);
        assert_eq!(resolved.background_color, color("#111111"));
        assert_eq!(resolved.foreground_color, color("#222222"));
        assert_eq!(resolved.divider_color, color("#333333"));
        assert_eq!(resolved.interaction_color, color("#444444"));
    }
}
