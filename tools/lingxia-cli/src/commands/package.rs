use crate::commands::build::{self, BuildExecuteOptions};
use anyhow::Result;
use clap::Args;

#[derive(Args, Clone)]
pub struct PackageOptions {
    /// Skip native Rust library compilation (use existing binaries)
    #[arg(long)]
    pub skip_native: bool,

    /// Android ABIs (comma-separated). Default: arm64-v8a. Use `all` for arm32 + arm64.
    #[arg(
        long,
        value_delimiter = ',',
        long_help = "Android ABIs (comma-separated).\n\nDefault: arm64-v8a.\nUse `--abis all` to build both arm32 and arm64.\n\nSupported values:\n  - all\n  - arm64-v8a\n  - armeabi-v7a"
    )]
    pub abis: Vec<String>,

    /// macOS architecture for native build
    #[arg(long, value_parser = ["arm64", "x86_64"])]
    pub macos_arch: Option<String>,

    /// Override LxApp view framework detection
    #[arg(long, value_parser = ["react", "vue", "html"])]
    pub framework: Option<String>,

    /// LxApp progress output mode
    #[arg(long, value_parser = ["task", "plain"])]
    pub progress: Option<String>,

    /// Environment (developer | preview | release; alias `dev`).
    #[arg(long = "env", value_parser = ["developer", "dev", "preview", "release"])]
    pub env_version: Option<String>,
}

pub struct PackageExecuteOptions {
    pub build_native: bool,
    pub abis: Vec<String>,
    pub macos_arch: Option<String>,
    pub framework: Option<String>,
    pub progress: Option<String>,
    pub platforms: Vec<String>,
    pub all_platforms: bool,
    pub env_version: Option<String>,
}

pub fn execute(options: PackageExecuteOptions) -> Result<()> {
    // `package` produces shippable artifacts; default to the release env when
    // --env is omitted. `build`/`dev` keep their developer default for
    // day-to-day work. Explicit --env on `package` always wins.
    let env_version = options.env_version.or_else(|| Some("release".to_string()));
    build::execute(BuildExecuteOptions {
        release: true,
        build_native: options.build_native,
        abis: options.abis,
        macos_arch: options.macos_arch,
        framework: options.framework,
        progress: options.progress,
        platforms: options.platforms,
        all_platforms: options.all_platforms,
        ipa: false,
        dmg: false,
        msix: false,
        package: true,
        // Packaging needs the full platform artifact, not just the native lib.
        native_only: false,
        env_version,
    })
}
