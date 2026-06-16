//! Huawei AppGallery Connect submission via the Publishing API.
//!
//! Flow: client-credentials token → get an upload URL → multipart-upload the
//! `.app`/`.hap` → bind the file to the app → submit for review.
//!
//! NOT E2E-verified — needs a real AppGallery Connect account. Implemented to
//! the documented Publishing API
//! (https://developer.huawei.com/consumer/en/doc/AppGallery-connect-Guides/agcapi-publishingapi).

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;

use super::backend::{SubmitOptions, http};
use super::creds::AppGalleryCreds;
use crate::config::AppGalleryConfig;

const API: &str = "https://connect-api.cloud.huawei.com/api";

struct Session {
    token: String,
    client_id: String,
}

impl Session {
    fn login(creds: &AppGalleryCreds) -> Result<Self> {
        let body = json!({
            "grant_type": "client_credentials",
            "client_id": creds.client_id,
            "client_secret": creds.client_secret,
        });
        let mut resp = http()
            .post(&format!("{API}/oauth2/v1/token"))
            .send_json(&body)
            .map_err(|e| anyhow::anyhow!("AppGallery token request failed: {e}"))?;
        let v: Value = resp
            .body_mut()
            .read_json()
            .context("parse token response")?;
        let token = v
            .get("access_token")
            .and_then(Value::as_str)
            .context("AppGallery response missing access_token")?
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
    creds: &AppGalleryCreds,
    cfg: &AppGalleryConfig,
    artifact: &Path,
    opts: &SubmitOptions,
) -> Result<()> {
    let app_id = &cfg.app_id;
    let session = Session::login(creds)?;
    println!("  {} authenticated with AppGallery Connect", "✓".green());

    let file_name = artifact
        .file_name()
        .and_then(|n| n.to_str())
        .context("artifact has no file name")?;
    let suffix = artifact
        .extension()
        .and_then(|e| e.to_str())
        .context("artifact has no extension")?;

    // 1. Get an upload URL + auth code.
    let up = session.get(&format!(
        "{API}/publish/v2/upload-url?appId={app_id}&suffix={suffix}"
    ))?;
    let upload_url = up
        .get("uploadUrl")
        .and_then(Value::as_str)
        .context("no uploadUrl in response")?;
    let auth_code = up
        .get("authCode")
        .and_then(Value::as_str)
        .context("no authCode in response")?;

    // 2. Multipart-upload the artifact.
    let dest = upload_file(upload_url, auth_code, artifact, file_name)?;
    println!("  {} uploaded {file_name}", "✓".green());

    // 3. Bind the uploaded file to the app.
    let bind = json!({
        "fileType": 5,
        "files": [{ "fileName": file_name, "fileDestUrl": dest }],
    });
    session.put(
        &format!("{API}/publish/v2/app-file-info?appId={app_id}"),
        &bind,
    )?;
    println!("  {} bound file to app {app_id}", "✓".green());

    if opts.draft {
        println!(
            "  {} draft uploaded — submit it in AppGallery Connect to send for review",
            "ℹ".blue()
        );
        return Ok(());
    }

    // 4. Submit for review (releaseType 1 = release now after approval).
    let mut url = format!("{API}/publish/v2/app-submit?appId={app_id}&releaseType=1");
    if let Some(notes) = &opts.release_notes {
        url.push_str(&format!("&remark={}", urlencode(notes)));
    }
    session.post(&url, &json!({}))?;
    println!("  {} submitted app {app_id} for review", "✓".green());
    Ok(())
}

pub fn status(creds: &AppGalleryCreds, cfg: &AppGalleryConfig) -> Result<()> {
    let session = Session::login(creds)?;
    let info = session.get(&format!("{API}/publish/v2/app-info?appId={}", cfg.app_id))?;
    let state = info
        .pointer("/appInfo/releaseState")
        .and_then(Value::as_i64)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("AppGallery app {} release state: {state}", cfg.app_id);
    Ok(())
}

/// Multipart-upload the artifact to the AGC upload URL; returns the
/// `fileDestUrl` used to bind it.
fn upload_file(
    upload_url: &str,
    auth_code: &str,
    artifact: &Path,
    file_name: &str,
) -> Result<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(artifact)
        .with_context(|| format!("open {}", artifact.display()))?
        .read_to_end(&mut bytes)
        .with_context(|| format!("read {}", artifact.display()))?;

    let boundary = format!("----LingXiaAGC{:x}", bytes.len());
    let mut body = Vec::new();
    let mut field = |name: &str, value: &str| {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    };
    field("authCode", auth_code);
    field("fileCount", "1");
    field("parseType", "1");
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: application/octet-stream\r\n\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let mut resp = http()
        .post(upload_url)
        .header(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .send(body.as_slice())
        .map_err(|e| anyhow::anyhow!("AppGallery upload failed: {e}"))?;
    let v: Value = resp
        .body_mut()
        .read_json()
        .context("parse upload response")?;
    v.pointer("/result/UploadFileRsp/fileInfoList/0/fileDestUlr")
        .or_else(|| v.pointer("/result/UploadFileRsp/fileInfoList/0/fileDestUrl"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .context("upload response missing fileDestUrl")
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
