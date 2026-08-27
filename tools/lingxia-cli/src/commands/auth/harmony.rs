//! Harmony AGC authentication commands (wallet-backed).

use anyhow::{Context, Result, anyhow, bail};
use colored::Colorize;
use dialoguer::{Input, Password, Select};

use crate::platform::harmony::{AgcApiCredentials, AgcConnectClient};
use crate::wallet::{Wallet, mask};

/// Options for Harmony login command.
pub struct HarmonyLoginOptions {
    pub mode: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub yes: bool,
}

/// Execute `lingxia auth login harmony`.
///
/// Official Harmony flow uses AGC Connect API mode.
pub fn harmony_login(options: HarmonyLoginOptions) -> Result<()> {
    println!("\n{}\n", "HarmonyOS Developer Authentication".cyan().bold());

    if let Some(mode) = options.mode.as_deref()
        && !mode.eq_ignore_ascii_case("api")
    {
        return Err(anyhow!(
            "Invalid mode '{}'. Harmony only supports `--mode api`.",
            mode
        ));
    }

    login_api_mode(&options)?;
    Ok(())
}

/// In-place login used by the resolver; returns the client id.
pub fn harmony_inline_login() -> Result<String> {
    eprintln!(
        "{} Missing Harmony AGC credentials; logging in now, then the command continues.",
        "→".cyan()
    );
    login_api_mode(&HarmonyLoginOptions {
        mode: None,
        client_id: None,
        client_secret: None,
        yes: false,
    })
}

/// AGC Connect API login (api mode). Returns the client id.
fn login_api_mode(options: &HarmonyLoginOptions) -> Result<String> {
    println!("{} Using API mode", "→".dimmed());
    println!(
        "  {} Use AGC `API密钥 > Connect API > API客户端` credentials with Project set to `N/A`.",
        "ℹ".blue()
    );
    println!();

    let (client_id, client_secret) = prompt_agc_api_credentials(options)?;

    let wallet = Wallet::open()?;
    wallet.notice_legacy_files();
    if let Some(existing) = wallet.load_harmony_agc(&client_id)?
        && existing.client_secret != client_secret
    {
        println!(
            "{} Replacing the stored secret for AGC client {}: {} -> {}",
            "ℹ".blue(),
            client_id,
            mask(&existing.client_secret),
            mask(&client_secret)
        );
        if !options.yes {
            if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                bail!("refusing to rotate the AGC secret non-interactively; pass --yes to confirm");
            }
            if !dialoguer::Confirm::new()
                .with_prompt("Continue?")
                .default(true)
                .interact()?
            {
                bail!("Login cancelled.");
            }
        }
    }

    println!();
    println!("  {} Validating credentials...", "→".dimmed());

    let client = AgcConnectClient::new();
    let token = client
        .get_token(&client_id, &client_secret)
        .context("Failed to authenticate with AGC API")?;
    println!("  {} Authentication successful!", "✓".green());

    let credentials = AgcApiCredentials {
        client_id: client_id.clone(),
        client_secret,
        token: Some(token),
    };
    let path = wallet.save_harmony_agc(&credentials)?;
    println!("  {} Credentials saved to {}", "✓".green(), path.display());

    println!();
    println!(
        "{} Logged in with AGC API (Client ID: {}...)",
        "✓".green().bold(),
        &client_id[..8.min(client_id.len())]
    );

    Ok(client_id)
}

fn prompt_agc_api_credentials(options: &HarmonyLoginOptions) -> Result<(String, String)> {
    let client_id = match &options.client_id {
        Some(id) => id.clone(),
        None => Input::<String>::new()
            .with_prompt("Client ID (Connect API API client, Project=N/A)")
            .interact_text()?,
    };

    let secret = match &options.client_secret {
        Some(secret) => secret.clone(),
        None => Password::new()
            .with_prompt("Client Secret (Key)")
            .interact()?,
    };

    Ok((client_id, secret))
}

/// Execute `lingxia auth logout harmony`.
pub fn harmony_logout(client_id: Option<String>) -> Result<()> {
    let wallet = Wallet::open()?;
    let identities = wallet.harmony_identities()?;

    if identities.is_empty() {
        println!("{} No Harmony AGC credentials stored.", "ℹ".blue());
        return Ok(());
    }

    let client_id = match client_id {
        Some(id) => id,
        None if identities.len() == 1 => identities[0].clone(),
        None => {
            if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                bail!(
                    "several AGC identities are stored ({}); pass --client-id",
                    identities.join(", ")
                );
            }
            let selection = Select::new()
                .with_prompt("Log out which AGC identity?")
                .items(&identities)
                .default(0)
                .interact()?;
            identities[selection].clone()
        }
    };

    if wallet.delete_harmony_identity(&client_id)? {
        println!(
            "{} Removed AGC credentials (and signing material) for {}.",
            "✓".green(),
            client_id
        );
    } else {
        println!(
            "{} No AGC credentials stored for {}.",
            "ℹ".blue(),
            client_id
        );
    }
    Ok(())
}

/// Print the Harmony section of `lingxia auth status`.
pub fn harmony_status() -> Result<()> {
    let wallet = Wallet::open()?;
    let identities = wallet.harmony_identities()?;

    println!("{}", "Harmony".cyan().bold());
    if identities.is_empty() {
        println!(
            "  {} No credentials. Fix: lingxia auth login harmony",
            "✗".red()
        );
        return Ok(());
    }
    for client_id in identities {
        let token_state = match wallet.load_harmony_agc(&client_id)? {
            Some(creds) => match creds.token {
                Some(token) if !AgcConnectClient::is_token_expired(&token) => "token valid",
                Some(_) => "token expired (refreshed on use)",
                None => "no cached token",
            },
            None => "unreadable",
        };
        println!("  {client_id}  AGC API ({token_state})");
    }
    Ok(())
}
