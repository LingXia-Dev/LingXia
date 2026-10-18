//! Authentication command facade: unified `auth login/logout/status/forget`.
//!
//! Provider-specific flows live in submodules.

use anyhow::{Result, bail};
use colored::Colorize;
use serde::Serialize;

use crate::resolver::{self, AppleChannel};

mod apple;
mod harmony;
mod stores;

pub use apple::{AppleLoginOptions, apple_login, apple_logout, apple_status, inline_login};
pub use harmony::{
    HarmonyLoginOptions, harmony_inline_login, harmony_login, harmony_logout, harmony_status,
};
pub use stores::{StoreLoginOptions, store_inline_login, store_login, store_logout};

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
    /// `environment`, `lingxiaYaml`, `thisCheckout`, or `onlyOne`.
    chosen_by: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    candidates: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppleTeamJson {
    team_id: String,
    mechanisms: String,
    capabilities: crate::wallet::AppleCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    account_type: Option<String>,
}

#[derive(Serialize)]
struct StoreIdentityJson {
    provider: String,
    identity: String,
}

#[derive(Serialize)]
struct LingxiaCredentialJson {
    server: String,
    env: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusJson {
    schema_version: u32,
    project_root: Option<String>,
    channels: Vec<ChannelStatusJson>,
    apple_teams: Vec<AppleTeamJson>,
    harmony_identities: Vec<String>,
    store_identities: Vec<StoreIdentityJson>,
    lingxia_credentials: Vec<LingxiaCredentialJson>,
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
        if project.config.harmony.is_some() {
            channels.push(resolver::diagnose_harmony_channel(project)?);
        }
    }
    let ready = channels.iter().all(|c| c.ready);

    if json {
        let wallet = crate::wallet::Wallet::open()?;
        let mut store_identities = Vec::new();
        for provider in stores::STORE_PROVIDERS {
            for identity in wallet.store_identities(provider)? {
                store_identities.push(StoreIdentityJson {
                    provider: provider.to_string(),
                    identity,
                });
            }
        }
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
                    chosen_by: c.chosen_by.map(|by| match by {
                        resolver::ChosenBy::Environment => "environment",
                        resolver::ChosenBy::ProjectConfig => "lingxiaYaml",
                        resolver::ChosenBy::ThisCheckout => "thisCheckout",
                        resolver::ChosenBy::OnlyOne => "onlyOne",
                    }),
                    candidates: c.candidates.clone(),
                })
                .collect(),
            apple_teams: wallet
                .apple_teams()?
                .into_iter()
                .map(|t| AppleTeamJson {
                    mechanisms: t.mechanisms(),
                    capabilities: t.capabilities(),
                    name: t.info.name.clone(),
                    account_type: t.info.account_type.clone(),
                    team_id: t.team_id,
                })
                .collect(),
            harmony_identities: wallet.harmony_identities()?,
            store_identities,
            lingxia_credentials: wallet
                .publish_entries()?
                .into_iter()
                .map(|(server, env)| LingxiaCredentialJson { server, env })
                .collect(),
        };
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(ready);
    }

    let project_name = project
        .as_ref()
        .and_then(|p| p.root.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    for diagnosis in &channels {
        print_channel(&project_name, diagnosis);
        println!();
    }

    apple_status()?;
    println!();
    harmony_status()?;
    println!();
    stores::stores_status()?;
    let lingxia_credentials = crate::wallet::Wallet::open()?.publish_entries()?;
    if !lingxia_credentials.is_empty() {
        println!("{}", "LingXia".cyan().bold());
        for (server, env) in lingxia_credentials {
            println!("  {server}  ({env})");
        }
    }
    Ok(ready)
}

/// One project channel, in words: what signs it and why, or what to do.
fn print_channel(project_name: &str, d: &resolver::ChannelDiagnosis) {
    use resolver::ChosenBy;
    use resolver::codes;
    let apple = d.channel != "harmony";
    let (platform, build) = match d.channel {
        "ios" => ("iOS", "an iOS build"),
        "macos" => ("macOS", "a macOS build"),
        _ => ("Harmony", "a Harmony build"),
    };
    let thing = if apple {
        "Apple team"
    } else {
        "Harmony AGC identity"
    };
    println!("{}", format!("{project_name} · {platform}").cyan().bold());

    if d.ready {
        let line = d
            .detail
            .as_deref()
            .or(d.credential.as_deref())
            .unwrap_or("");
        println!("  {} {line}", "✓".green());
        match d.chosen_by {
            Some(ChosenBy::ProjectConfig) => {
                println!("    set by {}.teamId in lingxia.yaml", d.channel)
            }
            Some(ChosenBy::ThisCheckout) => {
                if apple {
                    println!("    chosen earlier for this checkout (not set in lingxia.yaml)");
                } else {
                    println!("    chosen earlier for this checkout");
                }
                println!("    change: lingxia auth forget --platform {}", d.channel);
            }
            Some(ChosenBy::OnlyOne) => println!("    the only {thing} on this Mac"),
            Some(ChosenBy::Environment) | None => {}
        }
        return;
    }

    let fix = d.fix.as_deref().unwrap_or("");
    match d.error_code {
        Some(codes::CREDENTIAL_SELECTION_REQUIRED)
            if d.binding.is_some() && d.candidates.len() == 1 =>
        {
            let previous = d.binding.as_deref().unwrap_or("");
            println!(
                "  {} This checkout used {previous}, which this Mac has no login for any more.",
                "✗".red()
            );
            println!("    Now available: {}", d.candidates[0]);
            println!("    Use it: run {build} in a terminal and confirm,");
            println!(
                "            or lingxia auth forget --platform {}",
                d.channel
            );
        }
        Some(codes::CREDENTIAL_SELECTION_REQUIRED) if !d.candidates.is_empty() => {
            println!(
                "  {} No {thing} chosen yet. {} can sign it:",
                "✗".red(),
                d.candidates.len()
            );
            for candidate in &d.candidates {
                println!("      {candidate}");
            }
            println!("    Choose one:");
            if apple {
                println!(
                    "      lingxia.yaml  {}: {{ teamId: <team> }}   whole team and CI",
                    d.channel
                );
                println!("      or run {build} in a terminal and pick   this checkout only");
            } else {
                println!("      run {build} in a terminal and pick   this checkout only");
                println!("      or set LINGXIA_AGC_CLIENT_ID and LINGXIA_AGC_CLIENT_SECRET   CI");
            }
        }
        Some(codes::CREDENTIALS_MISSING) if d.constraint.is_some() => {
            println!(
                "  {} lingxia.yaml sets {}.teamId {}, but this Mac has no login for that team.",
                "✗".red(),
                d.channel,
                d.constraint.as_deref().unwrap_or("")
            );
            println!("    Log in: {fix}");
        }
        Some(codes::CREDENTIALS_MISSING) => {
            println!("  {} No {thing} on this Mac.", "✗".red());
            println!("    Log in: {fix}");
        }
        Some(codes::CREDENTIAL_CAPABILITY_MISSING) => {
            let team = d.detail.as_deref().or(d.binding.as_deref()).unwrap_or("");
            println!("  {} {team} has no login that can sign.", "✗".red());
            println!("    Log in: {fix}");
        }
        _ => println!("  {} {fix}", "✗".red()),
    }
}
