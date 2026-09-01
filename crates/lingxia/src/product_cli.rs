//! Host-owned commands on the product executable.

pub use lingxia_control_commands::transport::Transport;
use std::collections::HashSet;
use std::ffi::OsString;

/// A host-owned top-level product command.
pub type CommandHandler = fn(&dyn Transport, &[OsString]) -> i32;

/// Command registry supplied to [`crate::HostAddon::install_product_cli`].
///
/// Hosts declare their commands here; LingXia owns when registration runs and
/// strips its launcher argument before invoking a handler.
///
/// ```no_run
/// use std::ffi::OsString;
/// use lingxia::product_cli::{ProductCli, Transport};
///
/// struct AppHostAddon;
///
/// impl lingxia::HostAddon for AppHostAddon {
///     fn install_product_cli(&self, cli: &mut ProductCli) {
///         cli.command("workspace", "Manage workspaces", workspace_cli);
///     }
/// }
///
/// fn workspace_cli(_transport: &dyn Transport, args: &[OsString]) -> i32 {
///     println!("{args:?}");
///     0
/// }
/// ```
pub struct ProductCli {
    names: HashSet<&'static str>,
}

impl ProductCli {
    pub(crate) fn new() -> Self {
        Self {
            names: HashSet::new(),
        }
    }

    /// Add one host-owned top-level command.
    ///
    /// Names use lowercase ASCII words (digits allowed) separated by `-`.
    /// Invalid, duplicate, or framework-owned names fail immediately during
    /// host registration instead of shadowing another command at runtime.
    pub fn command(
        &mut self,
        name: &'static str,
        about: &'static str,
        execute: CommandHandler,
    ) -> &mut Self {
        assert!(
            valid_command_name(name),
            "invalid product CLI command name `{name}`; use lowercase ASCII words separated by `-`"
        );
        assert!(
            !lingxia_control_commands::is_builtin_product_command(name),
            "product CLI command `{name}` is reserved by LingXia"
        );
        assert!(
            self.names.insert(name),
            "product CLI command `{name}` was registered more than once"
        );
        lingxia_control_commands::register_extra_product_command(
            lingxia_control_commands::ExtraProductCommand {
                name,
                about,
                execute,
            },
        );
        self
    }
}

fn valid_command_name(name: &str) -> bool {
    !name.is_empty()
        && name.split('-').all(|word| {
            let mut bytes = word.bytes();
            bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
                && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

/// Answer as the command line if this invocation is one, and return the exit
/// code. `None` means carry on and be the app.
///
/// Call it as the first thing in `main`: initialization opens the app's
/// databases, and a command must not collide with an instance already running.
/// The state directory is resolved from packaged assets rather than the runtime
/// precisely so this can run before any of it.
#[cfg(target_os = "windows")]
pub fn run_if_invoked() -> Option<i32> {
    crate::host_addon::run_install_product_cli(&mut ProductCli::new());
    use lingxia_platform::traits::app_runtime::AppRuntime;
    let platform = lingxia_platform::Platform::from_env().ok()?;
    let state_dir = lingxia_app_context::app_state_dir(&platform.app_data_dir());
    lingxia_control_commands::entry::run_if_invoked(&state_dir)
}

/// The data directory is handed in by the platform layer on Apple, which knows
/// it before anything else runs.
#[cfg(not(target_os = "windows"))]
pub fn run_if_invoked_in(data_dir: &std::path::Path) -> Option<i32> {
    crate::host_addon::run_install_product_cli(&mut ProductCli::new());
    let state_dir = lingxia_app_context::app_state_dir(data_dir);
    lingxia_control_commands::entry::run_if_invoked(&state_dir)
}

#[cfg(test)]
mod tests {
    use super::{ProductCli, Transport, valid_command_name};
    use std::ffi::OsString;

    #[test]
    fn host_command_names_are_shell_friendly() {
        assert!(valid_command_name("cloud"));
        assert!(valid_command_name("cloud-sync"));
        assert!(valid_command_name("s3-sync"));
        for invalid in [
            "",
            "-cloud",
            "cloud-",
            "cloud--sync",
            "Cloud",
            "2cloud",
            "cloud_1",
        ] {
            assert!(!valid_command_name(invalid), "{invalid}");
        }
    }

    fn no_op(_transport: &dyn Transport, _args: &[OsString]) -> i32 {
        0
    }

    #[test]
    #[should_panic(expected = "reserved by LingXia")]
    fn hosts_cannot_shadow_framework_commands() {
        ProductCli::new().command("browser", "shadow browser", no_op);
    }
}
