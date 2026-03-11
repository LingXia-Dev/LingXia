use super::{CachedClass, get_cached_class};
use crate::error::PlatformError;
use jni::objects::{JClass, JValue};
use jni::{jni_sig, jni_str};
use lingxia_webview::platform::android::with_env;

/// Show download progress dialog
pub fn show_download_progress() -> Result<(), PlatformError> {
    let class: &JClass = get_cached_class(CachedClass::UpdateManager).map_err(|e| {
        PlatformError::Platform(format!("Failed to get UpdateManager class: {}", e))
    })?;
    with_env(|env| -> Result<(), PlatformError> {
        env.call_static_method(
            class,
            jni_str!("showDownloadProgress"),
            jni_sig!("()V"),
            &[],
        )
        .map_err(|e| {
            PlatformError::Platform(format!("Failed to call showDownloadProgress: {}", e))
        })?;
        Ok(())
    })
}

/// Update download progress
pub fn update_download_progress(progress: i32) -> Result<(), PlatformError> {
    let class: &JClass = get_cached_class(CachedClass::UpdateManager).map_err(|e| {
        PlatformError::Platform(format!("Failed to get UpdateManager class: {}", e))
    })?;
    with_env(|env| -> Result<(), PlatformError> {
        env.call_static_method(
            class,
            jni_str!("updateDownloadProgress"),
            jni_sig!("(I)V"),
            &[JValue::Int(progress)],
        )
        .map_err(|e| {
            PlatformError::Platform(format!("Failed to call updateDownloadProgress: {}", e))
        })?;
        Ok(())
    })
}

/// Dismiss download progress dialog
pub fn dismiss_download_progress() -> Result<(), PlatformError> {
    let class: &JClass = get_cached_class(CachedClass::UpdateManager).map_err(|e| {
        PlatformError::Platform(format!("Failed to get UpdateManager class: {}", e))
    })?;
    with_env(|env| -> Result<(), PlatformError> {
        env.call_static_method(
            class,
            jni_str!("dismissDownloadProgress"),
            jni_sig!("()V"),
            &[],
        )
        .map_err(|e| {
            PlatformError::Platform(format!("Failed to call dismissDownloadProgress: {}", e))
        })?;
        Ok(())
    })
}

/// Show update confirmation prompt.
///
/// # Arguments
/// * `callback_id` - Callback ID for result
/// * `update_info_json` - Optional JSON string with update details:
///   {"version":"1.2.0","size":15728640,"releaseNotes":["..."],"isForceUpdate":true}
pub fn show_update_prompt(
    callback_id: u64,
    update_info_json: Option<&str>,
) -> Result<(), PlatformError> {
    let class: &JClass = get_cached_class(CachedClass::UpdateManager).map_err(|e| {
        PlatformError::Platform(format!("Failed to get UpdateManager class: {}", e))
    })?;
    with_env(|env| -> Result<(), PlatformError> {
        // Prepare the JSON string first
        let json_jstring = if let Some(json) = update_info_json {
            env.new_string(json).map_err(|e| {
                PlatformError::Platform(format!("Failed to create JSON string: {}", e))
            })?
        } else {
            env.new_string("").map_err(|e| {
                PlatformError::Platform(format!("Failed to create empty string: {}", e))
            })?
        };

        env.call_static_method(
            class,
            jni_str!("showUpdatePrompt"),
            jni_sig!("(JLjava/lang/String;)V"),
            &[
                JValue::Long(callback_id as i64),
                JValue::Object(&json_jstring),
            ],
        )
        .map_err(|e| PlatformError::Platform(format!("Failed to call showUpdatePrompt: {}", e)))?;
        Ok(())
    })
}
