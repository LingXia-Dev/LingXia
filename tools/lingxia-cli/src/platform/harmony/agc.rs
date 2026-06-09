#![cfg_attr(target_os = "windows", allow(dead_code))]

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const AGC_TOKEN_URL: &str = "https://connect-api.cloud.huawei.com/api/oauth2/v1/token";
const AGC_PROVISION_API_BASE: &str = "https://connect-api.cloud.huawei.com/api/publish";
const AGC_CONNECT_API_HOST: &str = "https://connect-api.cloud.huawei.com";

pub struct AgcConnectClient {
    http_agent: ureq::Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgcToken {
    pub access_token: String,
    pub expires_at: i64,
    pub client_id: String,
}

#[derive(Debug, Deserialize)]
struct AgcTokenResponse {
    access_token: String,
    expires_in: i64,
    #[serde(default)]
    token_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgcApiCredentials {
    pub client_id: String,
    pub client_secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<AgcToken>,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub id: String,
    pub device_name: String,
    pub udid: String,
    pub device_type: i32,
}

#[derive(Debug, Clone)]
pub struct CertInfo {
    pub id: String,
    pub cert_name: String,
    pub cert_type: i32,
    pub cert_download_url: String,
}

#[derive(Debug, Clone)]
pub struct ProvisionInfo {
    pub id: String,
    pub provision_name: String,
    pub provision_type: i32,
    pub cert_id: String,
    pub cert_name: String,
    pub app_id: String,
    pub device_ids: Vec<String>,
    pub acl_permissions: Vec<String>,
    pub provision_download_url: String,
}

#[derive(Debug, Clone)]
pub struct AppIdInfo {
    pub app_id: String,
    pub package_name: String,
    pub app_name: String,
}

#[derive(Debug, Clone)]
pub struct CreateProfileParams {
    pub name: String,
    pub app_id: String,
    pub cert_id: String,
    pub device_ids: Vec<String>,
    pub acl_permissions: Vec<String>,
    pub is_debug: bool,
}

#[derive(Debug, Serialize)]
struct AddDeviceRequest {
    #[serde(rename = "deviceList")]
    device_list: Vec<AddDeviceInfo>,
}

#[derive(Debug, Serialize)]
struct AddDeviceInfo {
    #[serde(rename = "deviceName")]
    device_name: String,
    udid: String,
    #[serde(rename = "deviceType")]
    device_type: i32,
}

#[derive(Debug, Serialize)]
struct CertListRequest {
    #[serde(rename = "certType")]
    cert_type: i32,
}

#[derive(Debug, Serialize)]
struct CreateCertRequest {
    #[serde(rename = "certName")]
    cert_name: String,
    #[serde(rename = "certType")]
    cert_type: i32,
    csr: String,
}

#[derive(Debug, Serialize)]
struct DeleteCertRequest {
    #[serde(rename = "certIds")]
    cert_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CreateProfileRequest {
    #[serde(rename = "provisionName")]
    provision_name: String,
    #[serde(rename = "provisionType")]
    provision_type: i32,
    #[serde(rename = "certId")]
    cert_id: String,
    #[serde(rename = "appId")]
    app_id: String,
    #[serde(rename = "deviceIdList")]
    device_id_list: Vec<String>,
    #[serde(rename = "aclPermissionList")]
    acl_permission_list: Vec<String>,
}

impl AgcConnectClient {
    pub fn new() -> Self {
        Self {
            http_agent: crate::http_client::shared_native_roots_agent().clone(),
        }
    }

    pub fn get_token(&self, client_id: &str, client_secret: &str) -> Result<AgcToken> {
        let request_body = serde_json::json!({
            "grant_type": "client_credentials",
            "client_id": client_id,
            "client_secret": client_secret
        });

        let body_string =
            serde_json::to_string(&request_body).context("Failed to serialize token request")?;

        let response = self
            .http_agent
            .post(AGC_TOKEN_URL)
            .header("Content-Type", "application/json")
            .send(body_string.as_bytes())
            .context("Failed to request AGC access token")?;

        let body_text = response
            .into_body()
            .read_to_string()
            .context("Failed to read AGC token response")?;

        let root: Value =
            serde_json::from_str(&body_text).context("Failed to parse AGC token response")?;
        if root.get("access_token").is_none() {
            if let Some(ret) = root.get("ret") {
                let code = ret.get("code").and_then(value_to_i32).unwrap_or_default();
                let msg = ret
                    .get("msg")
                    .and_then(value_to_string)
                    .unwrap_or_else(|| "unknown error".to_string());
                let mut detail = format!("AGC token request failed ({code}): {msg}");
                if msg
                    .to_ascii_lowercase()
                    .contains("type of clientid not match")
                {
                    detail.push_str(
                        "\nHint: this client_id is not a valid Connect API client for Publishing/Provisioning.\n\
Use credentials from AGC `API密钥 > Connect API > API客户端`, and set Project to `N/A` (team-level).",
                    );
                }
                return Err(anyhow!(detail));
            }
            return Err(anyhow!(
                "AGC token response missing access_token. Raw response: {}",
                body_text
            ));
        }

        let token_response: AgcTokenResponse =
            serde_json::from_value(root).context("Failed to parse AGC token response fields")?;

        if let Some(token_type) = token_response.token_type.as_deref()
            && !token_type.eq_ignore_ascii_case("bearer")
        {
            return Err(anyhow!("Unexpected token type: {}", token_type));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        Ok(AgcToken {
            access_token: token_response.access_token,
            expires_at: now + token_response.expires_in,
            client_id: client_id.to_string(),
        })
    }

    pub fn is_token_expired(token: &AgcToken) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        token.expires_at <= now + 300
    }

    pub fn ensure_valid_token(&self, credentials: &AgcApiCredentials) -> Result<AgcToken> {
        if let Some(ref token) = credentials.token
            && !Self::is_token_expired(token)
        {
            return Ok(token.clone());
        }

        self.get_token(&credentials.client_id, &credentials.client_secret)
    }

    fn request_json<T: Serialize>(
        &self,
        method: &str,
        path: &str,
        token: &AgcToken,
        body: Option<&T>,
    ) -> Result<Value> {
        self.request_json_with_headers(method, path, token, body, &[])
    }

    fn request_json_with_headers<T: Serialize>(
        &self,
        method: &str,
        path: &str,
        token: &AgcToken,
        body: Option<&T>,
        extra_headers: &[(&str, String)],
    ) -> Result<Value> {
        let url = format!("{}{}", AGC_PROVISION_API_BASE, path);
        let auth_header = format!("Bearer {}", token.access_token);
        let mut headers = vec![
            ("Authorization", auth_header.as_str()),
            ("client_id", token.client_id.as_str()),
            ("Content-Type", "application/json"),
            ("Accept", "application/json"),
        ];
        for (name, value) in extra_headers {
            headers.push((*name, value.as_str()));
        }

        let response = match method {
            "GET" | "DELETE" => {
                crate::http_client::call_with_headers(&self.http_agent, method, &url, &headers)
            }
            "POST" | "PUT" => {
                if let Some(payload) = body {
                    let body_json =
                        serde_json::to_string(payload).context("Failed to serialize request")?;
                    crate::http_client::send_bytes_with_headers(
                        &self.http_agent,
                        method,
                        &url,
                        &headers,
                        body_json.as_bytes(),
                    )
                } else {
                    crate::http_client::send_bytes_with_headers(
                        &self.http_agent,
                        method,
                        &url,
                        &headers,
                        &[],
                    )
                }
            }
            _ => return Err(anyhow!("Unsupported HTTP method: {}", method)),
        };
        let response = match response {
            Ok(resp) => resp,
            Err(err) => {
                let mut message = format!("Failed request to {}", url);
                let text = err.to_string();
                if text.to_ascii_lowercase().contains("connection refused") {
                    message.push_str(
                        "\nHint: network proxy may be unreachable. Try unsetting `http_proxy`/`https_proxy`/`ALL_PROXY` and retry.",
                    );
                }
                return Err(anyhow!("{message}\nCaused by: {err}"));
            }
        };

        let status = response.status();
        let status_code = status.as_u16();
        let status_text = status.canonical_reason().unwrap_or("Unknown").to_string();
        let body = response
            .into_body()
            .read_to_string()
            .context("Failed to read response body")?;

        if status_code >= 400 {
            return Err(format_agc_http_error(
                status_code,
                &status_text,
                &url,
                &body,
            ));
        }

        if body.trim().is_empty() {
            return Ok(Value::Null);
        }

        serde_json::from_str(&body)
            .with_context(|| format!("Failed to parse AGC JSON response from {}", url))
    }

    pub fn query_devices(
        &self,
        token: &AgcToken,
        device_name: Option<&str>,
    ) -> Result<Vec<DeviceInfo>> {
        let mut path = "/v2/device/list?maxReqCount=100".to_string();
        if let Some(name) = device_name {
            path.push_str("&deviceName=");
            path.push_str(&urlencoding::encode(name));
        }
        let root = self.request_json::<()>("GET", &path, token, None)?;
        ensure_success_response(&root)?;

        let mut devices = Vec::new();
        for item in array_field(&root, &["deviceList", "device_list", "list"]) {
            let id = string_field(item, &["id", "deviceId"]).unwrap_or_default();
            let udid =
                string_field(item, &["udid", "deviceUdid", "serialNumber"]).unwrap_or_default();
            if id.is_empty() || udid.is_empty() {
                continue;
            }
            devices.push(DeviceInfo {
                id,
                device_name: string_field(item, &["deviceName", "name"])
                    .unwrap_or_else(|| "Harmony Device".to_string()),
                udid,
                device_type: int_field(item, &["deviceType"], 1),
            });
        }

        Ok(devices)
    }

    pub fn add_device(&self, token: &AgcToken, device_name: &str, udid: &str) -> Result<()> {
        let req = AddDeviceRequest {
            device_list: vec![AddDeviceInfo {
                device_name: device_name.to_string(),
                udid: udid.to_string(),
                device_type: 1,
            }],
        };

        let root = self.request_json("POST", "/v2/device", token, Some(&req))?;
        ensure_success_response(&root)?;

        let failed = root
            .get("failedCount")
            .or_else(|| root.get("failed_count"))
            .and_then(value_to_i32)
            .unwrap_or(0);
        if failed > 0 {
            return Err(anyhow!(
                "Failed to add Harmony device (failedCount={failed})"
            ));
        }

        Ok(())
    }

    pub fn query_certificates(&self, token: &AgcToken, cert_type: i32) -> Result<Vec<CertInfo>> {
        let req = CertListRequest { cert_type };
        let root = self.request_json("POST", "/v3/cert/list", token, Some(&req))?;
        ensure_success_response(&root)?;

        let mut certs = Vec::new();
        for item in array_field(&root, &["certList", "cert_list", "list"]) {
            let id = string_field(item, &["id", "certId"]).unwrap_or_default();
            let url =
                string_field(item, &["certDownloadUrl", "downloadUrl", "url"]).unwrap_or_default();
            if id.is_empty() || url.is_empty() {
                continue;
            }
            certs.push(CertInfo {
                id,
                cert_name: string_field(item, &["certName", "name"])
                    .unwrap_or_else(|| "LingXia Cert".to_string()),
                cert_type: int_field(item, &["certType"], cert_type),
                cert_download_url: url,
            });
        }

        Ok(certs)
    }

    pub fn create_certificate(
        &self,
        token: &AgcToken,
        csr: &str,
        is_debug: bool,
    ) -> Result<CertInfo> {
        let cert_name = format!(
            "lingxia_{}_{}.cer",
            if is_debug { "debug" } else { "release" },
            chrono::Utc::now().format("%Y%m%d%H%M%S")
        );
        let req = CreateCertRequest {
            cert_name,
            cert_type: if is_debug { 1 } else { 2 },
            csr: csr.to_string(),
        };
        let root = self.request_json("POST", "/v3/cert", token, Some(&req))?;
        ensure_success_response(&root)?;

        let info = object_field(&root, &["certInfo", "cert_info"])
            .ok_or_else(|| anyhow!("AGC did not return certInfo"))?;
        parse_cert_info(info, if is_debug { 1 } else { 2 })
    }

    pub fn delete_certificates(&self, token: &AgcToken, cert_ids: Vec<String>) -> Result<()> {
        if cert_ids.is_empty() {
            return Ok(());
        }
        let req = DeleteCertRequest { cert_ids };
        let root = self.request_json("POST", "/v2/cert/delete", token, Some(&req))?;
        ensure_success_response(&root)
    }

    pub fn query_profiles(
        &self,
        token: &AgcToken,
        provision_type: i32,
        app_id: Option<&str>,
    ) -> Result<Vec<ProvisionInfo>> {
        let path = "/v3/provision/list?fromRecCount=1&maxReqCount=100".to_string();
        let mut headers = Vec::new();
        if let Some(app_id) = app_id
            && !app_id.is_empty()
        {
            headers.push(("appId", app_id.to_string()));
        }

        let root = self.request_json_with_headers::<()>("GET", &path, token, None, &headers)?;
        ensure_success_response(&root)?;

        let mut out = Vec::new();
        for item in array_field(&root, &["provisionList", "provision_list", "list"]) {
            if let Ok(profile) = parse_provision_info(item, provision_type)
                && profile.provision_type == provision_type
            {
                out.push(profile);
            }
        }
        Ok(out)
    }

    pub fn create_profile(
        &self,
        token: &AgcToken,
        params: CreateProfileParams,
    ) -> Result<ProvisionInfo> {
        let req = CreateProfileRequest {
            provision_name: params.name.clone(),
            provision_type: if params.is_debug { 1 } else { 2 },
            cert_id: params.cert_id,
            app_id: params.app_id,
            device_id_list: params.device_ids,
            acl_permission_list: params.acl_permissions,
        };

        let root = self.request_json("POST", "/v3/provision", token, Some(&req))?;
        ensure_success_response(&root)?;
        let info = object_field(&root, &["provisionInfo", "provision_info"])
            .ok_or_else(|| anyhow!("AGC did not return provisionInfo"))?;
        parse_provision_info(info, if params.is_debug { 1 } else { 2 })
    }

    pub fn list_app_ids(
        &self,
        token: &AgcToken,
        package_name: Option<&str>,
    ) -> Result<Vec<AppIdInfo>> {
        let mut path = "/v2/appid-list?maxReqCount=100&packageTypes=7".to_string();
        if let Some(bundle) = package_name {
            path.push_str("&packageName=");
            path.push_str(&urlencoding::encode(bundle));
        }
        let root = self.request_json::<()>("GET", &path, token, None)?;
        ensure_success_response(&root)?;

        let mut out = Vec::new();
        for item in array_field(&root, &["appids", "appIdList", "app_list", "list"]) {
            // AGC may return compact `[{ "key": "<appName>", "value": "<appId>" }]` objects.
            let app_id = string_field(item, &["appId", "id", "appid", "value"]).unwrap_or_default();
            let package_name = string_field(item, &["packageName", "bundleId", "bundleName"])
                .or_else(|| package_name.map(str::to_string))
                .unwrap_or_default();
            if app_id.is_empty() || package_name.is_empty() {
                continue;
            }
            out.push(AppIdInfo {
                app_id,
                package_name,
                app_name: string_field(item, &["appName", "name", "key"])
                    .unwrap_or_else(|| "Harmony App".to_string()),
            });
        }

        Ok(out)
    }

    pub fn find_app_id_by_package_name(
        &self,
        token: &AgcToken,
        package_name: &str,
    ) -> Result<Option<AppIdInfo>> {
        let apps = self.list_app_ids(token, Some(package_name))?;
        Ok(apps
            .into_iter()
            .find(|app| app.package_name == package_name))
    }

    pub fn download_signed_asset(&self, token: &AgcToken, url: &str) -> Result<Vec<u8>> {
        let download_url = if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else if url.starts_with('/') {
            format!("{AGC_CONNECT_API_HOST}{url}")
        } else {
            format!("{AGC_CONNECT_API_HOST}/{url}")
        };

        let auth_header = format!("Bearer {}", token.access_token);
        let response = self
            .http_agent
            .get(&download_url)
            .header("Authorization", &auth_header)
            .header("client_id", &token.client_id)
            .call()
            .with_context(|| format!("Failed to download AGC asset: {download_url}"))?;

        response
            .into_body()
            .read_to_vec()
            .with_context(|| format!("Failed to read AGC asset body: {download_url}"))
    }
}

fn parse_cert_info(value: &Value, default_cert_type: i32) -> Result<CertInfo> {
    let id = string_field(value, &["id", "certId"]).unwrap_or_default();
    let cert_download_url = string_field(
        value,
        &[
            "certDownloadUrl",
            "downloadUrl",
            "url",
            "certObjectId",
            "objectId",
        ],
    )
    .unwrap_or_default();
    if id.is_empty() || cert_download_url.is_empty() {
        return Err(anyhow!("Invalid certInfo response from AGC"));
    }

    Ok(CertInfo {
        id,
        cert_name: string_field(value, &["certName", "name"])
            .unwrap_or_else(|| "LingXia Cert".to_string()),
        cert_type: int_field(value, &["certType"], default_cert_type),
        cert_download_url,
    })
}

fn parse_provision_info(value: &Value, default_type: i32) -> Result<ProvisionInfo> {
    let id = string_field(value, &["id", "provisionId"]).unwrap_or_default();
    let provision_download_url = string_field(
        value,
        &[
            "provisionDownloadUrl",
            "downloadUrl",
            "url",
            "provisionObjectId",
            "objectId",
        ],
    )
    .unwrap_or_default();
    if id.is_empty() || provision_download_url.is_empty() {
        return Err(anyhow!("Invalid provisionInfo response from AGC"));
    }

    let mut device_ids = array_field(value, &["deviceIdList", "deviceIds"])
        .iter()
        .filter_map(value_to_string)
        .collect::<Vec<_>>();
    if device_ids.is_empty() {
        for device in array_field(value, &["deviceList"]) {
            if let Some(id) = string_field(device, &["id", "deviceId"]) {
                device_ids.push(id);
            }
        }
    }

    let acl_permissions = array_field(value, &["aclPermissionList", "acl_permissions"])
        .iter()
        .filter_map(value_to_string)
        .collect::<Vec<_>>();

    Ok(ProvisionInfo {
        id,
        provision_name: string_field(value, &["provisionName", "name"])
            .unwrap_or_else(|| "LingXia Profile".to_string()),
        provision_type: int_field(value, &["provisionType"], default_type),
        cert_id: string_field(value, &["certId"]).unwrap_or_default(),
        cert_name: string_field(value, &["certName"]).unwrap_or_default(),
        app_id: string_field(value, &["appId", "appid"]).unwrap_or_default(),
        device_ids,
        acl_permissions,
        provision_download_url,
    })
}

fn ensure_success_response(root: &Value) -> Result<()> {
    let Some(ret) = root.get("ret") else {
        return Ok(());
    };

    let code = ret.get("code").and_then(value_to_i32).unwrap_or(0);
    if code == 0 {
        return Ok(());
    }

    let msg = ret
        .get("msg")
        .and_then(value_to_string)
        .unwrap_or_else(|| "Unknown AGC error".to_string());
    Err(anyhow!("AGC API Error {code}: {msg}"))
}

fn format_agc_http_error(status: u16, status_text: &str, url: &str, body: &str) -> anyhow::Error {
    let mut message = format!("AGC request failed ({status} {status_text}) for {url}");
    if !body.trim().is_empty() {
        message.push_str(&format!("\nResponse body: {}", body.trim()));
    }

    let lowered = format!(
        "{} {}",
        status_text.to_ascii_lowercase(),
        body.to_ascii_lowercase()
    );
    if status == 401 || status == 403 {
        if lowered.contains("client token auth failed")
            || lowered.contains("client token authorization fail")
        {
            message.push_str(
                "\nHint: This endpoint requires AGC Connect API client credentials. \
Project-level credentials (`type=project_client_id`) are not accepted for Publishing/Provisioning APIs. \
Create a Connect API client with Project=`N/A` (team-level).",
            );
        } else {
            message.push_str(
                "\nHint: AGC authorization failed. Use Connect API `API client` credentials \
(not `type=project_client_id`) with Project=`N/A` (team-level), and ensure the client role has Harmony publish/provision permissions.",
            );
        }
    }

    anyhow!(message)
}

fn object_field<'a>(root: &'a Value, names: &[&str]) -> Option<&'a Value> {
    for name in names {
        let Some(value) = root.get(*name) else {
            continue;
        };
        if value.is_object() {
            return Some(value);
        }
    }
    None
}

fn array_field<'a>(root: &'a Value, names: &[&str]) -> &'a [Value] {
    for name in names {
        let Some(value) = root.get(*name) else {
            continue;
        };
        if let Some(arr) = value.as_array() {
            return arr;
        }
    }
    &[]
}

fn string_field(root: &Value, names: &[&str]) -> Option<String> {
    for name in names {
        let Some(value) = root.get(*name) else {
            continue;
        };
        if let Some(string) = value_to_string(value)
            && !string.is_empty()
        {
            return Some(string);
        }
    }
    None
}

fn int_field(root: &Value, names: &[&str], fallback: i32) -> i32 {
    for name in names {
        let Some(value) = root.get(*name) else {
            continue;
        };
        if let Some(int) = value_to_i32(value) {
            return int;
        }
    }
    fallback
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(v) => Some(v.to_string()),
        Value::Number(v) => Some(v.to_string()),
        _ => None,
    }
}

fn value_to_i32(value: &Value) -> Option<i32> {
    match value {
        Value::Number(v) => v.as_i64().and_then(|i| i32::try_from(i).ok()),
        Value::String(v) => v.parse::<i32>().ok(),
        _ => None,
    }
}

impl Default for AgcConnectClient {
    fn default() -> Self {
        Self::new()
    }
}
