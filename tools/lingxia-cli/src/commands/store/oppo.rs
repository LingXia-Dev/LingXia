//! OPPO 软件商店 submission via the OPPO open platform's app-publish API.
//!
//! Flow: client-credentials auth → request an upload URL → upload the `.apk` →
//! submit the new version for the package.
//!
//! NOT E2E-verified — needs a real OPPO developer account. The exact
//! open-platform endpoints/payloads are not publicly stable, so uncertain steps
//! are marked with `// TODO: verify ...` rather than fabricated.

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;

use super::backend::{SubmitOptions, http};
use super::creds::OppoCreds;
use crate::config::OppoStoreConfig;

// TODO: verify OPPO open-platform API base host/path.
const API: &str = "https://oop-openapi-cn.heytapmobi.com";

struct Session {
    token: String,
}

impl Session {
    fn login(creds: &OppoCreds) -> Result<Self> {
        // TODO: verify OPPO auth endpoint + grant type (the documented flow
        // exchanges client_id/client_secret for an access token).
        let url = format!(
            "{API}/developer/v1/token?client_id={}&client_secret={}",
            urlencode(&creds.client_id),
            urlencode(&creds.client_secret),
        );
        let mut resp = http()
            .get(&url)
            .call()
            .map_err(|e| anyhow::anyhow!("OPPO token request failed: {e}"))?;
        let v: Value = resp
            .body_mut()
            .read_json()
            .context("parse token response")?;
        let token = v
            .pointer("/data/access_token")
            .or_else(|| v.get("access_token"))
            .and_then(Value::as_str)
            .context("OPPO response missing access_token")?
            .to_string();
        Ok(Self { token })
    }

    fn get(&self, url: &str) -> Result<Value> {
        let mut resp = http()
            .get(url)
            .header("access-token", &self.token)
            .call()
            .map_err(|e| anyhow::anyhow!("GET {url} failed: {e}"))?;
        resp.body_mut().read_json().context("parse response")
    }

    fn post(&self, url: &str, body: &Value) -> Result<Value> {
        let mut resp = http()
            .post(url)
            .header("access-token", &self.token)
            .send_json(body)
            .map_err(|e| anyhow::anyhow!("POST {url} failed: {e}"))?;
        resp.body_mut().read_json().context("parse response")
    }
}

pub fn submit(
    creds: &OppoCreds,
    cfg: &OppoStoreConfig,
    artifact: &Path,
    opts: &SubmitOptions,
) -> Result<()> {
    let pkg = &cfg.package_name;
    let session = Session::login(creds)?;
    println!("  {} authenticated with OPPO 软件商店", "✓".green());

    // 1. Request an upload URL, then upload the APK to it.
    // TODO: verify OPPO upload-url + upload endpoints and response fields.
    let up = session.get(&format!("{API}/resource/v1/upload/get-upload-url"))?;
    let upload_url = up
        .pointer("/data/upload_url")
        .and_then(Value::as_str)
        .context("OPPO response missing upload_url")?;
    let dest = upload_apk(upload_url, artifact)?;
    println!("  {} uploaded {}", "✓".green(), artifact.display());

    if opts.draft {
        println!(
            "  {} draft uploaded — submit it in the OPPO console to publish",
            "ℹ".blue()
        );
        return Ok(());
    }

    // 2. Submit the new version for the package.
    // TODO: verify OPPO app-submit endpoint + payload (app id + release notes).
    let mut body = json!({ "pkg_name": pkg, "apk_url": dest });
    if let Some(app_id) = &cfg.app_id {
        body["app_id"] = json!(app_id);
    }
    if let Some(notes) = &opts.release_notes {
        body["update_desc"] = json!(notes);
    }
    session.post(&format!("{API}/resource/v1/app/upd"), &body)?;
    println!("  {} submitted {pkg} for review", "✓".green());
    Ok(())
}

pub fn status(creds: &OppoCreds, cfg: &OppoStoreConfig) -> Result<()> {
    let session = Session::login(creds)?;
    // TODO: verify OPPO status/query endpoint + response shape.
    let info = session.get(&format!(
        "{API}/resource/v1/app/info?pkg_name={}",
        cfg.package_name
    ))?;
    let state = info
        .pointer("/data/audit_status")
        .map(|v| v.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("OPPO 软件商店 {} audit status: {state}", cfg.package_name);
    Ok(())
}

/// Upload the APK bytes to a pre-signed URL; returns the URL/reference to bind.
/// TODO: verify OPPO upload transport (PUT vs multipart) and response shape.
fn upload_apk(upload_url: &str, artifact: &Path) -> Result<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(artifact)
        .with_context(|| format!("open {}", artifact.display()))?
        .read_to_end(&mut bytes)
        .with_context(|| format!("read {}", artifact.display()))?;
    let mut resp = http()
        .post(upload_url)
        .header("Content-Type", "application/vnd.android.package-archive")
        .send(bytes.as_slice())
        .map_err(|e| anyhow::anyhow!("OPPO upload failed: {e}"))?;
    let v: Value = resp
        .body_mut()
        .read_json()
        .context("parse upload response")?;
    v.pointer("/data/url")
        .or_else(|| v.get("url"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .context("OPPO upload response missing url")
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
