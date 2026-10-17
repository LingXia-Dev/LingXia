//! `lingxia dev listen` in a standalone lxapp.
//!
//! With `--port`, this process is the session an installed host app dials,
//! and this lxapp is its content. Without it, a running host session builds,
//! watches, and keeps this lxapp open; the request's websocket stays open, and
//! closing it (Ctrl+C, or this process dying) drops the attach. The host's own
//! bundles keep watching.

use super::forward::DevPortForward;
use super::lxapp_watch::read_app_id;
use super::{
    PlatformType, log_store, loopback_ws_url, persistent_device_token, platform,
    platform_session_name, print_dev_banner, server, stop_dev_server, take_over_target_session,
};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use lingxia_control_protocol::dev_session::broker::{self, SessionContent, SessionInfo};
use lingxia_control_protocol::dev_session::select::{self, SelectError};
use lingxia_control_protocol::dev_session::ws_url_with_token;
use lingxia_control_protocol::{
    ControlRequest,
    dev_session::{DEV_SESSION_PROTOCOL_VERSION, DevSessionMessage, DevSessionRole, capabilities},
    methods,
};
use std::fmt;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tungstenite::client::IntoClientRequest;
use tungstenite::protocol::Message;
use tungstenite::{Error as WsError, WebSocket};

pub(super) struct AttachRequest {
    pub project_root: PathBuf,
    pub session: Option<String>,
    pub framework: Option<String>,
    pub release: bool,
    pub stop_requested: Arc<AtomicBool>,
    pub platform_arg: Option<String>,
    pub device: Option<String>,
    pub port: Option<u16>,
    pub name: Option<String>,
}

pub(super) fn is_standalone_lxapp(root: &Path) -> bool {
    root.join("lxapp.json").is_file() && !root.join("lingxia.yaml").is_file()
}

pub(super) fn execute(request: AttachRequest) -> Result<()> {
    if let Some(port) = request.port {
        return host_and_attach(request, port);
    }
    if request.platform_arg.is_some() || request.device.is_some() {
        return Err(anyhow!(
            "`-p` and `--device` say how the app reaches this session, so they go with \
             `--port`. Without it, this lxapp attaches to a running host session; use \
             `--session` when more than one is running."
        ));
    }
    if request.name.is_some() {
        return Err(anyhow!(
            "`--name` names a session this command starts, which needs `--port`. Use \
             `--session` to pick a running host session."
        ));
    }

    let source = resolve_source(&request.project_root)?;
    let app_id = read_app_id(&source)?;
    let mut sessions = broker::list_sessions().context("Failed to list dev sessions")?;
    // A session no app is connected to cannot open this lxapp, so only an
    // explicit `--session` may name one.
    if request.session.is_none() {
        sessions.retain(|session| {
            session.target == "lxapp"
                || log_store::session_state(session) == log_store::DevSessionState::Ready
        });
    }
    let session = select_host_session(&sessions, request.session.as_deref(), &source)
        .map_err(|err| anyhow!("{err}"))?;

    println!();
    println!(
        "{}",
        format!("Attaching {app_id} to the {} session.", session.target)
            .bold()
            .cyan()
    );
    println!("  {}  {}", "Session:".bold(), session_label(session));
    let token_path = server::attach_token_path(Path::new(&session.log_file));
    let attach_token = std::fs::read_to_string(&token_path)
        .map(|token| token.trim().to_string())
        .with_context(|| {
            format!(
                "That session predates attach tokens or belongs to another user ({} is \
                 unreadable). Restart it with this CLI.",
                token_path.display()
            )
        })?;
    attach_and_hold(
        &session.ws_url,
        &source,
        &attach_token,
        request.framework.as_deref(),
        request.release,
        &request.stop_requested,
    )
}

/// Be the session an installed host app dials on `port`, with this lxapp as
/// its content: prepared, built, watched, and opened on every connect that
/// finds it closed. Nothing is built, installed, or launched for the host.
fn host_and_attach(request: AttachRequest, port: u16) -> Result<()> {
    if request.session.is_some() {
        return Err(anyhow!(
            "`--session` picks a running host session. With `--port`, this command is the \
             session the app dials."
        ));
    }
    let platform: PlatformType = request
        .platform_arg
        .as_deref()
        .ok_or_else(|| {
            anyhow!(
                "`--port` needs `-p`: android and harmony reach this machine over USB, ios over \
                 Wi-Fi, macos and windows on loopback."
            )
        })?
        .parse()?;
    if request.device.is_some()
        && !matches!(platform, PlatformType::Android | PlatformType::Harmony)
    {
        return Err(anyhow!(
            "`--device` picks the USB device to forward the port to (android, harmony)."
        ));
    }
    let platform_name = platform_session_name(platform.clone());
    let source = resolve_source(&request.project_root)?;
    let app_id = read_app_id(&source)?;
    take_over_target_session(&source, platform_name)?;

    // An iOS host built on this machine dials this Mac over Wi-Fi (its
    // Bonjour name, which may resolve to IPv6) with this machine's device
    // token; the others dial loopback.
    let (bind_host, auth_token) = match platform {
        PlatformType::Ios => ("[::]", Some(persistent_device_token()?)),
        _ => ("127.0.0.1", None),
    };
    let mut server = server::start_server_at_with_roots(
        &source,
        &source,
        bind_host,
        port,
        request.stop_requested.clone(),
        auth_token.clone(),
    )
    .map_err(|err| port_error(err, bind_host, port))?;
    let _forward = match platform {
        PlatformType::Android => {
            let (device_id, _) =
                platform::android::resolve_dev_device_target(request.device.as_deref())?;
            Some(DevPortForward::android(Some(&device_id), port)?)
        }
        PlatformType::Harmony => Some(DevPortForward::harmony(request.device.as_deref(), port)?),
        _ => None,
    };
    let host_ws_url = match auth_token.as_deref() {
        Some(token) => ws_url_with_token(&loopback_ws_url(port), token),
        None => server.ws_url(),
    };
    let registration = log_store::register_session_with_content(
        &source,
        server.session(),
        platform_name,
        &host_ws_url,
        SessionContent::LxApp {
            path: source.to_string_lossy().into_owned(),
        },
    )?;

    println!();
    println!(
        "{}",
        "Listening for an installed host app. No build, install, or launch."
            .bold()
            .cyan()
    );
    let result = (|| -> Result<()> {
        server.rebuild_lxapp(&app_id, request.framework.as_deref(), request.release)?;
        server.watch_lxapps(request.framework.as_deref(), request.release);
        // A failed open is retried on the next connect; the session stays.
        if let Err(err) = server.present(&app_id) {
            eprintln!("  {} open {app_id} failed: {err:#}", "✗".red());
        }
        print_dev_banner(platform_name, "Ctrl+C", &[("Port", &port.to_string())]);
        if !server.runtime_connected() {
            println!("  {} waiting for the app to connect", "•".cyan());
        }
        let hint_at = std::time::Instant::now() + Duration::from_secs(15);
        let mut hinted = false;
        while !request.stop_requested.load(Ordering::Acquire) {
            if !hinted && !server.runtime_connected() && std::time::Instant::now() >= hint_at {
                hinted = true;
                println!(
                    "  {} still waiting. Bring the app to the foreground; its dev tab must \
                     show {port}.",
                    "•".yellow()
                );
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        Ok(())
    })();
    drop(registration);
    drop(_forward);
    stop_dev_server(server, result)
}

/// A busy port names the session that holds it, when it is one of ours.
fn port_error(err: anyhow::Error, bind_host: &str, port: u16) -> anyhow::Error {
    let owner = broker::list_sessions().ok().and_then(|sessions| {
        sessions.into_iter().find(|session| {
            parse_ws_addr(&session.ws_url).is_some_and(|addr| addr.ends_with(&format!(":{port}")))
        })
    });
    match owner {
        Some(session) => anyhow!(
            "Port {port} is held by the {} session for {}. Stop it with `lingxia dev stop {}`, \
             then run this again.",
            session.target,
            session.project_root,
            session
                .name
                .clone()
                .unwrap_or_else(|| format!("{}@{}", session.target, session.project_root))
        ),
        None => err.context(format!(
            "Could not listen on {bind_host}:{port}. That is the port the app dials (its dev \
             tab shows it); listen does not pick another."
        )),
    }
}

fn resolve_source(project_root: &Path) -> Result<PathBuf> {
    // `canonicalize` keeps the `\\?\` prefix on Windows, and Node then fails
    // the Vite build with `EISDIR: lstat 'D:'`.
    dunce::canonicalize(project_root)
        .with_context(|| format!("Failed to resolve {}", project_root.display()))
}

/// Ask the session at `ws_url` to build, open, and watch `source`, then hold
/// the request open until Ctrl+C. Closing it drops the watch.
fn attach_and_hold(
    ws_url: &str,
    source: &Path,
    attach_token: &str,
    framework: Option<&str>,
    release: bool,
    stop_requested: &AtomicBool,
) -> Result<()> {
    let mut websocket = connect_host(ws_url)?;
    send_message(
        &mut websocket,
        &DevSessionMessage::Hello {
            version: DEV_SESSION_PROTOCOL_VERSION,
            role: DevSessionRole::Controller,
            capabilities: vec![capabilities::REQUESTS.to_string()],
            build: None,
        },
    )?;
    let command_id = uuid::Uuid::new_v4().to_string();
    let mut params = serde_json::json!({
        "path": source,
        "attachToken": attach_token,
    });
    // Omitted, the host session's own `--release` applies.
    if release {
        params["release"] = serde_json::Value::Bool(true);
    }
    if let Some(framework) = framework {
        params["framework"] = serde_json::Value::String(framework.to_string());
    }
    send_message(
        &mut websocket,
        &DevSessionMessage::Request(ControlRequest {
            id: command_id.clone(),
            method: methods::session::LXAPP_ATTACH.to_string(),
            params: Some(params),
        }),
    )?;

    let (attached_id, opened) = wait_for_attach(&mut websocket, &command_id, stop_requested)?;
    if opened {
        println!("  {} Opened {attached_id}.", "✓".green());
    } else {
        println!(
            "  {} {attached_id} opens when the app connects.",
            "•".cyan()
        );
    }
    println!(
        "  {} Saving this directory rebuilds and reloads it until Ctrl+C. Build output is in \
         the host session's terminal.",
        "•".cyan()
    );

    hold_until_stop(&mut websocket, stop_requested);
    println!();
    println!("{}", format!("Detached {attached_id}.").yellow().bold());
    Ok(())
}

/// Which host session a standalone listen attaches to.
///
/// The default selector refuses another project's session, and this command's
/// directory is the lxapp, not the host. One host session is used as-is.
/// Several need `--session`. Runner sessions (`target` `lxapp`) are not hosts.
pub(super) fn select_host_session<'a>(
    sessions: &'a [SessionInfo],
    query: Option<&str>,
    cwd: &Path,
) -> Result<&'a SessionInfo, AttachSelectError> {
    let query = query.map(str::trim).filter(|query| !query.is_empty());
    if let Some(query) = query {
        let session =
            select::select(sessions, Some(query), cwd).map_err(AttachSelectError::Select)?;
        if session.target == "lxapp" {
            return Err(AttachSelectError::Runner {
                query: query.to_string(),
            });
        }
        return Ok(session);
    }
    let hosts: Vec<_> = sessions
        .iter()
        .filter(|session| session.target != "lxapp")
        .collect();
    match hosts.as_slice() {
        [] => Err(AttachSelectError::NoHost),
        [only] => Ok(*only),
        many => Err(AttachSelectError::Ambiguous(host_table(many))),
    }
}

#[derive(Debug)]
pub(super) enum AttachSelectError {
    NoHost,
    Runner { query: String },
    Select(SelectError),
    Ambiguous(String),
}

impl fmt::Display for AttachSelectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoHost => f.write_str(
                "No host dev session has an app connected. Run `lingxia dev listen -p <platform> \
                 --port <the port on the app's dev tab>` to be that session, or start \
                 `lingxia dev` in the host app.",
            ),
            Self::Runner { query } => write!(
                f,
                "`{query}` is a Runner session. `lingxia dev listen` here attaches to a host app \
                 session."
            ),
            Self::Select(err) => write!(f, "{err}"),
            Self::Ambiguous(table) => write!(
                f,
                "Several host sessions are running. Pick one with --session:\n\n{table}\n\n\
                 --session takes a NAME, a TARGET, or TARGET@<project-dir>."
            ),
        }
    }
}

fn host_table(sessions: &[&SessionInfo]) -> String {
    sessions
        .iter()
        .map(|session| {
            format!(
                "  {:<16} {:<10} {}",
                session.name.as_deref().unwrap_or("-"),
                session.target,
                session.project_root
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn session_label(session: &SessionInfo) -> String {
    match session.name.as_deref() {
        Some(name) => format!("{name}  {}", session.project_root),
        None => session.project_root.clone(),
    }
}

fn connect_host(ws_url: &str) -> Result<WebSocket<TcpStream>> {
    let authority = parse_ws_addr(ws_url).ok_or_else(|| anyhow!("invalid dev session URL"))?;
    let mut last_error = None;
    for socket_addr in authority
        .to_socket_addrs()
        .with_context(|| format!("Failed to resolve {authority}"))?
    {
        match TcpStream::connect_timeout(&socket_addr, Duration::from_secs(5)) {
            Ok(stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
                let request = ws_url
                    .into_client_request()
                    .context("Failed to build the dev websocket request")?;
                let (websocket, _) = tungstenite::client::client(request, stream)
                    .context("Failed to connect to the host dev session")?;
                return Ok(websocket);
            }
            Err(err) => last_error = Some(err),
        }
    }
    Err(anyhow!(
        "Could not connect to the host dev session{}",
        last_error.map(|err| format!(": {err}")).unwrap_or_default()
    ))
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

fn send_message(websocket: &mut WebSocket<TcpStream>, message: &DevSessionMessage) -> Result<()> {
    let text = serde_json::to_string(message).context("Failed to encode dev websocket message")?;
    websocket
        .send(Message::Text(text.into()))
        .context("Failed to send dev websocket message")
}

fn wait_for_attach(
    websocket: &mut WebSocket<TcpStream>,
    command_id: &str,
    stop_requested: &AtomicBool,
) -> Result<(String, bool)> {
    loop {
        if stop_requested.load(Ordering::Acquire) {
            let _ = websocket.close(None);
            return Err(anyhow!("Stopped before the host finished attaching."));
        }
        match websocket.read() {
            Ok(Message::Text(text)) => {
                let Ok(DevSessionMessage::Response(response)) = serde_json::from_str(&text) else {
                    continue;
                };
                if response.id != command_id {
                    continue;
                }
                if let Some(error) = response.error {
                    return Err(anyhow!(error.message));
                }
                let app_id = response
                    .result
                    .as_ref()
                    .and_then(|value| value.get("appId"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if app_id.is_empty() {
                    return Err(anyhow!("the host attached the lxapp without an app id"));
                }
                let opened = response
                    .result
                    .as_ref()
                    .and_then(|value| value.get("opened"))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true);
                return Ok((app_id, opened));
            }
            Ok(Message::Close(_)) => {
                return Err(anyhow!(
                    "The host session closed before the lxapp attached."
                ));
            }
            Ok(_) => {}
            Err(WsError::Io(err))
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) => {}
            Err(err) => {
                return Err(anyhow!(
                    "The host session closed before the lxapp attached: {err}"
                ));
            }
        }
    }
}

fn hold_until_stop(websocket: &mut WebSocket<TcpStream>, stop_requested: &AtomicBool) {
    loop {
        if stop_requested.load(Ordering::Acquire) {
            let _ = websocket.close(None);
            return;
        }
        match websocket.read() {
            Ok(Message::Close(_)) => return,
            Ok(_) => {}
            Err(WsError::Io(err))
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) => {}
            Err(_) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str, target: &str, root: &str, name: Option<&str>) -> SessionInfo {
        SessionInfo {
            session_id: id.to_string(),
            project_root: root.to_string(),
            content: None,
            target: target.to_string(),
            pid: 1,
            started_at: 1,
            executable: String::new(),
            ws_url: "ws://127.0.0.1:1".to_string(),
            log_file: String::new(),
            name: name.map(str::to_string),
            build: None,
            extra: Default::default(),
        }
    }

    #[test]
    fn attach_standalone_lxapp_is_lxapp_json_without_yaml() {
        let temp = tempfile::tempdir().unwrap();
        assert!(!is_standalone_lxapp(temp.path()));
        std::fs::write(temp.path().join("lxapp.json"), "{}").unwrap();
        assert!(is_standalone_lxapp(temp.path()));
        std::fs::write(temp.path().join("lingxia.yaml"), "app: {}\n").unwrap();
        assert!(!is_standalone_lxapp(temp.path()));
    }

    #[test]
    fn attach_selects_the_only_host_and_skips_a_runner() {
        let sessions = [
            session("runner1", "lxapp", "/work/chat", None),
            session("host1", "windows", "/work/showcase", Some("mark-win")),
        ];
        let picked = select_host_session(&sessions, None, Path::new("/work/chat")).unwrap();
        assert_eq!(picked.session_id, "host1");
    }

    #[test]
    fn attach_requires_a_session_when_several_hosts_are_running() {
        let sessions = [
            session("host1", "windows", "/work/showcase", Some("mark-win")),
            session("host2", "android", "/work/showcase", Some("gpu")),
        ];
        let err = select_host_session(&sessions, None, Path::new("/work/chat")).unwrap_err();
        let AttachSelectError::Ambiguous(table) = err else {
            panic!("expected an ambiguity");
        };
        assert!(table.contains("mark-win"));
        assert!(table.contains("gpu"));
    }

    #[test]
    fn attach_uses_a_session_name_and_refuses_a_runner_selector() {
        let sessions = [
            session("runner1", "lxapp", "/work/chat", Some("chat-run")),
            session("host1", "windows", "/work/showcase", Some("mark-win")),
        ];
        let cwd = Path::new("/work/chat");
        assert_eq!(
            select_host_session(&sessions, Some("mark-win"), cwd)
                .unwrap()
                .session_id,
            "host1"
        );
        let err = select_host_session(&sessions, Some("chat-run"), cwd).unwrap_err();
        assert!(matches!(err, AttachSelectError::Runner { .. }));
    }

    #[test]
    fn attach_reports_when_no_host_is_running() {
        let sessions = [session("runner1", "lxapp", "/work/chat", None)];
        let err = select_host_session(&sessions, None, Path::new("/work/chat")).unwrap_err();
        assert!(matches!(err, AttachSelectError::NoHost));
    }
}
