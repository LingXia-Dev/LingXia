use super::page_chrome::{
    PageChromeColor, PatchField, TabBarPresentation, TabBarVisibilityPreference, ValuePatchField,
};
use crate::LxApp;
use lingxia_app_context::ThemeStyle;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TabBarRuntimeStylePatch {
    #[serde(default)]
    pub foreground_color: PatchField<PageChromeColor>,
    #[serde(default)]
    pub selected_foreground_color: PatchField<PageChromeColor>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TabBarItemPatch {
    pub index: i32,
    #[serde(default)]
    pub text: PatchField<String>,
    #[serde(default)]
    pub icon_path: PatchField<String>,
    #[serde(default)]
    pub badge: PatchField<String>,
    #[serde(default)]
    pub red_dot: ValuePatchField<bool>,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TabBarPatch {
    #[serde(default)]
    pub visibility: ValuePatchField<TabBarVisibilityPreference>,
    #[serde(default)]
    pub style: PatchField<TabBarRuntimeStylePatch>,
    #[serde(default)]
    pub items: ValuePatchField<Vec<TabBarItemPatch>>,
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
    /// One icon per item, drawn as a template: the host tints it for both
    /// states and marks the active tab with its own indicator, so an item
    /// never ships a second piece of artwork.
    #[serde(default)]
    pub icon_path: Option<String>,

    #[serde(skip)]
    manifest_text: Option<String>,
    #[serde(skip)]
    manifest_icon_path: Option<String>,
    #[serde(skip)]
    pub badge: Option<String>,
    #[serde(skip)]
    pub has_red_dot: bool,
}

impl TabBarItem {
    fn initialize_runtime(&mut self, base_path: &Path) {
        self.icon_path = Some(resolve_asset_path(base_path, self.icon_path.as_deref()));
        self.manifest_text = self.text.clone();
        self.manifest_icon_path = self.icon_path.clone();
        self.badge = None;
        self.has_red_dot = false;
    }

    fn set_text_override(&mut self, value: Option<String>) {
        self.text = value.or_else(|| self.manifest_text.clone());
    }

    fn set_icon_override(&mut self, value: Option<String>) {
        self.icon_path = value.or_else(|| self.manifest_icon_path.clone());
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
    pub visibility: TabBarVisibilityPreference,
    #[serde(skip)]
    pub route_visible: bool,
    #[serde(skip)]
    pub selected_index: i32,
    #[serde(skip)]
    pub runtime_style: TabBarRuntimeStyle,
}

impl TabBar {
    pub const MIN_ITEMS: usize = 2;
    pub const MAX_ITEMS: usize = 10;
    /// Slots a compact (phone) tab strip renders. Beyond this the last slot
    /// becomes an overflow affordance instead of a tab.
    pub const COMPACT_SLOTS: usize = 5;

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
        }
        Ok(())
    }

    pub fn is_valid(&self) -> bool {
        (Self::MIN_ITEMS..=Self::MAX_ITEMS).contains(&self.items.len())
    }

    pub(crate) fn with_absolute_paths(&self, base_path: &Path) -> Self {
        let mut result = self.clone();
        result.visibility = TabBarVisibilityPreference::Auto;
        result.route_visible = true;
        result.selected_index = 0;
        result.runtime_style = TabBarRuntimeStyle::default();
        for item in &mut result.items {
            item.initialize_runtime(base_path);
        }
        result
    }

    /// First item a compact host must move into its overflow menu, or `None`
    /// when every item fits the strip. With overflow, the last strip slot is
    /// the "more" affordance, so one declared item gives up its slot too.
    pub fn compact_overflow_start(&self) -> Option<usize> {
        (self.items.len() > Self::COMPACT_SLOTS).then_some(Self::COMPACT_SLOTS - 1)
    }

    /// [`Self::compact_overflow_start`] flattened for the native bridges: the
    /// index, or `-1` when the strip shows every item.
    pub fn compact_overflow_start_index(&self) -> i32 {
        self.compact_overflow_start().map_or(-1, |start| start as i32)
    }

    /// Tab pages worth creating up front — exactly those holding a slot of
    /// their own. Warming a tab costs a page service and a WebView, which only
    /// pays off for a destination that is one tap away; anything the strip
    /// folds behind "more" is a tap further out and can load on first pick.
    pub fn preload_page_paths(&self) -> Vec<String> {
        let count = self.compact_overflow_start().unwrap_or(self.items.len());
        self.items
            .iter()
            .take(count)
            .map(|item| item.page_path.clone())
            .collect()
    }

    pub fn get_item(&self, index: i32) -> Option<&TabBarItem> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.items.get(index))
    }

    pub fn is_effectively_visible(&self) -> bool {
        match self.visibility {
            TabBarVisibilityPreference::Auto => self.route_visible,
            TabBarVisibilityPreference::Visible => true,
            TabBarVisibilityPreference::Hidden => false,
        }
    }

    pub fn is_desktop_group_visible(&self) -> bool {
        self.visibility != TabBarVisibilityPreference::Hidden
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.route_visible = visible;
    }

    pub fn set_api_hidden(&mut self, hidden: bool) {
        self.visibility = if hidden {
            TabBarVisibilityPreference::Hidden
        } else {
            TabBarVisibilityPreference::Auto
        };
    }

    pub fn set_visibility(&mut self, visibility: TabBarVisibilityPreference) {
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

    pub fn apply_patch<F>(&mut self, patch: &TabBarPatch, mut resolve_icon: F) -> Result<(), String>
    where
        F: FnMut(&str, &str) -> Result<String, String>,
    {
        if let ValuePatchField::Value(visibility) = patch.visibility {
            self.set_visibility(visibility);
        }
        match &patch.style {
            PatchField::Missing => {}
            PatchField::Null => self.runtime_style = TabBarRuntimeStyle::default(),
            PatchField::Value(style) => {
                apply_opaque_color_patch(
                    &style.foreground_color,
                    &mut self.runtime_style.foreground_color,
                    "tabBar.style.foregroundColor",
                )?;
                apply_opaque_color_patch(
                    &style.selected_foreground_color,
                    &mut self.runtime_style.selected_foreground_color,
                    "tabBar.style.selectedForegroundColor",
                )?;
            }
        }
        let mut indexes = HashSet::new();
        let items = match &patch.items {
            ValuePatchField::Missing => &[][..],
            ValuePatchField::Value(items) => items,
        };
        for (patch_index, item) in items.iter().enumerate() {
            let path = format!("tabBar.items[{patch_index}]");
            if self.get_item(item.index).is_none() {
                return Err(format!(
                    "{path}.index: index {} is out of range",
                    item.index
                ));
            }
            if !indexes.insert(item.index) {
                return Err(format!("{path}.index: duplicate index {}", item.index));
            }
            let badge = match &item.badge {
                PatchField::Value(value) if value.is_empty() => PatchField::Null,
                value => value.clone(),
            };
            if matches!(badge, PatchField::Value(_)) && item.red_dot == ValuePatchField::Value(true)
            {
                return Err(format!(
                    "{path}: badge and redDot true are mutually exclusive"
                ));
            }
            apply_string_patch(&item.text, |value| {
                self.set_item_text(item.index, value);
            });
            let icon = resolve_string_patch(
                &item.icon_path,
                &mut resolve_icon,
                &format!("{path}.iconPath"),
            )?;
            apply_string_patch(&icon, |value| {
                self.set_item_icon(item.index, value);
            });
            apply_string_patch(&badge, |value| {
                self.set_badge(item.index, value);
            });
            if let ValuePatchField::Value(red_dot) = item.red_dot {
                self.set_red_dot(item.index, red_dot);
            }
        }
        Ok(())
    }

    pub fn apply_patch_transactionally<F>(
        &mut self,
        patch: &TabBarPatch,
        resolve_icon: F,
    ) -> Result<bool, String>
    where
        F: FnMut(&str, &str) -> Result<String, String>,
    {
        let mut candidate = self.clone();
        candidate.apply_patch(patch, resolve_icon)?;
        let changed = candidate != *self;
        if changed {
            self.restore_patchable_from(&candidate);
        }
        Ok(changed)
    }

    pub(crate) fn restore_patchable_from(&mut self, original: &Self) {
        self.visibility = original.visibility;
        self.runtime_style = original.runtime_style;
        for (item, original_item) in self.items.iter_mut().zip(&original.items) {
            item.text.clone_from(&original_item.text);
            item.icon_path.clone_from(&original_item.icon_path);
            item.badge.clone_from(&original_item.badge);
            item.has_red_dot = original_item.has_red_dot;
        }
    }
}

fn apply_opaque_color_patch(
    field: &PatchField<PageChromeColor>,
    target: &mut Option<PageChromeColor>,
    path: &str,
) -> Result<(), String> {
    match field {
        PatchField::Missing => {}
        PatchField::Null => *target = None,
        PatchField::Value(color) if !color.is_opaque() => {
            return Err(format!("{path}: expected opaque #RRGGBB"));
        }
        PatchField::Value(color) => *target = Some(*color),
    }
    Ok(())
}

fn apply_string_patch(field: &PatchField<String>, mut apply: impl FnMut(Option<String>)) {
    match field {
        PatchField::Missing => {}
        PatchField::Null => apply(None),
        PatchField::Value(value) => apply(Some(value.clone())),
    }
}

fn resolve_string_patch<F>(
    field: &PatchField<String>,
    resolve: &mut F,
    path: &str,
) -> Result<PatchField<String>, String>
where
    F: FnMut(&str, &str) -> Result<String, String>,
{
    match field {
        PatchField::Value(value) => resolve(value, path).map(PatchField::Value),
        PatchField::Missing => Ok(PatchField::Missing),
        PatchField::Null => Ok(PatchField::Null),
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
mod patch_tests {
    use super::*;

    fn tabbar() -> TabBar {
        serde_json::from_value::<TabBar>(serde_json::json!({
            "items": [
                {"pagePath": "pages/home/index", "text": "Home"},
                {"pagePath": "pages/profile/index", "text": "Profile"}
            ]
        }))
        .unwrap()
        .with_absolute_paths(Path::new("/tmp/lxapp"))
    }

    fn apply(tabbar: &mut TabBar, patch: &TabBarPatch) -> Result<(), String> {
        tabbar.apply_patch(patch, |value, _| Ok(value.to_string()))
    }

    #[test]
    fn patch_parser_rejects_unknown_fields_and_preserves_null_tristate() {
        let error = serde_json::from_value::<TabBarPatch>(serde_json::json!({
            "items": [{"index": 0, "unknown": true}]
        }))
        .unwrap_err();
        assert!(error.to_string().contains("unknown field"));

        let patch: TabBarPatch = serde_json::from_value(serde_json::json!({
            "style": null,
            "items": [{"index": 0, "text": null, "badge": null}]
        }))
        .unwrap();
        assert_eq!(patch.style, PatchField::Null);
        assert!(matches!(
            patch.items,
            ValuePatchField::Value(ref items)
                if items[0].text == PatchField::Null && items[0].badge == PatchField::Null
        ));
    }

    #[test]
    fn patch_validates_indexes_and_badge_red_dot_exclusion() {
        let mut state = tabbar();
        let out_of_range: TabBarPatch = serde_json::from_value(serde_json::json!({
            "items": [{"index": 2, "text": "Other"}]
        }))
        .unwrap();
        assert!(
            apply(&mut state, &out_of_range)
                .unwrap_err()
                .contains("index 2 is out of range")
        );

        let duplicate: TabBarPatch = serde_json::from_value(serde_json::json!({
            "items": [{"index": 0, "text": "A"}, {"index": 0, "text": "B"}]
        }))
        .unwrap();
        assert!(
            apply(&mut state, &duplicate)
                .unwrap_err()
                .contains("duplicate index 0")
        );

        let conflicting: TabBarPatch = serde_json::from_value(serde_json::json!({
            "items": [{"index": 0, "badge": "1", "redDot": true}]
        }))
        .unwrap();
        assert!(
            apply(&mut state, &conflicting)
                .unwrap_err()
                .contains("mutually exclusive")
        );
    }

    #[test]
    fn empty_badge_is_removal_before_red_dot_exclusion() {
        let mut state = tabbar();
        state.set_badge(0, Some("1".to_string()));
        let patch: TabBarPatch = serde_json::from_value(serde_json::json!({
            "items": [{"index": 0, "badge": "", "redDot": true}]
        }))
        .unwrap();

        apply(&mut state, &patch).unwrap();

        assert_eq!(state.items[0].badge, None);
        assert!(state.items[0].has_red_dot);
    }

    #[test]
    fn validation_failure_does_not_apply_earlier_patch_fields() {
        let mut state = tabbar();
        let patch: TabBarPatch = serde_json::from_value(serde_json::json!({
            "visibility": "hidden",
            "items": [{"index": 2, "text": "Other"}]
        }))
        .unwrap();

        assert!(
            state
                .apply_patch_transactionally(&patch, |value, _| Ok(value.to_string()))
                .is_err()
        );
        assert_eq!(state.visibility, TabBarVisibilityPreference::Auto);
    }

    #[test]
    fn visibility_preference_can_force_an_off_route_tabbar_visible() {
        let mut state = tabbar();
        state.set_visible(false);
        assert!(!state.is_effectively_visible());

        state.set_visibility(TabBarVisibilityPreference::Visible);
        assert!(state.is_effectively_visible());
        assert!(state.is_desktop_group_visible());

        state.set_visibility(TabBarVisibilityPreference::Hidden);
        assert!(!state.is_effectively_visible());
        assert!(!state.is_desktop_group_visible());

        state.set_visible(true);
        assert!(!state.is_effectively_visible());
        state.set_visibility(TabBarVisibilityPreference::Auto);
        assert!(state.is_effectively_visible());
    }

    #[test]
    fn patch_and_rollback_preserve_navigation_owned_runtime_fields() {
        let mut state = tabbar();
        state.selected_index = 1;
        state.route_visible = false;
        let original = state.clone();
        let patch: TabBarPatch = serde_json::from_value(serde_json::json!({
            "visibility": "hidden",
            "items": [{"index": 0, "text": "Changed"}]
        }))
        .unwrap();

        apply(&mut state, &patch).unwrap();
        assert_eq!(state.selected_index, 1);
        assert!(!state.route_visible);

        state.selected_index = 0;
        state.route_visible = true;
        state.restore_patchable_from(&original);
        assert_eq!(state.items[0].text.as_deref(), Some("Home"));
        assert_eq!(state.selected_index, 0);
        assert!(state.route_visible);
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
    fn compact_overflow_starts_only_past_the_strip_capacity() {
        let items = |count: usize| {
            manifest(serde_json::json!({
                "items": (0..count)
                    .map(|index| serde_json::json!({ "pagePath": format!("pages/p{index}/index") }))
                    .collect::<Vec<_>>()
            }))
        };
        assert_eq!(items(2).compact_overflow_start(), None);
        assert_eq!(items(5).compact_overflow_start(), None);
        assert_eq!(items(6).compact_overflow_start(), Some(4));
        assert_eq!(items(10).compact_overflow_start(), Some(4));
        assert_eq!(items(5).compact_overflow_start_index(), -1);
        assert_eq!(items(10).compact_overflow_start_index(), 4);
    }

    #[test]
    fn preload_covers_the_strip_slots_only() {
        let items = |count: usize| {
            manifest(serde_json::json!({
                "items": (0..count)
                    .map(|index| serde_json::json!({ "pagePath": format!("pages/p{index}/index") }))
                    .collect::<Vec<_>>()
            }))
        };
        assert_eq!(items(2).preload_page_paths().len(), 2);
        assert_eq!(items(5).preload_page_paths().len(), 5);
        // Past the strip capacity the folded items stop being warmed, so a
        // 10-tab lxapp costs no more at launch than a 4-tab one.
        assert_eq!(items(6).preload_page_paths().len(), 4);
        assert_eq!(items(10).preload_page_paths().len(), 4);
        assert_eq!(
            items(10).preload_page_paths().last().map(String::as_str),
            Some("pages/p3/index")
        );
    }

    #[test]
    fn ten_items_validate_but_eleven_do_not() {
        let paths: Vec<String> = (0..11).map(|i| format!("pages/p{i}/index")).collect();
        let of = |count: usize| {
            manifest(serde_json::json!({
                "items": (0..count)
                    .map(|index| serde_json::json!({ "pagePath": format!("pages/p{index}/index") }))
                    .collect::<Vec<_>>()
            }))
        };
        assert!(of(10).validate(&paths).is_ok());
        assert!(of(11).validate(&paths).is_err());
        assert!(of(1).validate(&paths).is_err());
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
