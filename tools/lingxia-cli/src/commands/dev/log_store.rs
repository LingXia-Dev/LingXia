use anyhow::{Context, Result, anyhow};
use lingxia_devtool_protocol::broker::Registration;
pub use lingxia_devtool_protocol::broker::SessionInfo;
use lingxia_devtool_protocol::{DevtoolsPeerRole, DevtoolsWireMessage, handlers};
use lingxia_log::now_timestamp_ms;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tungstenite::WebSocket;
use tungstenite::client::IntoClientRequest;
use tungstenite::protocol::Message;
use uuid::Uuid;

pub const DEFAULT_LOG_RETENTION_DAYS: u64 = 7;
pub const DEV_DIR_NAME: &str = ".lingxia";
const WS_PROBE_TIMEOUT: Duration = Duration::from_millis(200);
/// Read/write budget for an interactive command round trip (e.g. shutdown).
/// The 200ms probe timeout is only meant for liveness checks; reusing it for a
/// full hello+command+result exchange makes graceful stop fail on a busy host.
const WS_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct DevLogSession {
    pub session_id: String,
    pub log_file: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevSessionState {
    Ready,
    Starting,
    Stale,
}

impl DevSessionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Starting => "starting",
            Self::Stale => "stale",
        }
    }
}

pub fn dev_dir(project_root: &Path) -> PathBuf {
    project_root.join(DEV_DIR_NAME)
}

pub fn create_session(project_root: &Path) -> Result<DevLogSession> {
    let dev_dir = dev_dir(project_root);
    let logs_dir = dev_dir.join("logs");
    cleanup_old_logs(&logs_dir, DEFAULT_LOG_RETENTION_DAYS)?;
    fs::create_dir_all(&logs_dir)
        .with_context(|| format!("Failed to create {}", logs_dir.display()))?;

    // Short ids: the broker keeps the per-user live-session set small, and
    // `lxdev --session` accepts prefixes, so 6 hex chars are plenty.
    let session_id = Uuid::new_v4().simple().to_string()[..6].to_string();
    Ok(DevLogSession {
        session_id: session_id.clone(),
        log_file: logs_dir.join(format!("{session_id}.jsonl")),
    })
}

/// Canonical project identity used in broker records: sessions register it,
/// project-scoped queries (`lingxia dev status/stop`, duplicate guard) filter
/// by it.
pub fn canonical_project_root(project_root: &Path) -> String {
    let canonical = fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    strip_verbatim_prefix(&canonical.display().to_string())
}

/// On Windows `fs::canonicalize` returns extended-length paths (`\\?\C:\…`,
/// `\\?\UNC\server\share\…`). The verbatim prefix is noise in an identity
/// that is also shown to users (`lxdev session list`'s PROJECT column) — strip
/// it back to the conventional form. Both sides of every comparison come from
/// this function, so matching stays consistent.
fn strip_verbatim_prefix(display: &str) -> String {
    if cfg!(windows) {
        if let Some(rest) = display.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{rest}");
        }
        if let Some(rest) = display.strip_prefix(r"\\?\") {
            return rest.to_string();
        }
    }
    display.to_string()
}

/// Spawn a detached per-user broker (`lingxia dev-broker`). Losing the bind
/// race to a concurrent spawn is fine — the loser exits and the caller
/// connects to the winner.
pub fn spawn_broker() -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let mut command = std::process::Command::new(exe);
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
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

/// Register this dev session with the per-user broker. The returned guard
/// keeps the registration alive (re-registering across broker restarts);
/// dropping it — or process exit — removes the session.
pub fn register_session(
    project_root: &Path,
    session: &DevLogSession,
    target: &str,
    ws_url: &str,
) -> Registration {
    register_session_with_content(
        project_root,
        session,
        target,
        ws_url,
        lingxia_devtool_protocol::broker::SessionContent::Host {
            path: canonical_project_root(project_root),
        },
    )
}

pub fn register_session_with_content(
    context_root: &Path,
    session: &DevLogSession,
    target: &str,
    ws_url: &str,
    content: lingxia_devtool_protocol::broker::SessionContent,
) -> Registration {
    let info = SessionInfo {
        session_id: session.session_id.clone(),
        project_root: canonical_project_root(context_root),
        content: Some(content),
        target: target.to_string(),
        pid: std::process::id(),
        started_at: now_timestamp_ms(),
        executable: std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        ws_url: ws_url.to_string(),
        log_file: session.log_file.display().to_string(),
    };
    lingxia_devtool_protocol::broker::register_session(info, spawn_broker)
}

/// Live sessions for this project, ordered by start time.
pub fn list_sessions(project_root: &Path) -> Result<Vec<SessionInfo>> {
    let root = canonical_project_root(project_root);
    let mut sessions: Vec<SessionInfo> =
        lingxia_devtool_protocol::broker::list_sessions_spawning(&spawn_broker)
            .context("Failed to query the dev-session broker")?
            .into_iter()
            .filter(|s| s.project_root == root)
            .collect();
    sessions.sort_by_key(|s| s.started_at);
    Ok(sessions)
}

pub fn session_state(info: &SessionInfo) -> DevSessionState {
    session_state_from_echo(devtools_ws_echo(&info.ws_url, WS_PROBE_TIMEOUT))
}

fn session_state_from_echo(echo: Option<(bool, Option<serde_json::Value>)>) -> DevSessionState {
    let Some((true, data)) = echo else {
        return DevSessionState::Stale;
    };
    if data
        .as_ref()
        .and_then(|value| value.get("runtimeConnected"))
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        DevSessionState::Ready
    } else {
        DevSessionState::Starting
    }
}

/// Live sessions for a given target in this project. Used by `lingxia dev` to
/// detect "another session is already running" before launching.
pub fn find_live_for_target(project_root: &Path, target: &str) -> Result<Vec<SessionInfo>> {
    Ok(list_sessions(project_root)?
        .into_iter()
        .filter(|s| s.target.eq_ignore_ascii_case(target))
        .collect())
}

pub fn resolve_session(project_root: &Path, selector: Option<&str>) -> Result<SessionInfo> {
    let all = list_sessions(project_root)?;
    if all.is_empty() {
        return Err(anyhow!(
            "No live dev session found for this project. Run `lingxia dev` first."
        ));
    }

    let mut candidates: Vec<SessionInfo> = all
        .into_iter()
        .filter(|session| match selector {
            Some(value) => {
                session.target.eq_ignore_ascii_case(value) || session.session_id.starts_with(value)
            }
            None => true,
        })
        .collect();

    if candidates.is_empty() {
        return Err(anyhow!(
            "No dev session matches the given selector ({:?}).",
            selector
        ));
    }

    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        _ => {
            let mut msg = String::from(
                "Multiple live dev sessions match. Pass a session id prefix or target:\n",
            );
            for session in &candidates {
                msg.push_str(&format!(
                    "  {}  target={}  pid={}  ws={}\n",
                    session.session_id, session.target, session.pid, session.ws_url
                ));
            }
            Err(anyhow!(msg.trim_end().to_string()))
        }
    }
}

pub fn request_shutdown(info: &SessionInfo) -> Result<()> {
    let mut websocket = connect_devtools_ws(&info.ws_url, WS_COMMAND_TIMEOUT)
        .ok_or_else(|| anyhow!("Failed to connect dev websocket: {}", info.ws_url))?;
    send_wire_message(
        &mut websocket,
        &DevtoolsWireMessage::Hello {
            role: DevtoolsPeerRole::Client,
            token: lingxia_devtool_protocol::token_from_ws_url(&info.ws_url),
        },
    )?;

    let command_id = format!("shutdown-{}", now_timestamp_ms());
    send_wire_message(
        &mut websocket,
        &DevtoolsWireMessage::Command {
            command_id: command_id.clone(),
            handler: handlers::session::SHUTDOWN.to_string(),
            args: None,
        },
    )?;

    loop {
        let message = websocket
            .read()
            .context("Failed to read dev websocket shutdown response")?;
        let Message::Text(text) = message else {
            continue;
        };
        match serde_json::from_str(&text) {
            Ok(DevtoolsWireMessage::Result {
                command_id: result_id,
                ok,
                error,
                ..
            }) if result_id == command_id => {
                if ok {
                    return Ok(());
                }
                return Err(anyhow!(
                    "{}",
                    error.unwrap_or_else(|| "shutdown command failed".to_string())
                ));
            }
            Ok(_) => continue,
            Err(err) => return Err(err).context("Failed to parse dev websocket shutdown response"),
        }
    }
}

fn devtools_ws_echo(ws_url: &str, timeout: Duration) -> Option<(bool, Option<serde_json::Value>)> {
    let mut websocket = connect_devtools_ws(ws_url, timeout)?;

    if send_wire_message(
        &mut websocket,
        &DevtoolsWireMessage::Hello {
            role: DevtoolsPeerRole::Client,
            token: lingxia_devtool_protocol::token_from_ws_url(ws_url),
        },
    )
    .is_err()
    {
        return None;
    }

    let command_id = format!("probe-{}", now_timestamp_ms());
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
        return None;
    }

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
            }) if result_id == command_id => return Some((ok, data)),
            Ok(_) => continue,
            Err(_) => return None,
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

pub fn cleanup_old_logs(logs_dir: &Path, retention_days: u64) -> Result<()> {
    if retention_days == 0 || !logs_dir.exists() {
        return Ok(());
    }

    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(retention_days.saturating_mul(86_400)))
        .ok_or_else(|| anyhow!("Failed to compute log retention cutoff"))?;
    for entry in
        fs::read_dir(logs_dir).with_context(|| format!("Failed to read {}", logs_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if modified < cutoff && metadata.is_file() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_project_local_dev_paths() {
        let temp = tempdir().unwrap();
        let session = create_session(temp.path()).unwrap();
        assert!(
            session
                .log_file
                .starts_with(temp.path().join(".lingxia").join("logs"))
        );
        assert_eq!(session.session_id.len(), 6);
    }

    #[test]
    fn cleanup_old_logs_removes_expired_entries_only() {
        let temp = tempdir().unwrap();
        let logs_dir = temp.path().join("logs");
        fs::create_dir_all(&logs_dir).unwrap();

        let old_log = logs_dir.join("old.jsonl");
        let new_log = logs_dir.join("new.jsonl");
        fs::write(&old_log, "old").unwrap();
        fs::write(&new_log, "new").unwrap();

        filetime::set_file_mtime(
            &old_log,
            filetime::FileTime::from_system_time(
                SystemTime::now() - Duration::from_secs(10 * 86_400),
            ),
        )
        .unwrap();

        cleanup_old_logs(&logs_dir, 7).unwrap();

        assert!(!old_log.exists());
        assert!(new_log.exists());
    }

    #[test]
    fn session_state_distinguishes_server_from_runtime_readiness() {
        assert_eq!(session_state_from_echo(None), DevSessionState::Stale);
        assert_eq!(
            session_state_from_echo(Some((
                true,
                Some(serde_json::json!({
                    "runtimeConnected": false
                }))
            ))),
            DevSessionState::Starting
        );
        assert_eq!(
            session_state_from_echo(Some((
                true,
                Some(serde_json::json!({
                    "runtimeConnected": true
                }))
            ))),
            DevSessionState::Ready
        );
    }
}
