use anyhow::Result;
use clap::{Parser, Subcommand, error::ErrorKind};

mod appicon;
mod cli_config;
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
mod npm;
mod path_completion;
mod permission_cache;
mod platform;
mod runner_cache;
mod runtime;
mod sdk_cache;
mod update;
mod versions;

#[derive(Parser)]
#[command(name = "lingxia")]
#[command(about = "LingXia CLI - Build cross-platform apps with ease", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Common build options shared between Build and Dev commands.
#[derive(clap::Args, Clone)]
struct BuildOptions {
    /// Release build (debug is the default)
    #[arg(long)]
    release: bool,

    /// Skip native Rust library compilation (use existing binaries)
    #[arg(long)]
    skip_native: bool,

    /// Override LxApp view framework detection (multi-framework demo projects
    /// only — a real project has exactly one; hidden from help)
    #[arg(long, hide = true, value_parser = ["react", "vue", "html"])]
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

    /// Extra Cargo feature(s) for the native Rust library. Can be repeated or
    /// comma-separated; also reads LINGXIA_NATIVE_FEATURES.
    #[arg(
        long = "native-feature",
        alias = "native-features",
        value_delimiter = ',',
        env = "LINGXIA_NATIVE_FEATURES"
    )]
    native_features: Vec<String>,

    /// Build with an optional private provider crate (e.g. `cloud`). Repeatable
    /// or comma-separated; also reads LINGXIA_WITH_PROVIDERS. The provider crate
    /// is injected only for this build, never committed. See --provider-path.
    #[arg(
        long = "with-provider",
        value_delimiter = ',',
        env = "LINGXIA_WITH_PROVIDERS"
    )]
    with_provider: Vec<String>,

    /// Local checkout path for the provider crate (used with --with-provider).
    /// Falls back to LINGXIA_PROVIDER_<NAME>_PATH / _GIT env when omitted.
    #[arg(long = "provider-path")]
    provider_path: Option<String>,
}

/// Platform-specific target overrides for distributable builds.
#[derive(clap::Args, Clone)]
struct PlatformBuildOptions {
    /// Android ABIs (comma-separated). Default: arm64-v8a. Use `all` for arm32 + arm64.
    #[arg(
        long = "android-abis",
        value_delimiter = ',',
        long_help = "Android ABIs (comma-separated).\n\nDefault: arm64-v8a.\nUse `--android-abis all` to build both arm32 and arm64.\n\nSupported values:\n  - all\n  - arm64-v8a\n  - armeabi-v7a"
    )]
    android_abis: Vec<String>,

    /// macOS architecture for native build; defaults to the host architecture
    #[arg(long, value_parser = ["arm64", "x86_64"])]
    macos_arch: Option<String>,
}

#[derive(clap::Args, Clone)]
struct DevOptions {
    #[command(flatten)]
    build_options: BuildOptions,

    /// Target platform (android, ios, macos, harmony, windows). Auto-detected if not specified.
    #[arg(short = 'p', long)]
    platform: Option<String>,

    /// Device ID (required if multiple devices connected)
    #[arg(short = 'd', long)]
    device: Option<String>,

    /// Reinstall app by uninstalling existing one first (best effort)
    #[arg(long)]
    reinstall: bool,

    /// Runner simulator device for `lingxia dev` on an lxapp (macOS and
    /// Windows runners): e.g. `iphone-15-pro`, `ipad`, `desktop-1440`. Only
    /// affects the lxapp runner window; ignored for native host apps.
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    runner: Option<String>,

    /// Start the dev session in the background and return after it is ready
    #[arg(long)]
    background: bool,

    #[command(subcommand)]
    action: Option<DevAction>,
}

#[derive(Subcommand, Clone)]
enum DevAction {
    /// List dev sessions for this project
    Status {
        /// Print pretty JSON
        #[arg(long)]
        json: bool,
    },
    /// Stop a dev session for this project
    Stop {
        /// Session id prefix or platform name. Omit when only one session is live.
        session: Option<String>,
        /// Kill the owning process if graceful shutdown fails
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum Commands {
    /// Show LingXia CLI version information
    Version {
        /// Print build, host, toolchain, and LingXia component versions
        #[arg(long)]
        verbose: bool,
    },

    /// Create a new LingXia project
    New {
        /// Project name
        name: Option<String>,

        /// Project type: native-app, lxapp
        #[arg(short = 't', long)]
        project_type: Option<String>,

        /// Target platforms (comma-separated): android, ios, macos, harmony, windows, all
        #[arg(short = 'p', long, value_delimiter = ',')]
        platform: Vec<String>,

        /// Package ID (e.g., com.example.app)
        #[arg(long)]
        package_id: Option<String>,

        /// Path to app icon (PNG, recommended 1024x1024)
        #[arg(long)]
        icon: Option<String>,

        /// (lxapp) Scaffold a LingXiao cloud worker: a `server/` worker (mock +
        /// live), `worker.json` routing, and a home page wired to `lx.cloud`.
        /// The worker id is always the lxapp's appId; it is not configurable.
        #[arg(long, hide = true)]
        worker: bool,

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

        /// Transparent artwork for Android/Harmony layered foregrounds
        /// (defaults to the main icon, which embeds its own background)
        #[arg(long)]
        foreground: Option<String>,

        /// Standalone conversion: write the source icon to this path instead of
        /// into a project. The extension picks the format — `.ico` (multi-size
        /// Windows icon) or `.png`. Used to (re)generate committed design assets
        /// (e.g. the dev-runner icon, favicon.ico, the appicon masters).
        #[arg(long)]
        output: Option<String>,

        /// Output size in px for `--output *.png` (default 1024). Ignored for `.ico`.
        #[arg(long)]
        size: Option<u32>,

        /// Preview only: analyze the source, render every platform treatment
        /// (masks, safe zones, small sizes) into icon-preview.html, and write
        /// nothing into the project.
        #[arg(long)]
        check: bool,
    },

    /// Build the project
    Build {
        #[command(flatten)]
        build_options: BuildOptions,

        #[command(flatten)]
        platform_options: PlatformBuildOptions,

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

        /// Android distribution format: `sideload` (APK, default) or `play` (AAB for Google Play).
        #[arg(long, value_parser = ["sideload", "play"])]
        dist: Option<String>,

        /// Package Windows build as an (unsigned) MSIX installer
        #[arg(long)]
        msix: bool,

        /// Self-sign the Windows MSIX: generate/reuse a self-signed cert
        /// (subject = the package Publisher), sign with signtool, and trust it
        /// so the package installs locally. Implies --msix.
        #[arg(long)]
        self_signed: bool,

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
            long_help = "Platforms to package (comma-separated).\n\nSupported values:\n  - android\n  - ios\n  - macos (aliases: mac, osx, macosx)\n  - harmony (alias: harmonyos)\n  - windows"
        )]
        platform: Vec<String>,

        /// Package all configured platforms (disabled by default)
        #[arg(long, conflicts_with = "platform")]
        all_platforms: bool,
    },

    /// List connected devices
    Devices {
        /// Target platform (android, ios, harmony, windows). Auto-detected if not specified.
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

        /// Target platform (android, ios, harmony, windows). Auto-detected if not specified.
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

    /// Per-user dev-session broker (started on demand by `lingxia dev`/`lxdev`)
    #[command(hide = true, name = "dev-broker")]
    DevBroker,

    /// Activate a process window from the signed-in Windows desktop session.
    #[cfg(target_os = "windows")]
    #[command(hide = true, name = "dev-focus-window")]
    DevFocusWindow {
        executable: String,
        excluded_pids: String,
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

    /// Submit installables to OS app stores (Microsoft Store, App Store, AppGallery)
    Store {
        #[command(subcommand)]
        action: commands::store::StoreAction,
    },

    /// Internal resource generation helpers
    #[command(hide = true)]
    Gen {
        #[command(subcommand)]
        command: GenCommand,
    },

    /// Publish a package to the LingXia server
    ///
    /// Run with no subcommand to upload; `lingxia publish login` saves the
    /// server URL + token to `~/.lingxia/cli/config.toml`.
    #[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
    Publish {
        #[command(subcommand)]
        action: Option<PublishAction>,

        #[command(flatten)]
        args: PublishArgs,
    },
}

#[derive(clap::Args)]
struct PublishArgs {
    /// Bearer token for authentication. Falls back to `[publish] token` in
    /// `~/.lingxia/cli/config.toml` when omitted.
    #[arg(long)]
    token: Option<String>,

    /// LingXia server URL
    #[arg(long)]
    lingxia_server: Option<String>,

    /// Path to the package archive (app only)
    #[arg(long = "package-path")]
    package_path: Option<String>,

    /// App platform to publish: android, macos, windows
    #[arg(long, value_parser = ["android", "macos", "windows"])]
    platform: Option<String>,

    /// Environment/channel for lxapp/lxplugin publishing: developer, preview,
    /// release. Defaults to developer; alias `dev` for developer.
    #[arg(long = "env", alias = "channel", value_parser = ["developer", "dev", "preview", "release"])]
    channel: Option<String>,

    /// Override lxapp view framework detection (multi-framework demo projects
    /// only — a real project has exactly one; hidden from help)
    #[arg(long, hide = true, value_parser = ["react", "vue", "html"])]
    framework: Option<String>,

    /// LxApp progress output mode
    #[arg(long, value_parser = ["task", "plain"])]
    progress: Option<String>,
}

#[derive(Subcommand)]
enum PublishAction {
    /// Save the publish server URL + token to `~/.lingxia/cli/config.toml`
    ///
    /// Pass `--env` to target a single channel's `[publish.<env>]` table;
    /// omit it to set the top-level defaults used by all channels. Existing
    /// values for other channels are preserved.
    Login {
        /// LingXia server URL to save
        #[arg(long)]
        server: Option<String>,

        /// Bearer token to save
        #[arg(long)]
        token: Option<String>,

        /// Channel to scope these credentials to: developer, preview, release.
        #[arg(long = "env", alias = "channel", value_parser = ["developer", "dev", "preview", "release"])]
        env: Option<String>,
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
    /// Import a Developer ID Application .p12 for macOS signing/notarization
    ImportDeveloperId {
        /// Path to the Developer ID Application .p12 certificate
        p12: String,

        /// Certificate password (will prompt if not provided)
        #[arg(long)]
        password: Option<String>,

        /// codesign identity name (auto-detected if not provided)
        #[arg(long)]
        identity: Option<String>,
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
        Commands::Version { verbose } => {
            commands::version::execute(verbose);
        }
        Commands::New {
            name,
            project_type,
            platform,
            package_id,
            icon,
            worker,
            yes,
        } => {
            commands::new::execute(name, project_type, platform, package_id, icon, worker, yes)?;
        }
        Commands::Icon {
            icon_path,
            platform,
            background_color,
            legacy,
            foreground,
            output,
            size,
            check,
        } => {
            commands::icon::execute(
                icon_path,
                platform,
                background_color,
                legacy,
                foreground,
                output,
                size,
                check,
            )?;
        }
        Commands::Build {
            build_options,
            platform_options,
            platform,
            all_platforms,
            ipa,
            dmg,
            dist,
            msix,
            self_signed,
            native_only,
        } => {
            commands::build::execute(commands::build::BuildExecuteOptions {
                release: build_options.release,
                build_native: !build_options.skip_native,
                android_abis: platform_options.android_abis,
                macos_arch: platform_options.macos_arch,
                framework: build_options.framework,
                progress: build_options.progress,
                platforms: platform,
                all_platforms,
                ipa,
                dmg,
                android_dist: dist,
                msix: msix || self_signed,
                self_signed,
                package: false,
                native_only,
                env_version: build_options.env_version,
                extra_native_features: build_options.native_features,
                with_provider: build_options.with_provider,
                provider_path: build_options.provider_path,
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
                android_abis: package_options.android_abis,
                macos_arch: package_options.macos_arch,
                framework: package_options.framework,
                progress: package_options.progress,
                platforms: platform,
                all_platforms,
                env_version: package_options.env_version,
                extra_native_features: package_options.native_features,
                with_provider: package_options.with_provider,
                provider_path: package_options.provider_path,
                android_dist: package_options.dist,
                msix: package_options.msix,
                self_signed: package_options.self_signed,
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
                framework: dev_options.build_options.framework,
                progress: dev_options.build_options.progress,
                device: dev_options.device,
                platform_arg: dev_options.platform,
                reinstall: dev_options.reinstall,
                env_version: dev_options.build_options.env_version,
                extra_native_features: dev_options.build_options.native_features,
                with_provider: dev_options.build_options.with_provider,
                provider_path: dev_options.build_options.provider_path,
                runner_device: dev_options.runner,
                background: dev_options.background,
                action: dev_options.action.map(|action| match action {
                    DevAction::Status { json } => commands::dev::DevSessionAction::Status { json },
                    DevAction::Stop { session, force } => {
                        commands::dev::DevSessionAction::Stop { session, force }
                    }
                }),
            })?;
        }
        Commands::DevBroker => {
            lingxia_devtool_protocol::broker::run_broker()?;
        }
        #[cfg(target_os = "windows")]
        Commands::DevFocusWindow {
            executable,
            excluded_pids,
        } => {
            commands::dev::focus_windows_launch(std::path::Path::new(&executable), &excluded_pids)?;
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
                AppleAuthAction::ImportDeveloperId {
                    p12,
                    password,
                    identity,
                } => {
                    commands::auth::apple_import_developer_id(p12, password, identity)?;
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
        Commands::Store { action } => {
            commands::store::run(action)?;
        }
        Commands::Gen { command } => match command {
            GenCommand::I18n(config) => {
                r#gen::i18n::run(config)?;
            }
            GenCommand::Icons(config) => {
                r#gen::icons::run(config)?;
            }
        },
        Commands::Publish { action, args } => match action {
            Some(PublishAction::Login { server, token, env }) => {
                commands::publish::save_login(server, token, env)?;
            }
            None => {
                commands::publish::execute(commands::publish::PublishOptions {
                    token: args.token,
                    lingxia_server: args.lingxia_server,
                    package: args.package_path,
                    platform: args.platform,
                    channel: args.channel,
                    framework: args.framework,
                    progress: args.progress,
                })?;
            }
        },
    }

    Ok(())
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn dev_runner_accepts_missing_value_for_device_list() {
        let cli = Cli::try_parse_from(["lingxia", "dev", "--runner"]).unwrap();
        let Commands::Dev { dev_options } = cli.command else {
            panic!("expected dev command");
        };
        assert_eq!(dev_options.runner.as_deref(), Some(""));
    }

    #[test]
    fn dev_runner_accepts_explicit_device_id() {
        let cli = Cli::try_parse_from(["lingxia", "dev", "--runner", "desktop-1440"]).unwrap();
        let Commands::Dev { dev_options } = cli.command else {
            panic!("expected dev command");
        };
        assert_eq!(dev_options.runner.as_deref(), Some("desktop-1440"));
    }

    #[test]
    fn dev_background_bootstrap_remains_available() {
        let cli = Cli::try_parse_from(["lingxia", "dev", "--background"]).unwrap();
        let Commands::Dev { dev_options } = cli.command else {
            panic!("expected dev command");
        };
        assert!(dev_options.background);
    }

    #[test]
    fn dev_lan_remote_control_is_removed() {
        assert!(Cli::try_parse_from(["lingxia", "dev", "--lan"]).is_err());
    }

    #[test]
    fn build_accepts_platform_specific_target_options() {
        let cli = Cli::try_parse_from([
            "lingxia",
            "build",
            "--platform",
            "android,macos",
            "--android-abis",
            "all",
            "--macos-arch",
            "arm64",
        ])
        .unwrap();
        let Commands::Build {
            platform_options, ..
        } = cli.command
        else {
            panic!("expected build command");
        };
        assert_eq!(platform_options.android_abis, vec!["all"]);
        assert_eq!(platform_options.macos_arch.as_deref(), Some("arm64"));
    }

    #[test]
    fn dev_does_not_accept_distribution_target_options() {
        assert!(Cli::try_parse_from(["lingxia", "dev", "--android-abis", "all"]).is_err());
        assert!(Cli::try_parse_from(["lingxia", "dev", "--macos-arch", "arm64"]).is_err());
    }

    #[test]
    fn version_command_accepts_verbose_flag() {
        let cli = Cli::try_parse_from(["lingxia", "version", "--verbose"]).unwrap();
        let Commands::Version { verbose } = cli.command else {
            panic!("expected version command");
        };
        assert!(verbose);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn internal_focus_command_accepts_launch_identity() {
        let cli = Cli::try_parse_from([
            "lingxia",
            "dev-focus-window",
            r"C:\Apps\Demo.exe",
            "123,456",
        ])
        .unwrap();
        let Commands::DevFocusWindow {
            executable,
            excluded_pids,
        } = cli.command
        else {
            panic!("expected dev-focus-window command");
        };
        assert_eq!(executable, r"C:\Apps\Demo.exe");
        assert_eq!(excluded_pids, "123,456");
    }
}
