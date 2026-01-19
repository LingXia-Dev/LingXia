use crate::error::PlatformError;
use crate::traits::pull_to_refresh::PullToRefresh;
use jni::objects::{JClass, JValue};
use lingxia_webview::get_env;

use super::Platform;

impl PullToRefresh for Platform {
    fn start_pull_down_refresh(&self, app_id: &str, path: &str) -> Result<(), PlatformError> {
        call_pull_to_refresh("startPullDownRefresh", app_id, path)
    }

    fn stop_pull_down_refresh(&self, app_id: &str, path: &str) -> Result<(), PlatformError> {
        call_pull_to_refresh("stopPullDownRefresh", app_id, path)
    }
}

fn call_pull_to_refresh(method: &str, app_id: &str, path: &str) -> Result<(), PlatformError> {
    let mut env =
        get_env().map_err(|e| PlatformError::Platform(format!("Failed to get JNIEnv: {}", e)))?;

    let clazz = super::get_cached_class(super::CachedClass::LxAppPullToRefresh)
        .map_err(|e| PlatformError::Platform(e.to_string()))?
        .as_obj();
    let clazz: &JClass = clazz.into();

    let app_id_jstring = env
        .new_string(app_id)
        .map_err(|e| PlatformError::Platform(format!("Failed to create app_id string: {:?}", e)))?;

    let path_jstring = env
        .new_string(path)
        .map_err(|e| PlatformError::Platform(format!("Failed to create path string: {:?}", e)))?;

    if let Err(e) = env.call_static_method(
        clazz,
        method,
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[
            JValue::Object(&app_id_jstring),
            JValue::Object(&path_jstring),
        ],
    ) {
        let _ = env.exception_clear();
        return Err(PlatformError::Platform(format!(
            "Failed to call {}: {:?}",
            method, e
        )));
    }

    Ok(())
}
