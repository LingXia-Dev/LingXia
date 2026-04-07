use anyhow::Result;
use clap::{Parser, Subcommand};

mod appicon;
mod commands;
mod config;
mod github;
mod host_assets;
mod http_client;
mod i18n;
mod lxapp;
mod path_completion;
mod permission_cache;
mod platform;
mod runtime;
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

/// Common build options shared between Build and Run commands
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

    /// Android ABIs (comma-separated)
    #[arg(
        long,
        value_delimiter = ',',
        long_help = "Android ABIs (comma-separated).\n\nSupported values:\n  - arm64-v8a\n  - armeabi-v7a"
    )]
    abis: Vec<String>,

    /// macOS architecture for native build
    #[arg(long, value_parser = ["arm64", "x86_64"])]
    macos_arch: Option<String>,

    /// Override LxApp view framework detection
    #[arg(long, value_parser = ["react", "vue", "html"])]
    framework: Option<String>,

    /// LxApp progress output mode
    #[arg(long, value_parser = ["task", "plain"])]
    progress: Option<String>,
}

#[derive(clap::Args, Clone)]
struct DevOptions {
    /// Release build (debug is the default)
    #[arg(long)]
    release: bool,

    /// Override LxApp view framework detection
    #[arg(long, value_parser = ["react", "vue", "html"])]
    framework: Option<String>,

    /// LxApp progress output mode
    #[arg(long, value_parser = ["task", "plain"])]
    progress: Option<String>,
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

        /// Target platforms (comma-separated): android, ios, macos, harmony, all
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

        /// Platforms to build (comma-separated).
        #[arg(
            long,
            value_delimiter = ',',
            long_help = "Platforms to build (comma-separated).\n\nSupported values:\n  - android\n  - ios\n  - macos (aliases: mac, osx, macosx)\n  - harmony (alias: harmonyos)"
        )]
        platform: Vec<String>,

        /// Build all configured platforms (disabled by default)
        #[arg(long, conflicts_with = "platform")]
        all_platforms: bool,

        /// Sign and package iOS build as IPA
        #[arg(long)]
        ipa: bool,

        /// Package macOS build as DMG
        #[arg(long)]
        dmg: bool,

        /// Package release artifacts (macOS update zip, LxApp/LxPlugin archive)
        #[arg(long)]
        package: bool,
    },

    /// List connected devices
    Devices {
        /// Target platform (android, ios, harmony). Auto-detected if not specified.
        #[arg(short = 'p', long)]
        platform: Option<String>,
    },

    /// Install the built app to a device
    Install {
        /// Path to artifact file (auto-detected if not specified)
        #[arg(short = 'a', long)]
        artifact: Option<String>,

        /// Device ID (required if multiple devices connected)
        #[arg(short = 'd', long)]
        device: Option<String>,

        /// Target platform (android, ios, harmony). Auto-detected if not specified.
        #[arg(short = 'p', long)]
        platform: Option<String>,

        /// Reinstall app by uninstalling existing one first (best effort)
        #[arg(long)]
        reinstall: bool,
    },

    /// Uninstall an app from a device
    Uninstall {
        /// Bundle ID / Package ID to uninstall
        bundle_id: String,

        /// Device ID (required if multiple devices connected)
        #[arg(short = 'd', long)]
        device: Option<String>,

        /// Target platform (android, ios, harmony). Auto-detected if not specified.
        #[arg(short = 'p', long)]
        platform: Option<String>,
    },

    /// Launch an installed app on a device
    Launch {
        /// Bundle ID / Package ID to launch
        bundle_id: String,

        /// Device ID (required if multiple devices connected)
        #[arg(short = 'd', long)]
        device: Option<String>,

        /// Target platform (android, ios, harmony). Auto-detected if not specified.
        #[arg(short = 'p', long)]
        platform: Option<String>,
    },

    /// Development mode for lxapp projects
    Dev {
        #[command(flatten)]
        dev_options: DevOptions,
    },

    /// Run mode: build, install, and launch app
    Run {
        #[command(flatten)]
        build_options: BuildOptions,

        /// Target platform (android, ios, macos, harmony). Auto-detected if not specified.
        #[arg(short = 'p', long)]
        platform: Option<String>,

        /// Device ID (required if multiple devices connected)
        #[arg(short = 'd', long)]
        device: Option<String>,

        /// Reinstall app by uninstalling existing one first (best effort)
        #[arg(long)]
        reinstall: bool,
    },

    /// Check development environment setup
    Doctor {
        /// Platforms to check (comma-separated). Defaults to configured platforms or all.
        #[arg(short = 'p', long, value_delimiter = ',')]
        platform: Vec<String>,
    },

    /// Developer account authentication (Apple/Harmony)
    Auth {
        #[command(subcommand)]
        provider: AuthProvider,
    },

    /// Interact with developer services (Apple, Harmony, etc.)
    Ds {
        #[command(subcommand)]
        platform: commands::ds::DsPlatform,
    },

    /// Publish a package to the API server
    Publish {
        /// Bearer token for authentication (or set LINGXIA_AUTH_TOKEN env var)
        #[arg(long, env = "LINGXIA_AUTH_TOKEN")]
        token: String,

        /// API server URL (overrides app.apiServer in lingxia.config.json)
        #[arg(long)]
        api_server: Option<String>,

        /// Target type: lxapp, lxplugin, app (auto-detected from project files if not specified)
        #[arg(long, value_parser = ["lxapp", "lxplugin", "app"])]
        target: Option<String>,

        /// Path to the package archive (auto-detected if not specified)
        #[arg(long)]
        package: Option<String>,

        /// Release channel (lxapp only): release, preview, developer
        #[arg(long, default_value = "developer", value_parser = ["release", "preview", "developer"])]
        release_type: String,
    },

}

#[derive(Subcommand)]
enum AuthProvider {
    /// Apple Developer authentication (iOS/macOS)
    Apple {
        #[command(subcommand)]
        action: AppleAuthAction,
    },
    /// Harmony authentication
    Harmony {
        #[command(subcommand)]
        action: HarmonyAuthAction,
    },
}

#[derive(Subcommand)]
enum AppleAuthAction {
    /// Login with Apple Developer account
    Login {
        /// Apple ID (email) for password mode
        #[arg(short, long)]
        username: Option<String>,

        /// Password for Apple ID mode (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,

        /// Authentication mode: key or password
        #[arg(short = 'm', long, value_parser = ["key", "password"])]
        mode: Option<String>,

        /// App Store Connect API Key ID (for --mode key)
        #[arg(long)]
        key_id: Option<String>,

        /// App Store Connect issuer ID (for --mode key)
        #[arg(long)]
        issuer_id: Option<String>,

        /// Path to App Store Connect private key (.p8) (for --mode key)
        #[arg(long)]
        private_key_path: Option<String>,

        /// Apple Developer Team ID (for --mode key)
        #[arg(long)]
        team_id: Option<String>,

        /// Replace existing credentials without interactive confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Logout and clear stored credentials
    Logout,
    /// Show current authentication status
    Status,
}

#[derive(Subcommand)]
enum HarmonyAuthAction {
    /// Login with Harmony account
    Login {
        /// Authentication mode: api
        #[arg(short = 'm', long, value_parser = ["api"])]
        mode: Option<String>,

        /// API mode client ID
        #[arg(long)]
        client_id: Option<String>,

        /// API mode client secret
        #[arg(long)]
        client_secret: Option<String>,

        /// Replace existing credentials without interactive confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Logout Harmony credentials
    Logout,
    /// Show Harmony authentication status
    Status,
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
            all_platforms,
            ipa,
            dmg,
            package,
        } => {
            commands::build::execute(commands::build::BuildExecuteOptions {
                release: build_options.release,
                features: build_options.features,
                build_native: !build_options.skip_native,
                abis: build_options.abis,
                macos_arch: build_options.macos_arch,
                framework: build_options.framework,
                progress: build_options.progress,
                platforms: platform,
                all_platforms,
                ipa,
                dmg,
                package,
            })?;
        }
        Commands::Devices { platform } => {
            commands::device::list_devices(platform)?;
        }
        Commands::Install {
            artifact,
            device,
            platform,
            reinstall,
        } => {
            commands::install::execute(artifact, device, platform, reinstall)?;
        }
        Commands::Uninstall {
            bundle_id,
            device,
            platform,
        } => {
            commands::device::uninstall(&bundle_id, device, platform)?;
        }
        Commands::Launch {
            bundle_id,
            device,
            platform,
        } => {
            commands::device::launch(&bundle_id, device, platform)?;
        }
        Commands::Dev { dev_options } => {
            commands::dev::execute_lxapp(commands::dev::LxAppDevOptions {
                release: dev_options.release,
                framework: dev_options.framework,
                progress: dev_options.progress,
            })?;
        }
        Commands::Run {
            build_options,
            platform,
            device,
            reinstall,
        } => {
            commands::dev::execute_run(commands::dev::RunExecuteOptions {
                release: build_options.release,
                features: build_options.features,
                build_native: !build_options.skip_native,
                abis: build_options.abis,
                macos_arch: build_options.macos_arch,
                framework: build_options.framework,
                progress: build_options.progress,
                device,
                platform_arg: platform,
                reinstall,
            })?;
        }
        Commands::Doctor { platform } => {
            commands::doctor::execute(platform)?;
        }
        Commands::Auth { provider } => match provider {
            AuthProvider::Apple { action } => match action {
                AppleAuthAction::Login {
                    username,
                    password,
                    mode,
                    key_id,
                    issuer_id,
                    private_key_path,
                    team_id,
                    yes,
                } => {
                    commands::auth::apple_login(commands::auth::AppleLoginOptions {
                        username,
                        password,
                        mode,
                        key_id,
                        issuer_id,
                        private_key_path,
                        team_id,
                        yes,
                    })?;
                }
                AppleAuthAction::Logout => {
                    commands::auth::apple_logout()?;
                }
                AppleAuthAction::Status => {
                    commands::auth::apple_status()?;
                }
            },
            AuthProvider::Harmony { action } => match action {
                HarmonyAuthAction::Login {
                    mode,
                    client_id,
                    client_secret,
                    yes,
                } => {
                    commands::auth::harmony_login(commands::auth::HarmonyLoginOptions {
                        mode,
                        client_id,
                        client_secret,
                        yes,
                    })?;
                }
                HarmonyAuthAction::Logout => {
                    commands::auth::harmony_logout()?;
                }
                HarmonyAuthAction::Status => {
                    commands::auth::harmony_status()?;
                }
            },
        },
        Commands::Ds { platform } => {
            commands::ds::execute(platform)?;
        }
        Commands::Publish {
            token,
            api_server,
            target,
            package,
            release_type,
        } => {
            commands::publish::execute(commands::publish::PublishOptions {
                token,
                api_server,
                target,
                package,
                release_type,
            })?;
        }
    }

    Ok(())
}
