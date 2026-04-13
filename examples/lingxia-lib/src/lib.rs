//! Native library for the example app (builds to .so/.a).
//!
//! This crate:
//! 1. Links the core `lingxia` runtime into the final static/shared library
//! 2. Exports host addon installation via platform FFI (JNI/NAPI/C)

mod extension;

struct ExampleAppAddon;

impl lingxia::HostAddon for ExampleAppAddon {
    fn install_logic_extensions(&self) {
        #[cfg(feature = "cloud")]
        let _ = lingxia_cloud::init();
        lingxia::register_logic_extension(Box::new(extension::HelloExtension));
    }
}

fn install_host_addon() {
    lingxia::install_host_addon(Box::new(ExampleAppAddon));
}

// Android: JNI export
#[cfg(target_os = "android")]
mod android {
    use jni::EnvUnowned;
    use jni::objects::JClass;

    #[inline]
    fn install_host_addon() {
        super::install_host_addon();
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_lingxia_example_lxapp_MainActivity_nativeInstallHostAddon<
        'local,
    >(
        _env: EnvUnowned<'local>,
        _class: JClass<'local>,
    ) {
        install_host_addon();
    }

    // Backward-compatible symbol for older example app package IDs.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_julibits_lingxia_muke_MainActivity_nativeInstallHostAddon<
        'local,
    >(
        _env: EnvUnowned<'local>,
        _class: JClass<'local>,
    ) {
        install_host_addon();
    }
}

// Harmony: NAPI export
#[cfg(target_env = "ohos")]
#[napi_derive_ohos::napi]
pub fn lingxia_install_host_addon() {
    install_host_addon();
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "C" fn lingxia_install_host_addon() {
    install_host_addon();
}
