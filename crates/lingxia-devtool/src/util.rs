use serde_json::{Map, Value, json};

pub(crate) fn run_async<T, E>(
    future: impl std::future::Future<Output = Result<T, E>>,
) -> Result<T, String>
where
    E: std::fmt::Display,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| err.to_string())?
        .block_on(future)
        .map_err(|err| err.to_string())
}

/// Read `(width, height)` from a PNG's IHDR chunk without a full decode.
pub(crate) fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    // 8-byte signature, then the IHDR chunk: 4-byte length, "IHDR" tag,
    // width (u32 BE), height (u32 BE).
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[0..8] != SIG || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    Some((w, h))
}

/// Build the unified screenshot JSON envelope returned by all `*.screenshot`
/// handlers:
///
/// ```json
/// { "target": "...", "kind": "screenshot", "coordinate_space": "...",
///   "format": "png", "width": W, "height": H, "size_bytes": N,
///   "image": { "mime": "image/png", "encoding": "base64", "data": "..." },
///   ...extra }
/// ```
///
/// Reserved keys (the ones above) are inserted last and silently overwrite
/// same-named keys passed in `extra` — they describe what we actually encoded,
/// so a caller cannot override them meaningfully. In debug builds a collision
/// triggers an assertion to catch accidental name reuse early.
pub(crate) fn png_response(
    target: &'static str,
    coordinate_space: &'static str,
    bytes: &[u8],
    extra: impl IntoIterator<Item = (&'static str, Value)>,
) -> Value {
    const RESERVED: [&str; 8] = [
        "target",
        "kind",
        "coordinate_space",
        "format",
        "width",
        "height",
        "size_bytes",
        "image",
    ];

    let mut object = Map::new();
    for (key, value) in extra {
        debug_assert!(
            !RESERVED.contains(&key),
            "png_response extra key `{key}` collides with reserved key — rename it",
        );
        object.insert(key.to_string(), value);
    }

    let encoded = {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(bytes)
    };
    object.insert("target".to_string(), json!(target));
    object.insert("kind".to_string(), json!("screenshot"));
    object.insert("coordinate_space".to_string(), json!(coordinate_space));
    object.insert("format".to_string(), json!("png"));
    if let Some((w, h)) = png_dimensions(bytes) {
        object.insert("width".to_string(), json!(w));
        object.insert("height".to_string(), json!(h));
    }
    object.insert("size_bytes".to_string(), json!(bytes.len()));
    object.insert(
        "image".to_string(),
        json!({ "mime": "image/png", "encoding": "base64", "data": encoded }),
    );
    Value::Object(object)
}
