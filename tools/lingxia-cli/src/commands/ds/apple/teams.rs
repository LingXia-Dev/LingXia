//! Teams subcommand for Apple Developer Services.

use anyhow::{Context, Result, anyhow};
use colored::Colorize;

use crate::platform::apple::anisette::OmnisetteProvider;
use crate::platform::apple::auth::CredentialStorage;
use crate::platform::apple::developer_services;
use crate::platform::apple::grandslam::DeviceInfo;

/// Execute teams command
pub fn execute() -> Result<()> {
    let storage = CredentialStorage::new()?;
    let credentials = storage
        .load()?
        .ok_or_else(|| anyhow!("Not logged in. Run 'lingxia auth apple login' first."))?;

    let (adsid, app_token) = match &credentials {
        crate::platform::apple::auth::AuthCredentials::AppleId {
            adsid, app_token, ..
        } => (adsid.clone(), app_token.clone()),
        crate::platform::apple::auth::AuthCredentials::AppStoreConnect { .. } => {
            return Err(anyhow!(
                "App Store Connect API keys are not supported for this command.\n\
                 Run 'lingxia auth apple login' and choose Password mode instead."
            ));
        }
    };

    let mut anisette_provider = OmnisetteProvider::new();
    let anisette = anisette_provider
        .fetch_anisette_data()
        .context("Failed to get anisette data")?;

    let device_info = DeviceInfo::default_macos();

    let teams = developer_services::list_teams(&adsid, &app_token, &device_info, &anisette)?;

    if teams.is_empty() {
        println!("{}", "No development teams found.".yellow());
        return Ok(());
    }

    println!("{}", "Development Teams".cyan().bold());
    println!();

    for team in teams {
        println!(
            "- {} [{}]: {}",
            team.name.bold(),
            team.account_type(),
            team.id
        );
        for membership in &team.memberships {
            println!("  {} ({})", membership.name.dimmed(), membership.platform);
        }
    }

    Ok(())
}
