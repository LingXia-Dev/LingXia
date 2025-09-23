//! Apple platform device implementation
use super::Platform;
use crate::error::PlatformError;
use crate::traits::Device;
use crate::{DeviceInfo, ScreenInfo};

#[cfg(target_os = "ios")]
use objc2::{extern_class, runtime::NSObject};

#[cfg(target_os = "ios")]
extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    #[unsafe(super(NSObject))]
    pub struct UIDevice;
);

// Platform Device trait implementation - direct implementation without delegation
impl Device for Platform {
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

    fn screen_info(&self, callback_id: u64) -> Result<(), PlatformError> {
        #[cfg(target_os = "ios")]
        {
            use objc2::rc::Retained;
            use objc2::{ClassType, extern_class, msg_send};
            use objc2_foundation::{NSObject, NSRect};

            extern_class!(
                #[derive(Debug, PartialEq, Eq, Hash)]
                #[unsafe(super(NSObject))]
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

            let screen_info = ScreenInfo {
                width: bounds.size.width,
                height: bounds.size.height,
                scale,
            };

            // Immediately invoke callback with result
            match serde_json::to_string(&screen_info) {
                Ok(json_data) => {
                    lingxia_messaging::invoke_callback(callback_id, true, json_data);
                    Ok(())
                }
                Err(e) => {
                    lingxia_messaging::invoke_callback(
                        callback_id,
                        false,
                        format!("JSON serialization error: {}", e),
                    );
                    Err(PlatformError::Platform(format!(
                        "JSON serialization error: {}",
                        e
                    )))
                }
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

                let screen_info = ScreenInfo {
                    width: frame.size.width,
                    height: frame.size.height,
                    scale,
                };

                // Immediately invoke callback with result
                match serde_json::to_string(&screen_info) {
                    Ok(json_data) => {
                        lingxia_messaging::invoke_callback(callback_id, true, json_data);
                        Ok(())
                    }
                    Err(e) => {
                        lingxia_messaging::invoke_callback(
                            callback_id,
                            false,
                            format!("JSON serialization error: {}", e),
                        );
                        Err(PlatformError::Platform(format!(
                            "JSON serialization error: {}",
                            e
                        )))
                    }
                }
            } else {
                lingxia_messaging::invoke_callback(
                    callback_id,
                    false,
                    "No main screen available on macOS".to_string(),
                );
                Err(PlatformError::Platform(
                    "No main screen available on macOS".to_string(),
                ))
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
        Err(PlatformError::Platform(
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
            // macOS can open tel: URLs but they'll be handled by default app (FaceTime, etc.)
            use std::process::Command;

            let tel_url = format!("tel:{}", phone_number);
            let result = Command::new("open").arg(&tel_url).output();

            match result {
                Ok(output) => {
                    if output.status.success() {
                        Ok(())
                    } else {
                        Err(PlatformError::Platform(
                            "Failed to open phone call URL".to_string(),
                        ))
                    }
                }
                Err(e) => Err(PlatformError::Platform(format!(
                    "Failed to execute open command: {}",
                    e
                ))),
            }
        }
    }
}

/// Get device model using system calls (like Swift version)
/// Returns device model string like "iPhone14,2" or "MacBookPro18,1"
fn get_device_model() -> String {
    use std::ffi::CStr;
    use std::mem;

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
    use objc2::rc::Retained;
    #[cfg(target_os = "ios")]
    use objc2::{ClassType, extern_class, msg_send};
    #[cfg(target_os = "ios")]
    use objc2_foundation::{NSObject, NSString};

    #[cfg(target_os = "ios")]
    {
        extern_class!(
            #[derive(Debug, PartialEq, Eq, Hash)]
            #[unsafe(super(NSObject))]
            pub struct UIDevice;
        );

        impl UIDevice {
            pub fn current() -> Retained<Self> {
                unsafe { msg_send![Self::class(), currentDevice] }
            }

            pub fn system_version(&self) -> Retained<NSString> {
                unsafe { msg_send![self, systemVersion] }
            }
        }

        let device = UIDevice::current();
        let version = device.system_version();
        format!("iOS {}", version.to_string())
    }

    #[cfg(target_os = "macos")]
    {
        // Use a simpler approach to get macOS version
        let output = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let version_str = String::from_utf8_lossy(&output.stdout);
                let version_str = version_str.trim();
                format!("macOS {}", version_str)
            }
            _ => {
                // Fallback to a generic version
                "macOS 14.0".to_string()
            }
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
                    let when = dispatch_time(DISPATCH_TIME_NOW, Self::CONTINUOUS_VIBRATION_INTERVAL_NS);
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

    AudioServicesPlaySystemSound(K_SYSTEM_SOUND_ID_VIBRATE);
    data.remaining_pulses -= 1;

    if data.remaining_pulses > 0 {
        let when = dispatch_time(DISPATCH_TIME_NOW, data.pulse_interval_ns);
        dispatch_after_f(
            when,
            main_dispatch_queue(),
            context,
            continuous_vibration_callback,
        );
    } else {
        unsafe { drop(Box::from_raw(context as *mut ContinuousVibrationData)) };
    }
}

#[cfg(target_os = "ios")]
type DispatchQueue = *mut std::ffi::c_void;

#[cfg(target_os = "ios")]
#[link(name = "System", kind = "dylib")]
#[link(name = "dispatch", kind = "dylib")]
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
