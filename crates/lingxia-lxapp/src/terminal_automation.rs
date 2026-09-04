//! Process-local bridge between native terminal workspaces and trusted automation.
//!
//! Native hosts own pane views, so they publish semantic snapshots and pull
//! queued commands here. The JavaScript automation driver consumes the same
//! registry without learning platform view types or relying on screen coordinates.

use crate::LxApp;
use crate::host::{AppResourceGrant, AppScope};
use lingxia_platform::Platform;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::oneshot;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostCommand {
    id: u64,
    action: String,
    params: Value,
}

#[derive(Default)]
struct Registry {
    next_id: u64,
    snapshots: HashMap<SurfaceKey, Value>,
    queues: HashMap<SurfaceKey, VecDeque<HostCommand>>,
    pending: HashMap<u64, PendingCommand>,
}

struct PendingCommand {
    surface: SurfaceKey,
    caller: TerminalAutomationAuthority,
    sender: oneshot::Sender<Result<Value, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SurfaceOwner {
    NativeHost,
    App { app_id: String, session_id: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SurfaceKey {
    owner: SurfaceOwner,
    id: String,
}

#[derive(Clone)]
enum AuthorityKind {
    NativeHost(std::sync::Weak<Platform>),
    App(AppScope),
    #[cfg(test)]
    NativeTest,
}

/// Opaque proof returned only to the native caller that successfully boots the
/// process-wide lxapp runtime. Native extensions must be handed this proof by
/// that host; there is no global accessor or standalone constructor.
pub struct NativeHostRuntimeToken {
    runtime: std::sync::Weak<Platform>,
}

impl NativeHostRuntimeToken {
    pub(crate) fn new(runtime: &std::sync::Arc<Platform>) -> Self {
        Self {
            runtime: std::sync::Arc::downgrade(runtime),
        }
    }

    pub(crate) fn runtime(&self) -> &std::sync::Weak<Platform> {
        &self.runtime
    }

    #[cfg(feature = "test-utils")]
    #[doc(hidden)]
    pub fn for_test(runtime: &std::sync::Arc<Platform>) -> Self {
        Self::new(runtime)
    }
}

/// Native-derived authority for binding and using terminal surface handles.
/// Its fields are private, so a bridge payload cannot name an owner.
#[derive(Clone)]
pub struct TerminalAutomationAuthority {
    kind: AuthorityKind,
}

impl TerminalAutomationAuthority {
    #[cfg(test)]
    pub(crate) fn native_for_test() -> Self {
        Self {
            kind: AuthorityKind::NativeTest,
        }
    }

    /// Derive authority from an authenticated native lxapp object.
    pub fn for_lxapp(app: &std::sync::Arc<LxApp>) -> Result<Self, String> {
        Self::for_app(AppScope::from_lxapp(app))
    }

    /// Bind authority to one live app session with a sealed host-automation grant.
    pub fn for_app(scope: AppScope) -> Result<Self, String> {
        if !scope
            .resource_grants()
            .contains(AppResourceGrant::AutomationHost)
        {
            return Err("terminal automation requires a live native AutomationHost grant".into());
        }
        Ok(Self {
            kind: AuthorityKind::App(scope),
        })
    }

    /// Derive terminal authority from the opaque proof returned to the native
    /// host at successful runtime bootstrap.
    pub fn for_native_runtime(proof: &NativeHostRuntimeToken) -> Self {
        Self {
            kind: AuthorityKind::NativeHost(proof.runtime.clone()),
        }
    }

    fn validate(&self) -> Result<(), String> {
        match &self.kind {
            AuthorityKind::NativeHost(runtime) => {
                validate_native_runtime(runtime, crate::get_platform().as_ref())
            }
            AuthorityKind::App(scope) => scope
                .resource_grants()
                .contains(AppResourceGrant::AutomationHost)
                .then_some(())
                .ok_or_else(|| {
                    "terminal automation authority no longer matches a live app session".into()
                }),
            #[cfg(test)]
            AuthorityKind::NativeTest => Ok(()),
        }
    }

    #[cfg(feature = "test-utils")]
    #[doc(hidden)]
    pub fn validate_native_runtime_for_test(
        &self,
        current: Option<&std::sync::Arc<Platform>>,
    ) -> Result<(), String> {
        match &self.kind {
            AuthorityKind::NativeHost(runtime) => validate_native_runtime(runtime, current),
            _ => Err("terminal authority is not native-runtime-bound".into()),
        }
    }

    fn owner(&self) -> SurfaceOwner {
        match &self.kind {
            AuthorityKind::NativeHost(_) => SurfaceOwner::NativeHost,
            AuthorityKind::App(scope) => SurfaceOwner::App {
                app_id: scope.identity().app_id().to_string(),
                session_id: scope.identity().session_id(),
            },
            #[cfg(test)]
            AuthorityKind::NativeTest => SurfaceOwner::NativeHost,
        }
    }
}

fn validate_native_runtime(
    expected: &std::sync::Weak<Platform>,
    current: Option<&std::sync::Arc<Platform>>,
) -> Result<(), String> {
    let expected = expected
        .upgrade()
        .ok_or_else(|| "native host runtime is no longer live".to_string())?;
    current
        .filter(|current| std::sync::Arc::ptr_eq(current, &expected))
        .map(|_| ())
        .ok_or_else(|| "terminal authority does not match the live native host".into())
}

/// A terminal surface capability bound to both registry owner and caller.
#[derive(Clone)]
pub struct TerminalSurfaceHandle {
    key: SurfaceKey,
    authority: TerminalAutomationAuthority,
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}

/// Publish the latest semantic state for one native terminal surface.
pub fn publish_snapshot(
    authority: &TerminalAutomationAuthority,
    surface_id: &str,
    snapshot_json: &str,
) -> Result<(), String> {
    authority.validate()?;
    let snapshot = serde_json::from_str(snapshot_json)
        .map_err(|error| format!("invalid terminal automation snapshot: {error}"))?;
    let key = SurfaceKey {
        owner: authority.owner(),
        id: surface_id.to_string(),
    };
    registry()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .snapshots
        .insert(key, snapshot);
    Ok(())
}

/// Remove a native workspace and fail commands that have not reached it.
pub fn remove_workspace(authority: &TerminalAutomationAuthority, surface_id: &str) {
    if authority.validate().is_err() {
        return;
    }
    let key = SurfaceKey {
        owner: authority.owner(),
        id: surface_id.to_string(),
    };
    let mut registry = registry().lock().unwrap_or_else(|error| error.into_inner());
    registry.snapshots.remove(&key);
    registry.queues.remove(&key);
    let pending = registry
        .pending
        .iter()
        .filter_map(|(id, command)| (command.surface == key).then_some(*id))
        .collect::<Vec<_>>();
    for id in pending {
        if let Some(command) = registry.pending.remove(&id) {
            let _ = command.sender.send(Err(format!(
                "terminal surface '{}' closed before the command completed",
                surface_id
            )));
        }
    }
}

/// Resolve a surface string into a capability bound to the native registry
/// owner and the exact caller session.
pub fn bind_surface(
    authority: &TerminalAutomationAuthority,
    surface_id: &str,
) -> Result<TerminalSurfaceHandle, String> {
    authority.validate()?;
    let requested = SurfaceKey {
        owner: authority.owner(),
        id: surface_id.to_string(),
    };
    let native = SurfaceKey {
        owner: SurfaceOwner::NativeHost,
        id: surface_id.to_string(),
    };
    let registry = registry().lock().unwrap_or_else(|error| error.into_inner());
    let key = if registry.snapshots.contains_key(&requested) {
        requested
    } else if matches!(authority.kind, AuthorityKind::App(_))
        && registry.snapshots.contains_key(&native)
    {
        native
    } else {
        return Err(format!(
            "terminal surface '{surface_id}' is not available for this owner"
        ));
    };
    Ok(TerminalSurfaceHandle {
        key,
        authority: authority.clone(),
    })
}

impl TerminalSurfaceHandle {
    /// Read a host-published workspace snapshot after revalidating the owner.
    pub fn snapshot(&self) -> Result<Value, String> {
        self.authority.validate()?;
        registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .snapshots
            .get(&self.key)
            .cloned()
            .ok_or_else(|| format!("terminal surface '{}' is no longer available", self.key.id))
    }

    /// Queue an operation for the bound workspace and await its result.
    pub async fn run_command(
        &self,
        action: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        self.authority.validate()?;
        run_bound_command(self, action, params, timeout).await
    }
}

async fn run_bound_command(
    handle: &TerminalSurfaceHandle,
    action: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, String> {
    let (id, receiver) = {
        let mut registry = registry().lock().unwrap_or_else(|error| error.into_inner());
        if !registry.snapshots.contains_key(&handle.key) {
            return Err(format!(
                "terminal surface '{}' is not available",
                handle.key.id
            ));
        }
        registry.next_id = registry.next_id.wrapping_add(1).max(1);
        let id = registry.next_id;
        let (sender, receiver) = oneshot::channel();
        registry.pending.insert(
            id,
            PendingCommand {
                surface: handle.key.clone(),
                caller: handle.authority.clone(),
                sender,
            },
        );
        registry
            .queues
            .entry(handle.key.clone())
            .or_default()
            .push_back(HostCommand {
                id,
                action: action.to_string(),
                params,
            });
        (id, receiver)
    };

    match tokio::time::timeout(timeout, receiver).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("terminal automation host dropped the command".to_string()),
        Err(_) => {
            cancel_command(id);
            Err(format!("terminal automation command '{action}' timed out"))
        }
    }
}

fn cancel_command(id: u64) {
    let mut registry = registry().lock().unwrap_or_else(|error| error.into_inner());
    registry.pending.remove(&id);
    for queue in registry.queues.values_mut() {
        queue.retain(|command| command.id != id);
    }
}

/// Pull the next live command for a native workspace as JSON.
pub fn take_command(authority: &TerminalAutomationAuthority, surface_id: &str) -> String {
    if authority.validate().is_err() {
        return String::new();
    }
    let key = SurfaceKey {
        owner: authority.owner(),
        id: surface_id.to_string(),
    };
    let mut registry = registry().lock().unwrap_or_else(|error| error.into_inner());
    loop {
        let command = registry.queues.get_mut(&key).and_then(VecDeque::pop_front);
        let Some(command) = command else {
            return String::new();
        };
        let Some(pending) = registry.pending.get(&command.id) else {
            continue;
        };
        if pending.caller.validate().is_err() {
            if let Some(pending) = registry.pending.remove(&command.id) {
                let _ = pending.sender.send(Err(
                    "terminal automation caller session closed before command dispatch".into(),
                ));
            }
            continue;
        }
        return serde_json::to_string(&command).unwrap_or_default();
    }
}

/// Complete one command after the native workspace has reconciled its layout.
pub fn complete_command(
    authority: &TerminalAutomationAuthority,
    id: u64,
    ok: bool,
    payload: &str,
) -> bool {
    if authority.validate().is_err() {
        return false;
    }
    let mut registry = registry().lock().unwrap_or_else(|error| error.into_inner());
    let Some(pending) = registry.pending.get(&id) else {
        return false;
    };
    if pending.surface.owner != authority.owner() {
        return false;
    }
    if pending.caller.validate().is_err() {
        if let Some(pending) = registry.pending.remove(&id) {
            let _ = pending.sender.send(Err(
                "terminal automation caller session closed before command completion".into(),
            ));
        }
        return false;
    }
    let sender = registry.pending.remove(&id);
    drop(registry);
    let Some(command) = sender else {
        return false;
    };
    let result = if ok {
        serde_json::from_str(payload)
            .map_err(|error| format!("invalid terminal automation result: {error}"))
    } else {
        Err(payload.to_string())
    };
    command.sender.send(result).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn native_snapshot_and_command_share_one_surface_identity() {
        let authority = TerminalAutomationAuthority::native_for_test();
        let surface = "terminal-automation-registry-test";
        remove_workspace(&authority, surface);
        publish_snapshot(
            &authority,
            surface,
            r#"{"surfaceId":"terminal-test","tabs":[]}"#,
        )
        .unwrap();
        let handle = bind_surface(&authority, surface).unwrap();
        assert_eq!(handle.snapshot().unwrap()["surfaceId"], "terminal-test");

        let command_handle = handle.clone();
        let task = tokio::spawn(async move {
            command_handle
                .run_command(
                    "split",
                    serde_json::json!({ "direction": "right" }),
                    Duration::from_secs(1),
                )
                .await
        });
        tokio::task::yield_now().await;

        let command: Value = serde_json::from_str(&take_command(&authority, surface)).unwrap();
        assert_eq!(command["action"], "split");
        assert_eq!(command["params"]["direction"], "right");
        assert!(complete_command(
            &authority,
            command["id"].as_u64().unwrap(),
            true,
            r#"{"surfaceId":"terminal-test","paneCount":2}"#,
        ));

        let result = task.await.unwrap().unwrap();
        assert_eq!(result["paneCount"], 2);
        remove_workspace(&authority, surface);
    }

    #[tokio::test]
    async fn closing_a_surface_fails_a_command_already_taken_by_the_host() {
        let authority = TerminalAutomationAuthority::native_for_test();
        let surface = "terminal-automation-close-test";
        remove_workspace(&authority, surface);
        publish_snapshot(
            &authority,
            surface,
            r#"{"surfaceId":"terminal-close","tabs":[]}"#,
        )
        .unwrap();
        let handle = bind_surface(&authority, surface).unwrap();
        let task = tokio::spawn(async move {
            handle
                .run_command(
                    "split",
                    serde_json::json!({ "direction": "down" }),
                    Duration::from_secs(1),
                )
                .await
        });
        tokio::task::yield_now().await;
        assert!(!take_command(&authority, surface).is_empty());

        remove_workspace(&authority, surface);
        let error = task
            .await
            .unwrap()
            .expect_err("closed surface fails command");
        assert!(error.contains("closed before the command completed"));
    }

    #[test]
    fn native_handle_cannot_guess_an_app_owned_surface_id() {
        let authority = TerminalAutomationAuthority::native_for_test();
        let surface = "terminal-owner-isolation-test";
        let key = SurfaceKey {
            owner: SurfaceOwner::App {
                app_id: "same.app".into(),
                session_id: 41,
            },
            id: surface.into(),
        };
        registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .snapshots
            .insert(key.clone(), serde_json::json!({ "surfaceId": surface }));
        assert!(bind_surface(&authority, surface).is_err());
        registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .snapshots
            .remove(&key);
    }

    #[test]
    fn same_app_id_sessions_have_distinct_surface_owners() {
        let first = SurfaceOwner::App {
            app_id: "same.app".into(),
            session_id: 51,
        };
        let takeover = SurfaceOwner::App {
            app_id: "same.app".into(),
            session_id: 52,
        };
        assert_ne!(first, takeover);
        assert_ne!(first, SurfaceOwner::NativeHost);
    }
}
