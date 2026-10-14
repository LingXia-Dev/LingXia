//! `AudioSource` → something a native player can open, under the same file
//! and network rules as every other `lx` media API.

use lingxia_media::audio::{AudioError, AudioErrorCode, QueueEntry};
use lxapp::{LxApp, LxAppError};
use rong::{AnyJSTypedArray, JSArrayBuffer, JSObject, JSValue};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(super) fn invalid_source(message: impl Into<String>) -> AudioError {
    AudioError::new(AudioErrorCode::InvalidSource, message)
}

pub(super) fn invalid_options(message: impl Into<String>) -> AudioError {
    AudioError::new(AudioErrorCode::InvalidOptions, message)
}

struct RawSource {
    uri: String,
    headers: BTreeMap<String, String>,
    referrer_origin: bool,
    mime_type: Option<String>,
}

fn read_source(value: &JSValue) -> Result<RawSource, AudioError> {
    if value.is_string() {
        let uri = value
            .clone()
            .to_rust::<String>()
            .map_err(|_| invalid_source("source must be a string or { uri }"))?;
        return Ok(RawSource {
            uri,
            headers: BTreeMap::new(),
            referrer_origin: false,
            mime_type: None,
        });
    }
    let Some(object) = value.clone().into_object() else {
        return Err(invalid_source("source must be a string or { uri }"));
    };
    let uri = object
        .get::<_, String>("uri")
        .map_err(|_| invalid_source("source.uri must be a string"))?;
    let mut headers = BTreeMap::new();
    if let Ok(value) = object.get::<_, JSValue>("headers")
        && !value.is_undefined()
        && !value.is_null()
    {
        let json = value
            .into_object()
            .ok_or_else(|| invalid_source("source.headers must be an object"))?
            .to_json_string()
            .map_err(|_| invalid_source("source.headers must be JSON-compatible"))?;
        let map: BTreeMap<String, serde_json::Value> = serde_json::from_str(&json)
            .map_err(|_| invalid_source("source.headers must map names to strings"))?;
        for (name, value) in map {
            let serde_json::Value::String(value) = value else {
                return Err(invalid_source(format!("header '{name}' must be a string")));
            };
            headers.insert(name, value);
        }
    }
    let referrer_origin = match optional_string(&object, "referrerPolicy")?.as_deref() {
        None | Some("no-referrer") => false,
        Some("origin") => true,
        Some(other) => {
            return Err(invalid_source(format!(
                "referrerPolicy must be 'no-referrer' or 'origin', got '{other}'"
            )));
        }
    };
    Ok(RawSource {
        uri,
        headers,
        referrer_origin,
        mime_type: optional_string(&object, "mimeType")?,
    })
}

pub(super) fn optional_string(object: &JSObject, key: &str) -> Result<Option<String>, AudioError> {
    match object.get::<_, JSValue>(key) {
        Ok(value) if value.is_undefined() || value.is_null() => Ok(None),
        Ok(value) if value.is_string() => Ok(value.to_rust::<String>().ok()),
        Ok(_) => Err(invalid_options(format!("{key} must be a string"))),
        Err(_) => Ok(None),
    }
}

fn is_remote(uri: &str) -> bool {
    let lower = uri.get(..8).unwrap_or(uri).to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

fn url_host(uri: &str) -> Option<&str> {
    let rest = uri.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = if let Some(bracketed) = host_port.strip_prefix('[') {
        bracketed.split_once(']')?.0
    } else {
        host_port.split(':').next()?
    };
    (!host.is_empty()).then_some(host)
}

/// `lx://lxapp/<path>` names this lxapp's package; the runtime spells it with
/// the app id in front.
fn package_uri(app: &LxApp, uri: &str) -> String {
    match uri.strip_prefix("lx://lxapp/") {
        Some(rest) if !rest.starts_with(&format!("{}/", app.appid)) => {
            format!("lx://lxapp/{}/{rest}", app.appid)
        }
        _ => uri.to_string(),
    }
}

fn resolve_local(app: &LxApp, uri: &str) -> Result<PathBuf, AudioError> {
    if !uri.starts_with("lx://") {
        return Err(invalid_source(format!(
            "'{uri}' is not an http(s) URL or lx:// URI; relative and native paths are not accepted"
        )));
    }
    let path = app
        .resolve_accessible_path(&package_uri(app, uri))
        .map_err(|error| match error {
            LxAppError::InvalidParameter(message) => invalid_source(message),
            other => AudioError::new(AudioErrorCode::SourceNotFound, other.to_string()),
        })?;
    if !path.is_file() {
        return Err(AudioError::new(
            AudioErrorCode::SourceNotFound,
            format!("no file at {uri}"),
        ));
    }
    Ok(path)
}

fn authorize_remote(app: &LxApp, uri: &str) -> Result<(), AudioError> {
    let host = url_host(uri).ok_or_else(|| invalid_source(format!("'{uri}' has no host")))?;
    if !app.is_domain_allowed(host) {
        return Err(AudioError::new(
            AudioErrorCode::NetworkDenied,
            format!("domain '{host}' is not allowed by lxapp security policy"),
        ));
    }
    if let Ok(address) = host.parse::<std::net::IpAddr>()
        && !lxapp::is_public_network_address(address)
        && !lxapp::is_dev_session()
    {
        return Err(AudioError::new(
            AudioErrorCode::NetworkDenied,
            format!("'{host}' is not a public address"),
        ));
    }
    Ok(())
}

/// A player source: an http(s) URL under the network grant, or a package /
/// userdata / usercache / temp file.
pub(super) fn resolve_player_source(
    app: &LxApp,
    value: &JSValue,
    start_ms: u64,
) -> Result<QueueEntry, AudioError> {
    let raw = read_source(value)?;
    let uri = raw.uri.trim();
    if is_remote(uri) {
        authorize_remote(app, uri)?;
        let mut headers = raw.headers;
        if raw.referrer_origin {
            headers.insert("Referer".into(), format!("https://{}.lxapp/", app.appid));
        }
        return Ok(QueueEntry {
            uri: uri.to_string(),
            headers,
            mime_type: raw.mime_type,
            start_ms,
        });
    }
    let path = resolve_local(app, uri)?;
    Ok(QueueEntry {
        uri: path.to_string_lossy().into_owned(),
        headers: BTreeMap::new(),
        mime_type: raw.mime_type,
        start_ms,
    })
}

/// In-memory cap for a sound: effects and short clips, not long-form media.
pub(super) const MAX_SOUND_BYTES: usize = 10 * 1024 * 1024;

/// A sound source: a local file, or bytes staged into this session's temp.
pub(super) fn resolve_sound_source(app: &LxApp, value: &JSValue) -> Result<PathBuf, AudioError> {
    if let Some(object) = value.clone().into_object()
        && let Ok(bytes) = object.get::<_, JSValue>("bytes")
        && !bytes.is_undefined()
    {
        let data = js_bytes(bytes)
            .ok_or_else(|| invalid_source("bytes must be a Uint8Array or ArrayBuffer"))?;
        if data.len() > MAX_SOUND_BYTES {
            return Err(AudioError::new(
                AudioErrorCode::SourceTooLarge,
                format!("a sound holds at most {MAX_SOUND_BYTES} bytes"),
            ));
        }
        let mime = optional_string(&object, "mimeType")?
            .ok_or_else(|| invalid_source("bytes need a mimeType"))?;
        let path = app
            .temp_output_path("audio", Some(extension_for(&mime)))
            .map_err(|e| invalid_source(e.to_string()))?;
        std::fs::write(&path, data).map_err(|e| invalid_source(e.to_string()))?;
        return Ok(path);
    }
    let raw = read_source(value)?;
    let uri = raw.uri.trim();
    if is_remote(uri) {
        return Err(invalid_source(
            "sounds load from lx:// files or bytes; download a remote sound first (lx.downloadFile)",
        ));
    }
    let path = resolve_local(app, uri)?;
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if size as usize > MAX_SOUND_BYTES {
        return Err(AudioError::new(
            AudioErrorCode::SourceTooLarge,
            format!("a sound holds at most {MAX_SOUND_BYTES} bytes; use a player for long audio"),
        ));
    }
    Ok(path)
}

fn js_bytes(value: JSValue) -> Option<Vec<u8>> {
    if value.is_array_buffer() {
        return value
            .to_rust::<JSArrayBuffer>()
            .ok()
            .map(|buffer| buffer.as_bytes().to_vec());
    }
    let typed = AnyJSTypedArray::from_object(value.into_object()?)?;
    typed.as_bytes().map(<[u8]>::to_vec)
}

fn extension_for(mime: &str) -> &'static str {
    match mime.split(';').next().unwrap_or("").trim() {
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/mp4" | "audio/aac" | "audio/x-m4a" => "m4a",
        "audio/ogg" | "audio/opus" => "ogg",
        "audio/flac" => "flac",
        _ => "wav",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_are_read_from_urls() {
        assert_eq!(
            url_host("https://cdn.example.com/a.mp3"),
            Some("cdn.example.com")
        );
        assert_eq!(url_host("http://user@10.0.0.2:8080/a"), Some("10.0.0.2"));
        assert_eq!(url_host("https://[::1]:443/x"), Some("::1"));
        assert_eq!(url_host("https:///x"), None);
        assert!(is_remote("HTTPS://x.y/z"));
        assert!(!is_remote("//cdn.example.com/a.mp3"));
        assert!(!is_remote("file:///tmp/a.mp3"));
    }

    #[test]
    fn mime_types_pick_an_extension() {
        assert_eq!(extension_for("audio/mpeg"), "mp3");
        assert_eq!(extension_for("audio/wav; codecs=1"), "wav");
        assert_eq!(extension_for("audio/ogg"), "ogg");
    }
}
