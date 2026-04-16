use super::log_store::{DevLogSession, create_session};
use anyhow::{Context, Result, anyhow};
use lingxia_devtool_protocol::{DevtoolsLogMessage, DevtoolsPeerRole, DevtoolsWireMessage};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tungstenite::protocol::Message;
use tungstenite::{Error as WsError, WebSocket, accept};

const SERVER_BIND_ADDR: &str = "127.0.0.1:0";
const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const COMMAND_TIMEOUT_BUFFER: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct DevServerHandle {
    session: DevLogSession,
    ws_addr: SocketAddr,
    stop_flag: Arc<AtomicBool>,
    server_thread: Option<JoinHandle<()>>,
}

impl DevServerHandle {
    pub fn ws_url(&self) -> String {
        format!("ws://{}", self.ws_addr)
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
    runtime_sender: Mutex<Option<Sender<DevtoolsWireMessage>>>,
    pending_results: Mutex<std::collections::HashMap<String, Sender<DevtoolsWireMessage>>>,
    command_lock: Mutex<()>,
}

impl DevServerState {
    fn new() -> Self {
        Self {
            runtime_sender: Mutex::new(None),
            pending_results: Mutex::new(std::collections::HashMap::new()),
            command_lock: Mutex::new(()),
        }
    }

    fn try_claim_runtime_sender(&self, sender: Sender<DevtoolsWireMessage>) -> bool {
        let mut guard = self
            .runtime_sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if guard.is_some() {
            return false;
        }
        *guard = Some(sender);
        true
    }

    fn clear_runtime_sender(&self) {
        let mut guard = self
            .runtime_sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = None;
    }

    fn runtime_sender(&self) -> Option<Sender<DevtoolsWireMessage>> {
        self.runtime_sender
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
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
}

pub fn start_server(project_root: &Path) -> Result<DevServerHandle> {
    let session = create_session(project_root)?;
    let listener = TcpListener::bind(SERVER_BIND_ADDR).context("Failed to bind dev websocket")?;
    listener
        .set_nonblocking(true)
        .context("Failed to set dev websocket listener nonblocking")?;
    let ws_addr = listener
        .local_addr()
        .context("Failed to resolve dev websocket address")?;
    let writer = Arc::new(SessionLogWriter::new(&session)?);
    let state = Arc::new(DevServerState::new());
    let stop_flag = Arc::new(AtomicBool::new(false));
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
                    if let Err(err) = handle_connection(stream, &writer, &state) {
                        if !stop_flag.load(Ordering::Acquire)
                            && !is_expected_websocket_shutdown_error(&err)
                        {
                            eprintln!("[lingxia dev] websocket connection failed: {err}");
                        }
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
    let mut websocket = accept(stream).context("Failed to accept websocket")?;
    let hello = read_wire_message(&mut websocket)?;
    let DevtoolsWireMessage::Hello { role } = hello else {
        return Err(anyhow!("First websocket message must be hello"));
    };
    match role {
        DevtoolsPeerRole::Devtool => handle_devtool_connection(websocket, writer, state),
        DevtoolsPeerRole::Client => handle_client_connection(websocket, state),
    }
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
    if !state.try_claim_runtime_sender(outgoing_tx) {
        let _ = websocket.close(None);
        return Err(anyhow!("a devtool runtime is already connected"));
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

    state.clear_runtime_sender();
    state.clear_pending_results();
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

    let Some(runtime_sender) = state.runtime_sender() else {
        send_wire_message(
            &mut websocket,
            &DevtoolsWireMessage::Result {
                command_id,
                ok: false,
                data: None,
                error: Some("devtool runtime is not connected".to_string()),
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
