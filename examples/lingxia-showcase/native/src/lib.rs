//! Native library for the example app (builds to .so/.a).
//!
//! This crate:
//! 1. Links the core `lingxia` runtime into the final static/shared library
//! 2. Exports host addon registration via platform FFI (JNI/NAPI/C)

#[cfg(feature = "standard")]
mod extension;

struct ExampleHostAddon;

impl lingxia::HostAddon for ExampleHostAddon {
    #[cfg(feature = "standard")]
    fn install_logic_extensions(&self) {
        lingxia::js::register_logic_extension(Box::new(extension::HelloExtension));
        // Cloud provider (lx.cloud/auth + update/fingerprint/push). Must register in
        // this hook — the logic context is built before `start_services`. Injected via
        // `--with-provider cloud`.
        #[cfg(feature = "cloud")]
        if let Err(err) = lingxia_cloud_client::init(lingxia_cloud_client::CloudOptions::default()) {
            log::error!("[cloud] provider init failed: {err}");
        }
    }

    fn start_services(&self) {
        #[cfg(feature = "devtools")]
        lingxia_devtool::start_devtool_bridge_from_env();
    }
}

fn register_host_addon() {
    lingxia::register_host_addon(Box::new(ExampleHostAddon));
}

// Android: JNI export
#[cfg(target_os = "android")]
mod android {
    use jni::EnvUnowned;
    use jni::objects::JClass;

    #[inline]
    fn register_host_addon() {
        super::register_host_addon();
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_lingxia_example_lxapp_MainActivity_nativeRegisterHostAddon<
        'local,
    >(
        _env: EnvUnowned<'local>,
        _class: JClass<'local>,
    ) {
        register_host_addon();
    }

    // Backward-compatible symbol for older example app package IDs.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_julibits_lingxia_muke_MainActivity_nativeRegisterHostAddon<
        'local,
    >(
        _env: EnvUnowned<'local>,
        _class: JClass<'local>,
    ) {
        register_host_addon();
    }
}

// Harmony: NAPI export
#[cfg(target_env = "ohos")]
#[napi_derive_ohos::napi]
pub fn lingxia_register_host_addon() {
    register_host_addon();
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "C" fn lingxia_register_host_addon() {
    register_host_addon();
}

#[cfg(target_os = "windows")]
pub fn lingxia_register_host_addon() {
    register_host_addon();
}
