//! `lx.supports(query)` — one capability query for every namespace.
//!
//! Runtime dispatch is emitted from the registry below, and a test keeps its
//! generated TypeScript metadata in lockstep. The answer is read live: `aside`
//! genuinely changes when a desktop window crosses the compact breakpoint. It
//! is an affordance for building UI, never a substitute for handling a
//! rejection.

use crate::i18n::{js_internal_error, js_invalid_parameter_error};
use lxapp::LxApp;
use rong::{JSContext, JSResult, JSValue};
use std::sync::Arc;

/// Values `{ capability: 'surface', value: … }` accepts, in type order.
const SURFACE_PLACEMENTS: &[&str] = &["main", "aside", "float", "window", "tab"];

/// Window decorations `{ surface: 'window', chrome }` may ask about.
const WINDOW_CHROMES: &[&str] = &["system", "full"];

/// Declares the boolean capabilities: the JS key, the TS doc line, and the
/// predicate that answers it.
macro_rules! flag_capabilities {
    ($( $(#[doc = $doc:literal])* $key:literal => $eval:expr );* $(;)?) => {
        /// Every declared flag key, in union order.
        const FLAG_KEYS: &[&str] = &[$($key),*];

        fn flag_supported(key: &str, lxapp: &Arc<LxApp>) -> Option<bool> {
            let _ = lxapp;
            match key {
                $($key => Some($eval(lxapp)),)*
                _ => None,
            }
        }
    };
}

flag_capabilities! {
    "terminal" => |lxapp: &Arc<LxApp>| terminal_supported(lxapp);
    "autostart" => |_: &Arc<LxApp>| autostart_supported();
    "notifications" => |_: &Arc<LxApp>| lingxia_app_context::capability::notifications();
    "browser" => |_: &Arc<LxApp>| lingxia_app_context::capability::browser();
    "proxy" => |_: &Arc<LxApp>| lingxia_app_context::capability::proxy();
    "selfUpdate" => |lxapp: &Arc<LxApp>| self_update_supported(lxapp);
    "process" => |lxapp: &Arc<LxApp>| lxapp.process_supported();
    "appUse" => |_: &Arc<LxApp>| lingxia_app_context::capability::app_use();
    "computerUse" => |_: &Arc<LxApp>| lingxia_app_context::capability::computer_use();
    "browserUse" => |_: &Arc<LxApp>| lingxia_app_context::capability::browser_use();
}

/// `lx.terminal`'s presence check, so the two can never disagree.
fn terminal_supported(lxapp: &Arc<LxApp>) -> bool {
    #[cfg(feature = "terminal")]
    {
        crate::terminal::eligible(lxapp)
    }
    #[cfg(not(feature = "terminal"))]
    {
        let _ = lxapp;
        false
    }
}

/// `lx.app.autostart`'s presence check, so the two can never disagree. Fenced
/// exactly like the member: `lingxia_platform::autostart_supported` only
/// exists where a startup item can exist at all.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn autostart_supported() -> bool {
    lingxia_app_context::autostart_enabled() && lingxia_platform::autostart_supported()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn autostart_supported() -> bool {
    false
}

fn self_update_supported(lxapp: &Arc<LxApp>) -> bool {
    use lingxia_platform::traits::update::UpdateService;
    lxapp.runtime.self_update_supported()
}

/// Answers `{ capability: 'surface', value: … }`, optionally qualified by
/// `chrome`. Only `aside` is width-dependent; `window` is a property of the
/// host build and does not flicker as a window is resized.
fn surface_supported(placement: &str, chrome: Option<&str>, lxapp: &Arc<LxApp>) -> bool {
    if let Some(chrome) = chrome
        && chrome == "full"
        && !crate::surface::window_full_chrome_available()
    {
        return false;
    }
    match placement {
        // Every host can put content in the main region or float it over one.
        "main" | "float" => true,
        "tab" => lingxia_app_context::capability::browser(),
        "aside" => crate::surface::aside_dock_available(lxapp),
        "window" => crate::surface::window_placement_available(),
        // The caller rejects anything outside `SURFACE_PLACEMENTS` before
        // reaching here. Answering false keeps an unlisted placement
        // unadvertised rather than advertising it by falling through.
        _ => false,
    }
}

/// A focused runtime context registers only part of the Logic surface. It must
/// not advertise the operations it left out, or the query would disagree with
/// what is actually callable there.
fn context_exposes_capability(key: &str, terminal_settings: bool) -> bool {
    !terminal_settings || key == "terminal"
}

/// The catalog as an error message renders it. One spelling, so a caller who
/// hits two different rejections does not see the same list two ways.
fn capability_catalog() -> String {
    let mut names = vec!["surface"];
    names.extend_from_slice(FLAG_KEYS);
    names.join(", ")
}

fn has_exact_keys(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len()
        && expected
            .iter()
            .all(|expected| actual.iter().any(|actual| actual == expected))
}

/// Whether this host exposes a capability to this Logic context, right now.
///
/// Synchronous, because it is meant to be called from render paths. The answer
/// is live and may be stale by the time you act on it — it is an affordance for
/// deciding what to render, not a replacement for handling a rejection.
/// `{ capability: 'surface', value: 'aside' }` in particular changes when a
/// desktop window crosses the compact breakpoint; pair it with
/// `lx.onSurfaceContext` instead of polling. The answer is per runtime context:
/// a context that does not expose an API reports false for it.
fn supports(ctx: JSContext, query: JSValue) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let terminal_settings = terminal_supported(&lxapp);
    // Taken as a value, not a `JSObject`: an untyped caller passing a string or
    // null would otherwise be rejected by argument conversion, with a shape
    // that does not match the invalid-parameter errors every other bad query
    // reports.
    let Some(query) = query.into_object() else {
        return Err(js_invalid_parameter_error(format!(
            "lx.supports requires a query object with a capability: {}",
            capability_catalog()
        )));
    };
    let keys = query.keys_as::<String>()?;
    let capability = query.get::<_, String>("capability").map_err(|_| {
        js_invalid_parameter_error(format!(
            "lx.supports requires a string capability: {}",
            capability_catalog()
        ))
    })?;

    if capability == "surface" {
        let chrome = query.get_opt::<_, String>("chrome")?;
        let expected: &[&str] = if chrome.is_some() {
            &["capability", "value", "chrome"]
        } else {
            &["capability", "value"]
        };
        if !has_exact_keys(&keys, expected) {
            return Err(js_invalid_parameter_error(
                "surface capability query requires `capability` and `value`, and accepts only `chrome` besides",
            ));
        }
        let placement = query.get::<_, String>("value").map_err(|_| {
            js_invalid_parameter_error("surface capability `value` must be a string")
        })?;
        if !SURFACE_PLACEMENTS.contains(&placement.as_str()) {
            return Err(js_invalid_parameter_error(format!(
                "unknown surface placement '{placement}'; expected {}",
                SURFACE_PLACEMENTS.join(", ")
            )));
        }
        if let Some(chrome) = chrome.as_deref() {
            if !WINDOW_CHROMES.contains(&chrome) {
                return Err(js_invalid_parameter_error(format!(
                    "unknown window chrome '{chrome}'; expected {}",
                    WINDOW_CHROMES.join(", ")
                )));
            }
            // The type says the same thing; an untyped caller hears it here.
            if placement != "window" {
                return Err(js_invalid_parameter_error(
                    "`chrome` qualifies `value: 'window'` and no other placement",
                ));
            }
        }
        if !context_exposes_capability(&capability, terminal_settings) {
            return Ok(false);
        }
        return Ok(surface_supported(&placement, chrome.as_deref(), &lxapp));
    }

    if !FLAG_KEYS.contains(&capability.as_str()) {
        return Err(js_invalid_parameter_error(format!(
            "unknown capability '{capability}'; expected {}",
            capability_catalog()
        )));
    }
    if !has_exact_keys(&keys, &["capability"]) {
        return Err(js_invalid_parameter_error(format!(
            "capability '{capability}' does not accept additional options"
        )));
    }
    if !context_exposes_capability(&capability, terminal_settings) {
        return Ok(false);
    }

    // Membership was checked against FLAG_KEYS above, which is emitted by the
    // same macro as this dispatch. Keep a hard tripwire instead of converting
    // any future registry defect into an unsupported answer.
    flag_supported(&capability, &lxapp).ok_or_else(|| {
        js_internal_error(format!(
            "capability registry has no predicate for declared key '{capability}'"
        ))
    })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;

        /// Boolean capability names accepted by `lx.supports`.
        type LxCapabilityFlag = r###"'terminal' | 'autostart' | 'notifications' | 'browser' | 'proxy' | 'selfUpdate' | 'process' | 'appUse' | 'computerUse' | 'browserUse'"###;

        /// Surface placements accepted by `lx.supports`.
        type LxSurfaceCapability = r###"'main' | 'aside' | 'float' | 'window' | 'tab'"###;

        /// One capability question per call. The catalog is closed, so
        /// completion enumerates it and a typo is a type error. `capability`
        /// is the discriminant; only the `surface` branch accepts a `value`.
        ///
        /// Two surface answers describe an *affordance*, not whether the call
        /// succeeds: `tab` is "the host has an in-app browser" — without it a
        /// url still opens, in the OS browser instead — and `aside` is "a
        /// docked region exists right now", while a compact layout still opens
        /// the url through the in-app browser's own chrome. Ask them to decide
        /// what to render, not whether to call.
        ///
        /// `chrome` qualifies a window and only a window: it asks whether this
        /// host can produce that decoration, not merely a window.
        ///
        type LxCapabilityQuery = r###"{
    capability: 'surface';
    value: 'window';
    chrome?: WindowChrome;
} | {
    capability: 'surface';
    value: Exclude<LxSurfaceCapability, 'window'>;
} | {
    capability: LxCapabilityFlag;
}"###;

        fn supports(
            ts_params = "query: LxCapabilityQuery",
            ts_return = "boolean"
        ) = supports;
    }
}

#[cfg(test)]
mod tests {
    use super::{FLAG_KEYS, SURFACE_PLACEMENTS, context_exposes_capability, has_exact_keys};

    /// `js_api!` needs type metadata as a string literal. Assert that its flag
    /// and surface catalogs still match the runtime registry.
    #[test]
    fn declared_union_matches_the_registry() {
        let source = include_str!("capability.rs");
        let declared_flags = source
            .split("type LxCapabilityFlag = r###\"")
            .nth(1)
            .and_then(|rest| rest.split("\"###").next())
            .expect("LxCapabilityFlag literal");

        let expected_flags = FLAG_KEYS
            .iter()
            .map(|value| format!("'{value}'"))
            .collect::<Vec<_>>()
            .join(" | ");
        assert_eq!(declared_flags, expected_flags);

        let declared_surfaces = source
            .split("type LxSurfaceCapability = r###\"")
            .nth(1)
            .and_then(|rest| rest.split("\"###").next())
            .expect("LxSurfaceCapability literal");
        let expected_surfaces = SURFACE_PLACEMENTS
            .iter()
            .map(|value| format!("'{value}'"))
            .collect::<Vec<_>>()
            .join(" | ");
        assert_eq!(declared_surfaces, expected_surfaces);
    }

    #[test]
    fn focused_terminal_context_advertises_only_its_registered_api() {
        assert!(context_exposes_capability("terminal", true));
        assert!(!context_exposes_capability("surface", true));
        assert!(!context_exposes_capability("autostart", true));
        assert!(context_exposes_capability("surface", false));
    }

    #[test]
    fn query_shape_requires_only_the_branch_fields() {
        let surface = vec!["value".to_string(), "capability".to_string()];
        let flag = vec!["capability".to_string()];
        let extra = vec!["capability".to_string(), "value".to_string()];

        assert!(has_exact_keys(&surface, &["capability", "value"]));
        assert!(has_exact_keys(&flag, &["capability"]));
        assert!(!has_exact_keys(&extra, &["capability"]));
    }
}
