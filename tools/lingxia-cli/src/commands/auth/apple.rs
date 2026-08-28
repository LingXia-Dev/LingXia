//! Apple authentication commands (wallet-backed).
//!
//! `lingxia auth login apple` stores credentials per Apple Team in the
//! wallet; mechanisms (ASC key, Apple ID session, Developer ID certificate)
//! co-exist per team. Logging into the same team + mechanism again is a key
//! rotation: the fingerprint change is shown and confirmed.

use crate::path_completion::FilePathCompleter;
use crate::platform::apple::anisette::OmnisetteProvider;
use crate::platform::apple::auth::{AuthCredentials, DeveloperIdCredentials};
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
    ];

    let selection = Select::new()
        .with_prompt("Select login mode")
        .items(&modes)
        .default(0)
        .interact()?;

    Ok(match selection {
        0 => "key".to_string(),
        1 => "password".to_string(),
        _ => "developer-id".to_string(),
    })
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
        if !matches!(normalized.as_str(), "key" | "password" | "developer-id") {
            return Err(anyhow!(
                "Invalid mode '{}'. Expected one of: key, password, developer-id",
                mode
            ));
        }

        if normalized != "key" && has_key_args {
            return Err(anyhow!(
                "API key parameters (--key-id/--issuer-id/--private-key-path) are only valid with --mode key."
            ));
        }
        if normalized != "password" && (username.is_some() || password.is_some()) {
            return Err(anyhow!(
                "--username/--password are only valid with --mode password."
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
            "Cannot infer mode from the given flags. Please specify --mode key|password|developer-id."
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

    println!();
    println!("{} Successfully logged in!", "✓".green());
    println!("  Apple ID: {}", username);
    println!("  Team ID:  {}", team_id);
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

    println!("{}", "Apple".cyan().bold());
    if teams.is_empty() {
        println!(
            "  {} No credentials. Fix: lingxia auth login apple",
            "✗".red()
        );
        return Ok(());
    }
    for team in teams {
        println!("  {}  {}", team.team_id, team.mechanisms());
        if team.has_apple_id
            && let Some(creds) = wallet.load_apple_auth(&team.team_id)?
            && creds.is_expired()
        {
            println!(
                "    {} Apple ID session expired. Fix: lingxia auth login apple --mode password",
                "⚠".yellow()
            );
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
