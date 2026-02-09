use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use dialoguer::{Input, Password, Select};

/// Options for Harmony login command.
pub struct HarmonyLoginOptions {
    pub mode: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub yes: bool,
}

/// Execute Harmony login command.
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

    harmony_login_api_mode(&options)
}

/// AGC Connect API login (api mode) - for CI/CD
fn harmony_login_api_mode(options: &HarmonyLoginOptions) -> Result<()> {
    use crate::platform::harmony::HarmonyAuthService;

    println!("{} Using API mode", "→".dimmed());
    println!(
        "  {} Use AGC `API密钥 > Connect API > API客户端` credentials with Project set to `N/A`.",
        "ℹ".blue()
    );
    println!();

    let auth_service = HarmonyAuthService::new()?;

    // Check for existing credentials
    if let Some(existing) = auth_service.load_credentials()?
        && !confirm_replace_harmony_agc_credentials(&existing.client_id, options.yes)?
    {
        println!("Login cancelled.");
        return Ok(());
    }

    let (client_id, client_secret) = prompt_harmony_agc_api_credentials(options)?;

    println!();
    println!("  {} Validating credentials...", "→".dimmed());

    let credentials = auth_service
        .authenticate(&client_id, &client_secret)
        .context("Failed to authenticate with AGC API")?;

    println!("  {} Authentication successful!", "✓".green());

    auth_service.save_credentials(&credentials)?;
    println!(
        "  {} Credentials saved to {}",
        "✓".green(),
        auth_service.storage_path().display()
    );

    println!();
    println!(
        "{} Logged in with AGC API (Client ID: {}...)",
        "✓".green().bold(),
        &client_id[..8.min(client_id.len())]
    );

    Ok(())
}

fn confirm_replace_harmony_agc_credentials(existing_client_id: &str, yes: bool) -> Result<bool> {
    println!(
        "{} Existing AGC API credentials found (Client ID: {}...)",
        "ℹ".blue(),
        &existing_client_id[..8.min(existing_client_id.len())]
    );

    if yes {
        return Ok(true);
    }

    let choices = vec!["Replace existing credentials", "Cancel"];
    let selection = Select::new()
        .with_prompt("What would you like to do?")
        .items(&choices)
        .default(1)
        .interact()?;

    Ok(selection == 0)
}

fn prompt_harmony_agc_api_credentials(options: &HarmonyLoginOptions) -> Result<(String, String)> {
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

/// Execute Harmony logout command.
pub fn harmony_logout() -> Result<()> {
    use crate::platform::harmony::HarmonyAuthService;

    let auth_service = HarmonyAuthService::new()?;
    let deleted_any = auth_service.clear_credentials()?;

    if deleted_any {
        println!(
            "{} API mode credentials removed from: {}",
            "✓".green(),
            auth_service.storage_path().display()
        );
    }

    if deleted_any {
        println!();
        println!("{} Successfully logged out.", "✓".green());
    } else {
        println!("{} Not currently logged in.", "ℹ".blue());
    }

    Ok(())
}

/// Execute Harmony status command.
pub fn harmony_status() -> Result<()> {
    use crate::platform::harmony::HarmonyAuthService;

    let auth_service = HarmonyAuthService::new()?;
    let status = auth_service.status()?;

    println!(
        "{}",
        "HarmonyOS Developer Authentication Status".cyan().bold()
    );
    println!();

    if status.is_none() {
        println!("{} Not logged in", "✗".red());
        println!();
        println!("Run 'lingxia auth harmony login --mode api' to authenticate.");
        return Ok(());
    }

    if let Some(status) = status {
        println!(
            "{} API (client credentials): {}",
            "•".cyan(),
            status.token_state.as_str()
        );
        println!(
            "  Client ID:  {}...",
            &status.client_id[..8.min(status.client_id.len())]
        );
        println!("  Storage:    {}", status.storage_path.display());
    }

    Ok(())
}
