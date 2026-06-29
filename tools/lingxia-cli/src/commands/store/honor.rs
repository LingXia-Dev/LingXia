//! Honor AppGallery (荣耀应用市场) submission via the Honor Developer open
//! platform's publishing API.
//!
//! Flow: client-credentials token → get an upload URL → upload the `.apk` →
//! bind the file to the app → submit for review. Honor's console descends from
//! Huawei AGC, so the shape mirrors `appgallery.rs`.
//!
//! NOT E2E-verified — needs a real Honor developer account. The exact
//! open-platform endpoints/payloads are not publicly stable, so uncertain steps
//! are marked with `// TODO: verify ...` rather than fabricated.

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;

use super::backend::{SubmitOptions, http};
use super::creds::HonorCreds;
use crate::config::HonorStoreConfig;

// TODO: verify Honor open-platform API base host/path.
const API: &str = "https://appmarket-openapi-drcn.cloud.honor.com/openapi/v1";

struct Session {
    token: String,
    client_id: String,
}

impl Session {
    fn login(creds: &HonorCreds) -> Result<Self> {
        // TODO: verify Honor auth endpoint + grant type.
        let body = json!({
            "grant_type": "client_credentials",
            "client_id": creds.client_id,
            "client_secret": creds.client_secret,
        });
        let mut resp = http()
            .post(&format!("{API}/oauth2/token"))
            .send_json(&body)
            .map_err(|e| anyhow::anyhow!("Honor token request failed: {e}"))?;
        let v: Value = resp
            .body_mut()
            .read_json()
            .context("parse token response")?;
        let token = v
            .get("access_token")
            .and_then(Value::as_str)
            .context("Honor response missing access_token")?
            .to_string();
        Ok(Self {
            token,
            client_id: creds.client_id.clone(),
        })
    }

    fn get(&self, url: &str) -> Result<Value> {
        let mut resp = http()
            .get(url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("client_id", &self.client_id)
            .call()
            .map_err(|e| anyhow::anyhow!("GET {url} failed: {e}"))?;
        resp.body_mut().read_json().context("parse response")
    }

    fn post(&self, url: &str, body: &Value) -> Result<Value> {
        let mut resp = http()
            .post(url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("client_id", &self.client_id)
            .send_json(body)
            .map_err(|e| anyhow::anyhow!("POST {url} failed: {e}"))?;
        resp.body_mut().read_json().context("parse response")
    }

    fn put(&self, url: &str, body: &Value) -> Result<Value> {
        let mut resp = http()
            .put(url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("client_id", &self.client_id)
            .send_json(body)
            .map_err(|e| anyhow::anyhow!("PUT {url} failed: {e}"))?;
        resp.body_mut().read_json().context("parse response")
    }
}

pub fn submit(
    creds: &HonorCreds,
    cfg: &HonorStoreConfig,
    artifact: &Path,
    opts: &SubmitOptions,
) -> Result<()> {
    let app_id = &cfg.app_id;
    let session = Session::login(creds)?;
    println!("  {} authenticated with Honor AppGallery", "✓".green());

    let file_name = artifact
        .file_name()
        .and_then(|n| n.to_str())
        .context("artifact has no file name")?;

    // 1. Get an upload URL.
    // TODO: verify Honor upload-url endpoint + query params.
    let up = session.get(&format!("{API}/publish/upload-url?appId={app_id}"))?;
    let upload_url = up
        .pointer("/data/uploadUrl")
        .or_else(|| up.get("uploadUrl"))
        .and_then(Value::as_str)
        .context("Honor response missing uploadUrl")?;

    // 2. Upload the APK.
    // TODO: verify Honor upload transport + response shape.
    let dest = upload_apk(upload_url, artifact)?;
    println!("  {} uploaded {file_name}", "✓".green());

    // 3. Bind the uploaded file to the app.
    // TODO: verify Honor file-bind endpoint + payload.
    session.put(
        &format!("{API}/publish/app-file-info?appId={app_id}"),
        &json!({ "files": [{ "fileName": file_name, "fileDestUrl": dest }] }),
    )?;
    println!("  {} bound file to app {app_id}", "✓".green());

    if opts.draft {
        println!(
            "  {} draft uploaded — submit it in the Honor console to publish",
            "ℹ".blue()
        );
        return Ok(());
    }

    // 4. Submit for review.
    // TODO: verify Honor app-submit endpoint + payload (release notes field).
    let mut body = json!({ "appId": app_id });
    if let Some(notes) = &opts.release_notes {
        body["remark"] = json!(notes);
    }
    session.post(&format!("{API}/publish/app-submit?appId={app_id}"), &body)?;
    println!("  {} submitted app {app_id} for review", "✓".green());
    Ok(())
}

pub fn status(creds: &HonorCreds, cfg: &HonorStoreConfig) -> Result<()> {
    let session = Session::login(creds)?;
    // TODO: verify Honor status/query endpoint + response shape.
    let info = session.get(&format!("{API}/publish/app-info?appId={}", cfg.app_id))?;
    let state = info
        .pointer("/data/releaseState")
        .map(|v| v.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("Honor AppGallery app {} release state: {state}", cfg.app_id);
    Ok(())
}

/// Upload the APK bytes to a pre-signed URL; returns the reference to bind.
/// TODO: verify Honor upload transport (multipart vs raw) and response shape.
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
        .map_err(|e| anyhow::anyhow!("Honor upload failed: {e}"))?;
    let v: Value = resp
        .body_mut()
        .read_json()
        .context("parse upload response")?;
    v.pointer("/data/fileDestUrl")
        .or_else(|| v.get("fileDestUrl"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .context("Honor upload response missing fileDestUrl")
}
