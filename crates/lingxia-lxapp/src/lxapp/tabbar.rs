use super::page_chrome::{PageChromeColor, TabBarPresentation, VisibilityPreference};
use crate::LxApp;
use lingxia_app_context::ThemeStyle;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TabBarStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground_color: Option<PageChromeColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_foreground_color: Option<PageChromeColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<PageChromeColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divider_color: Option<PageChromeColor>,
}

impl TabBarStyle {
    fn validate(self, presentation: TabBarPresentation) -> Result<Self, String> {
        for (field, color) in [
            ("foregroundColor", self.foreground_color),
            ("selectedForegroundColor", self.selected_foreground_color),
            ("backgroundColor", self.background_color),
        ] {
            if color.is_some_and(|color| !color.is_opaque()) {
                return Err(format!("tabBar.style.{field}: expected opaque #RRGGBB"));
            }
        }
        if presentation == TabBarPresentation::Immersive {
            if self.background_color.is_some() {
                return Err(
                    "tabBar.style.backgroundColor: must be omitted when tabBar.presentation is immersive"
                        .to_string(),
                );
            }
            if self.divider_color.is_some() {
                return Err(
                    "tabBar.style.dividerColor: must be omitted when tabBar.presentation is immersive"
                        .to_string(),
                );
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TabBarRuntimeStyle {
    pub foreground_color: Option<PageChromeColor>,
    pub selected_foreground_color: Option<PageChromeColor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedTabBarStyle {
    pub foreground_color: PageChromeColor,
    pub selected_foreground_color: PageChromeColor,
    pub background_color: Option<PageChromeColor>,
    pub divider_color: Option<PageChromeColor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TabBarItem {
    pub page_path: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub icon_path: Option<String>,
    #[serde(default)]
    pub selected_icon_path: Option<String>,

    #[serde(skip)]
    manifest_text: Option<String>,
    #[serde(skip)]
    manifest_icon_path: Option<String>,
    #[serde(skip)]
    manifest_selected_icon_path: Option<String>,
    #[serde(skip)]
    pub badge: Option<String>,
    #[serde(skip)]
    pub has_red_dot: bool,
}

impl TabBarItem {
    fn initialize_runtime(&mut self, base_path: &Path) {
        self.icon_path = Some(resolve_asset_path(base_path, self.icon_path.as_deref()));
        self.selected_icon_path = Some(match self.selected_icon_path.as_deref() {
            Some(path) if !path.trim().is_empty() => resolve_asset_path(base_path, Some(path)),
            _ => self.icon_path.clone().unwrap_or_default(),
        });
        self.manifest_text = self.text.clone();
        self.manifest_icon_path = self.icon_path.clone();
        self.manifest_selected_icon_path = self.selected_icon_path.clone();
        self.badge = None;
        self.has_red_dot = false;
    }

    fn set_text_override(&mut self, value: Option<String>) {
        self.text = value.or_else(|| self.manifest_text.clone());
    }

    fn set_icon_override(&mut self, value: Option<String>) {
        self.icon_path = value.or_else(|| self.manifest_icon_path.clone());
    }

    fn set_selected_icon_override(&mut self, value: Option<String>) {
        self.selected_icon_path = value.or_else(|| self.manifest_selected_icon_path.clone());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TabBar {
    #[serde(default)]
    pub presentation: TabBarPresentation,
    #[serde(default)]
    pub style: TabBarStyle,
    pub items: Vec<TabBarItem>,

    #[serde(skip)]
    pub visibility: VisibilityPreference,
    #[serde(skip)]
    pub route_visible: bool,
    #[serde(skip)]
    pub selected_index: i32,
    #[serde(skip)]
    pub runtime_style: TabBarRuntimeStyle,
    #[serde(skip)]
    pub revision: u64,
}

impl TabBar {
    pub const MIN_ITEMS: usize = 2;
    pub const MAX_ITEMS: usize = 5;

    pub fn validate(&mut self, page_paths: &[String]) -> Result<(), String> {
        if !(Self::MIN_ITEMS..=Self::MAX_ITEMS).contains(&self.items.len()) {
            return Err(format!(
                "tabBar.items: expected {} to {} items",
                Self::MIN_ITEMS,
                Self::MAX_ITEMS
            ));
        }
        self.style = self.style.validate(self.presentation)?;
        for (index, item) in self.items.iter_mut().enumerate() {
            item.page_path = item.page_path.trim().trim_start_matches('/').to_string();
            if !page_paths.iter().any(|path| path == &item.page_path) {
                return Err(format!(
                    "tabBar.items[{index}].pagePath: '{}' is not a registered page",
                    item.page_path
                ));
            }
            validate_asset_path(item.icon_path.as_deref(), index, "iconPath")?;
            validate_asset_path(
                item.selected_icon_path.as_deref(),
                index,
                "selectedIconPath",
            )?;
        }
        Ok(())
    }

    pub fn is_valid(&self) -> bool {
        (Self::MIN_ITEMS..=Self::MAX_ITEMS).contains(&self.items.len())
    }

    pub(crate) fn with_absolute_paths(&self, base_path: &Path) -> Self {
        let mut result = self.clone();
        result.visibility = VisibilityPreference::Auto;
        result.route_visible = true;
        result.selected_index = 0;
        result.runtime_style = TabBarRuntimeStyle::default();
        result.revision = 0;
        for item in &mut result.items {
            item.initialize_runtime(base_path);
        }
        result
    }

    pub fn get_item(&self, index: i32) -> Option<&TabBarItem> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.items.get(index))
    }

    pub fn is_effectively_visible(&self) -> bool {
        self.visibility == VisibilityPreference::Auto && self.route_visible
    }

    pub fn is_desktop_group_visible(&self) -> bool {
        self.visibility == VisibilityPreference::Auto
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.route_visible = visible;
    }

    pub fn set_api_hidden(&mut self, hidden: bool) {
        self.visibility = if hidden {
            VisibilityPreference::Hidden
        } else {
            VisibilityPreference::Auto
        };
    }

    pub fn set_visibility(&mut self, visibility: VisibilityPreference) {
        self.visibility = visibility;
    }

    pub fn set_badge(&mut self, index: i32, text: Option<String>) -> bool {
        let Some(item) = usize::try_from(index)
            .ok()
            .and_then(|index| self.items.get_mut(index))
        else {
            return false;
        };
        item.badge = text.filter(|text| !text.is_empty());
        if item.badge.is_some() {
            item.has_red_dot = false;
        }
        true
    }

    pub fn remove_badge(&mut self, index: i32) -> bool {
        self.set_badge(index, None)
    }

    pub fn set_red_dot(&mut self, index: i32, show: bool) -> bool {
        let Some(item) = usize::try_from(index)
            .ok()
            .and_then(|index| self.items.get_mut(index))
        else {
            return false;
        };
        item.has_red_dot = show;
        if show {
            item.badge = None;
        }
        true
    }

    pub fn set_item_text(&mut self, index: i32, text: Option<String>) -> bool {
        let Some(item) = usize::try_from(index)
            .ok()
            .and_then(|index| self.items.get_mut(index))
        else {
            return false;
        };
        item.set_text_override(text);
        true
    }

    pub fn set_item_icon(&mut self, index: i32, icon_path: Option<String>) -> bool {
        let Some(item) = usize::try_from(index)
            .ok()
            .and_then(|index| self.items.get_mut(index))
        else {
            return false;
        };
        item.set_icon_override(icon_path);
        true
    }

    pub fn set_item_selected_icon(&mut self, index: i32, icon_path: Option<String>) -> bool {
        let Some(item) = usize::try_from(index)
            .ok()
            .and_then(|index| self.items.get_mut(index))
        else {
            return false;
        };
        item.set_selected_icon_override(icon_path);
        true
    }

    pub fn get_selected_index(&self) -> i32 {
        self.selected_index
    }

    pub fn set_selected_index(&mut self, index: i32) -> &mut Self {
        if index >= 0 && (index as usize) < self.items.len() {
            self.selected_index = index;
        }
        self
    }

    pub fn clear_selected_index(&mut self) -> &mut Self {
        self.selected_index = -1;
        self
    }

    pub fn find_index_by_path(&self, path: &str) -> Option<i32> {
        self.items
            .iter()
            .position(|item| item.page_path == path)
            .map(|index| index as i32)
    }

    pub fn is_tabbar_page(&self, path: &str) -> bool {
        self.items.iter().any(|item| item.page_path == path)
    }

    pub fn get_tabbar_pages(&self) -> Vec<String> {
        self.items
            .iter()
            .map(|item| item.page_path.clone())
            .collect()
    }

    pub fn resolved_style(
        &self,
        theme: Option<&ThemeStyle>,
        defaults: ResolvedTabBarStyle,
    ) -> ResolvedTabBarStyle {
        let theme_foreground = theme.and_then(|style| style.muted_foreground_color);
        let theme_selected = theme.and_then(|style| style.accent_color);
        let theme_background = theme.and_then(|style| style.surface_background_color);
        let theme_divider = theme.and_then(|style| style.separator_color);
        let standard = self.presentation == TabBarPresentation::Standard;
        ResolvedTabBarStyle {
            foreground_color: self
                .runtime_style
                .foreground_color
                .or(self.style.foreground_color)
                .or_else(|| theme_foreground.map(theme_color))
                .unwrap_or(defaults.foreground_color),
            selected_foreground_color: self
                .runtime_style
                .selected_foreground_color
                .or(self.style.selected_foreground_color)
                .or_else(|| theme_selected.map(theme_color))
                .unwrap_or(defaults.selected_foreground_color),
            background_color: standard.then(|| {
                self.style
                    .background_color
                    .or_else(|| theme_background.map(theme_color))
                    .or(defaults.background_color)
                    .expect("standard tabbar defaults include a background")
            }),
            divider_color: standard.then(|| {
                self.style
                    .divider_color
                    .or_else(|| theme_divider.map(theme_color))
                    .or(defaults.divider_color)
                    .expect("standard tabbar defaults include a divider")
            }),
        }
    }
}

fn resolve_asset_path(base_path: &Path, value: Option<&str>) -> String {
    value
        .filter(|path| !path.trim().is_empty())
        .map(|path| base_path.join(path).to_string_lossy().to_string())
        .unwrap_or_default()
}

fn validate_asset_path(value: Option<&str>, index: usize, field: &str) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let path = Path::new(value);
    if path.is_absolute()
        || value.contains('\\')
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "tabBar.items[{index}].{field}: path must stay within the lxapp package"
        ));
    }
    Ok(())
}

fn theme_color(color: lingxia_app_context::ThemeColor) -> PageChromeColor {
    PageChromeColor::from_rgba((color.rgb() << 8) | 0xff)
}

impl LxApp {
    pub fn get_tabbar(&self) -> Option<TabBar> {
        let state = self.state.lock().unwrap();
        state
            .tabbar
            .as_ref()
            .filter(|tabbar| tabbar.is_valid())
            .cloned()
    }

    pub fn get_tabbar_item(&self, index: i32) -> Option<TabBarItem> {
        let state = self.state.lock().unwrap();
        let tabbar = state.tabbar.as_ref().filter(|tabbar| tabbar.is_valid())?;
        tabbar.get_item(index).cloned()
    }

    pub fn with_tabbar_mut<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut TabBar) -> R,
    {
        let mut state = self.state.lock().unwrap();
        state
            .tabbar
            .as_mut()
            .filter(|tabbar| tabbar.is_valid())
            .map(f)
    }

    pub fn resolved_tabbar_style(&self) -> Option<ResolvedTabBarStyle> {
        let tabbar = self.get_tabbar()?;
        let dark = self.appearance_state().resolved.is_dark();
        let theme = lingxia_app_context::theme().and_then(|theme| theme.style(dark));
        let defaults = if dark {
            ResolvedTabBarStyle {
                foreground_color: PageChromeColor::from_rgba(0xAEAEB2FF),
                selected_foreground_color: PageChromeColor::from_rgba(0x0A84FFFF),
                background_color: Some(PageChromeColor::from_rgba(0x1C1C1EFF)),
                divider_color: Some(PageChromeColor::from_rgba(0x38383AFF)),
            }
        } else {
            ResolvedTabBarStyle {
                foreground_color: PageChromeColor::from_rgba(0x666666FF),
                selected_foreground_color: PageChromeColor::from_rgba(0x1677FFFF),
                background_color: Some(PageChromeColor::from_rgba(0xFFFFFFFF)),
                divider_color: Some(PageChromeColor::from_rgba(0xF0F0F0FF)),
            }
        };
        Some(tabbar.resolved_style(theme, defaults))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(value: serde_json::Value) -> TabBar {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn immersive_rejects_surface_colors_with_exact_path() {
        let mut tabbar = manifest(serde_json::json!({
            "presentation": "immersive",
            "style": { "backgroundColor": "#FFFFFF" },
            "items": [
                { "pagePath": "pages/home/index" },
                { "pagePath": "pages/settings/index" }
            ]
        }));
        let error = tabbar
            .validate(&["pages/home/index".into(), "pages/settings/index".into()])
            .unwrap_err();
        assert!(error.starts_with("tabBar.style.backgroundColor:"));
    }

    #[test]
    fn runtime_null_reveals_manifest_item() {
        let mut item: TabBarItem = serde_json::from_value(serde_json::json!({
            "pagePath": "pages/home/index",
            "text": "Home"
        }))
        .unwrap();
        item.initialize_runtime(Path::new("/app"));
        item.set_text_override(Some("Inbox".into()));
        assert_eq!(item.text.as_deref(), Some("Inbox"));
        item.set_text_override(None);
        assert_eq!(item.text.as_deref(), Some("Home"));
    }

    #[test]
    fn badge_and_red_dot_clear_each_other() {
        let mut tabbar = manifest(serde_json::json!({
            "items": [
                { "pagePath": "pages/home/index" },
                { "pagePath": "pages/settings/index" }
            ]
        }));
        tabbar.set_badge(0, Some("3".into()));
        tabbar.set_red_dot(0, true);
        assert!(tabbar.items[0].badge.is_none());
        assert!(tabbar.items[0].has_red_dot);
        tabbar.set_badge(0, Some("4".into()));
        assert_eq!(tabbar.items[0].badge.as_deref(), Some("4"));
        assert!(!tabbar.items[0].has_red_dot);
    }
}
