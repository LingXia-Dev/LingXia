use crate::{SettingsDestination, ValidatedStaticSettingsTargets};
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};
use thiserror::Error;

type NativeSettingsAction = Arc<dyn Fn() -> Result<(), String> + Send + Sync>;

/// Startup-only registrar for native Settings actions declared in the static
/// target catalog. The resulting handler map is sealed before runtime starts.
pub struct NativeSettingsActionRegistrar {
    allowed: crate::SealedNativeActionRegistry,
    handlers: BTreeMap<String, NativeSettingsAction>,
}

impl NativeSettingsActionRegistrar {
    pub(crate) fn new(allowed: crate::SealedNativeActionRegistry) -> Self {
        Self {
            allowed,
            handlers: BTreeMap::new(),
        }
    }

    pub fn register<F>(&mut self, action_id: &str, action: F) -> Result<(), String>
    where
        F: Fn() -> Result<(), String> + Send + Sync + 'static,
    {
        if !self.allowed.contains(action_id) {
            return Err(format!(
                "native Settings action is not statically declared: {action_id}"
            ));
        }
        if self.handlers.contains_key(action_id) {
            return Err(format!(
                "native Settings action handler was registered more than once: {action_id}"
            ));
        }
        self.handlers
            .insert(action_id.to_string(), Arc::new(action));
        Ok(())
    }

    pub(crate) fn seal(self) -> SealedNativeSettingsActions {
        SealedNativeSettingsActions {
            handlers: self.handlers,
        }
    }
}

pub(crate) struct SealedNativeSettingsActions {
    handlers: BTreeMap<String, NativeSettingsAction>,
}

impl SealedNativeSettingsActions {
    fn invoke(&self, action_id: &str) -> Result<(), SettingsDestinationResolveError> {
        let action = self.handlers.get(action_id).cloned().ok_or_else(|| {
            SettingsDestinationResolveError::NativeActionMissing {
                action_id: action_id.to_string(),
            }
        })?;
        action().map_err(
            |message| SettingsDestinationResolveError::NativeActionFailed {
                action_id: action_id.to_string(),
                message,
            },
        )
    }
}

static NATIVE_ACTIONS: OnceLock<SealedNativeSettingsActions> = OnceLock::new();
static CONTROL_AUTHORITY: OnceLock<lxapp::NativeControlPlaneAuthority> = OnceLock::new();

pub(crate) fn install_native_actions(actions: SealedNativeSettingsActions) -> Result<(), String> {
    NATIVE_ACTIONS
        .set(actions)
        .map_err(|_| "native Settings actions were already initialized".to_string())
}

pub(crate) fn install_control_authority(
    authority: lxapp::NativeControlPlaneAuthority,
) -> Result<(), String> {
    CONTROL_AUTHORITY
        .set(authority)
        .map_err(|_| "native Settings control authority was already initialized".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsDestinationResolution {
    ControlAppPage {
        app_id: String,
        session_id: u64,
    },
    BrowserControlPage {
        tab_id: String,
        browser_session_id: u64,
    },
    NativeAction {
        action_id: String,
    },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SettingsDestinationResolveError {
    #[error("static Settings targets are not initialized")]
    NotInitialized,
    #[error("no Settings destination is configured")]
    NotConfigured,
    #[error("Settings control app '{actual}' does not match sealed control app '{expected}'")]
    ControlAppIdentityMismatch { expected: String, actual: String },
    #[error("failed to open Settings control page: {0}")]
    ControlApp(String),
    #[error("failed to navigate trusted browser Settings page: {0}")]
    Browser(String),
    #[error("native Settings action '{action_id}' is not statically declared")]
    NativeActionNotDeclared { action_id: String },
    #[error("native Settings action '{action_id}' has no installed handler")]
    NativeActionMissing { action_id: String },
    #[error("native Settings action '{action_id}' failed: {message}")]
    NativeActionFailed { action_id: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ControlPageIdentity {
    app_id: String,
    session_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BrowserPageIdentity {
    tab_id: String,
    browser_session_id: u64,
}

trait SettingsDestinationRuntime {
    fn open_control_page(
        &self,
        app_id: &str,
        page: &str,
        query: Option<&BTreeMap<String, serde_json::Value>>,
    ) -> Result<ControlPageIdentity, String>;

    fn navigate_browser_control_page(
        &self,
        route: &str,
        query: Option<&BTreeMap<String, serde_json::Value>>,
    ) -> Result<BrowserPageIdentity, String>;

    fn invoke_native_action(&self, action_id: &str) -> Result<(), SettingsDestinationResolveError>;
}

struct BootstrapSettingsDestinationRuntime;

fn query_value(query: Option<&BTreeMap<String, serde_json::Value>>) -> Option<serde_json::Value> {
    query.map(|query| {
        serde_json::Value::Object(
            query
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        )
    })
}

impl SettingsDestinationRuntime for BootstrapSettingsDestinationRuntime {
    fn open_control_page(
        &self,
        app_id: &str,
        page: &str,
        query: Option<&BTreeMap<String, serde_json::Value>>,
    ) -> Result<ControlPageIdentity, String> {
        let query = query_value(query);
        let options = lxapp::LxAppStartupOptions::for_page(Some(page), query.as_ref())?;
        let authority = CONTROL_AUTHORITY
            .get()
            .ok_or_else(|| "native Settings control authority is not initialized".to_string())?;
        let app = lxapp::open_control_lxapp_page(authority, app_id, options)
            .map_err(|error| error.to_string())?;
        Ok(ControlPageIdentity {
            app_id: app.appid.clone(),
            session_id: app.session_id(),
        })
    }

    fn navigate_browser_control_page(
        &self,
        route: &str,
        query: Option<&BTreeMap<String, serde_json::Value>>,
    ) -> Result<BrowserPageIdentity, String> {
        let route = route.trim().trim_start_matches('/');
        let mut url = format!("lingxia://{route}");
        if let Some(query) = query_value(query) {
            url = lxapp::append_page_query(url, &query)?;
        }
        let (tab_id, browser_session_id) = crate::browser::navigate_trusted_control_page(&url)
            .map_err(|error| error.to_string())?;
        Ok(BrowserPageIdentity {
            tab_id,
            browser_session_id,
        })
    }

    fn invoke_native_action(&self, action_id: &str) -> Result<(), SettingsDestinationResolveError> {
        NATIVE_ACTIONS
            .get()
            .ok_or(SettingsDestinationResolveError::NotInitialized)?
            .invoke(action_id)
    }
}

/// Resolve and execute the sealed Settings destination against current runtime
/// registries. No live app, tab, WebView, session, handler, or closure is kept
/// between calls.
pub fn resolve_settings_destination()
-> Result<SettingsDestinationResolution, SettingsDestinationResolveError> {
    let snapshot = crate::settings_target::validated()
        .ok_or(SettingsDestinationResolveError::NotInitialized)?;
    resolve_from_snapshot(snapshot, &BootstrapSettingsDestinationRuntime)
}

fn resolve_from_snapshot(
    snapshot: &ValidatedStaticSettingsTargets,
    runtime: &dyn SettingsDestinationRuntime,
) -> Result<SettingsDestinationResolution, SettingsDestinationResolveError> {
    let destination = snapshot
        .destination()
        .cloned()
        .ok_or(SettingsDestinationResolveError::NotConfigured)?;
    match destination {
        SettingsDestination::ControlAppPage {
            app_id,
            page,
            query,
        } => {
            let expected = snapshot.control_app_id().unwrap_or_default();
            if app_id != expected {
                return Err(
                    SettingsDestinationResolveError::ControlAppIdentityMismatch {
                        expected: expected.to_string(),
                        actual: app_id,
                    },
                );
            }
            let identity = runtime
                .open_control_page(&app_id, &page, query.as_ref())
                .map_err(SettingsDestinationResolveError::ControlApp)?;
            Ok(SettingsDestinationResolution::ControlAppPage {
                app_id: identity.app_id,
                session_id: identity.session_id,
            })
        }
        SettingsDestination::BrowserControlPage { route, query } => {
            let identity = runtime
                .navigate_browser_control_page(&route, query.as_ref())
                .map_err(SettingsDestinationResolveError::Browser)?;
            Ok(SettingsDestinationResolution::BrowserControlPage {
                tab_id: identity.tab_id,
                browser_session_id: identity.browser_session_id,
            })
        }
        SettingsDestination::NativeAction { action_id } => {
            if !snapshot.native_actions().contains(&action_id) {
                return Err(SettingsDestinationResolveError::NativeActionNotDeclared { action_id });
            }
            runtime.invoke_native_action(&action_id)?;
            Ok(SettingsDestinationResolution::NativeAction { action_id })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::sync::Mutex;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum FakeControlState {
        Missing,
        ClosedControl(u64),
        OpenControl(u64),
        OpenStandard(u64),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum FakeBrowserDocument {
        Missing,
        Internal(String),
        External,
        Discarded,
    }

    #[derive(Debug)]
    struct FakeBrowserState {
        current_session: u64,
        tab_session: Option<u64>,
        generation: u64,
        navigation_count: usize,
        document: FakeBrowserDocument,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ControlNavigation {
        session_id: u64,
        page: String,
        query: Option<BTreeMap<String, serde_json::Value>>,
    }

    #[derive(Default)]
    struct FakeRuntime {
        next_control_session: RefCell<u64>,
        control: RefCell<Option<FakeControlState>>,
        control_navigations: RefCell<Vec<ControlNavigation>>,
        browser: RefCell<Option<FakeBrowserState>>,
        browser_urls: RefCell<Vec<String>>,
        native_handlers: RefCell<BTreeMap<String, Result<(), String>>>,
        native_invocations: RefCell<Vec<String>>,
    }

    impl FakeRuntime {
        fn set_control(&self, state: FakeControlState) {
            *self.control.borrow_mut() = Some(state);
        }

        fn set_browser(&self, state: FakeBrowserState) {
            *self.browser.borrow_mut() = Some(state);
        }

        fn browser_state(&self) -> std::cell::Ref<'_, FakeBrowserState> {
            std::cell::Ref::map(self.browser.borrow(), |state| state.as_ref().unwrap())
        }
    }

    impl SettingsDestinationRuntime for FakeRuntime {
        fn open_control_page(
            &self,
            app_id: &str,
            page: &str,
            query: Option<&BTreeMap<String, serde_json::Value>>,
        ) -> Result<ControlPageIdentity, String> {
            let state = self.control.borrow().unwrap_or(FakeControlState::Missing);
            let session_id = match state {
                FakeControlState::OpenControl(session_id)
                | FakeControlState::ClosedControl(session_id) => session_id,
                FakeControlState::Missing | FakeControlState::OpenStandard(_) => {
                    let next = self.next_control_session.borrow().saturating_add(1);
                    *self.next_control_session.borrow_mut() = next;
                    next
                }
            };
            *self.control.borrow_mut() = Some(FakeControlState::OpenControl(session_id));
            self.control_navigations
                .borrow_mut()
                .push(ControlNavigation {
                    session_id,
                    page: page.to_string(),
                    query: query.cloned(),
                });
            Ok(ControlPageIdentity {
                app_id: app_id.to_string(),
                session_id,
            })
        }

        fn navigate_browser_control_page(
            &self,
            route: &str,
            query: Option<&BTreeMap<String, serde_json::Value>>,
        ) -> Result<BrowserPageIdentity, String> {
            let query = query_value(query);
            let mut url = format!("lingxia://{}", route.trim_start_matches('/'));
            if let Some(query) = query {
                url = lxapp::append_page_query(url, &query)?;
            }
            let mut browser = self.browser.borrow_mut();
            let browser = browser.as_mut().ok_or("browser is not initialized")?;
            if browser.tab_session != Some(browser.current_session)
                || matches!(
                    browser.document,
                    FakeBrowserDocument::Missing | FakeBrowserDocument::Discarded
                )
            {
                browser.generation += 1;
                browser.tab_session = Some(browser.current_session);
            }
            browser.navigation_count += 1;
            browser.document = FakeBrowserDocument::Internal(url.clone());
            self.browser_urls.borrow_mut().push(url);
            Ok(BrowserPageIdentity {
                tab_id: "settings".to_string(),
                browser_session_id: browser.current_session,
            })
        }

        fn invoke_native_action(
            &self,
            action_id: &str,
        ) -> Result<(), SettingsDestinationResolveError> {
            let result = self
                .native_handlers
                .borrow()
                .get(action_id)
                .cloned()
                .ok_or_else(|| SettingsDestinationResolveError::NativeActionMissing {
                    action_id: action_id.to_string(),
                })?;
            self.native_invocations
                .borrow_mut()
                .push(action_id.to_string());
            result.map_err(
                |message| SettingsDestinationResolveError::NativeActionFailed {
                    action_id: action_id.to_string(),
                    message,
                },
            )
        }
    }

    fn control_snapshot() -> ValidatedStaticSettingsTargets {
        ValidatedStaticSettingsTargets::for_runtime_test(
            Some(SettingsDestination::ControlAppPage {
                app_id: "control".to_string(),
                page: "settings".to_string(),
                query: Some(BTreeMap::from([
                    ("compact".to_string(), serde_json::json!(true)),
                    ("source".to_string(), serde_json::json!("menu")),
                ])),
            }),
            Some("control"),
            &[],
        )
    }

    fn browser_snapshot() -> ValidatedStaticSettingsTargets {
        ValidatedStaticSettingsTargets::for_runtime_test(
            Some(SettingsDestination::BrowserControlPage {
                route: "/settings".to_string(),
                query: Some(BTreeMap::from([(
                    "section".to_string(),
                    serde_json::json!("privacy"),
                )])),
            }),
            None,
            &[],
        )
    }

    #[test]
    fn control_page_resolves_current_control_identity_for_every_navigation() {
        let runtime = FakeRuntime::default();
        runtime.set_control(FakeControlState::Missing);

        let first = resolve_from_snapshot(&control_snapshot(), &runtime).unwrap();
        let second = resolve_from_snapshot(&control_snapshot(), &runtime).unwrap();
        assert_eq!(
            (first, second),
            (
                SettingsDestinationResolution::ControlAppPage {
                    app_id: "control".to_string(),
                    session_id: 1,
                },
                SettingsDestinationResolution::ControlAppPage {
                    app_id: "control".to_string(),
                    session_id: 1,
                },
            )
        );

        runtime.set_control(FakeControlState::ClosedControl(4));
        let reopened = resolve_from_snapshot(&control_snapshot(), &runtime).unwrap();
        assert!(matches!(
            reopened,
            SettingsDestinationResolution::ControlAppPage { session_id: 4, .. }
        ));

        runtime.set_control(FakeControlState::OpenStandard(8));
        let promoted = resolve_from_snapshot(&control_snapshot(), &runtime).unwrap();
        assert!(matches!(
            promoted,
            SettingsDestinationResolution::ControlAppPage { session_id: 2, .. }
        ));
        assert_eq!(
            *runtime.control.borrow(),
            Some(FakeControlState::OpenControl(2))
        );

        // Simulate restart replacing the Arc/session held by the registry. The
        // next resolve must not reuse the identity returned above.
        runtime.set_control(FakeControlState::OpenControl(55));
        let restarted = resolve_from_snapshot(&control_snapshot(), &runtime).unwrap();
        assert!(matches!(
            restarted,
            SettingsDestinationResolution::ControlAppPage { session_id: 55, .. }
        ));
        let navigations = runtime.control_navigations.borrow();
        assert_eq!(navigations.len(), 5);
        assert!(navigations.iter().all(|navigation| {
            navigation.page == "settings"
                && navigation
                    .query
                    .as_ref()
                    .is_some_and(|query| query.get("source") == Some(&serde_json::json!("menu")))
        }));
    }

    #[test]
    fn browser_page_always_requests_fresh_trusted_navigation() {
        let runtime = FakeRuntime::default();
        runtime.set_browser(FakeBrowserState {
            current_session: 10,
            tab_session: None,
            generation: 0,
            navigation_count: 0,
            document: FakeBrowserDocument::Missing,
        });

        let first = resolve_from_snapshot(&browser_snapshot(), &runtime).unwrap();
        assert!(matches!(
            first,
            SettingsDestinationResolution::BrowserControlPage {
                browser_session_id: 10,
                ..
            }
        ));
        assert_eq!(runtime.browser_state().navigation_count, 1);
        assert_eq!(runtime.browser_state().generation, 1);

        // Same stable tab and same URL still performs another trusted load.
        resolve_from_snapshot(&browser_snapshot(), &runtime).unwrap();
        assert_eq!(runtime.browser_state().navigation_count, 2);

        runtime.browser.borrow_mut().as_mut().unwrap().document = FakeBrowserDocument::External;
        resolve_from_snapshot(&browser_snapshot(), &runtime).unwrap();
        assert_eq!(runtime.browser_state().navigation_count, 3);
        assert!(matches!(
            runtime.browser_state().document,
            FakeBrowserDocument::Internal(ref url) if url == "lingxia://settings?section=privacy"
        ));

        runtime.browser.borrow_mut().as_mut().unwrap().document = FakeBrowserDocument::Discarded;
        resolve_from_snapshot(&browser_snapshot(), &runtime).unwrap();
        assert_eq!(runtime.browser_state().generation, 2);
        assert_eq!(runtime.browser_state().navigation_count, 4);

        // Browser restart leaves the stable tab's old session stale. Resolution
        // creates a new generation and returns only the current session.
        runtime
            .browser
            .borrow_mut()
            .as_mut()
            .unwrap()
            .current_session = 11;
        let restarted = resolve_from_snapshot(&browser_snapshot(), &runtime).unwrap();
        assert!(matches!(
            restarted,
            SettingsDestinationResolution::BrowserControlPage {
                browser_session_id: 11,
                ..
            }
        ));
        assert_eq!(runtime.browser_state().generation, 3);
        assert_eq!(runtime.browser_state().navigation_count, 5);
        assert_eq!(runtime.browser_urls.borrow().len(), 5);
    }

    #[test]
    fn native_action_is_looked_up_again_and_missing_never_reuses_a_handler() {
        let snapshot = ValidatedStaticSettingsTargets::for_runtime_test(
            Some(SettingsDestination::NativeAction {
                action_id: "preferences".to_string(),
            }),
            None,
            &["preferences"],
        );
        let runtime = FakeRuntime::default();
        runtime
            .native_handlers
            .borrow_mut()
            .insert("preferences".to_string(), Ok(()));
        resolve_from_snapshot(&snapshot, &runtime).unwrap();
        runtime.native_handlers.borrow_mut().remove("preferences");

        assert_eq!(
            resolve_from_snapshot(&snapshot, &runtime).unwrap_err(),
            SettingsDestinationResolveError::NativeActionMissing {
                action_id: "preferences".to_string(),
            }
        );
        assert_eq!(
            *runtime.native_invocations.borrow(),
            vec!["preferences".to_string()]
        );
    }

    #[test]
    fn invalid_snapshot_fails_before_any_runtime_target_or_frame() {
        let snapshot = ValidatedStaticSettingsTargets::for_runtime_test(
            Some(SettingsDestination::ControlAppPage {
                app_id: "wrong".to_string(),
                page: "settings".to_string(),
                query: None,
            }),
            Some("control"),
            &[],
        );
        let runtime = FakeRuntime::default();
        assert!(matches!(
            resolve_from_snapshot(&snapshot, &runtime),
            Err(SettingsDestinationResolveError::ControlAppIdentityMismatch { .. })
        ));
        assert!(runtime.control_navigations.borrow().is_empty());
        assert!(runtime.browser_urls.borrow().is_empty());
        assert!(runtime.native_invocations.borrow().is_empty());

        let undeclared = ValidatedStaticSettingsTargets::for_runtime_test(
            Some(SettingsDestination::NativeAction {
                action_id: "missing".to_string(),
            }),
            None,
            &[],
        );
        assert!(matches!(
            resolve_from_snapshot(&undeclared, &runtime),
            Err(SettingsDestinationResolveError::NativeActionNotDeclared { .. })
        ));
        assert!(runtime.native_invocations.borrow().is_empty());
    }

    #[test]
    fn native_action_registrar_accepts_only_one_handler_for_declared_ids() {
        let declarations =
            ValidatedStaticSettingsTargets::for_runtime_test(None, None, &["preferences"]);
        let calls = Arc::new(Mutex::new(0usize));
        let call_counter = calls.clone();
        let mut registrar =
            NativeSettingsActionRegistrar::new(declarations.native_actions().clone());
        registrar
            .register("preferences", move || {
                *call_counter.lock().unwrap() += 1;
                Ok(())
            })
            .unwrap();
        assert!(registrar.register("preferences", || Ok(())).is_err());
        assert!(registrar.register("undeclared", || Ok(())).is_err());

        let actions = registrar.seal();
        actions.invoke("preferences").unwrap();
        assert_eq!(*calls.lock().unwrap(), 1);
        assert!(matches!(
            actions.invoke("undeclared"),
            Err(SettingsDestinationResolveError::NativeActionMissing { .. })
        ));
    }
}
