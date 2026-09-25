use anyhow::{Context, Result};
pub use lingxia_control_protocol::dev_session::broker::SessionInfo;
use lingxia_control_protocol::{
    ControlRequest,
    dev_session::{DEV_SESSION_PROTOCOL_VERSION, DevSessionMessage, DevSessionRole, capabilities},
    methods,
};
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
    let mut sessions =
        lingxia_control_protocol::dev_session::broker::list_sessions_spawning(&spawn_broker)
            .context("Failed to query the dev-session broker")?;
    sessions.sort_by_key(|s| s.started_at);
    Ok(sessions)
}

/// Resolve which session a `lxdev` subcommand should target: see
/// [`lingxia_control_protocol::dev_session::select`] for the rules — an
/// explicit selector (name, target, `target@dir`, ordinal, id prefix), else
/// the session of this directory's project, else the only session; anything
/// else is refused with a table of candidates.
pub fn resolve_session(selector: &SessionSelector) -> Result<SessionInfo> {
    let all = list_all_sessions()?;
    let cwd = std::env::current_dir().unwrap_or_default();
    lingxia_control_protocol::dev_session::select::select(&all, selector.query.as_deref(), &cwd)
        .cloned()
        .map_err(|err| anyhow::anyhow!("{err}"))
}

/// What to pass as `--session` in a printed hint to reach `info` again from
/// this directory; `None` when no selector is needed.
pub fn hint_selector(info: &SessionInfo) -> Option<String> {
    let all = list_all_sessions().ok()?;
    let cwd = std::env::current_dir().ok()?;
    lingxia_control_protocol::dev_session::select::hint_selector(&all, info, &cwd)
}

/// A session's readiness and the build its runtime reported.
#[derive(Debug, Clone)]
pub struct SessionProbe {
    pub state: SessionState,
    pub runtime_build: Option<lingxia_control_protocol::dev_session::PeerBuild>,
}

pub fn probe_session(info: &SessionInfo) -> SessionProbe {
    devtools_session_state(&info.ws_url, WS_PROBE_TIMEOUT)
}

fn devtools_session_state(ws_url: &str, timeout: Duration) -> SessionProbe {
    let stale = SessionProbe {
        state: SessionState::Stale,
        runtime_build: None,
    };
    let Some(mut websocket) = connect_devtools_ws(ws_url, timeout) else {
        return stale;
    };

    if send_wire_message(
        &mut websocket,
        &DevSessionMessage::Hello {
            version: DEV_SESSION_PROTOCOL_VERSION,
            role: DevSessionRole::Controller,
            capabilities: vec![capabilities::REQUESTS.to_string()],
            build: None,
        },
    )
    .is_err()
    {
        return stale;
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
        &DevSessionMessage::Request(ControlRequest {
            id: command_id.clone(),
            method: methods::ECHO.to_string(),
            params: None,
        }),
    )
    .is_err()
    {
        return stale;
    }

    loop {
        let Ok(message) = websocket.read() else {
            return stale;
        };
        let Message::Text(text) = message else {
            continue;
        };
        match serde_json::from_str(&text) {
            Ok(DevSessionMessage::Response(response)) if response.id == command_id => {
                let runtime_build = response
                    .result
                    .as_ref()
                    .and_then(|value| value.get("runtimeBuild"))
                    .and_then(|value| serde_json::from_value(value.clone()).ok());
                return SessionProbe {
                    state: session_state_from_echo_result(
                        response.error.is_none(),
                        response.result,
                    ),
                    runtime_build,
                };
            }
            Ok(_) => continue,
            Err(_) => return stale,
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
    message: &DevSessionMessage,
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
