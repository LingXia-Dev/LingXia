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

/// Build the screenshot JSON envelope returned by all `*.screenshot`
/// handlers: always `{format, size_bytes, data_base64, ...extra}`.
///
/// Reserved keys (`format`, `size_bytes`, `data_base64`) are always
/// inserted last and silently overwrite any same-named keys passed in
/// `extra` — they describe what we actually encoded, so a caller cannot
/// override them meaningfully. In debug builds this collision triggers an
/// assertion to catch accidental name reuse early.
pub(crate) fn png_response(
    bytes: &[u8],
    extra: impl IntoIterator<Item = (&'static str, Value)>,
) -> Value {
    const RESERVED: [&str; 3] = ["format", "size_bytes", "data_base64"];

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
    object.insert("format".to_string(), json!("png"));
    object.insert("size_bytes".to_string(), json!(bytes.len()));
    object.insert("data_base64".to_string(), json!(encoded));
    Value::Object(object)
}
