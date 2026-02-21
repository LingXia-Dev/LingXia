use crate::error::PlatformError;
use crate::traits::network::Network;
use jni::objects::{JClass, JValue};
use lingxia_webview::get_env;

use super::Platform;

fn call_network_method(method_name: &str, callback_id: u64) -> Result<(), PlatformError> {
    let mut env =
        get_env().map_err(|e| PlatformError::Platform(format!("Failed to get JNI env: {}", e)))?;
    let network_class_ref = super::get_cached_class(super::CachedClass::LxAppNetwork)
        .map_err(|e| PlatformError::Platform(format!("Failed to get LxAppNetwork class: {}", e)))?;
    // Convert cached global class ref to a `JClass` descriptor for this call.
    let network_class: &JClass = network_class_ref.as_obj().into();

    env.call_static_method(
        network_class,
        method_name,
        "(J)V",
        &[JValue::Long(callback_id as i64)],
    )
    .map_err(|e| PlatformError::Platform(format!("JNI call to {} failed: {}", method_name, e)))?;

    Ok(())
}

impl Network for Platform {
    fn get_network_info(&self, callback_id: u64) -> Result<(), PlatformError> {
        call_network_method("getNetworkInfo", callback_id)
    }

    fn add_network_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        call_network_method("addNetworkChangeListener", callback_id)
    }

    fn remove_network_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        call_network_method("removeNetworkChangeListener", callback_id)
    }
}
