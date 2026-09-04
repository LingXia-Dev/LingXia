#[cfg(feature = "devtool")]
use crate::host::NativeDevtoolsAuthority;
use crate::host::{AppResourceGrant, NativeHostRuntimeAuthority};
use crate::{AppSessionClass, LxApp};
use std::sync::{Arc, Mutex, OnceLock};

/// Host lifecycle extension points that can register additional runtime behavior.
pub trait HostAddon: Send + Sync {
    /// Registers product CLI commands before command-line arguments are parsed.
    ///
    /// This runs before runtime initialization, so it must only publish command
    /// definitions. Register the matching in-app control handlers from
    /// [`Self::install_host_apis`].
    #[cfg(feature = "product-cli")]
    fn install_product_cli(&self, _cli: &mut crate::product_cli::ProductCli) {}
    /// Runs before LingXia initialization begins.
    fn before_init(&self) {}
    /// Contributes immutable Settings target metadata before any lxapp,
    /// AppService, WebView, or document session is created.
    fn configure_static_settings_targets(&self, _catalog: &mut crate::StaticSettingsTargetCatalog) {
    }
    /// Installs callbacks only for NativeAction ids declared in the static
    /// Settings target catalog. The registrar is sealed before runtime starts.
    fn install_native_settings_actions(
        &self,
        _registrar: &mut crate::NativeSettingsActionRegistrar,
    ) -> Result<(), String> {
        Ok(())
    }
    /// Registers JS logic extensions when the `standard` feature is enabled.
    #[cfg(feature = "standard")]
    fn install_logic_extensions(&self) {}
    /// Registers native host APIs before the runtime starts serving requests.
    fn install_host_apis(&self) {}
    /// Assign the manifest-requested privileged resources this native product
    /// approves for a newly created session. The authority is sealed after
    /// this callback returns and is never reachable from bridge payloads.
    fn issue_app_resource_grants(&self, _authority: &mut NativeHostRuntimeAuthority<'_>) {}
    /// Assign devtools-only automation resources to one newly created session.
    #[cfg(feature = "devtool")]
    fn issue_devtools_app_resource_grants(&self, _authority: &mut NativeDevtoolsAuthority<'_>) {}
    /// Picks the campaign screen shown after this cold start's launch face,
    /// with a countdown the user can skip.
    ///
    /// Runs once the runtime is up, not on the cold-start path, so reading a
    /// file or checking a clock here costs the launch nothing — but an answer
    /// that arrives after the launch face lifts is dropped, so this is not
    /// the place to wait on a network. Choose only among assets already on
    /// disk; hand any downloading to [`crate::spawn`], which lands it in time
    /// for a later launch.
    fn select_campaign(&self, _launch: &crate::splash::Launch) -> crate::splash::CampaignChoice {
        crate::splash::CampaignChoice::default()
    }

    /// Runs after LingXia initialization succeeds.
    fn after_init(&self) {}
    /// Starts long-lived services after the host runtime is warmed up.
    fn start_services(&self) {}
}

static HOST_ADDONS: OnceLock<Mutex<Vec<Arc<dyn HostAddon>>>> = OnceLock::new();

fn host_addons() -> &'static Mutex<Vec<Arc<dyn HostAddon>>> {
    HOST_ADDONS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Registers a host addon for future LingXia initialization cycles.
pub fn register_host_addon(addon: Box<dyn HostAddon>) {
    let mut installed = host_addons()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    installed.push(Arc::from(addon));
}

fn snapshot_host_addons() -> Vec<Arc<dyn HostAddon>> {
    host_addons()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

#[cfg(feature = "product-cli")]
pub(crate) fn run_install_product_cli(cli: &mut crate::product_cli::ProductCli) {
    let installed = snapshot_host_addons();
    for addon in installed.iter() {
        addon.install_product_cli(cli);
    }
}

pub(crate) fn run_before_init() {
    let installed = snapshot_host_addons();
    for addon in installed.iter() {
        addon.before_init();
    }
}

pub(crate) fn run_configure_static_settings_targets(
    catalog: &mut crate::StaticSettingsTargetCatalog,
) {
    let installed = snapshot_host_addons();
    for addon in installed.iter() {
        addon.configure_static_settings_targets(catalog);
    }
}

pub(crate) fn run_install_native_settings_actions(
    registrar: &mut crate::NativeSettingsActionRegistrar,
) -> Result<(), String> {
    let installed = snapshot_host_addons();
    for addon in installed.iter() {
        addon.install_native_settings_actions(registrar)?;
    }
    Ok(())
}

pub(crate) fn run_install_logic_extensions() {
    #[cfg(feature = "standard")]
    {
        let installed = snapshot_host_addons();
        for addon in installed.iter() {
            addon.install_logic_extensions();
        }
    }
}

pub(crate) fn run_install_host_apis() {
    let installed = snapshot_host_addons();
    for addon in installed.iter() {
        addon.install_host_apis();
    }
}

pub(crate) fn resolve_app_resource_grants(
    _app: &Arc<LxApp>,
    authority: &mut NativeHostRuntimeAuthority<'_>,
) {
    // `capabilities.process` is native-authored bootstrap policy, but it
    // applies only to the native-assigned ControlApp and still intersects the
    // lxapp's manifest request.
    issue_builtin_host_grants(authority, lingxia_app_context::process_enabled());

    for addon in snapshot_host_addons() {
        addon.issue_app_resource_grants(authority);
    }
}

#[cfg(feature = "devtool")]
pub(crate) fn resolve_devtools_resource_grants(
    _app: &Arc<LxApp>,
    authority: &mut NativeDevtoolsAuthority<'_>,
) {
    // Only a linked native devtools addon receives this authority. A dev
    // websocket/env signal alone never grants host automation.
    for addon in snapshot_host_addons() {
        addon.issue_devtools_app_resource_grants(authority);
    }
}

fn issue_builtin_host_grants(
    authority: &mut NativeHostRuntimeAuthority<'_>,
    process_enabled: bool,
) {
    if authority.session_class() == AppSessionClass::ControlApp && process_enabled {
        authority.grant(AppResourceGrant::Process);
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod resource_grant_tests {
    use super::*;
    use std::collections::HashSet;

    fn authority<'a>(
        session_id: u64,
        session_class: AppSessionClass,
        requested: impl IntoIterator<Item = AppResourceGrant>,
        grants: &'a mut HashSet<AppResourceGrant>,
    ) -> NativeHostRuntimeAuthority<'a> {
        NativeHostRuntimeAuthority::for_test(
            "same.app",
            session_id,
            session_class,
            requested,
            grants,
        )
    }

    #[test]
    fn manifest_request_is_not_a_grant_and_native_issuer_cannot_expand_it() {
        let mut grants = HashSet::new();
        let mut authority = authority(
            7,
            AppSessionClass::ControlApp,
            [AppResourceGrant::Downloads],
            &mut grants,
        );
        assert!(authority.requested(AppResourceGrant::Downloads));
        assert!(!authority.grant(AppResourceGrant::Process));
        assert!(authority.grant(AppResourceGrant::Downloads));
        assert_eq!(grants, HashSet::from([AppResourceGrant::Downloads]));
    }

    #[test]
    fn process_bootstrap_grant_requires_control_class_host_policy_and_manifest_request() {
        for (class, enabled, requested, expected) in [
            (AppSessionClass::StandardApp, true, true, false),
            (AppSessionClass::ControlApp, false, true, false),
            (AppSessionClass::ControlApp, true, false, false),
            (AppSessionClass::ControlApp, true, true, true),
        ] {
            let mut grants = HashSet::new();
            let requests = requested.then_some(AppResourceGrant::Process).into_iter();
            let mut authority = authority(9, class, requests, &mut grants);
            issue_builtin_host_grants(&mut authority, enabled);
            assert_eq!(grants.contains(&AppResourceGrant::Process), expected);
        }
    }

    #[test]
    fn same_app_id_authorities_do_not_share_session_grants() {
        let mut first = HashSet::new();
        let mut second = HashSet::new();
        authority(
            10,
            AppSessionClass::ControlApp,
            [AppResourceGrant::AutomationHost],
            &mut first,
        )
        .grant(AppResourceGrant::AutomationHost);
        let second_authority = authority(
            11,
            AppSessionClass::ControlApp,
            [AppResourceGrant::AutomationHost],
            &mut second,
        );
        assert_eq!(second_authority.app_id(), "same.app");
        assert_ne!(second_authority.session_id(), 10);
        assert!(first.contains(&AppResourceGrant::AutomationHost));
        assert!(second.is_empty());
    }

    #[cfg(feature = "devtool")]
    #[test]
    fn devtools_authority_can_issue_only_session_bound_automation() {
        let mut grants = HashSet::new();
        let mut authority = NativeDevtoolsAuthority::for_test(
            "dev.app",
            13,
            AppSessionClass::StandardApp,
            [
                AppResourceGrant::Automation,
                AppResourceGrant::AutomationHost,
            ],
            &mut grants,
        );
        assert!(authority.grant_automation());
        assert_eq!(authority.app_id(), "dev.app");
        assert_eq!(authority.session_id(), 13);
        assert_eq!(authority.session_class(), AppSessionClass::StandardApp);
        assert_eq!(
            grants,
            HashSet::from([
                AppResourceGrant::Automation,
                AppResourceGrant::AutomationHost
            ])
        );
    }

    #[cfg(feature = "devtool")]
    #[test]
    fn devtools_native_policy_cannot_expand_manifest_requests() {
        for requested in [
            Vec::new(),
            vec![AppResourceGrant::Automation],
            vec![AppResourceGrant::AutomationHost],
        ] {
            let mut grants = HashSet::new();
            let mut authority = NativeDevtoolsAuthority::for_test(
                "same.app",
                21,
                AppSessionClass::StandardApp,
                requested.clone(),
                &mut grants,
            );
            authority.grant_automation();
            assert_eq!(grants, requested.into_iter().collect());
        }
    }
}

/// Whether any addon is installed — lets startup-path work skip entirely.
pub(crate) fn any_registered() -> bool {
    !snapshot_host_addons().is_empty()
}

/// First addon that names a campaign wins. The screen has one writer by
/// construction — a second opinion would just be a race for the same pixels.
pub(crate) fn run_select_campaign(launch: &crate::splash::Launch) -> crate::splash::CampaignChoice {
    let installed = snapshot_host_addons();
    for addon in installed.iter() {
        let choice = addon.select_campaign(launch);
        if choice != crate::splash::CampaignChoice::default() {
            return choice;
        }
    }
    crate::splash::CampaignChoice::default()
}

pub(crate) fn run_after_init() {
    let installed = snapshot_host_addons();
    for addon in installed.iter() {
        addon.after_init();
    }
}

pub(crate) fn run_start_services() {
    let installed = snapshot_host_addons();
    for addon in installed.iter() {
        addon.start_services();
    }
}

#[cfg(all(test, feature = "product-cli"))]
mod tests {
    use super::{HostAddon, register_host_addon, run_install_product_cli};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static PRODUCT_CLI_INSTALLS: AtomicUsize = AtomicUsize::new(0);

    struct ProductCliAddon;

    impl HostAddon for ProductCliAddon {
        fn install_product_cli(&self, _cli: &mut crate::product_cli::ProductCli) {
            PRODUCT_CLI_INSTALLS.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn registered_addons_install_product_cli_before_runtime_startup() {
        let before = PRODUCT_CLI_INSTALLS.load(Ordering::SeqCst);
        register_host_addon(Box::new(ProductCliAddon));
        run_install_product_cli(&mut crate::product_cli::ProductCli::new());
        assert_eq!(PRODUCT_CLI_INSTALLS.load(Ordering::SeqCst), before + 1);
    }
}
