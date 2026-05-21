//! Android platform device implementation

use super::with_env;
use crate::error::PlatformError;
use crate::traits::device::{Device, DeviceHardware};
use crate::traits::secure_store::SecureStore;
use crate::{DeviceInfo, ScreenInfo};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use jni::objects::{JClass, JObject, JString, JValue};
use jni::signature::MethodSignature;
use jni::strings::JNIStr;
use jni::{Env, jni_sig, jni_str};
use std::fs;
use std::process::Command;

use super::Platform;

fn host_context<'a>(env: &mut Env<'a>) -> Result<JObject<'a>, Box<dyn std::error::Error>> {
    super::application_context(env)
        .ok_or_else(|| "Application context not registered via set_application_context".into())
}

/// Get Android system property using getprop command
pub fn get_system_property(key: &str) -> Option<String> {
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
        let result = with_env(|env| -> Result<ScreenInfo, Box<dyn std::error::Error>> {
            // Use the registered application context — available from
            // `lingxia_platform::Platform::from_java` onward, well before any
            // Activity has a chance to attach. This sidesteps the timing
            // window where lxapp module-level JS calls getScreenInfo before
            // the first Activity reaches onCreate.
            let context_obj = host_context(env)?;

            let resources: JObject = env
                .call_method(
                    context_obj,
                    jni_str!("getResources"),
                    jni_sig!("()Landroid/content/res/Resources;"),
                    &[],
                )?
                .l()?;
            let metrics: JObject = env
                .call_method(
                    resources,
                    jni_str!("getDisplayMetrics"),
                    jni_sig!("()Landroid/util/DisplayMetrics;"),
                    &[],
                )?
                .l()?;

            let width_px = env
                .get_field(&metrics, jni_str!("widthPixels"), jni_sig!("I"))?
                .i()? as f64;
            let height_px = env
                .get_field(&metrics, jni_str!("heightPixels"), jni_sig!("I"))?
                .i()? as f64;
            let density = env
                .get_field(&metrics, jni_str!("density"), jni_sig!("F"))?
                .f()? as f64;
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
        });

        result.unwrap_or(ScreenInfo {
            width: 0.0,
            height: 0.0,
            scale: 1.0,
        })
    }

    fn vibrate(&self, long: bool) -> Result<(), PlatformError> {
        let device_class: &jni::objects::JClass =
            super::get_cached_class(super::CachedClass::LxAppDevice)
                .map_err(|e| PlatformError::Platform(e.to_string()))?;
        with_env(|env| -> Result<(), PlatformError> {
            env.call_static_method(
                device_class,
                jni_str!("vibrate"),
                jni_sig!("(Z)V"),
                &[long.into()],
            )?;
            Ok(())
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to vibrate via JNI: {}", e)))
    }

    fn make_phone_call(&self, phone_number: &str) -> Result<(), PlatformError> {
        let device_class: &jni::objects::JClass =
            super::get_cached_class(super::CachedClass::LxAppDevice)
                .map_err(|e| PlatformError::Platform(e.to_string()))?;
        with_env(|env| -> Result<(), PlatformError> {
            let phone_number_jstring = env.new_string(phone_number)?;
            env.call_static_method(
                device_class,
                jni_str!("makePhoneCall"),
                jni_sig!("(Ljava/lang/String;)V"),
                &[JValue::Object(&phone_number_jstring)],
            )?;
            Ok(())
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to make phone call: {}", e)))
    }
}

/// Helper to get Android ID via JNI (Settings.Secure.ANDROID_ID).
pub fn get_android_id() -> Option<String> {
    use jni::objects::JValue;

    match with_env(|env| -> Result<String, Box<dyn std::error::Error>> {
        let context_obj = host_context(env)?;

        // Get ContentResolver
        let content_resolver = env
            .call_method(
                context_obj,
                jni_str!("getContentResolver"),
                jni_sig!("()Landroid/content/ContentResolver;"),
                &[],
            )?
            .l()?;

        // Settings.Secure.ANDROID_ID
        let settings_secure_class = env.find_class(jni_str!("android/provider/Settings$Secure"))?;
        let android_id_key = env.new_string("android_id")?;

        let result = env.call_static_method(
            settings_secure_class,
            jni_str!("getString"),
            jni_sig!("(Landroid/content/ContentResolver;Ljava/lang/String;)Ljava/lang/String;"),
            &[
                JValue::Object(&content_resolver),
                JValue::Object(&android_id_key),
            ],
        )?;

        let obj = result.l()?;
        if !obj.is_null() {
            let jstr = unsafe { jni::objects::JString::from_raw(env, obj.into_raw() as _) };
            let android_id = jstr.try_to_string(env)?;
            let android_id = android_id.trim().to_string();
            if !android_id.is_empty() {
                return Ok(android_id);
            }
        }
        Err("Settings.Secure.getString(android_id) returned empty".into())
    }) {
        Ok(s) => Some(s),
        Err(e) => {
            log::warn!("Failed to get Android ID via JNI: {}", e);
            None
        }
    }
}

/// Read text from shared external storage via Android SDK bridge.
///
/// The SDK resolves `storage_key` to `/sdcard/.lingxia/<appid>/<storage_key>`.
pub fn read_external_storage_text(storage_key: &str) -> Result<Option<String>, PlatformError> {
    let device_class: &jni::objects::JClass =
        super::get_cached_class(super::CachedClass::LxAppDevice)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;

    with_env(
        |env| -> Result<Option<String>, Box<dyn std::error::Error>> {
            let storage_key_jstring = env.new_string(storage_key)?;
            let result = env.call_static_method(
                device_class,
                jni_str!("readExternalStorageText"),
                jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
                &[JValue::Object(&storage_key_jstring)],
            )?;

            let obj = result.l()?;
            if obj.is_null() {
                return Ok(None);
            }

            let value = unsafe { JString::from_raw(env, obj.into_raw() as _) };
            let text = value.try_to_string(env)?;
            Ok(Some(text))
        },
    )
    .map_err(|e| PlatformError::Platform(format!("Failed to read external storage text: {}", e)))
}

/// Write text to shared external storage via Android SDK bridge.
///
/// Returns `Ok(true)` on success, `Ok(false)` on denied/unavailable, and `Err` for JNI failure.
pub fn write_external_storage_text(storage_key: &str, value: &str) -> Result<bool, PlatformError> {
    let device_class: &jni::objects::JClass =
        super::get_cached_class(super::CachedClass::LxAppDevice)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;

    with_env(|env| -> Result<bool, Box<dyn std::error::Error>> {
        let storage_key_jstring = env.new_string(storage_key)?;
        let value_jstring = env.new_string(value)?;
        let result = env.call_static_method(
            device_class,
            jni_str!("writeExternalStorageText"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Z"),
            &[
                JValue::Object(&storage_key_jstring),
                JValue::Object(&value_jstring),
            ],
        )?;

        Ok(result.z()?)
    })
    .map_err(|e| PlatformError::Platform(format!("Failed to write external storage text: {}", e)))
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
        match with_env(|env| -> Result<u64, Box<dyn std::error::Error>> {
            let env_class = env.find_class(jni_str!("android/os/Environment"))?;
            let data_dir = env
                .call_static_method(
                    env_class,
                    jni_str!("getDataDirectory"),
                    jni_sig!("()Ljava/io/File;"),
                    &[],
                )?
                .l()?;

            let path_str_obj = env
                .call_method(
                    data_dir,
                    jni_str!("getPath"),
                    jni_sig!("()Ljava/lang/String;"),
                    &[],
                )?
                .l()?;
            let path_jstr =
                unsafe { jni::objects::JString::from_raw(env, path_str_obj.into_raw() as _) };

            let statfs_class = env.find_class(jni_str!("android/os/StatFs"))?;
            let statfs = env.new_object(
                statfs_class,
                jni_sig!("(Ljava/lang/String;)V"),
                &[JValue::Object(&path_jstr)],
            )?;

            let total_bytes = env
                .call_method(statfs, jni_str!("getTotalBytes"), jni_sig!("()J"), &[])?
                .j()?;

            Ok(total_bytes as u64)
        }) {
            Ok(bytes) => Ok(bytes),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to get storage info: {}",
                e
            ))),
        }
    }
}

fn get_lxapp_device_class() -> Result<&'static JClass<'static>, PlatformError> {
    super::get_cached_class(super::CachedClass::LxAppDevice)
        .map(|class| class.as_ref())
        .map_err(|e| PlatformError::Platform(e.to_string()))
}

fn call_lxapp_device_string_method<'sig, 'sig_args, N, S>(
    method_name: N,
    signature: S,
    error_context: &str,
    storage_key: &str,
) -> Result<Option<String>, PlatformError>
where
    N: AsRef<JNIStr>,
    S: AsRef<MethodSignature<'sig, 'sig_args>>,
{
    let device_class = get_lxapp_device_class()?;

    with_env(
        |env| -> Result<Option<String>, Box<dyn std::error::Error>> {
            let storage_key_jstring = env.new_string(storage_key)?;
            let result = env.call_static_method(
                device_class,
                method_name,
                signature,
                &[JValue::Object(&storage_key_jstring)],
            )?;

            let obj = result.l()?;
            if obj.is_null() {
                return Ok(None);
            }

            let value = unsafe { JString::from_raw(env, obj.into_raw() as _) };
            let text = value.try_to_string(env)?;
            Ok(Some(text))
        },
    )
    .map_err(|e| PlatformError::Platform(format!("Failed to {}: {}", error_context, e)))
}

fn call_lxapp_device_void_method<'sig, 'sig_args, N, S>(
    method_name: N,
    signature: S,
    error_context: &str,
    storage_key: &str,
    value_base64: Option<&str>,
) -> Result<(), PlatformError>
where
    N: AsRef<JNIStr>,
    S: AsRef<MethodSignature<'sig, 'sig_args>>,
{
    let device_class = get_lxapp_device_class()?;

    with_env(|env| -> Result<(), Box<dyn std::error::Error>> {
        let storage_key_jstring = env.new_string(storage_key)?;
        let value_jstring = value_base64
            .map(|value| env.new_string(value))
            .transpose()?;

        let mut args = vec![JValue::Object(&storage_key_jstring)];
        if let Some(value_jstring) = value_jstring.as_ref() {
            args.push(JValue::Object(value_jstring));
        }

        env.call_static_method(device_class, method_name, signature, &args)?;
        Ok(())
    })
    .map_err(|e| PlatformError::Platform(format!("Failed to {}: {}", error_context, e)))
}

fn read_secure_store_base64(storage_key: &str) -> Result<Option<String>, PlatformError> {
    call_lxapp_device_string_method(
        jni_str!("readSecureStoreValueBase64"),
        jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
        "read Android secure store",
        storage_key,
    )
}

fn write_secure_store_base64(storage_key: &str, value_base64: &str) -> Result<(), PlatformError> {
    call_lxapp_device_void_method(
        jni_str!("writeSecureStoreValueBase64"),
        jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
        "write Android secure store",
        storage_key,
        Some(value_base64),
    )
}

fn delete_secure_store_value(storage_key: &str) -> Result<(), PlatformError> {
    call_lxapp_device_void_method(
        jni_str!("deleteSecureStoreValue"),
        jni_sig!("(Ljava/lang/String;)V"),
        "delete Android secure store",
        storage_key,
        None,
    )
}

impl SecureStore for Platform {
    fn read(&self, key: &str) -> Result<Option<Vec<u8>>, PlatformError> {
        let Some(encoded) = read_secure_store_base64(key)? else {
            return Ok(None);
        };

        STANDARD.decode(encoded).map(Some).map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to decode Android secure store value for key {}: {}",
                key, e
            ))
        })
    }

    fn write(&self, key: &str, value: &[u8]) -> Result<(), PlatformError> {
        write_secure_store_base64(key, &STANDARD.encode(value))
    }

    fn delete(&self, key: &str) -> Result<(), PlatformError> {
        delete_secure_store_value(key)
    }
}

/// Get Android API level (SDK_INT).
pub fn get_api_level() -> i32 {
    get_system_property("ro.build.version.sdk")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Check if device has telephony feature (is a phone).
pub fn has_telephony_feature() -> bool {
    match with_env(|env| -> Result<bool, Box<dyn std::error::Error>> {
        let context = host_context(env)?;
        let pm = env
            .call_method(
                context,
                jni_str!("getPackageManager"),
                jni_sig!("()Landroid/content/pm/PackageManager;"),
                &[],
            )?
            .l()?;
        let feature_str = env.new_string("android.hardware.telephony")?;
        let has_feature = env
            .call_method(
                pm,
                jni_str!("hasSystemFeature"),
                jni_sig!("(Ljava/lang/String;)Z"),
                &[JValue::Object(&feature_str)],
            )?
            .z()?;
        Ok(has_feature)
    }) {
        Ok(result) => result,
        Err(e) => {
            log::warn!("[device] Failed to check telephony feature: {}", e);
            false
        }
    }
}
