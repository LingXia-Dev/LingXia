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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Ready,
    Starting,
    Stale,
}

impl SessionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Starting => "starting",
            Self::Stale => "stale",
        }
    }
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

/// Resolve which session a `lxdev` subcommand should target.
///
/// The common case is one live session and no selector: use it. With several
/// sessions, `--session` (or `LXDEV_SESSION`) picks one by session id prefix
/// or target name; anything ambiguous or unmatched is an error, never a
/// guess.
pub fn resolve_session(selector: &SessionSelector) -> Result<SessionInfo> {
    let all = list_all_sessions()?;
    if all.is_empty() {
        bail!("No live dev session found. Run `lingxia dev` first.");
    }

    let Some(needle) = selector.query.as_deref() else {
        if all.len() == 1 {
            return Ok(all.into_iter().next().unwrap());
        }
        bail!(pick_message(&all));
    };

    let candidates: Vec<SessionInfo> = all
        .iter()
        .filter(|s| s.target.eq_ignore_ascii_case(needle) || s.session_id.starts_with(needle))
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

/// The disambiguation listing: session id, container target, mounted content.
fn pick_message(sessions: &[SessionInfo]) -> String {
    let mut msg =
        String::from("Multiple LingXia dev sessions are live. Pick one with --session:\n\n");
    for s in sessions {
        let location = abbreviate_home(
            s.content
                .as_ref()
                .map(|content| content.display())
                .unwrap_or(&s.project_root),
        );
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

pub fn session_state(info: &SessionInfo) -> SessionState {
    devtools_session_state(&info.ws_url, WS_PROBE_TIMEOUT)
}

fn devtools_session_state(ws_url: &str, timeout: Duration) -> SessionState {
    let Some(mut websocket) = connect_devtools_ws(ws_url, timeout) else {
        return SessionState::Stale;
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
        return SessionState::Stale;
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
        return SessionState::Stale;
    }

    loop {
        let Ok(message) = websocket.read() else {
            return SessionState::Stale;
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
            }) if result_id == command_id => return session_state_from_echo_result(ok, data),
            Ok(_) => continue,
            Err(_) => return SessionState::Stale,
        }
    }
}

fn session_state_from_echo_result(ok: bool, data: Option<serde_json::Value>) -> SessionState {
    if !ok {
        return SessionState::Stale;
    }
    if data
        .as_ref()
        .and_then(|value| value.get("runtimeConnected"))
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        SessionState::Ready
    } else {
        SessionState::Starting
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
            content: None,
            target: target.to_string(),
            pid: 1,
            started_at,
            executable: "/usr/local/bin/lingxia".to_string(),
            ws_url: "ws://127.0.0.1:1".to_string(),
            log_file: "/p/.lingxia/logs/x.jsonl".to_string(),
        }
    }

    #[test]
    fn pick_message_lists_id_target_path() {
        let msg = pick_message(&[session("a1b2c3", "macos", 1), session("d4e5f6", "lxapp", 2)]);
        assert!(msg.contains("a1b2c3  macos"));
        assert!(msg.contains("d4e5f6  lxapp"));
    }

    #[test]
    fn session_state_distinguishes_server_from_runtime_readiness() {
        assert_eq!(
            session_state_from_echo_result(false, None),
            SessionState::Stale
        );
        assert_eq!(
            session_state_from_echo_result(
                true,
                Some(serde_json::json!({ "runtimeConnected": false }))
            ),
            SessionState::Starting
        );
        assert_eq!(
            session_state_from_echo_result(
                true,
                Some(serde_json::json!({ "runtimeConnected": true }))
            ),
            SessionState::Ready
        );
    }
}
