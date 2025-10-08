//! Android platform location implementation

use crate::error::PlatformError;
use crate::traits::Location;
use jni::objects::JValue;
use jni::sys::{JNI_FALSE, JNI_TRUE};
use lingxia_webview::get_env;

use super::Platform;

impl Location for Platform {
    fn is_location_enabled(&self) -> Result<bool, PlatformError> {
        match || -> Result<bool, Box<dyn std::error::Error>> {
            let location_class: &jni::objects::JClass =
                super::get_cached_class(super::CachedClass::LxAppLocation)?
                    .as_obj()
                    .into();
            let mut env = get_env()?;

            let result = env.call_static_method(location_class, "isLocationEnabled", "()Z", &[])?;
            Ok(result.z()?)
        }() {
            Ok(value) => Ok(value),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to determine location availability via JNI: {}",
                e
            ))),
        }
    }

    fn request_location(
        &self,
        callback_id: u64,
        config: crate::LocationRequestConfig,
    ) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let location_class: &jni::objects::JClass =
                super::get_cached_class(super::CachedClass::LxAppLocation)?
                    .as_obj()
                    .into();
            let mut env = get_env()?;

            // Pass configuration parameters to Android implementation
            env.call_static_method(
                location_class,
                "requestLocation",
                "(JZZI)V",
                &[
                    JValue::Long(callback_id as i64),
                    JValue::Bool(if config.is_high_accuracy {
                        JNI_TRUE
                    } else {
                        JNI_FALSE
                    }),
                    JValue::Bool(if config.include_altitude {
                        JNI_TRUE
                    } else {
                        JNI_FALSE
                    }),
                    JValue::Int(config.high_accuracy_expire_time.unwrap_or(10000) as i32),
                ],
            )?;
            Ok(())
        }() {
            Ok(()) => Ok(()),
            Err(e) => {
                lingxia_messaging::invoke_callback(
                    callback_id,
                    false,
                    format!("Failed to request location via JNI: {}", e),
                );
                Err(PlatformError::Platform(format!(
                    "Failed to request location via JNI: {}",
                    e
                )))
            }
        }
    }
}
