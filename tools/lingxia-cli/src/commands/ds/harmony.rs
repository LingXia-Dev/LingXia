use crate::platform::harmony::{AgcConnectClient, AgcCredentialStorage, AgcToken};
use anyhow::{Context, Result, anyhow};
use clap::Subcommand;
use colored::Colorize;

#[derive(Subcommand)]
pub enum HarmonyCommand {
    /// List Harmony app IDs
    Appids {
        /// Harmony package name / bundle id (e.g. app.lingxia.wow)
        #[arg(long)]
        package_name: Option<String>,
    },
    /// List registered Harmony devices
    Devices,
    /// List Harmony signing certificates
    Certificates {
        /// Show release certificates (default is debug)
        #[arg(long)]
        release: bool,
    },
    /// List Harmony provisioning profiles
    Profiles {
        /// Show release profiles (default is debug)
        #[arg(long)]
        release: bool,
    },
}

pub fn execute(command: HarmonyCommand) -> Result<()> {
    with_client(|client, token| match command {
        HarmonyCommand::Appids { package_name } => {
            let Some(package_name) = package_name.as_deref() else {
                return Err(anyhow!("AGC `appid-list` API requires `--package-name`.\n"));
            };

            let appids = client.list_app_ids(token, Some(package_name))?;
            println!("{}", "Harmony App IDs".cyan().bold());
            println!();
            if appids.is_empty() {
                println!(
                    "{}",
                    format!("No app IDs found for package `{package_name}`.").yellow()
                );
                return Ok(());
            }

            for app in appids {
                println!("- appId: {}", app.app_id.bold());
                println!("  package: {}", app.package_name);
                if !app.app_name.is_empty() {
                    println!("  name: {}", app.app_name);
                }
                println!();
            }
            Ok(())
        }
        HarmonyCommand::Devices => {
            let devices = client.query_devices(token, None)?;
            println!("{}", "Harmony Devices".cyan().bold());
            println!();
            if devices.is_empty() {
                println!("{}", "No devices found.".yellow());
                return Ok(());
            }

            for device in devices {
                println!("- id: {}", device.id.bold());
                println!("  name: {}", device.device_name);
                println!("  udid: {}", device.udid);
                println!("  type: {}", device.device_type);
                println!();
            }
            Ok(())
        }
        HarmonyCommand::Certificates { release } => {
            let cert_type = if release { 2 } else { 1 };
            let certs = client.query_certificates(token, cert_type)?;
            println!(
                "{}",
                format!(
                    "Harmony {} Certificates",
                    if release { "Release" } else { "Debug" }
                )
                .cyan()
                .bold()
            );
            println!();
            if certs.is_empty() {
                println!("{}", "No certificates found.".yellow());
                return Ok(());
            }

            for cert in certs {
                println!("- id: {}", cert.id.bold());
                println!("  name: {}", cert.cert_name);
                println!("  type: {}", cert.cert_type);
                println!("  download: {}", cert.cert_download_url);
                println!();
            }
            Ok(())
        }
        HarmonyCommand::Profiles { release } => {
            let profile_type = if release { 2 } else { 1 };
            let profiles = client.query_profiles(token, profile_type, None)?;
            println!(
                "{}",
                format!(
                    "Harmony {} Profiles",
                    if release { "Release" } else { "Debug" }
                )
                .cyan()
                .bold()
            );
            println!();
            if profiles.is_empty() {
                println!("{}", "No profiles found.".yellow());
                return Ok(());
            }

            for profile in profiles {
                println!("- id: {}", profile.id.bold());
                println!("  name: {}", profile.provision_name);
                println!("  appId: {}", profile.app_id);
                println!("  certId: {}", profile.cert_id);
                if !profile.device_ids.is_empty() {
                    println!("  devices: {}", profile.device_ids.join(", "));
                }
                println!("  download: {}", profile.provision_download_url);
                println!();
            }
            Ok(())
        }
    })
}

fn with_client<F>(f: F) -> Result<()>
where
    F: FnOnce(&AgcConnectClient, &AgcToken) -> Result<()>,
{
    let storage = AgcCredentialStorage::new()?;
    let mut credentials = storage.load()?.ok_or_else(|| {
        anyhow!("Not logged in with AGC API mode. Run `lingxia auth harmony login --mode api`.")
    })?;

    let client = AgcConnectClient::new();
    let token = client
        .ensure_valid_token(&credentials)
        .context("Failed to refresh AGC access token")?;

    let changed = credentials.token.as_ref().is_none_or(|old| {
        old.access_token != token.access_token || old.expires_at != token.expires_at
    });
    if changed {
        credentials.token = Some(token.clone());
        storage.save(&credentials)?;
    }

    f(&client, &token)
}
