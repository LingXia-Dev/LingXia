//! Caller-owned capture provider construction.
//!
//! The pipeline takes an explicit [`CaptureProviderSet`]. This module only
//! builds the providers a host declared — it is not a process-global registry.

pub use lingxia_media::capture::CaptureProviderSet;

/// Android provider for the tracks declared on this host. Empty when capture
/// is not declared or this is not an Android build.
#[cfg(all(feature = "android-capture", target_os = "android"))]
pub fn android_providers() -> CaptureProviderSet {
    let tracks = lingxia_app_context::capabilities_config()
        .map(|capabilities| capabilities.media_capture)
        .unwrap_or_default();
    if !tracks.is_enabled() {
        return CaptureProviderSet::new([]);
    }
    CaptureProviderSet::new([std::sync::Arc::new(
        lingxia_platform::capture::android::AndroidCaptureProvider::new(
            tracks.visual,
            tracks.system_audio,
            tracks.microphone,
        ),
    )])
}
