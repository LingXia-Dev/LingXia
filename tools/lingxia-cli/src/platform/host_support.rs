use super::detector::PlatformType;
use super::{apple, windows};
use anyhow::Result;
#[cfg(not(target_os = "windows"))]
use anyhow::anyhow;

pub(crate) fn ensure_supported_host(platform: &PlatformType) -> Result<()> {
    match platform {
        PlatformType::Ios | PlatformType::MacOs => apple::ensure_macos(),
        PlatformType::Windows => windows::ensure_supported_host(),
        PlatformType::Android | PlatformType::Harmony => Ok(()),
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn unsupported_host(platform: &PlatformType) -> anyhow::Error {
    anyhow!(
        "{} is not supported on this host ({})",
        platform.as_str(),
        std::env::consts::OS
    )
}
