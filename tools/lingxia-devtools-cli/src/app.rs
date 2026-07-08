use anyhow::{Result, bail};
use clap::Args;

/// Captures whatever followed `lxdev app` so the removed namespace can emit a
/// targeted migration hint without requiring the old (also removed) flags to
/// still parse.
#[derive(Args, Clone)]
#[command(disable_help_flag = true)]
pub struct AppOptions {
    #[arg(num_args = 0.., trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// The `app` namespace was removed in the command-contract refactor: its
/// session-scoped commands moved under `lxdev lxapp`. Map the old spellings to
/// their new homes with an actionable error instead of silently aliasing.
pub fn migrate(options: AppOptions) -> Result<()> {
    let sub = options.args.first().map(String::as_str).unwrap_or("");
    let hint: String = match sub {
        "screenshot" => "`lxdev app screenshot` moved to `lxdev lxapp screenshot`.\n\
             For a specific window, pass `--window <id>` from `lxdev lxapp windows`."
            .to_string(),
        "windows" => "`lxdev app windows` moved to `lxdev lxapp windows`.".to_string(),
        "mouse" => "`lxdev app mouse` moved to `lxdev lxapp page pointer` \
             (coordinates are now `--at X,Y`)."
            .to_string(),
        "key" => "`lxdev app key` moved to `lxdev lxapp page key` \
             (use `--text` / `--key` and `--modifier ctrl|shift|alt|meta`)."
            .to_string(),
        "" => "The `lxdev app` namespace was removed. Session commands live under \
             `lxdev lxapp`: screenshot, windows, `page pointer`, `page key`."
            .to_string(),
        other => {
            format!("`lxdev app {other}` was removed. See `lxdev lxapp` for session commands.")
        }
    };
    bail!("{hint}");
}
