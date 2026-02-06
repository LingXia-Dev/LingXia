//! Apple Developer Services commands.
//!
//! Commands for interacting with Apple's Developer Services API.

use anyhow::Result;
use clap::Subcommand;

mod certificates;
mod client;
mod devices;
mod identifiers;
mod profiles;
mod teams;

/// Apple Developer Services subcommands
#[derive(Subcommand)]
pub enum AppleCommand {
    /// List development teams
    Teams,
    /// List certificates
    Certificates,
    /// List bundle identifiers (App IDs)
    Identifiers,
    /// List registered devices
    Devices,
    /// List provisioning profiles
    Profiles,
}

/// Execute the Apple command
pub fn execute(command: AppleCommand) -> Result<()> {
    match command {
        AppleCommand::Teams => teams::execute(),
        AppleCommand::Certificates => certificates::execute(),
        AppleCommand::Identifiers => identifiers::execute(),
        AppleCommand::Devices => devices::execute(),
        AppleCommand::Profiles => profiles::execute(),
    }
}
