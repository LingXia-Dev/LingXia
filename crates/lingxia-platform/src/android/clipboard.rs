use jni::objects::{JClass, JObject, JString, JValue};
use jni::{jni_sig, jni_str};

use crate::error::PlatformError;
use crate::traits::clipboard::{
    ClipboardContents, ClipboardKind, ClipboardReadRequest, ClipboardService, ClipboardTypes,
    ClipboardWrite, parse_native_reply,
};

use super::{Platform, with_env};

fn read_jni_string(env: &mut jni::Env, obj: JObject<'_>) -> Result<String, PlatformError> {
    if obj.is_null() {
        return Err(PlatformError::Platform(
            "Android clipboard returned null".to_string(),
        ));
    }
    let value = unsafe { JString::from_raw(env, obj.into_raw() as _) };
    value
        .try_to_string(env)
        .map_err(|e| PlatformError::Platform(e.to_string()))
}

fn call_write_or_read(
    method_write: bool,
    kind: &str,
    payload: &str,
) -> Result<String, PlatformError> {
    with_env(|env| -> Result<String, PlatformError> {
        let class: &JClass = super::get_cached_class(super::CachedClass::LxAppClipboard)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;
        let kind_j = env.new_string(kind)?;
        let payload_j = env.new_string(payload)?;
        let result = if method_write {
            env.call_static_method(
                class,
                jni_str!("write"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;"),
                &[JValue::Object(&kind_j), JValue::Object(&payload_j)],
            )?
        } else {
            env.call_static_method(
                class,
                jni_str!("read"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;"),
                &[JValue::Object(&kind_j), JValue::Object(&payload_j)],
            )?
        };
        if env.exception_check() {
            env.exception_clear();
            return Err(PlatformError::Platform(
                "Android clipboard JNI call threw an exception".to_string(),
            ));
        }
        read_jni_string(env, result.l()?)
    })
    .map_err(|e| PlatformError::Platform(format!("Failed to call Android clipboard: {e}")))
}

fn call_clear_or_types(clear: bool) -> Result<String, PlatformError> {
    with_env(|env| -> Result<String, PlatformError> {
        let class: &JClass = super::get_cached_class(super::CachedClass::LxAppClipboard)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;
        let result = if clear {
            env.call_static_method(
                class,
                jni_str!("clear"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            )?
        } else {
            env.call_static_method(
                class,
                jni_str!("types"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            )?
        };
        if env.exception_check() {
            env.exception_clear();
            return Err(PlatformError::Platform(
                "Android clipboard JNI call threw an exception".to_string(),
            ));
        }
        read_jni_string(env, result.l()?)
    })
    .map_err(|e| PlatformError::Platform(format!("Failed to call Android clipboard: {e}")))
}

impl ClipboardService for Platform {
    async fn clipboard_write(&self, item: ClipboardWrite) -> Result<(), PlatformError> {
        let payload = match &item {
            ClipboardWrite::Text(text) => call_write_or_read(true, "text", text)?,
            ClipboardWrite::Image { path } => call_write_or_read(true, "image", path)?,
        };
        parse_native_reply(&payload)?.into_unit()
    }

    async fn clipboard_read(
        &self,
        request: ClipboardReadRequest,
    ) -> Result<ClipboardContents, PlatformError> {
        let kind = request.kind.map(ClipboardKind::as_str).unwrap_or("");
        let dest = request.image_output_path.as_deref().unwrap_or("");
        let payload = call_write_or_read(false, kind, dest)?;
        parse_native_reply(&payload)?.into_contents()
    }

    async fn clipboard_clear(&self) -> Result<(), PlatformError> {
        parse_native_reply(&call_clear_or_types(true)?)?.into_unit()
    }

    async fn clipboard_types(&self) -> Result<ClipboardTypes, PlatformError> {
        parse_native_reply(&call_clear_or_types(false)?)?.into_types()
    }
}
