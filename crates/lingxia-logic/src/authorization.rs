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
    SurfaceDeclaredOverride,
    SurfaceOpenBuiltin,
    NavigatorHostTerminalSettings,
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
        Self::SurfaceDeclaredOverride,
        Self::SurfaceOpenBuiltin,
        Self::NavigatorHostTerminalSettings,
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
            Self::SurfaceDeclaredOverride => "lx.surface.openDeclared.override",
            Self::SurfaceOpenBuiltin => "lx.surface.openUrl.builtin",
            Self::NavigatorHostTerminalSettings => "lx.navigateToApp.hostTerminalSettings",
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

#[cfg(test)]
mod tests {
    use super::*;
    use lxapp::AppSessionClass;
    use std::collections::HashSet;

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
        let standard = host::AuthenticatedCaller::lxapp_session_for_test(
            "same.app",
            41,
            AppSessionClass::StandardApp,
        );
        let control = host::AuthenticatedCaller::lxapp_session_for_test(
            "same.app",
            42,
            AppSessionClass::ControlApp,
        );
        let browser = host::AuthenticatedCaller::browser_document_for_test();

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
}
