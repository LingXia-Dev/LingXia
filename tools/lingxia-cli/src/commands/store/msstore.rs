//! Microsoft Store submission via the Partner Center Store submission API.
//!
//! Flow: Azure AD client-credentials token → get the app → remove any pending
//! submission → create a submission → upload the package zip to the returned
//! Azure blob SAS URL → commit → poll status.
//!
//! NOT E2E-verified — needs a real Partner Center account. Implemented to the
//! documented API (https://learn.microsoft.com/windows/uwp/monetize/create-and-manage-submissions-using-windows-store-services).

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;

use super::backend::{SubmitOptions, http};
use super::creds::MsStoreCreds;
use crate::config::MsStoreConfig;

const RESOURCE: &str = "https://manage.devcenter.microsoft.com";
const API_BASE: &str = "https://manage.devcenter.microsoft.com/v1.0/my";

fn token(creds: &MsStoreCreds) -> Result<String> {
    let url = format!(
        "https://login.microsoftonline.com/{}/oauth2/token",
        creds.tenant
    );
    let form = format!(
        "grant_type=client_credentials&client_id={}&client_secret={}&resource={}",
        urlencode(&creds.client_id),
        urlencode(&creds.client_secret),
        urlencode(RESOURCE)
    );
    let mut resp = http()
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(form.as_bytes())
        .map_err(|e| anyhow::anyhow!("Azure AD token request failed: {e}"))?;
    let body: Value = resp
        .body_mut()
        .read_json()
        .context("parse Azure AD token response")?;
    body.get("access_token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .context("Azure AD response missing access_token")
}

fn auth_get(token: &str, url: &str) -> Result<Value> {
    let mut resp = http()
        .get(url)
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|e| anyhow::anyhow!("GET {url} failed: {e}"))?;
    resp.body_mut().read_json().context("parse response")
}

pub fn submit(
    creds: &MsStoreCreds,
    cfg: &MsStoreConfig,
    artifact: &Path,
    opts: &SubmitOptions,
) -> Result<()> {
    let app_id = &cfg.app_id;
    let token = token(creds)?;
    println!("  {} authenticated with Partner Center", "✓".green());

    // 1. App + any pending submission (only one open submission is allowed).
    let app = auth_get(&token, &format!("{API_BASE}/applications/{app_id}"))?;
    if let Some(pending) = app
        .get("pendingApplicationSubmission")
        .and_then(|p| p.get("id"))
        .and_then(Value::as_str)
    {
        let url = format!("{API_BASE}/applications/{app_id}/submissions/{pending}");
        let _ = http()
            .delete(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .call();
        println!("  {} removed a pending submission", "✓".green());
    }

    // 2. Create a new submission (clones the last published one).
    let mut resp = http()
        .post(&format!("{API_BASE}/applications/{app_id}/submissions"))
        .header("Authorization", &format!("Bearer {token}"))
        .send("".as_bytes())
        .map_err(|e| anyhow::anyhow!("create submission failed: {e}"))?;
    let mut submission: Value = resp.body_mut().read_json().context("parse submission")?;
    let submission_id = submission
        .get("id")
        .and_then(Value::as_str)
        .context("submission missing id")?
        .to_string();
    let upload_url = submission
        .get("fileUploadUrl")
        .and_then(Value::as_str)
        .context("submission missing fileUploadUrl")?
        .to_string();
    println!("  {} created submission {submission_id}", "✓".green());

    // 3. Reference the package + optional release notes, then PUT the metadata.
    let pkg_name = artifact
        .file_name()
        .and_then(|n| n.to_str())
        .context("artifact has no file name")?;
    set_package(&mut submission, pkg_name);
    if let Some(notes) = &opts.release_notes {
        set_release_notes(&mut submission, notes);
    }
    http()
        .put(&format!(
            "{API_BASE}/applications/{app_id}/submissions/{submission_id}"
        ))
        .header("Authorization", &format!("Bearer {token}"))
        .send_json(&submission)
        .map_err(|e| anyhow::anyhow!("update submission metadata failed: {e}"))?;

    // 4. Upload the package zip to the Azure blob SAS URL (block blob).
    upload_package(&upload_url, artifact)?;
    println!("  {} uploaded {pkg_name}", "✓".green());

    if opts.draft {
        println!(
            "  {} draft created — commit it in Partner Center to send for review",
            "ℹ".blue()
        );
        return Ok(());
    }

    // 5. Commit for processing/certification.
    http()
        .post(&format!(
            "{API_BASE}/applications/{app_id}/submissions/{submission_id}/commit"
        ))
        .header("Authorization", &format!("Bearer {token}"))
        .send("".as_bytes())
        .map_err(|e| anyhow::anyhow!("commit submission failed: {e}"))?;
    println!("  {} committed submission {submission_id}", "✓".green());
    Ok(())
}

pub fn status(creds: &MsStoreCreds, cfg: &MsStoreConfig) -> Result<()> {
    let app_id = &cfg.app_id;
    let token = token(creds)?;
    let app = auth_get(&token, &format!("{API_BASE}/applications/{app_id}"))?;
    let Some(sub_id) = app
        .get("pendingApplicationSubmission")
        .and_then(|p| p.get("id"))
        .and_then(Value::as_str)
    else {
        println!("No pending Microsoft Store submission.");
        return Ok(());
    };
    let st = auth_get(
        &token,
        &format!("{API_BASE}/applications/{app_id}/submissions/{sub_id}/status"),
    )?;
    let status = st
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    println!("Microsoft Store submission {sub_id}: {status}");
    Ok(())
}

fn set_package(submission: &mut Value, pkg_name: &str) {
    // Mark any existing packages for deletion and add the new one as
    // PendingUpload, per the Store submission API.
    if let Some(existing) = submission
        .get_mut("applicationPackages")
        .and_then(Value::as_array_mut)
    {
        for p in existing.iter_mut() {
            if let Some(obj) = p.as_object_mut() {
                obj.insert("fileStatus".into(), json!("PendingDelete"));
            }
        }
        existing.push(json!({ "fileName": pkg_name, "fileStatus": "PendingUpload" }));
    } else {
        submission["applicationPackages"] =
            json!([{ "fileName": pkg_name, "fileStatus": "PendingUpload" }]);
    }
}

fn set_release_notes(submission: &mut Value, notes: &str) {
    if let Some(listings) = submission
        .get_mut("listings")
        .and_then(Value::as_object_mut)
    {
        for (_lang, listing) in listings.iter_mut() {
            if let Some(base) = listing
                .get_mut("baseListing")
                .and_then(Value::as_object_mut)
            {
                base.insert("releaseNotes".into(), json!(notes));
            }
        }
    }
}

/// Upload the package to the Azure blob SAS URL as a single block blob. The
/// Store API expects a zip; a `.msixupload` is already a zip container, so it is
/// uploaded as-is.
fn upload_package(upload_url: &str, artifact: &Path) -> Result<()> {
    let mut bytes = Vec::new();
    std::fs::File::open(artifact)
        .with_context(|| format!("open {}", artifact.display()))?
        .read_to_end(&mut bytes)
        .with_context(|| format!("read {}", artifact.display()))?;
    http()
        .put(upload_url)
        .header("x-ms-blob-type", "BlockBlob")
        .header("Content-Type", "application/zip")
        .send(bytes.as_slice())
        .map_err(|e| anyhow::anyhow!("blob upload failed: {e}"))?;
    Ok(())
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
