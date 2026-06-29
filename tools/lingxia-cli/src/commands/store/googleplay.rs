//! Google Play submission via the Google Play Developer API v3.
//!
//! Flow: service-account OAuth2 (RS256 JWT assertion → access token) →
//! `edits.insert` → upload the `.aab` (`edits.bundles.upload`) or `.apk`
//! (`edits.apks.upload`) → `edits.tracks.update` (assign the new version code to
//! a track) → `edits.commit`.
//!
//! NOT E2E-verified — needs a real Google Play Console service account. Built to
//! the documented Developer API
//! (https://developers.google.com/android-publisher/api-ref/rest).

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use colored::Colorize;
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;

use super::backend::{SubmitOptions, http};
use super::creds::GooglePlayCreds;
use crate::config::GooglePlayConfig;

const OAUTH_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const SCOPE: &str = "https://www.googleapis.com/auth/androidpublisher";
const API: &str = "https://androidpublisher.googleapis.com/androidpublisher/v3";
const UPLOAD_API: &str = "https://androidpublisher.googleapis.com/upload/androidpublisher/v3";

struct Session {
    token: String,
}

impl Session {
    fn login(creds: &GooglePlayCreds) -> Result<Self> {
        let (client_email, private_key) = service_account(creds)?;
        let assertion = sign_assertion(&client_email, &private_key)?;
        let form = format!(
            "grant_type={}&assertion={}",
            urlencode("urn:ietf:params:oauth:grant-type:jwt-bearer"),
            urlencode(&assertion)
        );
        let mut resp = http()
            .post(OAUTH_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send(form.as_bytes())
            .map_err(|e| anyhow::anyhow!("Google OAuth token request failed: {e}"))?;
        let v: Value = resp
            .body_mut()
            .read_json()
            .context("parse Google token response")?;
        let token = v
            .get("access_token")
            .and_then(Value::as_str)
            .context("Google token response missing access_token")?
            .to_string();
        Ok(Self { token })
    }

    fn post(&self, url: &str, body: &Value) -> Result<Value> {
        let mut resp = http()
            .post(url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .send_json(body)
            .map_err(|e| anyhow::anyhow!("POST {url} failed: {e}"))?;
        resp.body_mut().read_json().context("parse response")
    }

    fn put(&self, url: &str, body: &Value) -> Result<Value> {
        let mut resp = http()
            .put(url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .send_json(body)
            .map_err(|e| anyhow::anyhow!("PUT {url} failed: {e}"))?;
        resp.body_mut().read_json().context("parse response")
    }

    fn get(&self, url: &str) -> Result<Value> {
        let mut resp = http()
            .get(url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .call()
            .map_err(|e| anyhow::anyhow!("GET {url} failed: {e}"))?;
        resp.body_mut().read_json().context("parse response")
    }

    /// Upload the artifact bytes to an `edits.*.upload` media endpoint.
    fn upload(&self, url: &str, artifact: &Path) -> Result<Value> {
        let mut bytes = Vec::new();
        std::fs::File::open(artifact)
            .with_context(|| format!("open {}", artifact.display()))?
            .read_to_end(&mut bytes)
            .with_context(|| format!("read {}", artifact.display()))?;
        let mut resp = http()
            .post(url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("Content-Type", "application/octet-stream")
            .send(bytes.as_slice())
            .map_err(|e| anyhow::anyhow!("Google Play upload failed: {e}"))?;
        resp.body_mut().read_json().context("parse upload response")
    }
}

pub fn submit(
    creds: &GooglePlayCreds,
    cfg: &GooglePlayConfig,
    artifact: &Path,
    opts: &SubmitOptions,
) -> Result<()> {
    let pkg = &cfg.package_name;
    let track = opts
        .track
        .clone()
        .or_else(|| cfg.default_track.clone())
        .unwrap_or_else(|| "internal".to_string());

    let session = Session::login(creds)?;
    println!("  {} authenticated with Google Play", "✓".green());

    // 1. Open an edit transaction.
    let edit = session.post(&format!("{API}/applications/{pkg}/edits"), &json!({}))?;
    let edit_id = edit
        .get("id")
        .and_then(Value::as_str)
        .context("edits.insert response missing id")?
        .to_string();

    // 2. Upload the bundle (.aab) or apk (.apk) as upload media.
    let ext = artifact
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let kind = if ext == "apk" { "apks" } else { "bundles" };
    let uploaded = session.upload(
        &format!("{UPLOAD_API}/applications/{pkg}/edits/{edit_id}/{kind}?uploadType=media"),
        artifact,
    )?;
    let version_code = uploaded
        .get("versionCode")
        .and_then(Value::as_i64)
        .context("upload response missing versionCode")?;
    println!(
        "  {} uploaded version code {version_code} ({kind})",
        "✓".green()
    );

    // 3. Assign the version code to a track release. `draft` keeps the release
    // unpublished; otherwise it rolls out to 100%.
    let release_status = if opts.draft { "draft" } else { "completed" };
    let mut release = json!({
        "versionCodes": [version_code.to_string()],
        "status": release_status,
    });
    if let Some(notes) = &opts.release_notes {
        release["releaseNotes"] = json!([{ "language": "en-US", "text": notes }]);
    }
    session.put(
        &format!("{API}/applications/{pkg}/edits/{edit_id}/tracks/{track}"),
        &json!({ "track": track, "releases": [release] }),
    )?;
    println!(
        "  {} assigned to track '{track}' ({release_status})",
        "✓".green()
    );

    // 4. Commit the edit. Without a commit the edit is discarded; for drafts the
    // release simply stays unpublished after commit.
    session.post(
        &format!("{API}/applications/{pkg}/edits/{edit_id}:commit"),
        &json!({}),
    )?;
    println!("  {} committed edit {edit_id}", "✓".green());
    Ok(())
}

pub fn status(creds: &GooglePlayCreds, cfg: &GooglePlayConfig) -> Result<()> {
    let pkg = &cfg.package_name;
    let session = Session::login(creds)?;
    // Open a throwaway edit to read current track releases.
    let edit = session.post(&format!("{API}/applications/{pkg}/edits"), &json!({}))?;
    let edit_id = edit
        .get("id")
        .and_then(Value::as_str)
        .context("edits.insert response missing id")?;
    let tracks = session.get(&format!("{API}/applications/{pkg}/edits/{edit_id}/tracks"))?;
    match tracks.get("tracks").and_then(Value::as_array) {
        Some(list) if !list.is_empty() => {
            println!("Google Play tracks for {pkg}:");
            for t in list {
                let name = t.get("track").and_then(Value::as_str).unwrap_or("?");
                let codes: Vec<String> = t
                    .get("releases")
                    .and_then(Value::as_array)
                    .map(|rs| {
                        rs.iter()
                            .filter_map(|r| r.get("status").and_then(Value::as_str))
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                println!("  track {name}: {}", codes.join(", "));
            }
        }
        _ => println!("No Google Play track releases found for {pkg} yet."),
    }
    Ok(())
}

/// Resolve `(client_email, private_key)` from the JSON key file or inline fields.
fn service_account(creds: &GooglePlayCreds) -> Result<(String, String)> {
    if let Some(path) = &creds.service_account_json {
        let path = expand(path);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read service-account JSON {}", path.display()))?;
        let v: Value = serde_json::from_str(&text).context("parse service-account JSON")?;
        let email = v
            .get("client_email")
            .and_then(Value::as_str)
            .context("service-account JSON missing client_email")?
            .to_string();
        let key = v
            .get("private_key")
            .and_then(Value::as_str)
            .context("service-account JSON missing private_key")?
            .to_string();
        return Ok((email, key));
    }
    let email = creds
        .client_email
        .clone()
        .context("Google Play creds missing client_email")?;
    let key = creds
        .private_key
        .clone()
        .context("Google Play creds missing private_key")?;
    Ok((email, key))
}

/// Mint a short-lived RS256 JWT assertion for the OAuth2 token exchange.
fn sign_assertion(client_email: &str, private_key_pem: &str) -> Result<String> {
    let now = chrono::Utc::now().timestamp();
    let header = json!({ "alg": "RS256", "typ": "JWT" });
    let claims = json!({
        "iss": client_email,
        "scope": SCOPE,
        "aud": OAUTH_TOKEN_URL,
        "iat": now,
        "exp": now + 3600,
    });
    let signing_input = format!(
        "{}.{}",
        B64URL.encode(serde_json::to_vec(&header)?),
        B64URL.encode(serde_json::to_vec(&claims)?)
    );
    let sig = rs256_sign(private_key_pem, &signing_input)?;
    Ok(format!("{signing_input}.{}", B64URL.encode(sig)))
}

/// RS256 = RSASSA-PKCS1-v1.5 over SHA-256. Hash with `sha2`, then sign the
/// pre-hashed DigestInfo with `rsa` (avoids crossing digest-crate versions).
fn rs256_sign(private_key_pem: &str, signing_input: &str) -> Result<Vec<u8>> {
    use rsa::RsaPrivateKey;
    use rsa::pkcs8::DecodePrivateKey;
    use sha2::{Digest, Sha256};

    let key = RsaPrivateKey::from_pkcs8_pem(private_key_pem)
        .context("parse Google service-account private_key (expected PKCS#8 RSA PEM)")?;
    let hashed = Sha256::digest(signing_input.as_bytes());
    // ASN.1 DigestInfo prefix for SHA-256 (RFC 8017, §9.2 / appendix B.1).
    const SHA256_PREFIX: [u8; 19] = [
        0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01,
        0x05, 0x00, 0x04, 0x20,
    ];
    let scheme = rsa::Pkcs1v15Sign {
        hash_len: Some(hashed.len()),
        prefix: SHA256_PREFIX.to_vec().into_boxed_slice(),
    };
    key.sign(scheme, &hashed)
        .context("RS256-sign Google service-account JWT")
}

fn expand(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    std::path::PathBuf::from(path)
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
