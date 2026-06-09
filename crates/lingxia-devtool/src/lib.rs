//! Devtool runtime bridge and protocol helpers for LingXia apps.
//!
//! Host libraries decide how this service is installed into their own
//! `HostAddon`; this crate only exposes the service entry points.

use lingxia_log::{AttachedLogStream, LogLevel, LogMessage, LogTag, attach_log_stream_default};
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;
use tungstenite::protocol::Message;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Error as WsError, WebSocket, connect};

mod app;
mod browser;
mod lxapp;
mod lxapp_nav;
mod lxapp_page;
mod util;

pub use lingxia_devtool_protocol::{
    DevtoolsLogLevel, DevtoolsLogMessage, DevtoolsLogSource, DevtoolsPeerRole, DevtoolsWireMessage,
    handlers,
};

const DEV_WS_URL_ENV: &str = "LINGXIA_DEV_WS_URL";

pub fn start_devtool_bridge_from_env() {
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

fn run_dev_bridge(ws_url: String) {
    let mut connect_failures = 0u32;
    loop {
        match connect(ws_url.as_str()) {
            Ok((mut websocket, _)) => {
                if connect_failures > 0 {
                    log::info!(
                        "Connected devtool websocket after {} failed attempts",
                        connect_failures
                    );
                }
                connect_failures = 0;
                if let Err(err) = send_wire_message(
                    &mut websocket,
                    &DevtoolsWireMessage::Hello {
                        role: DevtoolsPeerRole::Devtool,
                    },
                ) {
                    log::warn!("Failed to send devtool hello: {}", err);
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }

                configure_read_timeout(&mut websocket);

                let attached = match attach_log_stream_default() {
                    Ok(attached) => attached,
                    Err(err) => {
                        log::warn!("Failed to attach devtool log stream: {}", err);
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
                if let Some(wire) = parse_wire_message(message)? {
                    handle_incoming_message(websocket, wire)?;
                }
            }
            Err(WsError::Io(err))
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut => {}
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                return Err("websocket closed".to_string());
            }
            Err(err) => return Err(err.to_string()),
        }

        thread::sleep(Duration::from_millis(50));
    }
}

fn handle_incoming_message(
    websocket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    message: DevtoolsWireMessage,
) -> Result<(), String> {
    let DevtoolsWireMessage::Command {
        command_id,
        handler,
        args,
    } = message
    else {
        return Ok(());
    };

    let result = if let Some(result) = app::handle_app_command(&handler, args.clone()) {
        command_result(command_id, result)
    } else if let Some(result) = browser::handle_browser_command(&handler, args.clone()) {
        command_result(command_id, result)
    } else if let Some(result) = lxapp_nav::handle_lxapp_nav_command(&handler, args.clone()) {
        command_result(command_id, result)
    } else if let Some(result) = lxapp_page::handle_lxapp_page_command(&handler, args.clone()) {
        command_result(command_id, result)
    } else if let Some(result) = lxapp::handle_lxapp_command(&handler, args.clone()) {
        command_result(command_id, result)
    } else {
        match handler.as_str() {
            handlers::ECHO => DevtoolsWireMessage::Result {
                command_id,
                ok: true,
                data: args,
                error: None,
            },
            other => DevtoolsWireMessage::Result {
                command_id,
                ok: false,
                data: None,
                error: Some(format!("unknown handler: {}", other)),
            },
        }
    };
    send_wire_message(websocket, &result)
}

fn command_result(
    command_id: String,
    result: Result<Option<serde_json::Value>, String>,
) -> DevtoolsWireMessage {
    match result {
        Ok(data) => DevtoolsWireMessage::Result {
            command_id,
            ok: true,
            data,
            error: None,
        },
        Err(error) => DevtoolsWireMessage::Result {
            command_id,
            ok: false,
            data: None,
            error: Some(error),
        },
    }
}

fn send_log_batch(
    websocket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    logs: &[LogMessage],
) -> Result<(), String> {
    send_wire_message(
        websocket,
        &DevtoolsWireMessage::LogBatch {
            logs: logs.iter().map(devtools_log_message).collect(),
        },
    )
}

fn devtools_log_message(value: &LogMessage) -> DevtoolsLogMessage {
    DevtoolsLogMessage {
        timestamp_ms: value.timestamp_ms,
        source: devtools_log_source(value.tag),
        level: devtools_log_level(value.level),
        appid: value.appid.clone(),
        path: value.path.clone(),
        message: value.message.clone(),
    }
}

fn devtools_log_level(value: LogLevel) -> DevtoolsLogLevel {
    match value {
        LogLevel::Verbose => DevtoolsLogLevel::Verbose,
        LogLevel::Debug => DevtoolsLogLevel::Debug,
        LogLevel::Info => DevtoolsLogLevel::Info,
        LogLevel::Warn => DevtoolsLogLevel::Warn,
        LogLevel::Error => DevtoolsLogLevel::Error,
    }
}

fn devtools_log_source(value: LogTag) -> DevtoolsLogSource {
    match value {
        LogTag::Native => DevtoolsLogSource::Native,
        LogTag::WebViewConsole => DevtoolsLogSource::WebViewConsole,
        LogTag::LxAppServiceConsole => DevtoolsLogSource::LxAppServiceConsole,
    }
}

fn send_wire_message(
    websocket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    message: &DevtoolsWireMessage,
) -> Result<(), String> {
    let text = serde_json::to_string(message).map_err(|err| err.to_string())?;
    websocket
        .send(Message::Text(text.into()))
        .map_err(|err| err.to_string())
}

fn parse_wire_message(message: Message) -> Result<Option<DevtoolsWireMessage>, String> {
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
