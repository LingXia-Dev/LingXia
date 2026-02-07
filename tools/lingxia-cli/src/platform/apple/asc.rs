//! App Store Connect API client.
//!
//! Provides JWT-based authentication and API calls for:
//! - Certificates
//! - Provisioning Profiles
//! - Devices
//! - Bundle IDs

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const ASC_API_BASE: &str = "https://api.appstoreconnect.apple.com/v1";

#[derive(Clone, Copy)]
enum HttpMethod {
    Get,
    Post,
    Delete,
}

impl HttpMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Delete => "DELETE",
        }
    }
}

/// App Store Connect API client
pub struct AppStoreConnectClient {
    key_id: String,
    issuer_id: String,
    private_key: String,
}

impl AppStoreConnectClient {
    /// Create a new client from stored credentials
    pub fn new(
        key_id: &str,
        issuer_id: &str,
        private_key_pem: &str,
        _team_id: &str,
    ) -> Result<Self> {
        if !private_key_pem.contains("BEGIN PRIVATE KEY") {
            return Err(anyhow!(
                "Invalid App Store Connect private key. Expected PKCS#8 PEM content."
            ));
        }

        Ok(Self {
            key_id: key_id.to_string(),
            issuer_id: issuer_id.to_string(),
            private_key: private_key_pem.to_string(),
        })
    }

    /// Generate a JWT token for API authentication
    ///
    /// JWT tokens for App Store Connect:
    /// - Algorithm: ES256 (ECDSA with P-256 and SHA-256)
    /// - Expiration: 20 minutes max
    pub fn generate_token(&self) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("System time before UNIX epoch")?
            .as_secs();

        // JWT Header
        let header = JwtHeader {
            alg: "ES256".to_string(),
            kid: self.key_id.clone(),
            typ: "JWT".to_string(),
        };

        // JWT Payload
        let payload = JwtPayload {
            iss: self.issuer_id.clone(),
            iat: now,
            exp: now + 1200, // 20 minutes
            aud: "appstoreconnect-v1".to_string(),
        };

        // Encode header and payload
        let header_json = serde_json::to_string(&header)?;
        let payload_json = serde_json::to_string(&payload)?;

        let header_b64 = base64_url_encode(header_json.as_bytes());
        let payload_b64 = base64_url_encode(payload_json.as_bytes());

        let message = format!("{}.{}", header_b64, payload_b64);

        // Sign with ES256
        let signature = self.sign_es256(message.as_bytes())?;
        let signature_b64 = base64_url_encode(&signature);

        Ok(format!("{}.{}", message, signature_b64))
    }

    /// Sign data using ES256 (ECDSA with P-256)
    fn sign_es256(&self, data: &[u8]) -> Result<Vec<u8>> {
        use p256::ecdsa::{Signature, SigningKey, signature::Signer};
        use p256::pkcs8::DecodePrivateKey;

        // Parse the private key (PEM format)
        let signing_key = SigningKey::from_pkcs8_pem(&self.private_key)
            .context("Failed to parse private key. Expected PKCS#8 PEM format.")?;

        // Sign the data
        let signature: Signature = signing_key.sign(data);

        // Return the signature in raw format (r || s, 64 bytes)
        Ok(signature.to_bytes().to_vec())
    }

    /// Make an authenticated GET request to the API
    pub fn get(&self, endpoint: &str) -> Result<serde_json::Value> {
        let body_str = self.request(HttpMethod::Get, endpoint, None)?;
        let body: serde_json::Value =
            serde_json::from_str(&body_str).context("Failed to parse JSON response")?;
        Ok(body)
    }

    /// Make an authenticated POST request to the API
    pub fn post(&self, endpoint: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let body_str = self.request(HttpMethod::Post, endpoint, Some(body))?;
        let body: serde_json::Value =
            serde_json::from_str(&body_str).context("Failed to parse JSON response")?;
        Ok(body)
    }

    /// Make an authenticated DELETE request to the API.
    pub fn delete(&self, endpoint: &str) -> Result<()> {
        self.request(HttpMethod::Delete, endpoint, None)?;
        Ok(())
    }

    fn request(
        &self,
        method: HttpMethod,
        endpoint: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<String> {
        let token = self.generate_token()?;
        let url = format!("{}{}", ASC_API_BASE, endpoint);
        let auth = format!("Bearer {}", token);

        let mut response = match method {
            HttpMethod::Get => ureq::get(&url)
                .config()
                .http_status_as_error(false)
                .build()
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .call(),
            HttpMethod::Post => {
                let body_json =
                    serde_json::to_string(body.context("Missing request body for POST")?)
                        .context("Failed to serialize request body")?;
                ureq::post(&url)
                    .config()
                    .http_status_as_error(false)
                    .build()
                    .header("Authorization", &auth)
                    .header("Content-Type", "application/json")
                    .send(body_json.as_bytes())
            }
            HttpMethod::Delete => ureq::delete(&url)
                .config()
                .http_status_as_error(false)
                .build()
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .call(),
        }
        .with_context(|| format!("API request failed: {} {}", method.as_str(), endpoint))?;

        let status = response.status();
        let body_str = response.body_mut().read_to_string().unwrap_or_default();

        if !status.is_success() {
            let detail = format_api_error_detail(&body_str);
            return Err(anyhow!(
                "API request failed: {} {} (HTTP {}){}",
                method.as_str(),
                endpoint,
                status.as_u16(),
                detail
            ));
        }

        Ok(body_str)
    }

    /// Create a new certificate
    pub fn create_certificate(
        &self,
        csr_content: &str,
        cert_type: CertificateType,
    ) -> Result<Certificate> {
        let body = serde_json::json!({
            "data": {
                "type": "certificates",
                "attributes": {
                    "csrContent": csr_content,
                    "certificateType": cert_type.as_str()
                }
            }
        });

        let response = self.post("/certificates", &body)?;
        parse_data_object(&response)
    }

    /// List all certificates
    pub fn list_certificates(&self) -> Result<Vec<Certificate>> {
        let response = self.get("/certificates")?;
        parse_data_array(&response)
    }

    /// Delete a certificate by ID.
    pub fn delete_certificate(&self, id: &str) -> Result<()> {
        self.delete(&format!("/certificates/{}", id))
    }

    /// List all registered devices
    pub fn list_devices(&self) -> Result<Vec<Device>> {
        let response = self.get("/devices")?;
        parse_data_array(&response)
    }

    /// Register a new device
    pub fn register_device(
        &self,
        name: &str,
        udid: &str,
        platform: DevicePlatform,
    ) -> Result<Device> {
        let body = serde_json::json!({
            "data": {
                "type": "devices",
                "attributes": {
                    "name": name,
                    "udid": udid,
                    "platform": platform.as_str()
                }
            }
        });

        let response = self.post("/devices", &body)?;
        parse_data_object(&response)
    }

    /// Create a new bundle ID
    pub fn create_bundle_id(
        &self,
        identifier: &str,
        name: &str,
        platform: BundleIdPlatform,
    ) -> Result<BundleId> {
        let body = serde_json::json!({
            "data": {
                "type": "bundleIds",
                "attributes": {
                    "identifier": identifier,
                    "name": name,
                    "platform": platform.as_str()
                }
            }
        });

        let response = self.post("/bundleIds", &body)?;
        parse_data_object(&response)
    }

    /// Find a bundle ID by identifier
    pub fn find_bundle_id(&self, identifier: &str) -> Result<Option<BundleId>> {
        let response = self.get(&format!("/bundleIds?filter[identifier]={}", identifier))?;
        let bundle_ids: Vec<BundleId> = parse_data_array(&response)?;
        Ok(bundle_ids.into_iter().next())
    }

    /// List all bundle IDs
    pub fn list_bundle_ids(&self) -> Result<Vec<BundleId>> {
        let response = self.get("/bundleIds")?;
        parse_data_array(&response)
    }

    /// Create a new provisioning profile
    pub fn create_profile(
        &self,
        name: &str,
        profile_type: ProfileType,
        bundle_id: &str,
        certificate_ids: &[String],
        device_ids: &[String],
    ) -> Result<Profile> {
        let body = serde_json::json!({
            "data": {
                "type": "profiles",
                "attributes": {
                    "name": name,
                    "profileType": profile_type.as_str()
                },
                "relationships": {
                    "bundleId": {
                        "data": {
                            "type": "bundleIds",
                            "id": bundle_id
                        }
                    },
                    "certificates": {
                        "data": certificate_ids.iter().map(|id| {
                            serde_json::json!({
                                "type": "certificates",
                                "id": id
                            })
                        }).collect::<Vec<_>>()
                    },
                    "devices": {
                        "data": device_ids.iter().map(|id| {
                            serde_json::json!({
                                "type": "devices",
                                "id": id
                            })
                        }).collect::<Vec<_>>()
                    }
                }
            }
        });

        let response = self.post("/profiles", &body)?;
        parse_data_object(&response)
    }

    /// Download a provisioning profile content (base64 decoded)
    pub fn download_profile(&self, id: &str) -> Result<Vec<u8>> {
        let profile = self.get_profile(id)?;
        let content_b64 = profile
            .attributes
            .profile_content
            .ok_or_else(|| anyhow!("Profile content not available"))?;

        base64_decode(&content_b64)
    }

    /// Get a specific profile by ID
    pub fn get_profile(&self, id: &str) -> Result<Profile> {
        let response = self.get(&format!("/profiles/{}", id))?;
        parse_data_object(&response)
    }

    /// List all provisioning profiles
    pub fn list_profiles(&self) -> Result<Vec<Profile>> {
        let response = self.get("/profiles")?;
        parse_data_array(&response)
    }
}

#[derive(Serialize)]
struct JwtHeader {
    alg: String,
    kid: String,
    typ: String,
}

#[derive(Serialize)]
struct JwtPayload {
    iss: String,
    iat: u64,
    exp: u64,
    aud: String,
}

// API Response Types

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Certificate {
    pub id: String,
    pub attributes: CertificateAttributes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificateAttributes {
    pub name: Option<String>,
    pub certificate_type: Option<String>,
    pub display_name: Option<String>,
    pub serial_number: Option<String>,
    pub platform: Option<String>,
    pub expiration_date: Option<String>,
    pub certificate_content: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum CertificateType {
    IosDevelopment,
}

impl CertificateType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CertificateType::IosDevelopment => "IOS_DEVELOPMENT",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub attributes: DeviceAttributes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceAttributes {
    pub name: Option<String>,
    pub udid: Option<String>,
    pub device_class: Option<String>,
    pub status: Option<String>,
    pub platform: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum DevicePlatform {
    Ios,
}

impl DevicePlatform {
    pub fn as_str(&self) -> &'static str {
        match self {
            DevicePlatform::Ios => "IOS",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleId {
    pub id: String,
    pub attributes: BundleIdAttributes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleIdAttributes {
    pub name: Option<String>,
    pub identifier: Option<String>,
    pub platform: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum BundleIdPlatform {
    Ios,
}

impl BundleIdPlatform {
    pub fn as_str(&self) -> &'static str {
        match self {
            BundleIdPlatform::Ios => "IOS",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub attributes: ProfileAttributes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileAttributes {
    pub name: Option<String>,
    pub profile_type: Option<String>,
    pub profile_state: Option<String>,
    pub profile_content: Option<String>,
    pub uuid: Option<String>,
    pub expiration_date: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum ProfileType {
    IosAppDevelopment,
}

impl ProfileType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProfileType::IosAppDevelopment => "IOS_APP_DEVELOPMENT",
        }
    }
}

/// Parse a JSON:API response data array
fn parse_data_array<T: for<'de> Deserialize<'de>>(response: &serde_json::Value) -> Result<Vec<T>> {
    let data = response
        .get("data")
        .ok_or_else(|| anyhow!("Missing 'data' in response"))?;

    serde_json::from_value(data.clone()).context("Failed to parse response data array")
}

/// Parse a JSON:API response data object
fn parse_data_object<T: for<'de> Deserialize<'de>>(response: &serde_json::Value) -> Result<T> {
    let data = response
        .get("data")
        .ok_or_else(|| anyhow!("Missing 'data' in response"))?;

    serde_json::from_value(data.clone()).context("Failed to parse response data object")
}

/// Base64 URL-safe encoding (no padding)
fn base64_url_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// Base64 standard decoding
fn base64_decode(data: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .context("Failed to decode base64 data")
}

fn format_api_error_detail(body_str: &str) -> String {
    let parsed = serde_json::from_str::<serde_json::Value>(body_str).ok();
    if let Some(v) = parsed
        && let Some(errors) = v.get("errors").and_then(|e| e.as_array())
        && let Some(first) = errors.first()
    {
        let status = first
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or_default();
        let code = first
            .get("code")
            .and_then(|s| s.as_str())
            .unwrap_or_default();
        let title = first
            .get("title")
            .and_then(|s| s.as_str())
            .unwrap_or_default();
        let detail = first
            .get("detail")
            .and_then(|s| s.as_str())
            .unwrap_or_default();

        let mut parts = Vec::new();
        if !status.is_empty() {
            parts.push(format!("status={status}"));
        }
        if !code.is_empty() {
            parts.push(format!("code={code}"));
        }
        if !title.is_empty() {
            parts.push(format!("title={title}"));
        }
        if !detail.is_empty() {
            parts.push(format!("detail={detail}"));
        }

        if !parts.is_empty() {
            return format!(" [{}]", parts.join(", "));
        }
    }

    let compact = body_str.trim().replace('\n', " ");
    if compact.is_empty() {
        String::new()
    } else {
        format!(" [body={}]", compact)
    }
}
