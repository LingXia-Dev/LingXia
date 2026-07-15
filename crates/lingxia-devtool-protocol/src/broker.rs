//! Per-user dev-session broker.
//!
//! `lingxia dev` registers each live session over a local IPC connection and
//! keeps that connection open; the broker drops the session when the
//! connection closes, so registration itself is the liveness signal. `lxdev`
//! (and `lingxia dev status/stop`) query the broker instead of scanning a
//! project-local sessions directory.
//!
//! Transport: Unix domain socket (macOS/Linux, `~/.lingxia/broker.sock`,
//! 0600) or a per-user named pipe (Windows). Wire format: one JSON value per
//! line, versioned; records are self-describing so older brokers pass through
//! fields they do not know.

use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(unix)]
use interprocess::local_socket::{GenericFilePath, ToFsName};
#[cfg(windows)]
use interprocess::local_socket::{GenericNamespaced, ToNsName};
use interprocess::local_socket::{ListenerOptions, Name, Stream, prelude::*};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

/// One live dev session, as registered with the broker.
///
/// `target` is `"lxapp"` for a standalone lxapp Runner session, otherwise the
/// platform name (`macos`, `android`, `ios`, `harmony`, `windows`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub project_root: String,
    pub target: String,
    pub pid: u32,
    #[serde(default)]
    pub started_at: u64,
    pub ws_url: String,
    pub log_file: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Register { v: u32, session: SessionInfo },
    List { v: u32 },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "reply", rename_all = "snake_case")]
enum Response {
    Registered { v: u32 },
    Sessions { v: u32, sessions: Vec<SessionInfo> },
    Error { v: u32, message: String },
}

/// Local IPC name for the current user's broker.
///
/// Unix uses a filesystem socket under `~/.lingxia` (user-owned, so the
/// socket inherits user-only access and we tighten it to 0600 after bind).
/// Windows uses a per-user named pipe; pipe names are machine-global, so the
/// user name is part of the pipe name.
fn broker_name() -> std::io::Result<Name<'static>> {
    #[cfg(unix)]
    {
        socket_path()?.to_fs_name::<GenericFilePath>()
    }
    #[cfg(windows)]
    {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "default".to_string());
        format!("lingxia-dev-broker-{user}").to_ns_name::<GenericNamespaced>()
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(std::io::Error::other("unsupported platform"))
    }
}

#[cfg(unix)]
fn socket_path() -> std::io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| std::io::Error::other("HOME not set"))?;
    let dir = home.join(".lingxia");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("broker.sock"))
}

fn write_json<W: Write, T: Serialize>(w: &mut W, value: &T) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    w.write_all(&line)?;
    w.flush()
}

fn read_json<R: BufRead, T: for<'de> Deserialize<'de>>(r: &mut R) -> std::io::Result<Option<T>> {
    let mut line = String::new();
    if r.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(line.trim())?))
}

// ---------------------------------------------------------------------------
// Broker server (the hidden `lingxia dev-broker` process)
// ---------------------------------------------------------------------------

/// Run the broker accept loop. Returns `Ok(false)` without serving when
/// another live broker already owns the socket (the normal lost-race exit).
pub fn run_broker() -> std::io::Result<bool> {
    let name = broker_name()?;
    let listener = match ListenerOptions::new().name(name.clone()).create_sync() {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
            // Either a live broker owns the name, or (unix) a stale socket
            // file survived a crash. Probing with a connect distinguishes.
            if Stream::connect(name.clone()).is_ok() {
                return Ok(false);
            }
            #[cfg(unix)]
            {
                let _ = std::fs::remove_file(socket_path()?);
                ListenerOptions::new().name(name).create_sync()?
            }
            #[cfg(not(unix))]
            return Err(err);
        }
        Err(err) => return Err(err),
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // The ws_url in a session record grants full automation control:
        // the socket must be readable by this user only.
        std::fs::set_permissions(socket_path()?, std::fs::Permissions::from_mode(0o600))?;
    }

    let sessions: Arc<Mutex<Vec<SessionInfo>>> = Arc::new(Mutex::new(Vec::new()));
    for conn in listener.incoming() {
        let Ok(conn) = conn else { continue };
        let sessions = Arc::clone(&sessions);
        std::thread::spawn(move || serve_connection(conn, sessions));
    }
    Ok(true)
}

fn serve_connection(conn: Stream, sessions: Arc<Mutex<Vec<SessionInfo>>>) {
    let mut reader = BufReader::new(conn);
    loop {
        let request: Option<Request> = match read_json(&mut reader) {
            Ok(request) => request,
            Err(_) => return,
        };
        let Some(request) = request else { return };
        match request {
            Request::List { .. } => {
                let snapshot = sessions.lock().map(|s| s.clone()).unwrap_or_default();
                let _ = write_json(
                    reader.get_mut(),
                    &Response::Sessions {
                        v: PROTOCOL_VERSION,
                        sessions: snapshot,
                    },
                );
            }
            Request::Register { session, .. } => {
                let session_id = session.session_id.clone();
                {
                    let Ok(mut live) = sessions.lock() else {
                        return;
                    };
                    if live.iter().any(|s| s.session_id == session_id) {
                        let _ = write_json(
                            reader.get_mut(),
                            &Response::Error {
                                v: PROTOCOL_VERSION,
                                message: format!("session id {session_id} already registered"),
                            },
                        );
                        return;
                    }
                    live.push(session);
                }
                let _ = write_json(
                    reader.get_mut(),
                    &Response::Registered {
                        v: PROTOCOL_VERSION,
                    },
                );
                // The connection is the liveness signal: block until it
                // closes (session exit, crash, or explicit teardown), then
                // drop the registration.
                let mut sink = String::new();
                loop {
                    sink.clear();
                    match reader.read_line(&mut sink) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
                if let Ok(mut live) = sessions.lock() {
                    live.retain(|s| s.session_id != session_id);
                }
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Client side
// ---------------------------------------------------------------------------

fn connect() -> std::io::Result<Stream> {
    Stream::connect(broker_name()?)
}

/// Connect to the broker, spawning it via `spawn_broker` when unreachable.
/// `spawn_broker` must start a detached `<binary> dev-broker` process and is
/// allowed to fail (e.g. `lxdev` without a `lingxia` binary on PATH).
fn connect_or_spawn(spawn_broker: &dyn Fn() -> std::io::Result<()>) -> std::io::Result<Stream> {
    if let Ok(stream) = connect() {
        return Ok(stream);
    }
    spawn_broker()?;
    let mut last_err = std::io::Error::other("broker did not come up");
    for _ in 0..20 {
        match connect() {
            Ok(stream) => return Ok(stream),
            Err(err) => last_err = err,
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(last_err)
}

/// List live sessions. Never spawns a broker: with no broker there are no
/// registered sessions, which reads as an empty list.
pub fn list_sessions() -> std::io::Result<Vec<SessionInfo>> {
    let stream = match connect() {
        Ok(stream) => stream,
        Err(_) => return Ok(Vec::new()),
    };
    let mut reader = BufReader::new(stream);
    write_json(
        reader.get_mut(),
        &Request::List {
            v: PROTOCOL_VERSION,
        },
    )?;
    match read_json(&mut reader)? {
        Some(Response::Sessions { sessions, .. }) => Ok(sessions),
        Some(Response::Error { message, .. }) => Err(std::io::Error::other(message)),
        _ => Err(std::io::Error::other("unexpected broker reply")),
    }
}

/// List live sessions, spawning the broker first when unreachable. Useful
/// after a broker crash: a fresh broker lets surviving sessions re-register.
pub fn list_sessions_spawning(
    spawn_broker: &dyn Fn() -> std::io::Result<()>,
) -> std::io::Result<Vec<SessionInfo>> {
    if connect().is_err() {
        // Best effort: sessions need a moment to notice and re-register.
        if connect_or_spawn(spawn_broker).is_ok() {
            std::thread::sleep(Duration::from_millis(300));
        }
    }
    list_sessions()
}

/// Keeps a session registered for as long as the value lives. The owning
/// thread re-registers after a broker restart; dropping the guard (or process
/// exit) ends the registration.
pub struct Registration {
    shutdown: Arc<AtomicBool>,
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

/// Register `session` with the broker and keep it registered. The background
/// thread holds the registration connection open and, when the broker goes
/// away, reconnects and re-registers so a broker restart does not strand the
/// session. `spawn_broker` starts a detached broker process.
pub fn register_session(
    session: SessionInfo,
    spawn_broker: impl Fn() -> std::io::Result<()> + Send + 'static,
) -> Registration {
    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&shutdown);
    std::thread::spawn(move || {
        while !flag.load(Ordering::SeqCst) {
            let Ok(stream) = connect_or_spawn(&spawn_broker) else {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            };
            let mut reader = BufReader::new(stream);
            let registered = write_json(
                reader.get_mut(),
                &Request::Register {
                    v: PROTOCOL_VERSION,
                    session: session.clone(),
                },
            )
            .is_ok()
                && matches!(
                    read_json::<_, Response>(&mut reader),
                    Ok(Some(Response::Registered { .. }))
                );
            if registered {
                // Park on the connection; EOF/error means the broker died.
                let mut sink = String::new();
                loop {
                    sink.clear();
                    match reader.read_line(&mut sink) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
            }
            if !flag.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    });
    Registration { shutdown }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str) -> SessionInfo {
        SessionInfo {
            session_id: id.to_string(),
            project_root: "/tmp/p".to_string(),
            target: "macos".to_string(),
            pid: 42,
            started_at: 1,
            ws_url: "ws://127.0.0.1:1".to_string(),
            log_file: "/tmp/p/.lingxia/logs/x.jsonl".to_string(),
        }
    }

    #[test]
    fn wire_roundtrip() {
        let req = Request::Register {
            v: PROTOCOL_VERSION,
            session: session("abc123"),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: Request = serde_json::from_str(&json).unwrap();
        match back {
            Request::Register { session, .. } => assert_eq!(session.session_id, "abc123"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn session_record_tolerates_unknown_fields() {
        let json = r#"{
            "session_id": "a1b2c3", "project_root": "/p", "target": "lxapp",
            "pid": 7, "started_at": 5, "ws_url": "ws://x", "log_file": "/l",
            "from_the_future": true
        }"#;
        let info: SessionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.target, "lxapp");
    }
}
