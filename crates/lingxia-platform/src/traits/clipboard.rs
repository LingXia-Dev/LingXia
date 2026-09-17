use crate::error::PlatformError;

/// Representations the product clipboard can round-trip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardKind {
    Text,
    Image,
}

impl ClipboardKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "text" => Some(Self::Text),
            "image" => Some(Self::Image),
            _ => None,
        }
    }
}

/// One write: text or a local image file, never both.
#[derive(Debug, Clone)]
pub enum ClipboardWrite {
    Text(String),
    Image { path: String },
}

/// What to surface from a read. `None` means every representation this host can.
#[derive(Debug, Clone, Default)]
pub struct ClipboardReadRequest {
    pub kind: Option<ClipboardKind>,
    /// Host writes a read image here as a regular file. Required when an image
    /// may be returned; Logic allocates it under the lxapp temp dir.
    pub image_output_path: Option<String>,
}

/// Completed clipboard read. `canceled` is only for a dismissed paste prompt.
#[derive(Debug, Clone, Default)]
pub struct ClipboardContents {
    pub canceled: bool,
    pub text: Option<String>,
    pub image_path: Option<String>,
    pub image_mime: Option<String>,
}

impl ClipboardContents {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn canceled() -> Self {
        Self {
            canceled: true,
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.canceled && self.text.is_none() && self.image_path.is_none()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ClipboardTypes {
    pub canceled: bool,
    pub kinds: Vec<ClipboardKind>,
}

/// JSON envelope used by Apple / Android / Harmony native adapters.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct NativeClipboardReply {
    #[serde(default)]
    pub ok: Option<bool>,
    #[serde(default)]
    pub canceled: bool,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default, rename = "imagePath")]
    pub image_path: Option<String>,
    #[serde(default, rename = "imageMime")]
    pub image_mime: Option<String>,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub error: Option<u32>,
    #[serde(default)]
    pub detail: Option<String>,
}

pub fn parse_native_reply(payload: &str) -> Result<NativeClipboardReply, PlatformError> {
    let trimmed = payload.trim();
    if trimmed.is_empty() {
        return Ok(NativeClipboardReply {
            ok: Some(true),
            ..NativeClipboardReply::default()
        });
    }
    serde_json::from_str(trimmed)
        .map_err(|e| PlatformError::Platform(format!("clipboard returned invalid payload: {e}")))
}

impl NativeClipboardReply {
    pub fn into_unit(self) -> Result<(), PlatformError> {
        self.ensure_ok()?;
        Ok(())
    }

    pub fn into_contents(self) -> Result<ClipboardContents, PlatformError> {
        self.ensure_ok()?;
        if self.canceled {
            return Ok(ClipboardContents::canceled());
        }
        Ok(ClipboardContents {
            canceled: false,
            text: self.text,
            image_path: self.image_path.filter(|value| !value.is_empty()),
            image_mime: self.image_mime.filter(|value| !value.is_empty()),
        })
    }

    pub fn into_types(self) -> Result<ClipboardTypes, PlatformError> {
        self.ensure_ok()?;
        if self.canceled {
            return Ok(ClipboardTypes {
                canceled: true,
                kinds: Vec::new(),
            });
        }
        let mut kinds = Vec::new();
        for token in self.types {
            if let Some(kind) = ClipboardKind::parse(&token)
                && !kinds.contains(&kind)
            {
                kinds.push(kind);
            }
        }
        Ok(ClipboardTypes {
            canceled: false,
            kinds,
        })
    }

    fn ensure_ok(&self) -> Result<(), PlatformError> {
        if let Some(code) = self.error {
            return Err(PlatformError::BusinessError(code));
        }
        if self.ok == Some(false) {
            return Err(PlatformError::Platform(
                self.detail
                    .clone()
                    .unwrap_or_else(|| "clipboard operation failed".to_string()),
            ));
        }
        Ok(())
    }
}

pub trait ClipboardService: Send + Sync + 'static {
    fn clipboard_write(
        &self,
        item: ClipboardWrite,
    ) -> impl std::future::Future<Output = Result<(), PlatformError>> + Send;

    fn clipboard_read(
        &self,
        request: ClipboardReadRequest,
    ) -> impl std::future::Future<Output = Result<ClipboardContents, PlatformError>> + Send;

    fn clipboard_clear(
        &self,
    ) -> impl std::future::Future<Output = Result<(), PlatformError>> + Send;

    fn clipboard_types(
        &self,
    ) -> impl std::future::Future<Output = Result<ClipboardTypes, PlatformError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::{ClipboardKind, parse_native_reply};

    #[test]
    fn empty_payload_is_success() {
        let reply = parse_native_reply("").unwrap();
        assert_eq!(reply.ok, Some(true));
        assert!(reply.into_unit().is_ok());
    }

    #[test]
    fn permission_error_is_business_code() {
        let reply = parse_native_reply(r#"{"ok":false,"error":3008,"detail":"denied"}"#).unwrap();
        match reply.into_unit() {
            Err(crate::error::PlatformError::BusinessError(3008)) => {}
            other => panic!("expected business 3008, got {other:?}"),
        }
    }

    #[test]
    fn canceled_read_has_no_payload() {
        let contents = parse_native_reply(r#"{"ok":true,"canceled":true}"#)
            .unwrap()
            .into_contents()
            .unwrap();
        assert!(contents.canceled);
        assert!(contents.text.is_none());
    }

    #[test]
    fn types_ignore_unknown_tokens() {
        let peeked = parse_native_reply(r#"{"ok":true,"types":["text","html","image","text"]}"#)
            .unwrap()
            .into_types()
            .unwrap();
        assert_eq!(peeked.kinds, [ClipboardKind::Text, ClipboardKind::Image]);
    }
}
