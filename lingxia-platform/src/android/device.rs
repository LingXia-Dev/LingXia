//! Android platform device implementation

use crate::DeviceInfo;
use crate::error::PlatformError;
use crate::traits::Device;
use jni::objects::JValue;
use lingxia_webview::get_env;
use std::process::Command;

use super::Platform;

/// Get Android system property using getprop command
fn get_system_property(key: &str) -> Option<String> {
    Command::new("getprop")
        .arg(key)
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            } else {
                None
            }
        })
}

// Platform Device trait implementation - direct implementation without delegation
impl Device for Platform {
    fn device_info(&self) -> DeviceInfo {
        // Use pure Rust implementation with system properties
        let brand = get_system_property("ro.product.brand")
            .or_else(|| get_system_property("ro.product.manufacturer"))
            .unwrap_or_else(|| "Unknown".to_string());

        let model = get_system_property("ro.product.model")
            .or_else(|| get_system_property("ro.product.name"))
            .unwrap_or_else(|| "Unknown".to_string());

        let android_version = get_system_property("ro.build.version.release")
            .unwrap_or_else(|| "Unknown".to_string());

        let system = format!("Android {}", android_version);

        DeviceInfo {
            brand,
            model,
            system,
        }
    }

    fn screen_info(&self, callback_id: u64) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let lxapp_class: &jni::objects::JClass = super::get_lxapp_class()?.as_obj().into();
            let mut jni_env = get_env()?;

            jni_env.call_static_method(
                lxapp_class,
                "getScreenInfo",
                "(J)V",
                &[(callback_id as jni::sys::jlong).into()],
            )?;
            Ok(())
        }() {
            Ok(_) => Ok(()),
            Err(e) => {
                lingxia_messaging::invoke_callback(
                    callback_id,
                    false,
                    format!("Failed to call getScreenInfo via JNI: {}", e),
                );
                Err(PlatformError::Platform(format!(
                    "Failed to call getScreenInfo via JNI: {}",
                    e
                )))
            }
        }
    }

    fn vibrate(&self, long: bool) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let lxapp_class: &jni::objects::JClass = super::get_lxapp_class()?.as_obj().into();
            let mut jni_env = get_env()?;

            jni_env.call_static_method(lxapp_class, "vibrate", "(Z)V", &[long.into()])?;
            Ok(())
        }() {
            Ok(_) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to vibrate via JNI: {}",
                e
            ))),
        }
    }

    fn make_phone_call(&self, phone_number: &str) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = get_env()?;
            let lxapp_class: &jni::objects::JClass = super::get_lxapp_class()?.as_obj().into();

            let phone_number_jstring = env.new_string(phone_number)?;

            env.call_static_method(
                lxapp_class,
                "makePhoneCall",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&phone_number_jstring)],
            )?;
            Ok(())
        }() {
            Ok(_) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to make phone call: {}",
                e
            ))),
        }
    }
}
