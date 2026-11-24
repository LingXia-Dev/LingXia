//! The app's native library: its host addon and `#[lingxia::native]` routes.
//!
//! Write routes in any module; `lingxia::host!` registers them all and exports the
//! entrypoint each platform SDK calls.

lingxia::host!(AppHostAddon);

struct AppHostAddon;

impl lingxia::HostAddon for AppHostAddon {
    fn start_services(&self) {
        #[cfg(feature = "devtools")]
        lingxia_control_runtime::start_dev_session_bridge_from_env();
        #[cfg(feature = "control")]
        if let Err(error) = lingxia_control_runtime::local_control::install(true) {
            log::warn!("local control unavailable: {error}");
        }
    }
}
