//! Attached remote dev sessions (`lxdev attach`): named ws URLs persisted in
//! `~/.lingxia/lxdev-remotes.json` so a LAN session pairs once and then
//! behaves like any local session in listing and `--session` selection.

use crate::project::SessionInfo;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSession {
    pub name: String,
    pub ws_url: String,
}

/// One-off `--ws <url>` target: no pairing, identity comes from the URL only.
/// An empty `log_file` routes `logs` through the dev server.
pub fn direct_session_info(ws_url: &str) -> SessionInfo {
    SessionInfo {
        session_id: "-".to_string(),
        project_root: String::new(),
        target: "ws".to_string(),
        pid: 0,
        started_at: 0,
        ws_url: ws_url.to_string(),
        log_file: String::new(),
        remote_name: Some("ws".to_string()),
    }
}

fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("Cannot determine home directory (HOME/USERPROFILE unset)"))
}

fn store_path() -> Result<PathBuf> {
    Ok(home_dir()?.join(".lingxia").join("lxdev-remotes.json"))
}

pub fn list_remotes() -> Result<Vec<RemoteSession>> {
    let path = store_path()?;
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    serde_json::from_str(&raw)
        .with_context(|| format!("Corrupt remote-session store: {}", path.display()))
}

fn save_remotes(remotes: &[RemoteSession]) -> Result<()> {
    let path = store_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create ~/.lingxia")?;
    }
    let raw = serde_json::to_string_pretty(remotes).context("Failed to encode remote sessions")?;
    std::fs::write(&path, raw).context("Failed to write the remote-session store")
}

/// Derive a default attach name from the URL host.
fn default_name(ws_url: &str) -> Option<String> {
    let rest = ws_url.strip_prefix("ws://")?;
    let authority = rest.split(['/', '?']).next()?;
    let host = authority
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or(authority);
    (!host.is_empty()).then(|| host.to_string())
}

pub fn attach(ws_url: &str, name: Option<String>) -> Result<()> {
    if !ws_url.starts_with("ws://") {
        bail!("Expected a ws:// URL (from `lingxia dev --lan` on the remote machine)");
    }
    let name = match name.or_else(|| default_name(ws_url)) {
        Some(name) => name,
        None => bail!("Could not derive a name from the URL; pass --name"),
    };
    let candidate = RemoteSession {
        name: name.clone(),
        ws_url: ws_url.to_string(),
    };
    let info = crate::project::remote_session_info(&candidate);
    if !crate::project::remote_is_reachable(&info) {
        bail!(
            "No dev session answered at {ws_url}.\n\
             Check that `lingxia dev --lan` is running on the remote machine,\n\
             the URL includes its ?token=, and the firewall allows the port."
        );
    }
    let mut remotes = list_remotes()?;
    remotes.retain(|remote| remote.name != name);
    remotes.push(candidate);
    save_remotes(&remotes)?;
    let state = crate::project::session_state(&info);
    println!(
        "Attached {name:?}: {} session {} ({}, state={}). Use it with: lxdev --session {name} …",
        info.target,
        info.session_id,
        info.project_root,
        state.as_str()
    );
    Ok(())
}

pub fn detach(name: &str) -> Result<()> {
    let mut remotes = list_remotes()?;
    let before = remotes.len();
    remotes.retain(|remote| remote.name != name);
    if remotes.len() == before {
        bail!("No attached remote session named {name:?}");
    }
    save_remotes(&remotes)?;
    println!("Detached remote session {name:?}.");
    Ok(())
}

pub fn detach_unreachable() -> Result<()> {
    let remotes = list_remotes()?;
    let (reachable, unreachable) = partition_unreachable(remotes, |remote| {
        let info = crate::project::remote_session_info(remote);
        crate::project::remote_is_reachable(&info)
    });
    if unreachable.is_empty() {
        println!("No unreachable remote sessions to detach.");
        return Ok(());
    }

    save_remotes(&reachable)?;
    let names = unreachable
        .iter()
        .map(|remote| format!("{:?}", remote.name))
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "Detached {} unreachable remote session{}: {names}.",
        unreachable.len(),
        if unreachable.len() == 1 { "" } else { "s" }
    );
    Ok(())
}

fn partition_unreachable(
    remotes: Vec<RemoteSession>,
    mut is_reachable: impl FnMut(&RemoteSession) -> bool,
) -> (Vec<RemoteSession>, Vec<RemoteSession>) {
    remotes.into_iter().partition(|remote| is_reachable(remote))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(name: &str) -> RemoteSession {
        RemoteSession {
            name: name.to_string(),
            ws_url: format!("ws://{name}:39000/?token=test"),
        }
    }

    #[test]
    fn unreachable_partition_preserves_reachable_order() {
        let remotes = vec![remote("win"), remote("offline"), remote("lab")];
        let (reachable, unreachable) =
            partition_unreachable(remotes, |remote| remote.name != "offline");

        assert_eq!(
            reachable
                .iter()
                .map(|remote| remote.name.as_str())
                .collect::<Vec<_>>(),
            ["win", "lab"]
        );
        assert_eq!(unreachable[0].name, "offline");
    }
}
