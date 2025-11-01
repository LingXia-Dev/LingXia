//! Android platform device implementation

use crate::error::PlatformError;
use crate::traits::Device;
use crate::{DeviceInfo, ScreenInfo};
use jni::objects::{JObject, JValue};
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

        let market_name = get_system_property("ro.product.marketname")
            .or_else(|| get_system_property("ro.config.marketing_name"))
            .unwrap_or_else(|| model.clone());

        let android_version = get_system_property("ro.build.version.release")
            .unwrap_or_else(|| "Unknown".to_string());

        let system = format!("Android {}", android_version);

        DeviceInfo {
            brand,
            model,
            market_name,
            system,
        }
    }

    fn screen_info(&self) -> ScreenInfo {
        // Synchronous retrieval via JNI: getCurrentActivity -> Resources -> DisplayMetrics
        match || -> Result<ScreenInfo, Box<dyn std::error::Error>> {
            let mut env = get_env()?;

            // Get current activity
            let lxapp_class: &jni::objects::JClass =
                super::get_cached_class(super::CachedClass::LxApp)?
                    .as_obj()
                    .into();

            let activity_obj = env
                .call_static_method(
                    lxapp_class,
                    "getCurrentActivity",
                    "()Lcom/lingxia/lxapp/LxAppActivity;",
                    &[],
                )?
                .l()?;

            // If activity is null, fall back to defaults
            if activity_obj.is_null() {
                return Ok(ScreenInfo {
                    width: 0.0,
                    height: 0.0,
                    scale: 1.0,
                });
            }

            // resources = activity.getResources()
            let resources: JObject = env
                .call_method(
                    activity_obj,
                    "getResources",
                    "()Landroid/content/res/Resources;",
                    &[],
                )?
                .l()?;

            // metrics = resources.getDisplayMetrics()
            let metrics: JObject = env
                .call_method(
                    resources,
                    "getDisplayMetrics",
                    "()Landroid/util/DisplayMetrics;",
                    &[],
                )?
                .l()?;

            // Read widthPixels, heightPixels, density from DisplayMetrics
            let width_px = env.get_field(&metrics, "widthPixels", "I")?.i()? as f64;
            let height_px = env.get_field(&metrics, "heightPixels", "I")?.i()? as f64;
            let density = env.get_field(&metrics, "density", "F")?.f()? as f64;

            let scale = if density > 0.0 {
                (density * 10.0).round() / 10.0
            } else {
                1.0
            };
            let width = (width_px / density).round();
            let height = (height_px / density).round();

            Ok(ScreenInfo {
                width,
                height,
                scale,
            })
        }() {
            Ok(info) => info,
            Err(_) => ScreenInfo {
                width: 0.0,
                height: 0.0,
                scale: 1.0,
            },
        }
    }

    fn vibrate(&self, long: bool) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let device_class: &jni::objects::JClass =
                super::get_cached_class(super::CachedClass::LxAppDevice)?
                    .as_obj()
                    .into();
            let mut jni_env = get_env()?;

            jni_env.call_static_method(device_class, "vibrate", "(Z)V", &[long.into()])?;
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
            let device_class: &jni::objects::JClass =
                super::get_cached_class(super::CachedClass::LxAppDevice)?
                    .as_obj()
                    .into();

            let phone_number_jstring = env.new_string(phone_number)?;

            env.call_static_method(
                device_class,
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
