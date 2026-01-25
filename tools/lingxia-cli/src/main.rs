use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod config;
mod platform;

#[derive(Parser)]
#[command(name = "lingxia")]
#[command(about = "LingXia CLI - Build cross-platform apps with ease", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Common build options shared between Build and Dev commands
#[derive(clap::Args, Clone)]
struct BuildOptions {
    /// Build profile: debug or release
    #[arg(short = 'p', long, default_value = "debug")]
    profile: Option<String>,

    /// Rust features to enable (comma-separated)
    #[arg(short = 'f', long, value_delimiter = ',')]
    features: Vec<String>,

    /// Skip native library compilation (use existing binaries)
    #[arg(long)]
    skip_native: bool,

    /// Target architectures
    #[arg(short = 't', long, value_delimiter = ',')]
    targets: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new LingXia project
    New {
        /// Project name
        name: Option<String>,

        /// Project type: native-app, lxapp
        #[arg(short = 't', long)]
        project_type: Option<String>,

        /// Target platform: android, ios, harmony
        #[arg(short = 'p', long)]
        platform: Option<String>,

        /// Package ID (e.g., com.example.app)
        #[arg(long)]
        package_id: Option<String>,

        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Build the project
    Build {
        #[command(flatten)]
        build_options: BuildOptions,
    },

    /// Install the built app to a device
    Install {
        /// Path to artifact file (auto-detected if not specified)
        #[arg(short = 'a', long)]
        artifact: Option<String>,

        /// Device ID (required if multiple devices connected)
        #[arg(short = 'd', long)]
        device: Option<String>,
    },

    /// Development mode: build, install, and launch app
    Dev {
        #[command(flatten)]
        build_options: BuildOptions,

        /// Device ID (required if multiple devices connected)
        #[arg(short = 'd', long)]
        device: Option<String>,
    },

    /// Check Android development environment setup
    Doctor,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New {
            name,
            project_type,
            platform,
            package_id,
            yes,
        } => {
            commands::new::execute(
                name,
                project_type,
                platform,
                package_id,
                yes,
            )?;
        }
        Commands::Build { build_options } => {
            commands::build::execute(
                build_options.profile,
                build_options.features,
                build_options.skip_native,
                build_options.targets,
            )?;
        }
        Commands::Install { artifact, device } => {
            commands::install::execute(artifact, device)?;
        }
        Commands::Dev {
            build_options,
            device,
        } => {
            commands::dev::execute(
                build_options.profile,
                build_options.features,
                build_options.skip_native,
                build_options.targets,
                device,
            )?;
        }
        Commands::Doctor => {
            commands::doctor::execute()?;
        }
    }

    Ok(())
}
