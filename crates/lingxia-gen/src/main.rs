use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "lingxia-gen")]
#[command(about = "Generate LingXia SDK resources")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate i18n resources
    I18n(lingxia_gen::i18n::I18nConfig),
    /// Convert icons to platform-specific resources
    Icons(lingxia_gen::icons::IconsConfig),
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::I18n(config) => lingxia_gen::i18n::run(config),
        Command::Icons(config) => lingxia_gen::icons::run(config),
    }
}
