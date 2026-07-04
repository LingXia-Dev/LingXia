use crate::client;
use anyhow::Result;
use clap::Args;
use serde_json::json;

/// Rebuild the lxapp front-end bundle for the selected session's project. The
/// dev orchestrator owns the build, so this works even when no lxapp runtime is
/// attached; build output streams to the `lingxia dev` session log, and the
/// client receives only success/failure.
#[derive(Args, Clone)]
pub struct RebuildOptions {
    /// Release (minified) build
    #[arg(long)]
    pub release: bool,
    /// Framework to build when the project ships more than one (react, vue, html)
    #[arg(long)]
    pub framework: Option<String>,
    /// Print JSON output
    #[arg(long)]
    pub json: bool,
}

// A cold lxapp build (npm install + vite) can exceed the default 120s command
// window, so request a generous timeout from the client + server.
const BUILD_TIMEOUT_MS: u64 = 600_000;

pub fn run(ws_url: &str, release: bool, framework: Option<&str>) -> Result<()> {
    client::execute_command(
        ws_url,
        lingxia_devtool_protocol::handlers::lxapp::BUILD,
        Some(json!({
            "release": release,
            "framework": framework,
            "timeout_ms": BUILD_TIMEOUT_MS,
        })),
    )?;
    Ok(())
}

pub fn execute(ws_url: &str, options: &RebuildOptions) -> Result<()> {
    run(ws_url, options.release, options.framework.as_deref())?;

    if options.json {
        println!("{}", json!({ "ok": true, "release": options.release }));
    } else {
        let suffix = if options.release { " (release)" } else { "" };
        println!("✓ lxapp rebuild complete{suffix}");
    }
    Ok(())
}
