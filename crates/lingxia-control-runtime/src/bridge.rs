//! The development websocket bridge.
//!
//! Only development hosts carry this: it dials the `lingxia dev` session over
//! TCP and streams logs at it. A shipped product that wants automation uses
//! [`crate::local_control`] instead, which reaches the same [`crate::dispatch`]
//! without any of the network stack below.

use lingxia_control_protocol::dev_session::{
    DEV_SESSION_PROTOCOL_VERSION, DevSessionEvent, DevSessionLog, DevSessionLogLevel,
    DevSessionMessage, DevSessionRole, capabilities,
};
use lingxia_log::{AttachedLogStream, LogLevel, LogMessage, LogTag, attach_log_stream_default};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};
use tungstenite::protocol::Message;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Error as WsError, WebSocket};

use crate::dispatch;

const DEV_WS_URL_ENV: &str = "LINGXIA_DEV_WS_URL";
const RUNNER_DISPLAY_LANGUAGE_ENV: &str = "LINGXIA_RUNNER_DISPLAY_LANGUAGE";
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const SESSION_STALE_AFTER: Duration = Duration::from_secs(15);

struct RunnerDisplayLanguageLease(Option<lxapp::DisplayLanguageSessionOwner>);

impl RunnerDisplayLanguageLease {
    fn acquire() -> Self {
        let owner = std::env::var(RUNNER_DISPLAY_LANGUAGE_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .and_then(
                |value| match value.parse::<lxapp::DisplayLanguagePreference>() {
                    Ok(preference) => {
                        Some(lxapp::install_display_language_session_override(preference))
                    }
                    Err(error) => {
                        log::warn!("Ignoring invalid Runner display language: {error}");
                        None
                    }
                },
            );
        Self(owner)
    }
}

impl Drop for RunnerDisplayLanguageLease {
    fn drop(&mut self) {
        if let Some(owner) = self.0.take() {
            lxapp::clear_display_language_session_override(owner);
        }
    }
}

pub fn start_dev_session_bridge_from_env() {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }

    let ws_url = match dev_ws_url() {
        Some(value) => value,
        None => {
            log::info!("Devtool bridge disabled because no dev websocket URL is configured");
            return;
        }
    };

    thread::spawn(move || run_dev_bridge(ws_url));
}

fn dev_ws_url() -> Option<String> {
    std::env::var(DEV_WS_URL_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            lingxia_app_context::app_config()
                .and_then(|config| config.dev_ws_url.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
}

/// Connect with explicit TCP + handshake timeouts. A plain `connect` blocks
/// forever when a stale port-forward listener accepts the socket but the far
/// side is gone — the bridge then never retries.
fn connect_with_timeout(
    ws_url: &str,
) -> Result<WebSocket<MaybeTlsStream<std::net::TcpStream>>, WsError> {
    let authority = ws_url
        .trim_start_matches("ws://")
        .split(['/', '?'])
        .next()
        .unwrap_or_default();
    let addr = authority.parse::<std::net::SocketAddr>().map_err(|err| {
        WsError::Url(tungstenite::error::UrlError::UnableToConnect(
            err.to_string(),
        ))
    })?;
    let stream =
        std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(WsError::Io)?;
    prevent_sigpipe(&stream).map_err(WsError::Io)?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(WsError::Io)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(WsError::Io)?;
    let (websocket, _) =
        tungstenite::client(ws_url, MaybeTlsStream::Plain(stream)).map_err(|err| match err {
            tungstenite::HandshakeError::Failure(err) => err,
            // A read timeout during the handshake surfaces as Interrupted —
            // exactly the stale-tunnel case this timeout exists to catch.
            tungstenite::HandshakeError::Interrupted(_) => WsError::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "websocket handshake timed out",
            )),
        })?;
    Ok(websocket)
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn prevent_sigpipe(stream: &std::net::TcpStream) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;

    let enabled: libc::c_int = 1;
    // Rust is linked as a static library into Apple hosts, so its executable
    // startup cannot install the usual SIGPIPE disposition. Keep a stale dev
    // websocket write local to this connection instead of terminating the app.
    let result = unsafe {
        libc::setsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_NOSIGPIPE,
            std::ptr::from_ref(&enabled).cast(),
            std::mem::size_of_val(&enabled) as libc::socklen_t,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
fn prevent_sigpipe(_stream: &std::net::TcpStream) -> std::io::Result<()> {
    Ok(())
}

fn run_dev_bridge(ws_url: String) {
    let mut connect_failures = 0u32;
    loop {
        match connect_with_timeout(ws_url.as_str()) {
            Ok(mut websocket) => {
                if connect_failures > 0 {
                    log::info!(
                        "Connected devtool websocket after {} failed attempts",
                        connect_failures
                    );
                }
                connect_failures = 0;
                if let Err(err) = send_wire_message(
                    &mut websocket,
                    &DevSessionMessage::Hello {
                        version: DEV_SESSION_PROTOCOL_VERSION,
                        role: DevSessionRole::Runtime,
                        capabilities: vec![
                            capabilities::REQUESTS.to_string(),
                            capabilities::LOG_EVENTS.to_string(),
                        ],
                    },
                ) {
                    log::warn!("Failed to send devtool hello: {}", err);
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }

                // The override belongs to this concrete websocket session.
                // Dropping this lease clears normal disconnects, failed setup,
                // crashes of the peer, and every reconnect attempt.
                let _display_language_lease = RunnerDisplayLanguageLease::acquire();

                configure_read_timeout(&mut websocket);

                let attached = match attach_log_stream_default() {
                    Ok(attached) => attached,
                    Err(err) => {
                        log::warn!("Failed to attach devtool log stream: {}", err);
                        drop(_display_language_lease);
                        thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                };

                if let Err(err) = bridge_loop(&mut websocket, attached) {
                    log::warn!("Devtool bridge disconnected: {}", err);
                }
            }
            Err(err) => {
                connect_failures = connect_failures.saturating_add(1);
                log_connect_failure(connect_failures, &err);
            }
        }

        thread::sleep(reconnect_delay(connect_failures));
    }
}

fn reconnect_delay(connect_failures: u32) -> Duration {
    match connect_failures {
        0 => Duration::from_millis(500),
        1 => Duration::from_secs(1),
        2 => Duration::from_secs(2),
        _ => Duration::from_secs(5),
    }
}

fn log_connect_failure(attempt: u32, err: &WsError) {
    if attempt == 1 {
        log::warn!(
            "Failed to connect devtool websocket; retrying in background: {}",
            err
        );
    } else if attempt.is_multiple_of(12) {
        log::warn!(
            "Still unable to connect devtool websocket after {} attempts: {}",
            attempt,
            err
        );
    } else {
        log::debug!(
            "Failed to connect devtool websocket attempt {}: {}",
            attempt,
            err
        );
    }
}

fn bridge_loop(
    websocket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    attached: AttachedLogStream,
) -> Result<(), String> {
    let (recent, mut receiver) = attached.into_parts();
    let mut last_received = Instant::now();
    let mut last_ping = Instant::now();
    for chunk in recent.chunks(128) {
        send_log_batch(websocket, chunk)?;
    }

    loop {
        let mut batch = Vec::new();
        while batch.len() < 64 {
            match receiver.try_recv() {
                Ok(message) => batch.push(message),
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(skipped)) => {
                    log::warn!("Devtool log stream lagged and skipped {} messages", skipped);
                    break;
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                    return Err("log stream closed".to_string());
                }
            }
        }

        if !batch.is_empty() {
            send_log_batch(websocket, &batch)?;
        }

        match websocket.read() {
            Ok(message) => {
                last_received = Instant::now();
                if let Some(wire) = parse_wire_message(message)? {
                    handle_incoming_message(websocket, wire)?;
                }
            }
            Err(WsError::Io(err)) if is_retryable_read_error(&err) => {}
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                return Err("websocket closed".to_string());
            }
            Err(err) => return Err(err.to_string()),
        }

        if last_received.elapsed() >= SESSION_STALE_AFTER {
            return Err("devtool websocket heartbeat timed out".to_string());
        }
        if last_ping.elapsed() >= HEARTBEAT_INTERVAL {
            websocket
                .send(Message::Ping(Vec::new().into()))
                .map_err(|error| error.to_string())?;
            last_ping = Instant::now();
        }

        thread::sleep(Duration::from_millis(50));
    }
}

fn is_retryable_read_error(err: &std::io::Error) -> bool {
    if matches!(
        err.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ) {
        return true;
    }

    // A timed socket read can surface as ERROR_IO_PENDING instead of
    // WouldBlock on Windows. The overlapped read is still healthy.
    #[cfg(windows)]
    if err.raw_os_error() == Some(997) {
        return true;
    }

    false
}

fn handle_incoming_message(
    websocket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    message: DevSessionMessage,
) -> Result<(), String> {
    let DevSessionMessage::Request(request) = message else {
        return Ok(());
    };
    send_wire_message(websocket, &DevSessionMessage::Response(dispatch(request)))
}

fn send_log_batch(
    websocket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    logs: &[LogMessage],
) -> Result<(), String> {
    send_wire_message(
        websocket,
        &DevSessionMessage::EventBatch {
            events: logs.iter().map(dev_session_log_event).collect(),
        },
    )
}

fn dev_session_log_event(value: &LogMessage) -> DevSessionEvent {
    DevSessionEvent::log(
        value.timestamp_ms,
        dev_session_log_origin(value.tag),
        DevSessionLog {
            level: dev_session_log_level(value.level),
            appid: value.appid.clone(),
            path: value.path.clone(),
            target: value.target.clone(),
            message: value.message.clone(),
            attributes: Default::default(),
        },
    )
    .expect("DevSessionLog must serialize")
}

fn dev_session_log_level(value: LogLevel) -> DevSessionLogLevel {
    match value {
        LogLevel::Verbose => DevSessionLogLevel::Verbose,
        LogLevel::Debug => DevSessionLogLevel::Debug,
        LogLevel::Info => DevSessionLogLevel::Info,
        LogLevel::Warn => DevSessionLogLevel::Warn,
        LogLevel::Error => DevSessionLogLevel::Error,
    }
}

fn dev_session_log_origin(value: LogTag) -> &'static str {
    match value {
        LogTag::Native => "native",
        LogTag::WebViewConsole => "lxview",
        LogTag::LxAppServiceConsole => "lxlogic",
        LogTag::BrowserConsole => "browser",
        LogTag::Automation => "automation",
    }
}

fn send_wire_message(
    websocket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    message: &DevSessionMessage,
) -> Result<(), String> {
    let text = serde_json::to_string(message).map_err(|err| err.to_string())?;
    websocket
        .send(Message::Text(text.into()))
        .map_err(|err| err.to_string())
}

fn parse_wire_message(message: Message) -> Result<Option<DevSessionMessage>, String> {
    match message {
        Message::Text(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|err| err.to_string()),
        Message::Ping(_) | Message::Pong(_) | Message::Close(_) | Message::Frame(_) => Ok(None),
        Message::Binary(_) => Err("binary websocket messages are not supported".to_string()),
    }
}

fn configure_read_timeout(websocket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>) {
    if let MaybeTlsStream::Plain(stream) = websocket.get_mut() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
    }
}

#[cfg(test)]
mod tests {
    use super::is_retryable_read_error;

    #[test]
    fn retries_nonblocking_socket_reads() {
        for kind in [std::io::ErrorKind::WouldBlock, std::io::ErrorKind::TimedOut] {
            assert!(is_retryable_read_error(&std::io::Error::from(kind)));
        }
        assert!(!is_retryable_read_error(&std::io::Error::from(
            std::io::ErrorKind::ConnectionReset
        )));
    }

    #[cfg(windows)]
    #[test]
    fn retries_pending_windows_overlapped_reads() {
        assert!(is_retryable_read_error(&std::io::Error::from_raw_os_error(
            997
        )));
    }
}
