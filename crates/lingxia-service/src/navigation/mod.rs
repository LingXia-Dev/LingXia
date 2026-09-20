//! Product navigation: where a tap goes, and how it gets there.
//!
//! Notifications, menus, the tray, and inbound App Links describe a
//! [`NavigationTarget`]; this module validates it, waits for the host to be
//! ready, and hands it to the registered handler. It is product navigation
//! infrastructure, not part of any one of those entry points.

pub mod intent;

mod dispatch;
mod registry;
mod target;

pub use dispatch::{
    NavigationRequest, NavigationSource, activate, dispatch, install_activate_handler,
    install_feedback_handler, is_ready, mark_ready, report_unavailable, validate,
};
pub use registry::{
    NavigationRoute, NavigationRoutes, RouteHandler, RouteParam, RouteParamKind, install,
    is_sealed, route_names,
};
pub use target::{
    MAX_ROUTE_NAME_CHARS, MAX_ROUTE_PARAM_COUNT, MAX_ROUTE_PARAM_DEPTH, MAX_ROUTE_PARAMS_BYTES,
    NavigationError, NavigationTarget,
};

/// Open what a notification tap was carrying.
///
/// The tap itself means "come forward", so the product activates whatever the
/// token resolves to — including a token that no longer resolves, which
/// activates and reports why instead of navigating somewhere else.
///
/// Returns `1` when a target was dispatched, `0` when the token was unknown,
/// already consumed, or its target is gone.
pub fn activate_notification(token: &str) -> i32 {
    activate();
    let token = token.trim();
    if token.is_empty() {
        return 0;
    }
    if !intent::is_initialized() {
        intent::defer(token);
        return 1;
    }
    open_resolved_token(token)
}

/// Replay taps that arrived before [`intent::init`]. Call after the store and
/// the route registry are up; each token then takes the ordinary path.
pub fn drain_deferred_activations() {
    for token in intent::take_deferred() {
        let _ = activate_notification(&token);
    }
}

fn open_resolved_token(token: &str) -> i32 {
    let resolved = match intent::resolve(token) {
        Ok(Some(resolved)) => resolved,
        // The OS delivered this tap twice. The first one already navigated.
        Ok(None) => {
            log::debug!("merging a repeat notification activation");
            return 1;
        }
        Err(error) => {
            report_unavailable(&error);
            return 0;
        }
    };
    log::info!(
        "notification {} tapped: {}",
        resolved.id,
        resolved.target.describe()
    );
    match dispatch(NavigationRequest::from_notification(
        resolved.target,
        resolved.id,
    )) {
        Ok(()) => 1,
        Err(error) => {
            report_unavailable(&error);
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tap_before_the_store_exists_is_not_reported_gone() {
        intent::with_uninitialized_store(|| {
            assert_eq!(activate_notification("early-token"), 1);
            assert_eq!(intent::take_deferred(), vec!["early-token".to_string()]);
        });
    }
}
