use crate::client;
use anyhow::Result;
use clap::Args;
use lingxia_control_protocol::methods;
use serde_json::{Value, json};

/// Rebuild the lxapp front-end bundle for the selected session's project, then
/// reload the running lxapp so the new bundle is live — the session is already
/// attached, so one command completes the edit → build → reload loop. The dev
/// orchestrator owns the build; build output streams to the `lingxia dev`
/// session log, and the client receives only success/failure. When no lxapp
/// runtime is attached the build still succeeds and the reload is skipped.
#[derive(Args, Clone)]
pub struct ReloadOptions {
    /// Release (minified) build
    #[arg(long)]
    pub release: bool,
    /// Framework to build when the project ships more than one (react, vue,
    /// html) — multi-framework demo projects only; hidden from help
    #[arg(long, hide = true)]
    pub framework: Option<String>,
    /// Build only; skip reloading the running lxapp
    #[arg(long)]
    pub build_only: bool,
    /// LxApp to reload after the build; defaults to current
    #[arg(long, default_value = "current")]
    pub app: String,
    /// Print JSON output
    #[arg(long)]
    pub json: bool,
}

// A cold lxapp build (npm install + vite) can exceed the default 120s command
// window, so request a generous timeout from the client + server.
const BUILD_TIMEOUT_MS: u64 = 600_000;

pub fn run(
    ws_url: &str,
    release: bool,
    framework: Option<&str>,
    appid: Option<&str>,
) -> Result<()> {
    client::execute_command(
        ws_url,
        methods::lxapp::BUILD,
        Some(json!({
            "release": release,
            "framework": framework,
            "appid": appid,
            "timeout_ms": BUILD_TIMEOUT_MS,
        })),
    )?;
    Ok(())
}

pub fn execute(ws_url: &str, options: &ReloadOptions) -> Result<()> {
    let appid = resolve_target(ws_url, &options.app)?;
    run(
        ws_url,
        options.release,
        options.framework.as_deref(),
        appid.as_deref(),
    )?;

    let reloaded = if options.build_only {
        None
    } else {
        reload_target(ws_url, appid.as_deref())?
    };

    if options.json {
        println!(
            "{}",
            json!({
                "ok": true,
                "release": options.release,
                "reloaded": reloaded,
            })
        );
    } else {
        let suffix = if options.release { " (release)" } else { "" };
        match &reloaded {
            Some(appid) => {
                println!("✓ lxapp bundle rebuilt{suffix}, reloaded {appid}")
            }
            None if options.build_only => println!("✓ lxapp bundle rebuilt{suffix}"),
            None => println!("✓ lxapp bundle rebuilt{suffix} (no running lxapp to reload)"),
        }
    }
    Ok(())
}

/// Resolve `app` before building so the orchestrator can rebuild the matching
/// resource bundle. `None` means no lxapp runtime is attached — a bare build
/// environment, not an error.
fn resolve_target(ws_url: &str, app: &str) -> Result<Option<String>> {
    if app == "current" {
        let current = client::execute_command(ws_url, methods::lxapp::CURRENT, None)?;
        Ok(current
            .as_ref()
            .and_then(|value| value.get("appid"))
            .and_then(Value::as_str)
            .filter(|appid| !appid.is_empty())
            .map(ToOwned::to_owned))
    } else {
        Ok(Some(app.to_string()))
    }
}

fn reload_target(ws_url: &str, appid: Option<&str>) -> Result<Option<String>> {
    let Some(appid) = appid else {
        return Ok(None);
    };
    client::execute_command(
        ws_url,
        methods::lxapp::RESTART,
        Some(json!({ "appid": appid })),
    )?;
    Ok(Some(appid.to_string()))
}
