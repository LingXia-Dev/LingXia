//! Apple authentication commands (wallet-backed).
//!
//! `lingxia auth login apple` stores credentials per Apple Team in the
//! wallet; mechanisms (ASC key, Apple ID session, Developer ID certificate,
//! App-specific password) co-exist per team. Logging into the same team +
//! mechanism again is a key rotation: the fingerprint change is shown and
//! confirmed.

use crate::path_completion::FilePathCompleter;
use crate::platform::apple::altool;
use crate::platform::apple::anisette::OmnisetteProvider;
use crate::platform::apple::auth::{
    AppPasswordCredentials, AuthCredentials, DeveloperIdCredentials,
};
use crate::platform::apple::developer_services;
use crate::platform::apple::grandslam::{
    DeviceInfo, GrandSlamClient, GrandSlamLoginData, TwoFactorMode, TwoFactorRequired,
};
use crate::resolver::{self, AppleNeed};
use crate::wallet::{Wallet, credential_fingerprint, display_fingerprint};
use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use colored::Colorize;
use dialoguer::{Confirm, Input, Password, Select};
use std::path::PathBuf;

pub struct AppleLoginOptions {
    pub username: Option<String>,
    pub password: Option<String>,
    pub mode: Option<String>,
    pub key_id: Option<String>,
    pub issuer_id: Option<String>,
    pub private_key_path: Option<String>,
    pub team_id: Option<String>,
    pub p12: Option<String>,
    pub p12_password: Option<String>,
    pub identity: Option<String>,
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
        self.key_id.is_some() || self.issuer_id.is_some() || self.private_key_path.is_some()
    }
}

/// Execute `lingxia auth login apple`.
pub fn apple_login(options: AppleLoginOptions) -> Result<()> {
    let AppleLoginOptions {
        username,
        password,
        mode,
        key_id,
        issuer_id,
        private_key_path,
        team_id,
        p12,
        p12_password,
        identity,
        yes,
    } = options;

    println!("\n{}\n", "Apple Developer Authentication".cyan().bold());

    let key_args = ApiKeyLoginArgs {
        key_id,
        issuer_id,
        private_key_path: private_key_path.as_deref().map(expand_path),
        team_id: team_id.clone(),
    };
    let mode = resolve_login_mode(
        mode,
        username.as_deref(),
        password.as_deref(),
        key_args.has_any(),
        p12.is_some() || p12_password.is_some(),
    )?;

    let wallet = Wallet::open()?;
    wallet.notice_legacy_files();
    match mode.as_str() {
        "key" => {
            login_with_api_key(&wallet, key_args, yes)?;
        }
        "developer-id" => {
            login_developer_id(&wallet, team_id, p12, p12_password, identity, yes)?;
        }
        "app-password" => {
            login_app_password(&wallet, team_id, username, password, yes)?;
        }
        _ => {
            login_with_password(&wallet, username, password, yes)?;
        }
    }

    Ok(())
}

/// In-place login used by the resolver when a command finds credentials
/// missing: runs the right mechanism, then the original command continues.
/// Returns the team that was logged in.
pub fn inline_login(required_team: Option<&str>, need: AppleNeed) -> Result<String> {
    resolver::announce_inline_login(need, required_team);
    let wallet = Wallet::open()?;

    let team = match need {
        AppleNeed::Asc => login_with_api_key(
            &wallet,
            ApiKeyLoginArgs {
                team_id: required_team.map(str::to_string),
                ..Default::default()
            },
            false,
        )?,
        AppleNeed::DeveloperId => login_developer_id(
            &wallet,
            required_team.map(str::to_string),
            None,
            None,
            None,
            false,
        )?,
        AppleNeed::Auth => match select_login_mode()?.as_str() {
            "key" => login_with_api_key(
                &wallet,
                ApiKeyLoginArgs {
                    team_id: required_team.map(str::to_string),
                    ..Default::default()
                },
                false,
            )?,
            _ => login_with_password(&wallet, None, None, false)?,
        },
        AppleNeed::Upload => match select_upload_login_mode()?.as_str() {
            "key" => login_with_api_key(
                &wallet,
                ApiKeyLoginArgs {
                    team_id: required_team.map(str::to_string),
                    ..Default::default()
                },
                false,
            )?,
            _ => login_app_password(
                &wallet,
                required_team.map(str::to_string),
                None,
                None,
                false,
            )?,
        },
    };

    if let Some(required) = required_team
        && team != required
    {
        bail!(
            "{}: logged in team {team}, but this project requires team {required} (lingxia.yaml)",
            resolver::codes::CREDENTIAL_IDENTITY_MISMATCH
        );
    }
    Ok(team)
}

/// Interactively select login mode
fn select_login_mode() -> Result<String> {
    let modes = vec![
        "API Key        (requires paid Apple Developer Program membership)",
        "Password       (works with any Apple ID but uses private APIs)",
        "Developer ID   (.p12 certificate for macOS distribution/notarization)",
        "App password   (Apple ID + App-specific password, for App Store uploads)",
    ];

    let selection = Select::new()
        .with_prompt("Select login mode")
        .items(&modes)
        .default(0)
        .interact()?;

    Ok(match selection {
        0 => "key".to_string(),
        1 => "password".to_string(),
        2 => "developer-id".to_string(),
        _ => "app-password".to_string(),
    })
}

/// Login modes that can upload to the App Store.
fn select_upload_login_mode() -> Result<String> {
    let modes = vec![
        "API Key        (requires paid Apple Developer Program membership)",
        "App password   (Apple ID + App-specific password)",
    ];
    let selection = Select::new()
        .with_prompt("Select login mode")
        .items(&modes)
        .default(0)
        .interact()?;
    Ok(if selection == 0 {
        "key"
    } else {
        "app-password"
    }
    .to_string())
}

fn resolve_login_mode(
    mode: Option<String>,
    username: Option<&str>,
    password: Option<&str>,
    has_key_args: bool,
    has_p12_args: bool,
) -> Result<String> {
    if let Some(mode) = mode {
        let normalized = mode.trim().to_ascii_lowercase();
        if !matches!(
            normalized.as_str(),
            "key" | "password" | "developer-id" | "app-password"
        ) {
            return Err(anyhow!(
                "Invalid mode '{}'. Expected one of: key, password, developer-id, app-password",
                mode
            ));
        }

        if normalized != "key" && has_key_args {
            return Err(anyhow!(
                "API key parameters (--key-id/--issuer-id/--private-key-path) are only valid with --mode key."
            ));
        }
        if !matches!(normalized.as_str(), "password" | "app-password")
            && (username.is_some() || password.is_some())
        {
            return Err(anyhow!(
                "--username/--password are only valid with --mode password or app-password."
            ));
        }
        if normalized != "developer-id" && has_p12_args {
            return Err(anyhow!(
                "--p12/--p12-password are only valid with --mode developer-id."
            ));
        }
        return Ok(normalized);
    }

    match (
        has_key_args,
        username.is_some() || password.is_some(),
        has_p12_args,
    ) {
        (true, false, false) => Ok("key".to_string()),
        (false, true, false) => Ok("password".to_string()),
        (false, false, true) => Ok("developer-id".to_string()),
        (false, false, false) => select_login_mode(),
        _ => Err(anyhow!(
            "Cannot infer mode from the given flags. Please specify --mode key|password|developer-id|app-password."
        )),
    }
}

/// Show the fingerprint change and confirm before replacing an existing slot.
fn confirm_rotation<T: serde::Serialize>(old: &T, new: &T, what: &str, yes: bool) -> Result<()> {
    let old_fingerprint = credential_fingerprint(old)?;
    let new_fingerprint = credential_fingerprint(new)?;
    if old_fingerprint == new_fingerprint {
        return Ok(());
    }
    println!(
        "{} Replacing the stored {what}: {} -> {}",
        "ℹ".blue(),
        display_fingerprint(&old_fingerprint),
        display_fingerprint(&new_fingerprint)
    );
    if yes {
        return Ok(());
    }
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        bail!("refusing to rotate {what} non-interactively; pass --yes to confirm");
    }
    if !Confirm::new()
        .with_prompt("Continue?")
        .default(true)
        .interact()?
    {
        bail!("Login cancelled.");
    }
    Ok(())
}

/// Login with Apple ID (password mode). Returns the team id.
fn login_with_password(
    wallet: &Wallet,
    username: Option<String>,
    password: Option<String>,
    yes: bool,
) -> Result<String> {
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

    // Step 2: Advertise the same machine the anisette OTP was issued for.
    let device_info = DeviceInfo::for_anisette(&anisette_data);

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
    let (team, app_token) =
        select_team(&client, &login_data, &device_info, &mut anisette_provider)?;
    let team_id = team.id.clone();

    let credentials = AuthCredentials::AppleId {
        adsid: login_data.adsid.clone(),
        token: login_data.idms_token.clone(),
        app_token,
        team_id: team_id.clone(),
        expiry: chrono::Utc::now() + chrono::Duration::hours(24),
    };

    if let Some(old) = wallet.load_apple_id(&team_id)? {
        confirm_rotation(&old, &credentials, "Apple ID session", yes)?;
    }

    let path = wallet.save_apple_auth(&credentials)?;
    wallet.save_apple_team_info(
        &team_id,
        &crate::wallet::AppleTeamInfo {
            name: Some(team.name.clone()),
            account_type: Some(team.account_type().to_string()),
            apple_id: Some(username.clone()),
        },
    )?;

    println!();
    println!("{} Successfully logged in!", "✓".green());
    println!("  Apple ID: {}", username);
    println!("  Team:     {} ({})", team.name, team_id);
    println!("  Credentials saved to: {}", path.display());

    Ok(team_id)
}

/// Login with App Store Connect API Key. Returns the team id.
fn login_with_api_key(wallet: &Wallet, args: ApiKeyLoginArgs, yes: bool) -> Result<String> {
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

    let credentials = AuthCredentials::AppStoreConnect {
        key_id: key_id.clone(),
        issuer_id: issuer_id.clone(),
        private_key_pem,
        team_id: team_id.clone(),
        cached_signing_identity: None,
        cached_distribution_identity: None,
    };

    if let Some(AuthCredentials::AppStoreConnect {
        key_id: old_key_id,
        issuer_id: old_issuer_id,
        private_key_pem: old_private_key_pem,
        ..
    }) = wallet.load_apple_asc(&team_id)?
    {
        let AuthCredentials::AppStoreConnect {
            private_key_pem: new_private_key_pem,
            ..
        } = &credentials
        else {
            unreachable!()
        };
        confirm_rotation(
            &(&old_key_id, &old_issuer_id, &old_private_key_pem),
            &(&key_id, &issuer_id, new_private_key_pem),
            "ASC key",
            yes,
        )?;
    }

    let path = wallet.save_apple_auth(&credentials)?;

    println!();
    println!("{} Successfully logged in!", "✓".green());
    println!("  Key ID: {}", key_id);
    println!("  Team ID: {}", team_id);
    println!("  Credentials saved to: {}", path.display());

    Ok(team_id)
}

/// Store a Developer ID Application `.p12` certificate for one team
/// (macOS distribution/notarization). Returns the team id.
fn login_developer_id(
    wallet: &Wallet,
    team_id: Option<String>,
    p12: Option<String>,
    p12_password: Option<String>,
    identity: Option<String>,
    yes: bool,
) -> Result<String> {
    println!("{}", "Developer ID Certificate".bold());
    println!();

    let team_id: String = if let Some(value) = team_id {
        value
    } else {
        Input::new()
            .with_prompt("Team ID (e.g., ABCDE12345)")
            .interact_text()?
    };

    let p12_path = if let Some(p) = p12 {
        expand_path(&p)
    } else {
        let p: String = Input::new()
            .with_prompt("Path to Developer ID Application .p12 file")
            .completion_with(&FilePathCompleter::new())
            .interact_text()?;
        expand_path(&p)
    };
    if !p12_path.exists() {
        return Err(anyhow!(".p12 file not found: {}", p12_path.display()));
    }

    let bytes = std::fs::read(&p12_path)
        .with_context(|| format!("Failed to read {}", p12_path.display()))?;
    let password = if let Some(p) = p12_password {
        p
    } else {
        Password::new()
            .with_prompt("Certificate (.p12) password")
            .interact()?
    };

    let certificate_identity = verify_developer_id_team(&bytes, &password, &team_id)?;
    let identity = identity.filter(|value| !value.trim().is_empty());
    if let Some(requested) = &identity
        && requested != &certificate_identity
    {
        bail!(
            "Developer ID identity mismatch: --identity is `{requested}`, but the certificate is `{certificate_identity}`"
        );
    }

    let credentials = DeveloperIdCredentials {
        p12_base64: STANDARD.encode(&bytes),
        password,
        identity: Some(certificate_identity),
    };

    if let Some(old) = wallet.load_apple_developer_id(&team_id)? {
        confirm_rotation(&old, &credentials, "Developer ID certificate", yes)?;
    }

    let path = wallet.save_apple_developer_id(&team_id, &credentials)?;

    println!();
    println!("{} Developer ID certificate imported.", "✓".green());
    println!("  Team ID:  {}", team_id);
    println!("  Saved to: {}", path.display());
    println!(
        "  {} This must be a 'Developer ID Application' certificate (for macOS distribution/notarization).",
        "ℹ".blue()
    );

    Ok(team_id)
}

/// Apple's `xxxx-xxxx-xxxx-xxxx` form of a typed or pasted App-specific
/// password: dashes, spaces, and control bytes dropped, then the dashes put
/// back. `None` when it is not 16 letters.
fn normalize_app_password(input: &str) -> Option<String> {
    let letters: Vec<char> = input
        .chars()
        .filter(|c| *c != '-' && !c.is_whitespace() && !c.is_control())
        .collect();
    if letters.len() != 16 || !letters.iter().all(char::is_ascii_alphabetic) {
        return None;
    }
    Some(
        letters
            .chunks(4)
            .map(|chunk| chunk.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("-"),
    )
}

/// Store an Apple ID with an App-specific password for App Store uploads.
/// altool checks the pair and lists the teams it can upload for.
fn login_app_password(
    wallet: &Wallet,
    team_id: Option<String>,
    username: Option<String>,
    password: Option<String>,
    yes: bool,
) -> Result<String> {
    println!("{}", "App-specific Password".bold());
    println!(
        "  {} Create one at account.apple.com → Sign-In and Security → App-Specific Passwords.",
        "ℹ".blue()
    );
    println!();

    // The Apple ID a session login already recorded for this team.
    let known_apple_id = match &team_id {
        Some(team) => wallet
            .apple_team(team)?
            .and_then(|slots| slots.info.apple_id),
        None => None,
    };
    let apple_id = match username {
        Some(value) => value,
        None => {
            let mut input = Input::<String>::new().with_prompt("Apple ID");
            if let Some(known) = known_apple_id {
                input = input.default(known);
            }
            input.interact_text()?
        }
    };
    let password = match password {
        Some(value) => value,
        None => Password::new()
            .with_prompt("App-specific password")
            .interact()?,
    };
    let Some(password) = normalize_app_password(&password) else {
        bail!(
            "that is not an App-specific password: expected 16 letters \
             (`xxxx-xxxx-xxxx-xxxx`). The Apple ID password does not work here."
        );
    };
    let credentials = AppPasswordCredentials {
        apple_id: apple_id.trim().to_string(),
        password,
    };

    println!("  {} Checking with altool...", "→".dimmed());
    let providers = altool::list_providers(&credentials)?;
    let provider = match &team_id {
        Some(team) => providers
            .iter()
            .find(|p| &p.team_id == team)
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "{} cannot upload for team {team} (it can for: {})",
                    credentials.apple_id,
                    providers
                        .iter()
                        .map(|p| p.team_id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?,
        None => match providers.as_slice() {
            [] => bail!("{} can upload for no team", credentials.apple_id),
            [only] => only.clone(),
            many => {
                let labels: Vec<String> = many
                    .iter()
                    .map(|p| match &p.name {
                        Some(name) => format!("{} ({name})", p.team_id),
                        None => p.team_id.clone(),
                    })
                    .collect();
                let selection = Select::new()
                    .with_prompt("Select a team")
                    .items(&labels)
                    .default(0)
                    .interact()?;
                many[selection].clone()
            }
        },
    };
    let team_id = provider.team_id;

    if let Some(old) = wallet.load_apple_app_password(&team_id)? {
        confirm_rotation(&old, &credentials, "App-specific password", yes)?;
    }
    let path = wallet.save_apple_app_password(&team_id, &credentials)?;
    wallet.save_apple_team_info(
        &team_id,
        &crate::wallet::AppleTeamInfo {
            name: provider.name,
            account_type: None,
            apple_id: Some(credentials.apple_id.clone()),
        },
    )?;

    println!();
    println!("{} App-specific password saved.", "✓".green());
    println!("  Team ID:  {}", team_id);
    println!("  Saved to: {}", path.display());
    Ok(team_id)
}

fn verify_developer_id_team(bytes: &[u8], password: &str, expected_team: &str) -> Result<String> {
    let certificates = p12_certificates(bytes, password)?;
    for certificate in certificates {
        if let Some(common_name) = validate_developer_id_certificate(&certificate, expected_team)? {
            return Ok(common_name);
        }
    }
    bail!("the PKCS#12 archive has no `Developer ID Application` certificate")
}

fn validate_developer_id_certificate(
    cert_der: &[u8],
    expected_team: &str,
) -> Result<Option<String>> {
    let Some((common_name, team_id)) = developer_id_certificate_identity(cert_der)? else {
        return Ok(None);
    };
    if team_id != expected_team {
        bail!(
            "{}: Developer ID certificate belongs to Apple team {team_id}, not {expected_team}",
            resolver::codes::CREDENTIAL_IDENTITY_MISMATCH
        );
    }
    Ok(Some(common_name))
}

fn developer_id_certificate_identity(cert_der: &[u8]) -> Result<Option<(String, String)>> {
    use x509_parser::prelude::*;

    let (_, certificate) = X509Certificate::from_der(cert_der)
        .map_err(|error| anyhow!("Failed to parse certificate in PKCS#12 archive: {error:?}"))?;
    let common_name = certificate
        .subject()
        .iter_common_name()
        .next()
        .and_then(|value| value.as_str().ok());
    let Some(common_name) =
        common_name.filter(|value| value.starts_with("Developer ID Application:"))
    else {
        return Ok(None);
    };
    let team_id = certificate
        .subject()
        .iter_organizational_unit()
        .next()
        .and_then(|value| value.as_str().ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("Developer ID certificate has no Apple Team ID (subject OU)"))?;
    Ok(Some((common_name.to_string(), team_id.to_string())))
}

#[cfg(not(target_os = "windows"))]
fn p12_certificates(bytes: &[u8], password: &str) -> Result<Vec<Vec<u8>>> {
    use openssl::pkcs12::Pkcs12;

    let archive = Pkcs12::from_der(bytes).context("Invalid PKCS#12 archive")?;
    let parsed = archive
        .parse2(password)
        .context("Failed to decrypt PKCS#12 archive with the provided password")?;
    let certificate = parsed
        .cert
        .ok_or_else(|| anyhow!("PKCS#12 archive contains no leaf certificate"))?;
    Ok(vec![
        certificate
            .to_der()
            .context("Failed to encode PKCS#12 leaf certificate")?,
    ])
}

#[cfg(target_os = "windows")]
fn p12_certificates(bytes: &[u8], password: &str) -> Result<Vec<Vec<u8>>> {
    let store = schannel::cert_store::CertStore::import_pkcs12(bytes, Some(password))
        .context("Failed to decrypt PKCS#12 archive with the provided password")?;
    let certificates: Vec<Vec<u8>> = store
        .certs()
        .map(|certificate| certificate.to_der().to_vec())
        .collect();
    if certificates.is_empty() {
        bail!("PKCS#12 archive contains no certificates");
    }
    Ok(certificates)
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

/// Execute `lingxia auth logout apple`.
pub fn apple_logout(team_id: Option<String>) -> Result<()> {
    let wallet = Wallet::open()?;
    let teams = wallet.apple_teams()?;

    if teams.is_empty() {
        println!("{} No Apple credentials stored.", "ℹ".blue());
        return Ok(());
    }

    let team_id = match team_id {
        Some(team) => team,
        None if teams.len() == 1 => teams[0].team_id.clone(),
        None => {
            if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                bail!(
                    "several Apple teams are stored ({}); pass --team-id",
                    teams
                        .iter()
                        .map(|t| t.team_id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            let labels: Vec<String> = teams
                .iter()
                .map(|t| format!("{}   {}", t.team_id, t.mechanisms()))
                .collect();
            let selection = Select::new()
                .with_prompt("Log out which team?")
                .items(&labels)
                .default(0)
                .interact()?;
            teams[selection].team_id.clone()
        }
    };

    if wallet.delete_apple_team(&team_id)? {
        println!("{} Removed credentials for team {}.", "✓".green(), team_id);
    } else {
        println!("{} No credentials stored for team {}.", "ℹ".blue(), team_id);
        return Ok(());
    }

    // Last team gone: clear the anisette device fingerprint too.
    if wallet.apple_teams()?.is_empty() {
        let anisette_cache = OmnisetteProvider::cache_path()?;
        if anisette_cache.exists() {
            std::fs::remove_file(&anisette_cache)?;
            println!("{} Anisette cache cleared.", "✓".green());
        }
    }

    Ok(())
}

/// Print the Apple section of `lingxia auth status`.
pub fn apple_status() -> Result<()> {
    let wallet = Wallet::open()?;
    wallet.notice_legacy_files();
    let teams = wallet.apple_teams()?;

    println!("{}", "Apple accounts on this Mac".cyan().bold());
    if teams.is_empty() {
        println!("  {} None. Log in: lingxia auth login apple", "✗".red());
        return Ok(());
    }
    for team in teams {
        let mut line = format!("  {}   {}", team.display(), team.mechanisms());
        let session = if team.has_apple_id {
            wallet.load_apple_id(&team.team_id)?
        } else {
            None
        };
        if let Some(AuthCredentials::AppleId { expiry, .. }) = &session {
            let local = expiry.with_timezone(&chrono::Local);
            if session.as_ref().is_some_and(AuthCredentials::is_expired) {
                line.push_str(&format!(" · {}", "session expired".yellow()));
            } else {
                line.push_str(&format!(" · session until {}", local.format("%m-%d %H:%M")));
            }
        }
        println!("{line}");
        if team.label().is_none() && team.has_apple_id {
            println!("    (log in again with the Apple ID to show the team name)");
        }
        if session.as_ref().is_some_and(AuthCredentials::is_expired) {
            println!("    Renew: lingxia auth login apple --mode password");
        }
    }
    Ok(())
}

/// Fetch developer teams and let the user pick one.
///
/// If there is exactly one team, it is selected automatically.
/// Returns the team and the app token.
fn select_team(
    client: &GrandSlamClient,
    login_data: &GrandSlamLoginData,
    device_info: &DeviceInfo,
    anisette_provider: &mut OmnisetteProvider,
) -> Result<(developer_services::DeveloperTeam, String)> {
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
        return Ok((team.clone(), app_token));
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

    Ok((teams[selection].clone(), app_token))
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

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};

    fn certificate(common_name: &str, team_id: &str) -> Vec<u8> {
        let key = KeyPair::generate().unwrap();
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(DnType::CommonName, common_name);
        distinguished_name.push(DnType::OrganizationalUnitName, team_id);
        let mut params = CertificateParams::default();
        params.distinguished_name = distinguished_name;
        params.self_signed(&key).unwrap().der().to_vec()
    }

    #[test]
    fn app_password_normalized_to_apple_form() {
        let canonical = Some("abcd-efgh-ijkl-mnop".to_string());
        assert_eq!(normalize_app_password("abcd-efgh-ijkl-mnop"), canonical);
        assert_eq!(normalize_app_password("abcdefghijklmnop"), canonical);
        assert_eq!(normalize_app_password(" abcd efgh-ijkl\tmnop\n"), canonical);
        assert_eq!(
            normalize_app_password("\u{1b}abcd-efgh-ijkl-mnop"),
            canonical
        );
        assert_eq!(normalize_app_password("abcd-efgh-ijkl"), None);
        assert_eq!(normalize_app_password("MyPassw0rd!MyPass"), None);
    }

    #[test]
    fn extracts_developer_id_team_from_certificate_subject() {
        let identity = developer_id_certificate_identity(&certificate(
            "Developer ID Application: Example (TEAMAAAAAA)",
            "TEAMAAAAAA",
        ))
        .unwrap();
        assert_eq!(
            identity,
            Some((
                "Developer ID Application: Example (TEAMAAAAAA)".to_string(),
                "TEAMAAAAAA".to_string()
            ))
        );
    }

    #[test]
    fn rejects_non_developer_id_application_certificates() {
        let identity = developer_id_certificate_identity(&certificate(
            "Apple Development: Example (TEAMAAAAAA)",
            "TEAMAAAAAA",
        ))
        .unwrap();
        assert!(identity.is_none());
    }

    #[test]
    fn rejects_developer_id_certificate_from_another_team() {
        let error = validate_developer_id_certificate(
            &certificate(
                "Developer ID Application: Example (TEAMAAAAAA)",
                "TEAMAAAAAA",
            ),
            "TEAMBBBBBB",
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with(resolver::codes::CREDENTIAL_IDENTITY_MISMATCH)
        );
    }
}
