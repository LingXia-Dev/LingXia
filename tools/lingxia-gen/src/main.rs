use anyhow::Result;
use clap::{Parser, Subcommand};
use lingxia_gen::i18n::{self, GenConfig as I18nConfig};
use lingxia_gen::icons::{self, IconsConfig};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate i18n resources
    I18n(I18nConfig),
    /// Sync SVG icons to platform-specific formats
    Icons(IconsConfig),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::I18n(config) => i18n::run(config),
        Commands::Icons(config) => icons::run(config),
    }
}
