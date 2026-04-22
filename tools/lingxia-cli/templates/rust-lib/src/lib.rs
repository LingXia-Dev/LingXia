//! Native library for the example app (builds to .so/.a).
//!
//! This crate:
//! 1. Re-exports platform FFI symbols from lingxia
//! 2. Exports host addon registration via platform FFI (JNI/NAPI/C)

// Re-export platform FFI symbols from lingxia
#[cfg(target_os = "android")]
pub use lingxia::android::*;

{{APPLE_REEXPORT}}
{{HARMONY_REEXPORT}}

struct AppHostAddon;

impl lingxia::HostAddon for AppHostAddon {
    fn start_services(&self) {
        #[cfg(all(feature = "devtools", any(target_os = "ios", target_os = "macos")))]
        lingxia_devtool::start_devtool_bridge_from_env();
    }
}

fn register_host_addons() {
    lingxia::register_host_addon(Box::new(AppHostAddon));
}

{{ANDROID_EXPORT_BLOCK}}
{{HARMONY_EXPORT_BLOCK}}
{{APPLE_EXPORT_BLOCK}}
