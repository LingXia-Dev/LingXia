//! Native library for the example app (builds to .so/.a).
//!
//! This crate:
//! 1. Re-exports platform FFI symbols from lingxia
//! 2. Exports extension registration via platform FFI (JNI/C)
//! 3. Contains user-defined JS logic extensions

mod hello;

// Re-export platform FFI symbols from lingxia
#[cfg(target_os = "android")]
pub use lingxia::android::*;

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use lingxia::apple::*;

#[cfg(target_env = "ohos")]
pub use lingxia::harmony::*;

/// Internal: Register all extensions.
fn do_register_extensions() {
    // Initialize cloud (provider + JS extension) when feature is enabled
    #[cfg(feature = "cloud")]
    lingxia_cloud::init();

    // Register app-specific extensions
    lingxia::register_logic_extension(Box::new(hello::HelloExtension));
}

#[cfg(target_os = "android")]
mod android {
    use jni::JNIEnv;
    use jni::objects::JClass;

    /// Called from Kotlin: MainActivity.registerNativeExtensions()
    /// Package path matches the example app: com.lingxia.example.lxapp
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_lingxia_example_lxapp_MainActivity_registerNativeExtensions(
        _env: JNIEnv,
        _class: JClass,
    ) {
        super::do_register_extensions();
    }
}

// Harmony: Export via NAPI
#[cfg(target_env = "ohos")]
#[napi_derive_ohos::napi]
pub fn lingxia_register_extensions() {
    do_register_extensions();
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "C" fn lingxia_register_extensions() {
    do_register_extensions();
}
