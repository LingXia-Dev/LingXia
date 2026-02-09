//! Apple authentication commands.

use crate::path_completion::FilePathCompleter;
use crate::platform::apple::anisette::OmnisetteProvider;
use crate::platform::apple::auth::{AuthCredentials, CredentialStorage};
use crate::platform::apple::developer_services;
use crate::platform::apple::grandslam::{
    DeviceInfo, GrandSlamClient, GrandSlamLoginData, TwoFactorMode, TwoFactorRequired,
};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use dialoguer::{Input, Password, Select};
use std::path::PathBuf;

pub struct AppleLoginOptions {
    pub username: Option<String>,
    pub password: Option<String>,
    pub mode: Option<String>,
    pub key_id: Option<String>,
    pub issuer_id: Option<String>,
    pub private_key_path: Option<String>,
    pub team_id: Option<String>,
    pub yes: bool,
}

#[derive(Default)]
struct ApiKeyLoginArgs {
    key_id: Option<String>,
    issuer_id: Option<String>,
    private_key_path: Option<PathBuf>,
    team_id: Option<String>,
}

impl ApiKeyLoginArgs {
    fn has_any(&self) -> bool {
        self.key_id.is_some()
            || self.issuer_id.is_some()
            || self.private_key_path.is_some()
            || self.team_id.is_some()
    }
}

/// Execute Apple login command.
pub fn apple_login(options: AppleLoginOptions) -> Result<()> {
    let AppleLoginOptions {
        username,
        password,
        mode,
        key_id,
        issuer_id,
        private_key_path,
        team_id,
        yes,
    } = options;

    println!("\n{}\n", "Apple Developer Authentication".cyan().bold());

    let key_args = ApiKeyLoginArgs {
        key_id,
        issuer_id,
        private_key_path: private_key_path.as_deref().map(expand_path),
        team_id,
    };
    let mode = resolve_login_mode(
        mode,
        username.as_deref(),
        password.as_deref(),
        key_args.has_any(),
    )?;

    // Check for existing credentials
    let storage = CredentialStorage::new()?;
    if let Some(existing) = storage.load()? {
        println!(
            "{} Already logged in as {} (Team: {})",
            "ℹ".blue(),
            existing.credential_type(),
            existing.team_id()
        );

        if yes {
            println!("{} Replacing existing credentials.", "ℹ".blue());
        } else {
            let choices = vec!["Replace existing credentials", "Cancel"];
            let selection = Select::new()
                .with_prompt("What would you like to do?")
                .items(&choices)
                .default(1)
                .interact()?;

            if selection == 1 {
                println!("Login cancelled.");
                return Ok(());
            }
        }
    }

    match mode.as_str() {
        "key" => login_with_api_key(&storage, key_args)?,
        _ => login_with_password(&storage, username, password)?,
    }

    Ok(())
}

/// Interactively select login mode
fn select_login_mode() -> Result<String> {
    let modes = vec![
        "API Key     (requires paid Apple Developer Program membership)",
        "Password    (works with any Apple ID but uses private APIs)",
    ];

    let selection = Select::new()
        .with_prompt("Select login mode")
        .items(&modes)
        .default(1)
        .interact()?;

    match selection {
        0 => Ok("key".to_string()),
        _ => Ok("password".to_string()),
    }
}

fn resolve_login_mode(
    mode: Option<String>,
    username: Option<&str>,
    password: Option<&str>,
    has_key_args: bool,
) -> Result<String> {
    if let Some(mode) = mode {
        let normalized = mode.trim().to_ascii_lowercase();
        if normalized != "key" && normalized != "password" {
            return Err(anyhow!(
                "Invalid mode '{}'. Expected one of: key, password",
                mode
            ));
        }

        if normalized == "password" && has_key_args {
            return Err(anyhow!(
                "API key parameters (--key-id/--issuer-id/--private-key-path/--team-id) are only valid with --mode key."
            ));
        }
        if normalized == "key" && (username.is_some() || password.is_some()) {
            return Err(anyhow!(
                "--username/--password are only valid with --mode password."
            ));
        }
        return Ok(normalized);
    }

    if has_key_args {
        if username.is_some() || password.is_some() {
            return Err(anyhow!(
                "Cannot infer mode: both password and API key parameters were provided. Please specify --mode key or --mode password."
            ));
        }
        return Ok("key".to_string());
    }

    if username.is_some() || password.is_some() {
        return Ok("password".to_string());
    }

    select_login_mode()
}

/// Login with Apple ID (password mode)
fn login_with_password(
    storage: &CredentialStorage,
    username: Option<String>,
    password: Option<String>,
) -> Result<()> {
    println!("{}", "Apple ID Authentication".bold());
    println!();

    // Get username (Apple ID)
    let username = if let Some(u) = username {
        u
    } else {
        Input::new()
            .with_prompt("Apple ID (email)")
            .interact_text()?
    };

    // Validate email format (basic check)
    if !username.contains('@') {
        return Err(anyhow!(
            "Invalid Apple ID format. Please enter your email address."
        ));
    }

    // Get password
    let password = if let Some(p) = password {
        p
    } else {
        Password::new().with_prompt("Password").interact()?
    };

    println!();
    println!("⏳ Authenticating...");

    // Step 1: Get Anisette data
    println!("  {} Getting device fingerprint...", "→".dimmed());
    let mut anisette_provider = OmnisetteProvider::new();
    let anisette_data = anisette_provider
        .fetch_anisette_data()
        .context("Failed to get Anisette data")?;
    println!("  {} Anisette data obtained", "✓".green());

    // Step 2: Create device info
    let device_info = DeviceInfo::default_macos();

    // Step 3: Authenticate with GrandSlam
    println!("  {} Authenticating with Apple...", "→".dimmed());
    let mut client = GrandSlamClient::new();

    let login_data = match client.authenticate(&username, &password, &device_info, &anisette_data) {
        Ok(data) => {
            println!("  {} Authentication successful", "✓".green());
            data
        }
        Err(e) => {
            // Check if this is a 2FA required error
            if let Some(tfa) = e.downcast_ref::<TwoFactorRequired>() {
                println!("  {} Two-factor authentication required", "!".yellow());

                // Handle based on the 2FA mode
                match tfa.mode {
                    TwoFactorMode::Auto => {
                        // 2FA was automatically triggered
                        println!();
                        println!("A verification code should appear on your trusted Apple device.");
                    }
                    TwoFactorMode::TrustedDevice => {
                        // Need to request trusted device push with fresh anisette data
                        println!(
                            "  {} Requesting verification from your devices...",
                            "→".dimmed()
                        );
                        let fresh_anisette = anisette_provider
                            .fetch_anisette_data()
                            .context("Failed to get fresh anisette data")?;
                        client
                            .request_trusted_device_push(tfa, &device_info, &fresh_anisette)
                            .context("Failed to request trusted device verification")?;
                        println!("  {} Request sent!", "✓".green());
                        println!();
                        println!("Check your iPhone/iPad for a notification.");
                        println!("Tap 'Allow' to see the verification code.");
                    }
                    TwoFactorMode::Sms => {
                        // TODO: Implement SMS request
                        println!();
                        println!("SMS verification is not yet implemented.");
                        println!("Please check your trusted device for a code.");
                    }
                }
                println!("Enter the 6-digit code below:");

                // Prompt for 2FA code
                let code: String = Input::new()
                    .with_prompt("Verification code")
                    .validate_with(|input: &String| -> Result<(), &str> {
                        if input.len() == 6 && input.chars().all(|c| c.is_ascii_digit()) {
                            Ok(())
                        } else {
                            Err("Please enter a 6-digit code")
                        }
                    })
                    .interact_text()?;

                println!();
                println!("  {} Validating code...", "→".dimmed());

                // Validate the 2FA code
                client
                    .validate_2fa(&code, tfa, &device_info, &mut anisette_provider)
                    .context("2FA validation failed")?;

                println!("  {} Code validated", "✓".green());

                // Re-authenticate to get full tokens (need fresh anisette data)
                println!("  {} Completing authentication...", "→".dimmed());
                let fresh_anisette = anisette_provider
                    .fetch_anisette_data()
                    .context("Failed to refresh anisette data for re-authentication")?;
                let result =
                    client.authenticate(&username, &password, &device_info, &fresh_anisette)?;
                println!("  {} Authentication successful", "✓".green());
                result
            } else {
                return Err(e);
            }
        }
    };

    // Fetch app tokens and teams
    let (team_id, app_token) =
        select_team(&client, &login_data, &device_info, &mut anisette_provider)?;

    // Save credentials
    let credentials = AuthCredentials::AppleId {
        adsid: login_data.adsid.clone(),
        token: login_data.idms_token.clone(),
        app_token,
        team_id: team_id.clone(),
        expiry: chrono::Utc::now() + chrono::Duration::hours(24),
    };

    storage.save(&credentials)?;

    println!();
    println!("{} Successfully logged in!", "✓".green());
    println!("  Apple ID: {}", username);
    println!("  Team ID:  {}", team_id);
    println!("  Credentials saved to: {}", storage.path().display());

    Ok(())
}

/// Login with App Store Connect API Key
fn login_with_api_key(storage: &CredentialStorage, args: ApiKeyLoginArgs) -> Result<()> {
    println!("{}", "App Store Connect API Key Authentication".bold());
    let needs_prompt = args.key_id.is_none()
        || args.issuer_id.is_none()
        || args.private_key_path.is_none()
        || args.team_id.is_none();

    if needs_prompt {
        println!();
        println!("To create an API key:");
        println!("  1. Open https://appstoreconnect.apple.com/");
        println!("  2. Go to Users and Access -> Integrations -> App Store Connect API");
        println!("  3. Click '+' to create a new key");
        println!("  4. Give it a name and select 'App Manager' access (or higher, e.g. Admin)");
        println!("  5. Download the .p8 file (you can only download it once!)");
        println!();
    }

    let key_id: String = if let Some(value) = args.key_id {
        value
    } else {
        Input::new()
            .with_prompt("API Key ID (e.g., ABC123DEF4)")
            .interact_text()?
    };
    let issuer_id: String = if let Some(value) = args.issuer_id {
        value
    } else {
        Input::new()
            .with_prompt("Issuer ID (UUID from API Keys page)")
            .interact_text()?
    };
    let private_key_path = if let Some(path) = args.private_key_path {
        path
    } else {
        let key_path: String = Input::new()
            .with_prompt("Path to .p8 private key file")
            .completion_with(&FilePathCompleter::new())
            .interact_text()?;
        expand_path(&key_path)
    };

    if args.team_id.is_none() {
        println!();
        println!("Your Team ID can be found at:");
        println!("  https://developer.apple.com/account -> Membership Details");
        println!();
    }

    let team_id: String = if let Some(value) = args.team_id {
        value
    } else {
        Input::new()
            .with_prompt("Team ID (e.g., ABCDE12345)")
            .interact_text()?
    };

    let private_key_pem =
        validate_api_key_credentials(&key_id, &issuer_id, &private_key_path, &team_id)?;

    // Save credentials
    let credentials = AuthCredentials::AppStoreConnect {
        key_id: key_id.clone(),
        issuer_id: issuer_id.clone(),
        private_key_pem,
        team_id: team_id.clone(),
        cached_signing_identity: None,
    };

    storage.save(&credentials)?;

    println!();
    println!("{} Successfully logged in!", "✓".green());
    println!("  Key ID: {}", key_id);
    println!("  Team ID: {}", team_id);
    println!("  Credentials saved to: {}", storage.path().display());

    Ok(())
}

fn validate_api_key_credentials(
    key_id: &str,
    issuer_id: &str,
    private_key_path: &std::path::Path,
    team_id: &str,
) -> Result<String> {
    if key_id.len() != 10 {
        return Err(anyhow!(
            "Invalid Key ID format. It should be 10 characters."
        ));
    }

    if !issuer_id.contains('-') || issuer_id.len() != 36 {
        return Err(anyhow!(
            "Invalid Issuer ID format. It should be a UUID (e.g., 12345678-1234-1234-1234-123456789012)."
        ));
    }

    if !private_key_path.exists() {
        return Err(anyhow!(
            "Private key file not found: {}",
            private_key_path.display()
        ));
    }

    let key_content = std::fs::read_to_string(private_key_path)
        .with_context(|| format!("Failed to read key file: {}", private_key_path.display()))?;
    if !key_content.contains("BEGIN PRIVATE KEY") {
        return Err(anyhow!(
            "Invalid private key file. Expected a PKCS#8 format .p8 file."
        ));
    }

    if team_id.len() != 10 {
        return Err(anyhow!(
            "Invalid Team ID format. It should be 10 characters."
        ));
    }

    Ok(key_content)
}

/// Execute Apple logout command.
pub fn apple_logout() -> Result<()> {
    let storage = CredentialStorage::new()?;

    let mut deleted_anything = false;

    // Delete credentials
    if storage.delete()? {
        println!(
            "{} Credentials removed from: {}",
            "✓".green(),
            storage.path().display()
        );
        deleted_anything = true;
    }

    // Also clear anisette cache to ensure fresh device fingerprint on next login
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
    let anisette_cache = home.join(".lingxia").join("anisette_cache.json");
    if anisette_cache.exists() {
        std::fs::remove_file(&anisette_cache)?;
        println!(
            "{} Anisette cache cleared: {}",
            "✓".green(),
            anisette_cache.display()
        );
        deleted_anything = true;
    }

    if deleted_anything {
        println!();
        println!("{} Successfully logged out.", "✓".green());
    } else {
        println!("{} Not currently logged in.", "ℹ".blue());
    }

    Ok(())
}

/// Execute Apple status command.
pub fn apple_status() -> Result<()> {
    let storage = CredentialStorage::new()?;

    match storage.load()? {
        Some(credentials) => {
            println!("{}", "Apple Developer Authentication Status".cyan().bold());
            println!();
            println!("{} Logged in", "✓".green());
            println!();

            match &credentials {
                AuthCredentials::AppStoreConnect {
                    key_id,
                    issuer_id,
                    team_id,
                    ..
                } => {
                    println!("  Type:      App Store Connect API Key");
                    println!("  Key ID:    {}", key_id);
                    println!("  Issuer ID: {}", issuer_id);
                    println!("  Team ID:   {}", team_id);
                }
                AuthCredentials::AppleId {
                    adsid,
                    team_id,
                    expiry,
                    ..
                } => {
                    println!("  Type:    Apple ID");
                    println!("  ADSID:   {}", adsid);
                    println!("  Team ID: {}", team_id);
                    println!("  Expires: {}", expiry);

                    if credentials.is_expired() {
                        println!();
                        println!(
                            "{} Token has expired. Run 'lingxia auth apple login' to refresh.",
                            "⚠".yellow()
                        );
                    }
                }
            }

            println!();
            println!("  Credentials stored at: {}", storage.path().display());
        }
        None => {
            println!("{}", "Apple Developer Authentication Status".cyan().bold());
            println!();
            println!("{} Not logged in", "✗".red());
            println!();
            println!("Run 'lingxia auth apple login' to authenticate.");
        }
    }

    Ok(())
}

/// Fetch developer teams and let the user pick one.
///
/// If there is exactly one team, it is selected automatically.
/// Returns (team_id, app_token) tuple.
fn select_team(
    client: &GrandSlamClient,
    login_data: &GrandSlamLoginData,
    device_info: &DeviceInfo,
    anisette_provider: &mut OmnisetteProvider,
) -> Result<(String, String)> {
    println!();
    println!("  {} Fetching app tokens...", "→".dimmed());

    let anisette = anisette_provider
        .fetch_anisette_data()
        .context("Failed to get anisette data for app token fetch")?;

    let app_token = client
        .fetch_app_tokens(login_data, device_info, &anisette)
        .context("Failed to fetch app tokens")?;
    println!("  {} App tokens obtained", "✓".green());

    println!("  {} Fetching developer teams...", "→".dimmed());

    let anisette = anisette_provider
        .fetch_anisette_data()
        .context("Failed to get anisette data for team listing")?;

    let teams =
        developer_services::list_teams(&login_data.adsid, &app_token, device_info, &anisette)?;

    if teams.is_empty() {
        return Err(anyhow!("No developer teams found for this Apple ID."));
    }

    if teams.len() == 1 {
        let team = &teams[0];
        println!(
            "  {} Team: {} ({}) [{}]",
            "✓".green(),
            team.name,
            team.id,
            team.account_type()
        );
        return Ok((team.id.clone(), app_token));
    }

    // Multiple teams — let the user choose
    println!("  {} Found {} teams", "✓".green(), teams.len());
    println!();

    let labels: Vec<String> = teams
        .iter()
        .map(|t| format!("{} ({}) [{}]", t.name, t.id, t.account_type()))
        .collect();

    let selection = Select::new()
        .with_prompt("Select a team")
        .items(&labels)
        .default(0)
        .interact()?;

    let team = &teams[selection];
    Ok((team.id.clone(), app_token))
}

/// Expand ~ in path to home directory
fn expand_path(path: &str) -> PathBuf {
    if let Some(suffix) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(suffix);
    }
    PathBuf::from(path)
}
