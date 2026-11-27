//! The app's native library: its host addon and `#[lingxia::native]` routes.
//!
//! Write routes in any module; `lingxia::host!` registers them all, starts the dev
//! session under `lingxia dev`, and exports the entrypoint each platform SDK calls.

lingxia::host!(AppHostAddon);

struct AppHostAddon;

impl lingxia::HostAddon for AppHostAddon {
    fn start_services(&self) {
        // Opens the declared agent control; pass your product's saved setting instead of `true`.
        #[cfg(feature = "control")]
        if let Err(error) = lingxia_control_runtime::local_control::install(true) {
            log::warn!("local control unavailable: {error}");
        }
    }
}

#[derive(serde::Deserialize)]
struct Greeting {
    name: String,
    count: u32,
}

/// The home page calls this as `native.hello.greet({ name, count })`.
#[lingxia::native("hello.greet")]
fn greet(input: Greeting) -> String {
    format!("Hello, {}! Greeting number {}.", input.name, input.count)
}
