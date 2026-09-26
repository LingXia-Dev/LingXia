use anyhow::{Context, Result, anyhow};
use lingxia_control_protocol::dev_session::broker::Registration;
pub use lingxia_control_protocol::dev_session::broker::SessionInfo;
use lingxia_control_protocol::{
    ControlRequest,
    dev_session::{DEV_SESSION_PROTOCOL_VERSION, DevSessionMessage, DevSessionRole, capabilities},
    methods,
};
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
/// project-scoped queries (`lingxia dev stop`, duplicate guard) filter
/// by it.
pub fn canonical_project_root(project_root: &Path) -> String {
    let canonical = fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    strip_verbatim_prefix(&canonical.display().to_string())
}

/// On Windows `fs::canonicalize` returns extended-length paths (`\\?\C:\…`,
/// `\\?\UNC\server\share\…`). The verbatim prefix is noise in an identity
/// that is also shown to users (`lxdev session`'s CONTENT column) — strip
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

/// `lingxia dev --name`: the alias every session this process registers
/// carries.
static SESSION_NAME: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

pub fn set_session_name(name: Option<String>) {
    let _ = SESSION_NAME.set(name);
}

/// Register this dev session with the per-user broker. The returned guard
/// keeps the registration alive (re-registering across broker restarts);
/// dropping it — or process exit — removes the session.
pub fn register_session(
    project_root: &Path,
    session: &DevLogSession,
    target: &str,
    ws_url: &str,
) -> Result<Registration> {
    register_session_with_content(
        project_root,
        session,
        target,
        ws_url,
        lingxia_control_protocol::dev_session::broker::SessionContent::Host {
            path: canonical_project_root(project_root),
        },
    )
}

pub fn register_session_with_content(
    context_root: &Path,
    session: &DevLogSession,
    target: &str,
    ws_url: &str,
    content: lingxia_control_protocol::dev_session::broker::SessionContent,
) -> Result<Registration> {
    let name = SESSION_NAME.get().cloned().flatten();
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
        name: name.clone(),
        build: Some(env!("LINGXIA_BUILD_VERSION").to_string()),
        extra: Default::default(),
    };
    ensure_current_broker(name.is_some())?;
    let registration =
        lingxia_control_protocol::dev_session::broker::register_session(info, spawn_broker);
    if let Some(name) = &name {
        verify_registered_name(&session.session_id, name)?;
    }
    Ok(registration)
}

/// `--name` is how scripts address the session: a broker that dropped it
/// must stop the session here, not leave `lxdev --session NAME` to fail.
fn verify_registered_name(session_id: &str, name: &str) -> Result<()> {
    use lingxia_control_protocol::dev_session::broker;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let sessions = broker::list_sessions().unwrap_or_default();
        if let Some(registered) = sessions.iter().find(|s| s.session_id == session_id) {
            return check_registered_name(registered, name);
        }
        if std::time::Instant::now() >= deadline {
            eprintln!(
                "⚠ Could not confirm that the dev broker registered this session as {name:?}."
            );
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn check_registered_name(registered: &SessionInfo, name: &str) -> Result<()> {
    if registered.name.as_deref() == Some(name) {
        return Ok(());
    }
    Err(anyhow!(
        "the dev broker registered this session without its name {name:?} (it is an older \
         build that does not know `--name`). Stop the `lingxia dev-broker` process and \
         start the session again."
    ))
}

/// The per-user broker outlives the `lingxia` that spawned it, so it can be
/// a stale build (another checkout, rebuilt or upgraded since) — and a stale
/// broker drops the record fields it does not know, such as `--name`.
/// Replace it: asked to exit when idle, else terminated; its live sessions
/// re-register with the new broker by themselves. When it cannot be
/// replaced, `required` (a `--name` depends on it) makes that an error.
pub fn ensure_current_broker(required: bool) -> Result<()> {
    use lingxia_control_protocol::dev_session::broker;
    let current = broker::BrokerBuild::current(env!("CARGO_PKG_VERSION"));
    let Some(stale) = stale_broker(broker::probe_broker(), &current) else {
        return Ok(());
    };
    match replace_broker(&stale, &current) {
        Ok(()) => {
            let moved = match stale.sessions {
                Some(live) if live > 0 => {
                    format!("; its {live} live session(s) re-register with the new one")
                }
                _ => String::new(),
            };
            let pid = stale
                .pid
                .map(|pid| format!(" (pid {pid})"))
                .unwrap_or_default();
            eprintln!(
                "ℹ Replaced the dev broker{pid}: it was {}{moved}.",
                stale.why
            );
            Ok(())
        }
        Err(err) => {
            let pid = stale
                .pid
                .map(|pid| format!(" (pid {pid})"))
                .unwrap_or_default();
            let message = format!(
                "The dev broker{pid} is {} and could not be replaced ({err:#}). Stop that \
                 `lingxia dev-broker` process; the next `lingxia dev` starts a current one.",
                stale.why
            );
            if required {
                return Err(anyhow!(
                    "{message}\n`--name` needs a current broker: an older one drops the name."
                ));
            }
            eprintln!("⚠ {message}");
            Ok(())
        }
    }
}

/// A running broker that is not this build.
#[derive(Debug)]
struct StaleBroker {
    why: String,
    pid: Option<u32>,
    /// `None` for a broker too old to report them.
    sessions: Option<usize>,
}

fn stale_broker(
    probe: lingxia_control_protocol::dev_session::broker::BrokerProbe,
    current: &lingxia_control_protocol::dev_session::broker::BrokerBuild,
) -> Option<StaleBroker> {
    use lingxia_control_protocol::dev_session::broker::BrokerProbe;
    match probe {
        BrokerProbe::Absent => None,
        BrokerProbe::Legacy => Some(StaleBroker {
            why: "an older build without version reporting".to_string(),
            pid: None,
            sessions: None,
        }),
        BrokerProbe::Running(info) => {
            // Only a broker started as a library reports no build.
            let why = info.build.as_ref()?.mismatch(current)?;
            Some(StaleBroker {
                why,
                pid: Some(info.pid),
                sessions: Some(info.sessions),
            })
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BrokerRestart {
    /// Idle: ask it to exit.
    AskToExit,
    /// Sessions use it, which re-register once it is gone: terminate it.
    Terminate(u32),
    /// Too old to say its pid: find the `dev-broker` process and terminate it.
    FindAndTerminate,
}

fn broker_restart_plan(stale: &StaleBroker) -> BrokerRestart {
    match (stale.sessions, stale.pid) {
        (Some(0), _) => BrokerRestart::AskToExit,
        (_, Some(pid)) => BrokerRestart::Terminate(pid),
        (_, None) => BrokerRestart::FindAndTerminate,
    }
}

fn replace_broker(
    stale: &StaleBroker,
    current: &lingxia_control_protocol::dev_session::broker::BrokerBuild,
) -> Result<()> {
    use lingxia_control_protocol::dev_session::broker::{self, BrokerProbe};
    let plan = broker_restart_plan(stale);
    let asked = plan == BrokerRestart::AskToExit
        && matches!(broker::shutdown_idle_broker(), Ok(true))
        && wait_broker_gone();
    if !asked {
        // Refused (a session registered meanwhile), or never idle.
        let pids = match (plan, stale.pid) {
            (BrokerRestart::FindAndTerminate, _) | (_, None) => broker_processes(),
            (_, Some(pid)) => vec![pid],
        };
        if pids.is_empty() {
            return Err(anyhow!("no `lingxia dev-broker` process found"));
        }
        for pid in pids {
            terminate_broker_process(pid);
        }
        if !wait_broker_gone() {
            return Err(anyhow!("it did not exit"));
        }
    }
    // Start the current build right away, before the old broker's sessions
    // come back to re-register and could start their own build instead.
    spawn_broker().context("could not start a new dev broker")?;
    for _ in 0..30 {
        if let BrokerProbe::Running(info) = broker::probe_broker() {
            return match info
                .build
                .as_ref()
                .and_then(|build| build.mismatch(current))
            {
                None => Ok(()),
                Some(why) => Err(anyhow!("another build took its place: {why}")),
            };
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(anyhow!("the new dev broker did not come up"))
}

/// What telling a broker process apart needs: its command line and user.
fn refresh_broker_candidates(system: &mut sysinfo::System, which: sysinfo::ProcessesToUpdate) {
    use sysinfo::{ProcessRefreshKind, UpdateKind};
    system.refresh_processes_specifics(
        which,
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_user(UpdateKind::Always),
    );
}

/// This user's `lingxia dev-broker` processes, other than this process.
fn broker_processes() -> Vec<u32> {
    use sysinfo::{ProcessesToUpdate, System};
    let mut system = System::new();
    refresh_broker_candidates(&mut system, ProcessesToUpdate::All);
    let me = sysinfo::get_current_pid().ok();
    let my_user = me
        .and_then(|pid| system.process(pid))
        .and_then(|process| process.user_id().cloned());
    system
        .processes()
        .iter()
        .filter(|(pid, process)| {
            Some(**pid) != me
                && is_broker_command(process.cmd())
                && (my_user.is_none() || process.user_id() == my_user.as_ref())
        })
        .map(|(pid, _)| pid.as_u32())
        .collect()
}

fn is_broker_command(cmd: &[std::ffi::OsString]) -> bool {
    cmd.get(1).is_some_and(|arg| arg == "dev-broker")
        && cmd.first().is_some_and(|exe| {
            Path::new(exe)
                .file_stem()
                .is_some_and(|stem| stem.to_string_lossy().starts_with("lingxia"))
        })
}

fn terminate_broker_process(pid: u32) {
    use sysinfo::{ProcessesToUpdate, Signal, System};
    let pid = sysinfo::Pid::from_u32(pid);
    let mut system = System::new();
    refresh_broker_candidates(&mut system, ProcessesToUpdate::Some(&[pid]));
    // Only a broker: the pid came from the broker itself or a process scan,
    // but guard against its reuse since.
    let Some(process) = system.process(pid).filter(|p| is_broker_command(p.cmd())) else {
        return;
    };
    if process.kill_with(Signal::Term).is_none() {
        process.kill();
    }
    for _ in 0..20 {
        system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        if system.process(pid).is_none() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if let Some(process) = system.process(pid) {
        process.kill();
    }
}

fn wait_broker_gone() -> bool {
    use lingxia_control_protocol::dev_session::broker::{self, BrokerProbe};
    for _ in 0..30 {
        if matches!(broker::probe_broker(), BrokerProbe::Absent) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// Live sessions for this project, ordered by start time.
pub fn list_sessions(project_root: &Path) -> Result<Vec<SessionInfo>> {
    let root = canonical_project_root(project_root);
    let mut sessions: Vec<SessionInfo> =
        lingxia_control_protocol::dev_session::broker::list_sessions_spawning(&spawn_broker)
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

pub fn request_shutdown(info: &SessionInfo) -> Result<()> {
    let mut websocket = connect_devtools_ws(&info.ws_url, WS_COMMAND_TIMEOUT)
        .ok_or_else(|| anyhow!("Failed to connect dev websocket: {}", info.ws_url))?;
    send_wire_message(
        &mut websocket,
        &DevSessionMessage::Hello {
            version: DEV_SESSION_PROTOCOL_VERSION,
            role: DevSessionRole::Controller,
            capabilities: vec![capabilities::REQUESTS.to_string()],
            build: None,
        },
    )?;

    let command_id = format!("shutdown-{}", now_timestamp_ms());
    send_wire_message(
        &mut websocket,
        &DevSessionMessage::Request(ControlRequest {
            id: command_id.clone(),
            method: methods::session::SHUTDOWN.to_string(),
            params: None,
        }),
    )?;

    loop {
        let message = websocket
            .read()
            .context("Failed to read dev websocket shutdown response")?;
        let Message::Text(text) = message else {
            continue;
        };
        match serde_json::from_str(&text) {
            Ok(DevSessionMessage::Response(response)) if response.id == command_id => {
                if response.error.is_none() {
                    return Ok(());
                }
                return Err(anyhow!("{}", response.error.unwrap().message));
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
        &DevSessionMessage::Hello {
            version: DEV_SESSION_PROTOCOL_VERSION,
            role: DevSessionRole::Controller,
            capabilities: vec![capabilities::REQUESTS.to_string()],
            build: None,
        },
    )
    .is_err()
    {
        return None;
    }

    let command_id = format!("probe-{}", now_timestamp_ms());
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
            Ok(DevSessionMessage::Response(response)) if response.id == command_id => {
                return Some((response.error.is_none(), response.result));
            }
            Ok(_) => continue,
            Err(_) => return None,
        }
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
    fn a_stale_broker_is_replaced_even_while_sessions_use_it() {
        use lingxia_control_protocol::dev_session::broker::{BrokerBuild, BrokerInfo, BrokerProbe};
        let current = BrokerBuild {
            version: "0.19.0".into(),
            executable: "/bin/lingxia".into(),
            modified_ms: 2,
        };
        let running = |build: BrokerBuild, sessions| {
            BrokerProbe::Running(BrokerInfo {
                build: Some(build),
                pid: 77,
                sessions,
            })
        };
        assert!(stale_broker(BrokerProbe::Absent, &current).is_none());
        assert!(stale_broker(running(current.clone(), 3), &current).is_none());
        // Upgraded in place since it started, with sessions of its own: it
        // used to be left running (and dropped the new session's --name).
        let upgraded = BrokerBuild {
            modified_ms: 1,
            ..current.clone()
        };
        let busy = stale_broker(running(upgraded.clone(), 2), &current).unwrap();
        assert!(busy.why.contains("rebuilt"), "{}", busy.why);
        assert_eq!(broker_restart_plan(&busy), BrokerRestart::Terminate(77));
        let idle = stale_broker(running(upgraded, 0), &current).unwrap();
        assert_eq!(broker_restart_plan(&idle), BrokerRestart::AskToExit);
        let legacy = stale_broker(BrokerProbe::Legacy, &current).unwrap();
        assert_eq!(
            broker_restart_plan(&legacy),
            BrokerRestart::FindAndTerminate
        );
    }

    #[test]
    fn a_session_registered_without_its_name_is_an_error() {
        let session = |name: Option<&str>| -> SessionInfo {
            serde_json::from_value(serde_json::json!({
                "session_id": "abc123", "project_root": "/p", "target": "runner", "pid": 1,
                "ws_url": "ws://127.0.0.1:1", "log_file": "", "name": name
            }))
            .unwrap()
        };
        assert!(check_registered_name(&session(Some("ui")), "ui").is_ok());
        let err = check_registered_name(&session(None), "ui").unwrap_err();
        assert!(err.to_string().contains("without its name \"ui\""), "{err}");
    }

    #[test]
    fn only_a_lingxia_dev_broker_command_is_a_broker() {
        let cmd = |args: &[&str]| {
            args.iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>()
        };
        assert!(is_broker_command(&cmd(&[
            "/home/u/.local/bin/lingxia",
            "dev-broker"
        ])));
        assert!(!is_broker_command(&cmd(&["/bin/lingxia", "dev"])));
        assert!(!is_broker_command(&cmd(&["/bin/other", "dev-broker"])));
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
