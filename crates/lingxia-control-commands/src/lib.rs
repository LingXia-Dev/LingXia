//! The command surface a LingXia product exposes, independent of who mounts it.
//!
//! `lxdev` is a development tool and always will be — it dials a `lingxia dev`
//! session over the network to drive a phone. But the commands it offers are
//! not its own: they belong to the platform, and a shipped product that wants
//! a command line needs the same ones. Keeping one definition
//! here is what stops the two from drifting the moment a flag is added to one.
//!
//! Not every command needs a transport. `desktop` automates the local OS
//! through `lingxia-device-io` and never talks to a running app, so it
//! works in any binary that links it. The namespaces that do need a live app
//! reach it over the dev websocket in `lxdev` and over the product's local
//! control socket in a shipped binary.

pub mod app;
pub mod browser;
mod console;
#[cfg(feature = "desktop")]
pub mod desktop;
pub mod entry;
mod extra;
pub mod guard;
pub mod output;
pub mod transport;

/// Parse and run a command line on a thread with room for it.
///
/// The clap tree here is deep enough that an unoptimized build overflows
/// Windows' 1 MiB main-thread stack while building it; 8 MiB is the main
/// stack Linux and macOS give a process.
pub fn with_cli_stack<T: Send + 'static>(run: impl FnOnce() -> T + Send + 'static) -> T {
    const CLI_STACK_BYTES: usize = 8 * 1024 * 1024;
    let worker = std::thread::Builder::new()
        .name("cli".to_string())
        .stack_size(CLI_STACK_BYTES)
        .spawn(run)
        .expect("spawn the command-line thread");
    match worker.join() {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

#[doc(hidden)]
pub use extra::{ExtraProductCommand, register_extra_product_command};

#[doc(hidden)]
pub fn is_builtin_product_command(name: &str) -> bool {
    entry::is_builtin_command_name(name)
}
