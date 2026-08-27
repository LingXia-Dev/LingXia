//! `lingxia store` — submit installables to OS app stores (Microsoft Store,
//! App Store, AppGallery, Google Play, Xiaomi, OPPO, Honor). Talks to OS stores
//! only; never the LingXia server (that's `publish`) and never builds (that's
//! `build`). Credentials come from the wallet (`lingxia auth login <provider>`)
//! or the per-provider env groups; package identity comes from the platform
//! blocks in `lingxia.yaml`.

mod appgallery;
mod appstore;
mod artifact_identity;
mod backend;
pub(crate) mod creds;
mod googleplay;
mod honor;
mod msstore;
mod oppo;
mod xiaomi;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use colored::Colorize;
use std::env;

use crate::config::LingXiaConfig;
use crate::resolver::{self, AppleChannel, AppleNeed, AscMaterial};
use backend::{StorePlatform, SubmitOptions, find_artifact};
use creds::{resolve_googleplay, resolve_honor, resolve_msstore, resolve_oppo, resolve_xiaomi};

#[derive(Subcommand)]
pub enum StoreAction {
    /// Upload the built artifact (dist/<platform>/) to the OS store
    Submit {
        #[arg(short, long)]
        platform: String,
        /// Create the submission without committing it for review
        #[arg(long)]
        draft: bool,
        /// Release notes / "what's new" text
        #[arg(long)]
        release_notes: Option<String>,
        /// Release track/channel (store-specific)
        #[arg(long)]
        track: Option<String>,
    },
    /// Poll submission / processing status
    Status {
        #[arg(short, long)]
        platform: String,
    },
}

pub fn run(action: StoreAction) -> Result<()> {
    match action {
        StoreAction::Submit {
            platform,
            draft,
            release_notes,
            track,
        } => submit(
            StorePlatform::parse(&platform)?,
            SubmitOptions {
                draft,
                release_notes,
                track,
            },
        ),
        StoreAction::Status { platform } => status(StorePlatform::parse(&platform)?),
    }
}

fn load_config() -> Result<LingXiaConfig> {
    let root = env::current_dir().context("get current directory")?;
    LingXiaConfig::load(&root)
}

/// The platform-block identity the artifact must carry for this store.
fn expected_identity(config: &LingXiaConfig, platform: StorePlatform) -> Result<String> {
    Ok(match platform {
        StorePlatform::Ios | StorePlatform::Macos => apple_bundle_id(config, platform)?,
        StorePlatform::GooglePlay
        | StorePlatform::Xiaomi
        | StorePlatform::Oppo
        | StorePlatform::Honor => android_package(config)?.to_string(),
        StorePlatform::Harmony => config
            .harmony
            .as_ref()
            .map(|h| h.bundle_name.clone())
            .context("missing `harmony.bundleName` in lingxia.yaml")?,
        StorePlatform::Windows => {
            let fallback = config
                .app
                .as_ref()
                .map(|a| a.product_name.trim())
                .unwrap_or_default();
            crate::platform::windows::msix::sanitize_identity(
                config
                    .windows
                    .as_ref()
                    .and_then(|w| w.app_id.as_deref())
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .unwrap_or(fallback),
            )
        }
    })
}

fn android_package(config: &LingXiaConfig) -> Result<&str> {
    config
        .android
        .as_ref()
        .map(|a| a.package_id.as_str())
        .context("missing `android.packageId` in lingxia.yaml")
}

fn apple_bundle_id(config: &LingXiaConfig, platform: StorePlatform) -> Result<String> {
    match platform {
        StorePlatform::Ios => config
            .ios
            .as_ref()
            .map(|c| c.bundle_id.clone())
            .context("missing `ios.bundleId` in lingxia.yaml"),
        StorePlatform::Macos => config
            .macos
            .as_ref()
            .and_then(|c| c.bundle_id.clone())
            .context("missing `macos.bundleId` in lingxia.yaml"),
        _ => bail!("not an Apple platform"),
    }
}

/// Resolve the ASC key for store use (env group → wallet, in-place login).
fn asc_material(platform: StorePlatform) -> Result<AscMaterial> {
    let channel = match platform {
        StorePlatform::Ios => AppleChannel::Ios,
        StorePlatform::Macos => AppleChannel::Macos,
        _ => bail!("not an Apple platform"),
    };
    match resolver::resolve_apple_auth(Some(channel), AppleNeed::Asc)?.auth {
        crate::platform::apple::auth::AuthCredentials::AppStoreConnect {
            key_id,
            issuer_id,
            private_key_pem,
            ..
        } => Ok(AscMaterial {
            key_id,
            issuer_id,
            private_key_pem,
        }),
        _ => unreachable!("Asc need resolves to an ASC key"),
    }
}

fn submit(platform: StorePlatform, opts: SubmitOptions) -> Result<()> {
    let config = load_config()?;
    let root = env::current_dir()?;
    // Validate the artifact before touching credentials or the network, so a
    // wrong/missing package fails before any login flow starts.
    let artifact = find_artifact(&root, platform)?;
    artifact_identity::verify(&artifact, &expected_identity(&config, platform)?)?;

    println!(
        "{} submitting {} to {}",
        "→".cyan(),
        artifact.display(),
        platform.store_name()
    );

    match platform {
        StorePlatform::Windows => {
            let cfg = config
                .windows
                .as_ref()
                .and_then(|w| w.store.as_ref())
                .context("missing `windows.store` (appId) in lingxia.yaml")?;
            msstore::submit(&resolve_msstore()?, cfg, &artifact, &opts)?;
        }
        StorePlatform::Ios | StorePlatform::Macos => {
            appstore::submit(&asc_material(platform)?, platform, &artifact, &opts)?;
        }
        StorePlatform::Harmony => {
            let cfg = config
                .harmony
                .as_ref()
                .and_then(|h| h.store.as_ref())
                .context("missing `harmony.store` (appId) in lingxia.yaml")?;
            let agc = resolver::resolve_harmony_agc(true)?.credentials;
            appgallery::submit(&agc, cfg, &artifact, &opts)?;
        }
        StorePlatform::GooglePlay => {
            let default_track = config
                .android
                .as_ref()
                .and_then(|a| a.google_play_store.as_ref())
                .and_then(|s| s.default_track.as_deref());
            googleplay::submit(
                &resolve_googleplay()?,
                android_package(&config)?,
                default_track,
                &artifact,
                &opts,
            )?;
        }
        StorePlatform::Xiaomi => {
            xiaomi::submit(
                &resolve_xiaomi()?,
                android_package(&config)?,
                &artifact,
                &opts,
            )?;
        }
        StorePlatform::Oppo => {
            let app_id = config
                .android
                .as_ref()
                .and_then(|a| a.oppo_store.as_ref())
                .and_then(|s| s.app_id.as_deref());
            oppo::submit(
                &resolve_oppo()?,
                android_package(&config)?,
                app_id,
                &artifact,
                &opts,
            )?;
        }
        StorePlatform::Honor => {
            let cfg = config
                .android
                .as_ref()
                .and_then(|a| a.honor_store.as_ref())
                .context("missing `android.honorStore` (appId) in lingxia.yaml")?;
            honor::submit(&resolve_honor()?, &cfg.app_id, &artifact, &opts)?;
        }
    }
    println!("{} submit flow complete", "✓".green());
    Ok(())
}

fn status(platform: StorePlatform) -> Result<()> {
    let config = load_config()?;
    match platform {
        StorePlatform::Windows => {
            let cfg = config
                .windows
                .as_ref()
                .and_then(|w| w.store.as_ref())
                .context("missing `windows.store` (appId) in lingxia.yaml")?;
            msstore::status(&resolve_msstore()?, cfg)?;
        }
        StorePlatform::Ios | StorePlatform::Macos => {
            let bundle_id = apple_bundle_id(&config, platform)?;
            appstore::status(&asc_material(platform)?, &bundle_id)?;
        }
        StorePlatform::Harmony => {
            let cfg = config
                .harmony
                .as_ref()
                .and_then(|h| h.store.as_ref())
                .context("missing `harmony.store` (appId) in lingxia.yaml")?;
            let agc = resolver::resolve_harmony_agc(true)?.credentials;
            appgallery::status(&agc, cfg)?;
        }
        StorePlatform::GooglePlay => {
            googleplay::status(&resolve_googleplay()?, android_package(&config)?)?;
        }
        StorePlatform::Xiaomi => {
            xiaomi::status(&resolve_xiaomi()?, android_package(&config)?)?;
        }
        StorePlatform::Oppo => {
            oppo::status(&resolve_oppo()?, android_package(&config)?)?;
        }
        StorePlatform::Honor => {
            let cfg = config
                .android
                .as_ref()
                .and_then(|a| a.honor_store.as_ref())
                .context("missing `android.honorStore` (appId) in lingxia.yaml")?;
            honor::status(&resolve_honor()?, &cfg.app_id)?;
        }
    }
    Ok(())
}
