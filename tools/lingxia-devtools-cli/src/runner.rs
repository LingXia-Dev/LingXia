//! `lxdev runner`: the simulated environment owned by the runner host —
//! device preset, orientation, appearance and display language.
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
    /// Print the simulated environment (device, orientation, appearance, language)
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
        /// Adaptive preset instead of an id (phone, fold, trifold, pad, desktop)
        #[arg(long, value_enum, conflicts_with = "id")]
        preset: Option<DevicePresetArg>,
        /// Folding-device posture (outer, open, dual, tabletop); with --preset or the current device
        #[arg(long, value_enum)]
        posture: Option<PostureArg>,
        /// Force landscape orientation
        #[arg(long, conflicts_with = "portrait")]
        landscape: bool,
        /// Force portrait orientation
        #[arg(long)]
        portrait: bool,
        /// Simulated appearance for the device screen
        #[arg(long, value_enum)]
        appearance: Option<AppearanceArg>,
        /// Display language for this dev session: a BCP-47 tag (`zh-CN`,
        /// `sr-Latn-RS`) or `auto` for the system. Shadows
        /// the product's display language and is not saved.
        #[arg(long, value_name = "TAG")]
        language: Option<String>,
        /// Simulated system text size as a factor of the default (1.3, 2), 0.5 to 3
        #[arg(long, value_name = "FACTOR")]
        text_scale: Option<f64>,
        /// Simulated system contrast
        #[arg(long, value_enum)]
        contrast: Option<ContrastArg>,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
}

#[derive(ValueEnum, Clone, Copy)]
pub enum DevicePresetArg {
    Phone,
    Fold,
    Trifold,
    Pad,
    Desktop,
}

impl DevicePresetArg {
    fn as_str(self) -> &'static str {
        match self {
            DevicePresetArg::Phone => "phone",
            DevicePresetArg::Fold => "fold",
            DevicePresetArg::Trifold => "trifold",
            DevicePresetArg::Pad => "pad",
            DevicePresetArg::Desktop => "desktop",
        }
    }
}

#[derive(ValueEnum, Clone, Copy)]
pub enum PostureArg {
    Open,
    Dual,
    Outer,
    Tabletop,
}

impl PostureArg {
    fn as_str(self) -> &'static str {
        match self {
            PostureArg::Open => "open",
            PostureArg::Dual => "dual",
            PostureArg::Outer => "outer",
            PostureArg::Tabletop => "tabletop",
        }
    }
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
pub enum ContrastArg {
    /// The system's standard contrast
    Standard,
    /// The system's increased contrast
    High,
}

impl ContrastArg {
    fn as_str(self) -> &'static str {
        match self {
            ContrastArg::Standard => "standard",
            ContrastArg::High => "high",
        }
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
                print_state(&data)?;
            }
        }
        RunnerCommand::Set {
            id,
            preset,
            posture,
            landscape,
            portrait,
            appearance,
            language,
            text_scale,
            contrast,
            json,
        } => {
            if id.is_none()
                && preset.is_none()
                && posture.is_none()
                && !landscape
                && !portrait
                && appearance.is_none()
                && language.is_none()
                && text_scale.is_none()
                && contrast.is_none()
            {
                anyhow::bail!(
                    "nothing to change: pass --id, --preset, --posture, --landscape/--portrait, --appearance, --language, --text-scale or --contrast"
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
                    "preset": preset.map(DevicePresetArg::as_str),
                    "posture": posture.map(PostureArg::as_str),
                    "landscape": orientation,
                    "appearance": appearance.map(AppearanceArg::as_str),
                    "language": language,
                    "textScale": text_scale,
                    "contrast": contrast.map(ContrastArg::as_str),
                })),
            )?
            .unwrap_or(Value::Null);
            if json {
                print_json(&data, false)?;
            } else {
                print_state(&data)?;
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
        "{:<3}  {:<20}  {:<8}  {:<11}  {:<5}  ID",
        "CUR", "NAME", "GROUP", "SIZE", "RATIO"
    );
    for preset in array {
        let id = preset.get("id").and_then(Value::as_str).unwrap_or("-");
        let name = preset.get("name").and_then(Value::as_str).unwrap_or("");
        let group = preset.get("group").and_then(Value::as_str).unwrap_or("");
        let width = points(preset, "width");
        let height = points(preset, "height");
        let current = preset
            .get("current")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        println!(
            "{:<3}  {:<20}  {:<8}  {:<11}  {:<5}  {}",
            if current { " * " } else { "" },
            name,
            group,
            format!("{width}x{height}"),
            pixel_ratio(preset),
            id
        );
    }
}

/// A length in points: whole when it is, else to one decimal (an Android phone's 392.7).
fn points(data: &Value, key: &str) -> String {
    let value = data.get(key).and_then(Value::as_f64).unwrap_or(0.0);
    if value.fract().abs() < 1e-6 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

/// `@3x`: the device pixels per point a GPU page renders at.
fn pixel_ratio(data: &Value) -> String {
    let ratio = data
        .get("pixelRatio")
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    format!("@{ratio}x")
}

fn print_state(data: &Value) -> Result<()> {
    println!("{}", describe_state(data)?);
    Ok(())
}

fn describe_state(data: &Value) -> Result<String> {
    if data.is_null() {
        return Ok("No environment reported by the session.".to_string());
    }
    let name = data.get("name").and_then(Value::as_str).unwrap_or("");
    let id = data.get("id").and_then(Value::as_str).unwrap_or("-");
    let width = points(data, "width");
    let height = points(data, "height");
    let landscape = data
        .get("landscape")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let orientation = if landscape { "landscape" } else { "portrait" };
    let appearance = data
        .get("appearance")
        .and_then(Value::as_str)
        .unwrap_or("system");
    let mut line = format!(
        "{name} ({id})  {width}x{height} {}  {orientation}  appearance: {appearance}",
        pixel_ratio(data)
    );
    if let Some(language) = data.get("language").and_then(Value::as_str) {
        line.push_str(&format!("  language: {language}"));
    }
    // The defaults go unsaid.
    let text_scale = data.get("textScale").and_then(Value::as_f64).unwrap_or(1.0);
    if (text_scale - 1.0).abs() > 0.001 {
        line.push_str(&format!("  text scale: {text_scale}"));
    }
    if data.get("contrast").and_then(Value::as_str) == Some("high") {
        line.push_str("  contrast: high");
    }
    Ok(line)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_names_device_orientation_and_appearance() {
        let state = json!({
            "name": "iPhone 15 Pro", "id": "iphone-15-pro", "width": 393, "height": 852,
            "pixelRatio": 3, "landscape": false, "appearance": "dark",
        });
        assert_eq!(
            describe_state(&state).unwrap(),
            "iPhone 15 Pro (iphone-15-pro)  393x852 @3x  portrait  appearance: dark"
        );
        let state = json!({
            "name": "iPad", "id": "ipad", "width": 820, "height": 1180,
            "landscape": true, "appearance": "system", "language": "zh-CN",
        });
        assert_eq!(
            describe_state(&state).unwrap(),
            "iPad (ipad)  820x1180 @1x  landscape  appearance: system  language: zh-CN"
        );
    }
}
