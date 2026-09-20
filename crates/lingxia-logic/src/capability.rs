//! Stable, context-scoped feature contracts. Permissions and layout stay live
//! at their own API boundaries, never in a supports lookup.

use crate::i18n::js_internal_error;
use lxapp::LxApp;
use rong::{HostError, JSContext, JSContextService, JSResult, JSValue};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock, Weak};

#[derive(Debug, Deserialize)]
struct FeatureEntry {
    key: String,
    #[serde(default)]
    requires: Vec<String>,
    predicate: Predicate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum Predicate {
    Always,
    Terminal,
    Autostart,
    Notifications,
    Banner,
    Browser,
    Proxy,
    SelfUpdate,
    Process,
    AppUse,
    ComputerUse,
    BrowserUse,
    MediaCapture,
    Window,
    FullChrome,
}

fn registry() -> &'static Result<Vec<FeatureEntry>, String> {
    static REGISTRY: OnceLock<Result<Vec<FeatureEntry>, String>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let entries: Vec<FeatureEntry> = serde_json::from_str(include_str!("features.json"))
            .map_err(|error| error.to_string())?;
        // Validate every dependency, including branches unsupported on this OS.
        resolve(&entries, |_| true)?;
        Ok(entries)
    })
}

fn resolve(
    entries: &[FeatureEntry],
    evaluate: impl Fn(Predicate) -> bool,
) -> Result<BTreeSet<String>, String> {
    let mut index = BTreeMap::new();
    for entry in entries {
        if index.insert(entry.key.as_str(), entry).is_some() {
            return Err(format!("duplicate feature {}", entry.key));
        }
    }
    fn visit<'a>(
        key: &'a str,
        index: &BTreeMap<&'a str, &'a FeatureEntry>,
        visiting: &mut BTreeSet<&'a str>,
        resolved: &mut BTreeMap<&'a str, bool>,
        evaluate: &impl Fn(Predicate) -> bool,
    ) -> Result<bool, String> {
        if let Some(value) = resolved.get(key) {
            return Ok(*value);
        }
        let entry = index
            .get(key)
            .ok_or_else(|| format!("missing feature {key}"))?;
        if !visiting.insert(key) {
            return Err(format!("cyclic feature dependency at {key}"));
        }
        let mut supported = evaluate(entry.predicate);
        for dependency in &entry.requires {
            supported &= visit(dependency, index, visiting, resolved, evaluate)?;
        }
        visiting.remove(key);
        resolved.insert(key, supported);
        Ok(supported)
    }
    let mut resolved = BTreeMap::new();
    for key in index.keys() {
        visit(key, &index, &mut BTreeSet::new(), &mut resolved, &evaluate)?;
    }
    Ok(resolved
        .into_iter()
        .filter(|(_, yes)| *yes)
        .map(|(key, _)| key.to_string())
        .collect())
}

struct FeatureSnapshot {
    supported: BTreeSet<String>,
    app: Weak<LxApp>,
    context_id: String,
}

impl JSContextService for FeatureSnapshot {
    fn on_shutdown(&self) {
        if let Some(app) = self.app.upgrade() {
            app.remove_logic_feature_snapshot(&self.context_id);
        }
    }
}

pub(crate) fn is_control_app(ctx: &JSContext) -> bool {
    LxApp::from_ctx(ctx).is_ok_and(|app| app.is_control_app())
}

fn autostart_supported() -> bool {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        lingxia_app_context::autostart_enabled() && lingxia_platform::autostart_supported()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

/// Called before namespace injection and before any product Logic runs.
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    if ctx.get_service::<FeatureSnapshot>().is_some() {
        return Ok(());
    }
    let app = LxApp::from_ctx(ctx)?;
    #[cfg(feature = "terminal")]
    let focused = crate::terminal::owns_context(ctx)?;
    #[cfg(not(feature = "terminal"))]
    let focused = false;
    use lingxia_platform::traits::update::UpdateService;
    let entries = registry().as_ref().map_err(js_internal_error)?;
    let supported = resolve(entries, |predicate| {
        if focused && predicate != Predicate::Terminal {
            return false;
        }
        match predicate {
            Predicate::Always => true,
            Predicate::Terminal => focused,
            Predicate::Autostart => autostart_supported(),
            Predicate::Notifications => lingxia_app_context::capability::notifications(),
            Predicate::Banner => app.is_control_app() && lingxia_platform::banner_supported(),
            Predicate::Browser => lingxia_app_context::capability::browser(),
            Predicate::Proxy => lingxia_app_context::capability::proxy(),
            Predicate::SelfUpdate => lingxia_app_context::update::self_update_allowed(
                app.runtime.self_update_supported(),
                app.runtime.installed_from_store(),
            ),
            Predicate::Process => app.process_supported(),
            Predicate::AppUse => lingxia_app_context::capability::app_use(),
            Predicate::ComputerUse => lingxia_app_context::capability::computer_use(),
            Predicate::BrowserUse => lingxia_app_context::capability::browser_use(),
            Predicate::MediaCapture => lingxia_app_context::capability::media_capture(),
            Predicate::Window => crate::surface::window_placement_available(),
            Predicate::FullChrome => crate::surface::window_full_chrome_available(),
        }
    })
    .map_err(js_internal_error)?;
    let context_id = uuid::Uuid::new_v4().to_string();
    app.record_logic_feature_snapshot(context_id.clone(), supported.iter().cloned().collect());
    ctx.set_service(FeatureSnapshot {
        supported,
        app: Arc::downgrade(&app),
        context_id,
    });
    register_api(ctx)
}

pub(crate) fn exposes(ctx: &JSContext, key: &str) -> bool {
    ctx.get_service::<FeatureSnapshot>()
        .is_some_and(|snapshot| snapshot.supported.contains(key))
}

/// Frozen feature support, not permission or current layout. Unknown strings
/// return false; non-strings throw TypeError. Required features also need an
/// appropriate lxapp.json minRuntime.
fn supports(ctx: JSContext, feature: JSValue) -> JSResult<bool> {
    if !feature.is_string() {
        return Err(HostError::new(
            rong::error::E_INVALID_ARG,
            "lx.supports requires a feature string",
        )
        .with_name("TypeError")
        .into());
    }
    Ok(exposes(&ctx, &feature.to_rust::<String>()?))
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn supports(ts_params = "feature: LxFeature", ts_return = "boolean") = supports;
    }
}

// The per-context freeze itself lives in FeatureSnapshot/ctx.set_service and is
// covered by the showcase API test, not from here.
#[cfg(test)]
mod tests {
    use super::*;

    fn entries(json: &str) -> Vec<FeatureEntry> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn registry_is_valid_and_dependencies_are_derived() {
        let entries = registry().as_ref().unwrap();
        let supported = resolve(entries, |predicate| predicate != Predicate::Window).unwrap();
        assert!(!supported.contains("surface.window.fullChrome"));
        assert!(supported.contains("app.notification"));
        let without_browser =
            resolve(entries, |predicate| predicate != Predicate::Browser).unwrap();
        assert!(!without_browser.contains("app.browser"));
        assert!(!without_browser.contains("surface.tab"));
    }

    #[test]
    fn invalid_graphs_fail_even_when_unsupported() {
        for json in [
            r#"[{"key":"a","requires":[],"predicate":"Always"},{"key":"a","requires":[],"predicate":"Always"}]"#,
            r#"[{"key":"a","requires":["missing"],"predicate":"Always"}]"#,
            r#"[{"key":"a","requires":["b"],"predicate":"Always"},{"key":"b","requires":["a"],"predicate":"Always"}]"#,
        ] {
            assert!(resolve(&entries(json), |_| false).is_err());
        }
    }

    #[test]
    fn focused_context_only_exposes_terminal() {
        let snapshot = resolve(registry().as_ref().unwrap(), |predicate| {
            predicate == Predicate::Terminal
        })
        .unwrap();
        assert_eq!(snapshot.into_iter().collect::<Vec<_>>(), vec!["terminal"]);
    }

    #[test]
    fn resolved_keys_are_sorted_and_exclude_unknown_features() {
        let all = resolve(registry().as_ref().unwrap(), |_| true).unwrap();
        let keys = all.iter().collect::<Vec<_>>();
        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(!all.contains(""));
        assert!(!all.contains("future.feature"));
        for baseline in ["app.cache", "surface.main", "surface.float"] {
            assert!(!all.contains(baseline));
        }
        assert!(all.contains("surface.window"));
        assert!(all.contains("surface.window.fullChrome"));

        let without_window = resolve(registry().as_ref().unwrap(), |predicate| {
            predicate != Predicate::Window
        })
        .unwrap();
        assert!(!without_window.contains("surface.window"));
        assert!(!without_window.contains("surface.window.fullChrome"));
    }
}
