use anyhow::{Context, Result, anyhow};
use lingxia_devtool_protocol::{DevtoolsPeerRole, DevtoolsWireMessage, handlers};
use lingxia_log::now_timestamp_ms;
use serde::{Deserialize, Serialize};
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
pub const SESSIONS_DIR_NAME: &str = "sessions";
pub const SESSION_INFO_VERSION: u32 = 2;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub version: u32,
    pub session_id: String,
    pub pid: u32,
    pub platform: String,
    pub started_at: u64,
    pub ws_url: String,
    pub log_file: String,
}

pub fn dev_dir(project_root: &Path) -> PathBuf {
    project_root.join(DEV_DIR_NAME)
}

pub fn sessions_dir(project_root: &Path) -> PathBuf {
    dev_dir(project_root).join(SESSIONS_DIR_NAME)
}

pub fn session_file_path(project_root: &Path, session_id: &str) -> PathBuf {
    sessions_dir(project_root).join(format!("{session_id}.json"))
}

pub fn create_session(project_root: &Path) -> Result<DevLogSession> {
    let dev_dir = dev_dir(project_root);
    let logs_dir = dev_dir.join("logs");
    cleanup_old_logs(&logs_dir, DEFAULT_LOG_RETENTION_DAYS)?;
    fs::create_dir_all(&logs_dir)
        .with_context(|| format!("Failed to create {}", logs_dir.display()))?;

    let session_id = format!("{}-{}", now_timestamp_ms(), Uuid::new_v4().simple());
    Ok(DevLogSession {
        session_id: session_id.clone(),
        log_file: logs_dir.join(format!("{session_id}.jsonl")),
    })
}

/// Atomically write the session metadata to `.lingxia/sessions/<id>.json`.
/// Uses tmp + rename so concurrent `lxdev` readers never see a partial file.
pub fn write_session(
    project_root: &Path,
    session: &DevLogSession,
    platform: &str,
    ws_url: &str,
) -> Result<()> {
    let dir = sessions_dir(project_root);
    fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    let info = SessionInfo {
        version: SESSION_INFO_VERSION,
        session_id: session.session_id.clone(),
        pid: std::process::id(),
        platform: platform.to_string(),
        started_at: now_timestamp_ms(),
        ws_url: ws_url.to_string(),
        log_file: session.log_file.display().to_string(),
    };
    let bytes = serde_json::to_vec_pretty(&info).context("Failed to encode session info")?;
    let final_path = session_file_path(project_root, &session.session_id);
    let tmp_path = final_path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp_path)
            .with_context(|| format!("Failed to create {}", tmp_path.display()))?;
        file.write_all(&bytes)
            .with_context(|| format!("Failed to write {}", tmp_path.display()))?;
        file.sync_all().ok();
    }
    fs::rename(&tmp_path, &final_path).with_context(|| {
        format!(
            "Failed to rename {} -> {}",
            tmp_path.display(),
            final_path.display()
        )
    })?;
    Ok(())
}

/// Remove this session's metadata file. Logs are kept under retention policy.
pub fn remove_session(project_root: &Path, session_id: &str) -> Result<()> {
    let path = session_file_path(project_root, session_id);
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("Failed to remove {}", path.display()))?;
    }
    Ok(())
}

/// Enumerate all parseable session metadata files under `.lingxia/sessions/`.
/// Malformed/unreadable files are skipped silently.
pub fn list_sessions(project_root: &Path) -> Result<Vec<SessionInfo>> {
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

/// Probe the devtools WS endpoint to see if the session is still reachable.
/// PID is stored only for diagnostics — protocol reachability is the truth.
pub fn is_stale(info: &SessionInfo) -> bool {
    !devtools_ws_reachable(&info.ws_url, WS_PROBE_TIMEOUT)
}

/// Remove session files that fail the devtools WS probe. Malformed files are
/// removed silently because they do not contain a printable session id.
/// Returns parsed stale entries so callers can surface a one-line warning.
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

/// Convenience: return the live session for a given platform, if exactly one
/// exists. Used by `lingxia dev` to detect "another session is already running
/// for this platform" before launching.
pub fn find_live_for_platform(project_root: &Path, platform: &str) -> Result<Vec<SessionInfo>> {
    let sessions = list_sessions(project_root)?;
    Ok(sessions
        .into_iter()
        .filter(|s| s.platform.eq_ignore_ascii_case(platform) && !is_stale(s))
        .collect())
}

pub fn resolve_session(project_root: &Path, selector: Option<&str>) -> Result<SessionInfo> {
    let all = list_sessions(project_root)?;
    if all.is_empty() {
        return Err(anyhow!(
            "No active dev session found. Run `lingxia dev` in this project first.\n\
             (Looked under {})",
            sessions_dir(project_root).display()
        ));
    }

    let mut candidates: Vec<SessionInfo> = all
        .into_iter()
        .filter(|session| match selector {
            Some(value) => {
                session.platform.eq_ignore_ascii_case(value)
                    || session.session_id.starts_with(value)
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

    if candidates.len() > 1 {
        candidates.retain(|session| !is_stale(session));
    }

    match candidates.len() {
        0 => Err(anyhow!(
            "All matching dev sessions are unreachable. Run `lingxia dev status` to inspect them."
        )),
        1 => Ok(candidates.remove(0)),
        _ => {
            let mut msg = String::from(
                "Multiple active dev sessions match. Pass a session id prefix or platform:\n",
            );
            for session in &candidates {
                msg.push_str(&format!(
                    "  {}  platform={}  pid={}  ws={}\n",
                    session.session_id, session.platform, session.pid, session.ws_url
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
    }

    #[test]
    fn write_and_list_session_roundtrip() {
        let temp = tempdir().unwrap();
        let session = create_session(temp.path()).unwrap();
        write_session(temp.path(), &session, "macos", "ws://127.0.0.1:65535").unwrap();
        let sessions = list_sessions(temp.path()).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, session.session_id);
        assert_eq!(sessions[0].platform, "macos");
        assert_eq!(sessions[0].ws_url, "ws://127.0.0.1:65535");
        assert_eq!(sessions[0].version, SESSION_INFO_VERSION);
        assert_eq!(sessions[0].pid, std::process::id());
    }

    #[test]
    fn remove_session_clears_file() {
        let temp = tempdir().unwrap();
        let session = create_session(temp.path()).unwrap();
        write_session(temp.path(), &session, "ios", "ws://127.0.0.1:65535").unwrap();
        remove_session(temp.path(), &session.session_id).unwrap();
        assert!(list_sessions(temp.path()).unwrap().is_empty());
    }

    #[test]
    fn prune_stale_removes_unreachable_sessions() {
        let temp = tempdir().unwrap();
        let session = create_session(temp.path()).unwrap();
        // Port 1 is never bound by anything legitimate; TCP probe will fail.
        write_session(temp.path(), &session, "harmony", "ws://127.0.0.1:1").unwrap();
        let pruned = prune_stale(temp.path()).unwrap();
        assert_eq!(pruned.len(), 1);
        assert!(list_sessions(temp.path()).unwrap().is_empty());
    }

    #[test]
    fn prune_stale_removes_malformed_session_files() {
        let temp = tempdir().unwrap();
        let dir = sessions_dir(temp.path());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("bad.json"), b"{not json").unwrap();

        let pruned = prune_stale(temp.path()).unwrap();
        assert!(pruned.is_empty());
        assert!(list_sessions(temp.path()).unwrap().is_empty());
        assert!(!dir.join("bad.json").exists());
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
}
