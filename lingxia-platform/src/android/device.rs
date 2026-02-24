//! Android platform device implementation

use crate::error::PlatformError;
use crate::traits::device::{Device, DeviceHardware, DeviceSecureStore};
use crate::{DeviceInfo, ScreenInfo};
use jni::objects::{JObject, JValue};
use lingxia_webview::get_env;
use std::fs;
use std::process::Command;

use super::Platform;

fn get_lxapp_context<'a>(
    env: &mut jni::JNIEnv<'a>,
) -> Result<JObject<'a>, Box<dyn std::error::Error>> {
    let lxapp_class: &jni::objects::JClass = super::get_cached_class(super::CachedClass::LxApp)?
        .as_obj()
        .into();

    let mut context_obj = env
        .call_static_method(
            lxapp_class,
            "getCurrentActivity",
            "()Lcom/lingxia/lxapp/LxAppActivity;",
            &[],
        )?
        .l()?;

    if context_obj.is_null() {
        context_obj = env
            .call_static_method(
                lxapp_class,
                "applicationContext",
                "()Landroid/content/Context;",
                &[],
            )?
            .l()?;
    }

    if context_obj.is_null() {
        return Err("LxApp context not available".into());
    }

    Ok(context_obj)
}

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

        let os_name = "Android".to_string();
        let os_version = get_system_property("ro.build.version.release")
            .unwrap_or_else(|| "Unknown".to_string());

        DeviceInfo {
            brand,
            model,
            market_name,
            os_name,
            os_version,
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

/// Helper to get Android ID via JNI (Settings.Secure.ANDROID_ID).
pub fn get_android_id() -> Option<String> {
    use jni::objects::JValue;

    match || -> Result<String, Box<dyn std::error::Error>> {
        let mut env = get_env()?;
        let context_obj = get_lxapp_context(&mut env)?;

        // Get ContentResolver
        let content_resolver = env
            .call_method(
                context_obj,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )?
            .l()?;

        // Settings.Secure.ANDROID_ID
        let settings_secure_class = env.find_class("android/provider/Settings$Secure")?;
        let android_id_key = env.new_string("android_id")?;

        let result = env.call_static_method(
            settings_secure_class,
            "getString",
            "(Landroid/content/ContentResolver;Ljava/lang/String;)Ljava/lang/String;",
            &[
                JValue::Object(&content_resolver),
                JValue::Object(&android_id_key),
            ],
        )?;

        let obj = result.l()?;
        if !obj.is_null() {
            let jstr: jni::objects::JString = obj.into();
            let rust_str = env.get_string(&jstr)?;
            let android_id = rust_str.to_string_lossy().trim().to_string();
            if !android_id.is_empty() {
                return Ok(android_id);
            }
        }

        Err("Settings.Secure.getString(android_id) returned empty".into())
    }() {
        Ok(s) => Some(s),
        Err(e) => {
            log::warn!("Failed to get Android ID via JNI: {}", e);
            None
        }
    }
}

impl DeviceHardware for Platform {
    fn get_memory_info(&self) -> Result<u64, PlatformError> {
        // Read /proc/meminfo
        if let Ok(contents) = fs::read_to_string("/proc/meminfo") {
            for line in contents.lines() {
                if line.starts_with("MemTotal:") {
                    // Example: MemTotal:        3942548 kB
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<u64>() {
                            return Ok(kb * 1024);
                        }
                    }
                }
            }
        }
        Err(PlatformError::Platform(
            "Failed to read /proc/meminfo".to_string(),
        ))
    }

    fn get_cpu_count(&self) -> usize {
        // Method 1: Check /sys/devices/system/cpu/present (e.g., "0-7")
        if let Ok(present) = fs::read_to_string("/sys/devices/system/cpu/present") {
            // Format can be "0-7" or "0-3,4-7"
            let count = present
                .trim()
                .split(',')
                .map(|range| {
                    let parts: Vec<&str> = range.split('-').collect();
                    if parts.len() == 2 {
                        if let (Ok(start), Ok(end)) =
                            (parts[0].parse::<usize>(), parts[1].parse::<usize>())
                        {
                            if end >= start {
                                return end - start + 1;
                            }
                        }
                    } else if parts.len() == 1 {
                        if parts[0].parse::<usize>().is_ok() {
                            return 1;
                        }
                    }
                    0
                })
                .sum::<usize>();
            if count > 0 {
                return count;
            }
        }

        // Method 2: Count /sys/devices/system/cpu/cpu[0-9]+ directories
        if let Ok(entries) = fs::read_dir("/sys/devices/system/cpu") {
            let count = entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    if let Ok(name) = entry.file_name().into_string() {
                        name.starts_with("cpu")
                            && name.len() > 3
                            && name[3..].chars().all(|c| c.is_ascii_digit())
                    } else {
                        false
                    }
                })
                .count();
            if count > 0 {
                return count;
            }
        }

        // Fallback: Return 1 for stability (do NOT use available_parallelism)
        1
    }

    fn get_storage_total_bytes(&self) -> Result<u64, PlatformError> {
        // Use JNI to get StatFs for data directory
        match || -> Result<u64, Box<dyn std::error::Error>> {
            let mut env = get_env()?;

            // Environment.getDataDirectory()
            let env_class = env.find_class("android/os/Environment")?;
            let data_dir = env
                .call_static_method(env_class, "getDataDirectory", "()Ljava/io/File;", &[])?
                .l()?;

            // file.getPath()
            let path_str_obj = env
                .call_method(data_dir, "getPath", "()Ljava/lang/String;", &[])?
                .l()?;
            let path_jstr: jni::objects::JString = path_str_obj.into();

            // new StatFs(path)
            let statfs_class = env.find_class("android/os/StatFs")?;
            let statfs = env.new_object(
                statfs_class,
                "(Ljava/lang/String;)V",
                &[JValue::Object(&path_jstr)],
            )?;

            // statfs.getTotalBytes()
            let total_bytes = env.call_method(statfs, "getTotalBytes", "()J", &[])?.j()?;

            Ok(total_bytes as u64)
        }() {
            Ok(bytes) => Ok(bytes),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to get storage info: {}",
                e
            ))),
        }
    }
}

impl DeviceSecureStore for Platform {}

/// Get Android API level (SDK_INT).
pub fn get_api_level() -> i32 {
    get_system_property("ro.build.version.sdk")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Check if device has telephony feature (is a phone).
pub fn has_telephony_feature() -> bool {
    match || -> Result<bool, Box<dyn std::error::Error>> {
        let mut env = get_env()?;
        let context = get_lxapp_context(&mut env)?;

        // PackageManager pm = context.getPackageManager()
        let pm = env
            .call_method(
                context,
                "getPackageManager",
                "()Landroid/content/pm/PackageManager;",
                &[],
            )?
            .l()?;

        // pm.hasSystemFeature(PackageManager.FEATURE_TELEPHONY)
        let feature_str = env.new_string("android.hardware.telephony")?;
        let has_feature = env
            .call_method(
                pm,
                "hasSystemFeature",
                "(Ljava/lang/String;)Z",
                &[JValue::Object(&feature_str)],
            )?
            .z()?;

        Ok(has_feature)
    }() {
        Ok(result) => result,
        Err(e) => {
            log::warn!("[device] Failed to check telephony feature: {}", e);
            false
        }
    }
}
