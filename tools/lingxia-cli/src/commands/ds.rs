//! Developer Services commands.
//!
//! Platform-agnostic entry point for developer services.
//! Subcommands are organized by platform (apple, harmony, etc.)

use anyhow::Result;
use clap::Subcommand;

pub mod apple;
pub mod harmony;

/// Developer Services platform subcommands
#[derive(Subcommand)]
pub enum DsPlatform {
    /// Interact with Apple Developer Services
    Apple {
        #[command(subcommand)]
        command: apple::AppleCommand,
    },
    /// Interact with Harmony Developer Services
    Harmony {
        #[command(subcommand)]
        command: harmony::HarmonyCommand,
    },
}

/// Execute the ds command
pub fn execute(platform: DsPlatform) -> Result<()> {
    match platform {
        DsPlatform::Apple { command } => apple::execute(command),
        DsPlatform::Harmony { command } => harmony::execute(command),
    }
}
