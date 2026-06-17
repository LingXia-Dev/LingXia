//! `lingxia store` — submit installables to OS app stores (Microsoft Store,
//! App Store, AppGallery). Talks to OS stores only; never the LingXia server
//! (that's `publish`) and never builds (that's `build`).

mod appgallery;
mod appstore;
mod backend;
mod creds;
mod msstore;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use colored::Colorize;
use dialoguer::{Input, Password};
use std::env;

use crate::config::LingXiaConfig;
use backend::{StorePlatform, SubmitOptions, find_artifact};
use creds::{
    AppGalleryCreds, AppStoreCreds, MsStoreCreds, StoreCredentials, resolve_appgallery,
    resolve_appstore, resolve_msstore,
};

#[derive(Subcommand)]
pub enum StoreAction {
    /// Store API credentials in ~/.lingxia/store/credentials.toml
    Login {
        /// Target platform: windows, ios, macos, harmony
        #[arg(short, long)]
        platform: String,
    },
    /// Remove cached credentials for a platform
    Logout {
        #[arg(short, long)]
        platform: String,
    },
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
        StoreAction::Login { platform } => login(StorePlatform::parse(&platform)?),
        StoreAction::Logout { platform } => logout(StorePlatform::parse(&platform)?),
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

fn login(platform: StorePlatform) -> Result<()> {
    println!(
        "\n{}\n",
        format!("{} credentials", platform.store_name())
            .cyan()
            .bold()
    );
    let mut file = StoreCredentials::load()?;
    match platform {
        StorePlatform::Windows => {
            file.msstore = Some(MsStoreCreds {
                tenant: prompt("Azure AD tenant ID")?,
                client_id: prompt("Client ID")?,
                client_secret: prompt_secret("Client secret")?,
                seller_id: prompt_opt("Seller ID (optional)")?,
            });
        }
        StorePlatform::Ios | StorePlatform::Macos => {
            file.appstore = Some(AppStoreCreds {
                issuer_id: prompt("App Store Connect issuer ID")?,
                key_id: prompt("API key ID")?,
                key_path: prompt("Path to .p8 private key")?,
            });
        }
        StorePlatform::Harmony => {
            file.appgallery = Some(AppGalleryCreds {
                client_id: prompt("AppGallery client ID")?,
                client_secret: prompt_secret("Client secret")?,
            });
        }
    }
    file.save()?;
    println!(
        "\n{} saved to {}",
        "✓".green(),
        creds::credentials_path()?.display()
    );
    Ok(())
}

fn logout(platform: StorePlatform) -> Result<()> {
    let mut file = StoreCredentials::load()?;
    match platform {
        StorePlatform::Windows => file.msstore = None,
        StorePlatform::Ios | StorePlatform::Macos => file.appstore = None,
        StorePlatform::Harmony => file.appgallery = None,
    }
    file.save()?;
    println!(
        "{} cleared {} credentials",
        "✓".green(),
        platform.store_name()
    );
    Ok(())
}

fn submit(platform: StorePlatform, opts: SubmitOptions) -> Result<()> {
    let config = load_config()?;
    let root = env::current_dir()?;
    let artifact = find_artifact(&root, platform)?;
    let file = StoreCredentials::load()?;

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
            msstore::submit(&resolve_msstore(&file)?, cfg, &artifact, &opts)?;
        }
        StorePlatform::Ios | StorePlatform::Macos => {
            // bundle_id is read for status; submit only needs creds + artifact.
            let _cfg = apple_cfg(&config, platform)?;
            appstore::submit(&resolve_appstore(&file)?, platform, &artifact, &opts)?;
        }
        StorePlatform::Harmony => {
            let cfg = config
                .harmony
                .as_ref()
                .and_then(|h| h.store.as_ref())
                .context("missing `harmony.store` (appId) in lingxia.yaml")?;
            appgallery::submit(&resolve_appgallery(&file)?, cfg, &artifact, &opts)?;
        }
    }
    println!("{} submit flow complete", "✓".green());
    Ok(())
}

fn status(platform: StorePlatform) -> Result<()> {
    let config = load_config()?;
    let file = StoreCredentials::load()?;
    match platform {
        StorePlatform::Windows => {
            let cfg = config
                .windows
                .as_ref()
                .and_then(|w| w.store.as_ref())
                .context("missing `windows.store` (appId) in lingxia.yaml")?;
            msstore::status(&resolve_msstore(&file)?, cfg)?;
        }
        StorePlatform::Ios | StorePlatform::Macos => {
            let cfg = apple_cfg(&config, platform)?;
            appstore::status(&resolve_appstore(&file)?, cfg)?;
        }
        StorePlatform::Harmony => {
            let cfg = config
                .harmony
                .as_ref()
                .and_then(|h| h.store.as_ref())
                .context("missing `harmony.store` (appId) in lingxia.yaml")?;
            appgallery::status(&resolve_appgallery(&file)?, cfg)?;
        }
    }
    Ok(())
}

fn apple_cfg(
    config: &LingXiaConfig,
    platform: StorePlatform,
) -> Result<&crate::config::AppStoreConfig> {
    let cfg = match platform {
        StorePlatform::Ios => config.ios.as_ref().and_then(|c| c.store.as_ref()),
        StorePlatform::Macos => config.macos.as_ref().and_then(|c| c.store.as_ref()),
        _ => unreachable!(),
    };
    cfg.with_context(|| {
        format!(
            "missing `{}.store` (bundleId) in lingxia.yaml",
            if platform == StorePlatform::Ios {
                "ios"
            } else {
                "macos"
            }
        )
    })
}

fn prompt(label: &str) -> Result<String> {
    Input::new()
        .with_prompt(label)
        .interact_text()
        .with_context(|| format!("read {label}"))
}

fn prompt_opt(label: &str) -> Result<Option<String>> {
    let v: String = Input::new()
        .with_prompt(label)
        .allow_empty(true)
        .interact_text()
        .with_context(|| format!("read {label}"))?;
    Ok(Some(v).filter(|s| !s.trim().is_empty()))
}

fn prompt_secret(label: &str) -> Result<String> {
    let v = Password::new()
        .with_prompt(label)
        .interact()
        .with_context(|| format!("read {label}"))?;
    if v.trim().is_empty() {
        bail!("{label} cannot be empty");
    }
    Ok(v)
}
