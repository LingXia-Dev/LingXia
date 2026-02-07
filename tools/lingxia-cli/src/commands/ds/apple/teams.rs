//! Teams subcommand for Apple Developer Services.

use anyhow::Result;
use colored::Colorize;

use super::client::with_client;

/// Execute teams command
pub fn execute() -> Result<()> {
    with_client(|client| {
        let teams = client.list_teams()?;

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
    })
}
