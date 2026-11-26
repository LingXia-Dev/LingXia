//! The one error every LingXia layer returns, and the shape it takes on the wire:
//! `{ code, message, reason?, data? }`, the same for Logic, a WebView and a channel.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// The framework's codes. An app's own codes never start with `E_`.
pub mod code {
    pub const ABORT: &str = "E_ABORT";
    pub const ALREADY_EXISTS: &str = "E_ALREADY_EXISTS";
    pub const CAPABILITY_MISSING: &str = "E_CAPABILITY_MISSING";
    pub const INTERNAL: &str = "E_INTERNAL";
    pub const INVALID_ARG: &str = "E_INVALID_ARG";
    pub const INVALID_DATA: &str = "E_INVALID_DATA";
    pub const INVALID_STATE: &str = "E_INVALID_STATE";
    pub const IO: &str = "E_IO";
    pub const NETWORK: &str = "E_NETWORK";
    pub const NOT_FOUND: &str = "E_NOT_FOUND";
    pub const NOT_SUPPORTED: &str = "E_NOT_SUPPORTED";
    pub const OUT_OF_RANGE: &str = "E_OUT_OF_RANGE";
    pub const PERMISSION_DENIED: &str = "E_PERMISSION_DENIED";
    pub const QUOTA: &str = "E_QUOTA";
    pub const STREAM: &str = "E_STREAM";
    pub const SURFACE_CLOSED: &str = "E_SURFACE_CLOSED";
    pub const SURFACE_CONFLICT: &str = "E_SURFACE_CONFLICT";
    pub const SURFACE_ROLE_LOCKED: &str = "E_SURFACE_ROLE_LOCKED";
    pub const TIMEOUT: &str = "E_TIMEOUT";
    pub const TYPE: &str = "E_TYPE";

    /// Every framework code; `LX_ERROR_CODES` in `@lingxia/types` is the same list.
    pub const ALL: [&str; 20] = [
        ABORT,
        ALREADY_EXISTS,
        CAPABILITY_MISSING,
        INTERNAL,
        INVALID_ARG,
        INVALID_DATA,
        INVALID_STATE,
        IO,
        NETWORK,
        NOT_FOUND,
        NOT_SUPPORTED,
        OUT_OF_RANGE,
        PERMISSION_DENIED,
        QUOTA,
        STREAM,
        SURFACE_CLOSED,
        SURFACE_CONFLICT,
        SURFACE_ROLE_LOCKED,
        TIMEOUT,
        TYPE,
    ];
}

/// The numeric codes platforms and older paths raise, each with the code and `reason` it
/// becomes. A number is input only: it never reaches the wire. `i18n/error` has one entry per row.
pub const BUSINESS_CODES: &[(u32, &str, Option<&str>)] = &[
    (1000, code::INTERNAL, None),
    (1001, code::INTERNAL, None),
    (1002, code::INVALID_ARG, None),
    (1003, code::NOT_FOUND, None),
    (1004, code::INTERNAL, None),
    (1005, code::INTERNAL, None),
    (2000, code::ABORT, Some("userCanceled")),
    (2001, code::ABORT, Some("userDeclined")),
    (3000, code::PERMISSION_DENIED, None),
    (
        3001,
        code::PERMISSION_DENIED,
        Some("cameraPermissionDenied"),
    ),
    (
        3002,
        code::PERMISSION_DENIED,
        Some("locationPermissionDenied"),
    ),
    (
        3003,
        code::PERMISSION_DENIED,
        Some("microphonePermissionDenied"),
    ),
    (
        3004,
        code::PERMISSION_DENIED,
        Some("photosPermissionDenied"),
    ),
    (
        3005,
        code::PERMISSION_DENIED,
        Some("storagePermissionDenied"),
    ),
    (3006, code::PERMISSION_DENIED, None),
    (3007, code::PERMISSION_DENIED, None),
    (
        3008,
        code::PERMISSION_DENIED,
        Some("clipboardPermissionDenied"),
    ),
    (4000, code::INVALID_STATE, None),
    (4001, code::INVALID_STATE, Some("locationServicesDisabled")),
    (4002, code::INVALID_STATE, None),
    (4003, code::INVALID_STATE, None),
    (4004, code::INVALID_STATE, Some("refreshNotEnabled")),
    (5000, code::NETWORK, None),
    (5001, code::NETWORK, Some("networkUnavailable")),
    (5002, code::TIMEOUT, None),
    (5003, code::NETWORK, Some("serverErrorStatus")),
    (5004, code::NETWORK, Some("connectionFailed")),
    (6000, code::NOT_SUPPORTED, None),
    (6001, code::NOT_SUPPORTED, Some("deviceUnsupported")),
    (6002, code::NOT_SUPPORTED, Some("hostUpgradeRequired")),
    (12000, code::INVALID_STATE, None),
    (12001, code::NETWORK, Some("wifiSystemError")),
    (12002, code::NETWORK, Some("wifiPasswordWrong")),
    (12003, code::TIMEOUT, Some("wifiTimedOut")),
    (12004, code::NETWORK, None),
    (12005, code::NOT_SUPPORTED, Some("wifiUnsupported")),
    (12006, code::NETWORK, Some("wifiPermissionDenied")),
    (12007, code::NETWORK, Some("wifiPromptUnanswered")),
    (12008, code::NETWORK, Some("wifiConnectionDeclined")),
    (12009, code::INVALID_STATE, Some("wifiDisabled")),
    (12010, code::NOT_FOUND, Some("wifiNetworkNotFound")),
];

/// The code and `reason` a numeric code becomes; an unknown number is `E_INTERNAL`.
pub fn business_code(number: u32) -> (&'static str, Option<&'static str>) {
    BUSINESS_CODES
        .iter()
        .find(|(known, _, _)| *known == number)
        .map_or((code::INTERNAL, None), |(_, code, reason)| (*code, *reason))
}

/// What went wrong (`code`), a plain-English `message` for a log, why (`reason`) where the
/// code alone does not say, and structured facts (`data`). The `source` stays on this side.
#[derive(Debug, Clone)]
pub struct Error {
    code: Cow<'static, str>,
    message: String,
    /// Rarely set, boxed so `Result<_, Error>` stays small.
    extra: Option<Box<Extra>>,
}

#[derive(Debug, Clone, Default)]
struct Extra {
    reason: Option<Cow<'static, str>>,
    data: Option<Value>,
    source: Option<Arc<dyn std::error::Error + Send + Sync>>,
}

impl Error {
    pub fn new(code: impl Into<Cow<'static, str>>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            extra: None,
        }
    }

    fn extra(&mut self) -> &mut Extra {
        self.extra.get_or_insert_with(Default::default)
    }

    /// A numeric code's error, with its `reason`.
    pub fn business(number: u32, message: impl Into<String>) -> Self {
        let (code, reason) = business_code(number);
        let error = Self::new(code, message);
        match reason {
            Some(reason) => error.with_reason(reason),
            None => error,
        }
    }

    pub fn invalid_arg(message: impl Into<String>) -> Self {
        Self::new(code::INVALID_ARG, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(code::NOT_FOUND, message)
    }

    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::new(code::PERMISSION_DENIED, message)
    }

    pub fn not_supported(message: impl Into<String>) -> Self {
        Self::new(code::NOT_SUPPORTED, message)
    }

    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self::new(code::INVALID_STATE, message)
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(code::TIMEOUT, message)
    }

    pub fn network(message: impl Into<String>) -> Self {
        Self::new(code::NETWORK, message)
    }

    pub fn aborted(message: impl Into<String>) -> Self {
        Self::new(code::ABORT, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(code::INTERNAL, message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(code::IO, message)
    }

    pub fn invalid_data(message: impl Into<String>) -> Self {
        Self::new(code::INVALID_DATA, message)
    }

    pub fn with_reason(mut self, reason: impl Into<Cow<'static, str>>) -> Self {
        self.extra().reason = Some(reason.into());
        self
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.extra().data = Some(data);
        self
    }

    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.extra().source = Some(Arc::new(source));
        self
    }

    /// What a `#[derive(NativeError)]` enum sends: its serde `code` tag, the rest as `data`.
    #[doc(hidden)]
    pub fn __app(error: &impl Serialize, message: String) -> Self {
        let mut fields = match serde_json::to_value(error) {
            Ok(Value::Object(fields)) => fields,
            Ok(_) => return Self::internal(message),
            Err(err) => return Self::internal(format!("{message} (unsendable: {err})")),
        };
        let Some(Value::String(code)) = fields.remove("code") else {
            return Self::internal(message);
        };
        let error = Self::new(code, message);
        if fields.is_empty() {
            error
        } else {
            error.with_data(Value::Object(fields))
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn reason(&self) -> Option<&str> {
        self.extra.as_ref()?.reason.as_deref()
    }

    pub fn data(&self) -> Option<&Value> {
        self.extra.as_ref()?.data.as_ref()
    }

    pub fn is(&self, code: &str) -> bool {
        self.code == code
    }

    /// The code for a runtime that keeps codes as `&'static str`. Codes are a small closed set
    /// per app, so an owned one is interned once.
    pub fn static_code(&self) -> &'static str {
        match &self.code {
            Cow::Borrowed(code) => code,
            Cow::Owned(code) => intern(code),
        }
    }
}

fn intern(code: &str) -> &'static str {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    if let Some(known) = code::ALL.iter().find(|known| **known == code) {
        return known;
    }
    static CODES: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut codes = CODES
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(known) = codes.get(code) {
        return known;
    }
    let leaked: &'static str = Box::leak(code.to_owned().into_boxed_str());
    codes.insert(leaked);
    leaked
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)?;
        if let Some(reason) = self.reason() {
            write!(f, " ({reason})")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.extra
            .as_ref()?
            .source
            .as_deref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}

#[derive(Serialize, Deserialize)]
struct Wire {
    code: String,
    message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Wire {
            code: self.code.to_string(),
            message: self.message.clone(),
            reason: self.reason().map(str::to_string),
            data: self.data().cloned(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Error {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = Wire::deserialize(deserializer)?;
        let mut error = Self::new(wire.code, wire.message);
        if let Some(reason) = wire.reason {
            error = error.with_reason(reason);
        }
        if let Some(data) = wire.data {
            error = error.with_data(data);
        }
        Ok(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        let code = match error.kind() {
            std::io::ErrorKind::NotFound => code::NOT_FOUND,
            std::io::ErrorKind::PermissionDenied => code::PERMISSION_DENIED,
            _ => code::IO,
        };
        Self::new(code, error.to_string()).with_source(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::new(code::INVALID_DATA, error.to_string()).with_source(error)
    }
}

impl<T> From<std::sync::mpsc::SendError<T>> for Error {
    fn from(error: std::sync::mpsc::SendError<T>) -> Self {
        Self::internal(error.to_string())
    }
}

/// A `lingxia::Error` inside comes back as it was; anything else is `E_INTERNAL`.
impl From<anyhow::Error> for Error {
    fn from(error: anyhow::Error) -> Self {
        match error.downcast::<Error>() {
            Ok(error) => error,
            Err(error) => {
                let mut built = Self::internal(format!("{error:#}"));
                let source: Box<dyn std::error::Error + Send + Sync> = error.into();
                built.extra().source = Some(Arc::from(source));
                built
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wire_shape_omits_what_is_absent_and_keeps_the_source_home() {
        let bare = serde_json::to_value(Error::not_found("note 7")).unwrap();
        assert_eq!(
            bare,
            serde_json::json!({ "code": "E_NOT_FOUND", "message": "note 7" })
        );
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "locked");
        let full = Error::from(io)
            .with_reason("photosPermissionDenied")
            .with_data(serde_json::json!({ "path": "/a" }));
        assert!(std::error::Error::source(&full).is_some());
        let wire = serde_json::to_value(&full).unwrap();
        assert_eq!(wire["code"], "E_PERMISSION_DENIED");
        assert_eq!(wire["reason"], "photosPermissionDenied");
        let back: Error = serde_json::from_value(wire).unwrap();
        assert_eq!(
            (back.code(), back.reason(), back.data()),
            (full.code(), full.reason(), full.data())
        );
    }

    #[test]
    fn an_app_error_sends_its_tag_as_code_and_its_fields_as_data() {
        #[derive(Serialize)]
        #[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
        enum NotesError {
            NotFound { id: i64 },
            Locked,
        }
        let error = Error::__app(&NotesError::NotFound { id: 7 }, "note 7 not found".into());
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            serde_json::json!({ "code": "NOT_FOUND", "message": "note 7 not found", "data": { "id": 7 } })
        );
        let error = Error::__app(&NotesError::Locked, "locked".into());
        assert_eq!((error.code(), error.data()), ("LOCKED", None));
    }

    #[test]
    fn anyhow_keeps_a_coded_error_and_makes_the_rest_internal() {
        let coded = anyhow::Error::new(Error::not_found("note 7"));
        assert_eq!(Error::from(coded).code(), code::NOT_FOUND);
        let other = Error::from(anyhow::anyhow!("disk full").context("saving note"));
        assert_eq!(other.code(), code::INTERNAL);
        assert_eq!(other.message(), "saving note: disk full");
        assert!(std::error::Error::source(&other).is_some());
    }

    #[test]
    fn a_numeric_code_becomes_its_code_and_reason() {
        let error = Error::business(3002, "denied in settings");
        assert_eq!(error.code(), code::PERMISSION_DENIED);
        assert_eq!(error.reason(), Some("locationPermissionDenied"));
        assert_eq!(error.message(), "denied in settings");
        assert_eq!(Error::business(1002, "x").reason(), None);
        assert_eq!(Error::business(99, "x").code(), code::INTERNAL);
    }

    #[test]
    fn every_business_code_maps_to_a_framework_code() {
        for (_, code, _) in BUSINESS_CODES {
            assert!(code::ALL.contains(code), "{code}");
        }
    }

    #[test]
    fn the_typescript_code_list_is_this_list() {
        let ts = include_str!("../../../packages/lingxia-types/src/error.ts");
        let start = ts.find("LX_ERROR_CODES = [").unwrap();
        let end = start + ts[start..].find("] as const").unwrap();
        let listed: Vec<&str> = ts[start..end]
            .split('\'')
            .filter(|part| part.starts_with("E_"))
            .collect();
        assert_eq!(listed, code::ALL);
    }
}
