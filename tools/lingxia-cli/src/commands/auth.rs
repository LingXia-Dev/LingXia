//! Authentication command facade: unified `auth login/logout/status/forget`.
//!
//! Provider-specific flows live in submodules.

use anyhow::{Result, bail};
use colored::Colorize;
use serde::Serialize;

use crate::resolver::{self, AppleChannel};

mod apple;
mod harmony;

pub use apple::{AppleLoginOptions, apple_login, apple_logout, apple_status, inline_login};
pub use harmony::{HarmonyLoginOptions, harmony_login, harmony_logout, harmony_status};

/// `lingxia auth forget --platform <channel>`: drop this checkout's automatic
/// credential selection; the next command re-resolves. Never touches secrets.
pub fn auth_forget(platform: &str) -> Result<()> {
    let Some(project) = resolver::detect_project()? else {
        bail!("not inside a LingXia project (no lingxia.yaml found upwards from here)");
    };
    let bindings = crate::binding::BindingStore::open()?;
    if bindings.forget(&project.root, platform)? {
        println!(
            "{} Dropped the {platform} credential selection for this checkout; the next command re-resolves.",
            "✓".green()
        );
    } else {
        println!(
            "{} No stored selection for {platform} in this checkout.",
            "ℹ".blue()
        );
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelStatusJson {
    channel: String,
    constraint: Option<String>,
    binding: Option<String>,
    credential: Option<String>,
    status: &'static str,
    error_code: Option<&'static str>,
    fix: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppleTeamJson {
    team_id: String,
    mechanisms: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusJson {
    schema_version: u32,
    project_root: Option<String>,
    channels: Vec<ChannelStatusJson>,
    apple_teams: Vec<AppleTeamJson>,
}

/// `lingxia auth status`: per-channel project view (inside a project) plus the
/// wallet view. Returns whether every diagnosed channel is ready.
pub fn auth_status(json: bool) -> Result<bool> {
    let project = resolver::detect_project()?;

    let mut channels = Vec::new();
    if let Some(project) = &project {
        for channel in [AppleChannel::Ios, AppleChannel::Macos] {
            let configured = match channel {
                AppleChannel::Ios => project.config.ios.is_some(),
                AppleChannel::Macos => project.config.macos.is_some(),
            };
            if configured {
                channels.push(resolver::diagnose_apple_channel(project, channel)?);
            }
        }
    }
    let ready = channels.iter().all(|c| c.ready);

    if json {
        let wallet = crate::wallet::Wallet::open()?;
        let payload = StatusJson {
            schema_version: 1,
            project_root: project
                .as_ref()
                .map(|p| p.root.to_string_lossy().into_owned()),
            channels: channels
                .iter()
                .map(|c| ChannelStatusJson {
                    channel: c.channel.to_string(),
                    constraint: c.constraint.clone(),
                    binding: c.binding.clone(),
                    credential: c.credential.clone(),
                    status: if c.ready {
                        "readyLocal"
                    } else {
                        "actionRequired"
                    },
                    error_code: c.error_code,
                    fix: c.fix.clone(),
                })
                .collect(),
            apple_teams: wallet
                .apple_teams()?
                .into_iter()
                .map(|t| AppleTeamJson {
                    mechanisms: t.mechanisms(),
                    team_id: t.team_id,
                })
                .collect(),
        };
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(ready);
    }

    for diagnosis in &channels {
        println!("{}", format!("Apple / {}", diagnosis.channel).cyan().bold());
        match &diagnosis.constraint {
            Some(team) => println!("  constraint: {team} (lingxia.yaml)"),
            None => println!("  constraint: none"),
        }
        if let Some(binding) = &diagnosis.binding {
            println!("  binding:    {binding} (this checkout)");
        }
        if let Some(credential) = &diagnosis.credential {
            println!("  credential: {credential}");
        }
        if diagnosis.ready {
            println!("  result:     {}", "readyLocal".green());
        } else {
            println!(
                "  result:     {} ({})",
                "actionRequired".yellow(),
                diagnosis.error_code.unwrap_or("UNKNOWN")
            );
            if let Some(fix) = &diagnosis.fix {
                println!("  next:       {fix}");
            }
        }
        println!();
    }

    apple_status()?;
    println!();
    harmony_status()?;
    Ok(ready)
}
