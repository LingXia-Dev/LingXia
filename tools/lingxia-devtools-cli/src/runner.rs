//! `lxdev runner`: the simulated environment owned by the runner host —
//! device preset, orientation, appearance, and the simulated host capsule.
//! Runner sessions only.

use crate::client;
use crate::project::SessionInfo;
use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};
use lingxia_control_protocol::methods;
use serde_json::{Value, json};

#[derive(Args, Clone)]
pub struct RunnerOptions {
    #[command(subcommand)]
    command: RunnerCommand,
}

#[derive(Subcommand, Clone)]
pub enum RunnerCommand {
    /// List the device presets the runner can simulate
    Presets {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Print the simulated environment (device, orientation, appearance)
    Get {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Update the simulated environment; only the given properties change
    Set {
        /// Device preset id (see `lxdev runner presets`)
        #[arg(long)]
        id: Option<String>,
        /// Force landscape orientation
        #[arg(long, conflicts_with = "portrait")]
        landscape: bool,
        /// Force portrait orientation
        #[arg(long)]
        portrait: bool,
        /// Simulated appearance for the device screen
        #[arg(long, value_enum)]
        appearance: Option<AppearanceArg>,
        /// Show or hide the simulated host capsule (phone presets draw it by
        /// default, as a real host does for a non-home lxapp)
        #[arg(long, value_enum)]
        capsule: Option<CapsuleArg>,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
}

#[derive(ValueEnum, Clone, Copy)]
pub enum AppearanceArg {
    /// Follow the host OS appearance
    System,
    /// Pin the simulated screen to light
    Light,
    /// Pin the simulated screen to dark
    Dark,
}

impl AppearanceArg {
    fn as_str(self) -> &'static str {
        match self {
            AppearanceArg::System => "system",
            AppearanceArg::Light => "light",
            AppearanceArg::Dark => "dark",
        }
    }
}

#[derive(ValueEnum, Clone, Copy)]
pub enum CapsuleArg {
    /// Draw the host capsule over phone presets (the non-home default)
    On,
    /// Hide the capsule, simulating a home-lxapp placement
    Off,
}

impl CapsuleArg {
    fn as_bool(self) -> bool {
        matches!(self, CapsuleArg::On)
    }
}

pub fn execute(info: &SessionInfo, options: RunnerOptions) -> Result<()> {
    let ws_url = info.ws_url.as_str();
    match options.command {
        RunnerCommand::Presets { json } => {
            let data = client::execute_command(ws_url, methods::runner::PRESETS, None)?
                .unwrap_or_else(|| json!([]));
            if json {
                print_json(&data, false)?;
            } else {
                print_presets(&data);
            }
        }
        RunnerCommand::Get { json } => {
            let data =
                client::execute_command(ws_url, methods::runner::GET, None)?.unwrap_or(Value::Null);
            if json {
                print_json(&data, false)?;
            } else {
                print_state(&data);
            }
        }
        RunnerCommand::Set {
            id,
            landscape,
            portrait,
            appearance,
            capsule,
            json,
        } => {
            if id.is_none() && !landscape && !portrait && appearance.is_none() && capsule.is_none()
            {
                anyhow::bail!(
                    "nothing to change: pass --id, --landscape/--portrait, --appearance, or --capsule"
                );
            }
            // Leave orientation to the runner's normal selector behavior
            // unless a flag pins it.
            let orientation = if landscape {
                Some(true)
            } else if portrait {
                Some(false)
            } else {
                None
            };
            let data = client::execute_command(
                ws_url,
                methods::runner::SET,
                Some(json!({
                    "id": id,
                    "landscape": orientation,
                    "appearance": appearance.map(AppearanceArg::as_str),
                    "capsule": capsule.map(CapsuleArg::as_bool),
                })),
            )?
            .unwrap_or(Value::Null);
            if json {
                print_json(&data, false)?;
            } else {
                print_state(&data);
            }
        }
    }
    Ok(())
}

fn print_presets(data: &Value) {
    let Some(array) = data.as_array() else {
        let _ = print_json(data, false);
        return;
    };
    if array.is_empty() {
        println!("No presets reported by the session.");
        return;
    }
    println!(
        "{:<3}  {:<20}  {:<8}  {:<11}  ID",
        "CUR", "NAME", "GROUP", "SIZE"
    );
    for preset in array {
        let id = preset.get("id").and_then(Value::as_str).unwrap_or("-");
        let name = preset.get("name").and_then(Value::as_str).unwrap_or("");
        let group = preset.get("group").and_then(Value::as_str).unwrap_or("");
        let width = preset.get("width").and_then(Value::as_u64).unwrap_or(0);
        let height = preset.get("height").and_then(Value::as_u64).unwrap_or(0);
        let current = preset
            .get("current")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        println!(
            "{:<3}  {:<20}  {:<8}  {:<11}  {}",
            if current { " * " } else { "" },
            name,
            group,
            format!("{width}x{height}"),
            id
        );
    }
}

fn print_state(data: &Value) {
    if data.is_null() {
        println!("No environment reported by the session.");
        return;
    }
    let name = data.get("name").and_then(Value::as_str).unwrap_or("");
    let id = data.get("id").and_then(Value::as_str).unwrap_or("-");
    let width = data.get("width").and_then(Value::as_u64).unwrap_or(0);
    let height = data.get("height").and_then(Value::as_u64).unwrap_or(0);
    let landscape = data
        .get("landscape")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let orientation = if landscape { "landscape" } else { "portrait" };
    let appearance = data
        .get("appearance")
        .and_then(Value::as_str)
        .unwrap_or("system");
    // Old runners predate the field; absent means the pill is in its default
    // shown state, so print nothing rather than a guess.
    let capsule = match data.get("capsule").and_then(Value::as_bool) {
        Some(true) => "  capsule: on",
        Some(false) => "  capsule: off",
        None => "",
    };
    println!("{name} ({id})  {width}x{height}  {orientation}  appearance: {appearance}{capsule}");
}

fn print_json(value: &Value, pretty: bool) -> Result<()> {
    let encoded = if pretty {
        serde_json::to_string_pretty(value)?
    } else {
        serde_json::to_string(value)?
    };
    println!("{encoded}");
    Ok(())
}
