//! App Store Connect submission.
//!
//! The binary is uploaded with `xcrun altool` (the supported transport; needs
//! Xcode, macOS only). Status is read from the App Store Connect API using an
//! ES256 JWT minted locally from the `.p8` key.
//!
//! NOT E2E-verified — needs a real App Store Connect account + Xcode.

use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use colored::Colorize;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

use super::backend::{StorePlatform, SubmitOptions, http};
use super::creds::AppStoreCreds;
use crate::config::AppStoreConfig;

const ASC_BASE: &str = "https://api.appstoreconnect.apple.com";

/// Mint a short-lived (≤20 min) ES256 JWT for the App Store Connect API.
pub fn asc_jwt(creds: &AppStoreCreds) -> Result<String> {
    use p256::ecdsa::{Signature, SigningKey, signature::Signer};
    use p256::pkcs8::DecodePrivateKey;

    let key_path = expand(&creds.key_path);
    let pem = std::fs::read_to_string(&key_path)
        .with_context(|| format!("read ASC private key {}", key_path.display()))?;
    let signing_key =
        SigningKey::from_pkcs8_pem(&pem).context("parse ASC .p8 (expected PKCS#8 EC P-256)")?;

    let now = chrono::Utc::now().timestamp();
    let header = json!({ "alg": "ES256", "kid": creds.key_id, "typ": "JWT" });
    let payload = json!({
        "iss": creds.issuer_id,
        "iat": now,
        "exp": now + 1200,
        "aud": "appstoreconnect-v1",
    });
    let signing_input = format!(
        "{}.{}",
        B64URL.encode(serde_json::to_vec(&header)?),
        B64URL.encode(serde_json::to_vec(&payload)?)
    );
    let sig: Signature = signing_key.sign(signing_input.as_bytes());
    Ok(format!("{signing_input}.{}", B64URL.encode(sig.to_bytes())))
}

pub fn submit(
    creds: &AppStoreCreds,
    platform: StorePlatform,
    artifact: &Path,
    opts: &SubmitOptions,
) -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("App Store upload uses `xcrun altool`, which requires Xcode on macOS.");
    }
    // altool resolves the API key from a private_keys search dir.
    stage_private_key(creds)?;

    let type_arg = match platform {
        StorePlatform::Ios => "ios",
        StorePlatform::Macos => "macos",
        _ => bail!("App Store submit expects --platform ios or macos"),
    };

    println!("  uploading {} via altool…", artifact.display());
    let status = Command::new("xcrun")
        .args(["altool", "--upload-app", "--type", type_arg, "--file"])
        .arg(artifact)
        .args(["--apiKey", &creds.key_id, "--apiIssuer", &creds.issuer_id])
        .status()
        .context("run xcrun altool")?;
    if !status.success() {
        bail!("altool upload failed (exit {:?})", status.code());
    }
    println!("  {} uploaded to App Store Connect", "✓".green());
    println!(
        "  {} processing takes a few minutes; then attach the build to a version \
         and submit for review (`lingxia store status --platform {type_arg}` to poll){}",
        "ℹ".blue(),
        opts.release_notes
            .as_ref()
            .map(|_| " — release notes are set per-version in App Store Connect")
            .unwrap_or("")
    );
    Ok(())
}

pub fn status(creds: &AppStoreCreds, cfg: &AppStoreConfig) -> Result<()> {
    let jwt = asc_jwt(creds)?;
    // Latest builds for the app's bundle id → processing state.
    let url = format!(
        "{ASC_BASE}/v1/builds?filter[app.bundleId]={}&limit=5&sort=-uploadedDate",
        cfg.bundle_id
    );
    let mut resp = http()
        .get(&url)
        .header("Authorization", &format!("Bearer {jwt}"))
        .call()
        .map_err(|e| anyhow::anyhow!("App Store Connect builds query failed: {e}"))?;
    let body: Value = resp
        .body_mut()
        .read_json()
        .context("parse builds response")?;
    let builds = body.get("data").and_then(Value::as_array);
    match builds {
        Some(b) if !b.is_empty() => {
            println!("App Store Connect builds for {}:", cfg.bundle_id);
            for build in b {
                let attrs = build.get("attributes");
                let version = attrs
                    .and_then(|a| a.get("version"))
                    .and_then(Value::as_str)
                    .unwrap_or("?");
                let state = attrs
                    .and_then(|a| a.get("processingState"))
                    .and_then(Value::as_str)
                    .unwrap_or("?");
                println!("  build {version}: {state}");
            }
        }
        _ => println!("No builds found for {} yet.", cfg.bundle_id),
    }
    Ok(())
}

/// altool searches `~/.appstoreconnect/private_keys/AuthKey_<id>.p8`; make sure
/// the configured key is there.
fn stage_private_key(creds: &AppStoreCreds) -> Result<()> {
    let src = expand(&creds.key_path);
    let dir = dirs::home_dir()
        .context("home dir")?
        .join(".appstoreconnect")
        .join("private_keys");
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let dst = dir.join(format!("AuthKey_{}.p8", creds.key_id));
    if src != dst {
        std::fs::copy(&src, &dst)
            .with_context(|| format!("stage {} -> {}", src.display(), dst.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o600));
        }
    }
    Ok(())
}

fn expand(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    std::path::PathBuf::from(path)
}
