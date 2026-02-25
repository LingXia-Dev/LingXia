//! Native library for the example app (builds to .so/.a).
//!
//! This crate:
//! 1. Re-exports platform FFI symbols from lingxia
//! 2. Exports extension registration via platform FFI (JNI/NAPI/C)

// Re-export platform FFI symbols from lingxia
#[cfg(target_os = "android")]
pub use lingxia::android::*;

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use lingxia::apple::*;

#[cfg(target_env = "ohos")]
pub use lingxia::harmony::*;

// Custom Extensions Registration
// This function is called from the native host before LxApp initialization.
fn do_register_extensions() {
    // Register custom extensions here
    // Example: lingxia::register_logic_extension(Box::new(MyExtension));
}

// Android: JNI export
#[cfg(target_os = "android")]
mod android {
    use jni::EnvUnowned;
    use jni::objects::JClass;

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_{{PACKAGE_ID_UNDERSCORE}}_MainActivity_registerNativeExtensions(
        _env: EnvUnowned,
        _class: JClass,
    ) {
        super::do_register_extensions();
    }
}

// Harmony: NAPI export
#[cfg(target_env = "ohos")]
#[napi_derive_ohos::napi]
pub fn lingxia_register_extensions() {
    do_register_extensions();
}

// iOS/macOS: C export
#[cfg(any(target_os = "ios", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "C" fn lingxia_register_extensions() {
    do_register_extensions();
}
