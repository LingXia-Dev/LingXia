//! OS-store credential login flows (wallet-backed).
//!
//! Identities: Google Play = service-account email, Microsoft Store = tenant,
//! Xiaomi/OPPO/Honor = client id. Multiple identities per provider co-exist;
//! logging into the same identity again is a confirmed rotation.

use anyhow::{Context, Result, anyhow, bail};
use colored::Colorize;
use dialoguer::{Confirm, Input, Password};

use crate::commands::store::creds::{
    GooglePlayCreds, HonorCreds, MsStoreCreds, OppoCreds, XiaomiCreds,
};
use crate::wallet::{Wallet, credential_fingerprint, display_fingerprint};

pub const STORE_PROVIDERS: &[&str] = &["googleplay", "xiaomi", "oppo", "honor", "msstore"];

#[derive(Default)]
pub struct StoreLoginOptions {
    pub service_account_json: Option<String>,
    pub tenant: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub seller_id: Option<String>,
    pub yes: bool,
}

fn provider_title(provider: &str) -> &'static str {
    match provider {
        "googleplay" => "Google Play",
        "xiaomi" => "Xiaomi GetApps",
        "oppo" => "OPPO 软件商店",
        "honor" => "Honor AppGallery",
        "msstore" => "Microsoft Store",
        _ => "store",
    }
}

/// Execute `lingxia auth login <store-provider>`.
pub fn store_login(provider: &str, options: StoreLoginOptions) -> Result<()> {
    println!(
        "\n{}\n",
        format!("{} credentials", provider_title(provider))
            .cyan()
            .bold()
    );
    store_login_inner(provider, options)?;
    Ok(())
}

/// In-place login used by store resolution; returns the identity.
pub fn store_inline_login(provider: &str) -> Result<String> {
    eprintln!(
        "{} Missing {} credentials; logging in now, then the command continues.",
        "→".cyan(),
        provider_title(provider)
    );
    store_login_inner(provider, StoreLoginOptions::default())
}

fn store_login_inner(provider: &str, options: StoreLoginOptions) -> Result<String> {
    let wallet = Wallet::open()?;
    wallet.notice_legacy_files();

    let (identity, secret_fingerprint, save): (String, String, Box<dyn FnOnce() -> Result<_>>) =
        match provider {
            "googleplay" => {
                let path = match options.service_account_json {
                    Some(p) => p,
                    None => prompt("Path to service-account JSON key")?,
                };
                let expanded = expand(&path);
                let content = std::fs::read_to_string(&expanded)
                    .with_context(|| format!("read {}", expanded.display()))?;
                let (email, creds, fingerprint) = parse_googleplay_service_account(&content)?;
                let w = Wallet::open()?;
                let id = email.clone();
                (
                    email,
                    fingerprint,
                    Box::new(move || w.save_store_creds("googleplay", &id, &creds)),
                )
            }
            "msstore" => {
                let tenant = opt_or_prompt(options.tenant, "Azure AD tenant ID")?;
                let client_id = opt_or_prompt(options.client_id, "Client ID")?;
                let client_secret = secret_or_prompt(options.client_secret, "Client secret")?;
                let seller_id = optional_nonempty(options.seller_id);
                let creds = MsStoreCreds {
                    tenant: tenant.clone(),
                    client_id,
                    client_secret,
                    seller_id,
                };
                let fingerprint = credential_fingerprint(&creds)?;
                let w = Wallet::open()?;
                let id = tenant.clone();
                (
                    tenant,
                    fingerprint,
                    Box::new(move || w.save_store_creds("msstore", &id, &creds)),
                )
            }
            "xiaomi" | "oppo" | "honor" => {
                let client_id = opt_or_prompt(
                    options.client_id,
                    &format!("{} client ID", provider_title(provider)),
                )?;
                let client_secret = secret_or_prompt(options.client_secret, "Client secret")?;
                let w = Wallet::open()?;
                let id = client_id.clone();
                let p: &'static str = match provider {
                    "xiaomi" => "xiaomi",
                    "oppo" => "oppo",
                    _ => "honor",
                };
                let (fingerprint, save): (String, Box<dyn FnOnce() -> Result<_>>) = match p {
                    "xiaomi" => {
                        let creds = XiaomiCreds {
                            client_id: client_id.clone(),
                            client_secret,
                        };
                        let fingerprint = credential_fingerprint(&creds)?;
                        (
                            fingerprint,
                            Box::new(move || w.save_store_creds(p, &id, &creds)),
                        )
                    }
                    "oppo" => {
                        let creds = OppoCreds {
                            client_id: client_id.clone(),
                            client_secret,
                        };
                        let fingerprint = credential_fingerprint(&creds)?;
                        (
                            fingerprint,
                            Box::new(move || w.save_store_creds(p, &id, &creds)),
                        )
                    }
                    _ => {
                        let creds = HonorCreds {
                            client_id: client_id.clone(),
                            client_secret,
                        };
                        let fingerprint = credential_fingerprint(&creds)?;
                        (
                            fingerprint,
                            Box::new(move || w.save_store_creds(p, &id, &creds)),
                        )
                    }
                };
                (client_id, fingerprint, save)
            }
            other => bail!("unknown store provider: {other}"),
        };

    // Same identity + different material is a rotation: show it and confirm.
    if let Some(old) = existing_fingerprint(&wallet, provider, &identity)?
        && old != secret_fingerprint
    {
        let old_display = display_fingerprint(&old);
        let new_display = display_fingerprint(&secret_fingerprint);
        println!(
            "{} Replacing the stored {} credential for {identity}: {old_display} -> {new_display}",
            "ℹ".blue(),
            provider_title(provider)
        );
        if !options.yes {
            if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                bail!("refusing to rotate credentials non-interactively; pass --yes to confirm");
            }
            if !Confirm::new()
                .with_prompt("Continue?")
                .default(true)
                .interact()?
            {
                bail!("Login cancelled.");
            }
        }
    }

    let path = save()?;
    println!();
    println!(
        "{} Saved {} credentials.",
        "✓".green(),
        provider_title(provider)
    );
    println!("  Identity: {identity}");
    println!("  Saved to: {}", path.display());
    Ok(identity)
}

fn parse_googleplay_service_account(content: &str) -> Result<(String, GooglePlayCreds, String)> {
    let json: serde_json::Value =
        serde_json::from_str(content).context("parse service-account JSON")?;
    let email = json
        .get("client_email")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("service-account JSON has no `client_email`"))?
        .to_string();
    let private_key = json
        .get("private_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("service-account JSON has no `private_key`"))?
        .to_string();
    let creds = GooglePlayCreds {
        service_account_json: None,
        client_email: Some(email.clone()),
        private_key: Some(private_key),
    };
    let fingerprint = credential_fingerprint(&creds)?;
    Ok((email, creds, fingerprint))
}

fn existing_fingerprint(wallet: &Wallet, provider: &str, identity: &str) -> Result<Option<String>> {
    Ok(match provider {
        "googleplay" => wallet
            .load_store_creds::<GooglePlayCreds>(provider, identity)?
            .map(|credentials| credential_fingerprint(&credentials))
            .transpose()?,
        "msstore" => wallet
            .load_store_creds::<MsStoreCreds>(provider, identity)?
            .map(|credentials| credential_fingerprint(&credentials))
            .transpose()?,
        "xiaomi" => wallet
            .load_store_creds::<XiaomiCreds>(provider, identity)?
            .map(|credentials| credential_fingerprint(&credentials))
            .transpose()?,
        "oppo" => wallet
            .load_store_creds::<OppoCreds>(provider, identity)?
            .map(|credentials| credential_fingerprint(&credentials))
            .transpose()?,
        "honor" => wallet
            .load_store_creds::<HonorCreds>(provider, identity)?
            .map(|credentials| credential_fingerprint(&credentials))
            .transpose()?,
        _ => None,
    })
}

/// Execute `lingxia auth logout <store-provider>`.
pub fn store_logout(provider: &str, identity: Option<String>) -> Result<()> {
    let wallet = Wallet::open()?;
    let identities = wallet.store_identities(provider)?;

    if identities.is_empty() {
        println!(
            "{} No {} credentials stored.",
            "ℹ".blue(),
            provider_title(provider)
        );
        return Ok(());
    }

    let identity = match identity {
        Some(id) => id,
        None if identities.len() == 1 => identities[0].clone(),
        None => {
            if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                bail!(
                    "several identities are stored ({}); pass --identity",
                    identities.join(", ")
                );
            }
            let selection = dialoguer::Select::new()
                .with_prompt("Log out which identity?")
                .items(&identities)
                .default(0)
                .interact()?;
            identities[selection].clone()
        }
    };

    if wallet.delete_store_identity(provider, &identity)? {
        println!(
            "{} Removed {} credentials for {}.",
            "✓".green(),
            provider_title(provider),
            identity
        );
    } else {
        println!(
            "{} No {} credentials stored for {}.",
            "ℹ".blue(),
            provider_title(provider),
            identity
        );
    }
    Ok(())
}

/// Print the store sections of `lingxia auth status` (silent when empty).
pub fn stores_status() -> Result<()> {
    let wallet = Wallet::open()?;
    let mut printed = false;
    for provider in STORE_PROVIDERS {
        let identities = wallet.store_identities(provider)?;
        if identities.is_empty() {
            continue;
        }
        if !printed {
            println!("{}", "Stores".cyan().bold());
            printed = true;
        }
        for identity in identities {
            println!("  {}  {}", provider, identity);
        }
    }
    Ok(())
}

fn prompt(label: &str) -> Result<String> {
    Input::new()
        .with_prompt(label)
        .interact_text()
        .with_context(|| format!("read {label}"))
}

fn opt_or_prompt(value: Option<String>, label: &str) -> Result<String> {
    match value {
        Some(v) => Ok(v),
        None => prompt(label),
    }
}

fn optional_nonempty(value: Option<String>) -> Option<String> {
    value.filter(|candidate| !candidate.trim().is_empty())
}

fn secret_or_prompt(value: Option<String>, label: &str) -> Result<String> {
    let v = match value {
        Some(v) => v,
        None => Password::new()
            .with_prompt(label)
            .interact()
            .with_context(|| format!("read {label}"))?,
    };
    if v.trim().is_empty() {
        bail!("{label} cannot be empty");
    }
    Ok(v)
}

fn expand(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    std::path::PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn googleplay_login_materializes_the_service_account_in_the_wallet() {
        let json = r#"{
            "client_email": "publisher@example.iam.gserviceaccount.com",
            "private_key_id": "ABC123DEF456",
            "private_key": "-----BEGIN PRIVATE KEY-----\nkey\n-----END PRIVATE KEY-----\n"
        }"#;

        let (identity, creds, fingerprint) = parse_googleplay_service_account(json).unwrap();

        assert_eq!(identity, "publisher@example.iam.gserviceaccount.com");
        assert_eq!(fingerprint.len(), 64);
        assert!(creds.service_account_json.is_none());
        assert_eq!(creds.client_email.as_deref(), Some(identity.as_str()));
        assert!(
            creds
                .private_key
                .as_deref()
                .is_some_and(|key| key.contains("BEGIN PRIVATE KEY"))
        );
    }

    #[test]
    fn googleplay_relogin_compares_the_stored_credential_digest() {
        let json = r#"{
            "client_email": "publisher@example.iam.gserviceaccount.com",
            "private_key_id": "ABC123DEF456",
            "private_key": "-----BEGIN PRIVATE KEY-----\nkey\n-----END PRIVATE KEY-----\n"
        }"#;
        let (identity, credentials, fingerprint) = parse_googleplay_service_account(json).unwrap();
        let state = tempfile::tempdir().unwrap();
        let wallet = Wallet::at(state.path());
        wallet
            .save_store_creds("googleplay", &identity, &credentials)
            .unwrap();

        assert_eq!(
            existing_fingerprint(&wallet, "googleplay", &identity)
                .unwrap()
                .as_deref(),
            Some(fingerprint.as_str())
        );
    }

    #[test]
    fn omitted_optional_seller_id_stays_absent() {
        assert_eq!(optional_nonempty(None), None);
        assert_eq!(optional_nonempty(Some("  ".to_string())), None);
        assert_eq!(
            optional_nonempty(Some("seller-1".to_string())).as_deref(),
            Some("seller-1")
        );
    }
}
