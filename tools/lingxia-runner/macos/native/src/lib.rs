struct RunnerDevtoolAddon;

struct MacRunnerDeviceController;

unsafe extern "C" {
    fn lingxia_runner_device_list_json() -> *mut std::ffi::c_char;
    fn lingxia_runner_device_get_json() -> *mut std::ffi::c_char;
    /// `id` may be null (keep the current preset). `landscape`: -1 keep /
    /// 0 portrait / 1 landscape. `appearance`: -1 keep / 0 system / 1 light /
    /// 2 dark.
    fn lingxia_runner_device_set_json(
        id: *const std::ffi::c_char,
        landscape: i32,
        appearance: i32,
        capsule: i32,
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
        id: Option<&str>,
        landscape: Option<bool>,
        appearance: Option<lingxia::dev::Appearance>,
        capsule: Option<bool>,
    ) -> Result<lingxia::dev::DeviceState, String> {
        if let Some(id) = id {
            let entries = self.list()?;
            if !entries.iter().any(|entry| entry.id == id) {
                return Err(format!("unknown device id: {id}"));
            }
        }
        let id = id
            .map(|id| std::ffi::CString::new(id).map_err(|_| "device id contains NUL".to_string()))
            .transpose()?;
        let landscape = landscape.map_or(-1, i32::from);
        let appearance = appearance.map_or(-1, |appearance| match appearance {
            lingxia::dev::Appearance::System => 0,
            lingxia::dev::Appearance::Light => 1,
            lingxia::dev::Appearance::Dark => 2,
        });
        let capsule = capsule.map_or(-1, i32::from);
        parse_runner_json(
            unsafe {
                lingxia_runner_device_set_json(
                    id.as_ref().map_or(std::ptr::null(), |id| id.as_ptr()),
                    landscape,
                    appearance,
                    capsule,
                )
            },
            "update the simulated environment",
        )
    }
}

impl lingxia::HostAddon for RunnerDevtoolAddon {
    // Cloud provider (lx.cloud/auth + update/fingerprint/push). Must register in this
    // hook — the logic context is built before `start_services`. Injected via
    // `--with-provider cloud`.
    //
    // We bring the cloud client up, so we configure its endpoint here: a user can
    // point the runner at a server / identity via ~/.lingxia/runner/config.toml
    // without rebuilding. Unset fields fall back to the app config.
    #[cfg(feature = "cloud")]
    fn install_logic_extensions(&self) {
        use lingxia_cloud_client::CloudOptions;

        let cfg = lingxia_runner_config::from_env();
        let mut options = CloudOptions::default();
        if let Some(server) = cfg.lingxia_server {
            options = options.lingxia_server(server);
        }
        if let Some(id) = cfg.lingxia_id {
            options = options.lingxia_id(id);
        }
        if let Err(err) = lingxia_cloud_client::init(options) {
            eprintln!("[cloud] provider init failed: {err}");
        }
    }

    fn start_services(&self) {
        // The Runner is a dev/test harness: grant lx.automation() to every
        // lxapp it launches so test scripts need not declare the privilege.
        lingxia::set_automation_auto_grant(true);
        lingxia::dev::register_device_controller(Box::new(MacRunnerDeviceController));
        lingxia_control_runtime::start_dev_session_bridge_from_env();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lingxia_register_host_addon() {
    lingxia::register_host_addon(Box::new(RunnerDevtoolAddon));
}
