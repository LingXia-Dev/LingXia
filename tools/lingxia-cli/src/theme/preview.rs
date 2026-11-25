//! `lingxia theme preview`: the current lxapp in the Runner under its theme, one key per look.

use anyhow::{Context, Result, anyhow, bail};
use console::{Key, Term};
use lingxia_control_protocol::dev_session::broker;
use lingxia_control_protocol::dev_session::{
    DEV_SESSION_PROTOCOL_VERSION, DevSessionMessage, DevSessionRole, capabilities,
};
use lingxia_control_protocol::{ControlRequest, methods};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tungstenite::Error as WsError;
use tungstenite::protocol::Message;

use super::{THEME_PATH, load};
use crate::commands::dev::attach::{connect_host, send_message};

/// Each look as the Runner sets it: its name, the appearance and the contrast.
const LOOKS: [(&str, &str, &str); 4] = [
    ("light", "light", "standard"),
    ("dark", "dark", "standard"),
    ("high-contrast-light", "light", "high"),
    ("high-contrast-dark", "dark", "high"),
];

pub fn run() -> Result<()> {
    if !Path::new("lxapp.json").is_file() {
        bail!("run `lingxia theme preview` in an lxapp: it previews that lxapp's {THEME_PATH}");
    }
    let theme = Path::new(THEME_PATH);
    // Fail on the theme's own problems before a build reaches them.
    let pack = load(theme.is_file().then_some(theme))?;
    let mut dev = Command::new(std::env::current_exe()?)
        .arg("dev")
        .stdin(Stdio::null())
        .spawn()
        .context("Failed to start `lingxia dev`")?;
    let result = drive(&mut dev, &pack.name);
    let _ = dev.kill();
    let _ = dev.wait();
    result
}

fn drive(dev: &mut Child, name: &str) -> Result<()> {
    let ws_url = session(dev)?;
    let mut look = 0;
    // The Runner registers before its window connects, so the first look waits for it.
    let first = Instant::now() + Duration::from_secs(120);
    while let Err(error) = set(&ws_url, look) {
        if Instant::now() >= first || dev.try_wait()?.is_some() {
            return Err(error);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    println!(
        "Previewing {name}: Space or Enter for the next look, 1-4 for light, dark, high-contrast light, high-contrast dark; q quits."
    );
    println!("  {}", LOOKS[look].0);
    let term = Term::stdout();
    loop {
        look = match term.read_key()? {
            Key::Enter | Key::Char(' ') => (look + 1) % LOOKS.len(),
            Key::Char(digit @ '1'..='4') => digit as usize - '1' as usize,
            Key::Char('q') | Key::Escape => return Ok(()),
            _ => continue,
        };
        set(&ws_url, look)?;
        println!("  {}", LOOKS[look].0);
    }
}

/// The dev websocket of the session `dev` registers.
fn session(dev: &mut Child) -> Result<String> {
    loop {
        if let Some(status) = dev.try_wait()? {
            bail!("`lingxia dev` exited ({status}) before its session was ready");
        }
        if let Some(session) = broker::list_sessions()
            .unwrap_or_default()
            .into_iter()
            .find(|session| session.pid == dev.id())
        {
            return Ok(session.ws_url);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn set(ws_url: &str, look: usize) -> Result<()> {
    let (_, appearance, contrast) = LOOKS[look];
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
    let id = uuid::Uuid::new_v4().to_string();
    send_message(
        &mut websocket,
        &DevSessionMessage::Request(ControlRequest {
            id: id.clone(),
            method: methods::runner::SET.to_string(),
            params: Some(serde_json::json!({ "appearance": appearance, "contrast": contrast })),
        }),
    )?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        match websocket.read() {
            Ok(Message::Text(text)) => {
                let Ok(DevSessionMessage::Response(response)) = serde_json::from_str(&text) else {
                    continue;
                };
                if response.id == id {
                    let _ = websocket.close(None);
                    return response
                        .error
                        .map_or(Ok(()), |error| Err(anyhow!(error.message)));
                }
            }
            Ok(_) => {}
            // The socket reads with a short timeout; a quiet one is not a lost one.
            Err(WsError::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(error.into()),
        }
    }
    bail!("the Runner did not answer runner.set")
}
