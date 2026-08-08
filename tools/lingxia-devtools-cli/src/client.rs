use anyhow::{Context, Result, anyhow};
use lingxia_control_protocol::{
    ControlRequest,
    dev_session::{DEV_SESSION_PROTOCOL_VERSION, DevSessionMessage, DevSessionRole, capabilities},
};
use serde_json::Value;
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tungstenite::protocol::Message;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{WebSocket, connect};

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const COMMAND_TIMEOUT_BUFFER: Duration = Duration::from_secs(5);

pub fn execute_command(
    ws_url: &str,
    handler: impl Into<String>,
    args: Option<Value>,
) -> Result<Option<Value>> {
    let handler = handler.into();
    let timeout = command_timeout(args.as_ref());
    let (mut websocket, _) =
        connect(ws_url).with_context(|| format!("Failed to connect dev websocket: {ws_url}"))?;
    configure_read_timeout(&mut websocket, timeout);

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
            method: handler,
            params: args,
        }),
    )?;

    loop {
        let message = websocket
            .read()
            .context("Failed to read dev websocket response")?;
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
            return Err(anyhow!("{}", error.message));
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

impl lingxia_control_cli::transport::Transport for DevSession<'_> {
    fn request(&self, method: &str, params: Option<Value>) -> Result<Option<Value>> {
        execute_command(self.ws_url, method, params)
    }
}
