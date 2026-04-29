use crate::error::PlatformError;
use crate::traits::network::Network;
use jni::jni_sig;
use jni::objects::{JClass, JValue};
use jni::strings::JNIString;

use super::Platform;

fn call_network_method(method_name: &str, callback_id: u64) -> Result<(), PlatformError> {
    let network_class: &JClass = super::get_cached_class(super::CachedClass::LxAppNetwork)
        .map_err(|e| PlatformError::Platform(format!("Failed to get LxAppNetwork class: {}", e)))?;
    let method = JNIString::new(method_name);

    super::with_env(|env| {
        env.call_static_method(
            network_class,
            &method,
            jni_sig!("(J)V"),
            &[JValue::Long(callback_id as i64)],
        )
        .map_err(|e| {
            PlatformError::Platform(format!("JNI call to {} failed: {}", method_name, e))
        })?;

        Ok(())
    })
}

impl Network for Platform {
    async fn get_network_info(&self) -> Result<String, PlatformError> {
        crate::rt::native_call(|callback_id| call_network_method("getNetworkInfo", callback_id))
            .await
    }

    fn add_network_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        call_network_method("addNetworkChangeListener", callback_id)
    }

    fn remove_network_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        call_network_method("removeNetworkChangeListener", callback_id)
    }
}
