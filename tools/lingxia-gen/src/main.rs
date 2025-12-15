use anyhow::Result;
use clap::{Parser, Subcommand};
use lingxia_gen::assets::{self, AssetsConfig};
use lingxia_gen::i18n::{self, I18nConfig};
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
    /// Sync static assets
    Assets(AssetsConfig),
    /// Convert icons to platform-specific formats
    Icons(IconsConfig),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::I18n(config) => i18n::run(config),
        Commands::Assets(config) => assets::run(config),
        Commands::Icons(config) => icons::run(config),
    }
}
