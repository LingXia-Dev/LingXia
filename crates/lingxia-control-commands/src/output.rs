//! Where a command's bytes land.
//!
//! Shared because every capture-producing command answers the same question —
//! stdout, a named path, or a dated file under `.lingxia/screenshots` — and a
//! second copy would drift on the day one of them grows an option.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::Value;

/// Reduce a title or app name to something safe in a filename.
pub fn safe_component(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Write PNG bytes to `-` (stdout), an explicit path, or the default folder.
pub fn write_png(output: Option<String>, default_filename: String, bytes: &[u8]) -> Result<()> {
    use std::fs;
    use std::io::Write;

    if matches!(output.as_deref(), Some("-")) {
        std::io::stdout()
            .lock()
            .write_all(bytes)
            .context("failed to write screenshot to stdout")?;
        return Ok(());
    }

    let path: PathBuf = match output {
        Some(path) => PathBuf::from(path),
        None => {
            let dir = std::env::current_dir()?
                .join(".lingxia")
                .join("screenshots");
            fs::create_dir_all(&dir)
                .with_context(|| format!("failed to create {}", dir.display()))?;
            dir.join(default_filename)
        }
    };

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&path, bytes).with_context(|| format!("failed to write {}", path.display()))?;
    println!("{}  ({} bytes)", path.display(), bytes.len());
    Ok(())
}

/// Pull PNG bytes out of a handler response. Every namespace that captures
/// returns the same envelope, so this decodes once rather than per namespace.
pub fn decode_png_payload(data: &Value, handler: &str) -> Result<Vec<u8>> {
    use base64::Engine as _;

    // Unified envelope nests the payload under image.data; fall back to the
    // legacy top-level data_base64 for older runners.
    let b64 = data
        .get("image")
        .and_then(|image| image.get("data"))
        .and_then(Value::as_str)
        .or_else(|| data.get("data_base64").and_then(Value::as_str))
        .with_context(|| format!("{handler} response missing image.data"))?;
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .context("failed to base64-decode screenshot payload")
}
