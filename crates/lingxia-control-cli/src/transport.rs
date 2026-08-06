//! How a command reaches a running app.
//!
//! Commands that drive a live app need one of these; `desktop` does not, since
//! it automates the OS directly. `lxdev` supplies a websocket implementation
//! because it may be talking to a phone across the network. A shipped product
//! supplies [`ControlSocket`], which never leaves the machine.

use std::io::{BufRead, BufReader, Write};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

/// One request, one response. Deliberately the whole interface: every command
/// in this crate is request/response, and a transport that had to model
/// streaming would push that shape onto all of them.
pub trait Transport {
    fn request(&self, method: &str, params: Option<Value>) -> Result<Option<Value>>;
}

impl<T: Transport + ?Sized> Transport for &T {
    fn request(&self, method: &str, params: Option<Value>) -> Result<Option<Value>> {
        (**self).request(method, params)
    }
}

/// The product's local control socket: a Unix domain socket on macOS, a named
/// pipe on Windows. Both are byte streams that `std` alone can open, so a
/// client needs no platform crate — the server side is where the difference
/// (security descriptors, peer identity) actually lives.
pub struct ControlSocket {
    endpoint: String,
}

impl ControlSocket {
    /// Point at an endpoint reported by the product's `control::endpoint_name`.
    pub fn at(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    /// Connect per request rather than holding one open: commands are short,
    /// a stale handle across a product restart is a worse failure than the
    /// microseconds a local connect costs, and the server serves each
    /// connection on its own thread anyway.
    fn connect(&self) -> Result<Connection> {
        Connection::open(&self.endpoint)
            .with_context(|| format!("failed to reach the control socket at {}", self.endpoint))
    }
}

impl Transport for ControlSocket {
    fn request(&self, method: &str, params: Option<Value>) -> Result<Option<Value>> {
        let mut connection = self.connect()?;
        let request = serde_json::json!({
            "type": "request",
            "id": "1",
            "method": method,
            "params": params,
        });
        connection.write_line(&serde_json::to_string(&request)?)?;
        let line = connection.read_line()?;
        parse_response(&line, method)
    }
}

fn parse_response(line: &str, method: &str) -> Result<Option<Value>> {
    let reply: Value =
        serde_json::from_str(line).with_context(|| format!("{method} returned invalid JSON"))?;
    if let Some(error) = reply.get("error").filter(|value| !value.is_null()) {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("request failed");
        let code = error.get("code").and_then(Value::as_str).unwrap_or("error");
        bail!("{method} failed ({code}): {message}");
    }
    Ok(reply
        .get("result")
        .cloned()
        .filter(|value| !value.is_null()))
}

#[cfg(unix)]
struct Connection(std::os::unix::net::UnixStream);

#[cfg(unix)]
impl Connection {
    fn open(endpoint: &str) -> std::io::Result<Self> {
        std::os::unix::net::UnixStream::connect(endpoint).map(Self)
    }

    fn write_line(&mut self, line: &str) -> Result<()> {
        self.0.write_all(line.as_bytes())?;
        self.0.write_all(b"\n")?;
        self.0.flush()?;
        Ok(())
    }

    fn read_line(&mut self) -> Result<String> {
        let mut reader = BufReader::new(self.0.try_clone()?);
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err(anyhow!("the app closed the connection without answering"));
        }
        Ok(line)
    }
}

#[cfg(windows)]
struct Connection(std::fs::File);

#[cfg(windows)]
impl Connection {
    fn open(endpoint: &str) -> std::io::Result<Self> {
        // A byte-mode duplex pipe opens like a file, which keeps the client
        // free of the Windows crate entirely.
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(endpoint)
            .map(Self)
    }

    fn write_line(&mut self, line: &str) -> Result<()> {
        self.0.write_all(line.as_bytes())?;
        self.0.write_all(b"\n")?;
        self.0.flush()?;
        Ok(())
    }

    fn read_line(&mut self) -> Result<String> {
        let mut reader = BufReader::new(self.0.try_clone()?);
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err(anyhow!("the app closed the connection without answering"));
        }
        Ok(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surfaces_the_error_a_handler_reported() {
        let error = parse_response(
            r#"{"type":"response","id":"1","error":{"code":"unknown_method","message":"unknown method: nope"}}"#,
            "nope",
        )
        .expect_err("an error response must not be read as success");
        assert!(error.to_string().contains("unknown_method"));
        assert!(error.to_string().contains("unknown method: nope"));
    }

    #[test]
    fn reads_a_result_and_treats_a_missing_one_as_none() {
        let value =
            parse_response(r#"{"type":"response","id":"1","result":{"ok":1}}"#, "x").unwrap();
        assert_eq!(value, Some(serde_json::json!({"ok": 1})));
        assert_eq!(
            parse_response(r#"{"type":"response","id":"1"}"#, "x").unwrap(),
            None
        );
    }
}
