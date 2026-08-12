//! `lx.supports(query)` — one capability query for every namespace.
//!
//! Each capability is declared once below, and both the runtime dispatch and
//! the `LxCapabilityQuery` union are derived from that declaration. The answer
//! is read live: `aside` genuinely changes when a desktop window crosses the
//! compact breakpoint. It is an affordance for building UI, never a substitute
//! for handling a rejection.

use crate::i18n::js_invalid_parameter_error;
use lxapp::LxApp;
use rong::{JSContext, JSObject, JSResult};
use std::sync::Arc;

/// Placements `{ surface: … }` accepts, in the order they appear in the union.
const SURFACE_PLACEMENTS: &[&str] = &["main", "aside", "float", "window", "tab"];

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
    "nativeFileReview" => |lxapp: &Arc<LxApp>| native_file_review_supported(lxapp);
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

/// `lx.app.autostart`'s presence check, so the two can never disagree.
fn autostart_supported() -> bool {
    lingxia_app_context::autostart_enabled() && lingxia_platform::autostart_supported()
}

fn self_update_supported(lxapp: &Arc<LxApp>) -> bool {
    use lingxia_platform::traits::update::UpdateService;
    lxapp.runtime.self_update_supported()
}

fn native_file_review_supported(lxapp: &Arc<LxApp>) -> bool {
    use lingxia_platform::traits::file::FileService;
    lxapp.runtime.native_review_supported()
}

/// Answers `{ surface: … }`. Only `aside` is width-dependent; `window` is a
/// property of the host build and does not flicker as a window is resized.
fn surface_supported(placement: &str, lxapp: &Arc<LxApp>) -> bool {
    if !SURFACE_PLACEMENTS.contains(&placement) {
        return false;
    }
    match placement {
        "tab" => lingxia_app_context::capability::browser(),
        "aside" => crate::surface::aside_dock_available(lxapp),
        "window" => crate::surface::window_placement_available(),
        // `main` and `float` are the always-available placements.
        _ => true,
    }
}

/// Whether this host can do something, right now.
///
/// Synchronous, because the callers are render paths and menu construction. The
/// answer is live and may be stale by the time you act on it — it is an
/// affordance for deciding what to render, not a replacement for handling a
/// rejection. `{ surface: 'aside' }` in particular changes when a desktop
/// window crosses the compact breakpoint; pair it with `lx.onSurfaceContext`
/// instead of polling.
fn supports(ctx: JSContext, query: JSObject) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    if let Some(placement) = query.get_opt::<_, String>("surface")? {
        return Ok(surface_supported(&placement, &lxapp));
    }

    for key in FLAG_KEYS {
        if query.get_opt::<_, bool>(*key)?.is_some() {
            return Ok(flag_supported(key, &lxapp).unwrap_or(false));
        }
    }

    Err(js_invalid_parameter_error(format!(
        "lx.supports expects one capability key: surface, {}",
        FLAG_KEYS.join(", ")
    )))
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;

        /// One capability question per call. The catalog is a closed union, so
        /// completion enumerates it and a typo is a compile error.
        ///
        type LxCapabilityQuery = r###"{ surface: 'main' | 'aside' | 'float' | 'window' | 'tab' }
  | { terminal: true }
  | { autostart: true }
  | { notifications: true }
  | { browser: true }
  | { proxy: true }
  | { selfUpdate: true }
  | { nativeFileReview: true }"###;

        fn supports(
            ts_params = "query: LxCapabilityQuery",
            ts_return = "boolean"
        ) = supports;
    }
}

#[cfg(test)]
mod tests {
    use super::{FLAG_KEYS, SURFACE_PLACEMENTS};

    /// `js_api!` needs the union as a string literal, so it cannot be pasted
    /// from the declarations above. Assert instead that it still matches them,
    /// so a capability can never be added without appearing in the type.
    #[test]
    fn declared_union_matches_the_registry() {
        let source = include_str!("capability.rs");
        let declared = source
            .split("type LxCapabilityQuery = r###\"")
            .nth(1)
            .and_then(|rest| rest.split("\"###").next())
            .expect("LxCapabilityQuery literal");

        let mut expected = format!(
            "{{ surface: {} }}",
            SURFACE_PLACEMENTS
                .iter()
                .map(|value| format!("'{value}'"))
                .collect::<Vec<_>>()
                .join(" | ")
        );
        for key in FLAG_KEYS {
            expected.push_str(&format!("\n  | {{ {key}: true }}"));
        }

        assert_eq!(declared, expected);
    }
}
