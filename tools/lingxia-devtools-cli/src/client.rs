use anyhow::{Context, Result, anyhow};
use lingxia_devtool_protocol::{DevtoolsPeerRole, DevtoolsWireMessage};
use serde_json::Value;
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tungstenite::protocol::Message;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{WebSocket, connect};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

pub fn execute_command(
    ws_url: &str,
    handler: impl Into<String>,
    args: Option<Value>,
) -> Result<Option<Value>> {
    let (mut websocket, _) =
        connect(ws_url).with_context(|| format!("Failed to connect dev websocket: {ws_url}"))?;
    configure_read_timeout(&mut websocket);

    send_wire_message(
        &mut websocket,
        &DevtoolsWireMessage::Hello {
            role: DevtoolsPeerRole::Client,
        },
    )?;

    let command_id = command_id();
    send_wire_message(
        &mut websocket,
        &DevtoolsWireMessage::Command {
            command_id: command_id.clone(),
            handler: handler.into(),
            args,
        },
    )?;

    loop {
        let message = websocket
            .read()
            .context("Failed to read dev websocket response")?;
        let Message::Text(text) = message else {
            continue;
        };
        let wire: DevtoolsWireMessage =
            serde_json::from_str(&text).context("Failed to parse dev websocket response")?;
        let DevtoolsWireMessage::Result {
            command_id: result_id,
            ok,
            data,
            error,
        } = wire
        else {
            continue;
        };
        if result_id != command_id {
            continue;
        }
        if ok {
            return Ok(data);
        }
        return Err(anyhow!(
            "{}",
            error.unwrap_or_else(|| "devtool command failed".to_string())
        ));
    }
}

fn send_wire_message(
    websocket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    message: &DevtoolsWireMessage,
) -> Result<()> {
    let text = serde_json::to_string(message).context("Failed to encode dev websocket message")?;
    websocket
        .send(Message::Text(text.into()))
        .context("Failed to send dev websocket message")
}

fn configure_read_timeout(websocket: &mut WebSocket<MaybeTlsStream<TcpStream>>) {
    if let MaybeTlsStream::Plain(stream) = websocket.get_mut() {
        let _ = stream.set_read_timeout(Some(COMMAND_TIMEOUT));
    }
}

fn command_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("lxdev-{nanos}")
}
