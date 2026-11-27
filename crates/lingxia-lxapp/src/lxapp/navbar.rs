use super::localized_text::LocalizedText;
use super::page_chrome::{PageChromeColor, PatchField, ValuePatchField, VisibilityPreference};
use lingxia_theme::{Rgba, Role, ThemeLooks};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum NavigationStyle {
    #[default]
    Default,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NavigationBarConfig {
    /// A string, or one per language; see [`LocalizedText`].
    #[serde(default)]
    pub title: LocalizedText,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NavigationBarPatch {
    #[serde(default)]
    pub title: PatchField<String>,
    #[serde(default)]
    pub home_button: ValuePatchField<VisibilityPreference>,
}

impl NavigationBarPatch {
    /// A GPU page's title has one writer, its View's `<page title>`; Logic changes it through page data.
    pub fn check_writer(&self, renderer: crate::PageRenderer) -> Result<(), String> {
        if renderer == crate::PageRenderer::Gpu && self.title != PatchField::Missing {
            return Err(
                "navigationBar.update: a GPU page's title is its View's <page title>; \
                 set it from page data"
                    .into(),
            );
        }
        Ok(())
    }

    pub fn apply(&self, state: &mut NavigationBarState) -> Result<(), String> {
        match &self.title {
            PatchField::Missing => {}
            PatchField::Null => state.set_runtime_title(None),
            PatchField::Value(title) => state.set_runtime_title(Some(title.clone())),
        }
        if let ValuePatchField::Value(home_button) = self.home_button {
            state.set_home_button_preference(home_button);
        }
        Ok(())
    }

    pub fn apply_transactionally(&self, state: &mut NavigationBarState) -> Result<bool, String> {
        let mut candidate = state.clone();
        self.apply(&mut candidate)?;
        let changed = candidate != *state;
        if changed {
            state.restore_patchable_from(&candidate);
        }
        Ok(changed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedNavigationBarStyle {
    pub background_color: PageChromeColor,
    pub foreground_color: PageChromeColor,
    pub divider_color: PageChromeColor,
}

impl ResolvedNavigationBarStyle {
    pub const fn foreground_text_style(self) -> &'static str {
        let rgb = self.foreground_color.rgba() >> 8;
        if ((rgb >> 16) & 0xff) + ((rgb >> 8) & 0xff) + (rgb & 0xff) < 384 {
            "black"
        } else {
            "white"
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedCapsuleStyle {
    pub background_color: PageChromeColor,
    pub foreground_color: PageChromeColor,
    pub divider_color: PageChromeColor,
    pub interaction_color: PageChromeColor,
}

impl ResolvedCapsuleStyle {
    /// The capsule is product chrome: it takes the host theme's roles.
    pub fn from_palette(looks: &ThemeLooks, dark: bool, high_contrast: bool) -> Self {
        let role = |role| chrome_color(looks.role(role, dark, high_contrast));
        Self {
            background_color: role(Role::Surface),
            foreground_color: role(Role::Text),
            divider_color: role(Role::Line),
            interaction_color: role(Role::NeutralSoft),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NavigationBarState {
    pub navigation_style: NavigationStyle,
    pub manifest: NavigationBarConfig,
    pub runtime_title: Option<String>,
    pub home_button: VisibilityPreference,

    #[serde(skip)]
    pub show_navbar: bool,
    /// Where the page sits in its stack decides these two: filled where the bar is read
    /// ([`crate::LxApp::get_navbar_state`]), never stored, so rebuilding the state cannot lose them.
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
            home_button: VisibilityPreference::Auto,
            show_navbar: matches!(style, NavigationStyle::Default),
            show_back_button: false,
            navigation_home_button: false,
        }
    }

    pub fn is_custom_navigation(&self) -> bool {
        matches!(self.navigation_style, NavigationStyle::Custom)
    }

    /// The runtime title if set, else the declared one for the effective
    /// display language, resolved per read so a language switch shows.
    pub fn title(&self) -> String {
        match &self.runtime_title {
            Some(title) => title.clone(),
            None => self
                .manifest
                .title
                .resolve(&super::display_language())
                .to_string(),
        }
    }

    pub fn home_button_visible(&self) -> bool {
        self.show_navbar
            && self.home_button == VisibilityPreference::Auto
            && self.navigation_home_button
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

    pub(crate) fn restore_patchable_from(&mut self, original: &Self) {
        self.runtime_title.clone_from(&original.runtime_title);
        self.home_button = original.home_button;
    }
}

#[cfg(test)]
mod patch_tests {
    use super::*;

    #[test]
    fn patch_rejects_unknown_fields_and_preserves_null_tristate() {
        let error =
            serde_json::from_value::<NavigationBarPatch>(serde_json::json!({"unknown": true}))
                .unwrap_err();
        assert!(error.to_string().contains("unknown field"));

        let patch: NavigationBarPatch =
            serde_json::from_value(serde_json::json!({"title": null})).unwrap();
        assert_eq!(patch.title, PatchField::Null);
        let error = serde_json::from_value::<NavigationBarPatch>(serde_json::json!({"style": {}}))
            .unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn logic_cannot_set_a_gpu_page_title_but_patches_the_rest() {
        let title: NavigationBarPatch =
            serde_json::from_value(serde_json::json!({"title": "Inbox"})).unwrap();
        let error = title.check_writer(crate::PageRenderer::Gpu).unwrap_err();
        assert!(error.contains("<page title>"), "{error}");
        assert!(title.check_writer(crate::PageRenderer::Webview).is_ok());
        let style: NavigationBarPatch =
            serde_json::from_value(serde_json::json!({"homeButton": "hidden"})).unwrap();
        assert!(style.check_writer(crate::PageRenderer::Gpu).is_ok());
    }

    #[test]
    fn patch_and_rollback_preserve_navigation_owned_visibility() {
        let mut state = NavigationBarState {
            show_navbar: false,
            show_back_button: true,
            navigation_home_button: true,
            ..NavigationBarState::default()
        };
        let original = state.clone();
        let patch: NavigationBarPatch = serde_json::from_value(serde_json::json!({
            "title": "Changed",
            "homeButton": "hidden"
        }))
        .unwrap();

        patch.apply(&mut state).unwrap();
        assert!(!state.show_navbar);
        assert!(state.show_back_button);
        assert!(state.navigation_home_button);

        state.show_navbar = true;
        state.show_back_button = false;
        state.navigation_home_button = false;
        state.restore_patchable_from(&original);
        assert_eq!(state.title(), original.title());
        assert!(state.show_navbar);
        assert!(!state.show_back_button);
        assert!(!state.navigation_home_button);
    }
}

impl crate::LxApp {
    /// The bar takes the roles of the theme the page renders with, so bar and page are one surface.
    pub fn resolved_navigation_bar_style(&self, _path: &str) -> ResolvedNavigationBarStyle {
        let dark = self.appearance_state().resolved.is_dark();
        let looks = self.theme_looks();
        let role = |role| chrome_color(looks.role(role, dark, super::theme::high_contrast()));
        ResolvedNavigationBarStyle {
            background_color: role(Role::Surface),
            foreground_color: role(Role::Text),
            divider_color: role(Role::Line),
        }
    }

    pub fn resolved_capsule_style(&self) -> ResolvedCapsuleStyle {
        let dark = self.appearance_state().resolved.is_dark();
        let looks = super::theme::host_looks().unwrap_or_else(|| lingxia_theme::looks(None));
        ResolvedCapsuleStyle::from_palette(&looks, dark, super::theme::high_contrast())
    }
}

pub(crate) fn chrome_color(color: Rgba) -> PageChromeColor {
    let [r, g, b, _] = color.0;
    PageChromeColor::from_rgba(u32::from_be_bytes([r, g, b, 0xff]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_home_button_cannot_be_forced_by_navigation() {
        let mut state = NavigationBarState {
            navigation_home_button: true,
            ..NavigationBarState::default()
        };
        state.set_home_button_preference(VisibilityPreference::Hidden);
        assert!(!state.home_button_visible());
    }

    #[test]
    fn the_capsule_takes_the_theme_roles() {
        let looks = lingxia_theme::looks(None);
        let capsule = ResolvedCapsuleStyle::from_palette(&looks, true, false);
        assert_eq!(
            capsule.background_color,
            chrome_color(looks.role(Role::Surface, true, false))
        );
        assert_eq!(
            capsule.foreground_color,
            chrome_color(looks.role(Role::Text, true, false))
        );
    }
}
