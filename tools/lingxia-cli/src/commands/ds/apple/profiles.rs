//! Profiles subcommand for Apple Developer Services.

use anyhow::Result;
use colored::Colorize;

use super::client::with_client;

/// Execute profiles command
pub fn execute() -> Result<()> {
    with_client(|client| {
        let profiles = client.list_provisioning_profiles()?;

        if profiles.is_empty() {
            println!("{}", "No provisioning profiles found.".yellow());
            return Ok(());
        }

        println!("{}", "Provisioning Profiles".cyan().bold());
        println!();

        for profile in profiles {
            println!("- id: {}", profile.id.bold());
            println!("  name: {}", profile.name);

            if let Some(platform) = &profile.platform {
                println!("  platform: {}", platform);
            }

            if let Some(profile_type) = &profile.profile_type {
                println!("  profile type: {}", profile_type);
            }

            if let Some(status) = &profile.status {
                let status_colored = match status.as_str() {
                    "Active" => status.green(),
                    "Expired" | "Invalid" => status.red(),
                    _ => status.yellow(),
                };
                println!("  profile state: {}", status_colored);
            }

            if let Some(uuid) = &profile.uuid {
                println!("  uuid: {}", uuid);
            }

            if let Some(date) = &profile.expiration_date {
                println!("  expiration date: {}", date);
            }

            // Display team identifier
            if let Some(team_id) = &profile.team_identifier {
                println!("    team identifiers:");
                println!("    - {}", team_id);
            }

            // Display entitlements
            if let Some(ents) = &profile.entitlements {
                println!("    entitlements: {}", ents);
            }
        }

        Ok(())
    })
}
