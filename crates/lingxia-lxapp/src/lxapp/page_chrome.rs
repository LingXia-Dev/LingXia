use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;

use super::navbar::NavigationBarState;
use super::tabbar::TabBar;
use crate::{LxApp, LxAppError, PageInstance};
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_platform::traits::ui::UIUpdate;
use lingxia_webview::WebViewController;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppearancePreference {
    #[default]
    Auto,
    Light,
    Dark,
}

impl AppearancePreference {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

impl FromStr for AppearancePreference {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            other => Err(format!(
                "appearance: expected auto, light, or dark; received '{other}'"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolvedAppearance {
    #[default]
    Light,
    Dark,
}

impl ResolvedAppearance {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub const fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VisibilityPreference {
    #[default]
    Auto,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TabBarPresentation {
    #[default]
    Standard,
    Immersive,
}

/// A CSS-order RGBA color (`#RRGGBB` or `#RRGGBBAA`) parsed once by core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageChromeColor(u32);

impl PageChromeColor {
    pub const fn from_rgba(rgba: u32) -> Self {
        Self(rgba)
    }

    pub const fn rgba(self) -> u32 {
        self.0
    }

    /// Native bridge order used by Apple, Android, HarmonyOS, and Windows.
    pub const fn argb(self) -> u32 {
        self.0.rotate_right(8)
    }

    pub const fn alpha(self) -> u8 {
        self.0 as u8
    }

    pub const fn is_opaque(self) -> bool {
        self.alpha() == u8::MAX
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        let hex = value
            .strip_prefix('#')
            .ok_or_else(|| "expected #RRGGBB or #RRGGBBAA".to_string())?;
        let rgba = match hex.len() {
            6 => u32::from_str_radix(hex, 16)
                .map(|rgb| (rgb << 8) | 0xff)
                .map_err(|_| "expected hexadecimal #RRGGBB".to_string())?,
            8 => u32::from_str_radix(hex, 16)
                .map_err(|_| "expected hexadecimal #RRGGBBAA".to_string())?,
            _ => return Err("expected #RRGGBB or #RRGGBBAA".to_string()),
        };
        Ok(Self(rgba))
    }
}

impl std::fmt::Display for PageChromeColor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_opaque() {
            write!(formatter, "#{:06X}", self.0 >> 8)
        } else {
            write!(formatter, "#{:08X}", self.0)
        }
    }
}

impl Serialize for PageChromeColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for PageChromeColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PageChromeRect {
    pub width: f64,
    pub height: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EffectivePageChromeLayout {
    pub revision: u64,
    pub bottom_inset: f64,
    pub capsule_rect: Option<PageChromeRect>,
    pub capsule_inline_end_inset: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LxAppAppearanceState {
    pub preference: AppearancePreference,
    pub resolved: ResolvedAppearance,
    pub revision: u64,
}

impl Default for LxAppAppearanceState {
    fn default() -> Self {
        Self {
            preference: AppearancePreference::Auto,
            resolved: ResolvedAppearance::Light,
            revision: 0,
        }
    }
}

/// Install the page-side snapshot contract and apply its initial value.
pub(crate) fn bootstrap_script(
    layout: &EffectivePageChromeLayout,
    appearance: ResolvedAppearance,
) -> String {
    let layout = serde_json::to_string(layout).unwrap_or_else(|_| "{}".to_string());
    let appearance = serde_json::to_string(&appearance).unwrap_or_else(|_| "\"light\"".to_string());
    format!(
        r#"(() => {{
  const publish = (raw, scheme) => {{
    const rect = raw.capsuleRect == null ? null : Object.freeze({{ ...raw.capsuleRect }});
    const layout = Object.freeze({{ ...raw, capsuleRect: rect }});
    const root = document.documentElement;
    if (root) {{
      root.style.setProperty('--lx-page-chrome-bottom-inset', `${{layout.bottomInset}}px`);
      root.style.setProperty('--lx-page-chrome-capsule-inline-end-inset', `${{layout.capsuleInlineEndInset}}px`);
      root.style.colorScheme = scheme;
    }}
    globalThis.__lingxiaPageChromeLayout = layout;
    globalThis.dispatchEvent(new CustomEvent('lxpagechromechange', {{ detail: layout }}));
  }};
  if (!globalThis.__lingxiaApplyPageChrome) {{
    Object.defineProperty(globalThis, 'lxPageChrome', {{
      configurable: false,
      enumerable: true,
      value: Object.freeze({{
        get layout() {{ return globalThis.__lingxiaPageChromeLayout; }}
      }})
    }});
    Object.defineProperty(globalThis, '__lingxiaApplyPageChrome', {{
      configurable: false,
      enumerable: false,
      value: publish
    }});
  }}
  globalThis.__lingxiaApplyPageChrome({layout}, {appearance});
}})();"#
    )
}

fn publication_script(
    layout: &EffectivePageChromeLayout,
    appearance: ResolvedAppearance,
) -> String {
    let layout = serde_json::to_string(layout).unwrap_or_else(|_| "{}".to_string());
    format!(
        "globalThis.__lingxiaApplyPageChrome?.({layout}, {});",
        serde_json::to_string(&appearance).unwrap_or_else(|_| "\"light\"".to_string())
    )
}

impl LxApp {
    pub fn appearance_state(&self) -> LxAppAppearanceState {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .appearance
    }

    pub(crate) fn next_page_chrome_revision(&self) -> u64 {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.page_chrome_revision = state.page_chrome_revision.saturating_add(1);
        state.page_chrome_revision
    }

    fn restore_page_chrome_revision(&self, revision: u64) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.page_chrome_revision == revision {
            state.page_chrome_revision = revision.saturating_sub(1);
        }
    }

    pub(crate) fn publish_page_chrome(
        &self,
        page: &PageInstance,
        revision: u64,
        appearance: ResolvedAppearance,
    ) -> Result<(), LxAppError> {
        let instance_id = page.instance_id_string();
        let layout = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if state.page_chrome_revision != revision {
                return Ok(());
            }
            let layout = state.page_chrome_layouts.entry(instance_id).or_default();
            layout.revision = revision;
            layout.clone()
        };
        if let Some(webview) = page.webview() {
            webview
                .exec_js(&publication_script(&layout, appearance))
                .map_err(|error| LxAppError::WebView(error.to_string()))?;
        }
        Ok(())
    }

    pub(crate) async fn publish_realized_page_chrome(
        &self,
        page: &PageInstance,
        revision: u64,
        appearance: ResolvedAppearance,
    ) -> Result<(), LxAppError> {
        let capsule_rect = if self.get_navbar_state(&page.path()).is_custom_navigation() {
            self.runtime
                .measure_page_chrome_capsule(self.appid.clone())
                .await?
                .map(|payload| {
                    serde_json::from_str::<PageChromeRect>(&payload)
                        .map_err(|error| LxAppError::Runtime(error.to_string()))
                })
                .transpose()?
        } else {
            None
        };
        let bottom_inset = self
            .get_tabbar()
            .filter(|tabbar| {
                tabbar.presentation == TabBarPresentation::Immersive
                    && tabbar.is_effectively_visible()
            })
            .map_or(0.0, |_| immersive_tabbar_inset());
        let capsule_inline_end_inset = capsule_rect
            .map(|rect| rect.width + capsule_trailing_inset())
            .unwrap_or(0.0);
        {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if state.page_chrome_revision != revision {
                return Ok(());
            }
            state.page_chrome_layouts.insert(
                page.instance_id_string(),
                EffectivePageChromeLayout {
                    revision,
                    bottom_inset,
                    capsule_rect,
                    capsule_inline_end_inset,
                },
            );
        }
        self.publish_page_chrome(page, revision, appearance)
    }

    async fn apply_page_chrome_commit(
        &self,
        page: &PageInstance,
        revision: u64,
        appearance: ResolvedAppearance,
    ) -> Result<(), LxAppError> {
        self.runtime
            .apply_page_chrome_revision(self.appid.clone(), revision)
            .await?;
        self.publish_realized_page_chrome(page, revision, appearance)
            .await
    }

    pub async fn commit_navigation_bar(
        &self,
        page: PageInstance,
        candidate: NavigationBarState,
    ) -> Result<(), LxAppError> {
        let _guard = self.page_chrome_mutation_lock.lock().await;
        let original = page
            .get_navbar_state()
            .ok_or_else(|| LxAppError::ResourceNotFound("active page".to_string()))?;
        if original == candidate {
            return Ok(());
        }
        page.get_navbar_state_mut(|state| *state = candidate)
            .ok_or_else(|| LxAppError::ResourceNotFound("active page".to_string()))?;
        let revision = self.next_page_chrome_revision();
        let appearance = self.appearance_state().resolved;
        if let Err(error) = self
            .apply_page_chrome_commit(&page, revision, appearance)
            .await
        {
            let _ = page.get_navbar_state_mut(|state| *state = original);
            self.restore_page_chrome_revision(revision);
            return Err(error);
        }
        Ok(())
    }

    pub async fn commit_tabbar(&self, candidate: TabBar) -> Result<(), LxAppError> {
        let _guard = self.page_chrome_mutation_lock.lock().await;
        let original = self
            .get_tabbar()
            .ok_or_else(|| LxAppError::ResourceNotFound("declared tabbar".to_string()))?;
        if original == candidate {
            return Ok(());
        }
        let page = self.current_page()?;
        self.with_tabbar_mut(|tabbar| *tabbar = candidate)
            .ok_or_else(|| LxAppError::ResourceNotFound("declared tabbar".to_string()))?;
        let revision = self.next_page_chrome_revision();
        let appearance = self.appearance_state().resolved;
        if let Err(error) = self
            .apply_page_chrome_commit(&page, revision, appearance)
            .await
        {
            let _ = self.with_tabbar_mut(|tabbar| *tabbar = original);
            self.restore_page_chrome_revision(revision);
            return Err(error);
        }
        Ok(())
    }

    pub async fn set_appearance_preference(
        &self,
        preference: AppearancePreference,
    ) -> Result<(), LxAppError> {
        let _guard = self.page_chrome_mutation_lock.lock().await;
        let original = self.appearance_state();
        let resolved = match preference {
            AppearancePreference::Light => ResolvedAppearance::Light,
            AppearancePreference::Dark => ResolvedAppearance::Dark,
            AppearancePreference::Auto => {
                if self.runtime.host_appearance_dark() {
                    ResolvedAppearance::Dark
                } else {
                    ResolvedAppearance::Light
                }
            }
        };
        if original.preference == preference && original.resolved == resolved {
            return Ok(());
        }
        let page = self.current_page()?;
        let revision = self.next_page_chrome_revision();
        {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            state.appearance = LxAppAppearanceState {
                preference,
                resolved,
                revision,
            };
        }
        let apply = self
            .runtime
            .apply_lxapp_appearance(&self.appid, resolved.is_dark())
            .map_err(LxAppError::from);
        let apply = match apply {
            Ok(()) => {
                self.apply_page_chrome_commit(&page, revision, resolved)
                    .await
            }
            Err(error) => Err(error),
        };
        let stored = if apply.is_ok() {
            lingxia_service::settings::set_lxapp_appearance(
                &self.runtime.app_data_dir(),
                &self.appid,
                preference.as_str(),
            )
            .map_err(|error| LxAppError::Runtime(error.to_string()))
        } else {
            Ok(())
        };
        if let Err(error) = apply.and(stored) {
            {
                let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
                state.appearance = original;
            }
            self.restore_page_chrome_revision(revision);
            let _ = self
                .runtime
                .apply_lxapp_appearance(&self.appid, original.resolved.is_dark());
            let _ = self
                .runtime
                .apply_page_chrome_revision(self.appid.clone(), original.revision)
                .await;
            let _ = self.publish_page_chrome(&page, original.revision, original.resolved);
            return Err(error);
        }
        Ok(())
    }
}

const fn immersive_tabbar_inset() -> f64 {
    #[cfg(target_os = "android")]
    {
        return 64.0;
    }
    #[cfg(target_os = "ios")]
    {
        return 64.0;
    }
    #[cfg(target_env = "ohos")]
    {
        return 72.0;
    }
    #[allow(unreachable_code)]
    0.0
}

const fn capsule_trailing_inset() -> f64 {
    #[cfg(any(target_os = "android", target_os = "ios", target_env = "ohos"))]
    {
        return 12.0;
    }
    #[allow(unreachable_code)]
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_parses_css_order_alpha() {
        let opaque = PageChromeColor::parse("#A1B2C3").unwrap();
        assert_eq!(opaque.rgba(), 0xA1B2C3FF);
        assert!(opaque.is_opaque());
        assert_eq!(opaque.to_string(), "#A1B2C3");

        let translucent = PageChromeColor::parse("#A1B2C380").unwrap();
        assert_eq!(translucent.rgba(), 0xA1B2C380);
        assert_eq!(translucent.alpha(), 0x80);
        assert_eq!(translucent.to_string(), "#A1B2C380");
    }

    #[test]
    fn appearance_rejects_host_global_system_spelling() {
        assert_eq!("auto".parse(), Ok(AppearancePreference::Auto));
        assert!("system".parse::<AppearancePreference>().is_err());
        assert!("DARK".parse::<AppearancePreference>().is_err());
        assert!(" dark ".parse::<AppearancePreference>().is_err());
    }
}
