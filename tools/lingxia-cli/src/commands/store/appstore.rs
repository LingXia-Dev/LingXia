//! App Store Connect submission.
//!
//! The binary is uploaded with `xcrun altool` (the supported transport; needs
//! Xcode, macOS only). Status is read from the App Store Connect API using an
//! ES256 JWT minted locally from the `.p8` key.
//!
//! NOT E2E-verified — needs a real App Store Connect account + Xcode.

use super::processing::{BuildSelection, Record, State};
use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use colored::Colorize;
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::backend::{StorePlatform, SubmitOptions, http};
use crate::resolver::AscMaterial;

const ASC_BASE: &str = "https://api.appstoreconnect.apple.com";

/// Mint a short-lived (≤20 min) ES256 JWT for the App Store Connect API.
pub fn asc_jwt(creds: &AscMaterial) -> Result<String> {
    use p256::ecdsa::{Signature, SigningKey, signature::Signer};
    use p256::pkcs8::DecodePrivateKey;

    let signing_key = SigningKey::from_pkcs8_pem(&creds.private_key_pem)
        .context("parse ASC .p8 (expected PKCS#8 EC P-256)")?;

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
    creds: &AscMaterial,
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

    eprintln!("  uploading {} via altool…", artifact.display());
    let status = Command::new("xcrun")
        .args(["altool", "--upload-app", "--type", type_arg, "--file"])
        .arg(artifact)
        .args(["--apiKey", &creds.key_id, "--apiIssuer", &creds.issuer_id])
        .stdout(Stdio::from(std::io::stderr()))
        .status()
        .context("run xcrun altool")?;
    if !status.success() {
        bail!("altool upload failed (exit {:?})", status.code());
    }
    eprintln!("  {} uploaded to App Store Connect", "✓".green());
    eprintln!(
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

pub fn status(creds: &AscMaterial, bundle_id: &str) -> Result<()> {
    let app_id = resolve_app_id(creds, bundle_id)?;
    for record in query_builds(
        creds,
        &app_id,
        StorePlatform::Ios,
        &BuildSelection::default(),
        None,
        Duration::from_secs(180),
    )? {
        println!(
            "  build {}: {:?}",
            record.build_number.as_deref().unwrap_or("?"),
            record.state
        );
    }
    Ok(())
}

pub fn resolve_app_id(creds: &AscMaterial, bundle_id: &str) -> Result<String> {
    let url = format!(
        "{ASC_BASE}/v1/apps?filter[bundleId]={}&limit=2",
        urlencoding::encode(bundle_id)
    );
    let body = get_json(creds, &url, Duration::from_secs(180))?;
    let apps = body
        .get("data")
        .and_then(Value::as_array)
        .context("ASC response missing data")?;
    if apps.len() != 1 {
        bail!(
            "Expected one App Store Connect app for {bundle_id}, found {}",
            apps.len()
        );
    }
    Ok(apps[0]
        .get("id")
        .and_then(Value::as_str)
        .context("ASC app missing id")?
        .to_owned())
}

fn get_json(creds: &AscMaterial, url: &str, budget: Duration) -> Result<Value> {
    let jwt = asc_jwt(creds)?;
    let mut response = http()
        .get(url)
        .header("Authorization", &format!("Bearer {jwt}"))
        .config()
        .timeout_global(Some(budget))
        .build()
        .call()
        .context("App Store Connect query failed")?;
    super::processing::check_http_status(response.status().as_u16())?;
    response
        .body_mut()
        .read_json()
        .context("parse App Store Connect response")
}

fn builds_url(
    app_id: &str,
    platform: StorePlatform,
    selection: &BuildSelection,
    id: Option<&str>,
) -> String {
    let platform = if platform == StorePlatform::Macos {
        "MAC_OS"
    } else {
        "IOS"
    };
    let mut url = format!(
        "{ASC_BASE}/v1/builds?filter[app]={}&filter[preReleaseVersion.platform]={platform}&include=preReleaseVersion&limit=5&sort=-uploadedDate",
        urlencoding::encode(app_id)
    );
    for (key, value) in [
        ("version", selection.build_number.as_deref()),
        ("preReleaseVersion.version", selection.version.as_deref()),
        ("id", id),
    ] {
        if let Some(value) = value {
            url.push_str(&format!("&filter[{key}]={}", urlencoding::encode(value)));
        }
    }
    url
}

pub fn query_builds(
    creds: &AscMaterial,
    app_id: &str,
    platform: StorePlatform,
    selection: &BuildSelection,
    id: Option<&str>,
    budget: Duration,
) -> Result<Vec<Record>> {
    let body = get_json(creds, &builds_url(app_id, platform, selection, id), budget)?;
    parse_builds(&body, app_id)
}

fn parse_builds(body: &Value, app_id: &str) -> Result<Vec<Record>> {
    let builds = body
        .get("data")
        .and_then(Value::as_array)
        .context("ASC response missing builds data")?;
    builds
        .iter()
        .map(|build| {
            let mut record = Record::new(app_id, State::Unknown);
            record.submission_id = Some(
                build
                    .get("id")
                    .and_then(Value::as_str)
                    .context("ASC build missing id")?
                    .to_owned(),
            );
            record.build_number = build
                .pointer("/attributes/version")
                .and_then(Value::as_str)
                .map(str::to_owned);
            record.raw_state = build
                .pointer("/attributes/processingState")
                .and_then(Value::as_str)
                .map(str::to_owned);
            record.state = match record.raw_state.as_deref() {
                Some("PROCESSING") => State::Processing,
                Some("VALID") => State::Complete,
                Some("FAILED" | "INVALID") => State::Failed,
                _ => State::Unknown,
            };
            let release_id = build
                .pointer("/relationships/preReleaseVersion/data/id")
                .and_then(Value::as_str);
            record.version = body
                .get("included")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find(|item| {
                    item.get("type").and_then(Value::as_str) == Some("preReleaseVersions")
                        && item.get("id").and_then(Value::as_str) == release_id
                })
                .and_then(|item| item.pointer("/attributes/version"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            Ok(record)
        })
        .collect()
}

pub fn query_one(
    creds: &AscMaterial,
    app_id: &str,
    platform: StorePlatform,
    selection: &BuildSelection,
    id: Option<&str>,
    budget: Duration,
) -> Result<Record> {
    let mut builds = query_builds(creds, app_id, platform, selection, id, budget)?;
    if builds.len() > 1 {
        bail!(
            "Multiple builds matched; specify the submission id or exact version and build number"
        );
    }
    Ok(builds.pop().unwrap_or_else(|| {
        let mut record = Record::new(app_id, State::Pending);
        record.submission_id = id.map(str::to_owned);
        record.version = selection.version.clone();
        record.build_number = selection.build_number.clone();
        record
    }))
}

pub fn upload_selection(artifact: &Path, requested: &BuildSelection) -> Result<BuildSelection> {
    if artifact
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("ipa"))
    {
        let actual = super::artifact_identity::ipa_build_selection(artifact)?;
        if requested
            .version
            .as_ref()
            .is_some_and(|v| Some(v) != actual.version.as_ref())
            || requested
                .build_number
                .as_ref()
                .is_some_and(|v| Some(v) != actual.build_number.as_ref())
        {
            bail!("Requested version/build number does not match the IPA");
        }
        Ok(actual)
    } else {
        Ok(requested.clone())
    }
}

/// altool searches `~/.appstoreconnect/private_keys/AuthKey_<id>.p8`; make sure
/// the resolved key is there.
fn stage_private_key(creds: &AscMaterial) -> Result<()> {
    let dir = dirs::home_dir()
        .context("home dir")?
        .join(".appstoreconnect")
        .join("private_keys");
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let dst = dir.join(format!("AuthKey_{}.p8", creds.key_id));
    std::fs::write(&dst, creds.private_key_pem.as_bytes())
        .with_context(|| format!("stage {}", dst.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_status_is_checked_before_accepting_a_success_body() {
        use p256::pkcs8::EncodePrivateKey;
        let key = p256::SecretKey::from_slice(&[1; 32]).unwrap();
        let creds = AscMaterial {
            key_id: "test".into(),
            issuer_id: "test".into(),
            private_key_pem: key
                .to_pkcs8_pem(p256::pkcs8::LineEnding::LF)
                .unwrap()
                .to_string(),
        };
        for status in [200, 401, 429, 503] {
            let (url, server) = super::super::processing::test_server(status, "{\"data\":[]}");
            let result = get_json(&creds, &format!("{url}/v1/builds"), Duration::from_secs(5));
            let request = server.join().unwrap();
            assert!(request.starts_with("GET /v1/builds "));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer ")
            );
            assert_eq!(result.is_ok(), status == 200);
        }
    }

    #[test]
    fn query_filters_exact_app_platform_version_and_build() {
        let selection = BuildSelection {
            version: Some("2.0".into()),
            build_number: Some("42".into()),
        };
        let url = builds_url("123", StorePlatform::Ios, &selection, None);
        assert!(url.contains("filter[app]=123"));
        assert!(url.contains("filter[preReleaseVersion.platform]=IOS"));
        assert!(url.contains("filter[preReleaseVersion.version]=2.0"));
        assert!(url.contains("filter[version]=42"));
        assert!(!url.contains("app.bundleId"));
        let url = builds_url(
            "123",
            StorePlatform::Macos,
            &BuildSelection::default(),
            Some("id/with?chars"),
        );
        assert!(url.contains("platform]=MAC_OS"));
        assert!(url.contains("filter[id]=id%2Fwith%3Fchars"));
    }

    #[test]
    fn distinguishes_processing_failure_success_and_unknown() {
        for (raw, expected) in [
            ("PROCESSING", State::Processing),
            ("VALID", State::Complete),
            ("FAILED", State::Failed),
            ("INVALID", State::Failed),
            ("NEW_STATE", State::Unknown),
        ] {
            let body = json!({"data": [{"id": "build-id", "attributes": {"version": "42", "processingState": raw},
                "relationships": {"preReleaseVersion": {"data": {"id": "v2"}}}}],
                "included": [{"type": "preReleaseVersions", "id": "v2", "attributes": {"version": "2.0"}}]});
            let records = parse_builds(&body, "app-id").unwrap();
            assert_eq!(records[0].state, expected);
            assert_eq!(records[0].submission_id.as_deref(), Some("build-id"));
            assert_eq!(records[0].version.as_deref(), Some("2.0"));
            assert_eq!(records[0].build_number.as_deref(), Some("42"));
        }
        assert!(parse_builds(&json!({"errors": []}), "app").is_err());
        assert!(
            parse_builds(&json!({"data": []}), "app")
                .unwrap()
                .is_empty()
        );
    }
}
