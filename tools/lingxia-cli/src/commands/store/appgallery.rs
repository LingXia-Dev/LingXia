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
}

pub fn submit(
    creds: &AgcApiCredentials,
    cfg: &AppGalleryConfig,
    artifact: &Path,
    _opts: &SubmitOptions,
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

    let bind = session.put(
        &format!("{API}/publish/v3/app-package-info?appId={app_id}"),
        &json!({ "fileName": file_name, "objectId": object_id }),
    )?;
    require_agc_ok(&bind, "bind package")?;
    let mut record = Record::new(app_id, State::Uploaded);
    record.submission_id = bind
        .get("packageId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    eprintln!(
        "  bound file to app {app_id} (packageId {:?})",
        record.submission_id
    );
    Ok(record)
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
