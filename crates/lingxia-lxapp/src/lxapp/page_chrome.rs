use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;

use super::navbar::NavigationBarPatch;
use super::tabbar::TabBarPatch;
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
pub enum TabBarVisibilityPreference {
    #[default]
    Auto,
    Visible,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TabBarPresentation {
    #[default]
    Standard,
    Immersive,
}

/// A JSON patch field that distinguishes omission, explicit null, and a value.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PatchField<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

/// A non-null JSON patch field that still distinguishes omission from a value.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ValuePatchField<T> {
    #[default]
    Missing,
    Value(T),
}

impl<'de, T> Deserialize<'de> for ValuePatchField<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Self::Value)
    }
}

impl<'de, T> Deserialize<'de> for PatchField<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.is_null() {
            return Ok(Self::Null);
        }
        T::deserialize(value)
            .map(Self::Value)
            .map_err(serde::de::Error::custom)
    }
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
    /// Height of the runtime-owned drag strip a `chrome: 'full'` window keeps
    /// above the page. Zero everywhere else.
    pub top_inset: f64,
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
    const current = globalThis.__lingxiaPageChromeLayout;
    if (current && raw.revision < current.revision) return;
    const rect = raw.capsuleRect == null ? null : Object.freeze({{ ...raw.capsuleRect }});
    const layout = Object.freeze({{ ...raw, capsuleRect: rect }});
    const root = document.documentElement;
    if (root) {{
      root.style.setProperty('--lx-page-chrome-top-inset', `${{layout.topInset}}px`);
      root.style.setProperty('--lx-page-chrome-bottom-inset', `${{layout.bottomInset}}px`);
      root.style.setProperty('--lx-page-chrome-capsule-inline-end-inset', `${{layout.capsuleInlineEndInset}}px`);
      root.style.colorScheme = scheme;
      root.setAttribute('data-theme', scheme);
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
        "var f = globalThis.__lingxiaApplyPageChrome; if (f) f({layout}, {});",
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

    fn restore_page_chrome_revision(&self, revision: u64) -> u64 {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.page_chrome_revision = rollback_revision(state.page_chrome_revision, revision);
        state.page_chrome_revision
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
                .map_err(LxAppError::from)?;
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
        let top_inset = self.full_chrome_drag_strip_inset(page);
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
                    top_inset,
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

    fn current_page_for_chrome(&self) -> Result<PageInstance, LxAppError> {
        self.current_page().map_err(|error| match error {
            LxAppError::WebView(_) => LxAppError::ResourceNotFound("active page".to_string()),
            error => error,
        })
    }

    async fn compensate_page_chrome_rollback(
        &self,
        page: &PageInstance,
        failed_revision: u64,
        appearance: ResolvedAppearance,
    ) {
        let restored_revision = self.restore_page_chrome_revision(failed_revision);
        let _ = self
            .apply_page_chrome_commit(page, restored_revision, appearance)
            .await;
    }

    pub async fn commit_navigation_bar(
        &self,
        page: PageInstance,
        patch: NavigationBarPatch,
    ) -> Result<(), LxAppError> {
        let _guard = self.page_chrome_mutation_lock.lock().await;
        let original = page
            .get_navbar_state()
            .ok_or_else(|| LxAppError::ResourceNotFound("active page".to_string()))?;
        let changed = page
            .get_navbar_state_mut(|state| patch.apply_transactionally(state))
            .ok_or_else(|| LxAppError::ResourceNotFound("active page".to_string()))?
            .map_err(LxAppError::InvalidParameter)?;
        if !changed {
            return Ok(());
        }
        let revision = self.next_page_chrome_revision();
        let appearance = self.appearance_state().resolved;
        if let Err(error) = self
            .apply_page_chrome_commit(&page, revision, appearance)
            .await
        {
            let _ = page.get_navbar_state_mut(|state| state.restore_patchable_from(&original));
            self.compensate_page_chrome_rollback(&page, revision, appearance)
                .await;
            return Err(error);
        }
        Ok(())
    }

    pub async fn commit_tabbar(&self, patch: TabBarPatch) -> Result<(), LxAppError> {
        let _guard = self.page_chrome_mutation_lock.lock().await;
        let page = self.current_page_for_chrome()?;
        let original = self
            .get_tabbar()
            .ok_or_else(|| LxAppError::ResourceNotFound("declared tabbar".to_string()))?;
        let changed = self
            .with_tabbar_mut(|tabbar| {
                tabbar.apply_patch_transactionally(&patch, |value, path| {
                    self.resolve_accessible_path(value)
                        .map(|path| path.to_string_lossy().into_owned())
                        // Keep the reason. Collapsing every failure into one
                        // sentence about packaging hid what actually went
                        // wrong — a network URL, a missing file, a traversal
                        // attempt all read identically, and only one of them
                        // was about packaging.
                        //
                        // `detail()`, not `Display`: this sentence already says
                        // which field failed, and the variant labels would
                        // stack into "Invalid parameter: iconPath: Invalid
                        // parameter: …" or "traversal not allowed not found".
                        .map_err(|error| {
                            let reason = error
                                .detail()
                                .map(str::to_string)
                                .unwrap_or_else(|| error.to_string());
                            format!("{path}: {reason}")
                        })
                })
            })
            .ok_or_else(|| LxAppError::ResourceNotFound("declared tabbar".to_string()))?
            .map_err(LxAppError::InvalidParameter)?;
        if !changed {
            return Ok(());
        }
        let revision = self.next_page_chrome_revision();
        let appearance = self.appearance_state().resolved;
        if let Err(error) = self
            .apply_page_chrome_commit(&page, revision, appearance)
            .await
        {
            let _ = self.with_tabbar_mut(|tabbar| tabbar.restore_patchable_from(&original));
            self.compensate_page_chrome_rollback(&page, revision, appearance)
                .await;
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
        let page = self.current_page_for_chrome()?;
        if original.preference == preference && original.resolved == resolved {
            lingxia_service::settings::set_lxapp_appearance(
                &self.runtime.app_data_dir(),
                &self.appid,
                preference.as_str(),
            )
            .map_err(|error| LxAppError::Runtime(error.to_string()))?;
            self.publish_host_color_mode(preference, resolved);
            return Ok(());
        }
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
                restore_appearance_state(&mut state.appearance, original);
            }
            let _ = self
                .runtime
                .apply_lxapp_appearance(&self.appid, original.resolved.is_dark());
            self.compensate_page_chrome_rollback(&page, revision, original.resolved)
                .await;
            return Err(error);
        }
        self.publish_appearance_to_background_pages(&page, revision, resolved);
        self.publish_host_color_mode(preference, resolved);
        Ok(())
    }

    /// Push the app's appearance into the platform's own night mode.
    ///
    /// Not for the launch face — that is one picture in every appearance and
    /// resolves the same either way. It is for everything the platform draws
    /// from its own night mode: the activity theme, and the canvas the host
    /// paints behind a page.
    ///
    /// Only the home lxapp speaks for the app: it is the one whose appearance
    /// the user sees at launch.
    fn publish_host_color_mode(
        &self,
        preference: AppearancePreference,
        resolved: ResolvedAppearance,
    ) {
        if lingxia_app_context::home_app_id() != Some(self.appid.as_str()) {
            return;
        }
        // Auto clears the override rather than pinning today's answer: the
        // next launch must follow the system as it is then, not as it was here.
        let dark = match preference {
            AppearancePreference::Auto => None,
            _ => Some(resolved.is_dark()),
        };
        if let Some(platform) = crate::lxapp::runtime_registry::get_platform() {
            use lingxia_platform::traits::ui::UIUpdate;
            platform.set_host_color_mode(dark);
        }
    }

    /// Stamp the live scheme onto a page about to be (re)shown: a cached
    /// document may hold the palette from before an appearance change, and
    /// it must not enter the transition stale.
    pub fn republish_page_scheme(&self, page: &PageInstance) {
        let appearance = self.appearance_state().resolved;
        let revision = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .page_chrome_revision;
        let _ = self.publish_page_chrome(page, revision, appearance);
    }

    /// The commit publishes the realized layout to the current page only;
    /// every other live page still carries the scheme in its own document
    /// (colorScheme/data-theme), so re-stamp them or platforms that cannot
    /// flip prefers-color-scheme in place (Android) keep stale palettes.
    fn publish_appearance_to_background_pages(
        &self,
        current: &PageInstance,
        revision: u64,
        appearance: ResolvedAppearance,
    ) {
        let current_id = current.instance_id_string();
        let others: Vec<PageInstance> = {
            let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let pages_by_id = state
                .pages_by_id
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            pages_by_id
                .values()
                .filter(|page| page.instance_id_string() != current_id)
                .cloned()
                .collect()
        };
        for page in others {
            let _ = self.publish_page_chrome(&page, revision, appearance);
        }
    }
}

const fn rollback_revision(current: u64, failed: u64) -> u64 {
    if current == failed {
        failed.saturating_sub(1)
    } else {
        current
    }
}

fn restore_appearance_state(current: &mut LxAppAppearanceState, original: LxAppAppearanceState) {
    *current = original;
}

const fn immersive_tabbar_inset() -> f64 {
    #[cfg(target_os = "android")]
    {
        return 64.0;
    }
    #[cfg(target_os = "ios")]
    {
        // This fixed contract includes the common home-indicator safe area;
        // native measurement is intentionally deferred until the host exposes it.
        return 64.0;
    }
    #[cfg(target_env = "ohos")]
    {
        return 72.0;
    }
    #[allow(unreachable_code)]
    // Desktop has no mobile overlay bar, so immersive content needs no inset.
    0.0
}

const fn capsule_trailing_inset() -> f64 {
    // Applied only when a capsule was actually measured, and a measured capsule
    // means phone-chrome metrics — including on macOS, where the Runner's
    // simulated phone draws the same 12pt-trailing pill as iOS.
    12.0
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

    #[test]
    fn scripts_reject_stale_revisions_without_optional_chaining() {
        let bootstrap = bootstrap_script(
            &EffectivePageChromeLayout {
                revision: 4,
                ..Default::default()
            },
            ResolvedAppearance::Dark,
        );
        assert!(bootstrap.contains("raw.revision < current.revision"));

        let publication = publication_script(
            &EffectivePageChromeLayout {
                revision: 5,
                ..Default::default()
            },
            ResolvedAppearance::Light,
        );
        assert!(publication.contains("var f = globalThis.__lingxiaApplyPageChrome"));
        assert!(!publication.contains("?."));
    }

    #[test]
    fn rollback_republishes_the_restored_current_revision() {
        assert_eq!(rollback_revision(5, 5), 4);
        assert_eq!(rollback_revision(6, 5), 6);
        assert_eq!(rollback_revision(0, 0), 0);
    }

    #[test]
    fn appearance_rollback_restores_preference_resolution_and_revision() {
        let original = LxAppAppearanceState {
            preference: AppearancePreference::Auto,
            resolved: ResolvedAppearance::Light,
            revision: 4,
        };
        let mut current = LxAppAppearanceState {
            preference: AppearancePreference::Dark,
            resolved: ResolvedAppearance::Dark,
            revision: 5,
        };

        restore_appearance_state(&mut current, original);

        assert_eq!(current, original);
        assert_eq!(rollback_revision(5, 5), current.revision);
    }
}
