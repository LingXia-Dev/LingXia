//! Microsoft Store submission via the Partner Center Store submission API.
//!
//! Flow: Azure AD client-credentials token → get the app → create a submission
//! → upload a ZIP containing the package to the returned Azure blob SAS URL.
//! Existing drafts are preserved; a submission this run created is deleted
//! again when a later step fails. Review is started in Partner Center.
//!
//! NOT E2E-verified — needs a real Partner Center account. Implemented to the
//! documented API (https://learn.microsoft.com/windows/uwp/monetize/create-and-manage-submissions-using-windows-store-services).

use anyhow::{Context, Result, bail};
use colored::Colorize;
use serde_json::{Value, json};
use std::path::Path;

use super::processing::check_http_status;

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
    let archive = package_archive(artifact)?;
    let app_id = &cfg.app_id;
    let token = token(creds)?;
    println!("  {} authenticated with Partner Center", "✓".green());

    // Only one open submission is allowed. Preserve drafts created by users
    // or earlier uploads instead of silently deleting their work.
    let app = auth_get(&token, &format!("{API_BASE}/applications/{app_id}"))?;
    ensure_no_pending_submission(&app)?;

    // 2. Create a new submission (clones the last published one).
    let mut resp = http()
        .post(&format!("{API_BASE}/applications/{app_id}/submissions"))
        .header("Authorization", &format!("Bearer {token}"))
        .send("".as_bytes())
        .map_err(|e| anyhow::anyhow!("create submission failed: {e}"))?;
    check_http_status(resp.status().as_u16()).context("create submission")?;
    let mut submission: Value = resp.body_mut().read_json().context("parse submission")?;
    let submission_id = submission
        .get("id")
        .and_then(Value::as_str)
        .context("submission missing id")?
        .to_string();
    println!("  {} created submission {submission_id}", "✓".green());

    // From here on the pending draft is ours: a failure deletes it so a retry
    // is not stopped by the gate above.
    let submission_url = format!("{API_BASE}/applications/{app_id}/submissions/{submission_id}");
    let pkg_name = archive.name.as_str();
    let filled = fill_submission(
        &token,
        &submission_url,
        &mut submission,
        pkg_name,
        opts,
        archive.file.path(),
    );
    if let Err(err) = filled {
        return Err(discard_own_submission(err, &submission_id, || {
            delete_submission(&token, &submission_url)
        }));
    }
    println!("  {} uploaded {pkg_name}", "✓".green());

    println!(
        "  {} uploaded — commit the submission in Partner Center to send for review",
        "ℹ".blue()
    );
    Ok(())
}

/// Steps 3–4 on a submission this run created: reference the package and
/// optional release notes, PUT the metadata, then upload the archive to the
/// submission's Azure blob SAS URL.
fn fill_submission(
    token: &str,
    submission_url: &str,
    submission: &mut Value,
    pkg_name: &str,
    opts: &SubmitOptions,
    archive: &Path,
) -> Result<()> {
    let upload_url = submission
        .get("fileUploadUrl")
        .and_then(Value::as_str)
        .context("submission missing fileUploadUrl")?
        .to_string();
    set_package(submission, pkg_name);
    if let Some(notes) = &opts.release_notes {
        set_release_notes(submission, notes);
    }
    let resp = http()
        .put(submission_url)
        .header("Authorization", &format!("Bearer {token}"))
        .send_json(&*submission)
        .map_err(|e| anyhow::anyhow!("update submission metadata failed: {e}"))?;
    check_http_status(resp.status().as_u16()).context("update submission metadata")?;
    upload_package(&upload_url, archive)
}

fn delete_submission(token: &str, submission_url: &str) -> Result<()> {
    let resp = http()
        .delete(submission_url)
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|e| anyhow::anyhow!("delete submission failed: {e}"))?;
    check_http_status(resp.status().as_u16()).context("delete submission")
}

/// Turn a failure after this run created `submission_id` into the error the
/// user sees. The draft is deleted so a retry can create a fresh one; when even
/// that fails, the user must remove it in Partner Center. Pre-existing drafts
/// never reach here: `ensure_no_pending_submission` stops before one is made.
fn discard_own_submission(
    err: anyhow::Error,
    submission_id: &str,
    delete: impl FnOnce() -> Result<()>,
) -> anyhow::Error {
    match delete() {
        Ok(()) => anyhow::anyhow!(
            "{err:#}; the submission {submission_id} this upload created was deleted, so a \
             retry starts clean"
        ),
        Err(cleanup) => anyhow::anyhow!(
            "{err:#}; the submission {submission_id} this upload created could not be \
             deleted ({cleanup:#}); remove it in Partner Center before retrying"
        ),
    }
}

fn ensure_no_pending_submission(app: &Value) -> Result<()> {
    if let Some(pending) = app.get("pendingApplicationSubmission")
        && !pending.is_null()
    {
        let id = pending
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        bail!(
            "Microsoft Store already has pending submission {id}; finish or remove it \
             in Partner Center before uploading a new submission. The existing draft was preserved."
        );
    }
    Ok(())
}

/// The outer archive the SAS endpoint expects, holding the package under the
/// name the submission metadata references.
struct PackageArchive {
    /// Package file name: the archive's only entry and `applicationPackages[].fileName`.
    name: String,
    file: tempfile::NamedTempFile,
}

/// The SAS upload is an outer archive whose entry names match the submission
/// metadata. MSIX and MSIXUPLOAD are packages, even though they also use ZIP.
fn package_archive(artifact: &Path) -> Result<PackageArchive> {
    let name = artifact
        .file_name()
        .and_then(|name| name.to_str())
        .context("artifact has no file name")?
        .to_string();
    let mut source =
        std::fs::File::open(artifact).with_context(|| format!("open {}", artifact.display()))?;
    let mut file = tempfile::NamedTempFile::new().context("create Store upload archive")?;
    let mut writer = zip::ZipWriter::new(file.as_file_mut());
    // zip64: an MSIXUPLOAD can exceed 4 GiB.
    writer.start_file(
        &name,
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .large_file(true),
    )?;
    std::io::copy(&mut source, &mut writer).context("write Store package to upload archive")?;
    writer.finish()?;
    Ok(PackageArchive { name, file })
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

/// Upload the outer package archive as a single Azure block blob, streamed
/// from disk; ureq takes Content-Length from the file's metadata.
fn upload_package(upload_url: &str, archive: &Path) -> Result<()> {
    let file =
        std::fs::File::open(archive).with_context(|| format!("open {}", archive.display()))?;
    let resp = http()
        .put(upload_url)
        .header("x-ms-blob-type", "BlockBlob")
        .header("Content-Type", "application/zip")
        .send(file)
        .map_err(|e| anyhow::anyhow!("blob upload failed: {e}"))?;
    check_http_status(resp.status().as_u16()).context("blob upload")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn upload_archive_contains_the_package_referenced_by_metadata() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["app.msix", "app.msixupload"] {
            let artifact = dir.path().join(name);
            let payload = b"package contents";
            std::fs::write(&artifact, payload).unwrap();
            let archive = package_archive(&artifact).unwrap();
            assert_eq!(archive.name, name);
            let mut zip =
                zip::ZipArchive::new(std::fs::File::open(archive.file.path()).unwrap()).unwrap();
            assert_eq!(zip.len(), 1);
            let mut package = zip.by_name(name).unwrap();
            let mut actual = Vec::new();
            package.read_to_end(&mut actual).unwrap();
            assert_eq!(actual, payload);
            let mut submission = json!({});
            set_package(&mut submission, &archive.name);
            assert_eq!(submission["applicationPackages"][0]["fileName"], name);
        }
    }

    #[test]
    fn failed_upload_deletes_only_the_submission_it_created() {
        let mut deleted = false;
        let err = discard_own_submission(anyhow::anyhow!("blob upload failed"), "42", || {
            deleted = true;
            Ok(())
        });
        assert!(deleted);
        let msg = err.to_string();
        assert!(msg.starts_with("blob upload failed"));
        assert!(msg.contains("submission 42"));
        assert!(msg.contains("retry starts clean"));

        let err = discard_own_submission(anyhow::anyhow!("blob upload failed"), "42", || {
            Err(anyhow::anyhow!("Store returned HTTP 500"))
        });
        let msg = err.to_string();
        assert!(msg.starts_with("blob upload failed"));
        assert!(msg.contains("submission 42"));
        assert!(msg.contains("Store returned HTTP 500"));
        assert!(msg.contains("remove it in Partner Center"));
    }

    #[test]
    fn existing_drafts_block_new_submissions() {
        for pending in [json!({"id": "123"}), json!({})] {
            let err = ensure_no_pending_submission(&json!({
                "pendingApplicationSubmission": pending
            }))
            .unwrap_err()
            .to_string();
            assert!(err.contains("existing draft was preserved"));
        }
        ensure_no_pending_submission(&json!({"pendingApplicationSubmission": null})).unwrap();
        ensure_no_pending_submission(&json!({})).unwrap();
    }
}
