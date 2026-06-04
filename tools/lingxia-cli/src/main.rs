use anyhow::Result;
use clap::{Parser, Subcommand, error::ErrorKind};

mod appicon;
mod commands;
mod config;
// `gen` is a reserved keyword in Rust 2024 — escape it so the module name
// stays aligned with the user-facing `lingxia gen …` subcommand.
mod r#gen;
mod github;
#[path = "assets.rs"]
mod host_assets;
mod http_client;
mod i18n;
mod lxapp;
mod path_completion;
mod permission_cache;
mod platform;
mod runtime;
mod update;
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

    /// Skip native Rust library compilation (use existing binaries)
    #[arg(long)]
    skip_native: bool,

    /// Android ABIs (comma-separated). Default: arm64-v8a. Use `all` for arm32 + arm64.
    #[arg(
        long,
        value_delimiter = ',',
        long_help = "Android ABIs (comma-separated).\n\nDefault: arm64-v8a.\nUse `--abis all` to build both arm32 and arm64.\n\nSupported values:\n  - all\n  - arm64-v8a\n  - armeabi-v7a"
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

    /// Environment (developer | preview | release; alias `dev` for developer).
    /// All commands default to developer when --env is omitted; release and
    /// preview must be requested explicitly. Independent from --release/debug
    /// profile.
    #[arg(long = "env", value_parser = ["developer", "dev", "preview", "release"])]
    env_version: Option<String>,
}

#[derive(clap::Args, Clone)]
struct DevOptions {
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

    /// Allow starting another dev session for a platform that already has one
    /// running in this project. Without this flag, `lingxia dev` refuses to
    /// launch a second same-platform session so that `lxdev` cannot silently
    /// connect to the wrong one (the canonical "human + agent both ran
    /// `lingxia dev -p ios`" footgun).
    #[arg(long)]
    parallel: bool,
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

        /// Build only the native library, skipping platform packaging. Harmony
        /// stops after the .so (no ohpm/hvigor/.hap) — useful for CI to verify
        /// the cross-compile without the gated API-21 HarmonyOS SDK.
        #[arg(long)]
        native_only: bool,
    },

    /// Remove generated build artifacts
    Clean,

    /// Package release artifacts for publishing or delivery
    Package {
        #[command(flatten)]
        package_options: commands::package::PackageOptions,

        /// Platforms to package (comma-separated).
        #[arg(
            long,
            value_delimiter = ',',
            long_help = "Platforms to package (comma-separated).\n\nSupported values:\n  - android\n  - ios\n  - macos (aliases: mac, osx, macosx)\n  - harmony (alias: harmonyos)"
        )]
        platform: Vec<String>,

        /// Package all configured platforms (disabled by default)
        #[arg(long, conflicts_with = "platform")]
        all_platforms: bool,
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

        /// Suppress progress UI output (useful for automation)
        #[arg(long)]
        quiet: bool,
    },

    /// Uninstall an app from a device
    Uninstall {
        /// Bundle ID / Package ID to uninstall. If omitted, LingXia will try to infer it from lingxia.yaml.
        bundle_id: Option<String>,

        /// Device ID (required if multiple devices connected)
        #[arg(short = 'd', long)]
        device: Option<String>,

        /// Target platform (android, ios, harmony). Auto-detected if not specified.
        #[arg(short = 'p', long)]
        platform: Option<String>,
    },

    /// Launch an installed app on a device
    Launch {
        /// Bundle ID / Package ID to launch. If omitted, LingXia will try to infer it from lingxia.yaml.
        bundle_id: Option<String>,

        /// Device ID (required if multiple devices connected)
        #[arg(short = 'd', long)]
        device: Option<String>,

        /// Target platform (android, ios, harmony). Auto-detected if not specified.
        #[arg(short = 'p', long)]
        platform: Option<String>,

        /// Restart the app by terminating an existing instance before launch (best effort)
        #[arg(long)]
        restart: bool,
    },

    /// Development mode for app and lxapp projects
    Dev {
        #[command(flatten)]
        dev_options: DevOptions,
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

    /// Internal resource generation helpers
    #[command(hide = true)]
    Gen {
        #[command(subcommand)]
        command: GenCommand,
    },

    /// Publish a package to the LingXia server
    Publish {
        /// Bearer token for authentication (or set LINGXIA_AUTH_TOKEN env var)
        #[arg(long, env = "LINGXIA_AUTH_TOKEN")]
        token: String,

        /// LingXia server URL
        #[arg(long)]
        lingxia_server: Option<String>,

        /// Path to the package archive (app only)
        #[arg(long = "package-path")]
        package_path: Option<String>,

        /// App platform to publish: android, macos
        #[arg(long, value_parser = ["android", "macos"])]
        platform: Option<String>,

        /// Release channel for lxapp/lxplugin publishing: release, preview, developer.
        #[arg(long = "env", alias = "channel", value_parser = ["developer", "dev", "preview", "release"])]
        channel: Option<String>,

        /// Override lxapp view framework detection
        #[arg(long, value_parser = ["react", "vue", "html"])]
        framework: Option<String>,

        /// LxApp progress output mode
        #[arg(long, value_parser = ["task", "plain"])]
        progress: Option<String>,
    },
}

#[derive(Subcommand)]
enum GenCommand {
    /// Generate i18n resources
    I18n(r#gen::i18n::I18nConfig),
    /// Convert icons to platform-specific resources
    Icons(r#gen::icons::IconsConfig),
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
    let raw_args: Vec<String> = std::env::args().collect();
    update::maybe_auto_update();

    let cli = match Cli::try_parse_from(&raw_args) {
        Ok(cli) => cli,
        Err(err) => {
            err.print()?;
            let exit_code = match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => 0,
                _ => 2,
            };
            std::process::exit(exit_code);
        }
    };

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
            native_only,
        } => {
            commands::build::execute(commands::build::BuildExecuteOptions {
                release: build_options.release,
                build_native: !build_options.skip_native,
                abis: build_options.abis,
                macos_arch: build_options.macos_arch,
                framework: build_options.framework,
                progress: build_options.progress,
                platforms: platform,
                all_platforms,
                ipa,
                dmg,
                package: false,
                native_only,
                env_version: build_options.env_version,
            })?;
        }
        Commands::Clean => {
            commands::clean::execute()?;
        }
        Commands::Package {
            package_options,
            platform,
            all_platforms,
        } => {
            commands::package::execute(commands::package::PackageExecuteOptions {
                build_native: !package_options.skip_native,
                abis: package_options.abis,
                macos_arch: package_options.macos_arch,
                framework: package_options.framework,
                progress: package_options.progress,
                platforms: platform,
                all_platforms,
                env_version: package_options.env_version,
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
            quiet,
        } => {
            commands::install::execute(artifact, device, platform, reinstall, quiet)?;
        }
        Commands::Uninstall {
            bundle_id,
            device,
            platform,
        } => {
            commands::device::uninstall(bundle_id.as_deref(), device, platform)?;
        }
        Commands::Launch {
            bundle_id,
            device,
            platform,
            restart,
        } => {
            commands::device::launch(bundle_id.as_deref(), device, platform, restart)?;
        }
        Commands::Dev { dev_options } => {
            commands::dev::execute(commands::dev::DevExecuteOptions {
                release: dev_options.build_options.release,
                build_native: !dev_options.build_options.skip_native,
                abis: dev_options.build_options.abis,
                macos_arch: dev_options.build_options.macos_arch,
                framework: dev_options.build_options.framework,
                progress: dev_options.build_options.progress,
                device: dev_options.device,
                platform_arg: dev_options.platform,
                reinstall: dev_options.reinstall,
                env_version: dev_options.build_options.env_version,
                parallel: dev_options.parallel,
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
        Commands::Gen { command } => match command {
            GenCommand::I18n(config) => {
                r#gen::i18n::run(config)?;
            }
            GenCommand::Icons(config) => {
                r#gen::icons::run(config)?;
            }
        },
        Commands::Publish {
            token,
            lingxia_server,
            package_path,
            platform,
            channel,
            framework,
            progress,
        } => {
            commands::publish::execute(commands::publish::PublishOptions {
                token,
                lingxia_server,
                package: package_path,
                platform,
                channel,
                framework,
                progress,
            })?;
        }
    }

    Ok(())
}
