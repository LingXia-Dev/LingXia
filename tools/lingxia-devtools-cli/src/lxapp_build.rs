use crate::client;
use anyhow::Result;
use clap::Args;
use lingxia_control_protocol::methods;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

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

pub fn execute(project_root: &Path, ws_url: &str, options: &ReloadOptions) -> Result<()> {
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
        if let Some(appid) = reloaded.as_deref() {
            report_page_registry_drift(project_root, ws_url, appid);
        }
    }
    Ok(())
}

/// The session builds its page registry when it starts, so a page added to
/// `lxapp.json` afterwards is invisible to a reload. Saying nothing leaves the
/// next `nav` answering `unknown page name` for a page the file clearly
/// declares — the build succeeded, so the edit looks applied.
fn report_page_registry_drift(project_root: &Path, ws_url: &str, appid: &str) {
    let Some(declared) = manifest_page_names(project_root, appid) else {
        return;
    };
    let Some(running) = session_page_names(ws_url, appid) else {
        return;
    };
    let added: Vec<&str> = declared.difference(&running).map(String::as_str).collect();
    let removed: Vec<&str> = running.difference(&declared).map(String::as_str).collect();
    if added.is_empty() && removed.is_empty() {
        return;
    }
    let mut parts = Vec::new();
    if !added.is_empty() {
        parts.push(format!("added {}", added.join(", ")));
    }
    if !removed.is_empty() {
        parts.push(format!("removed {}", removed.join(", ")));
    }
    eprintln!(
        "warning: lxapp.json declares a different page list than this session is running ({}). \
Restart `lingxia dev` to apply it; a reload only rebuilds the bundle.",
        parts.join("; ")
    );
}

/// Page names from the project's own manifest, and only when it is the lxapp
/// that was reloaded — a host project reloads bundles declared elsewhere.
fn manifest_page_names(project_root: &Path, appid: &str) -> Option<BTreeSet<String>> {
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(project_root.join("lxapp.json")).ok()?)
            .ok()?;
    if manifest.get("appId").and_then(Value::as_str) != Some(appid) {
        return None;
    }
    Some(
        manifest
            .get("pages")?
            .as_array()?
            .iter()
            .filter_map(|page| page.get("name")?.as_str().map(ToOwned::to_owned))
            .collect(),
    )
}

fn session_page_names(ws_url: &str, appid: &str) -> Option<BTreeSet<String>> {
    let pages = client::execute_command(
        ws_url,
        methods::lxapp::PAGES,
        Some(json!({ "appid": appid })),
    )
    .ok()??;
    Some(
        pages
            .get("pages")?
            .as_array()?
            .iter()
            .filter_map(|page| page.get("name")?.as_str().map(ToOwned::to_owned))
            .collect(),
    )
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
