//! Xiaomi GetApps (小米应用商店) submission via the developer open platform's
//! app-publish ("应用上传/推送") API.
//!
//! Flow: client-credentials auth → request an upload slot → upload the `.apk` →
//! submit/commit the new version for the package.
//!
//! NOT E2E-verified — needs a real Xiaomi developer account. The exact
//! open-platform endpoints/payloads are not publicly stable, so uncertain steps
//! are marked with `// TODO: verify ...` rather than fabricated.

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;

use super::backend::{SubmitOptions, http};
use super::creds::XiaomiCreds;
use crate::config::XiaomiStoreConfig;

// TODO: verify Xiaomi open-platform API base host/path.
const API: &str = "https://api.developer.xiaomi.com/devupload";

struct Session {
    token: String,
}

impl Session {
    fn login(creds: &XiaomiCreds) -> Result<Self> {
        // TODO: verify Xiaomi auth endpoint + grant type (some flows use an
        // RSA-signed request rather than a bearer token).
        let body = json!({
            "grant_type": "client_credentials",
            "client_id": creds.client_id,
            "client_secret": creds.client_secret,
        });
        let mut resp = http()
            .post(&format!("{API}/oauth2/token"))
            .send_json(&body)
            .map_err(|e| anyhow::anyhow!("Xiaomi token request failed: {e}"))?;
        let v: Value = resp
            .body_mut()
            .read_json()
            .context("parse token response")?;
        let token = v
            .get("access_token")
            .and_then(Value::as_str)
            .context("Xiaomi response missing access_token")?
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

    fn get(&self, url: &str) -> Result<Value> {
        let mut resp = http()
            .get(url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .call()
            .map_err(|e| anyhow::anyhow!("GET {url} failed: {e}"))?;
        resp.body_mut().read_json().context("parse response")
    }
}

pub fn submit(
    creds: &XiaomiCreds,
    cfg: &XiaomiStoreConfig,
    artifact: &Path,
    opts: &SubmitOptions,
) -> Result<()> {
    let pkg = &cfg.package_name;
    let session = Session::login(creds)?;
    println!("  {} authenticated with Xiaomi GetApps", "✓".green());

    // 1. Upload the APK bytes.
    // TODO: verify Xiaomi upload endpoint/field names and whether an upload slot
    // must be requested first.
    let dest = upload_apk(&session, pkg, artifact)?;
    println!("  {} uploaded {}", "✓".green(), artifact.display());

    if opts.draft {
        println!(
            "  {} draft uploaded — submit it in the Xiaomi console to publish",
            "ℹ".blue()
        );
        return Ok(());
    }

    // 2. Submit the new version for the package.
    // TODO: verify Xiaomi submit/commit endpoint + payload (release notes field).
    let mut body = json!({ "packageName": pkg, "apk": dest });
    if let Some(notes) = &opts.release_notes {
        body["updateDesc"] = json!(notes);
    }
    session.post(&format!("{API}/dev/push"), &body)?;
    println!("  {} submitted {pkg} for review", "✓".green());
    Ok(())
}

pub fn status(creds: &XiaomiCreds, cfg: &XiaomiStoreConfig) -> Result<()> {
    let session = Session::login(creds)?;
    // TODO: verify Xiaomi status/query endpoint + response shape.
    let info = session.get(&format!("{API}/dev/query?packageName={}", cfg.package_name))?;
    let state = info
        .get("auditStatus")
        .map(|v| v.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("Xiaomi GetApps {} audit status: {state}", cfg.package_name);
    Ok(())
}

/// Upload the APK and return a server-side reference to bind to the package.
/// TODO: verify Xiaomi upload transport (multipart vs. raw) and response shape.
fn upload_apk(session: &Session, pkg: &str, artifact: &Path) -> Result<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(artifact)
        .with_context(|| format!("open {}", artifact.display()))?
        .read_to_end(&mut bytes)
        .with_context(|| format!("read {}", artifact.display()))?;
    let mut resp = http()
        .post(&format!("{API}/dev/upload?packageName={pkg}"))
        .header("Authorization", &format!("Bearer {}", session.token))
        .header("Content-Type", "application/vnd.android.package-archive")
        .send(bytes.as_slice())
        .map_err(|e| anyhow::anyhow!("Xiaomi upload failed: {e}"))?;
    let v: Value = resp
        .body_mut()
        .read_json()
        .context("parse upload response")?;
    v.get("fileId")
        .or_else(|| v.get("url"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .context("Xiaomi upload response missing file reference")
}
