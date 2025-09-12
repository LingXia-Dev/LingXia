use super::ffi;
use crate::error::PlatformError;
use crate::{AppRuntime, AssetFileEntry, DeviceInfo};
use std::ffi::CStr;
use std::io::{Cursor, Read};
use std::mem;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use objc2_foundation::NSProcessInfo;

#[cfg(target_os = "ios")]
use objc2::{ClassType, extern_class, msg_send, rc::Retained, runtime::NSObject};

#[cfg(target_os = "ios")]
use objc2_foundation::NSString;

#[cfg(target_os = "ios")]
extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    #[unsafe(super(NSObject))]
    pub struct UIDevice;
);

#[cfg(target_os = "ios")]
impl UIDevice {
    pub fn current() -> Retained<Self> {
        unsafe { msg_send![Self::class(), currentDevice] }
    }

    #[allow(dead_code)]
    pub fn model(&self) -> Retained<NSString> {
        unsafe { msg_send![self, model] }
    }

    pub fn system_version(&self) -> Retained<NSString> {
        unsafe { msg_send![self, systemVersion] }
    }
}

/// Platform implementation for Apple platforms (iOS/macOS)
#[derive(Clone)]
pub struct Platform {
    pub data_dir: String,
    pub cache_dir: String,
}

unsafe impl Send for Platform {}
unsafe impl Sync for Platform {}

impl Platform {
    /// Create a new Platform instance
    pub fn new(data_dir: String, cache_dir: String) -> Result<Self, PlatformError> {
        Ok(Platform {
            data_dir,
            cache_dir,
        })
    }
}

impl AppRuntime for Platform {
    fn app_data_dir(&self) -> PathBuf {
        PathBuf::from(&self.data_dir)
    }

    fn app_cache_dir(&self) -> PathBuf {
        PathBuf::from(&self.cache_dir)
    }

    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, PlatformError> {
        let data = super::resources::read_asset_data(path);

        if data.is_empty() {
            Err(PlatformError::AssetNotFound(path.to_string()))
        } else {
            Ok(Box::new(Cursor::new(data)))
        }
    }

    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, PlatformError>> + 'a> {
        let entries = self.collect_files_recursively(asset_dir);
        Box::new(entries.into_iter())
    }

    fn device_info(&self) -> DeviceInfo {
        let brand = "Apple".to_string(); // Fixed for Apple devices
        let model = get_device_model();
        let system = get_system_version();

        DeviceInfo {
            brand,
            model,
            system,
        }
    }

    fn open_lxapp(&self, appid: String, path: String) -> Result<(), PlatformError> {
        if ffi::open_lxapp(&appid, &path) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to open lxapp: appid={}, path={}",
                appid, path
            )))
        }
    }

    fn close_lxapp(&self, appid: String) -> Result<(), PlatformError> {
        if ffi::close_lxapp(&appid) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to close lxapp: appid={}",
                appid
            )))
        }
    }

    fn navigate(
        &self,
        appid: String,
        path: String,
        animation_type: crate::traits::AnimationType,
    ) -> Result<(), PlatformError> {
        if ffi::navigate(&appid, &path, animation_type as i32) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to navigate: appid={}, path={}, animation_type={:?}",
                appid, path, animation_type
            )))
        }
    }

    fn launch_with_url(&self, url: String) -> Result<(), PlatformError> {
        ffi::launch_with_url(&url);
        Ok(())
    }
}

impl Platform {
    /// Recursively collect all files from a directory
    fn collect_files_recursively<'a>(
        &'a self,
        dir_path: &str,
    ) -> Vec<Result<AssetFileEntry<'a>, PlatformError>> {
        let mut all_files = Vec::new();
        let mut dirs_to_process = vec![dir_path.to_string()];

        while let Some(current_dir) = dirs_to_process.pop() {
            let contents = super::resources::list_asset_directory(&current_dir);

            for name in contents {
                let full_path = if current_dir.is_empty() || current_dir == "/" {
                    name.clone()
                } else {
                    format!("{}/{}", current_dir.trim_end_matches('/'), name)
                };

                // Try to read as file first
                let data = super::resources::read_asset_data(&full_path);

                if !data.is_empty() {
                    // It's a file, add it to results
                    let reader: Box<dyn Read + 'a> = Box::new(Cursor::new(data));
                    all_files.push(Ok(AssetFileEntry {
                        path: full_path,
                        reader,
                    }));
                } else {
                    // It might be a directory, try to list it
                    let sub_contents = super::resources::list_asset_directory(&full_path);
                    if !sub_contents.is_empty() {
                        // It's a directory with contents, add it to processing queue
                        dirs_to_process.push(full_path);
                    }
                }
            }
        }

        all_files
    }
}

/// Get device model using system calls (like Swift version)
/// Returns device model string like "iPhone14,2" or "MacBookPro18,1"
fn get_device_model() -> String {
    // Define utsname structure manually to avoid libc dependency
    #[repr(C)]
    struct UtsName {
        sysname: [i8; 256],
        nodename: [i8; 256],
        release: [i8; 256],
        version: [i8; 256],
        machine: [i8; 256],
    }

    unsafe extern "C" {
        fn uname(buf: *mut UtsName) -> i32;
    }

    unsafe {
        let mut system_info: UtsName = mem::zeroed();
        if uname(&mut system_info) == 0 {
            // Convert machine field to String (like Swift version)
            let machine_ptr = system_info.machine.as_ptr();
            let machine_cstr = CStr::from_ptr(machine_ptr);
            if let Ok(machine_str) = machine_cstr.to_str() {
                return machine_str.to_string();
            }
        }

        // Fallback if utsname fails
        #[cfg(target_os = "ios")]
        {
            "iPhone".to_string()
        }
        #[cfg(target_os = "macos")]
        {
            "Mac".to_string()
        }
    }
}

/// Get system version using objc2 bindings
/// Returns system version string like "iOS 17.0" or "macOS 14.0"
fn get_system_version() -> String {
    #[cfg(target_os = "ios")]
    {
        let device = UIDevice::current();
        let version = device.system_version();
        format!("iOS {}", version.to_string())
    }

    #[cfg(target_os = "macos")]
    {
        // Get NSProcessInfo shared instance
        let process_info = NSProcessInfo::processInfo();

        // Get operating system version (returns NSOperatingSystemVersion struct)
        let version = process_info.operatingSystemVersion();

        // Format as simple version string (e.g., "macOS 15.5")
        format!("macOS {}.{}", version.majorVersion, version.minorVersion)
    }
}
