//! Android platform location implementation

use crate::error::PlatformError;
use crate::traits::location::Location;
use jni::objects::JValue;
use jni::sys::{JNI_FALSE, JNI_TRUE};
use jni::{jni_sig, jni_str};

use super::Platform;

impl Location for Platform {
    fn is_location_enabled(&self) -> Result<bool, PlatformError> {
        match || -> Result<bool, Box<dyn std::error::Error>> {
            let location_class: &jni::objects::JClass =
                super::get_cached_class(super::CachedClass::LxAppLocation)?.as_ref();

            lingxia_webview::with_env(|env| {
                let result = env.call_static_method(
                    location_class,
                    jni_str!("isLocationEnabled"),
                    jni_sig!("()Z"),
                    &[],
                )?;
                Ok(result.z()?)
            })
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
        config: crate::traits::location::LocationRequestConfig,
    ) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let location_class: &jni::objects::JClass =
                super::get_cached_class(super::CachedClass::LxAppLocation)?.as_ref();

            lingxia_webview::with_env(|env| {
                // Pass configuration parameters to Android implementation
                env.call_static_method(
                    location_class,
                    jni_str!("requestLocation"),
                    jni_sig!("(JZZI)V"),
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
            })
        }() {
            Ok(()) => Ok(()),
            Err(e) => {
                lingxia_messaging::invoke_callback(callback_id, Err(1000));
                Err(PlatformError::Platform(format!(
                    "Failed to request location via JNI: {}",
                    e
                )))
            }
        }
    }
}
