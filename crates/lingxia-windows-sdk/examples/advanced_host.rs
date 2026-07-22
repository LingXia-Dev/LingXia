//! Advanced (bring-your-own-host) mode for the Windows SDK.
//!
//! The default, batteries-included path is [`lingxia_windows_sdk::quick_start`],
//! which opens the SDK's own windows and pumps the message loop for you. This
//! example shows the other mode: the host owns its window and message loop, and
//! the SDK supplies reusable view components. The host implements the
//! [`WindowsHostBackend`] hooks it wants to orchestrate against its own windows
//! and registers the backend with [`set_windows_host_backend`]; LingXia then
//! routes host-facing UI through that backend instead of a window the SDK
//! created.
//!
//! Build (the contract surface is Windows-only):
//!
//! ```text
//! cargo build --example advanced_host -p lingxia-windows-sdk \
//!     --no-default-features --features host-api --target x86_64-pc-windows-msvc
//! ```
//!
//! To also boot the lxapp runtime in this mode, enable the `runtime` feature and
//! drive it yourself instead of calling `quick_start`:
//!
//! ```ignore
//! lingxia_windows_contract::set_windows_host_backend(Arc::new(MyHostBackend::new()));
//! let _home_app_id = lingxia_windows_sdk::init_runtime(
//!     lingxia_windows_sdk::WindowsApp::from_env(),
//! )?;
//! // ... then run your own Win32 message loop instead of run_message_loop().
//! ```

#[cfg(windows)]
fn main() {
    advanced::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("the advanced_host example only runs on Windows");
}

#[cfg(windows)]
mod advanced {
    use std::sync::Arc;

    use lingxia_windows_contract::{WindowsHostBackend, set_windows_host_backend};

    pub fn run() {
        // Register the host-owned backend instead of the SDK default. Override
        // only the hooks your host orchestrates: window lookup/thread posting
        // for SDK components, panel/window presentation, chrome, and so on.
        set_windows_host_backend(Arc::new(MyHostBackend));
        // Optional SDK-managed component integrations (input/video overlays,
        // media preview, pull-to-refresh) without installing the default backend.
        #[cfg(feature = "components")]
        lingxia_windows_sdk::install_windows_components();
        boot_and_run();
    }

    // Booting the lxapp runtime needs the `runtime` feature. `init_runtime` is
    // host-agnostic: it presents no window, so the host owns the window and loop.
    #[cfg(feature = "runtime")]
    fn boot_and_run() {
        match lingxia_windows_sdk::init_runtime(lingxia_windows_sdk::WindowsApp::from_env()) {
            Ok(home_app_id) => {
                println!(
                    "runtime booted (home lxapp: {}); open your own window + pump your own loop",
                    home_app_id.as_deref().unwrap_or("none")
                );
                // Create your Win32 window for `home_app_id`, then drive messages,
                // e.g. `let _code = lingxia_windows_sdk::run_message_loop();`
            }
            Err(error) => eprintln!("init_runtime failed: {error}"),
        }
    }

    #[cfg(not(feature = "runtime"))]
    fn boot_and_run() {
        println!(
            "registered a custom WindowsHostBackend; enable `runtime` to boot the lxapp runtime"
        );
    }

    /// A skeleton backend. The contract provides conservative defaults for
    /// unsupported hooks; a real host overrides the hooks it wants to own.
    struct MyHostBackend;

    impl WindowsHostBackend for MyHostBackend {}
}
