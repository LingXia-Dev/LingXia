use crate::i18n::{js_error_from_business_code_with_detail, js_invalid_parameter_error};
use lingxia_service::navigation::{NavigationError, NavigationTarget};
use lxapp::AppSessionClass;
use lxapp::host::{self, EffectiveRoutePolicy, HostInvocationContext, RouteAudience};
use rong::{JSContext, JSResult};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LogicRoute {
    AppExit,
    AppGetServiceEnv,
    AppToggleServiceEnv,
    AppSetBadge,
    AppCacheSize,
    AppCacheClear,
    AppGetAppearancePreference,
    AppSetAppearancePreference,
    AppWatchAppearancePreference,
    AppGetDisplayLanguagePreference,
    AppSetDisplayLanguagePreference,
    AppWatchDisplayLanguagePreference,
    AppCheckUpdate,
    AppApplyUpdate,
    AppScreenshot,
    AppAutostartIsEnabled,
    AppAutostartSetEnabled,
    AppNotificationGetPermission,
    AppNotificationRequestPermission,
    AppNotificationShow,
    AppNotificationCancel,
    AppNotificationCancelAll,
    AppBannerShow,
    AppBannerDismiss,
    TraySetIcon,
    TraySetTitle,
    TraySetMenu,
    TrayOnClick,
    TrayShow,
    TrayHide,
    ShellSidebarReplace,
    ShellSidebarUpdate,
    ShellSidebarRemove,
    ShellSidebarClear,
    ShellOpenApp,
    ShellOpenBuiltin,
    ShellOpenDeclared,
    ShellReconfigure,
    ShellOpenHostTerminalSettings,
    SurfaceBringToFront,
    #[cfg(feature = "terminal")]
    TerminalSettingsGet,
    #[cfg(feature = "terminal")]
    TerminalSettingsUpdate,
    #[cfg(feature = "terminal")]
    TerminalSettingsReset,
    #[cfg(feature = "terminal")]
    TerminalSettingsOnChange,
    #[cfg(feature = "terminal")]
    TerminalSchemesList,
    #[cfg(feature = "terminal")]
    TerminalSchemesImport,
    #[cfg(feature = "terminal")]
    TerminalPreviewCreate,
    #[cfg(feature = "terminal")]
    TerminalPreviewShow,
    #[cfg(feature = "terminal")]
    TerminalPreviewClear,
    #[cfg(feature = "terminal")]
    TerminalPreviewClose,
    #[cfg(feature = "terminal")]
    TerminalFontsList,
    #[cfg(all(feature = "terminal", target_os = "windows"))]
    TerminalWindowsStatus,
    #[cfg(all(feature = "terminal", target_os = "windows"))]
    TerminalWindowsInstall,
    #[cfg(all(feature = "terminal", target_os = "windows"))]
    TerminalWindowsSetEnabled,
}

impl LogicRoute {
    pub(crate) const ALL: &'static [Self] = &[
        Self::AppExit,
        Self::AppGetServiceEnv,
        Self::AppToggleServiceEnv,
        Self::AppSetBadge,
        Self::AppCacheSize,
        Self::AppCacheClear,
        Self::AppGetAppearancePreference,
        Self::AppSetAppearancePreference,
        Self::AppWatchAppearancePreference,
        Self::AppGetDisplayLanguagePreference,
        Self::AppSetDisplayLanguagePreference,
        Self::AppWatchDisplayLanguagePreference,
        Self::AppCheckUpdate,
        Self::AppApplyUpdate,
        Self::AppScreenshot,
        Self::AppAutostartIsEnabled,
        Self::AppAutostartSetEnabled,
        Self::AppNotificationGetPermission,
        Self::AppNotificationRequestPermission,
        Self::AppNotificationShow,
        Self::AppNotificationCancel,
        Self::AppNotificationCancelAll,
        Self::AppBannerShow,
        Self::AppBannerDismiss,
        Self::TraySetIcon,
        Self::TraySetTitle,
        Self::TraySetMenu,
        Self::TrayOnClick,
        Self::TrayShow,
        Self::TrayHide,
        Self::ShellSidebarReplace,
        Self::ShellSidebarUpdate,
        Self::ShellSidebarRemove,
        Self::ShellSidebarClear,
        Self::ShellOpenApp,
        Self::ShellOpenBuiltin,
        Self::ShellOpenDeclared,
        Self::ShellReconfigure,
        Self::ShellOpenHostTerminalSettings,
        Self::SurfaceBringToFront,
        #[cfg(feature = "terminal")]
        Self::TerminalSettingsGet,
        #[cfg(feature = "terminal")]
        Self::TerminalSettingsUpdate,
        #[cfg(feature = "terminal")]
        Self::TerminalSettingsReset,
        #[cfg(feature = "terminal")]
        Self::TerminalSettingsOnChange,
        #[cfg(feature = "terminal")]
        Self::TerminalSchemesList,
        #[cfg(feature = "terminal")]
        Self::TerminalSchemesImport,
        #[cfg(feature = "terminal")]
        Self::TerminalPreviewCreate,
        #[cfg(feature = "terminal")]
        Self::TerminalPreviewShow,
        #[cfg(feature = "terminal")]
        Self::TerminalPreviewClear,
        #[cfg(feature = "terminal")]
        Self::TerminalPreviewClose,
        #[cfg(feature = "terminal")]
        Self::TerminalFontsList,
        #[cfg(all(feature = "terminal", target_os = "windows"))]
        Self::TerminalWindowsStatus,
        #[cfg(all(feature = "terminal", target_os = "windows"))]
        Self::TerminalWindowsInstall,
        #[cfg(all(feature = "terminal", target_os = "windows"))]
        Self::TerminalWindowsSetEnabled,
    ];

    /// `lx.terminal.*` serves the host-bundled Settings surface. The
    /// attention routes (`lx.host.notification.*`, `lx.host.banner.*`,
    /// `lx.surface.bringToFront`) admit every lxapp session and then scope
    /// what each may touch with [`AttentionScope`]. Every other direct route
    /// belongs to the ControlApp.
    ///
    /// Deliberately exhaustive: a catch-all arm would silently make the next
    /// route ControlApp-only, which is a breaking change for every guest
    /// lxapp that expected to call it.
    pub(crate) const fn audience(self) -> RouteAudience {
        match self {
            Self::AppExit
            | Self::AppGetServiceEnv
            | Self::AppToggleServiceEnv
            | Self::AppSetBadge
            | Self::AppCacheSize
            | Self::AppCacheClear
            | Self::AppGetAppearancePreference
            | Self::AppSetAppearancePreference
            | Self::AppWatchAppearancePreference
            | Self::AppGetDisplayLanguagePreference
            | Self::AppSetDisplayLanguagePreference
            | Self::AppWatchDisplayLanguagePreference
            | Self::AppCheckUpdate
            | Self::AppApplyUpdate
            | Self::AppScreenshot
            | Self::AppAutostartIsEnabled
            | Self::AppAutostartSetEnabled
            | Self::TraySetIcon
            | Self::TraySetTitle
            | Self::TraySetMenu
            | Self::TrayOnClick
            | Self::TrayShow
            | Self::TrayHide
            | Self::ShellSidebarReplace
            | Self::ShellSidebarUpdate
            | Self::ShellSidebarRemove
            | Self::ShellSidebarClear
            | Self::ShellOpenApp
            | Self::ShellOpenBuiltin
            | Self::ShellOpenDeclared
            | Self::ShellReconfigure
            | Self::ShellOpenHostTerminalSettings => RouteAudience::ControlAppOnly,
            Self::AppNotificationGetPermission
            | Self::AppNotificationRequestPermission
            | Self::AppNotificationShow
            | Self::AppNotificationCancel
            | Self::AppNotificationCancelAll
            | Self::AppBannerShow
            | Self::AppBannerDismiss
            | Self::SurfaceBringToFront => RouteAudience::AppSessionOnly,
            #[cfg(feature = "terminal")]
            Self::TerminalSettingsGet
            | Self::TerminalSettingsUpdate
            | Self::TerminalSettingsReset
            | Self::TerminalSettingsOnChange
            | Self::TerminalSchemesList
            | Self::TerminalSchemesImport
            | Self::TerminalPreviewCreate
            | Self::TerminalPreviewShow
            | Self::TerminalPreviewClear
            | Self::TerminalPreviewClose
            | Self::TerminalFontsList => RouteAudience::ControlSurfaceOnly,
            #[cfg(all(feature = "terminal", target_os = "windows"))]
            Self::TerminalWindowsStatus
            | Self::TerminalWindowsInstall
            | Self::TerminalWindowsSetEnabled => RouteAudience::ControlSurfaceOnly,
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::AppExit => "lx.host.exit",
            Self::AppGetServiceEnv => "lx.host.getServiceEnv",
            Self::AppToggleServiceEnv => "lx.host.toggleServiceEnv",
            Self::AppSetBadge => "lx.host.setBadge",
            Self::AppCacheSize => "lx.host.cache.size",
            Self::AppCacheClear => "lx.host.cache.clear",
            Self::AppGetAppearancePreference => "lx.host.control.appearance.getPreference",
            Self::AppSetAppearancePreference => "lx.host.control.appearance.setPreference",
            Self::AppWatchAppearancePreference => "lx.host.control.appearance.watchPreference",
            Self::AppGetDisplayLanguagePreference => {
                "lx.host.control.displayLanguage.getPreference"
            }
            Self::AppSetDisplayLanguagePreference => {
                "lx.host.control.displayLanguage.setPreference"
            }
            Self::AppWatchDisplayLanguagePreference => {
                "lx.host.control.displayLanguage.watchPreference"
            }
            Self::AppCheckUpdate => "lx.host.checkUpdate",
            Self::AppApplyUpdate => "lx.host.checkUpdate.apply",
            Self::AppScreenshot => "lx.host.screenshot",
            Self::AppAutostartIsEnabled => "lx.host.autostart.isEnabled",
            Self::AppAutostartSetEnabled => "lx.host.autostart.setEnabled",
            Self::AppNotificationGetPermission => "lx.host.notification.getPermission",
            Self::AppNotificationRequestPermission => "lx.host.notification.requestPermission",
            Self::AppNotificationShow => "lx.host.notification.show",
            Self::AppNotificationCancel => "lx.host.notification.cancel",
            Self::AppNotificationCancelAll => "lx.host.notification.cancelAll",
            Self::AppBannerShow => "lx.host.banner.show",
            Self::AppBannerDismiss => "lx.host.banner.dismiss",
            Self::TraySetIcon => "lx.tray.setIcon",
            Self::TraySetTitle => "lx.tray.setTitle",
            Self::TraySetMenu => "lx.tray.setMenu",
            Self::TrayOnClick => "lx.tray.onClick",
            Self::TrayShow => "lx.tray.show",
            Self::TrayHide => "lx.tray.hide",
            Self::ShellSidebarReplace => "lx.shell.sidebarActions.replace",
            Self::ShellSidebarUpdate => "lx.shell.sidebarActions.update",
            Self::ShellSidebarRemove => "lx.shell.sidebarActions.remove",
            Self::ShellSidebarClear => "lx.shell.sidebarActions.clear",
            Self::ShellOpenApp => "lx.shell.openApp",
            Self::ShellOpenBuiltin => "lx.shell.openBuiltin",
            Self::ShellOpenDeclared => "lx.shell.openDeclared",
            Self::ShellReconfigure => "lx.shell.reconfigure",
            Self::ShellOpenHostTerminalSettings => "lx.shell.openApp.hostTerminalSettings",
            Self::SurfaceBringToFront => "lx.surface.bringToFront",
            #[cfg(feature = "terminal")]
            Self::TerminalSettingsGet => "lx.terminal.settings.get",
            #[cfg(feature = "terminal")]
            Self::TerminalSettingsUpdate => "lx.terminal.settings.update",
            #[cfg(feature = "terminal")]
            Self::TerminalSettingsReset => "lx.terminal.settings.reset",
            #[cfg(feature = "terminal")]
            Self::TerminalSettingsOnChange => "lx.terminal.settings.onChange",
            #[cfg(feature = "terminal")]
            Self::TerminalSchemesList => "lx.terminal.colorSchemes.list",
            #[cfg(feature = "terminal")]
            Self::TerminalSchemesImport => "lx.terminal.colorSchemes.import",
            #[cfg(feature = "terminal")]
            Self::TerminalPreviewCreate => "lx.terminal.colorSchemes.createPreview",
            #[cfg(feature = "terminal")]
            Self::TerminalPreviewShow => "lx.terminal.colorSchemes.preview.show",
            #[cfg(feature = "terminal")]
            Self::TerminalPreviewClear => "lx.terminal.colorSchemes.preview.clear",
            #[cfg(feature = "terminal")]
            Self::TerminalPreviewClose => "lx.terminal.colorSchemes.preview.close",
            #[cfg(feature = "terminal")]
            Self::TerminalFontsList => "lx.terminal.fonts.list",
            #[cfg(all(feature = "terminal", target_os = "windows"))]
            Self::TerminalWindowsStatus => "lx.terminal.windows.status",
            #[cfg(all(feature = "terminal", target_os = "windows"))]
            Self::TerminalWindowsInstall => "lx.terminal.windows.install",
            #[cfg(all(feature = "terminal", target_os = "windows"))]
            Self::TerminalWindowsSetEnabled => "lx.terminal.windows.setEnabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LogicRouteMetadata {
    policy: EffectiveRoutePolicy,
}

impl LogicRouteMetadata {
    pub(crate) const fn policy(self) -> EffectiveRoutePolicy {
        self.policy
    }
}

static LOGIC_ROUTE_INVENTORY: OnceLock<HashMap<&'static str, LogicRouteMetadata>> = OnceLock::new();

pub(crate) fn logic_route_inventory() -> &'static HashMap<&'static str, LogicRouteMetadata> {
    LOGIC_ROUTE_INVENTORY.get_or_init(|| {
        let mut inventory = HashMap::with_capacity(LogicRoute::ALL.len());
        for route in LogicRoute::ALL {
            let previous = inventory.insert(
                route.name(),
                LogicRouteMetadata {
                    policy: EffectiveRoutePolicy::new(route.audience()),
                },
            );
            assert!(
                previous.is_none(),
                "duplicate Logic route: {}",
                route.name()
            );
        }
        inventory
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LogicAuthorizationDenied {
    route: LogicRoute,
}

impl LogicAuthorizationDenied {
    pub(crate) const fn route(self) -> LogicRoute {
        self.route
    }
}

pub(crate) fn invocation_from_context(ctx: &JSContext) -> JSResult<HostInvocationContext> {
    HostInvocationContext::for_logic_context(ctx)
}

/// The sole authorization point for privileged direct Logic routes.
pub(crate) fn authorize(
    invocation: &HostInvocationContext,
    route: LogicRoute,
) -> Result<(), LogicAuthorizationDenied> {
    authorize_caller(invocation.caller(), route)
}

fn authorize_caller(
    caller: &host::AuthenticatedCaller,
    route: LogicRoute,
) -> Result<(), LogicAuthorizationDenied> {
    let metadata = logic_route_inventory()
        .get(route.name())
        .expect("every Logic route enum value has immutable policy metadata");
    host::authorize(caller, metadata.policy().audience())
        .then_some(())
        .ok_or(LogicAuthorizationDenied { route })
}

/// Name the class that would have been admitted. A ControlSurface author told
/// to become "the Control app" is being pointed at the wrong fix.
pub(crate) fn denial_error(denied: LogicAuthorizationDenied) -> rong::RongJSError {
    let route = denied.route();
    let audience = match route.audience() {
        RouteAudience::ControlSurfaceOnly => "the host-bundled control surface",
        _ => "the Control app",
    };
    js_error_from_business_code_with_detail(
        3000,
        format!("{} is only available in {audience}", route.name()),
    )
}

pub(crate) fn require(ctx: &JSContext, route: LogicRoute) -> JSResult<HostInvocationContext> {
    let invocation = invocation_from_context(ctx)?;
    authorize(&invocation, route).map_err(denial_error)?;
    Ok(invocation)
}

/// Authenticate and authorize a direct Logic call before its raw arguments
/// are converted into route-specific Rust values.
pub(crate) fn require_before_decode<T>(
    ctx: &JSContext,
    route: LogicRoute,
    decode: impl FnOnce() -> JSResult<T>,
) -> JSResult<(HostInvocationContext, T)> {
    let invocation = invocation_from_context(ctx)?;
    let decoded =
        authorize_caller_then(invocation.caller(), route, decode).map_err(denial_error)??;
    Ok((invocation, decoded))
}

fn authorize_caller_then<T>(
    caller: &host::AuthenticatedCaller,
    route: LogicRoute,
    action: impl FnOnce() -> T,
) -> Result<T, LogicAuthorizationDenied> {
    authorize_caller(caller, route)?;
    Ok(action())
}

/// Whose notifications, banners, and surface an attention route acts on.
///
/// The Control app speaks for the product: its ids are used as given and its
/// notification may go anywhere. Every other lxapp session speaks only for
/// itself: its ids live in a namespace derived from its app id, so it cannot
/// replace or dismiss another app's, and its notification can open only
/// itself. Derived from the native session class, never from a payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AttentionScope {
    Product,
    App { app_id: String },
}

/// Why a scoped request was refused before anything reached the platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScopeRefusal {
    /// The Control app could make this request; this caller cannot.
    Denied(String),
    /// No caller could: the request itself is malformed.
    Invalid(String),
}

impl ScopeRefusal {
    pub(crate) fn into_js(self) -> rong::RongJSError {
        match self {
            Self::Denied(message) => js_error_from_business_code_with_detail(3000, message),
            Self::Invalid(message) => js_invalid_parameter_error(message),
        }
    }
}

impl AttentionScope {
    pub(crate) fn of(invocation: &HostInvocationContext) -> Self {
        Self::for_caller(invocation.caller(), &invocation.lxapp().appid)
    }

    /// `app_id` is the invocation's own lxapp, which bridge dispatch has
    /// already proved the caller's scope belongs to.
    fn for_caller(caller: &host::AuthenticatedCaller, app_id: &str) -> Self {
        match caller.session_class() {
            Some(AppSessionClass::ControlApp) => Self::Product,
            _ => Self::App {
                app_id: caller
                    .app_scope()
                    .map(|scope| scope.identity().app_id().to_string())
                    .unwrap_or_else(|| app_id.to_string()),
            },
        }
    }

    /// The guest lxapp this acts for; `None` for the product.
    pub(crate) fn owner(&self) -> Option<&str> {
        match self {
            Self::Product => None,
            Self::App { app_id } => Some(app_id),
        }
    }

    /// The id the platform and the host stores hold a caller's `id` under.
    ///
    /// A guest's id is a digest of its app id and its own id: fixed length,
    /// so it fits every platform's tag limit (Windows toasts allow 64
    /// characters), and no guest can name another's. The product's ids are
    /// unchanged, so existing Control-app notifications keep replacing.
    pub(crate) fn platform_id(&self, id: &str) -> String {
        match self {
            Self::Product => id.to_string(),
            Self::App { app_id } => {
                let mut digest = Sha256::new();
                digest.update(b"lingxia.attention.v1\0");
                digest.update(app_id.as_bytes());
                digest.update(b"\0");
                digest.update(id.as_bytes());
                let hex: String = digest
                    .finalize()
                    .iter()
                    .take(20)
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                format!("lxapp-{hex}")
            }
        }
    }

    /// Resolve a `show()` target for this caller.
    ///
    /// A guest may only target itself: `{ kind: 'app' }` with its own
    /// `appId` (or none, which means itself). An omitted target opens the
    /// guest too, rather than only bringing the product forward.
    pub(crate) fn notification_target(
        &self,
        route: &str,
        target: Option<serde_json::Value>,
    ) -> Result<NavigationTarget, ScopeRefusal> {
        let invalid =
            |error: NavigationError| ScopeRefusal::Invalid(format!("{route} {}", error.message()));
        let target = target.filter(|value| !value.is_null());
        let Self::App { app_id } = self else {
            return match target {
                None => Ok(NavigationTarget::Activate),
                Some(value) => NavigationTarget::from_json(&value).map_err(invalid),
            };
        };
        let mut object = match target {
            None => serde_json::Map::from_iter([("kind".to_string(), "app".into())]),
            Some(serde_json::Value::Object(object)) => object,
            Some(_) => {
                return Err(ScopeRefusal::Invalid(format!(
                    "{route} target must be an object"
                )));
            }
        };
        match object.get("kind").and_then(serde_json::Value::as_str) {
            Some("app") => {}
            Some(kind @ ("activate" | "page" | "route" | "appLink")) => {
                return Err(ScopeRefusal::Denied(format!(
                    "{route} target {{ kind: '{kind}' }} is only available in the Control app; \
                     a guest lxapp's notification opens only itself: {{ kind: 'app' }}"
                )));
            }
            // Unknown or missing kinds are malformed for every caller.
            _ => return NavigationTarget::from_json(&object.into()).map_err(invalid),
        }
        match object.get("appId") {
            None | Some(serde_json::Value::Null) => {
                object.insert("appId".to_string(), app_id.clone().into());
            }
            Some(serde_json::Value::String(target)) if target == app_id => {}
            Some(serde_json::Value::String(target)) => {
                return Err(ScopeRefusal::Denied(format!(
                    "{route} target appId '{target}' is another lxapp; only the Control app \
                     may open other lxapps, so a guest omits appId or names itself"
                )));
            }
            Some(_) => {
                return Err(ScopeRefusal::Invalid(format!(
                    "{route} target.appId must be a string"
                )));
            }
        }
        NavigationTarget::from_json(&object.into()).map_err(invalid)
    }
}

#[cfg(test)]
mod audience_tests {
    use super::LogicRoute;
    use lxapp::host::RouteAudience;

    /// The status item belongs to the product, not to whichever lxapp is
    /// running. Adding a `lx.tray.*` method without a gate is the regression
    /// this catches — the audience arm is exhaustive, so a new route cannot
    /// compile without choosing one, but it can still choose wrong.
    #[test]
    fn every_tray_route_is_control_app_only() {
        let tray: Vec<LogicRoute> = LogicRoute::ALL
            .iter()
            .copied()
            .filter(|route| route.name().starts_with("lx.tray."))
            .collect();
        assert!(!tray.is_empty(), "lx.tray has routes to gate");
        for route in tray {
            assert_eq!(
                route.audience(),
                RouteAudience::ControlAppOnly,
                "{} must stay Control-app only",
                route.name()
            );
        }
    }

    /// The badge paints the same product chrome, so it answers to the same gate.
    #[test]
    fn the_badge_route_is_control_app_only() {
        assert_eq!(
            LogicRoute::AppSetBadge.audience(),
            RouteAudience::ControlAppOnly
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lxapp::AppSessionClass;
    use std::collections::HashSet;

    unsafe extern "Rust" {
        #[link_name = "lingxia_lxapp_test_authenticated_caller_v1"]
        fn test_authenticated_caller(
            app_id: &str,
            session_id: u64,
            class: AppSessionClass,
        ) -> host::AuthenticatedCaller;
        #[link_name = "lingxia_lxapp_test_browser_caller_v1"]
        fn test_browser_caller() -> host::AuthenticatedCaller;
    }

    fn app_caller(
        app_id: &str,
        session_id: u64,
        class: AppSessionClass,
    ) -> host::AuthenticatedCaller {
        // SAFETY: the symbol is a private workspace test harness supplied by
        // the lxapp dev-dependency; it is absent from the safe public API.
        unsafe { test_authenticated_caller(app_id, session_id, class) }
    }

    fn browser_caller() -> host::AuthenticatedCaller {
        // SAFETY: see `app_caller`.
        unsafe { test_browser_caller() }
    }

    #[test]
    fn production_inventory_is_complete_unique_and_control_scoped() {
        let inventory = logic_route_inventory();
        assert_eq!(inventory.len(), LogicRoute::ALL.len());
        let names: HashSet<_> = LogicRoute::ALL.iter().map(|route| route.name()).collect();
        assert_eq!(names.len(), LogicRoute::ALL.len());
        for route in LogicRoute::ALL {
            let audience = inventory[route.name()].policy().audience();
            assert_eq!(audience, route.audience(), "{}", route.name());
            let expected = if route.name().starts_with("lx.terminal.") {
                RouteAudience::ControlSurfaceOnly
            } else if is_attention_route(*route) {
                RouteAudience::AppSessionOnly
            } else {
                RouteAudience::ControlAppOnly
            };
            assert_eq!(audience, expected, "{}", route.name());
        }
        // The entry that opens the Settings surface stays with home.
        assert_eq!(
            LogicRoute::ShellOpenHostTerminalSettings.audience(),
            RouteAudience::ControlAppOnly
        );
    }

    /// The routes a guest may call for itself. Widening this list widens
    /// what every downloaded lxapp can do.
    fn is_attention_route(route: LogicRoute) -> bool {
        route.name().starts_with("lx.host.notification.")
            || route.name().starts_with("lx.host.banner.")
            || route == LogicRoute::SurfaceBringToFront
    }

    #[test]
    fn the_attention_routes_are_exactly_these() {
        let attention: Vec<&str> = LogicRoute::ALL
            .iter()
            .filter(|route| route.audience() == RouteAudience::AppSessionOnly)
            .map(|route| route.name())
            .collect();
        assert_eq!(
            attention,
            [
                "lx.host.notification.getPermission",
                "lx.host.notification.requestPermission",
                "lx.host.notification.show",
                "lx.host.notification.cancel",
                "lx.host.notification.cancelAll",
                "lx.host.banner.show",
                "lx.host.banner.dismiss",
                "lx.surface.bringToFront",
            ]
        );
        // Composing other lxapps into the shell stays with the Control app.
        assert_eq!(
            LogicRoute::ShellOpenApp.audience(),
            RouteAudience::ControlAppOnly
        );
    }

    #[test]
    fn same_app_id_standard_and_browser_are_denied_for_every_direct_family() {
        let standard = app_caller("same.app", 41, AppSessionClass::StandardApp);
        let control = app_caller("same.app", 42, AppSessionClass::ControlApp);
        let surface = app_caller("same.app", 43, AppSessionClass::ControlSurface);
        let browser = browser_caller();

        for route in LogicRoute::ALL {
            let surface_route = route.audience() == RouteAudience::ControlSurfaceOnly;
            let attention = is_attention_route(*route);
            assert_eq!(
                authorize_caller(&standard, *route).is_ok(),
                attention,
                "{}",
                route.name()
            );
            assert_eq!(
                authorize_caller(&control, *route).is_ok(),
                !surface_route,
                "{}",
                route.name()
            );
            assert_eq!(
                authorize_caller(&surface, *route).is_ok(),
                surface_route || attention,
                "{}",
                route.name()
            );
            assert!(
                authorize_caller(&browser, *route).is_err(),
                "{}",
                route.name()
            );
        }
    }

    #[test]
    fn unauthorized_raw_call_never_decodes_malformed_arguments() {
        let standard = app_caller("same.app", 51, AppSessionClass::StandardApp);
        let control = app_caller("same.app", 52, AppSessionClass::ControlApp);
        let mut decoded = false;
        let denied = authorize_caller_then(
            &standard,
            LogicRoute::AppSetDisplayLanguagePreference,
            || decoded = true,
        );
        assert!(denied.is_err());
        assert!(!decoded, "unauthorized arguments must remain raw");

        authorize_caller_then(
            &control,
            LogicRoute::AppSetDisplayLanguagePreference,
            || decoded = true,
        )
        .expect("ControlApp is authorized");
        assert!(decoded, "authorized arguments may be decoded");
    }
}

#[cfg(test)]
mod attention_scope_tests {
    use super::*;
    use serde_json::json;

    unsafe extern "Rust" {
        #[link_name = "lingxia_lxapp_test_authenticated_caller_v1"]
        fn test_authenticated_caller(
            app_id: &str,
            session_id: u64,
            class: AppSessionClass,
        ) -> host::AuthenticatedCaller;
    }

    fn scope_of(app_id: &str, class: AppSessionClass) -> AttentionScope {
        // SAFETY: the private workspace test harness from the lxapp dev-dependency.
        let caller = unsafe { test_authenticated_caller(app_id, 7, class) };
        AttentionScope::for_caller(&caller, app_id)
    }

    fn guest() -> AttentionScope {
        scope_of("com.example.guest", AppSessionClass::StandardApp)
    }

    const ROUTE: &str = "lx.host.notification.show";

    #[test]
    fn the_scope_follows_the_native_class() {
        assert_eq!(
            scope_of("com.example.home", AppSessionClass::ControlApp),
            AttentionScope::Product
        );
        assert_eq!(guest().owner(), Some("com.example.guest"));
        // A ControlSurface is not the product either.
        assert_eq!(
            scope_of("com.example.settings", AppSessionClass::ControlSurface).owner(),
            Some("com.example.settings")
        );
    }

    #[test]
    fn product_ids_are_unchanged_and_guest_ids_are_namespaced() {
        assert_eq!(AttentionScope::Product.platform_id("order:42"), "order:42");
        let mine = guest().platform_id("run:1");
        assert_eq!(mine, guest().platform_id("run:1"), "stable across calls");
        assert!(mine.starts_with("lxapp-"));
        assert!(mine.chars().count() <= 64, "fits a Windows toast tag");
        assert_ne!(mine, guest().platform_id("run:2"));
        assert_ne!(mine, "run:1", "a guest cannot reach the product's id");
        let other = scope_of("com.example.other", AppSessionClass::StandardApp);
        assert_ne!(
            mine,
            other.platform_id("run:1"),
            "the same id in another app is another notification"
        );
        // The longest id a caller may pass still fits.
        assert!(guest().platform_id(&"x".repeat(64)).chars().count() <= 64);
    }

    #[test]
    fn a_guest_targets_only_itself() {
        let own = NavigationTarget::App {
            appid: "com.example.guest".into(),
            page: None,
            query: Default::default(),
        };
        assert_eq!(guest().notification_target(ROUTE, None), Ok(own.clone()));
        assert_eq!(
            guest().notification_target(ROUTE, Some(serde_json::Value::Null)),
            Ok(own.clone())
        );
        assert_eq!(
            guest().notification_target(ROUTE, Some(json!({ "kind": "app" }))),
            Ok(own.clone())
        );
        assert_eq!(
            guest().notification_target(
                ROUTE,
                Some(json!({ "kind": "app", "appId": "com.example.guest" }))
            ),
            Ok(own)
        );
        let with_query = guest()
            .notification_target(
                ROUTE,
                Some(json!({ "kind": "app", "query": { "session": "s1" } })),
            )
            .unwrap();
        assert!(matches!(
            with_query,
            NavigationTarget::App { ref appid, ref query, .. }
                if appid == "com.example.guest" && query["session"] == "s1"
        ));
    }

    #[test]
    fn a_guest_is_refused_other_apps_and_product_targets() {
        let refused = |target: serde_json::Value| guest().notification_target(ROUTE, Some(target));
        assert!(matches!(
            refused(json!({ "kind": "app", "appId": "com.example.home" })),
            Err(ScopeRefusal::Denied(message)) if message.contains("com.example.home")
        ));
        for kind in [
            json!({ "kind": "activate" }),
            json!({ "kind": "page", "page": "home" }),
            json!({ "kind": "route", "name": "downloads.detail" }),
            json!({ "kind": "appLink", "url": "https://example.com/x" }),
        ] {
            assert!(
                matches!(refused(kind.clone()), Err(ScopeRefusal::Denied(ref message))
                    if message.contains("only available in the Control app")),
                "{kind}"
            );
        }
        // Malformed for every caller stays a parameter error, not a denial.
        assert!(matches!(
            refused(json!({ "kind": "nope" })),
            Err(ScopeRefusal::Invalid(_))
        ));
        assert!(matches!(
            refused(json!({ "kind": "app", "appId": 7 })),
            Err(ScopeRefusal::Invalid(_))
        ));
        assert!(matches!(
            refused(json!({ "kind": "app", "page": "/pages/x/index" })),
            Err(ScopeRefusal::Invalid(_))
        ));
        assert!(matches!(
            refused(json!("app")),
            Err(ScopeRefusal::Invalid(_))
        ));
    }

    #[test]
    fn the_product_keeps_every_target_kind() {
        let product = AttentionScope::Product;
        assert_eq!(
            product.notification_target(ROUTE, None),
            Ok(NavigationTarget::Activate)
        );
        assert!(
            product
                .notification_target(ROUTE, Some(json!({ "kind": "route", "name": "a" })))
                .is_ok()
        );
        assert!(
            product
                .notification_target(
                    ROUTE,
                    Some(json!({ "kind": "app", "appId": "com.example.guest" }))
                )
                .is_ok()
        );
    }
}
