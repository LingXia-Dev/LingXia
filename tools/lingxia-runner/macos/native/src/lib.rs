struct RunnerDevtoolAddon;

struct MacRunnerDeviceController;

unsafe extern "C" {
    fn lingxia_runner_device_list_json() -> *mut std::ffi::c_char;
    fn lingxia_runner_device_get_json() -> *mut std::ffi::c_char;
    /// `id` may be null (keep the current preset). `landscape`: -1 keep /
    /// 0 portrait / 1 landscape. `appearance`: -1 keep / 0 system / 1 light /
    /// 2 dark. `text_scale`: 0 keep, else the factor. `contrast`: -1 keep /
    /// 0 standard / 1 high.
    fn lingxia_runner_device_set_json(
        id: *const std::ffi::c_char,
        landscape: i32,
        appearance: i32,
        text_scale: f64,
        contrast: i32,
    ) -> *mut std::ffi::c_char;
}

fn take_runner_json(pointer: *mut std::ffi::c_char, action: &str) -> Result<String, String> {
    if pointer.is_null() {
        return Err(format!("macOS Runner failed to {action}"));
    }
    // Swift allocates this with `strdup`; copy before releasing it with the
    // matching process allocator.
    let value = unsafe { std::ffi::CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned();
    unsafe { libc::free(pointer.cast()) };
    Ok(value)
}

fn parse_runner_json<T: serde::de::DeserializeOwned>(
    pointer: *mut std::ffi::c_char,
    action: &str,
) -> Result<T, String> {
    let json = take_runner_json(pointer, action)?;
    serde_json::from_str(&json).map_err(|err| format!("invalid macOS Runner device state: {err}"))
}

impl lingxia::dev::DeviceController for MacRunnerDeviceController {
    fn list(&self) -> Result<Vec<lingxia::dev::DeviceEntry>, String> {
        parse_runner_json(unsafe { lingxia_runner_device_list_json() }, "list devices")
    }

    fn get(&self) -> Result<lingxia::dev::DeviceState, String> {
        parse_runner_json(
            unsafe { lingxia_runner_device_get_json() },
            "read the current device",
        )
    }

    fn set(
        &self,
        change: &lingxia::dev::DeviceChange,
    ) -> Result<lingxia::dev::DeviceState, String> {
        let entries = self.list()?;
        let target = match change.id.as_deref() {
            Some(id) => Some(
                entries
                    .iter()
                    .find(|entry| entry.id == id)
                    .ok_or_else(|| format!("unknown device id: {id}"))?,
            ),
            None => None,
        };
        // The Runner selects a posture through a `<device-id>:<posture-id>` selector.
        let selector = match change.posture.as_deref() {
            Some(posture) => {
                let entry = match target {
                    Some(entry) => entry,
                    None => entries
                        .iter()
                        .find(|entry| entry.current)
                        .ok_or("no current device")?,
                };
                if !entry.postures.iter().any(|p| p.id == posture) {
                    return Err(format!("device '{}' has no posture '{posture}'", entry.id));
                }
                Some(format!("{}:{posture}", entry.id))
            }
            None => target.map(|entry| entry.id.clone()),
        };
        let id = selector
            .map(|id| std::ffi::CString::new(id).map_err(|_| "device id contains NUL".to_string()))
            .transpose()?;
        let landscape = change.landscape.map_or(-1, i32::from);
        let contrast = change.contrast.map_or(-1, |contrast| match contrast {
            lingxia::dev::Contrast::Standard => 0,
            lingxia::dev::Contrast::High => 1,
        });
        let appearance = change.appearance.map_or(-1, |appearance| match appearance {
            lingxia::dev::Appearance::System => 0,
            lingxia::dev::Appearance::Light => 1,
            lingxia::dev::Appearance::Dark => 2,
        });
        parse_runner_json(
            unsafe {
                lingxia_runner_device_set_json(
                    id.as_ref().map_or(std::ptr::null(), |id| id.as_ptr()),
                    landscape,
                    appearance,
                    change.text_scale.unwrap_or(0.0),
                    contrast,
                )
            },
            "update the simulated environment",
        )
    }
}

impl lingxia::HostAddon for RunnerDevtoolAddon {
    fn before_init(&self) {
        lingxia::register_runner_host();
        // An injected provider owns the registry when one is linked in — only
        // one may register.
        #[cfg(not(feature = "cloud"))]
        if let Some(provider) = lingxia_runner_config::RunnerRegistryProvider::from_env() {
            lingxia::provider::register_lxapp_registry_provider(Box::new(provider));
        }
    }

    fn issue_devtools_app_resource_grants(
        &self,
        authority: &mut lingxia::NativeDevtoolsAuthority<'_>,
    ) {
        authority.grant_automation();
    }

    // Cloud provider (lx.cloud/auth + update/fingerprint/push). Must register in this
    // hook — the logic context is built before `start_services`. Injected via
    // `--with-provider cloud`.
    //
    // Server and product id come from `lingxia_runner_config`; without a
    // product id the Runner runs without the cloud client.
    #[cfg(feature = "cloud")]
    fn install_logic_extensions(&self) {
        use lingxia_cloud_client::CloudOptions;

        let cfg = lingxia_runner_config::from_env();
        let Some(id) = cfg.lingxia_id else {
            return;
        };
        let mut options = CloudOptions::default().lingxia_id(id);
        if let Some(server) = cfg.lingxia_server {
            options = options.lingxia_server(server);
        }
        if let Err(err) = lingxia_cloud_client::init(options) {
            eprintln!("[cloud] provider init failed: {err}");
        }
    }

    fn start_services(&self) {
        lingxia::dev::register_device_controller(Box::new(MacRunnerDeviceController));
        lingxia_control_runtime::start_dev_session_bridge_from_env();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lingxia_register_host_addon() {
    lingxia::register_host_addon(Box::new(RunnerDevtoolAddon));
}

/// Schedule the shared transition without blocking the AppKit main thread.
///
/// # Safety
/// `id` must be null or a NUL-terminated C string that stays valid for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lingxia_runner_request_device(id: *const std::ffi::c_char) {
    if id.is_null() {
        return;
    }
    let id = unsafe { std::ffi::CStr::from_ptr(id) }
        .to_string_lossy()
        .into_owned();
    lingxia::dev::request_device_set(id, None);
}
