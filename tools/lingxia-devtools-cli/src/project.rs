use anyhow::{Context, Result, anyhow, bail};
use lingxia_devtool_protocol::{DevtoolsPeerRole, DevtoolsWireMessage, handlers};
use serde::Deserialize;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tungstenite::WebSocket;
use tungstenite::client::IntoClientRequest;
use tungstenite::protocol::Message;

const DEV_DIR_NAME: &str = ".lingxia";
const SESSIONS_DIR_NAME: &str = "sessions";
const WS_PROBE_TIMEOUT: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub pid: u32,
    pub platform: String,
    pub started_at: u64,
    pub ws_url: String,
    pub log_file: String,
}

#[derive(Debug, Default, Clone)]
pub struct SessionSelector {
    /// Session id prefix or platform name. `None` auto-selects when unambiguous.
    pub query: Option<String>,
}

impl SessionSelector {
    pub fn is_empty(&self) -> bool {
        self.query.is_none()
    }
}

pub fn sessions_dir(project_root: &Path) -> PathBuf {
    project_root.join(DEV_DIR_NAME).join(SESSIONS_DIR_NAME)
}

/// Enumerate every parseable session file under `.lingxia/sessions/`.
/// Malformed JSON / unreadable files are silently skipped.
pub fn list_all_sessions(project_root: &Path) -> Result<Vec<SessionInfo>> {
    let dir = sessions_dir(project_root);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else { continue };
        let Ok(info) = serde_json::from_slice::<SessionInfo>(&bytes) else {
            continue;
        };
        out.push(info);
    }
    out.sort_by_key(|s| s.started_at);
    Ok(out)
}

pub fn is_stale(info: &SessionInfo) -> bool {
    !devtools_ws_reachable(&info.ws_url, WS_PROBE_TIMEOUT)
}

/// Delete session files whose devtools WS probe fails. Malformed files are
/// removed silently because they do not contain a printable session id.
/// Returns parsed stale entries so `lxdev sessions prune` can print one line.
pub fn prune_stale(project_root: &Path) -> Result<Vec<SessionInfo>> {
    let dir = sessions_dir(project_root);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut pruned = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            let _ = fs::remove_file(&path);
            continue;
        };
        let Ok(info) = serde_json::from_slice::<SessionInfo>(&bytes) else {
            let _ = fs::remove_file(&path);
            continue;
        };
        if is_stale(&info) {
            let _ = fs::remove_file(&path);
            pruned.push(info);
        }
    }
    Ok(pruned)
}

/// Resolve which session a `lxdev` subcommand should target.
///
/// Avoids probing more than necessary: when only one session file exists,
/// reuse it directly without TCP probes; only escalate to liveness checks
/// when ambiguity actually exists.
pub fn resolve_session(project_root: &Path, selector: &SessionSelector) -> Result<SessionInfo> {
    let all = list_all_sessions(project_root)?;
    if all.is_empty() {
        bail!(
            "No active dev session found. Run `lingxia dev` in this project first.\n\
             (Looked under {})",
            sessions_dir(project_root).display()
        );
    }

    // Fast path: exactly one session file and no explicit selector → use it
    // without probing. If it's actually stale the subsequent WS connect will
    // surface a clearer error than a 200ms probe ever could.
    if selector.is_empty() && all.len() == 1 {
        return Ok(all.into_iter().next().unwrap());
    }

    // Match against the selector: one value, tried as a platform name first
    // (exact, case-insensitive) then as a session-id prefix. Ambiguity in the
    // surviving set is what triggers the explicit error.
    let mut candidates: Vec<SessionInfo> = all
        .into_iter()
        .filter(|s| match &selector.query {
            Some(needle) => {
                s.platform.eq_ignore_ascii_case(needle) || s.session_id.starts_with(needle)
            }
            None => true,
        })
        .collect();

    if candidates.is_empty() {
        bail!(
            "No dev session matches the given selector ({:?}). \
             Run `lxdev sessions` to see active sessions.",
            selector
        );
    }

    // If multiple candidates remain, probe and drop stale ones — a dead
    // session shouldn't force a user to disambiguate.
    if candidates.len() > 1 {
        candidates.retain(|s| !is_stale(s));
    }

    match candidates.len() {
        0 => Err(anyhow!(
            "All matching dev sessions are unreachable. \
             Run `lxdev sessions prune` to clean up, or start a new session."
        )),
        1 => Ok(candidates.into_iter().next().unwrap()),
        _ => {
            let mut msg = String::from(
                "Multiple active dev sessions match. Add --session <id-prefix|platform> to choose:\n",
            );
            for s in &candidates {
                msg.push_str(&format!(
                    "  {}  platform={}  pid={}  ws={}\n",
                    s.session_id, s.platform, s.pid, s.ws_url
                ));
            }
            Err(anyhow!(msg.trim_end().to_string()))
        }
    }
}

fn devtools_ws_reachable(ws_url: &str, timeout: Duration) -> bool {
    let Some(mut websocket) = connect_devtools_ws(ws_url, timeout) else {
        return false;
    };

    if send_wire_message(
        &mut websocket,
        &DevtoolsWireMessage::Hello {
            role: DevtoolsPeerRole::Client,
        },
    )
    .is_err()
    {
        return false;
    }

    let command_id = format!(
        "probe-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    );
    if send_wire_message(
        &mut websocket,
        &DevtoolsWireMessage::Command {
            command_id: command_id.clone(),
            handler: handlers::ECHO.to_string(),
            args: None,
        },
    )
    .is_err()
    {
        return false;
    }

    loop {
        let Ok(message) = websocket.read() else {
            return false;
        };
        let Message::Text(text) = message else {
            continue;
        };
        match serde_json::from_str(&text) {
            Ok(DevtoolsWireMessage::Result {
                command_id: result_id,
                ok,
                ..
            }) if result_id == command_id => return ok,
            Ok(_) => continue,
            Err(_) => return false,
        }
    }
}

fn send_wire_message(
    websocket: &mut WebSocket<impl Read + Write>,
    message: &DevtoolsWireMessage,
) -> Result<()> {
    let text = serde_json::to_string(message).context("Failed to encode dev websocket message")?;
    websocket
        .send(Message::Text(text.into()))
        .context("Failed to send dev websocket message")
}

fn connect_devtools_ws(ws_url: &str, timeout: Duration) -> Option<WebSocket<TcpStream>> {
    let addr = parse_ws_addr(ws_url)?;
    let mut last_error = None;
    for socket_addr in addr.to_socket_addrs().ok()? {
        match TcpStream::connect_timeout(&socket_addr, timeout) {
            Ok(stream) => {
                let _ = stream.set_read_timeout(Some(timeout));
                let _ = stream.set_write_timeout(Some(timeout));
                let request = ws_url.into_client_request().ok()?;
                let (websocket, _) = tungstenite::client::client(request, stream).ok()?;
                return Some(websocket);
            }
            Err(err) => last_error = Some(err),
        }
    }
    let _ = last_error;
    None
}

fn parse_ws_addr(ws_url: &str) -> Option<String> {
    let rest = ws_url.strip_prefix("ws://")?;
    let authority = rest.split('/').next().filter(|value| !value.is_empty())?;
    if authority.starts_with('[') {
        return Some(authority.to_string());
    }
    if authority.rsplit_once(':').is_some() {
        Some(authority.to_string())
    } else {
        Some(format!("{authority}:80"))
    }
}
