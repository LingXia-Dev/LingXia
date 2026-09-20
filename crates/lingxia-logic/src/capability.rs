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
    requires: Vec<String>,
    own: Own,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum Own {
    Always,
    Control,
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
    own: impl Fn(Own) -> bool,
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
        own: &impl Fn(Own) -> bool,
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
        let mut supported = own(entry.own);
        for dependency in &entry.requires {
            supported &= visit(dependency, index, visiting, resolved, own)?;
        }
        visiting.remove(key);
        resolved.insert(key, supported);
        Ok(supported)
    }
    let mut resolved = BTreeMap::new();
    for key in index.keys() {
        visit(key, &index, &mut BTreeSet::new(), &mut resolved, &own)?;
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
    let supported = resolve(entries, |own| {
        if focused && own != Own::Terminal {
            return false;
        }
        match own {
            Own::Always => true,
            Own::Control => app.is_control_app(),
            Own::Terminal => focused,
            Own::Autostart => autostart_supported(),
            Own::Notifications => lingxia_app_context::capability::notifications(),
            Own::Banner => app.is_control_app() && lingxia_platform::banner_supported(),
            Own::Browser => lingxia_app_context::capability::browser(),
            Own::Proxy => lingxia_app_context::capability::proxy(),
            Own::SelfUpdate => lingxia_app_context::update::self_update_allowed(
                app.runtime.self_update_supported(),
                app.runtime.installed_from_store(),
            ),
            Own::Process => app.process_supported(),
            Own::AppUse => lingxia_app_context::capability::app_use(),
            Own::ComputerUse => lingxia_app_context::capability::computer_use(),
            Own::BrowserUse => lingxia_app_context::capability::browser_use(),
            Own::MediaCapture => lingxia_app_context::capability::media_capture(),
            Own::Window => crate::surface::window_placement_available(),
            Own::FullChrome => crate::surface::window_full_chrome_available(),
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
/// appropriate lxapp.json minRuntime. The string API requires 0.18.0 or later.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(json: &str) -> Vec<FeatureEntry> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn registry_is_valid_and_dependencies_are_derived() {
        let entries = registry().as_ref().unwrap();
        let supported = resolve(entries, |own| own != Own::Window).unwrap();
        assert!(!supported.contains("surface.window.fullChrome"));
        assert!(supported.contains("app.notification"));
        assert!(!supported.contains("app.notification.routeTarget"));
    }

    #[test]
    fn invalid_graphs_fail_even_when_unsupported() {
        for json in [
            r#"[{"key":"a","requires":[],"own":"Always"},{"key":"a","requires":[],"own":"Always"}]"#,
            r#"[{"key":"a","requires":["missing"],"own":"Always"}]"#,
            r#"[{"key":"a","requires":["b"],"own":"Always"},{"key":"b","requires":["a"],"own":"Always"}]"#,
        ] {
            assert!(resolve(&entries(json), |_| false).is_err());
        }
    }

    #[test]
    fn focused_context_only_exposes_terminal() {
        let snapshot = resolve(registry().as_ref().unwrap(), |own| own == Own::Terminal).unwrap();
        assert_eq!(snapshot.into_iter().collect::<Vec<_>>(), vec!["terminal"]);
    }

    #[test]
    fn snapshot_is_sorted_and_does_not_recompute() {
        let enabled = std::cell::Cell::new(true);
        let snapshot = resolve(registry().as_ref().unwrap(), |_| enabled.get()).unwrap();
        enabled.set(false);
        assert!(snapshot.contains("surface.window"));
        assert!(!snapshot.contains(""));
        assert!(!snapshot.contains("future.feature"));
        let next = resolve(registry().as_ref().unwrap(), |_| enabled.get()).unwrap();
        assert!(next.is_empty());
    }
}
