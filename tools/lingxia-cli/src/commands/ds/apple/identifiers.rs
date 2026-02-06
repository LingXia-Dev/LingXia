//! Identifiers subcommand for Apple Developer Services.

use anyhow::Result;
use colored::Colorize;

use super::client::with_client;

/// Execute identifiers command
pub fn execute() -> Result<()> {
    with_client(|client| {
        let app_ids = client.list_app_ids()?;

        if app_ids.is_empty() {
            println!("{}", "No bundle identifiers found.".yellow());
            return Ok(());
        }

        println!("{}", "Bundle Identifiers".cyan().bold());
        println!();

        for app_id in app_ids {
            println!("- id: {}", app_id.id.bold());

            if let Some(name) = &app_id.name {
                println!("  name: {}", name);
            }

            println!("  identifier: {}", app_id.identifier);

            if let Some(platform) = &app_id.platform {
                println!("  platform: {}", platform);
            }
        }

        Ok(())
    })
}
