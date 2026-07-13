use anyhow::{Context, Result, bail};
pub use lingxia_devtool_protocol::broker::SessionInfo;
use lingxia_devtool_protocol::{DevtoolsPeerRole, DevtoolsWireMessage, handlers};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use tungstenite::WebSocket;
use tungstenite::client::IntoClientRequest;
use tungstenite::protocol::Message;

const WS_PROBE_TIMEOUT: Duration = Duration::from_millis(200);

#[derive(Debug, Default, Clone)]
pub struct SessionSelector {
    /// Listing ordinal, session id prefix, or target name.
    /// `None` auto-selects when exactly one session is live.
    pub query: Option<String>,
}

/// Spawn the per-user broker so sessions orphaned by a broker crash can
/// re-register and become visible again. `lxdev` never starts `lingxia dev`
/// itself — only the broker. Missing `lingxia` binary is fine: with no broker
/// there are no registered sessions either.
fn spawn_broker() -> std::io::Result<()> {
    let lingxia_bin = std::env::var("LINGXIA_BIN").unwrap_or_else(|_| "lingxia".to_string());
    let mut command = std::process::Command::new(lingxia_bin);
    command
        .arg("dev-broker")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    detach_process(&mut command);
    command.spawn().map(|_| ())
}

#[cfg(unix)]
fn detach_process(command: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(windows)]
fn detach_process(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    // DETACHED_PROCESS can still flash a console window when the console
    // subsystem `lingxia.exe` is launched from Windows Terminal/Explorer.
    // CREATE_NO_WINDOW keeps the broker fully headless while preserving its
    // independent process group.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

/// All live sessions for this user, ordered by start time.
pub fn list_all_sessions() -> Result<Vec<SessionInfo>> {
    let mut sessions = lingxia_devtool_protocol::broker::list_sessions_spawning(&spawn_broker)
        .context("Failed to query the dev-session broker")?;
    sessions.sort_by_key(|s| s.started_at);
    Ok(sessions)
}

/// Local broker sessions plus attached remote sessions (`lxdev attach`),
/// remotes last. This is the selection universe for `--session`.
pub fn list_selectable_sessions() -> Result<Vec<SessionInfo>> {
    let mut sessions = list_all_sessions()?;
    for remote in crate::remotes::list_remotes()? {
        sessions.push(remote_session_info(&remote));
    }
    Ok(sessions)
}

/// Live identity of an attached remote, fetched from its dev server so the
/// entry lists with the same fields as a local session. Unreachable remotes
/// keep placeholder identity (`session_id == "-"`) under their attach name.
pub fn remote_session_info(remote: &crate::remotes::RemoteSession) -> SessionInfo {
    let mut info = SessionInfo {
        session_id: "-".to_string(),
        project_root: String::new(),
        target: remote.name.clone(),
        pid: 0,
        started_at: 0,
        ws_url: remote.ws_url.clone(),
        log_file: String::new(),
        remote_name: Some(remote.name.clone()),
    };
    if let Some(data) = fetch_session_info(&remote.ws_url, Duration::from_secs(2)) {
        if let Some(id) = data.get("session_id").and_then(serde_json::Value::as_str) {
            info.session_id = id.to_string();
        }
        if let Some(target) = data.get("target").and_then(serde_json::Value::as_str) {
            info.target = target.to_string();
        }
        if let Some(root) = data.get("project_root").and_then(serde_json::Value::as_str) {
            info.project_root = root.to_string();
        }
        info.started_at = data
            .get("started_at")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default();
        info.pid = data
            .get("pid")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default() as u32;
    }
    info
}

/// Whether an attached remote entry answered `session.info` when listed.
pub fn remote_is_reachable(info: &SessionInfo) -> bool {
    info.remote_name.is_none() || info.session_id != "-"
}

fn fetch_session_info(ws_url: &str, timeout: Duration) -> Option<serde_json::Value> {
    let mut websocket = connect_devtools_ws(ws_url, timeout)?;
    send_wire_message(
        &mut websocket,
        &DevtoolsWireMessage::Hello {
            role: DevtoolsPeerRole::Client,
            token: lingxia_devtool_protocol::token_from_ws_url(ws_url),
        },
    )
    .ok()?;
    let command_id = format!(
        "info-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    );
    send_wire_message(
        &mut websocket,
        &DevtoolsWireMessage::Command {
            command_id: command_id.clone(),
            handler: handlers::session::INFO.to_string(),
            args: None,
        },
    )
    .ok()?;
    loop {
        let Ok(message) = websocket.read() else {
            return None;
        };
        let Message::Text(text) = message else {
            continue;
        };
        match serde_json::from_str(&text) {
            Ok(DevtoolsWireMessage::Result {
                command_id: result_id,
                ok,
                data,
                ..
            }) if result_id == command_id => return if ok { data } else { None },
            Ok(_) => continue,
            Err(_) => return None,
        }
    }
}

/// Resolve which session a `lxdev` subcommand should target.
///
/// The common case is one live session and no selector: use it. With several
/// sessions, `--session` (or `LXDEV_SESSION`) picks one by session id prefix
/// or target name; anything ambiguous or unmatched is an error, never a
/// guess.
pub fn resolve_session(selector: &SessionSelector) -> Result<SessionInfo> {
    let all = list_selectable_sessions()?;
    if all.is_empty() {
        bail!(
            "No live dev session found. Run `lingxia dev` first, or attach a \
             remote one with `lxdev attach <ws-url>`."
        );
    }

    let Some(needle) = selector.query.as_deref() else {
        if all.len() == 1 {
            return Ok(all.into_iter().next().unwrap());
        }
        // Attached remotes persist across the other machine's restarts, so an
        // unreachable one must not block auto-selection — it stays selectable
        // by name, but only live sessions count as candidates here.
        let mut live: Vec<SessionInfo> = all
            .iter()
            .filter(|s| remote_is_reachable(s))
            .cloned()
            .collect();
        if live.len() == 1 {
            return Ok(live.remove(0));
        }
        bail!(pick_message(if live.is_empty() { &all } else { &live }));
    };

    let candidates: Vec<SessionInfo> = all
        .iter()
        .filter(|s| {
            s.target.eq_ignore_ascii_case(needle)
                || s.session_id.starts_with(needle)
                || s.remote_name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(needle))
        })
        .cloned()
        .collect();

    match candidates.len() {
        0 => bail!(
            "No dev session matches --session {needle:?}.\n{}",
            pick_message(&all)
        ),
        1 => Ok(candidates.into_iter().next().unwrap()),
        _ => bail!(
            "--session {needle:?} is ambiguous.\n{}",
            pick_message(&candidates)
        ),
    }
}

/// The disambiguation listing: session id, target, project path.
fn pick_message(sessions: &[SessionInfo]) -> String {
    let mut msg =
        String::from("Multiple LingXia dev sessions are live. Pick one with --session:\n\n");
    for s in sessions {
        let location = match s.remote_name.as_deref() {
            Some(name) => format!("(remote {name}) {}", s.ws_url),
            None => abbreviate_home(&s.project_root),
        };
        msg.push_str(&format!(
            "  {}  {:<8} {}\n",
            s.session_id, s.target, location
        ));
    }
    msg.trim_end().to_string()
}

fn abbreviate_home(path: &str) -> String {
    let Some(home) = std::env::var_os("HOME") else {
        return path.to_string();
    };
    let home = home.to_string_lossy();
    match path.strip_prefix(home.as_ref()) {
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_string(),
    }
}

pub fn is_stale(info: &SessionInfo) -> bool {
    !devtools_ws_reachable(&info.ws_url, WS_PROBE_TIMEOUT)
}

fn devtools_ws_reachable(ws_url: &str, timeout: Duration) -> bool {
    let Some(mut websocket) = connect_devtools_ws(ws_url, timeout) else {
        return false;
    };

    if send_wire_message(
        &mut websocket,
        &DevtoolsWireMessage::Hello {
            role: DevtoolsPeerRole::Client,
            token: lingxia_devtool_protocol::token_from_ws_url(ws_url),
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
    let authority = rest
        .split(['/', '?'])
        .next()
        .filter(|value| !value.is_empty())?;
    if authority.starts_with('[') {
        return Some(authority.to_string());
    }
    if authority.rsplit_once(':').is_some() {
        Some(authority.to_string())
    } else {
        Some(format!("{authority}:80"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str, target: &str, started_at: u64) -> SessionInfo {
        SessionInfo {
            session_id: id.to_string(),
            project_root: "/p".to_string(),
            target: target.to_string(),
            pid: 1,
            started_at,
            ws_url: "ws://127.0.0.1:1".to_string(),
            log_file: "/p/.lingxia/logs/x.jsonl".to_string(),
            remote_name: None,
        }
    }

    #[test]
    fn pick_message_lists_id_target_path() {
        let msg = pick_message(&[session("a1b2c3", "macos", 1), session("d4e5f6", "lxapp", 2)]);
        assert!(msg.contains("a1b2c3  macos"));
        assert!(msg.contains("d4e5f6  lxapp"));
    }
}
