//! Devtool runtime bridge and protocol helpers for LingXia apps.
//!
//! Host libraries decide how this service is installed into their own
//! `HostAddon`; this crate only exposes the service entry points.

use std::sync::OnceLock;
use std::thread;
use std::time::Duration;
use tungstenite::protocol::Message;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Error as WsError, WebSocket, connect};

mod protocol;

pub use protocol::{
    DevtoolsLogLevel, DevtoolsLogMessage, DevtoolsLogSource, DevtoolsPeerRole, DevtoolsWireMessage,
};

const DEV_WS_URL_ENV: &str = "LINGXIA_DEV_WS_URL";
const ECHO_HANDLER: &str = "echo";

pub fn start_devtool_bridge_from_env() {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }

    let ws_url = match std::env::var(DEV_WS_URL_ENV) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            log::info!(
                "Devtool bridge disabled because {} is not set",
                DEV_WS_URL_ENV
            );
            return;
        }
    };

    thread::spawn(move || run_dev_bridge(ws_url));
}

fn run_dev_bridge(ws_url: String) {
    loop {
        match connect(ws_url.as_str()) {
            Ok((mut websocket, _)) => {
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

                let attached = match lingxia::log::attach_log_stream_default() {
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
                log::warn!("Failed to connect devtool websocket: {}", err);
            }
        }

        thread::sleep(Duration::from_millis(500));
    }
}

fn bridge_loop(
    websocket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    attached: lingxia::log::AttachedLogStream,
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
                Err(lingxia::tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                Err(lingxia::tokio::sync::broadcast::error::TryRecvError::Lagged(skipped)) => {
                    log::warn!("Devtool log stream lagged and skipped {} messages", skipped);
                    break;
                }
                Err(lingxia::tokio::sync::broadcast::error::TryRecvError::Closed) => {
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

    let result = match handler.as_str() {
        ECHO_HANDLER => DevtoolsWireMessage::Result {
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
    };
    send_wire_message(websocket, &result)
}

fn send_log_batch(
    websocket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    logs: &[lingxia::log::LogMessage],
) -> Result<(), String> {
    send_wire_message(
        websocket,
        &DevtoolsWireMessage::LogBatch {
            logs: logs.iter().map(DevtoolsLogMessage::from).collect(),
        },
    )
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
    match websocket.get_mut() {
        MaybeTlsStream::Plain(stream) => {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
        }
        _ => {}
    }
}
