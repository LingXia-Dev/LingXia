use super::log_store::{DevLogSession, create_session};
use anyhow::{Context, Result, anyhow};
use lingxia_devtool_protocol::{DevtoolsLogMessage, DevtoolsPeerRole, DevtoolsWireMessage};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tungstenite::handshake::server::{Request, Response};
use tungstenite::protocol::Message;
use tungstenite::{Error as WsError, WebSocket, accept_hdr};

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const COMMAND_TIMEOUT_BUFFER: Duration = Duration::from_secs(5);
const DEV_LXAPP_HTTP_PREFIX: &str = "/__lingxia/dev/lxapp/";

#[derive(Debug)]
pub struct DevServerHandle {
    session: DevLogSession,
    ws_addr: SocketAddr,
    stop_flag: Arc<AtomicBool>,
    server_thread: Option<JoinHandle<()>>,
}

impl DevServerHandle {
    /// Connectable ws URL for same-machine peers. A physical-device server
    /// may bind `0.0.0.0`, which is not a destination address, so render it as
    /// loopback for local `lxdev`.
    pub fn ws_url(&self) -> String {
        if self.ws_addr.ip().is_unspecified() {
            format!("ws://127.0.0.1:{}", self.ws_addr.port())
        } else {
            format!("ws://{}", self.ws_addr)
        }
    }

    pub fn port(&self) -> u16 {
        self.ws_addr.port()
    }

    pub fn session(&self) -> &DevLogSession {
        &self.session
    }

    pub fn stop(mut self) -> Result<()> {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(thread) = self.server_thread.take() {
            thread
                .join()
                .map_err(|_| anyhow!("dev server thread panicked"))?;
        }
        Ok(())
    }
}

struct SessionLogWriter {
    file: Mutex<File>,
}

impl SessionLogWriter {
    fn new(session: &DevLogSession) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&session.log_file)
            .with_context(|| format!("Failed to open {}", session.log_file.display()))?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }

    fn append_logs(&self, logs: &[DevtoolsLogMessage]) -> Result<()> {
        let mut file = self
            .file
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for log in logs {
            serde_json::to_writer(&mut *file, log).context("Failed to encode log line")?;
            file.write_all(b"\n")
                .context("Failed to write log newline")?;
        }
        file.flush().context("Failed to flush log file")?;
        Ok(())
    }
}

struct DevServerState {
    project_root: PathBuf,
    stop_flag: Arc<AtomicBool>,
    runtime_sender: Mutex<Option<(u64, Sender<DevtoolsWireMessage>)>>,
    next_runtime_id: AtomicU64,
    pending_results: Mutex<std::collections::HashMap<String, Sender<DevtoolsWireMessage>>>,
    command_lock: Mutex<()>,
    /// When set (non-loopback bind), every peer must present this token in
    /// its `Hello` and HTTP requests must carry it as `?token=`.
    auth_token: Option<String>,
    /// A runtime peer failed token auth since the last successful runtime
    /// connection — almost always a physical-device host binary that predates
    /// session tokens. Folded into the "runtime is not connected" client error so
    /// the operator sees the cause on their side of the wire.
    runtime_rejected: AtomicBool,
}

impl DevServerState {
    fn new(project_root: PathBuf, stop_flag: Arc<AtomicBool>, auth_token: Option<String>) -> Self {
        Self {
            project_root,
            stop_flag,
            runtime_sender: Mutex::new(None),
            next_runtime_id: AtomicU64::new(1),
            pending_results: Mutex::new(std::collections::HashMap::new()),
            command_lock: Mutex::new(()),
            auth_token,
            runtime_rejected: AtomicBool::new(false),
        }
    }

    fn authorizes(&self, presented: Option<&str>) -> bool {
        match &self.auth_token {
            None => true,
            Some(expected) => presented == Some(expected.as_str()),
        }
    }

    fn authorizes_websocket(
        &self,
        hello_token: Option<&str>,
        handshake_token: Option<&str>,
    ) -> bool {
        self.authorizes(hello_token) || self.authorizes(handshake_token)
    }

    fn claim_runtime_sender(&self, sender: Sender<DevtoolsWireMessage>) -> (u64, bool) {
        let runtime_id = self.next_runtime_id.fetch_add(1, Ordering::AcqRel);
        let mut guard = self
            .runtime_sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let replaced = guard.is_some();
        *guard = Some((runtime_id, sender));
        if replaced {
            self.clear_pending_results();
        }
        (runtime_id, replaced)
    }

    fn clear_runtime_sender(&self, runtime_id: u64) -> bool {
        let mut guard = self
            .runtime_sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if guard.as_ref().is_some_and(|(id, _)| *id == runtime_id) {
            *guard = None;
            return true;
        }
        false
    }

    fn runtime_sender(&self) -> Option<Sender<DevtoolsWireMessage>> {
        self.runtime_sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(|(_, sender)| sender.clone())
    }

    fn register_pending_result(&self, command_id: String, tx: Sender<DevtoolsWireMessage>) {
        self.pending_results
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(command_id, tx);
    }

    fn take_pending_result(&self, command_id: &str) -> Option<Sender<DevtoolsWireMessage>> {
        self.pending_results
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(command_id)
    }

    fn clear_pending_results(&self) {
        self.pending_results
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }

    fn lock_command_forwarding(&self) -> std::sync::MutexGuard<'_, ()> {
        self.command_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn request_shutdown(&self) {
        self.stop_flag.store(true, Ordering::Release);
    }
}

pub fn start_server_on_with_stop(
    project_root: &Path,
    bind_addr: &str,
    _platform: &str,
    stop_flag: Arc<AtomicBool>,
    auth_token: Option<String>,
) -> Result<DevServerHandle> {
    start_server_on_with_roots(
        project_root,
        project_root,
        bind_addr,
        _platform,
        stop_flag,
        auth_token,
    )
}

fn start_server_on_with_roots(
    session_root: &Path,
    content_root: &Path,
    bind_addr: &str,
    _platform: &str,
    stop_flag: Arc<AtomicBool>,
    auth_token: Option<String>,
) -> Result<DevServerHandle> {
    let session = create_session(session_root)?;
    let listener = TcpListener::bind(bind_addr).context("Failed to bind dev websocket")?;
    listener
        .set_nonblocking(true)
        .context("Failed to set dev websocket listener nonblocking")?;
    let ws_addr = listener
        .local_addr()
        .context("Failed to resolve dev websocket address")?;
    let writer = Arc::new(SessionLogWriter::new(&session)?);
    let state = Arc::new(DevServerState::new(
        content_root.to_path_buf(),
        stop_flag.clone(),
        auth_token,
    ));
    let thread_stop_flag = stop_flag.clone();
    let server_thread =
        thread::spawn(move || run_server(listener, writer, state, thread_stop_flag));

    Ok(DevServerHandle {
        session,
        ws_addr,
        stop_flag,
        server_thread: Some(server_thread),
    })
}

pub fn start_server_fixed_with_stop(
    project_root: &Path,
    host: &str,
    platform: &str,
    stop_flag: Arc<AtomicBool>,
    auth_token: Option<String>,
) -> Result<DevServerHandle> {
    let port = dev_port(&project_root.to_string_lossy(), platform);
    match start_server_on_with_stop(
        project_root,
        &format!("{host}:{port}"),
        platform,
        stop_flag.clone(),
        auth_token.clone(),
    ) {
        Ok(handle) => Ok(handle),
        Err(err) => {
            eprintln!(
                "⚠ dev port {port} unavailable ({err:#}); using an OS-assigned port — \
                 reconnection after a client restart may need a re-forward."
            );
            start_server_on_with_stop(
                project_root,
                &format!("{host}:0"),
                platform,
                stop_flag,
                auth_token,
            )
        }
    }
}

pub fn start_server_fixed_with_roots(
    session_root: &Path,
    content_root: &Path,
    host: &str,
    platform: &str,
    stop_flag: Arc<AtomicBool>,
    auth_token: Option<String>,
) -> Result<DevServerHandle> {
    let port = dev_port(&session_root.to_string_lossy(), platform);
    match start_server_on_with_roots(
        session_root,
        content_root,
        &format!("{host}:{port}"),
        platform,
        stop_flag.clone(),
        auth_token.clone(),
    ) {
        Ok(handle) => Ok(handle),
        Err(error) => {
            eprintln!(
                "⚠ dev port {port} unavailable ({error:#}); using an OS-assigned port — \
                 reconnection after a client restart may need a re-forward."
            );
            start_server_on_with_roots(
                session_root,
                content_root,
                &format!("{host}:0"),
                platform,
                stop_flag,
                auth_token,
            )
        }
    }
}

/// Deterministic dev-server port derived from a stable per-app key + platform,
/// so a restarted client reconnects to the same port (and the same adb/hdc
/// forward) without the host re-publishing a fresh OS-assigned port. FNV-1a over
/// `"{app_key}:{platform}"` mapped into a stable private range (39000–39999).
/// `app_key` is any value stable across the host's dev restarts (the dev flow
/// passes the project root path).
pub fn dev_port(app_key: &str, platform: &str) -> u16 {
    const FNV_OFFSET: u32 = 0x811c_9dc5;
    const FNV_PRIME: u32 = 0x0100_0193;
    let mut hash = FNV_OFFSET;
    for byte in format!("{app_key}:{platform}").bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    39_000 + (hash % 1000) as u16
}

fn run_server(
    listener: TcpListener,
    writer: Arc<SessionLogWriter>,
    state: Arc<DevServerState>,
    stop_flag: Arc<AtomicBool>,
) {
    while !stop_flag.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let writer = writer.clone();
                let state = state.clone();
                let stop_flag = stop_flag.clone();
                thread::spawn(move || {
                    if let Err(err) = handle_connection(stream, &writer, &state)
                        && !stop_flag.load(Ordering::Acquire)
                        && !is_expected_websocket_shutdown_error(&err)
                    {
                        eprintln!("[lingxia dev] websocket connection failed: {err}");
                    }
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                eprintln!("[lingxia dev] websocket accept failed: {err}");
                break;
            }
        }
    }
}

fn handle_connection(
    stream: TcpStream,
    writer: &SessionLogWriter,
    state: &DevServerState,
) -> Result<()> {
    if !is_websocket_request(&stream)? {
        return handle_http_connection(stream, state);
    }
    let (mut websocket, handshake_token) = accept_websocket(stream)?;
    let hello = read_wire_message(&mut websocket)?;
    let DevtoolsWireMessage::Hello { role, token } = hello else {
        return Err(anyhow!("First websocket message must be hello"));
    };
    if !state.authorizes_websocket(token.as_deref(), handshake_token.as_deref()) {
        let _ = websocket.close(None);
        if role == DevtoolsPeerRole::Devtool {
            state.runtime_rejected.store(true, Ordering::Release);
            return Err(anyhow!(
                "Rejected a runtime peer with a missing or wrong session token — the running \
                 device host likely predates session tokens; rebuild it from this revision \
                 (or reinstall the Runner)"
            ));
        }
        return Err(anyhow!(
            "Rejected dev websocket client with a missing or wrong session token"
        ));
    }
    match role {
        DevtoolsPeerRole::Devtool => {
            state.runtime_rejected.store(false, Ordering::Release);
            handle_devtool_connection(websocket, writer, state)
        }
        DevtoolsPeerRole::Client => handle_client_connection(websocket, state),
    }
}

/// Capture the token from the HTTP upgrade target as well as accepting it in
/// `Hello`. Device hosts built before the hello-token field still receive and
/// dial the tokened URL, so authenticating the handshake keeps cached builds
/// compatible without allowing tokenless device peers.
#[allow(clippy::result_large_err)] // Tungstenite fixes this type in its callback contract.
fn accept_websocket(stream: TcpStream) -> Result<(WebSocket<TcpStream>, Option<String>)> {
    let mut handshake_token = None;
    let websocket = accept_hdr(stream, |request: &Request, response: Response| {
        handshake_token = lingxia_devtool_protocol::token_from_ws_url(&request.uri().to_string());
        Ok(response)
    })
    .map_err(|err| anyhow!("Failed to accept websocket: {err}"))?;
    Ok((websocket, handshake_token))
}

fn is_websocket_request(stream: &TcpStream) -> Result<bool> {
    let mut buf = [0u8; 2048];
    let n = stream
        .peek(&mut buf)
        .context("Failed to inspect dev server request")?;
    let request = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
    Ok(request.contains("\r\nupgrade: websocket")
        || request.contains("\nupgrade: websocket")
        || request.contains("\r\nsec-websocket-key:")
        || request.contains("\nsec-websocket-key:"))
}

fn handle_http_connection(mut stream: TcpStream, state: &DevServerState) -> Result<()> {
    let request = read_http_request_head(&mut stream)?;
    let Some((method, target)) = parse_http_request_line(&request) else {
        return write_http_error(&mut stream, 400, "Bad Request");
    };
    if method != "GET" {
        return write_http_error(&mut stream, 405, "Method Not Allowed");
    }
    let presented = lingxia_devtool_protocol::token_from_ws_url(target);
    if !state.authorizes(presented.as_deref()) {
        return write_http_error(&mut stream, 403, "Forbidden");
    }
    let target = target
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(target);
    match handle_dev_lxapp_http_request(state, target) {
        Ok((content_type, body)) => write_http_response(&mut stream, 200, content_type, &body),
        Err(err) => {
            let message = err.to_string();
            let status = if message.contains("not found")
                || message.contains("No configured resource bundle")
                || message.contains("not listed")
            {
                404
            } else {
                400
            };
            write_http_error(&mut stream, status, &message)
        }
    }
}

fn read_http_request_head(stream: &mut TcpStream) -> Result<String> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 512];
    loop {
        let n = stream
            .read(&mut chunk)
            .context("Failed to read HTTP request")?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|window| window == b"\r\n\r\n") || buf.len() > 16 * 1024 {
            break;
        }
    }
    String::from_utf8(buf).context("HTTP request was not valid UTF-8")
}

fn parse_http_request_line(request: &str) -> Option<(&str, &str)> {
    let line = request.lines().next()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    Some((method, target))
}

fn handle_dev_lxapp_http_request(
    state: &DevServerState,
    target: &str,
) -> Result<(&'static str, Vec<u8>)> {
    let Some(rest) = target.strip_prefix(DEV_LXAPP_HTTP_PREFIX) else {
        return Err(anyhow!("dev HTTP endpoint not found"));
    };

    if let Some(app_id) = rest.strip_suffix("/manifest.json") {
        let app_id = decode_url_path_component(app_id)?;
        let manifest = super::lxapp_manifest::load_manifest(&state.project_root, &app_id)?;
        let body = serde_json::to_vec_pretty(&manifest)?;
        return Ok(("application/json; charset=utf-8", body));
    }

    let Some((app_id, relative_path)) = rest.split_once("/files/") else {
        return Err(anyhow!("dev HTTP endpoint not found"));
    };
    let app_id = decode_url_path_component(app_id)?;
    let relative_path = decode_url_path_component(relative_path)?;
    let file_path =
        super::lxapp_manifest::resolve_dist_file(&state.project_root, &app_id, &relative_path)?;
    let body = std::fs::read(&file_path)
        .with_context(|| format!("Failed to read {}", file_path.display()))?;
    Ok((content_type_for_path(&relative_path), body))
}

fn decode_url_path_component(value: &str) -> Result<String> {
    urlencoding::decode(value)
        .map(|value| value.into_owned())
        .map_err(|err| anyhow!("invalid URL path encoding: {}", err))
}

fn content_type_for_path(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n",
        body.len()
    )
    .context("Failed to write HTTP response headers")?;
    stream
        .write_all(body)
        .context("Failed to write HTTP response body")?;
    Ok(())
}

fn write_http_error(stream: &mut TcpStream, status: u16, message: &str) -> Result<()> {
    write_http_response(
        stream,
        status,
        "text/plain; charset=utf-8",
        message.as_bytes(),
    )
}

fn handle_devtool_connection(
    mut websocket: WebSocket<TcpStream>,
    writer: &SessionLogWriter,
    state: &DevServerState,
) -> Result<()> {
    websocket
        .get_mut()
        .set_read_timeout(Some(Duration::from_millis(100)))
        .context("Failed to set devtool websocket read timeout")?;

    let (outgoing_tx, outgoing_rx) = mpsc::channel::<DevtoolsWireMessage>();
    let (runtime_id, replaced_runtime) = state.claim_runtime_sender(outgoing_tx);
    if replaced_runtime {
        eprintln!("[lingxia dev] devtool runtime reconnected; replacing stale runtime connection");
    }

    let result = loop {
        if let Err(err) = drain_outgoing_messages(&mut websocket, &outgoing_rx) {
            if is_expected_websocket_shutdown_error(&err) {
                break Ok(());
            }
            break Err(err);
        }

        match websocket.read() {
            Ok(message) => match parse_text_message(message)? {
                ParsedWireMessage::Wire(DevtoolsWireMessage::LogBatch { logs }) => {
                    writer.append_logs(&logs)?;
                }
                ParsedWireMessage::Wire(DevtoolsWireMessage::Result {
                    command_id,
                    ok,
                    data,
                    error,
                }) => {
                    let payload = DevtoolsWireMessage::Result {
                        command_id: command_id.clone(),
                        ok,
                        data,
                        error,
                    };
                    if let Some(tx) = state.take_pending_result(&command_id) {
                        let _ = tx.send(payload);
                    }
                }
                ParsedWireMessage::Wire(
                    DevtoolsWireMessage::Hello { .. } | DevtoolsWireMessage::Command { .. },
                ) => {}
                ParsedWireMessage::Ignored => {}
                ParsedWireMessage::Closed => break Ok(()),
            },
            Err(WsError::Io(err))
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut => {}
            Err(err) if is_expected_tungstenite_shutdown_error(&err) => break Ok(()),
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => break Ok(()),
            Err(err) => break Err(err.into()),
        }
    };

    if state.clear_runtime_sender(runtime_id) {
        state.clear_pending_results();
    }
    result
}

fn is_expected_websocket_shutdown_error(err: &anyhow::Error) -> bool {
    let message = err.to_string();
    message.contains("Connection reset without closing handshake")
        || message.contains("Connection reset by peer")
        || message.contains("Broken pipe")
}

fn is_expected_tungstenite_shutdown_error(err: &WsError) -> bool {
    match err {
        WsError::ConnectionClosed | WsError::AlreadyClosed => true,
        WsError::Io(io) => matches!(
            io.kind(),
            std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::UnexpectedEof
        ),
        _ => err
            .to_string()
            .contains("Connection reset without closing handshake"),
    }
}

fn handle_client_connection(
    mut websocket: WebSocket<TcpStream>,
    state: &DevServerState,
) -> Result<()> {
    let message = read_wire_message(&mut websocket)?;
    let DevtoolsWireMessage::Command {
        command_id,
        handler,
        args,
    } = message
    else {
        return Err(anyhow!("Client websocket must send exactly one command"));
    };

    // `lxapp.build` is an orchestrator-level rebuild operation: the dev server
    // owns the project + build pipeline, so it builds in-process rather than
    // forwarding to the runtime (which has no build toolchain). Works even with
    // no app attached.
    if handler.as_str() == lingxia_devtool_protocol::handlers::lxapp::BUILD {
        let payload = match run_lxapp_build(&state.project_root, args.as_ref()) {
            Ok(()) => DevtoolsWireMessage::Result {
                command_id,
                ok: true,
                data: None,
                error: None,
            },
            Err(err) => DevtoolsWireMessage::Result {
                command_id,
                ok: false,
                data: None,
                error: Some(format!("{err:#}")),
            },
        };
        send_wire_message(&mut websocket, &payload)?;
        let _ = websocket.close(None);
        return Ok(());
    }

    if handler.as_str() == lingxia_devtool_protocol::handlers::ECHO {
        let runtime_connected = state.runtime_sender().is_some();
        send_wire_message(
            &mut websocket,
            &DevtoolsWireMessage::Result {
                command_id,
                ok: true,
                data: Some(serde_json::json!({
                    "runtimeConnected": runtime_connected,
                })),
                error: None,
            },
        )?;
        let _ = websocket.close(None);
        return Ok(());
    }

    if handler.as_str() == lingxia_devtool_protocol::handlers::session::SHUTDOWN {
        send_wire_message(
            &mut websocket,
            &DevtoolsWireMessage::Result {
                command_id,
                ok: true,
                data: None,
                error: None,
            },
        )?;
        state.request_shutdown();
        let _ = websocket.close(None);
        return Ok(());
    }

    let Some(runtime_sender) = state.runtime_sender() else {
        let error = if state.runtime_rejected.load(Ordering::Acquire) {
            "devtool runtime is not connected: a runtime peer was rejected for a missing or \
             wrong session token — the device host likely predates session tokens; rebuild \
             it from this revision (or reinstall the Runner)"
                .to_string()
        } else {
            "devtool runtime is not connected".to_string()
        };
        send_wire_message(
            &mut websocket,
            &DevtoolsWireMessage::Result {
                command_id,
                ok: false,
                data: None,
                error: Some(error),
            },
        )?;
        let _ = websocket.close(None);
        return Ok(());
    };

    let _command_guard = state.lock_command_forwarding();
    let command_timeout = command_timeout(args.as_ref());
    let (result_tx, result_rx) = mpsc::channel::<DevtoolsWireMessage>();
    state.register_pending_result(command_id.clone(), result_tx);
    let bridged_command = DevtoolsWireMessage::Command {
        command_id: command_id.clone(),
        handler,
        args,
    };

    runtime_sender.send(bridged_command).map_err(|_| {
        let _ = state.take_pending_result(&command_id);
        anyhow!("Failed to forward command to devtool")
    })?;

    match result_rx.recv_timeout(command_timeout) {
        Ok(result) => {
            send_wire_message(&mut websocket, &result)?;
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let _ = state.take_pending_result(&command_id);
            send_wire_message(
                &mut websocket,
                &DevtoolsWireMessage::Result {
                    command_id,
                    ok: false,
                    data: None,
                    error: Some("command timed out".to_string()),
                },
            )?;
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = state.take_pending_result(&command_id);
            send_wire_message(
                &mut websocket,
                &DevtoolsWireMessage::Result {
                    command_id,
                    ok: false,
                    data: None,
                    error: Some("command channel disconnected".to_string()),
                },
            )?;
        }
    }
    let _ = websocket.close(None);
    Ok(())
}

fn command_timeout(args: Option<&serde_json::Value>) -> Duration {
    let Some(timeout_ms) = args
        .and_then(|value| value.get("timeout_ms"))
        .and_then(serde_json::Value::as_u64)
    else {
        return DEFAULT_COMMAND_TIMEOUT;
    };
    Duration::from_millis(timeout_ms).saturating_add(COMMAND_TIMEOUT_BUFFER)
}

/// Rebuild the lxapp front-end bundle in-process. The dev orchestrator owns the
/// project + build pipeline; this mirrors `lingxia build` run in a standalone
/// lxapp dir. Output streams to the `lingxia dev` terminal; the client receives
/// only ok/error.
fn run_lxapp_build(project_root: &Path, args: Option<&serde_json::Value>) -> Result<()> {
    let release = args
        .and_then(|value| value.get("release"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let framework = args
        .and_then(|value| value.get("framework"))
        .and_then(serde_json::Value::as_str);
    let mut build_args = vec!["build".to_string()];
    if release {
        build_args.push("--release".to_string());
    }
    if let Some(framework) = framework {
        build_args.push("--framework".to_string());
        build_args.push(framework.to_string());
    }
    crate::lxapp::run_in_dir_for_dev(&build_args, &resolve_lxapp_dir(project_root)?)?;

    // Host dev sessions serve bundles through the generated manifest. Without
    // refreshing it here, the runtime sees the previous dist hash and a
    // successful `lxdev lxapp reload` silently restarts stale assets.
    refresh_lxapp_manifests(project_root)?;
    Ok(())
}

fn refresh_lxapp_manifests(project_root: &Path) -> Result<()> {
    if !project_root.join("lingxia.yaml").is_file() {
        return Ok(());
    }
    let config = crate::config::LingXiaConfig::load(project_root)?;
    super::lxapp_manifest::write_configured_manifests(project_root, &config)?;
    Ok(())
}

/// The directory holding the lxapp to build: the project root itself for a
/// standalone lxapp/lxplugin project, else the host project's home-lxapp
/// bundle path from `lingxia.yaml`.
fn resolve_lxapp_dir(project_root: &Path) -> Result<PathBuf> {
    if project_root.join("lxapp.json").exists() || project_root.join("lxplugin.json").exists() {
        return Ok(project_root.to_path_buf());
    }
    let config = crate::config::LingXiaConfig::load(project_root)?;
    let home_app_id = config
        .app
        .as_ref()
        .map(|app| app.home_app_id.as_str())
        .ok_or_else(|| anyhow!("Missing app settings in lingxia.yaml"))?;
    let path = config
        .resources
        .as_ref()
        .and_then(|resources| {
            resources
                .bundles
                .iter()
                .find(|bundle| bundle.app_id == home_app_id)
        })
        .and_then(|bundle| bundle.path.as_deref())
        .ok_or_else(|| {
            anyhow!("home lxapp '{home_app_id}' has no local path in lingxia.yaml resources")
        })?;
    Ok(project_root.join(path))
}

fn drain_outgoing_messages(
    websocket: &mut WebSocket<TcpStream>,
    rx: &Receiver<DevtoolsWireMessage>,
) -> Result<()> {
    while let Ok(message) = rx.try_recv() {
        send_wire_message(websocket, &message)?;
    }
    Ok(())
}

fn read_wire_message(websocket: &mut WebSocket<TcpStream>) -> Result<DevtoolsWireMessage> {
    loop {
        let message = websocket.read()?;
        match parse_text_message(message)? {
            ParsedWireMessage::Wire(parsed) => return Ok(parsed),
            ParsedWireMessage::Ignored => {}
            ParsedWireMessage::Closed => {
                return Err(anyhow!(
                    "websocket closed before receiving required message"
                ));
            }
        }
    }
}

enum ParsedWireMessage {
    Wire(DevtoolsWireMessage),
    Ignored,
    Closed,
}

fn parse_text_message(message: Message) -> Result<ParsedWireMessage> {
    match message {
        Message::Text(text) => {
            let parsed = serde_json::from_str::<DevtoolsWireMessage>(&text)
                .context("Failed to parse websocket JSON message")?;
            Ok(ParsedWireMessage::Wire(parsed))
        }
        Message::Binary(_) => Err(anyhow!("Binary websocket messages are not supported")),
        Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => Ok(ParsedWireMessage::Ignored),
        Message::Close(_) => Ok(ParsedWireMessage::Closed),
    }
}

pub fn send_wire_message<S>(
    websocket: &mut WebSocket<S>,
    message: &DevtoolsWireMessage,
) -> Result<()>
where
    S: std::io::Read + std::io::Write,
{
    let text = serde_json::to_string(message).context("Failed to encode websocket JSON message")?;
    websocket
        .send(Message::Text(text.into()))
        .context("Failed to send websocket message")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        DevServerState, accept_websocket, dev_port, read_wire_message, refresh_lxapp_manifests,
        send_wire_message,
    };
    use lingxia_devtool_protocol::{DevtoolsPeerRole, DevtoolsWireMessage};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::thread;

    fn authenticated_state() -> DevServerState {
        DevServerState::new(
            PathBuf::from("project"),
            Arc::new(AtomicBool::new(false)),
            Some("secret".to_string()),
        )
    }

    #[test]
    fn reload_manifest_refresh_tracks_rebuilt_dist() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path();
        let dist = project.join("lxapp/dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(
            project.join("lingxia.yaml"),
            "app:\n  projectName: demo-host\n  productName: Demo Host\n  productVersion: 1.0.0\n  platforms: [windows]\n  homeAppId: demo\nresources:\n  bundles:\n    - type: lxapp\n      appId: demo\n      path: lxapp\n",
        )
        .unwrap();
        std::fs::write(
            dist.join("lxapp.json"),
            r#"{"appId":"demo","version":"1.0.0","pages":{}}"#,
        )
        .unwrap();
        std::fs::write(dist.join("logic.js"), "Page({ value: 1 })").unwrap();

        refresh_lxapp_manifests(project).unwrap();
        let manifest_path = project.join(".lingxia/dev/lxapp/demo/manifest.json");
        let first: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();

        std::fs::write(dist.join("logic.js"), "Page({ value: 2 })").unwrap();
        refresh_lxapp_manifests(project).unwrap();
        let second: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();

        assert_ne!(first["distHash"], second["distHash"]);
    }

    #[test]
    fn websocket_auth_accepts_handshake_token_for_legacy_runner() {
        let state = authenticated_state();

        assert!(state.authorizes_websocket(None, Some("secret")));
        assert!(state.authorizes_websocket(Some("secret"), None));
        assert!(!state.authorizes_websocket(None, None));
        assert!(!state.authorizes_websocket(Some("wrong"), Some("wrong")));
    }

    #[test]
    fn websocket_upgrade_captures_token_for_legacy_runner_hello() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let (mut websocket, handshake_token) = accept_websocket(stream).unwrap();
            let hello = read_wire_message(&mut websocket).unwrap();
            let DevtoolsWireMessage::Hello { role, token } = hello else {
                panic!("expected hello");
            };
            assert_eq!(role, DevtoolsPeerRole::Devtool);
            assert_eq!(token, None);
            authenticated_state().authorizes_websocket(token.as_deref(), handshake_token.as_deref())
        });

        let url = format!("ws://{addr}/?token=secret");
        let (mut websocket, _) = tungstenite::connect(&url).unwrap();
        send_wire_message(
            &mut websocket,
            &DevtoolsWireMessage::Hello {
                role: DevtoolsPeerRole::Devtool,
                token: None,
            },
        )
        .unwrap();

        assert!(server.join().unwrap());
    }

    #[test]
    fn dev_port_is_stable_and_in_range() {
        let port = dev_port("com.example.app", "android");
        assert_eq!(port, dev_port("com.example.app", "android"));
        assert!((39_000..40_000).contains(&port));
    }

    #[test]
    fn dev_port_distinguishes_platforms() {
        assert_ne!(
            dev_port("com.example.app", "android"),
            dev_port("com.example.app", "ios")
        );
    }
}
