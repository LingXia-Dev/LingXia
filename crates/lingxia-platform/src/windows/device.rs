use super::{Platform, file, not_supported};
use crate::error::PlatformError;
use crate::traits::device::Device;
use crate::{DeviceInfo, ScreenInfo};

impl Device for Platform {
    fn device_info(&self) -> DeviceInfo {
        DeviceInfo {
            brand: "Microsoft".to_string(),
            model: std::env::consts::ARCH.to_string(),
            market_name: "Windows PC".to_string(),
            os_name: "Windows".to_string(),
            os_version: String::new(),
        }
    }

    fn screen_info(&self) -> ScreenInfo {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        ScreenInfo {
            width: width.max(0) as f64,
            height: height.max(0) as f64,
            scale: 1.0,
        }
    }

    fn vibrate(&self, _long: bool) -> Result<(), PlatformError> {
        not_supported("vibrate")
    }

    fn make_phone_call(&self, phone_number: &str) -> Result<(), PlatformError> {
        // Sync trait method: launch without waiting so the executor never blocks.
        file::open_with_shell_detached(&format!("tel:{phone_number}"))
    }
}
