use anyhow::{Context, Result};
use lingxia_control_protocol::{
    ControlRequest,
    dev_session::{
        DEV_SESSION_MAX_MESSAGE_BYTES, DEV_SESSION_PROTOCOL_VERSION, DevSessionMessage,
        DevSessionRole, capabilities,
    },
};
use serde_json::Value;
use std::io::ErrorKind;
use std::net::TcpStream;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tungstenite::protocol::Message;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{WebSocket, connect};

/// A command that ran out of time without an answer. Distinct from a transport
/// error because the two mean opposite things about the runtime: a silent
/// runtime is usually one busy inside a long spec, while a broken socket is one
/// that is gone. Callers that poll need to tell them apart to know whether
/// waiting longer is worth anything.
#[derive(Debug)]
pub struct CommandTimeout {
    pub handler: String,
    pub waited: Duration,
}

impl std::fmt::Display for CommandTimeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Timed out after {:?} waiting for a dev websocket response to {}",
            self.waited, self.handler
        )
    }
}

impl std::error::Error for CommandTimeout {}

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const COMMAND_TIMEOUT_BUFFER: Duration = Duration::from_secs(5);
const SHORT_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a single read may block. The command's own deadline is enforced by
/// the read loop, so this only decides how often that deadline is rechecked.
const READ_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Longest a status poll should sit on one socket. The dev server abandons a
/// forward at [`DEFAULT_COMMAND_TIMEOUT`]; returning slightly earlier lets the
/// caller decide on its own clock instead of opening another connection while
/// that forward still holds the command lock.
pub(crate) fn max_poll_wait() -> Duration {
    DEFAULT_COMMAND_TIMEOUT.saturating_sub(COMMAND_TIMEOUT_BUFFER)
}

/// A request the host answered with an error. Displays as the host's message,
/// as a plain error did; callers that need the code or data downcast to it.
#[derive(Debug)]
pub struct CommandError {
    pub code: String,
    pub message: String,
    pub data: Option<Value>,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

pub fn execute_command(
    ws_url: &str,
    handler: impl Into<String>,
    args: Option<Value>,
) -> Result<Option<Value>> {
    execute_command_until(ws_url, handler, args, None, &|| false)
}

/// Like [`execute_command`], with an explicit wait and a chance to return
/// early. `stop` is checked each time a read comes back empty, so a caller
/// can notice Ctrl-C without tearing the socket down on a timer of its own.
/// A `None` timeout keeps the handler's usual budget.
pub(crate) fn execute_command_until(
    ws_url: &str,
    handler: impl Into<String>,
    args: Option<Value>,
    timeout_override: Option<Duration>,
    stop: &dyn Fn() -> bool,
) -> Result<Option<Value>> {
    let handler = handler.into();
    let timeout =
        timeout_override.unwrap_or_else(|| default_command_timeout(&handler, args.as_ref()));
    let (mut websocket, _) =
        connect(ws_url).with_context(|| format!("Failed to connect dev websocket: {ws_url}"))?;
    configure_read_timeout(&mut websocket, READ_POLL_INTERVAL.min(timeout));
    websocket.set_config(|config| {
        config.max_frame_size = Some(DEV_SESSION_MAX_MESSAGE_BYTES);
        config.max_message_size = Some(DEV_SESSION_MAX_MESSAGE_BYTES);
    });

    send_wire_message(
        &mut websocket,
        &DevSessionMessage::Hello {
            version: DEV_SESSION_PROTOCOL_VERSION,
            role: DevSessionRole::Controller,
            capabilities: vec![capabilities::REQUESTS.to_string()],
        },
    )?;

    let command_id = command_id();
    send_wire_message(
        &mut websocket,
        &DevSessionMessage::Request(ControlRequest {
            id: command_id.clone(),
            method: handler.clone(),
            params: args,
        }),
    )?;

    let started = Instant::now();
    let deadline = started + timeout;
    loop {
        if stop() || Instant::now() >= deadline {
            return Err(CommandTimeout {
                handler: handler.clone(),
                waited: started.elapsed(),
            }
            .into());
        }
        let message = match websocket.read() {
            Ok(message) => message,
            /* A quiet socket is not a lost one. The read timeout above bounds
             * one syscall, not the command, and on Unix it surfaces as
             * `WouldBlock` (EAGAIN) — indistinguishable, as an io::Error, from
             * a real transport failure. Reporting it as one ended whole test
             * runs that were merely between events. EINTR is never a failure
             * either, and a timed read on Windows can surface as
             * ERROR_IO_PENDING (997) with neither of those kinds. tungstenite
             * keeps its partial frame across all of them, so resuming the
             * read is safe. */
            Err(tungstenite::Error::Io(err)) if is_quiet_read(&err) => continue,
            Err(err) => return Err(err).context("Failed to read dev websocket response"),
        };
        let Message::Text(text) = message else {
            continue;
        };
        let wire: DevSessionMessage =
            serde_json::from_str(&text).context("Failed to parse dev websocket response")?;
        let DevSessionMessage::Response(response) = wire else {
            continue;
        };
        if response.id != command_id {
            continue;
        }
        if let Some(error) = response.error {
            return Err(CommandError {
                code: error.code,
                message: error.message,
                data: error.data,
            }
            .into());
        }
        return Ok(response.result);
    }
}

fn send_wire_message(
    websocket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    message: &DevSessionMessage,
) -> Result<()> {
    let text = serde_json::to_string(message).context("Failed to encode dev websocket message")?;
    websocket
        .send(Message::Text(text.into()))
        .context("Failed to send dev websocket message")
}

fn configure_read_timeout(websocket: &mut WebSocket<MaybeTlsStream<TcpStream>>, timeout: Duration) {
    if let MaybeTlsStream::Plain(stream) = websocket.get_mut() {
        let _ = stream.set_read_timeout(Some(timeout));
    }
}

fn default_command_timeout(handler: &str, args: Option<&Value>) -> Duration {
    if matches!(handler, "session.test.poll" | "session.test.cancel") {
        SHORT_COMMAND_TIMEOUT
    } else {
        command_timeout(args)
    }
}

/// A read that only means "nothing yet". Same set the dev-session bridge
/// already retries, plus `Interrupted`: the host learned that Windows timed
/// reads can arrive as os error 997 rather than `WouldBlock` or `TimedOut`.
fn is_quiet_read(err: &std::io::Error) -> bool {
    if matches!(
        err.kind(),
        ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
    ) {
        return true;
    }
    #[cfg(windows)]
    if err.raw_os_error() == Some(997) {
        return true;
    }
    false
}

fn command_timeout(args: Option<&Value>) -> Duration {
    let Some(timeout_ms) = args
        .and_then(|value| value.get("timeout_ms"))
        .and_then(Value::as_u64)
    else {
        return DEFAULT_COMMAND_TIMEOUT;
    };
    Duration::from_millis(timeout_ms).saturating_add(COMMAND_TIMEOUT_BUFFER)
}

fn command_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("lxdev-{nanos}")
}

/// The dev websocket as a [`Transport`], so the shared command tables can run
/// over a session without knowing one exists.
pub struct DevSession<'a> {
    ws_url: &'a str,
}

impl<'a> DevSession<'a> {
    pub fn new(ws_url: &'a str) -> Self {
        Self { ws_url }
    }
}

impl lingxia_control_commands::transport::Transport for DevSession<'_> {
    fn request(&self, method: &str, params: Option<Value>) -> Result<Option<Value>> {
        execute_command(self.ws_url, method, params)
    }
}

#[cfg(test)]
mod large_frame_tests {
    use super::*;
    use lingxia_control_protocol::ControlResponse;

    #[test]
    fn receives_artifact_frames_above_tungstenite_default() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let bytes = 17 * 1024 * 1024;
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            loop {
                let message = socket.read().unwrap();
                if let DevSessionMessage::Request(request) =
                    serde_json::from_str(message.to_text().unwrap()).unwrap()
                {
                    let response = DevSessionMessage::Response(ControlResponse::success(
                        request.id,
                        Some(Value::String("a".repeat(bytes))),
                    ));
                    socket
                        .send(Message::Text(
                            serde_json::to_string(&response).unwrap().into(),
                        ))
                        .unwrap();
                    break;
                }
            }
        });
        let result = execute_command(&format!("ws://{address}"), "session.test.poll", None)
            .unwrap()
            .unwrap();
        assert_eq!(result.as_str().unwrap().len(), bytes);
        server.join().unwrap();
    }

    /// A server that never answers must surface a `CommandTimeout`, not a bare
    /// error: the test run loop waits through a timeout and only gives up on a
    /// transport failure, so the two have to stay distinguishable.
    #[test]
    fn a_silent_server_reports_a_timeout_not_a_transport_error() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            // Read the request, then answer nothing until the client gives up.
            let _ = socket.read();
            std::thread::sleep(Duration::from_secs(7));
        });
        let err = execute_command(&format!("ws://{address}"), "session.test.poll", None)
            .expect_err("a server that never answers must time out");
        assert!(
            err.downcast_ref::<CommandTimeout>().is_some(),
            "expected a CommandTimeout, got: {err:#}"
        );
        drop(server);
    }

    /// A server that stays silent past one read timeout and only then answers.
    /// The reply must still arrive: a quiet socket used to be reported as a
    /// lost connection, which ended whole test runs between events.
    #[test]
    fn keeps_waiting_through_a_read_timeout() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            loop {
                let message = socket.read().unwrap();
                if let DevSessionMessage::Request(request) =
                    serde_json::from_str(message.to_text().unwrap()).unwrap()
                {
                    // Longer than READ_POLL_INTERVAL, well inside the command
                    // timeout for `session.test.poll`.
                    std::thread::sleep(READ_POLL_INTERVAL * 3);
                    let response = DevSessionMessage::Response(ControlResponse::success(
                        request.id,
                        Some(Value::String("late".into())),
                    ));
                    socket
                        .send(Message::Text(
                            serde_json::to_string(&response).unwrap().into(),
                        ))
                        .unwrap();
                    break;
                }
            }
        });
        let result = execute_command(&format!("ws://{address}"), "session.test.poll", None)
            .unwrap()
            .unwrap();
        assert_eq!(result.as_str().unwrap(), "late");
        server.join().unwrap();
    }

    #[test]
    fn a_quiet_read_is_not_a_transport_failure() {
        for kind in [
            ErrorKind::WouldBlock,
            ErrorKind::TimedOut,
            ErrorKind::Interrupted,
        ] {
            assert!(is_quiet_read(&std::io::Error::from(kind)));
        }
        assert!(!is_quiet_read(&std::io::Error::from(
            ErrorKind::ConnectionReset
        )));
    }

    #[cfg(windows)]
    #[test]
    fn a_pending_windows_read_is_quiet() {
        assert!(is_quiet_read(&std::io::Error::from_raw_os_error(997)));
    }
}
