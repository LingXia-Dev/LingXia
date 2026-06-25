use crate::client;
use crate::project::SessionInfo;
use anyhow::Result;
use clap::Args;
use serde_json::json;

/// `lxdev build` — build the lxapp front-end bundle for the selected session's
/// project. The dev orchestrator owns the build, so this works even with no app
/// attached; build output streams to the `lingxia dev` terminal (see `lxdev
/// logs`), and the client receives only success/failure.
#[derive(Args, Clone)]
pub struct BuildOptions {
    /// Release (minified) build
    #[arg(long)]
    release: bool,
    /// Framework to build when the project ships more than one (react, vue, html)
    #[arg(long)]
    framework: Option<String>,
    /// Print JSON output
    #[arg(long)]
    json: bool,
}

// A cold lxapp build (npm install + vite) can exceed the default 120s command
// window, so request a generous timeout from the client + server.
const BUILD_TIMEOUT_MS: u64 = 600_000;

pub fn execute(info: &SessionInfo, options: BuildOptions) -> Result<()> {
    client::execute_command(
        &info.ws_url,
        lingxia_devtool_protocol::handlers::lxapp::BUILD,
        Some(json!({
            "release": options.release,
            "framework": options.framework,
            "timeout_ms": BUILD_TIMEOUT_MS,
        })),
    )?;

    if options.json {
        println!("{}", json!({ "ok": true, "release": options.release }));
    } else {
        let suffix = if options.release { " (release)" } else { "" };
        println!("✓ lxapp build complete{suffix}");
    }
    Ok(())
}
