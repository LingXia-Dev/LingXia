//! Process-local bridge between native terminal workspaces and trusted automation.
//!
//! Native hosts own pane views, so they publish semantic snapshots and pull
//! queued commands here. The JavaScript automation driver consumes the same
//! registry without learning platform view types or relying on screen coordinates.

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
    snapshots: HashMap<String, Value>,
    queues: HashMap<String, VecDeque<HostCommand>>,
    pending: HashMap<u64, PendingCommand>,
}

struct PendingCommand {
    surface_id: String,
    sender: oneshot::Sender<Result<Value, String>>,
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}

/// Publish the latest semantic state for one native terminal surface.
pub fn publish_snapshot(surface_id: &str, snapshot_json: &str) -> Result<(), String> {
    let snapshot = serde_json::from_str(snapshot_json)
        .map_err(|error| format!("invalid terminal automation snapshot: {error}"))?;
    registry()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .snapshots
        .insert(surface_id.to_string(), snapshot);
    Ok(())
}

/// Remove a native workspace and fail commands that have not reached it.
pub fn remove_workspace(surface_id: &str) {
    let mut registry = registry().lock().unwrap_or_else(|error| error.into_inner());
    registry.snapshots.remove(surface_id);
    registry.queues.remove(surface_id);
    let pending = registry
        .pending
        .iter()
        .filter_map(|(id, command)| (command.surface_id == surface_id).then_some(*id))
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

/// Read one host-published workspace snapshot.
pub fn snapshot(surface_id: &str) -> Result<Value, String> {
    registry()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .snapshots
        .get(surface_id)
        .cloned()
        .ok_or_else(|| format!("terminal surface '{surface_id}' is not available"))
}

/// Queue an operation for the native workspace and await its semantic result.
pub async fn run_command(
    surface_id: &str,
    action: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, String> {
    let (id, receiver) = {
        let mut registry = registry().lock().unwrap_or_else(|error| error.into_inner());
        if !registry.snapshots.contains_key(surface_id) {
            return Err(format!("terminal surface '{surface_id}' is not available"));
        }
        registry.next_id = registry.next_id.wrapping_add(1).max(1);
        let id = registry.next_id;
        let (sender, receiver) = oneshot::channel();
        registry.pending.insert(
            id,
            PendingCommand {
                surface_id: surface_id.to_string(),
                sender,
            },
        );
        registry
            .queues
            .entry(surface_id.to_string())
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
pub fn take_command(surface_id: &str) -> String {
    let mut registry = registry().lock().unwrap_or_else(|error| error.into_inner());
    loop {
        let command = registry
            .queues
            .get_mut(surface_id)
            .and_then(VecDeque::pop_front);
        let Some(command) = command else {
            return String::new();
        };
        if !registry.pending.contains_key(&command.id) {
            continue;
        }
        return serde_json::to_string(&command).unwrap_or_default();
    }
}

/// Complete one command after the native workspace has reconciled its layout.
pub fn complete_command(id: u64, ok: bool, payload: &str) -> bool {
    let sender = registry()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .pending
        .remove(&id);
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
        let surface = "terminal-automation-registry-test";
        remove_workspace(surface);
        publish_snapshot(surface, r#"{"surfaceId":"terminal-test","tabs":[]}"#).unwrap();
        assert_eq!(snapshot(surface).unwrap()["surfaceId"], "terminal-test");

        let task = tokio::spawn(run_command(
            surface,
            "split",
            serde_json::json!({ "direction": "right" }),
            Duration::from_secs(1),
        ));
        tokio::task::yield_now().await;

        let command: Value = serde_json::from_str(&take_command(surface)).unwrap();
        assert_eq!(command["action"], "split");
        assert_eq!(command["params"]["direction"], "right");
        assert!(complete_command(
            command["id"].as_u64().unwrap(),
            true,
            r#"{"surfaceId":"terminal-test","paneCount":2}"#,
        ));

        let result = task.await.unwrap().unwrap();
        assert_eq!(result["paneCount"], 2);
        remove_workspace(surface);
    }

    #[tokio::test]
    async fn closing_a_surface_fails_a_command_already_taken_by_the_host() {
        let surface = "terminal-automation-close-test";
        remove_workspace(surface);
        publish_snapshot(surface, r#"{"surfaceId":"terminal-close","tabs":[]}"#).unwrap();
        let task = tokio::spawn(run_command(
            surface,
            "split",
            serde_json::json!({ "direction": "down" }),
            Duration::from_secs(1),
        ));
        tokio::task::yield_now().await;
        assert!(!take_command(surface).is_empty());

        remove_workspace(surface);
        let error = task
            .await
            .unwrap()
            .expect_err("closed surface fails command");
        assert!(error.contains("closed before the command completed"));
    }
}
