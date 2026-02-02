use anyhow::Result;
use clap::{Parser, Subcommand};

mod appicon;
mod commands;
mod config;
mod github;
mod lxapp;
mod path_completion;
mod platform;
pub mod sdk;
mod versions;

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
    /// Release build (debug is the default)
    #[arg(long)]
    release: bool,

    /// Rust features to enable (comma-separated)
    #[arg(short = 'f', long, value_delimiter = ',')]
    features: Vec<String>,

    /// Skip native Rust library compilation (use existing binaries)
    #[arg(long)]
    skip_native: bool,

    /// Target architectures (native host builds only)
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

        /// Target platforms (comma-separated): android, ios, harmony, all
        #[arg(short = 'p', long, value_delimiter = ',')]
        platform: Vec<String>,

        /// Package ID (e.g., com.example.app)
        #[arg(long)]
        package_id: Option<String>,

        /// Path to app icon (PNG, recommended 1024x1024)
        #[arg(long)]
        icon: Option<String>,

        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Generate or update app icons
    Icon {
        /// Path to app icon (PNG, recommended 1024x1024)
        icon_path: String,

        /// Target platform (if not specified, use all platforms from config)
        #[arg(short = 'p', long)]
        platform: Option<String>,

        /// Background color for adaptive icons (hex, e.g., "#FFFFFF")
        #[arg(short = 'b', long)]
        background_color: Option<String>,

        /// Generate legacy icons for Android minSdk < 26
        #[arg(long)]
        legacy: bool,
    },

    /// Build the project
    Build {
        #[command(flatten)]
        build_options: BuildOptions,

        /// Platforms to build (comma-separated). Defaults to all detected platforms.
        #[arg(long, value_delimiter = ',')]
        platform: Vec<String>,
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
            icon,
            yes,
        } => {
            commands::new::execute(name, project_type, platform, package_id, icon, yes)?;
        }
        Commands::Icon {
            icon_path,
            platform,
            background_color,
            legacy,
        } => {
            commands::icon::execute(icon_path, platform, background_color, legacy)?;
        }
        Commands::Build {
            build_options,
            platform,
        } => {
            commands::build::execute(
                build_options.release,
                build_options.features,
                !build_options.skip_native,
                build_options.targets,
                platform,
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
                build_options.release,
                build_options.features,
                !build_options.skip_native,
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
