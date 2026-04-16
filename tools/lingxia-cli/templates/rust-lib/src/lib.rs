//! Native library for the example app (builds to .so/.a).
//!
//! This crate:
//! 1. Re-exports platform FFI symbols from lingxia
//! 2. Exports host addon installation via platform FFI (JNI/NAPI/C)

// Re-export platform FFI symbols from lingxia
#[cfg(target_os = "android")]
pub use lingxia::android::*;

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use lingxia::apple::*;

#[cfg(target_env = "ohos")]
pub use lingxia::harmony::*;

struct AppHostAddon;

impl lingxia::HostAddon for AppHostAddon {
    fn install_logic_extensions(&self) {
        // Register custom logic extensions here
        // Example: lingxia::register_logic_extension(Box::new(MyExtension));
    }
}

#[cfg(all(feature = "devtools", any(target_os = "ios", target_os = "macos")))]
struct DevtoolAddon;

#[cfg(all(feature = "devtools", any(target_os = "ios", target_os = "macos")))]
impl lingxia::HostAddon for DevtoolAddon {
    fn start_services(&self) {
        lingxia_devtool::start_devtool_bridge_from_env();
    }
}

fn install_host_addons() {
    lingxia::install_host_addon(Box::new(AppHostAddon));
    #[cfg(all(feature = "devtools", any(target_os = "ios", target_os = "macos")))]
    lingxia::install_host_addon(Box::new(DevtoolAddon));
}

// Android: JNI export
#[cfg(target_os = "android")]
mod android {
    use jni::EnvUnowned;
    use jni::objects::JClass;

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_{{PACKAGE_ID_UNDERSCORE}}_MainActivity_nativeInstallHostAddon(
        _env: EnvUnowned,
        _class: JClass,
    ) {
        super::install_host_addons();
    }
}

// Harmony: NAPI export
#[cfg(target_env = "ohos")]
#[napi_derive_ohos::napi]
pub fn lingxia_install_host_addon() {
    install_host_addons();
}

// iOS/macOS: C export
#[cfg(any(target_os = "ios", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "C" fn lingxia_install_host_addon() {
    install_host_addons();
}
