//! Developer ID code-signing + notarization for macOS app bundles.
//!
//! Notary (App Store Connect API) credentials are always required and resolved
//! store-first, env-fallback: the auth credential store
//! (`~/.lingxia/apple/credentials.json`, populated by
//! `lingxia auth apple login --mode key`) first, then the
//! `LINGXIA_APPLE_NOTARY_KEY` (`.p8` path) / `_KEY_ID` / `_ISSUER_ID` env vars.
//!
//! The **signing identity** is then resolved by environment:
//!
//! - **CI** — an explicit `.p12` from the credential store
//!   (`~/.lingxia/apple/developer-id.json`, populated by
//!   `lingxia auth apple import-developer-id`) or the
//!   `LINGXIA_APPLE_DEVELOPER_ID_P12` / `_P12_PASSWORD` env vars is imported into
//!   a throwaway keychain for the run. A runner has no Keychain, so it restores
//!   the cert from CI secrets onto an ephemeral machine — the only place the
//!   `.p12` + password are materialized.
//! - **Local dev** — when no `.p12` is configured, sign with the
//!   "Developer ID Application" identity already in the **login keychain** (the
//!   cert the developer manages in Keychain Access, and would have exported the
//!   `.p12` from). No import, no plaintext `.p12`/password persisted to disk.
//!
//! Notarization runs only when notary creds resolve AND a signing identity is
//! found. Otherwise this is a no-op and the app is left ad-hoc signed (the
//! SwiftPM default), which keeps local builds and tests green.
//!
//! Optional env vars:
//! - `LINGXIA_APPLE_DEVELOPER_ID_IDENTITY` — codesign identity name; if unset,
//!   auto-detected as the identity whose common name starts with
//!   "Developer ID Application".
//! - `LINGXIA_APPLE_ENTITLEMENTS` — path to an entitlements plist for codesign.

use crate::platform::apple::auth::{AuthCredentials, CredentialStorage, DeveloperIdCredentials};
use anyhow::{Context, Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Command;

const ENV_P12: &str = "LINGXIA_APPLE_DEVELOPER_ID_P12";
const ENV_P12_PASSWORD: &str = "LINGXIA_APPLE_DEVELOPER_ID_P12_PASSWORD";
const ENV_NOTARY_KEY: &str = "LINGXIA_APPLE_NOTARY_KEY";
const ENV_NOTARY_KEY_ID: &str = "LINGXIA_APPLE_NOTARY_KEY_ID";
const ENV_NOTARY_ISSUER_ID: &str = "LINGXIA_APPLE_NOTARY_ISSUER_ID";
const ENV_IDENTITY: &str = "LINGXIA_APPLE_DEVELOPER_ID_IDENTITY";
const ENV_ENTITLEMENTS: &str = "LINGXIA_APPLE_ENTITLEMENTS";

/// Best-effort mode (set in CI). When the notarization wait times out without a
/// rejection, warn and continue instead of failing the build: the app is already
/// Developer-ID signed and the submission is in flight, so an un-stapled artifact
/// is acceptable for a CI build check. Release builds leave this unset and demand
/// completion. Empty / `0` counts as off.
const ENV_NOTARIZE_BEST_EFFORT: &str = "LINGXIA_NOTARIZE_BEST_EFFORT";

/// Bound on `notarytool submit --wait`. Apple notarization is normally minutes,
/// but the service can stall; without a cap the wait blocks until the CI job's
/// own timeout kills the orphaned process. Kept under typical job timeouts so we
/// surface a clean warning (best-effort) or error (release) first.
const NOTARY_WAIT_TIMEOUT: &str = "25m";

const DEVELOPER_ID_PREFIX: &str = "Developer ID Application";

/// Resolved Developer-ID certificate material. The `.p12` lives at `p12_path`;
/// when `_temp_p12` is `Some` we materialized it from the credential store and
/// own its lifetime (removed on drop).
struct DeveloperIdMaterial {
    p12_path: String,
    p12_password: String,
    identity: Option<String>,
    _temp_p12: Option<TempFile>,
}

/// Resolved notary (App Store Connect API) material. The `.p8` lives at
/// `notary_key`; when `_temp_key` is `Some` we materialized it from the
/// credential store and own its lifetime (removed on drop).
struct NotaryMaterial {
    notary_key: String,
    notary_key_id: String,
    notary_issuer_id: String,
    _temp_key: Option<TempFile>,
}

/// Fully resolved inputs for a signing + notarization run.
struct NotarizeConfig {
    developer_id: DeveloperIdMaterial,
    notary: NotaryMaterial,
    entitlements: Option<String>,
}

/// A temp file removed (best-effort) when dropped.
struct TempFile {
    path: PathBuf,
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

/// Resolve the Developer ID certificate, store-first then env-fallback.
fn resolve_developer_id() -> Result<Option<DeveloperIdMaterial>> {
    // 1. Credential store (`~/.lingxia/apple/developer-id.json`).
    if let Some(creds) = DeveloperIdCredentials::load()? {
        let bytes = STANDARD
            .decode(creds.p12_base64.trim())
            .context("Failed to base64-decode stored Developer ID .p12")?;
        let path =
            std::env::temp_dir().join(format!("lingxia-developer-id-{}.p12", std::process::id()));
        std::fs::write(&path, &bytes)
            .with_context(|| format!("Failed to write temporary .p12 to {}", path.display()))?;
        let temp = TempFile { path };
        return Ok(Some(DeveloperIdMaterial {
            p12_path: temp.path.to_string_lossy().to_string(),
            p12_password: creds.password,
            identity: creds.identity.or_else(|| non_empty_env(ENV_IDENTITY)),
            _temp_p12: Some(temp),
        }));
    }

    // 2. Environment fallback.
    let (Some(p12_path), Some(p12_password)) =
        (non_empty_env(ENV_P12), non_empty_env(ENV_P12_PASSWORD))
    else {
        return Ok(None);
    };
    Ok(Some(DeveloperIdMaterial {
        p12_path,
        p12_password,
        identity: non_empty_env(ENV_IDENTITY),
        _temp_p12: None,
    }))
}

/// Resolve notary credentials, store-first then env-fallback.
fn resolve_notary() -> Result<Option<NotaryMaterial>> {
    // 1. Credential store (`~/.lingxia/apple/credentials.json`, ASC API key).
    if let Some(AuthCredentials::AppStoreConnect {
        key_id,
        issuer_id,
        private_key_pem,
        ..
    }) = CredentialStorage::new()?.load()?
    {
        let path =
            std::env::temp_dir().join(format!("lingxia-notary-key-{}.p8", std::process::id()));
        std::fs::write(&path, private_key_pem.as_bytes())
            .with_context(|| format!("Failed to write temporary .p8 to {}", path.display()))?;
        let temp = TempFile { path };
        return Ok(Some(NotaryMaterial {
            notary_key: temp.path.to_string_lossy().to_string(),
            notary_key_id: key_id,
            notary_issuer_id: issuer_id,
            _temp_key: Some(temp),
        }));
    }

    // 2. Environment fallback.
    let (Some(notary_key), Some(notary_key_id), Some(notary_issuer_id)) = (
        non_empty_env(ENV_NOTARY_KEY),
        non_empty_env(ENV_NOTARY_KEY_ID),
        non_empty_env(ENV_NOTARY_ISSUER_ID),
    ) else {
        return Ok(None);
    };
    Ok(Some(NotaryMaterial {
        notary_key,
        notary_key_id,
        notary_issuer_id,
        _temp_key: None,
    }))
}

/// Developer-ID sign + notarize `app_path` when credentials resolve.
///
/// Credentials are read store-first, env-fallback. Signing runs only when BOTH
/// a Developer ID certificate AND notary credentials resolve. Otherwise this
/// prints a short note and returns `Ok(())` without touching the app bundle
/// (it stays ad-hoc signed) — it never errors in the ad-hoc case.
pub fn maybe_sign_and_notarize(app_path: &Path) -> Result<()> {
    // Notarization always needs App Store Connect API creds; without them there
    // is nothing to do regardless of how the app would be signed.
    let Some(notary) = resolve_notary()? else {
        println!(
            "{} macOS app left ad-hoc signed — no notary creds \
             ('lingxia auth apple login --mode key' or {}/_KEY_ID/_ISSUER_ID)",
            "ℹ️".blue(),
            ENV_NOTARY_KEY
        );
        return Ok(());
    };

    let entitlements = non_empty_env(ENV_ENTITLEMENTS);

    // An explicit .p12 (env or credential store) is the CI path: a runner has no
    // keychain, so the restored-from-secrets cert is imported into a throwaway
    // keychain for the run.
    if let Some(developer_id) = resolve_developer_id()? {
        let config = NotarizeConfig {
            developer_id,
            notary,
            entitlements,
        };
        return sign_and_notarize(app_path, &config);
    }

    // Local path: sign with a Developer ID identity already in the login keychain
    // (the cert the .p12 would have been exported from). No import, no plaintext
    // .p12/password persisted — Keychain Access stays the source of truth.
    if let Some(identity) = find_login_keychain_identity() {
        return sign_and_notarize_local(app_path, &identity, entitlements.as_deref(), &notary);
    }

    println!(
        "{} macOS app left ad-hoc signed — add a Developer ID Application identity \
         to your login keychain, or set a .p12 \
         ('lingxia auth apple import-developer-id' or {}) for CI",
        "ℹ️".blue(),
        ENV_P12
    );
    Ok(())
}

fn sign_and_notarize(app_path: &Path, config: &NotarizeConfig) -> Result<()> {
    // Create + populate a temporary keychain. Everything after this point must
    // clean up the keychain (and any temp zip) even on failure, so the result
    // is captured and the cleanup runs unconditionally below.
    let keychain = TempKeychain::create()?;

    let result = (|| {
        keychain.import_p12(
            &config.developer_id.p12_path,
            &config.developer_id.p12_password,
        )?;

        let identity = match &config.developer_id.identity {
            Some(id) => id.clone(),
            None => keychain.find_developer_id_identity()?,
        };

        codesign(
            app_path,
            &identity,
            config.entitlements.as_deref(),
            Some(&keychain.path),
        )?;
        println!("  {} codesigned (Developer ID)", "✓".green());

        notarize(app_path, &config.notary)?;
        println!("  {} notarized", "✓".green());

        staple(app_path)?;
        println!("  {} stapled", "✓".green());

        Ok(())
    })();

    // Best-effort cleanup of the temporary keychain regardless of outcome.
    keychain.cleanup();

    result
}

/// Local signing path: sign with a Developer ID identity already present in the
/// login keychain (no temporary keychain, no `.p12` import), then notarize and
/// staple. Used on developer machines where the cert lives in Keychain Access.
fn sign_and_notarize_local(
    app_path: &Path,
    identity: &str,
    entitlements: Option<&str>,
    notary: &NotaryMaterial,
) -> Result<()> {
    codesign(app_path, identity, entitlements, None)?;
    println!(
        "  {} codesigned (Developer ID, login keychain)",
        "✓".green()
    );

    notarize(app_path, notary)?;
    println!("  {} notarized", "✓".green());

    staple(app_path)?;
    println!("  {} stapled", "✓".green());

    Ok(())
}

/// Find a "Developer ID Application" identity in the user's default keychain
/// search list (the login keychain) so a local build can sign with the cert the
/// developer already manages in Keychain Access. An explicit identity name via
/// `LINGXIA_APPLE_DEVELOPER_ID_IDENTITY` takes precedence.
fn find_login_keychain_identity() -> Option<String> {
    if let Some(identity) = non_empty_env(ENV_IDENTITY) {
        return Some(identity);
    }
    let output = Command::new("security")
        .args(["find-identity", "-v", "-p", "codesigning"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| parse_common_name(line).filter(|cn| cn.starts_with(DEVELOPER_ID_PREFIX)))
}

/// A temporary keychain used only for the duration of one signing run.
struct TempKeychain {
    path: PathBuf,
    password: String,
}

impl TempKeychain {
    fn create() -> Result<Self> {
        // Derive a non-secret-but-unguessable password without adding a crypto
        // dependency. The keychain lives in a private temp dir for seconds and
        // is deleted afterwards, so this is sufficient.
        let password = format!("lingxia-notarize-{}-kc", std::process::id());
        let path = std::env::temp_dir().join(format!(
            "lingxia-notarize-{}.keychain-db",
            std::process::id()
        ));

        // Remove any stale keychain at this path.
        if path.exists() {
            let _ = Command::new("security")
                .arg("delete-keychain")
                .arg(&path)
                .output();
        }

        run_security(
            &["create-keychain", "-p", &password],
            &path,
            "create temporary keychain",
        )?;
        run_security(
            &["unlock-keychain", "-p", &password],
            &path,
            "unlock temporary keychain",
        )?;
        // Disable auto-lock so a slow notarytool wait can't lock us out.
        let _ = Command::new("security")
            .args(["set-keychain-settings", "-t", "3600", "-u"])
            .arg(&path)
            .output();

        let kc = Self { path, password };
        kc.add_to_search_list()?;
        Ok(kc)
    }

    /// Prepend this keychain to the user search list so `codesign` and
    /// `find-identity` can see the imported identity.
    fn add_to_search_list(&self) -> Result<()> {
        let output = Command::new("security")
            .args(["list-keychains", "-d", "user"])
            .output()
            .context("Failed to list keychains")?;
        let existing = String::from_utf8_lossy(&output.stdout);
        let mut keychains: Vec<String> = existing
            .lines()
            .map(|l| l.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let self_path = self.path.to_string_lossy().to_string();
        keychains.retain(|k| k != &self_path);
        keychains.insert(0, self_path);

        let status = Command::new("security")
            .args(["list-keychains", "-d", "user", "-s"])
            .args(&keychains)
            .status()
            .context("Failed to set keychain search list")?;
        if !status.success() {
            return Err(anyhow!("Failed to add temporary keychain to search list"));
        }
        Ok(())
    }

    /// Import a Developer ID `.p12` and authorize `codesign` to use the key.
    fn import_p12(&self, p12_path: &str, p12_password: &str) -> Result<()> {
        if !Path::new(p12_path).exists() {
            return Err(anyhow!(
                "Developer ID .p12 does not point to an existing file: {}",
                p12_path
            ));
        }

        let output = Command::new("security")
            .arg("import")
            .arg(p12_path)
            .args(["-k"])
            .arg(&self.path)
            .args(["-P", p12_password])
            .args(["-T", "/usr/bin/codesign"])
            .args(["-f", "pkcs12"])
            .output()
            .context("Failed to import Developer ID .p12")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("security import failed: {}", stderr.trim()));
        }

        // Allow codesign (and Apple tooling) to use the private key without an
        // interactive prompt.
        let output = Command::new("security")
            .args([
                "set-key-partition-list",
                "-S",
                "apple-tool:,apple:,codesign:",
                "-s",
                "-k",
                &self.password,
            ])
            .arg(&self.path)
            .output()
            .context("Failed to set key partition list")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "security set-key-partition-list failed: {}",
                stderr.trim()
            ));
        }

        Ok(())
    }

    /// Find the "Developer ID Application" identity in this keychain.
    fn find_developer_id_identity(&self) -> Result<String> {
        let output = Command::new("security")
            .args(["find-identity", "-v", "-p", "codesigning"])
            .arg(&self.path)
            .output()
            .context("Failed to list signing identities")?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            if let Some(common_name) = parse_common_name(line)
                && common_name.starts_with(DEVELOPER_ID_PREFIX)
            {
                return Ok(common_name);
            }
        }

        Err(anyhow!(
            "No '{}' identity found in the imported certificate. \
             Set {} to override identity selection.\nfind-identity output:\n{}",
            DEVELOPER_ID_PREFIX,
            ENV_IDENTITY,
            stdout.trim()
        ))
    }

    fn cleanup(&self) {
        let _ = Command::new("security")
            .arg("delete-keychain")
            .arg(&self.path)
            .output();
    }
}

fn run_security(args: &[&str], keychain: &Path, action: &str) -> Result<()> {
    let output = Command::new("security")
        .args(args)
        .arg(keychain)
        .output()
        .with_context(|| format!("Failed to {action}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Failed to {}: {}", action, stderr.trim()));
    }
    Ok(())
}

/// Extract the quoted common name from a `security find-identity` line.
fn parse_common_name(line: &str) -> Option<String> {
    let start = line.find('"')?;
    let rest = &line[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Codesign the app bundle with a hardened runtime + secure timestamp.
fn codesign(
    app_path: &Path,
    identity: &str,
    entitlements: Option<&str>,
    keychain: Option<&Path>,
) -> Result<()> {
    let mut cmd = Command::new("codesign");
    cmd.args(["--force", "--deep", "--options", "runtime", "--timestamp"]);

    if let Some(ent) = entitlements {
        if !Path::new(ent).exists() {
            return Err(anyhow!(
                "{} does not point to an existing file: {}",
                ENV_ENTITLEMENTS,
                ent
            ));
        }
        cmd.arg("--entitlements").arg(ent);
    }

    cmd.arg("--sign").arg(identity);
    // CI path passes the temporary keychain; the local path signs against the
    // default search list (the login keychain).
    if let Some(keychain) = keychain {
        cmd.arg("--keychain").arg(keychain);
    }
    cmd.arg(app_path);

    let output = cmd.output().context("Failed to execute codesign")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "codesign failed for {}: {}",
            app_path.display(),
            stderr.trim()
        ));
    }
    Ok(())
}

/// Zip the signed app and submit it to Apple's notary service, waiting for the
/// result.
fn notarize(app_path: &Path, notary: &NotaryMaterial) -> Result<()> {
    if !Path::new(&notary.notary_key).exists() {
        return Err(anyhow!(
            "Notary key does not point to an existing file: {}",
            notary.notary_key
        ));
    }

    let zip_path =
        std::env::temp_dir().join(format!("lingxia-notarize-{}.zip", std::process::id()));
    // Remove any stale archive before recreating it.
    let _ = std::fs::remove_file(&zip_path);

    let result = (|| {
        let status = Command::new("ditto")
            .args(["-c", "-k", "--keepParent"])
            .arg(app_path)
            .arg(&zip_path)
            .status()
            .context("Failed to execute ditto for notarization archive")?;
        if !status.success() {
            return Err(anyhow!("ditto failed to create notarization archive"));
        }

        let best_effort = std::env::var(ENV_NOTARIZE_BEST_EFFORT)
            .map(|v| !matches!(v.trim(), "" | "0"))
            .unwrap_or(false);

        let output = Command::new("xcrun")
            .arg("notarytool")
            .arg("submit")
            .arg(&zip_path)
            .args(["--key", &notary.notary_key])
            .args(["--key-id", &notary.notary_key_id])
            .args(["--issuer", &notary.notary_issuer_id])
            .arg("--wait")
            .args(["--timeout", NOTARY_WAIT_TIMEOUT])
            .output()
            .context("Failed to execute xcrun notarytool submit")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // A real review rejection is always fatal. notarytool exits 0 even when
        // the submission status is "Invalid", so check the status explicitly.
        // On rejection, `submit` output says *that* it failed but not *why* —
        // fetch the per-submission notary log, which lists the actual issues
        // (unsigned nested code, missing hardened runtime, bad entitlements, …),
        // so a failed run is diagnosable.
        if stdout.contains("status: Invalid") || stdout.contains("status: Rejected") {
            let detail = submission_id(&stdout)
                .and_then(|id| fetch_notary_log(&id, notary))
                .map(|log| format!("\n\nNotary log:\n{log}"))
                .unwrap_or_default();
            return Err(anyhow!(
                "notarization was not accepted:\n{}{}",
                stdout.trim(),
                detail
            ));
        }

        if !output.status.success() {
            // Non-zero without a rejection means the wait hit NOTARY_WAIT_TIMEOUT
            // (or Apple is slow / unreachable) — the submission is still in
            // flight, not refused. In best-effort mode (CI) keep going with an
            // un-stapled, signed app; release builds demand a completed ticket.
            if best_effort {
                eprintln!(
                    "  {} notarization did not finish within {NOTARY_WAIT_TIMEOUT} — the app is \
                     Developer-ID signed and the submission is in flight; continuing un-stapled \
                     (best-effort).\n{}\n{}",
                    "⚠".yellow(),
                    stdout.trim(),
                    stderr.trim()
                );
                return Ok(());
            }
            return Err(anyhow!(
                "notarytool submit failed:\n{}\n{}",
                stdout.trim(),
                stderr.trim()
            ));
        }

        Ok(())
    })();

    let _ = std::fs::remove_file(&zip_path);
    result
}

/// Pull the submission id out of `notarytool submit` output (lines of the form
/// `  id: <uuid>`); used to fetch the detailed log on rejection.
fn submission_id(submit_output: &str) -> Option<String> {
    submit_output.lines().find_map(|line| {
        line.trim()
            .strip_prefix("id:")
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
    })
}

/// Fetch the detailed notary log for a submission. Best-effort: returns the
/// log JSON on success, `None` if the `log` call itself fails.
fn fetch_notary_log(id: &str, notary: &NotaryMaterial) -> Option<String> {
    let output = Command::new("xcrun")
        .arg("notarytool")
        .arg("log")
        .arg(id)
        .args(["--key", &notary.notary_key])
        .args(["--key-id", &notary.notary_key_id])
        .args(["--issuer", &notary.notary_issuer_id])
        .output()
        .ok()?;
    let log = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!log.is_empty()).then_some(log)
}

/// Staple the notarization ticket onto the app so it validates offline.
fn staple(app_path: &Path) -> Result<()> {
    let output = Command::new("xcrun")
        .arg("stapler")
        .arg("staple")
        .arg(app_path)
        .output()
        .context("Failed to execute xcrun stapler staple")?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "stapler staple failed:\n{}\n{}",
            stdout.trim(),
            stderr.trim()
        ));
    }
    Ok(())
}
