//! Where a tap goes.
//!
//! A product registers its internal locations once at startup and then names
//! them from anywhere — a notification, a menu item, the tray — instead of
//! spelling out an HTTPS URL for something that never leaves the device.
//!
//! ```no_run
//! use lingxia::navigation::{NavigationRoute, NavigationRoutes, RouteParam};
//!
//! fn routes(routes: &mut NavigationRoutes) -> Result<(), String> {
//!     routes.add(
//!         NavigationRoute::new("downloads.detail", |request| {
//!             let id = request.param_str("downloadId").unwrap_or_default();
//!             open_download_window(id);
//!             Ok(())
//!         })
//!         .param(RouteParam::string("downloadId")),
//!     )
//! }
//! # fn open_download_window(_id: &str) {}
//! ```
//!
//! Handlers navigate and nothing else. A side effect the user has to approve
//! belongs behind the screen the handler opens, not in the handler.

use lingxia_service::navigation as service;

pub use lingxia_service::navigation::{
    NavigationError, NavigationRequest, NavigationRoute, NavigationRoutes, NavigationSource,
    NavigationTarget, RouteParam, RouteParamKind, install_activate_handler,
};

/// Open a target from a product entry point such as a menu or the tray.
///
/// Validates against the sealed route registry (or the App Link host
/// allowlist) and waits for the runtime when it is still starting.
pub fn open(target: NavigationTarget, source: NavigationSource) -> crate::Result<()> {
    service::dispatch(NavigationRequest::new(target, source))
        .map_err(|error| crate::Error::invalid_request(error.to_string()))
}

/// A route that opens one page of one lxapp.
///
/// The lxapp and the page are fixed here, at registration: a caller names the
/// route, never a path. Schema-checked parameters become the page query, so
/// the page reads them exactly as it reads any other launch query. The scene
/// is the ordinary one — an internal route is not an App Link and does not
/// pretend to be `scene === 8003`.
///
/// ```no_run
/// use lingxia::navigation::{NavigationRoutes, RouteParam, lxapp_page_route};
///
/// fn routes(routes: &mut NavigationRoutes) -> Result<(), String> {
///     routes.add(
///         lxapp_page_route("orders.detail", "com.example.shop", "pages/order/index")
///             .param(RouteParam::string("orderId")),
///     )
/// }
/// ```
pub fn lxapp_page_route(
    name: impl Into<String>,
    appid: impl Into<String>,
    path: impl Into<String>,
) -> NavigationRoute {
    let appid = appid.into();
    let path = path.into();
    NavigationRoute::new(name, move |request| {
        let options = lxapp::LxAppStartupOptions::new(&path).set_query(page_query(request));
        let appid = appid.clone();
        std::mem::drop(crate::task::spawn(async move {
            if let Err(error) = lxapp::prepare_lxapp_open(&appid, options.release_type).await {
                lxapp::notify_lxapp_open_blocked(&error);
                return;
            }
            if let Err(error) = lxapp::open_lxapp(&appid, options) {
                log::warn!("navigation route could not open {appid}: {error}");
            }
        }));
        Ok(())
    })
}

/// Route parameters as a page query. Only what the route declared is here, so
/// there is nothing to filter — but values are still encoded, because a route
/// parameter is a value, never query syntax.
fn page_query(request: &NavigationRequest) -> String {
    let NavigationTarget::Route { params, .. } = &request.target else {
        return String::new();
    };
    params
        .iter()
        .map(|(key, value)| {
            let text = match value {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            format!("{}={}", encode_component(key), encode_component(&text))
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}

/// Check a target without opening it. Same rules the dispatcher applies.
pub fn validate(target: &NavigationTarget) -> crate::Result<()> {
    service::validate(target).map_err(|error| crate::Error::invalid_request(error.to_string()))
}

/// The route names this build registered, sorted. For diagnostics.
pub fn route_names() -> Vec<String> {
    service::route_names()
}

/// Open what an OS notification tap was carrying.
///
/// Platform SDK entry points call this with the token the OS handed back.
/// Returns `1` when a target was dispatched, `0` when the token no longer
/// resolves — the product still comes forward and says why.
pub fn activate_notification(token: &str) -> i32 {
    service::activate_notification(token)
}

/// Collect host routes, seal the registry, and point the intent store at the
/// product's private state directory.
pub(crate) fn install(state_dir: std::path::PathBuf) -> crate::Result<()> {
    service::intent::init(state_dir);
    service::intent::recover();
    let mut routes = NavigationRoutes::new();
    crate::host_addon::run_install_navigation_routes(&mut routes)
        .map_err(|error| crate::Error::internal(format!("navigation route: {error}")))?;
    let count = routes.len();
    service::install(routes).map_err(crate::Error::internal)?;
    log::info!("navigation registry sealed with {count} route(s)");
    Ok(())
}

/// The runtime can open targets now. Drains whatever a cold-start tap queued.
pub(crate) fn mark_ready() {
    service::mark_ready();
}

/// Say when a target cannot be opened.
///
/// Nothing here brings the product forward: every platform entry point
/// already activates the host before it hands the token back, so a tap that
/// resolves to nothing still lands on a visible product.
pub(crate) fn install_handlers() {
    service::install_feedback_handler(|error| report_unavailable(error.message()));
}

fn report_unavailable(detail: &str) {
    log::warn!("navigation target unavailable: {detail}");
    let Ok(platform) = crate::runtime::platform() else {
        return;
    };
    use lingxia_platform::traits::ui::{ToastIcon, ToastOptions, ToastPosition, UserFeedback};
    let _ = platform.show_toast(ToastOptions {
        title: unavailable_message(),
        icon: ToastIcon::Error,
        image: None,
        duration: 2500.0,
        mask: false,
        position: ToastPosition::Center,
    });
}

#[cfg(feature = "standard")]
fn unavailable_message() -> String {
    lingxia_logic::I18nKey::NotificationTargetUnavailable
        .get(&lxapp::display_language())
        .to_string()
}

#[cfg(not(feature = "standard"))]
fn unavailable_message() -> String {
    "This is no longer available.".to_string()
}
