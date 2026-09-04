use crate::i18n::js_error_from_business_code_with_detail;
use lxapp::host::{self, EffectiveRoutePolicy, HostInvocationContext, RouteAudience};
use rong::{JSContext, JSResult};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LogicRoute {
    AppExit,
    AppSetBadge,
    AppGetDisplayLanguageState,
    AppSetDisplayLanguagePreference,
    AppWatchDisplayLanguageState,
    AppCheckUpdate,
    AppApplyUpdate,
    AppScreenshot,
    AppAutostartIsEnabled,
    AppAutostartSetEnabled,
    ShellSidebarReplace,
    ShellSidebarUpdate,
    ShellSidebarRemove,
    ShellSidebarClear,
    ShellOpenApp,
    ShellOpenBuiltin,
    ShellOpenDeclared,
    ShellReconfigure,
    ShellOpenHostTerminalSettings,
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
        Self::AppSetBadge,
        Self::AppGetDisplayLanguageState,
        Self::AppSetDisplayLanguagePreference,
        Self::AppWatchDisplayLanguageState,
        Self::AppCheckUpdate,
        Self::AppApplyUpdate,
        Self::AppScreenshot,
        Self::AppAutostartIsEnabled,
        Self::AppAutostartSetEnabled,
        Self::ShellSidebarReplace,
        Self::ShellSidebarUpdate,
        Self::ShellSidebarRemove,
        Self::ShellSidebarClear,
        Self::ShellOpenApp,
        Self::ShellOpenBuiltin,
        Self::ShellOpenDeclared,
        Self::ShellReconfigure,
        Self::ShellOpenHostTerminalSettings,
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

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::AppExit => "lx.app.exit",
            Self::AppSetBadge => "lx.app.setBadge",
            Self::AppGetDisplayLanguageState => "lx.app.getDisplayLanguageState",
            Self::AppSetDisplayLanguagePreference => "lx.app.setDisplayLanguagePreference",
            Self::AppWatchDisplayLanguageState => "lx.app.onDisplayLanguageStateChange",
            Self::AppCheckUpdate => "lx.app.checkUpdate",
            Self::AppApplyUpdate => "lx.app.checkUpdate.apply",
            Self::AppScreenshot => "lx.app.screenshot",
            Self::AppAutostartIsEnabled => "lx.app.autostart.isEnabled",
            Self::AppAutostartSetEnabled => "lx.app.autostart.setEnabled",
            Self::ShellSidebarReplace => "lx.shell.sidebarActions.replace",
            Self::ShellSidebarUpdate => "lx.shell.sidebarActions.update",
            Self::ShellSidebarRemove => "lx.shell.sidebarActions.remove",
            Self::ShellSidebarClear => "lx.shell.sidebarActions.clear",
            Self::ShellOpenApp => "lx.shell.openApp",
            Self::ShellOpenBuiltin => "lx.shell.openBuiltin",
            Self::ShellOpenDeclared => "lx.shell.openDeclared",
            Self::ShellReconfigure => "lx.shell.reconfigure",
            Self::ShellOpenHostTerminalSettings => "lx.shell.openApp.hostTerminalSettings",
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
                    policy: EffectiveRoutePolicy::new(RouteAudience::ControlAppOnly),
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

pub(crate) fn require(ctx: &JSContext, route: LogicRoute) -> JSResult<HostInvocationContext> {
    let invocation = invocation_from_context(ctx)?;
    authorize(&invocation, route).map_err(|denied| {
        js_error_from_business_code_with_detail(
            3000,
            format!(
                "{} is only available in the Control app",
                denied.route().name()
            ),
        )
    })?;
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
        authorize_caller_then(invocation.caller(), route, decode).map_err(|denied| {
            js_error_from_business_code_with_detail(
                3000,
                format!(
                    "{} is only available in the Control app",
                    denied.route().name()
                ),
            )
        })??;
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
    fn production_inventory_is_complete_unique_and_control_only() {
        let inventory = logic_route_inventory();
        assert_eq!(inventory.len(), LogicRoute::ALL.len());
        let names: HashSet<_> = LogicRoute::ALL.iter().map(|route| route.name()).collect();
        assert_eq!(names.len(), LogicRoute::ALL.len());
        assert!(
            inventory
                .values()
                .all(|metadata| { metadata.policy().audience() == RouteAudience::ControlAppOnly })
        );
    }

    #[test]
    fn same_app_id_standard_and_browser_are_denied_for_every_direct_family() {
        let standard = app_caller("same.app", 41, AppSessionClass::StandardApp);
        let control = app_caller("same.app", 42, AppSessionClass::ControlApp);
        let browser = browser_caller();

        for route in LogicRoute::ALL {
            assert!(
                authorize_caller(&standard, *route).is_err(),
                "{}",
                route.name()
            );
            assert!(
                authorize_caller(&control, *route).is_ok(),
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
