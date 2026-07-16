use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use windows::Win32::System::Registry::HKEY_LOCAL_MACHINE;
use windows::core::HSTRING;

use super::{Platform, not_supported};
use crate::error::PlatformError;
use crate::traits::device::{Device, DeviceHardware};
use crate::{DeviceInfo, ScreenInfo};

const BIOS_KEY: &str = r"HARDWARE\DESCRIPTION\System\BIOS";
const CURRENT_VERSION_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

impl Device for Platform {
    fn device_info(&self) -> DeviceInfo {
        static INFO: OnceLock<DeviceInfo> = OnceLock::new();
        INFO.get_or_init(collect_device_info).clone()
    }

    fn screen_info(&self) -> ScreenInfo {
        use windows::Win32::UI::HiDpi::GetDpiForSystem;
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

        // GetSystemMetrics reports pixels at the DPI the process perceives, so
        // dividing by the system DPI yields logical (scale-independent)
        // dimensions. The process currently declares no DPI awareness, which
        // means the system virtualizes DPI to 96 and scale evaluates to 1.0;
        // once a manifest declares awareness, this same math reports the real
        // scale factor with no further changes.
        let dpi = unsafe { GetDpiForSystem() };
        let scale = if dpi == 0 { 1.0 } else { f64::from(dpi) / 96.0 };
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        ScreenInfo {
            width: f64::from(width.max(0)) / scale,
            height: f64::from(height.max(0)) / scale,
            scale,
        }
    }

    fn vibrate(&self, _long: bool) -> Result<(), PlatformError> {
        // Genuinely N/A: desktop PCs have no vibration actuator to drive.
        not_supported("vibrate")
    }

    fn make_phone_call(&self, _phone_number: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "makePhoneCall is not supported on Windows".to_string(),
        ))
    }
}

impl DeviceHardware for Platform {
    fn get_memory_info(&self) -> Result<u64, PlatformError> {
        use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        unsafe { GlobalMemoryStatusEx(&mut status) }.map_err(|err| {
            PlatformError::Platform(format!("GlobalMemoryStatusEx failed: {err}"))
        })?;
        Ok(status.ullTotalPhys)
    }

    fn get_storage_total_bytes(&self) -> Result<u64, PlatformError> {
        use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

        // Report the volume hosting the app data directory; the volume root
        // always exists even before the data directory is created.
        let root = volume_root(self.data_dir());
        let root_name = HSTRING::from(root.as_os_str());
        let mut total_bytes = 0u64;
        unsafe { GetDiskFreeSpaceExW(&root_name, None, Some(&mut total_bytes), None) }.map_err(
            |err| {
                PlatformError::Platform(format!(
                    "GetDiskFreeSpaceExW failed for {}: {err}",
                    root.display()
                ))
            },
        )?;
        Ok(total_bytes)
    }
}

fn volume_root(path: &Path) -> PathBuf {
    let mut root = PathBuf::new();
    for component in path.components().take(2) {
        match component {
            Component::Prefix(_) | Component::RootDir => root.push(component.as_os_str()),
            _ => break,
        }
    }
    if root.as_os_str().is_empty() {
        std::env::var_os("SystemDrive")
            // `SystemDrive` is like `C:`; append the separator to get the drive
            // root (`join("\\")` would replace the path with `\`).
            .map(|drive| {
                PathBuf::from(format!(
                    "{}\\",
                    drive.to_string_lossy().trim_end_matches('\\')
                ))
            })
            .unwrap_or_else(|| PathBuf::from("C:\\"))
    } else {
        root
    }
}

fn collect_device_info() -> DeviceInfo {
    let brand =
        read_hklm_string(BIOS_KEY, "SystemManufacturer").unwrap_or_else(|| "Unknown".to_string());
    let model = read_hklm_string(BIOS_KEY, "SystemProductName")
        .unwrap_or_else(|| std::env::consts::ARCH.to_string());
    // SystemFamily carries the marketing line (e.g. "ThinkPad X1"); fall back
    // to the product name when the firmware leaves it blank.
    let market_name = read_hklm_string(BIOS_KEY, "SystemFamily").unwrap_or_else(|| model.clone());
    DeviceInfo {
        brand,
        model,
        market_name,
        os_name: crate::os_label().to_string(),
        os_version: os_version_string(),
    }
}

/// Builds e.g. "10.0.26100 (24H2)" from the kernel version plus the marketing
/// release in the registry.
fn os_version_string() -> String {
    let core = match rtl_get_version() {
        Some((major, minor, build)) => format!("{major}.{minor}.{build}"),
        None => read_hklm_string(CURRENT_VERSION_KEY, "CurrentBuild").unwrap_or_default(),
    };
    // DisplayVersion (e.g. "24H2") replaced ReleaseId in Windows 10 20H2+.
    match read_hklm_string(CURRENT_VERSION_KEY, "DisplayVersion") {
        Some(display) if !core.is_empty() => format!("{core} ({display})"),
        Some(display) => display,
        None => core,
    }
}

/// Kernel-reported version; unlike GetVersionEx this is not subject to
/// manifest-based compatibility shims, so Windows 10/11 builds are accurate.
fn rtl_get_version() -> Option<(u32, u32, u32)> {
    // OSVERSIONINFOW; declared locally because the pinned windows-rs rev only
    // exposes RtlGetVersion through the Wdk crate, which is not a dependency.
    #[repr(C)]
    struct OsVersionInfoW {
        os_version_info_size: u32,
        major_version: u32,
        minor_version: u32,
        build_number: u32,
        platform_id: u32,
        csd_version: [u16; 128],
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlGetVersion(version_information: *mut OsVersionInfoW) -> i32;
    }

    let mut info = OsVersionInfoW {
        os_version_info_size: std::mem::size_of::<OsVersionInfoW>() as u32,
        major_version: 0,
        minor_version: 0,
        build_number: 0,
        platform_id: 0,
        csd_version: [0; 128],
    };
    let status = unsafe { RtlGetVersion(&mut info) };
    (status == 0).then_some((info.major_version, info.minor_version, info.build_number))
}

fn read_hklm_string(subkey: &str, value: &str) -> Option<String> {
    let text = super::registry::read_string(HKEY_LOCAL_MACHINE, subkey, value)?;
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}
