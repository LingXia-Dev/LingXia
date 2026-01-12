use super::{CachedClass, get_cached_class};
use jni::objects::{JClass, JValue};
use lingxia_webview::get_env;

/// Show download progress dialog
pub fn show_download_progress() -> Result<(), String> {
    let mut env = get_env().map_err(|e| format!("Failed to get JNIEnv: {}", e))?;

    let update_manager_class = get_cached_class(CachedClass::UpdateManager)
        .map_err(|e| format!("Failed to get UpdateManager class: {}", e))?;

    let class_ref = env
        .new_local_ref(update_manager_class.as_obj())
        .map_err(|e| format!("Failed to create local ref: {}", e))?;
    let class = JClass::from(class_ref);

    env.call_static_method(class, "showDownloadProgress", "()V", &[])
        .map_err(|e| format!("Failed to call showDownloadProgress: {}", e))?;

    Ok(())
}

/// Update download progress
pub fn update_download_progress(progress: i32) -> Result<(), String> {
    let mut env = get_env().map_err(|e| format!("Failed to get JNIEnv: {}", e))?;

    let update_manager_class = get_cached_class(CachedClass::UpdateManager)
        .map_err(|e| format!("Failed to get UpdateManager class: {}", e))?;

    let class_ref = env
        .new_local_ref(update_manager_class.as_obj())
        .map_err(|e| format!("Failed to create local ref: {}", e))?;
    let class = JClass::from(class_ref);

    env.call_static_method(
        class,
        "updateDownloadProgress",
        "(I)V",
        &[JValue::Int(progress)],
    )
    .map_err(|e| format!("Failed to call updateDownloadProgress: {}", e))?;

    Ok(())
}

/// Dismiss download progress dialog
pub fn dismiss_download_progress() -> Result<(), String> {
    let mut env = get_env().map_err(|e| format!("Failed to get JNIEnv: {}", e))?;

    let update_manager_class = get_cached_class(CachedClass::UpdateManager)
        .map_err(|e| format!("Failed to get UpdateManager class: {}", e))?;

    let class_ref = env
        .new_local_ref(update_manager_class.as_obj())
        .map_err(|e| format!("Failed to create local ref: {}", e))?;
    let class = JClass::from(class_ref);

    env.call_static_method(class, "dismissDownloadProgress", "()V", &[])
        .map_err(|e| format!("Failed to call dismissDownloadProgress: {}", e))?;

    Ok(())
}

/// Show update confirmation prompt.
///
/// # Arguments
/// * `callback_id` - Callback ID for result
/// * `update_info_json` - Optional JSON string with update details: {"version":"1.2.0","size":15728640,"releaseNotes":["..."]}
pub fn show_update_prompt(callback_id: u64, update_info_json: Option<&str>) -> Result<(), String> {
    let mut env = get_env().map_err(|e| format!("Failed to get JNIEnv: {}", e))?;

    let update_manager_class = get_cached_class(CachedClass::UpdateManager)
        .map_err(|e| format!("Failed to get UpdateManager class: {}", e))?;

    // Prepare the JSON string first
    let json_jstring = if let Some(json) = update_info_json {
        env.new_string(json)
            .map_err(|e| format!("Failed to create JSON string: {}", e))?
    } else {
        env.new_string("")
            .map_err(|e| format!("Failed to create empty string: {}", e))?
    };

    // Now get the class reference and call the method
    let class_ref = env
        .new_local_ref(update_manager_class.as_obj())
        .map_err(|e| format!("Failed to create local ref: {}", e))?;
    let class = JClass::from(class_ref);

    env.call_static_method(
        class,
        "showUpdatePrompt",
        "(JLjava/lang/String;)V",
        &[
            JValue::Long(callback_id as i64),
            JValue::Object(&json_jstring),
        ],
    )
    .map_err(|e| format!("Failed to call showUpdatePrompt: {}", e))?;

    Ok(())
}
