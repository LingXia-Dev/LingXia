//! Harmony platform device implementation

use crate::DeviceInfo;
use crate::error::PlatformError;
use crate::traits::Device;
use std::process::Command;

use super::Platform;

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Clone, Copy)]
struct Vibrator_Attribute {
    vibrator_id: i32,
    usage: i32,
}

#[link(name = "ohvibrator.z")]
unsafe extern "C" {
    fn OH_Vibrator_PlayVibration(duration: i32, attribute: Vibrator_Attribute) -> i32;
}

const VIBRATION_DURATION_SHORT_MS: i32 = 15;
const VIBRATION_DURATION_LONG_MS: i32 = 400;
const DEFAULT_VIBRATOR_ID: i32 = 0;
const VIBRATOR_USAGE_ALARM: i32 = 1;

/// Get system parameter using param command
fn get_system_param(key: &str) -> Option<String> {
    Command::new("param")
        .arg("get")
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
        // Use pure Rust implementation with system commands
        let brand = get_system_param("const.product.brand")
            .or_else(|| get_system_param("ro.product.brand"))
            .unwrap_or_else(|| "Unknown".to_string());

        let model = get_system_param("const.product.model")
            .or_else(|| get_system_param("ro.product.model"))
            .unwrap_or_else(|| "Unknown".to_string());

        let os_version = get_system_param("const.ohos.version.security_patch")
            .or_else(|| get_system_param("const.ohos.fullname"))
            .unwrap_or_else(|| "Unknown".to_string());

        // Construct system string with HarmonyOS version
        let system = format!("HarmonyOS {}", os_version);

        DeviceInfo {
            brand,
            model,
            system,
        }
    }

    fn screen_info(&self, callback_id: u64) -> Result<(), PlatformError> {
        lingxia_webview::tsfn::call_arkts("getScreenInfo", &[&callback_id.to_string()]).map_err(
            |e| {
                // Send error via callback
                lingxia_messaging::invoke_callback(
                    callback_id,
                    false,
                    format!("Failed to get screen info: {}", e),
                );
                PlatformError::Platform(format!("Failed to get screen info: {}", e))
            },
        )
    }

    fn vibrate(&self, long: bool) -> Result<(), PlatformError> {
        let duration = if long {
            VIBRATION_DURATION_LONG_MS
        } else {
            VIBRATION_DURATION_SHORT_MS
        };

        let attribute = Vibrator_Attribute {
            vibrator_id: DEFAULT_VIBRATOR_ID,
            usage: VIBRATOR_USAGE_ALARM,
        };

        let result = unsafe { OH_Vibrator_PlayVibration(duration, attribute) };
        if result == 0 {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to vibrate via OH_Vibrator_PlayVibration: error code {}",
                result
            )))
        }
    }

    fn make_phone_call(&self, phone_number: &str) -> Result<(), PlatformError> {
        lingxia_webview::tsfn::call_arkts("makePhoneCall", &[phone_number])
            .map_err(|e| PlatformError::Platform(format!("Failed to make phone call: {}", e)))
    }
}
