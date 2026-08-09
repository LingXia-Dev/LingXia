use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use lingxia_control_protocol::methods;
use serde_json::{Value, json};

use crate::transport::Transport;

#[derive(Args)]
pub struct TerminalOptions {
    #[command(subcommand)]
    command: TerminalCommand,
}

#[derive(Subcommand)]
enum TerminalCommand {
    /// Read, update, or reset terminal configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// List or import terminal color schemes
    Themes {
        #[command(subcommand)]
        command: ThemesCommand,
    },
    /// List installed terminal fonts
    Fonts {
        #[command(subcommand)]
        command: FontsCommand,
    },
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Show the resolved configuration
    Get {
        /// Print compact machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Merge a JSON object into the current configuration
    Apply {
        /// Partial TerminalConfig JSON object
        #[arg(long, value_name = "JSON")]
        patch: String,
        /// Print compact machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Remove user overrides
    Reset {
        #[arg(value_enum)]
        scope: Option<ResetScope>,
        /// Print compact machine-readable JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ResetScope {
    Font,
    Theme,
}

impl ResetScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Font => "font",
            Self::Theme => "theme",
        }
    }
}

#[derive(Subcommand)]
enum ThemesCommand {
    /// List available schemes with their full colors
    List {
        /// Print compact machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Import Windows Terminal JSON or Xresources/kitty text
    Import {
        file: PathBuf,
        #[arg(long)]
        name: Option<String>,
        /// Print compact machine-readable JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum FontsCommand {
    /// List installed monospace families and their capabilities
    List {
        /// Print compact machine-readable JSON
        #[arg(long)]
        json: bool,
    },
}

pub fn execute(transport: &dyn Transport, options: TerminalOptions) -> i32 {
    let result = match options.command {
        TerminalCommand::Config { command } => execute_config(transport, command),
        TerminalCommand::Themes { command } => execute_themes(transport, command),
        TerminalCommand::Fonts { command } => execute_fonts(transport, command),
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Error: {error}");
            10
        }
    }
}

fn execute_config(transport: &dyn Transport, command: ConfigCommand) -> anyhow::Result<()> {
    let (method, params, compact) = match command {
        ConfigCommand::Get { json } => (methods::terminal::CONFIG_GET, None, json),
        ConfigCommand::Apply { patch, json } => {
            let patch: Value = serde_json::from_str(&patch)?;
            (methods::terminal::CONFIG_APPLY, Some(patch), json)
        }
        ConfigCommand::Reset { scope, json } => (
            methods::terminal::CONFIG_RESET,
            Some(json!({"scope": scope.map(ResetScope::as_str)})),
            json,
        ),
    };
    print_response(transport.request(method, params)?, compact)
}

fn execute_themes(transport: &dyn Transport, command: ThemesCommand) -> anyhow::Result<()> {
    match command {
        ThemesCommand::List { json } => print_response(
            transport.request(methods::terminal::THEMES_LIST, None)?,
            json,
        ),
        ThemesCommand::Import { file, name, json } => {
            let text = std::fs::read_to_string(&file)?;
            print_response(
                transport.request(
                    methods::terminal::THEMES_IMPORT,
                    Some(json!({"text": text, "name": name})),
                )?,
                json,
            )
        }
    }
}

fn execute_fonts(transport: &dyn Transport, command: FontsCommand) -> anyhow::Result<()> {
    match command {
        FontsCommand::List { json } => print_response(
            transport.request(methods::terminal::FONTS_LIST, None)?,
            json,
        ),
    }
}

fn print_response(response: Option<Value>, compact: bool) -> anyhow::Result<()> {
    let value = response.unwrap_or(Value::Null);
    if compact {
        println!("{}", serde_json::to_string(&value)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&value)?);
    }
    Ok(())
}
