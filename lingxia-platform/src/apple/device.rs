//! Apple platform device implementation
use super::Platform;
use crate::error::PlatformError;
use crate::traits::device::{Device, DeviceHardware};
use crate::traits::secure_store::SecureStore;
use crate::{DeviceInfo, ScreenInfo};

#[cfg(target_os = "ios")]
use objc2::rc::Retained;
#[cfg(target_os = "ios")]
use objc2::{ClassType, extern_class, msg_send, runtime::NSObject};
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

    pub fn system_version(&self) -> Retained<NSString> {
        unsafe { msg_send![self, systemVersion] }
    }

    pub fn localized_model(&self) -> Retained<NSString> {
        unsafe { msg_send![self, localizedModel] }
    }
}

// Platform Device trait implementation - direct implementation without delegation
impl Device for Platform {
    fn device_info(&self) -> DeviceInfo {
        let brand = "Apple".to_string(); // Fixed for Apple devices
        let model = get_device_model();
        let os_name = if cfg!(target_os = "ios") {
            "iOS".to_string()
        } else {
            "macOS".to_string()
        };
        let os_version = get_os_version();
        DeviceInfo {
            brand,
            model,
            market_name: self.market_name.clone(),
            os_name,
            os_version,
        }
    }

    fn screen_info(&self) -> ScreenInfo {
        #[cfg(target_os = "ios")]
        {
            use objc2::rc::Retained;
            use objc2::{ClassType, extern_class, msg_send};
            use objc2_foundation::{NSObject as ObjNSObject, NSRect};

            extern_class!(
                #[derive(Debug, PartialEq, Eq, Hash)]
                #[unsafe(super(ObjNSObject))]
                pub struct UIScreen;
            );

            impl UIScreen {
                pub fn main() -> Retained<Self> {
                    unsafe { msg_send![Self::class(), mainScreen] }
                }
            }

            let screen = UIScreen::main();
            let bounds: NSRect = unsafe { msg_send![&screen, bounds] };
            let scale: f64 = unsafe { msg_send![&screen, scale] };

            ScreenInfo {
                width: bounds.size.width,
                height: bounds.size.height,
                scale,
            }
        }
        #[cfg(target_os = "macos")]
        {
            use objc2::rc::Retained;
            use objc2::{ClassType, extern_class, msg_send};
            use objc2_foundation::{NSObject, NSRect};

            extern_class!(
                #[derive(Debug, PartialEq, Eq, Hash)]
                #[unsafe(super(NSObject))]
                pub struct NSScreen;
            );

            impl NSScreen {
                pub fn main() -> Option<Retained<Self>> {
                    unsafe { msg_send![Self::class(), mainScreen] }
                }
            }

            if let Some(screen) = NSScreen::main() {
                let frame: NSRect = unsafe { msg_send![&screen, frame] };
                let scale: f64 = unsafe { msg_send![&screen, backingScaleFactor] };
                ScreenInfo {
                    width: frame.size.width,
                    height: frame.size.height,
                    scale,
                }
            } else {
                // Fallback when no main screen is detected
                ScreenInfo {
                    width: 0.0,
                    height: 0.0,
                    scale: 1.0,
                }
            }
        }
    }

    #[cfg(target_os = "ios")]
    fn vibrate(&self, long: bool) -> Result<(), PlatformError> {
        use objc2_foundation::MainThreadMarker;

        if MainThreadMarker::new().is_some() {
            // On main thread - execute directly
            Self::execute_vibration(long);
        } else {
            // Off main thread - dispatch to main thread
            unsafe {
                let request = Box::new(MainThreadVibrationRequest { long });
                let context = Box::into_raw(request) as *mut std::ffi::c_void;
                dispatch_async_f(
                    main_dispatch_queue(),
                    context,
                    vibration_main_thread_callback,
                );
            }
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn vibrate(&self, _long: bool) -> Result<(), PlatformError> {
        // macOS doesn't have haptic feedback like iOS
        Err(PlatformError::NotSupported(
            "Vibration not available on macOS".to_string(),
        ))
    }

    #[cfg(not(any(target_os = "ios", target_os = "macos")))]
    fn vibrate(&self, _long: bool) -> Result<(), PlatformError> {
        log::warn!("Vibration not supported on this platform");
        Ok(())
    }

    fn make_phone_call(&self, phone_number: &str) -> Result<(), PlatformError> {
        #[cfg(target_os = "ios")]
        {
            use objc2::rc::Retained;
            use objc2::{ClassType, extern_class, msg_send};
            use objc2_foundation::{NSObject, NSString, NSURL};

            extern_class!(
                #[derive(Debug, PartialEq, Eq, Hash)]
                #[unsafe(super(NSObject))]
                pub struct UIApplication;
            );

            impl UIApplication {
                pub fn shared() -> Retained<Self> {
                    unsafe { msg_send![Self::class(), sharedApplication] }
                }

                pub fn can_open_url(&self, url: &NSURL) -> bool {
                    unsafe { msg_send![self, canOpenURL: url] }
                }

                pub fn open_url(&self, url: &NSURL) {
                    unsafe { msg_send![self, openURL: url] }
                }
            }

            let tel_url_string = format!("tel:{}", phone_number);
            let url_string = NSString::from_str(&tel_url_string);

            let url = unsafe {
                let url_ptr: *mut NSURL = msg_send![NSURL::class(), URLWithString: &*url_string];
                if url_ptr.is_null() {
                    return Err(PlatformError::InvalidParameter(
                        "Invalid phone number format".to_string(),
                    ));
                }
                Retained::retain(url_ptr).unwrap()
            };

            let app = UIApplication::shared();
            if app.can_open_url(&url) {
                app.open_url(&url);
                Ok(())
            } else {
                Err(PlatformError::Platform(
                    "Cannot open tel: URL on this device".to_string(),
                ))
            }
        }
        #[cfg(target_os = "macos")]
        {
            let _ = phone_number;
            Err(PlatformError::NotSupported(
                "makePhoneCall is not supported on macOS".to_string(),
            ))
        }
    }
}

/// Get device model identifier.
///
/// - iOS: `uname().machine` → e.g. "iPhone14,2", "iPad13,1"
/// - macOS: `sysctl hw.model` → e.g. "MacBookPro16,2", "Mac14,7"
fn get_device_model() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Some(model) = sysctl_string(b"hw.model\0") {
            return model;
        }
        "Mac".to_string()
    }

    #[cfg(target_os = "ios")]
    {
        use std::ffi::CStr;
        use std::mem;

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
                let machine_ptr = system_info.machine.as_ptr();
                let machine_cstr = CStr::from_ptr(machine_ptr);
                if let Ok(machine_str) = machine_cstr.to_str() {
                    return machine_str.to_string();
                }
            }
            "iPhone".to_string()
        }
    }
}

/// Read a sysctl string value by name.
#[cfg(target_os = "macos")]
fn sysctl_string(name: &[u8]) -> Option<String> {
    use std::ffi::CStr;

    unsafe extern "C" {
        fn sysctlbyname(
            name: *const i8,
            oldp: *mut std::ffi::c_void,
            oldlenp: *mut usize,
            newp: *mut std::ffi::c_void,
            newlen: usize,
        ) -> i32;
    }

    unsafe {
        let mut size: usize = 0;
        let name_ptr = name.as_ptr() as *const i8;
        if sysctlbyname(
            name_ptr,
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return None;
        }
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size];
        if sysctlbyname(
            name_ptr,
            buf.as_mut_ptr() as *mut std::ffi::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return None;
        }
        let cstr = CStr::from_ptr(buf.as_ptr() as *const i8);
        cstr.to_str().ok().map(|s| s.to_string())
    }
}

pub(super) fn load_platform_market_name() -> String {
    #[cfg(target_os = "ios")]
    {
        let model_identifier = get_device_model();
        let device = UIDevice::current();
        let localized = device.localized_model().to_string();
        if localized.is_empty() {
            model_identifier.to_string()
        } else {
            localized
        }
    }

    #[cfg(target_os = "macos")]
    {
        let model_identifier = get_device_model();
        get_macos_market_name().unwrap_or(model_identifier)
    }
}

#[cfg(target_os = "macos")]
fn get_macos_market_name() -> Option<String> {
    use std::process::Command;

    let output = Command::new("system_profiler")
        .arg("SPHardwareDataType")
        .arg("-detailLevel")
        .arg("mini")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("Model Name:") {
            let name = value.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    None
}

/// Get OS version only (without OS name prefix).
fn get_os_version() -> String {
    #[cfg(target_os = "ios")]
    {
        let device = UIDevice::current();
        let version = device.system_version();
        version.to_string()
    }

    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let version_str = String::from_utf8_lossy(&output.stdout);
                version_str.trim().to_string()
            }
            _ => "Unknown".to_string(),
        }
    }
}

#[cfg(target_os = "ios")]
impl Platform {
    fn execute_vibration(long: bool) {
        log::info!("iOS vibration called with long: {}", long);

        unsafe {
            if long {
                // Long vibration: loop the default vibration sound to emulate continuous buzz
                let vibration_data = Box::new(ContinuousVibrationData {
                    remaining_pulses: Self::CONTINUOUS_VIBRATION_PULSES,
                    pulse_interval_ns: Self::CONTINUOUS_VIBRATION_INTERVAL_NS,
                });

                // Kick off the first pulse immediately
                AudioServicesPlaySystemSound(K_SYSTEM_SOUND_ID_VIBRATE);

                if vibration_data.remaining_pulses > 1 {
                    let context = Box::into_raw(vibration_data) as *mut std::ffi::c_void;
                    let when =
                        dispatch_time(DISPATCH_TIME_NOW, Self::CONTINUOUS_VIBRATION_INTERVAL_NS);
                    dispatch_after_f(
                        when,
                        main_dispatch_queue(),
                        context,
                        continuous_vibration_callback,
                    );
                }
            } else {
                AudioServicesPlaySystemSound(Self::SHORT_VIBRATION_SOUND_ID);
            }
        }
    }

    const SHORT_VIBRATION_SOUND_ID: u32 = 1519; // Peek (short, ~15ms)
    const CONTINUOUS_VIBRATION_PULSES: usize = 8; // Loop ~400ms total
    const CONTINUOUS_VIBRATION_INTERVAL_NS: i64 = 50_000_000; // 50ms between pulses
}

#[cfg(target_os = "ios")]
struct MainThreadVibrationRequest {
    long: bool,
}

#[cfg(target_os = "ios")]
struct ContinuousVibrationData {
    remaining_pulses: usize,
    pulse_interval_ns: i64,
}

#[cfg(target_os = "ios")]
unsafe extern "C" fn vibration_main_thread_callback(context: *mut std::ffi::c_void) {
    if context.is_null() {
        log::error!("iOS vibration main thread callback received null context");
        return;
    }

    let request = unsafe { Box::from_raw(context as *mut MainThreadVibrationRequest) };
    Platform::execute_vibration(request.long);
}

#[cfg(target_os = "ios")]
unsafe extern "C" fn continuous_vibration_callback(context: *mut std::ffi::c_void) {
    if context.is_null() {
        log::error!("iOS continuous vibration callback received null context");
        return;
    }

    let data = unsafe { &mut *(context as *mut ContinuousVibrationData) };

    if data.remaining_pulses <= 1 {
        unsafe { drop(Box::from_raw(context as *mut ContinuousVibrationData)) };
        return;
    }

    unsafe {
        AudioServicesPlaySystemSound(K_SYSTEM_SOUND_ID_VIBRATE);
    };
    data.remaining_pulses -= 1;

    if data.remaining_pulses > 0 {
        let when = unsafe { dispatch_time(DISPATCH_TIME_NOW, data.pulse_interval_ns) };
        unsafe {
            dispatch_after_f(
                when,
                main_dispatch_queue(),
                context,
                continuous_vibration_callback,
            );
        };
    } else {
        unsafe { drop(Box::from_raw(context as *mut ContinuousVibrationData)) };
    }
}

#[cfg(target_os = "ios")]
type DispatchQueue = *mut std::ffi::c_void;

#[cfg(target_os = "ios")]
#[link(name = "System", kind = "dylib")]
#[link(name = "dispatch")]
#[link(name = "AudioToolbox", kind = "framework")]
unsafe extern "C" {
    fn dispatch_async_f(
        queue: DispatchQueue,
        context: *mut std::ffi::c_void,
        work: unsafe extern "C" fn(*mut std::ffi::c_void),
    );
    fn dispatch_after_f(
        when: DispatchTime,
        queue: DispatchQueue,
        context: *mut std::ffi::c_void,
        work: unsafe extern "C" fn(*mut std::ffi::c_void),
    );
    fn dispatch_time(when: DispatchTime, delta: i64) -> DispatchTime;

    // AudioServices functions for vibration
    fn AudioServicesPlaySystemSound(inSystemSoundID: u32);
}

#[cfg(target_os = "ios")]
type DispatchTime = u64;

#[cfg(target_os = "ios")]
const DISPATCH_TIME_NOW: DispatchTime = 0;

#[cfg(target_os = "ios")]
const K_SYSTEM_SOUND_ID_VIBRATE: u32 = 0x00000FFF;

#[cfg(target_os = "ios")]
unsafe fn main_dispatch_queue() -> DispatchQueue {
    unsafe extern "C" {
        static _dispatch_main_q: std::ffi::c_void;
    }

    std::ptr::addr_of!(_dispatch_main_q) as *const _ as DispatchQueue
}

impl SecureStore for Platform {
    fn read(&self, key: &str) -> Result<Option<Vec<u8>>, PlatformError> {
        keychain_read(key)
    }

    fn write(&self, key: &str, value: &[u8]) -> Result<(), PlatformError> {
        keychain_write(key, value)
    }

    fn delete(&self, key: &str) -> Result<(), PlatformError> {
        keychain_delete(key)
    }
}

impl DeviceHardware for Platform {
    fn get_memory_info(&self) -> Result<u64, PlatformError> {
        get_physical_memory()
    }

    fn get_cpu_count(&self) -> usize {
        use std::mem;

        // Use sysctl hw.physicalcpu (or hw.ncpu) to get stable core count
        unsafe extern "C" {
            fn sysctlbyname(
                name: *const i8,
                oldp: *mut std::ffi::c_void,
                oldlenp: *mut usize,
                newp: *mut std::ffi::c_void,
                newlen: usize,
            ) -> i32;
        }

        let mut count: i32 = 0;
        let mut size = mem::size_of::<i32>();

        // Try hw.physicalcpu first (physical cores, stable)
        let name = b"hw.physicalcpu\0";
        let res = unsafe {
            sysctlbyname(
                name.as_ptr() as *const i8,
                &mut count as *mut i32 as *mut std::ffi::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };

        if res == 0 && count > 0 {
            return count as usize;
        }

        // Fallback to hw.ncpu
        let name = b"hw.ncpu\0";
        let res = unsafe {
            sysctlbyname(
                name.as_ptr() as *const i8,
                &mut count as *mut i32 as *mut std::ffi::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };

        if res == 0 && count > 0 {
            return count as usize;
        }

        // Fallback to 1 for stability
        1
    }

    fn get_storage_total_bytes(&self) -> Result<u64, PlatformError> {
        get_total_storage_bytes()
    }
}

// ============================================================================
// Keychain Implementation
// ============================================================================

#[cfg(any(target_os = "ios", target_os = "macos"))]
const KEYCHAIN_SERVICE: &str = "com.lingxia.sdk";

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn keychain_read(key: &str) -> Result<Option<Vec<u8>>, PlatformError> {
    use security_framework::passwords::get_generic_password;

    match get_generic_password(KEYCHAIN_SERVICE, key) {
        Ok(data) => Ok(Some(data.to_vec())),
        Err(e) if e.code() == -25300 => Ok(None), // errSecItemNotFound
        Err(e) => Err(PlatformError::Platform(format!(
            "Keychain read failed (OSStatus {}: {}): {}",
            e.code(),
            keychain_status_name(e.code()),
            e
        ))),
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn keychain_write(key: &str, value: &[u8]) -> Result<(), PlatformError> {
    use security_framework::passwords::{delete_generic_password, set_generic_password};

    // Delete existing item first (if any)
    let _ = delete_generic_password(KEYCHAIN_SERVICE, key);

    set_generic_password(KEYCHAIN_SERVICE, key, value).map_err(|e| {
        PlatformError::Platform(format!(
            "Keychain write failed (OSStatus {}: {}): {}",
            e.code(),
            keychain_status_name(e.code()),
            e
        ))
    })
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn keychain_delete(key: &str) -> Result<(), PlatformError> {
    use security_framework::passwords::delete_generic_password;

    match delete_generic_password(KEYCHAIN_SERVICE, key) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == -25300 => Ok(()), // errSecItemNotFound - not an error
        Err(e) => Err(PlatformError::Platform(format!(
            "Keychain delete failed (OSStatus {}: {}): {}",
            e.code(),
            keychain_status_name(e.code()),
            e
        ))),
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn keychain_status_name(code: i32) -> &'static str {
    match code {
        -128 => "user_canceled",
        -25300 => "item_not_found",
        -25308 => "interaction_not_allowed",
        -25293 => "auth_failed",
        -34018 => "missing_entitlement",
        _ => "unknown",
    }
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
fn keychain_read(_key: &str) -> Result<Option<Vec<u8>>, PlatformError> {
    Err(PlatformError::Platform(
        "Keychain not available on this platform".to_string(),
    ))
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
fn keychain_write(_key: &str, _value: &[u8]) -> Result<(), PlatformError> {
    Err(PlatformError::Platform(
        "Keychain not available on this platform".to_string(),
    ))
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
fn keychain_delete(_key: &str) -> Result<(), PlatformError> {
    Err(PlatformError::Platform(
        "Keychain not available on this platform".to_string(),
    ))
}

// ============================================================================
// Hardware Info Implementation
// ============================================================================

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn get_physical_memory() -> Result<u64, PlatformError> {
    use std::mem;

    unsafe extern "C" {
        fn sysctl(
            name: *const i32,
            namelen: u32,
            oldp: *mut std::ffi::c_void,
            oldlenp: *mut usize,
            newp: *mut std::ffi::c_void,
            newlen: usize,
        ) -> i32;
    }

    // CTL_HW = 6, HW_MEMSIZE = 24
    // Note: This returns the physical memory available to the OS.
    // On iOS/SoC devices, this may be less than the advertised RAM (e.g. ~3.76GB on a 4GB device)
    // because some memory is reserved for hardware (GPU, Secure Enclave, etc.).
    let mib: [i32; 2] = [6, 24];
    let mut mem_size: u64 = 0;
    let mut size = mem::size_of::<u64>();

    let result = unsafe {
        sysctl(
            mib.as_ptr(),
            2,
            &mut mem_size as *mut u64 as *mut std::ffi::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };

    if result == 0 {
        Ok(mem_size)
    } else {
        Err(PlatformError::Platform(
            "Failed to get physical memory".to_string(),
        ))
    }
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
fn get_physical_memory() -> Result<u64, PlatformError> {
    Err(PlatformError::Platform(
        "get_physical_memory not implemented".to_string(),
    ))
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn get_total_storage_bytes() -> Result<u64, PlatformError> {
    #[cfg(target_os = "ios")]
    {
        use objc2::{ClassType, msg_send};
        use objc2_foundation::{NSDictionary, NSFileManager, NSString};

        unsafe {
            let file_manager: *mut NSFileManager =
                msg_send![NSFileManager::class(), defaultManager];
            if file_manager.is_null() {
                return Err(PlatformError::Platform(
                    "Failed to get NSFileManager".to_string(),
                ));
            }

            // Get attributes of root filesystem
            let path = NSString::from_str("/");
            let attrs: *mut NSDictionary<NSString, objc2_foundation::NSObject> = msg_send![file_manager, attributesOfFileSystemForPath: &*path, error: std::ptr::null_mut::<*mut objc2_foundation::NSError>()];

            if attrs.is_null() {
                return Err(PlatformError::Platform(
                    "Failed to get filesystem attributes".to_string(),
                ));
            }

            let size_key = NSString::from_str("NSFileSystemSize");
            let size_obj: *mut objc2_foundation::NSNumber =
                msg_send![attrs, objectForKey: &*size_key];

            if size_obj.is_null() {
                return Err(PlatformError::Platform(
                    "Failed to get filesystem size".to_string(),
                ));
            }

            let total_bytes: u64 = msg_send![size_obj, unsignedLongLongValue];
            Ok(total_bytes)
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let output = Command::new("df")
            .arg("-k")
            .arg("/")
            .output()
            .map_err(|e| PlatformError::Platform(format!("Failed to run df: {}", e)))?;

        if !output.status.success() {
            return Err(PlatformError::Platform("df command failed".to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(kb) = parts[1].parse::<u64>() {
                    let total_bytes = kb * 1024;
                    return Ok(total_bytes);
                }
            }
        }

        Err(PlatformError::Platform(
            "Failed to parse df output".to_string(),
        ))
    }
}
