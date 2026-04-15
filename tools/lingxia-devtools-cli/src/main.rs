use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::Path;

mod browser;
mod client;
mod logs;
mod project;

#[derive(Parser)]
#[command(name = "lxdev")]
#[command(about = "LingXia devtools client", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Control browser tabs in the current dev session
    Browser(browser::BrowserOptions),
    /// Query and filter the current dev session log file
    Logs(logs::LogsOptions),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let project_root = std::env::current_dir()?;
    let info = project::read_dev_info(&project_root)?;

    match cli.command {
        Commands::Browser(options) => browser::execute(&info, options),
        Commands::Logs(options) => logs::execute(Path::new(&info.log_file), options),
    }
}
