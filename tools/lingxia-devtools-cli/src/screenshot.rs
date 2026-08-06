use anyhow::{Context, Result};
use serde_json::Value;

pub use lingxia_control_cli::output::{safe_component, write_png};

/// Pull PNG bytes out of a devtool response. Transport-side, so it stays here
/// rather than in the shared command crate: only the websocket namespaces
/// return captures as base64 in an envelope.
pub fn decode_png_payload(data: &Value, handler: &str) -> Result<Vec<u8>> {
    use base64::Engine as _;

    // Unified envelope nests the payload under image.data; fall back to the
    // legacy top-level data_base64 for older runners.
    let b64 = data
        .get("image")
        .and_then(|img| img.get("data"))
        .and_then(Value::as_str)
        .or_else(|| data.get("data_base64").and_then(Value::as_str))
        .with_context(|| format!("{handler} response missing image.data"))?;
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .context("failed to base64-decode screenshot payload")
}
