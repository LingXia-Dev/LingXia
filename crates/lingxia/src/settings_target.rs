use lingxia_app_context::{AppConfig, SettingsDestination};
use lingxia_platform::traits::app_runtime::AppRuntime;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::sync::OnceLock;
use thiserror::Error;

const BROWSER_APP_ID: &str = "app.lingxia.browser";
const BROWSER_MANIFEST_ASSET: &str = "app.lingxia.browser/lxapp.json";

/// Startup-only catalog populated by generated config and host addons.
///
/// It contains names and immutable policy requirements only. Runtime objects,
/// handlers, sessions, and callbacks cannot be registered here.
#[derive(Debug, Default)]
pub struct StaticSettingsTargetCatalog {
    destination_writers: Vec<(String, SettingsDestination)>,
    control_page_routes: BTreeMap<(String, String), BTreeSet<String>>,
    browser_page_routes: BTreeMap<String, BTreeSet<String>>,
    native_actions: BTreeMap<String, Vec<String>>,
}

impl StaticSettingsTargetCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare one browser control page and its complete bridge-route inventory.
    pub fn require_browser_page_routes<I, S>(&mut self, route: impl Into<String>, routes: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.browser_page_routes
            .entry(route.into())
            .or_default()
            .extend(routes.into_iter().map(Into::into));
    }

    /// Contribute a destination from one static configuration source.
    pub fn set_destination(&mut self, source: impl Into<String>, destination: SettingsDestination) {
        self.destination_writers.push((source.into(), destination));
    }

    /// Declare the bridge routes a control page requires at startup.
    pub fn require_control_page_routes<I, S>(
        &mut self,
        app_id: impl Into<String>,
        page: impl Into<String>,
        routes: I,
    ) where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.control_page_routes
            .entry((app_id.into(), page.into()))
            .or_default()
            .extend(routes.into_iter().map(Into::into));
    }

    /// Predeclare a native Settings action name. A duplicate is an error even
    /// when both declarations use the same spelling.
    pub fn register_native_action(
        &mut self,
        source: impl Into<String>,
        action_id: impl Into<String>,
    ) {
        self.native_actions
            .entry(action_id.into())
            .or_default()
            .push(source.into());
    }
}

/// Immutable action-name inventory retained after startup validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedNativeActionRegistry {
    action_ids: BTreeSet<String>,
}

impl SealedNativeActionRegistry {
    pub fn contains(&self, action_id: &str) -> bool {
        self.action_ids.contains(action_id)
    }
}

/// Validated startup snapshot. It still contains no live runtime state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedStaticSettingsTargets {
    destination: Option<SettingsDestination>,
    control_app_id: Option<String>,
    native_actions: SealedNativeActionRegistry,
}

impl ValidatedStaticSettingsTargets {
    pub fn destination(&self) -> Option<&SettingsDestination> {
        self.destination.as_ref()
    }

    pub fn control_app_id(&self) -> Option<&str> {
        self.control_app_id.as_deref()
    }

    pub fn native_actions(&self) -> &SealedNativeActionRegistry {
        &self.native_actions
    }

    #[cfg(test)]
    pub(crate) fn for_runtime_test(
        destination: Option<SettingsDestination>,
        control_app_id: Option<&str>,
        native_action_ids: &[&str],
    ) -> Self {
        Self {
            destination,
            control_app_id: control_app_id.map(str::to_string),
            native_actions: SealedNativeActionRegistry {
                action_ids: native_action_ids
                    .iter()
                    .map(|action_id| (*action_id).to_string())
                    .collect(),
            },
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StaticSettingsTargetError {
    #[error("conflicting Settings destinations from '{first}' and '{second}'")]
    ConflictingDestinations { first: String, second: String },
    #[error("native Settings action '{action_id}' was declared more than once by {sources}")]
    DuplicateNativeAction { action_id: String, sources: String },
    #[error("native Settings action id must not be empty")]
    EmptyNativeAction,
    #[error(
        "Settings target requires control app '{actual}', but the configured control app is '{expected}'"
    )]
    WrongControlApp { expected: String, actual: String },
    #[error("Settings target requires a control app, but this host has none")]
    MissingControlApp,
    #[error("failed to read static Settings asset '{path}': {message}")]
    MissingAsset { path: String, message: String },
    #[error("failed to parse static Settings manifest '{path}': {message}")]
    InvalidManifest { path: String, message: String },
    #[error("static Settings manifest '{path}' declares appId '{actual}', expected '{expected}'")]
    ManifestAppIdMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("control app '{app_id}' has no manifest page named '{page}'")]
    UnknownControlPage { app_id: String, page: String },
    #[error("browser Settings target requires capabilities.browser: true")]
    BrowserCapabilityDisabled,
    #[error("browser Settings target requires the browser-shell build feature")]
    BrowserShellUnavailable,
    #[error("browser route '{route}' is not a predeclared static Settings target")]
    UnknownBrowserRoute { route: String },
    #[error("native Settings action '{action_id}' is not predeclared")]
    UnknownNativeAction { action_id: String },
    #[error("required bridge route policy is ambiguous: {0}")]
    RoutePolicyConflict(#[from] lxapp::host::RoutePolicyConflict),
    #[error("required bridge route '{route}' for {target} is not registered")]
    UnknownBridgeRoute { target: String, route: String },
    #[error("required bridge route '{route}' for {target} has incompatible audience {actual:?}")]
    IncompatibleBridgeRoute {
        target: String,
        route: String,
        actual: lxapp::host::RouteAudience,
    },
}

trait StaticAssetReader {
    fn read_text(&self, path: &str) -> Result<String, String>;
    fn asset_exists(&self, path: &str) -> Result<(), String>;
}

struct PlatformAssetReader<'a> {
    runtime: &'a lingxia_platform::Platform,
}

impl StaticAssetReader for PlatformAssetReader<'_> {
    fn read_text(&self, path: &str) -> Result<String, String> {
        let mut reader = self
            .runtime
            .read_asset(path)
            .map_err(|error| error.to_string())?;
        let mut content = String::new();
        reader
            .read_to_string(&mut content)
            .map_err(|error| error.to_string())?;
        Ok(content)
    }

    fn asset_exists(&self, path: &str) -> Result<(), String> {
        self.runtime
            .read_asset(path)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct StaticLxAppManifest {
    #[serde(rename = "appId")]
    app_id: String,
    #[serde(default)]
    pages: Vec<StaticLxAppPage>,
}

#[derive(Debug, Deserialize)]
struct StaticLxAppPage {
    name: String,
    path: String,
}

pub(crate) fn validate_for_startup(
    app_config: &AppConfig,
    catalog: StaticSettingsTargetCatalog,
    runtime: &lingxia_platform::Platform,
) -> Result<ValidatedStaticSettingsTargets, StaticSettingsTargetError> {
    validate_with_inventory(
        app_config,
        catalog,
        &PlatformAssetReader { runtime },
        lxapp::host::route_policy,
        cfg!(feature = "browser-shell"),
    )
}

fn validate_with_inventory(
    app_config: &AppConfig,
    mut catalog: StaticSettingsTargetCatalog,
    assets: &dyn StaticAssetReader,
    route_policy: impl Fn(
        &str,
    ) -> Result<
        Option<lxapp::host::EffectiveRoutePolicy>,
        lxapp::host::RoutePolicyConflict,
    >,
    browser_shell_available: bool,
) -> Result<ValidatedStaticSettingsTargets, StaticSettingsTargetError> {
    catalog
        .destination_writers
        .sort_by(|left, right| left.0.cmp(&right.0));
    let destination = resolve_destination(&catalog.destination_writers)?;
    let native_actions = seal_native_actions(catalog.native_actions)?;

    if let Some(destination) = destination.as_ref() {
        match destination {
            SettingsDestination::ControlAppPage {
                app_id,
                page,
                query: _,
            } => {
                validate_control_page(app_config, assets, app_id, page)?;
                let routes = catalog
                    .control_page_routes
                    .get(&(app_id.clone(), page.clone()))
                    .cloned()
                    .unwrap_or_default();
                validate_route_policies(
                    format!("control page '{app_id}/{page}'"),
                    &routes,
                    &route_policy,
                    |audience| {
                        matches!(
                            audience,
                            lxapp::host::RouteAudience::ControlAppOnly
                                | lxapp::host::RouteAudience::ControlOnly
                        )
                    },
                )?;
            }
            SettingsDestination::BrowserControlPage { route, query: _ } => {
                let normalized_route = normalize_browser_route(route);
                let routes = catalog
                    .browser_page_routes
                    .get(normalized_route.as_str())
                    .ok_or_else(|| StaticSettingsTargetError::UnknownBrowserRoute {
                        route: route.clone(),
                    })?;
                validate_browser_page(
                    app_config,
                    assets,
                    normalized_route.as_str(),
                    browser_shell_available,
                )?;
                validate_route_policies(
                    format!("browser page '{route}'"),
                    routes,
                    &route_policy,
                    |audience| {
                        matches!(
                            audience,
                            lxapp::host::RouteAudience::BrowserControlOnly
                                | lxapp::host::RouteAudience::ControlOnly
                        )
                    },
                )?;
            }
            SettingsDestination::NativeAction { action_id } => {
                if !native_actions.contains(action_id) {
                    return Err(StaticSettingsTargetError::UnknownNativeAction {
                        action_id: action_id.clone(),
                    });
                }
            }
        }
    }

    Ok(ValidatedStaticSettingsTargets {
        destination,
        control_app_id: (!app_config.home_app_id.is_empty())
            .then(|| app_config.home_app_id.clone()),
        native_actions,
    })
}

fn resolve_destination(
    writers: &[(String, SettingsDestination)],
) -> Result<Option<SettingsDestination>, StaticSettingsTargetError> {
    let Some((first_source, first)) = writers.first() else {
        return Ok(None);
    };
    for (source, candidate) in &writers[1..] {
        if candidate != first {
            return Err(StaticSettingsTargetError::ConflictingDestinations {
                first: first_source.clone(),
                second: source.clone(),
            });
        }
    }
    Ok(Some(first.clone()))
}

fn seal_native_actions(
    actions: BTreeMap<String, Vec<String>>,
) -> Result<SealedNativeActionRegistry, StaticSettingsTargetError> {
    let mut action_ids = BTreeSet::new();
    for (action_id, mut sources) in actions {
        if action_id.trim().is_empty() {
            return Err(StaticSettingsTargetError::EmptyNativeAction);
        }
        if sources.len() > 1 {
            sources.sort();
            return Err(StaticSettingsTargetError::DuplicateNativeAction {
                action_id,
                sources: sources.join(", "),
            });
        }
        action_ids.insert(action_id);
    }
    Ok(SealedNativeActionRegistry { action_ids })
}

fn parse_manifest(
    assets: &dyn StaticAssetReader,
    path: &str,
) -> Result<StaticLxAppManifest, StaticSettingsTargetError> {
    let content =
        assets
            .read_text(path)
            .map_err(|message| StaticSettingsTargetError::MissingAsset {
                path: path.to_string(),
                message,
            })?;
    serde_json::from_str(&content).map_err(|error| StaticSettingsTargetError::InvalidManifest {
        path: path.to_string(),
        message: error.to_string(),
    })
}

fn validate_control_page(
    app_config: &AppConfig,
    assets: &dyn StaticAssetReader,
    app_id: &str,
    page: &str,
) -> Result<(), StaticSettingsTargetError> {
    if app_config.home_app_id.is_empty() {
        return Err(StaticSettingsTargetError::MissingControlApp);
    }
    if app_config.home_app_id != app_id {
        return Err(StaticSettingsTargetError::WrongControlApp {
            expected: app_config.home_app_id.clone(),
            actual: app_id.to_string(),
        });
    }
    let manifest_path = format!("{app_id}/lxapp.json");
    let manifest = parse_manifest(assets, &manifest_path)?;
    if manifest.app_id != app_id {
        return Err(StaticSettingsTargetError::ManifestAppIdMismatch {
            path: manifest_path,
            expected: app_id.to_string(),
            actual: manifest.app_id,
        });
    }
    let entry = manifest
        .pages
        .iter()
        .find(|entry| entry.name == page)
        .ok_or_else(|| StaticSettingsTargetError::UnknownControlPage {
            app_id: app_id.to_string(),
            page: page.to_string(),
        })?;
    validate_asset_exists(assets, &format!("{app_id}/{}", entry.path))
}

fn validate_browser_page(
    app_config: &AppConfig,
    assets: &dyn StaticAssetReader,
    route: &str,
    browser_shell_available: bool,
) -> Result<(), StaticSettingsTargetError> {
    if !app_config
        .capabilities
        .as_ref()
        .is_some_and(|capabilities| capabilities.browser)
    {
        return Err(StaticSettingsTargetError::BrowserCapabilityDisabled);
    }
    if !browser_shell_available {
        return Err(StaticSettingsTargetError::BrowserShellUnavailable);
    }
    let manifest = parse_manifest(assets, BROWSER_MANIFEST_ASSET)?;
    if manifest.app_id != BROWSER_APP_ID {
        return Err(StaticSettingsTargetError::ManifestAppIdMismatch {
            path: BROWSER_MANIFEST_ASSET.to_string(),
            expected: BROWSER_APP_ID.to_string(),
            actual: manifest.app_id,
        });
    }
    let entry = manifest
        .pages
        .iter()
        .find(|entry| entry.name == route.trim_start_matches('/'))
        .ok_or_else(|| StaticSettingsTargetError::UnknownBrowserRoute {
            route: route.to_string(),
        })?;
    validate_asset_exists(assets, &format!("{BROWSER_APP_ID}/{}", entry.path))
}

fn normalize_browser_route(route: &str) -> String {
    format!("/{}", route.trim().trim_start_matches('/'))
}

fn validate_asset_exists(
    assets: &dyn StaticAssetReader,
    path: &str,
) -> Result<(), StaticSettingsTargetError> {
    assets
        .asset_exists(path)
        .map_err(|message| StaticSettingsTargetError::MissingAsset {
            path: path.to_string(),
            message,
        })
}

fn validate_route_policies(
    target: String,
    routes: &BTreeSet<String>,
    route_policy: &impl Fn(
        &str,
    ) -> Result<
        Option<lxapp::host::EffectiveRoutePolicy>,
        lxapp::host::RoutePolicyConflict,
    >,
    compatible: impl Fn(lxapp::host::RouteAudience) -> bool,
) -> Result<(), StaticSettingsTargetError> {
    for route in routes {
        let Some(policy) = route_policy(route)? else {
            return Err(StaticSettingsTargetError::UnknownBridgeRoute {
                target,
                route: route.clone(),
            });
        };
        if !compatible(policy.audience()) {
            return Err(StaticSettingsTargetError::IncompatibleBridgeRoute {
                target,
                route: route.clone(),
                actual: policy.audience(),
            });
        }
    }
    Ok(())
}

static VALIDATED_TARGETS: OnceLock<ValidatedStaticSettingsTargets> = OnceLock::new();

pub(crate) fn install_validated(targets: ValidatedStaticSettingsTargets) -> Result<(), String> {
    VALIDATED_TARGETS
        .set(targets)
        .map_err(|_| "static Settings targets were already initialized".to_string())
}

#[allow(dead_code)]
pub(crate) fn validated() -> Option<&'static ValidatedStaticSettingsTargets> {
    VALIDATED_TARGETS.get()
}

/// Returns the bootstrap-validated Settings declaration, if this product has
/// one. Native chrome may use this only to decide whether to render an
/// affordance; clicks must still resolve through the initialized runtime handle.
pub fn static_settings_destination() -> Option<&'static SettingsDestination> {
    validated().and_then(ValidatedStaticSettingsTargets::destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lingxia_app_context::{CapabilitiesConfig, EnvVersion};
    use std::cell::Cell;
    use std::sync::Arc;

    struct TestHostHandler;

    impl lxapp::host::HostHandler for TestHostHandler {
        fn call<'a>(
            &'a self,
            _invocation: lxapp::host::HostInvocationContext,
            _input: Option<String>,
            _cancel: lxapp::host::HostCancel,
        ) -> lxapp::host::HostFuture<'a> {
            Box::pin(async { Ok(lxapp::host::HostOutput::Json("null".to_string())) })
        }
    }

    #[derive(Default)]
    struct FakeAssets {
        files: BTreeMap<String, String>,
        reads: Cell<usize>,
        runtime_creations: Cell<usize>,
    }

    impl StaticAssetReader for FakeAssets {
        fn read_text(&self, path: &str) -> Result<String, String> {
            self.reads.set(self.reads.get() + 1);
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| "not found".to_string())
        }

        fn asset_exists(&self, path: &str) -> Result<(), String> {
            self.reads.set(self.reads.get() + 1);
            self.files
                .contains_key(path)
                .then_some(())
                .ok_or_else(|| "not found".to_string())
        }
    }

    fn app_config(destination: Option<SettingsDestination>) -> AppConfig {
        AppConfig {
            product_name: "Settings Test".to_string(),
            product_version: "1.0.0".to_string(),
            lingxia_id: None,
            lingxia_server: None,
            env_version: EnvVersion::Release,
            home_app_id: "control".to_string(),
            home_app_version: "1.0.0".to_string(),
            cache_max_size_mb: 1024,
            storage: None,
            splash: None,
            dev_ws_url: None,
            dev_bundle_base_url: None,
            app_links: None,
            theme: None,
            settings_destination: destination,
            capabilities: Some(CapabilitiesConfig {
                browser: true,
                ..CapabilitiesConfig::default()
            }),
            panels: None,
        }
    }

    fn catalog_for(config: &AppConfig) -> StaticSettingsTargetCatalog {
        let mut catalog = StaticSettingsTargetCatalog::new();
        if let Some(destination) = config.settings_destination.clone() {
            catalog.set_destination("app.json", destination);
        }
        catalog
    }

    fn browser_catalog_for(config: &AppConfig) -> StaticSettingsTargetCatalog {
        let mut catalog = catalog_for(config);
        catalog.require_browser_page_routes(
            "/settings",
            [
                "app.getInfo",
                "downloads.chooseDirectory",
                "downloads.getSettings",
                "downloads.resetDirectory",
                "privacy.clearBrowsingData",
                "privacy.clearSiteData",
                "privacy.getSiteDataContext",
                "privacy.getUsage",
                "app.getDisplayLanguageState",
                "app.setDisplayLanguagePreference",
                "app.watchDisplayLanguageState",
            ],
        );
        catalog
    }

    fn control_assets() -> FakeAssets {
        FakeAssets {
            files: BTreeMap::from([
                (
                    "control/lxapp.json".to_string(),
                    r#"{"appId":"control","pages":[{"name":"settings","path":"pages/settings/index.html"}]}"#.to_string(),
                ),
                ("control/pages/settings/index.html".to_string(), "<html/>".to_string()),
            ]),
            ..FakeAssets::default()
        }
    }

    fn browser_assets() -> FakeAssets {
        FakeAssets {
            files: BTreeMap::from([
                (
                    BROWSER_MANIFEST_ASSET.to_string(),
                    r#"{"appId":"app.lingxia.browser","pages":[{"name":"settings","path":"pages/settings/index.html"}]}"#.to_string(),
                ),
                (
                    "app.lingxia.browser/pages/settings/index.html".to_string(),
                    "<html/>".to_string(),
                ),
            ]),
            ..FakeAssets::default()
        }
    }

    fn control_policy(
        _: &str,
    ) -> Result<Option<lxapp::host::EffectiveRoutePolicy>, lxapp::host::RoutePolicyConflict> {
        Ok(Some(lxapp::host::EffectiveRoutePolicy::new(
            lxapp::host::RouteAudience::ControlAppOnly,
        )))
    }

    fn browser_policy(
        _: &str,
    ) -> Result<Option<lxapp::host::EffectiveRoutePolicy>, lxapp::host::RoutePolicyConflict> {
        Ok(Some(lxapp::host::EffectiveRoutePolicy::new(
            lxapp::host::RouteAudience::BrowserControlOnly,
        )))
    }

    fn assert_runtime_registries_empty() {
        assert!(crate::runtime::platform().is_err());
        assert!(lxapp::get_platform().is_none());
        assert_eq!(
            lxapp::get_current_lxapp(),
            (String::new(), String::new(), 0)
        );
    }

    #[test]
    fn zero_destination_is_valid_and_has_no_startup_side_effects() {
        let config = app_config(None);
        let assets = FakeAssets::default();
        let validated =
            validate_with_inventory(&config, catalog_for(&config), &assets, |_| Ok(None), false)
                .unwrap();
        assert!(validated.destination().is_none());
        assert_eq!(assets.reads.get(), 0);
        assert_eq!(assets.runtime_creations.get(), 0);
        assert_runtime_registries_empty();
    }

    #[test]
    fn conflicting_destination_writers_fail_deterministically() {
        let config = app_config(None);
        let mut catalog = StaticSettingsTargetCatalog::new();
        catalog.set_destination(
            "z-addon",
            SettingsDestination::NativeAction {
                action_id: "second".to_string(),
            },
        );
        catalog.set_destination(
            "a-config",
            SettingsDestination::NativeAction {
                action_id: "first".to_string(),
            },
        );
        let error = validate_with_inventory(
            &config,
            catalog,
            &FakeAssets::default(),
            |_| Ok(None),
            false,
        )
        .unwrap_err();
        assert_eq!(
            error,
            StaticSettingsTargetError::ConflictingDestinations {
                first: "a-config".to_string(),
                second: "z-addon".to_string(),
            }
        );
        assert!(validated().is_none());
        assert_runtime_registries_empty();
    }

    #[test]
    fn control_page_validates_identity_manifest_asset_and_route_policy() {
        let destination = SettingsDestination::ControlAppPage {
            app_id: "control".to_string(),
            page: "settings".to_string(),
            query: None,
        };
        let config = app_config(Some(destination));
        let mut catalog = catalog_for(&config);
        catalog.require_control_page_routes("control", "settings", ["control.save"]);
        let assets = control_assets();
        let validated =
            validate_with_inventory(&config, catalog, &assets, control_policy, false).unwrap();
        assert!(validated.destination().is_some());
        assert_eq!(assets.runtime_creations.get(), 0);

        for (mut broken_config, broken_assets) in [
            {
                let mut config = app_config(Some(SettingsDestination::ControlAppPage {
                    app_id: "other".to_string(),
                    page: "settings".to_string(),
                    query: None,
                }));
                config.home_app_id = "control".to_string();
                (config, control_assets())
            },
            (
                app_config(Some(SettingsDestination::ControlAppPage {
                    app_id: "control".to_string(),
                    page: "missing".to_string(),
                    query: None,
                })),
                control_assets(),
            ),
            (
                app_config(Some(SettingsDestination::ControlAppPage {
                    app_id: "control".to_string(),
                    page: "settings".to_string(),
                    query: None,
                })),
                FakeAssets {
                    files: BTreeMap::from([(
                        "control/lxapp.json".to_string(),
                        r#"{"appId":"control","pages":[{"name":"settings","path":"missing.html"}]}"#.to_string(),
                    )]),
                    ..FakeAssets::default()
                },
            ),
        ] {
            let catalog = catalog_for(&broken_config);
            assert!(
                validate_with_inventory(
                    &broken_config,
                    catalog,
                    &broken_assets,
                    control_policy,
                    false,
                )
                .is_err()
            );
            broken_config.settings_destination = None;
        }
    }

    #[test]
    fn browser_page_requires_capability_static_route_asset_and_compatible_routes() {
        let destination = SettingsDestination::BrowserControlPage {
            route: "/settings".to_string(),
            query: None,
        };
        let config = app_config(Some(destination));
        validate_with_inventory(
            &config,
            browser_catalog_for(&config),
            &browser_assets(),
            browser_policy,
            true,
        )
        .unwrap();

        let mut disabled = config.clone();
        disabled.capabilities.as_mut().unwrap().browser = false;
        assert_eq!(
            validate_with_inventory(
                &disabled,
                browser_catalog_for(&disabled),
                &browser_assets(),
                browser_policy,
                true,
            )
            .unwrap_err(),
            StaticSettingsTargetError::BrowserCapabilityDisabled
        );
        assert!(
            validate_with_inventory(
                &config,
                browser_catalog_for(&config),
                &FakeAssets::default(),
                browser_policy,
                true,
            )
            .is_err()
        );

        let unknown = app_config(Some(SettingsDestination::BrowserControlPage {
            route: "/downloads".to_string(),
            query: None,
        }));
        assert!(matches!(
            validate_with_inventory(
                &unknown,
                browser_catalog_for(&unknown),
                &browser_assets(),
                browser_policy,
                true,
            ),
            Err(StaticSettingsTargetError::UnknownBrowserRoute { .. })
        ));
    }

    #[test]
    fn native_actions_are_predeclared_unique_and_sealed() {
        let config = app_config(Some(SettingsDestination::NativeAction {
            action_id: "openPreferences".to_string(),
        }));
        let mut catalog = catalog_for(&config);
        catalog.register_native_action("host", "openPreferences");
        let validated = validate_with_inventory(
            &config,
            catalog,
            &FakeAssets::default(),
            |_| Ok(None),
            false,
        )
        .unwrap();
        assert!(validated.native_actions().contains("openPreferences"));

        assert_eq!(
            validate_with_inventory(
                &config,
                catalog_for(&config),
                &FakeAssets::default(),
                |_| Ok(None),
                false,
            )
            .unwrap_err(),
            StaticSettingsTargetError::UnknownNativeAction {
                action_id: "openPreferences".to_string(),
            }
        );

        let mut duplicate = catalog_for(&config);
        duplicate.register_native_action("z-addon", "openPreferences");
        duplicate.register_native_action("a-addon", "openPreferences");
        assert_eq!(
            validate_with_inventory(
                &config,
                duplicate,
                &FakeAssets::default(),
                |_| Ok(None),
                false,
            )
            .unwrap_err(),
            StaticSettingsTargetError::DuplicateNativeAction {
                action_id: "openPreferences".to_string(),
                sources: "a-addon, z-addon".to_string(),
            }
        );
    }

    #[test]
    fn required_routes_reject_unknown_and_wrong_audience() {
        let config = app_config(Some(SettingsDestination::ControlAppPage {
            app_id: "control".to_string(),
            page: "settings".to_string(),
            query: None,
        }));
        let mut catalog = catalog_for(&config);
        catalog.require_control_page_routes("control", "settings", ["control.save"]);
        assert!(matches!(
            validate_with_inventory(&config, catalog, &control_assets(), |_| Ok(None), false,),
            Err(StaticSettingsTargetError::UnknownBridgeRoute { .. })
        ));

        let mut catalog = catalog_for(&config);
        catalog.require_control_page_routes("control", "settings", ["control.save"]);
        assert!(matches!(
            validate_with_inventory(
                &config,
                catalog,
                &control_assets(),
                |_| {
                    Ok(Some(lxapp::host::EffectiveRoutePolicy::new(
                        lxapp::host::RouteAudience::BrowserControlOnly,
                    )))
                },
                false,
            ),
            Err(StaticSettingsTargetError::IncompatibleBridgeRoute { .. })
        ));

        let browser = app_config(Some(SettingsDestination::BrowserControlPage {
            route: "/settings".to_string(),
            query: None,
        }));
        assert!(matches!(
            validate_with_inventory(
                &browser,
                browser_catalog_for(&browser),
                &browser_assets(),
                control_policy,
                true,
            ),
            Err(StaticSettingsTargetError::IncompatibleBridgeRoute { .. })
        ));
        validate_with_inventory(
            &browser,
            browser_catalog_for(&browser),
            &browser_assets(),
            |_| {
                Ok(Some(lxapp::host::EffectiveRoutePolicy::new(
                    lxapp::host::RouteAudience::ControlOnly,
                )))
            },
            true,
        )
        .expect("ControlOnly is compatible with both control target classes");
    }

    #[test]
    fn route_inventory_registration_is_idempotent_and_policy_conflicts_fail_closed() {
        fn register_inventory() {
            static REGISTERED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
            REGISTERED.get_or_init(|| {
                lxapp::host::register_host_route(
                    "settings_target_policy_test",
                    "save",
                    lxapp::host::RouteAudience::ControlAppOnly,
                    Arc::new(TestHostHandler),
                );
            });
        }

        for _ in 0..2 {
            register_inventory();
        }
        assert_eq!(
            lxapp::host::route_policy("settings_target_policy_test.save")
                .unwrap()
                .map(lxapp::host::EffectiveRoutePolicy::audience),
            Some(lxapp::host::RouteAudience::ControlAppOnly)
        );

        let conflicting = std::panic::catch_unwind(|| {
            lxapp::host::register_host_route(
                "settings_target_policy_test",
                "save",
                lxapp::host::RouteAudience::BrowserControlOnly,
                Arc::new(TestHostHandler),
            );
        });
        assert!(conflicting.is_err());
        assert_eq!(
            lxapp::host::route_policy("settings_target_policy_test.save")
                .unwrap()
                .map(lxapp::host::EffectiveRoutePolicy::audience),
            Some(lxapp::host::RouteAudience::ControlAppOnly),
            "conflicting registration must not replace the sealed policy"
        );
    }
}
