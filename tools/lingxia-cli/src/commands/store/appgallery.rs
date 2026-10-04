//! Huawei AppGallery Connect submission via the Publishing API.
//!
//! HarmonyOS 5+ flow: client-credentials token → OBS upload URL
//! (`/publish/v2/upload-url/for-obs`) → PUT the `.app`/`.hap` to OBS → bind
//! with `/publish/v3/app-package-info`. Review is started from the AGC console.
//!
//! The older `/publish/v2/upload-url` + multipart path is Android-oriented
//! and returns 204144645 for HarmonyOS 5 apps.

use super::processing::{Record, State};
use anyhow::{Context, Result, bail};
use colored::Colorize;
use serde_json::{Map, Value, json};
use std::path::Path;
use std::time::{Duration, Instant};

use super::backend::{SubmitOptions, http};
use crate::config::AppGalleryConfig;
use crate::platform::harmony::AgcApiCredentials;

const API: &str = "https://connect-api.cloud.huawei.com/api";

struct Session {
    token: String,
    client_id: String,
    refresh_at: Instant,
}

impl Session {
    fn login(creds: &AgcApiCredentials) -> Result<Self> {
        Self::login_with_budget(creds, Duration::from_secs(180))
    }

    fn login_with_budget(creds: &AgcApiCredentials, budget: Duration) -> Result<Self> {
        let body = json!({
            "grant_type": "client_credentials",
            "client_id": creds.client_id,
            "client_secret": creds.client_secret,
        });
        let mut resp = http()
            .post(&format!("{API}/oauth2/v1/token"))
            .config()
            .timeout_global(Some(budget))
            .build()
            .send_json(&body)
            .context("AppGallery token request failed")?;
        ensure_http_success(resp.status().as_u16())?;
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
            refresh_at: Instant::now()
                + Duration::from_secs(
                    v.get("expires_in")
                        .and_then(Value::as_u64)
                        .unwrap_or(300)
                        .saturating_sub(60)
                        .clamp(1, 86400),
                ),
        })
    }

    fn get(&self, url: &str) -> Result<Value> {
        self.get_with_budget(url, Duration::from_secs(180))
    }

    fn get_with_budget(&self, url: &str, budget: Duration) -> Result<Value> {
        let mut resp = http()
            .get(url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("client_id", &self.client_id)
            .config()
            .timeout_global(Some(budget))
            .build()
            .call()
            .context("AppGallery query failed")?;
        ensure_http_success(resp.status().as_u16())?;
        resp.body_mut().read_json().context("parse response")
    }

    fn put(&self, url: &str, body: &Value) -> Result<Value> {
        let mut resp = http()
            .put(url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("client_id", &self.client_id)
            .send_json(body)
            .map_err(|e| anyhow::anyhow!("PUT {url} failed: {e}"))?;
        ensure_http_success(resp.status().as_u16())?;
        resp.body_mut().read_json().context("parse response")
    }

    fn post(&self, url: &str, body: &Value) -> Result<Value> {
        let mut resp = http()
            .post(url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("client_id", &self.client_id)
            .send_json(body)
            .context("AppGallery test request failed")?;
        ensure_http_success(resp.status().as_u16())?;
        resp.body_mut()
            .read_json()
            .context("parse AppGallery test response")
    }
}

pub fn validate_submit_options(opts: &SubmitOptions, wait: bool) -> Result<()> {
    match opts.track.as_deref() {
        None | Some("production") => {
            if opts.test_version_id.is_some() {
                bail!("--test-version-id requires --track apptest");
            }
        }
        Some("apptest") => {
            if !wait {
                bail!(
                    "--track apptest requires --wait so the processed package can be bound to a test draft"
                );
            }
            if opts
                .test_version_id
                .as_deref()
                .is_some_and(|id| id.trim().is_empty())
            {
                bail!("AppTest version id must not be empty");
            }
            if opts
                .release_notes
                .as_deref()
                .is_some_and(|notes| notes.trim().is_empty() || notes.chars().count() > 50)
            {
                bail!(
                    "AppTest --release-notes must contain 1–50 characters (test version description)"
                );
            }
        }
        Some(track) => bail!("Unsupported Harmony track {track:?}; use production or apptest"),
    }
    Ok(())
}

pub fn submit(
    creds: &AgcApiCredentials,
    cfg: &AppGalleryConfig,
    artifact: &Path,
    opts: &SubmitOptions,
) -> Result<Record> {
    let app_id = &cfg.app_id;
    let session = Session::login(creds)?;
    eprintln!("  {} authenticated with AppGallery Connect", "✓".green());

    let file_name = artifact
        .file_name()
        .and_then(|n| n.to_str())
        .context("artifact has no file name")?;
    let content_length = std::fs::metadata(artifact)
        .with_context(|| format!("stat {}", artifact.display()))?
        .len();

    if artifact
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("hap"))
    {
        eprintln!(
            "{} AppGallery HarmonyOS 5 expects an .app package; a raw .hap may fail AGC parsing.",
            "Warning:".yellow()
        );
    }

    let up = session.get(&format!(
        "{API}/publish/v2/upload-url/for-obs?appId={app_id}&fileName={}&contentLength={content_length}",
        urlencode(file_name)
    ))?;
    require_agc_ok(&up, "get upload URL")?;
    let url_info = up
        .get("urlInfo")
        .context("AppGallery response missing urlInfo")?;
    let object_id = url_info
        .get("objectId")
        .and_then(Value::as_str)
        .context("urlInfo missing objectId")?
        .to_string();
    let upload_url = url_info
        .get("url")
        .and_then(Value::as_str)
        .context("urlInfo missing url")?;
    let headers = url_info.get("headers").and_then(Value::as_object);
    upload_to_obs(upload_url, headers, artifact)?;
    eprintln!("  {} uploaded {file_name}", "✓".green());

    let apptest = opts.track.as_deref() == Some("apptest");
    let bind = if apptest {
        session.post(
            &format!("{API}/publish/v2/test/version/pkg?appId={app_id}"),
            &apptest_package_body(file_name, &object_id),
        )?
    } else {
        session.put(
            &format!("{API}/publish/v3/app-package-info?appId={app_id}"),
            &json!({ "fileName": file_name, "objectId": object_id }),
        )?
    };
    require_agc_ok(&bind, "bind package")?;
    let mut record = Record::new(app_id, State::Uploaded);
    record.submission_id = if apptest {
        Some(apptest_package_id(&bind)?)
    } else {
        bind.get("packageId")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    eprintln!(
        "  bound file to app {app_id} (packageId {:?})",
        record.submission_id
    );
    Ok(record)
}

fn apptest_package_body(file_name: &str, object_id: &str) -> Value {
    json!({ "distributeMode": 1, "file": { "fileName": file_name, "objectId": object_id } })
}

fn apptest_package_id(body: &Value) -> Result<String> {
    let ids = body
        .get("pkgVersion")
        .and_then(Value::as_array)
        .context("AppTest response missing pkgVersion")?;
    if ids.len() != 1 {
        bail!("AppTest expected exactly one package id, got {}", ids.len());
    }
    Ok(ids[0]
        .as_str()
        .filter(|s| !s.is_empty())
        .context("AppTest package id missing")?
        .to_owned())
}

/// Prepare an invitation-test draft; review and tester notification remain explicit console actions.
pub fn prepare_apptest_version(
    creds: &AgcApiCredentials,
    cfg: &AppGalleryConfig,
    opts: &SubmitOptions,
    artifact: &Path,
) -> Result<String> {
    if let Some(id) = &opts.test_version_id {
        return Ok(id.clone());
    }
    let default_desc = artifact
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("LingXia AppTest")
        .chars()
        .take(50)
        .collect::<String>();
    let desc = opts.release_notes.as_deref().unwrap_or(&default_desc);
    let session = Session::login(creds)?;
    let response = session.post(
        &format!(
            "{API}/publish/v2/test/app/version?appId={}",
            urlencode(&cfg.app_id)
        ),
        &json!({ "releaseType": 6, "testType": 3, "testDesc": desc, "onshelfSelfDetect": 0 }),
    )?;
    require_agc_ok(&response, "create AppTest draft")?;
    Ok(response
        .get("versionId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .context("AppTest response missing versionId")?
        .to_owned())
}

pub fn bind_apptest_version(
    creds: &AgcApiCredentials,
    cfg: &AppGalleryConfig,
    version_id: &str,
    package_id: &str,
) -> Result<()> {
    let response = Session::login(creds)?.put(
        &format!(
            "{API}/publish/v2/test/app/version?appId={}",
            urlencode(&cfg.app_id)
        ),
        &json!({ "versionId": version_id, "pkgId": package_id }),
    )?;
    require_agc_ok(&response, "bind processed package to AppTest draft")?;
    eprintln!(
        "  {} package bound to AppTest draft {version_id}; configure testers and submit review in AppGallery Connect",
        "✓".green()
    );
    Ok(())
}

pub struct PackageQuery<'a> {
    creds: &'a AgcApiCredentials,
    cfg: &'a AppGalleryConfig,
    package_id: &'a str,
    session: Option<Session>,
}

impl<'a> PackageQuery<'a> {
    pub fn new(
        creds: &'a AgcApiCredentials,
        cfg: &'a AppGalleryConfig,
        package_id: &'a str,
    ) -> Self {
        Self {
            creds,
            cfg,
            package_id,
            session: None,
        }
    }

    pub fn query(&mut self, budget: Duration) -> Result<Record> {
        let started = Instant::now();
        if self
            .session
            .as_ref()
            .is_none_or(|s| Instant::now() >= s.refresh_at)
        {
            self.session = Some(Session::login_with_budget(self.creds, budget)?);
        }
        let remaining = budget.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(super::processing::failure(
                "STORE_REQUEST_RETRYABLE",
                "AGC authentication exhausted this query's time budget",
            ));
        }
        let body = self.session.as_ref().unwrap().get_with_budget(
            &format!(
                "{API}/publish/v3/package/compile/status?appId={}&pkgIds={}",
                urlencode(&self.cfg.app_id),
                urlencode(self.package_id)
            ),
            remaining,
        )?;
        parse_package_status(&body, &self.cfg.app_id, self.package_id)
    }
}

fn parse_package_status(body: &Value, app_id: &str, package_id: &str) -> Result<Record> {
    require_agc_ok(body, "query package processing")?;
    let packages = body
        .get("pkgStateList")
        .and_then(Value::as_array)
        .context("AGC response missing pkgStateList")?;
    let mut record = Record::new(app_id, State::Pending);
    record.submission_id = Some(package_id.to_owned());
    let matches = packages
        .iter()
        .filter(|pkg| pkg.get("pkgId").and_then(Value::as_str) == Some(package_id))
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        bail!("AGC returned duplicate package states");
    }
    if let Some(pkg) = matches.first() {
        let state = pkg.get("successStatus").and_then(Value::as_i64);
        record.raw_state = pkg.get("successStatus").map(Value::to_string);
        record.store_error_code = pkg
            .get("errorCode")
            .and_then(Value::as_str)
            .filter(|code| !code.is_empty() && *code != "0")
            .map(str::to_owned);
        // Huawei PackageStates: 0 = usable, 1 = parsing, 2 = unusable.
        record.state = match state {
            Some(0) => State::Complete,
            Some(1) => State::Processing,
            Some(2) => State::Failed,
            _ => State::Unknown,
        };
    }
    Ok(record)
}

pub fn query_app(creds: &AgcApiCredentials, cfg: &AppGalleryConfig) -> Result<Value> {
    let session = Session::login(creds)?;
    let info = session.get(&format!(
        "{API}/publish/v3/app-info?appId={}",
        urlencode(&cfg.app_id)
    ))?;
    require_agc_ok(&info, "query app info")?;
    Ok(
        serde_json::json!({"app_id": cfg.app_id, "release_state": info.pointer("/appInfo/releaseState")}),
    )
}

fn ensure_http_success(status: u16) -> Result<()> {
    super::processing::check_http_status(status)
}

pub fn status(creds: &AgcApiCredentials, cfg: &AppGalleryConfig) -> Result<()> {
    let session = Session::login(creds)?;
    let info = session.get(&format!("{API}/publish/v3/app-info?appId={}", cfg.app_id))?;
    require_agc_ok(&info, "query app info")?;
    let state = info
        .pointer("/appInfo/releaseState")
        .and_then(Value::as_i64)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    eprintln!("AppGallery app {} release state: {state}", cfg.app_id);
    Ok(())
}

fn require_agc_ok(v: &Value, what: &str) -> Result<()> {
    match v.pointer("/ret/code").and_then(Value::as_i64) {
        Some(0) => Ok(()),
        Some(code) => {
            let msg = v
                .pointer("/ret/msg")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            bail!("{what} failed: {code} {msg}")
        }
        None => bail!("{what} failed: response missing ret.code"),
    }
}

fn upload_to_obs(
    upload_url: &str,
    headers: Option<&Map<String, Value>>,
    artifact: &Path,
) -> Result<()> {
    let bytes = std::fs::read(artifact).with_context(|| format!("read {}", artifact.display()))?;
    eprintln!(
        "  {} uploading {} ({:.1} MB) to AppGallery OBS...",
        "→".dimmed(),
        artifact
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("package"),
        bytes.len() as f64 / (1024.0 * 1024.0)
    );
    let mut req = crate::http_client::create_agent(900).put(upload_url);
    if let Some(headers) = headers {
        for (key, value) in headers {
            if key.eq_ignore_ascii_case("host") {
                continue;
            }
            if let Some(value) = value.as_str() {
                req = req.header(key, value);
            }
        }
    }
    let mut resp = req
        .send(&bytes)
        .map_err(|e| anyhow::anyhow!("AppGallery OBS upload failed: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.body_mut().read_to_string().unwrap_or_default();
        bail!("AppGallery OBS upload HTTP {status}: {}", body.trim());
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apptest_upload_uses_test_area_and_exact_package_id() {
        assert_eq!(
            apptest_package_body("demo.app", "CN/demo.app"),
            json!({
                "distributeMode": 1, "file": {"fileName": "demo.app", "objectId": "CN/demo.app"}
            })
        );
        assert_eq!(
            apptest_package_id(&json!({"pkgVersion":["new"]})).unwrap(),
            "new"
        );
        for response in [
            json!({}),
            json!({"pkgVersion":[]}),
            json!({"pkgVersion":["old","new"]}),
            json!({"pkgVersion":[""]}),
        ] {
            assert!(apptest_package_id(&response).is_err());
        }
    }

    #[test]
    fn apptest_options_are_checked_before_upload() {
        let mut opts = SubmitOptions {
            track: Some("apptest".into()),
            ..Default::default()
        };
        assert!(validate_submit_options(&opts, true).is_ok());
        assert!(validate_submit_options(&opts, false).is_err());
        opts.release_notes = Some("汉".repeat(50));
        assert!(validate_submit_options(&opts, true).is_ok());
        opts.release_notes = Some("汉".repeat(51));
        assert!(validate_submit_options(&opts, true).is_err());
        opts.release_notes = None;
        opts.test_version_id = Some(" ".into());
        assert!(validate_submit_options(&opts, true).is_err());
        opts.test_version_id = Some("draft".into());
        opts.track = None;
        assert!(validate_submit_options(&opts, true).is_err());
        opts.test_version_id = None;
        opts.track = Some("typo".into());
        assert!(validate_submit_options(&opts, true).is_err());
    }

    #[test]
    fn http_failure_cannot_be_misread_as_successful_processing() {
        let session = Session {
            token: "test-token".into(),
            client_id: "test-client".into(),
            refresh_at: Instant::now(),
        };
        for status in [200, 403, 429, 503] {
            let (url, server) = super::super::processing::test_server(
                status,
                "{\"ret\":{\"code\":0},\"pkgStateList\":[{\"pkgId\":\"new\",\"successStatus\":0}]}",
            );
            let result = session.get_with_budget(
                &format!("{url}/publish/v3/package/compile/status?appId=app&pkgIds=new"),
                Duration::from_secs(5),
            );
            let request = server.join().unwrap();
            assert!(
                request.starts_with("GET /publish/v3/package/compile/status?appId=app&pkgIds=new ")
            );
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("client_id: test-client")
            );
            assert_eq!(result.is_ok(), status == 200);
        }
    }

    #[test]
    fn only_the_uploaded_package_can_complete() {
        for (raw, expected) in [
            (0, State::Complete),
            (1, State::Processing),
            (2, State::Failed),
            (99, State::Unknown),
        ] {
            let body = json!({"ret": {"code": 0}, "pkgStateList": [
                {"pkgId": "old", "successStatus": 0}, {"pkgId": "new", "successStatus": raw}]});
            let record = parse_package_status(&body, "app", "new").unwrap();
            assert_eq!(record.state, expected);
            assert_eq!(record.submission_id.as_deref(), Some("new"));
            assert_eq!(record.raw_state.as_deref(), Some(raw.to_string().as_str()));
        }
        let body =
            json!({"ret": {"code": 0}, "pkgStateList": [{"pkgId": "old", "successStatus": 0}]});
        assert_eq!(
            parse_package_status(&body, "app", "new").unwrap().state,
            State::Pending
        );
    }

    #[test]
    fn rejected_package_preserves_agc_error_code() {
        let body = json!({"ret": {"code": 0}, "pkgStateList": [
            {"pkgId": "new", "successStatus": 2, "errorCode": "991"}]});
        let record = parse_package_status(&body, "app", "new").unwrap();
        assert_eq!(record.state, State::Failed);
        assert_eq!(record.raw_state.as_deref(), Some("2"));
        assert_eq!(record.store_error_code.as_deref(), Some("991"));
        assert_eq!(
            serde_json::to_value(&record).unwrap()["store_error_code"],
            "991"
        );
    }

    #[test]
    fn malformed_and_error_responses_cannot_succeed() {
        for body in [
            json!({}),
            json!({"ret":{"code": 7}}),
            json!({"ret":{"code": 0}}),
            json!({"ret":{"code":0}, "pkgStateList":[{"pkgId":"new"},{"pkgId":"new"}]}),
        ] {
            assert!(parse_package_status(&body, "app", "new").is_err());
        }
        for state in [Value::Null, json!("0"), json!(false)] {
            let body =
                json!({"ret":{"code":0},"pkgStateList":[{"pkgId":"new","successStatus":state}]});
            assert_eq!(
                parse_package_status(&body, "app", "new").unwrap().state,
                State::Unknown
            );
        }
    }
}
