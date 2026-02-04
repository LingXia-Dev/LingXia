//! GrandSlam authentication for Apple ID login.
//!
//! This implements Apple's GrandSlam authentication protocol used for
//! authenticating with Apple ID credentials (email + password).

use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;

use super::anisette::AnisetteData;
use super::http_agent;
use super::srp::SrpClient;

const GSA_LOOKUP_URL: &str = "https://gsa.apple.com/grandslam/GsService2/lookup";

/// GrandSlam endpoints fetched from lookup
#[derive(Debug, Clone)]
pub struct GrandSlamEndpoints {
    pub gs_service: String,
    pub secondary_auth: String,
    pub trusted_device_secondary_auth: String,
    pub validate_code: String,
    pub mid_start_provisioning: String,
    pub mid_finish_provisioning: String,
}

/// Login data returned after successful authentication
#[derive(Debug, Clone)]
pub struct GrandSlamLoginData {
    pub adsid: String,
    pub idms_token: String,
    pub sk: Vec<u8>,     // Session key for decryption
    pub cookie: Vec<u8>, // Service cookie for subsequent requests
}

/// Two-factor authentication mode
#[derive(Debug, Clone, PartialEq)]
pub enum TwoFactorMode {
    /// 2FA was automatically triggered, just wait for code
    Auto,
    /// Need to request trusted device push notification
    TrustedDevice,
    /// Need to request SMS code
    Sms,
}

/// Two-factor authentication required error
#[derive(Debug, Clone)]
pub struct TwoFactorRequired {
    pub adsid: String,
    pub idms_token: String,
    pub session_key: Vec<u8>,
    pub mode: TwoFactorMode,
}

impl TwoFactorRequired {
    /// Get the identity token (Base64 of "adsid:idmsToken")
    pub fn identity_token(&self) -> String {
        use base64::Engine;
        let combined = format!("{}:{}", self.adsid, self.idms_token);
        base64::engine::general_purpose::STANDARD.encode(combined.as_bytes())
    }
}

impl std::fmt::Display for TwoFactorRequired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Two-factor authentication required")
    }
}

impl std::error::Error for TwoFactorRequired {}

/// GrandSlam client for Apple ID authentication
pub struct GrandSlamClient {
    endpoints: Option<GrandSlamEndpoints>,
}

impl GrandSlamClient {
    pub fn new() -> Self {
        Self { endpoints: None }
    }

    /// Lookup GrandSlam endpoints
    pub fn lookup(
        &mut self,
        device_info: &DeviceInfo,
        _anisette: &AnisetteData,
    ) -> Result<&GrandSlamEndpoints> {
        if self.endpoints.is_some() {
            return Ok(self.endpoints.as_ref().unwrap());
        }

        let mut request = http_agent().get(GSA_LOOKUP_URL);

        // Add headers
        request = request.header("X-Mme-Client-Info", &device_info.client_info);
        request = request.header("X-Mme-Device-Id", &device_info.device_id);
        request = request.header("X-Apple-I-Locale", "en_US");
        request = request.header("X-Apple-I-TimeZone", "UTC");

        let mut response = request
            .call()
            .context("Failed to lookup GrandSlam endpoints")?;

        let body = response
            .body_mut()
            .read_to_string()
            .context("Failed to read lookup response")?;

        // Parse plist response
        let plist: plist::Value = plist::from_bytes(body.as_bytes())
            .context("Failed to parse lookup response as plist")?;

        let urls = plist
            .as_dictionary()
            .and_then(|d| d.get("urls"))
            .and_then(|v| v.as_dictionary())
            .ok_or_else(|| anyhow!("Invalid lookup response format"))?;

        let get_url = |key: &str| -> Result<String> {
            urls.get(key)
                .and_then(|v| v.as_string())
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow!("Missing endpoint: {}", key))
        };

        self.endpoints = Some(GrandSlamEndpoints {
            gs_service: get_url("gsService")?,
            secondary_auth: get_url("secondaryAuth")?,
            trusted_device_secondary_auth: get_url("trustedDeviceSecondaryAuth")?,
            validate_code: get_url("validateCode")?,
            mid_start_provisioning: get_url("midStartProvisioning")?,
            mid_finish_provisioning: get_url("midFinishProvisioning")?,
        });

        Ok(self.endpoints.as_ref().unwrap())
    }

    /// Authenticate with username and password
    pub fn authenticate(
        &mut self,
        username: &str,
        password: &str,
        device_info: &DeviceInfo,
        anisette: &AnisetteData,
    ) -> Result<GrandSlamLoginData> {
        // Step 1: Lookup endpoints
        let endpoints = self.lookup(device_info, anisette)?.clone();

        // Step 2: Auth Init
        let mut srp_client = SrpClient::new();
        let public_key = srp_client.public_key_bytes();

        let init_response = self.auth_init(
            &endpoints.gs_service,
            username,
            &public_key,
            device_info,
            anisette,
        )?;

        // Step 3: Process SRP challenge
        let m1 = srp_client.process_challenge(
            username,
            password,
            &init_response.salt,
            init_response.iterations,
            init_response.is_legacy_protocol,
            &init_response.server_public_key,
        )?;

        // Step 4: Auth Complete
        let complete_response = self.auth_complete(
            &endpoints.gs_service,
            username,
            &init_response.cookie,
            &m1,
            device_info,
            anisette,
        )?;

        // Step 5: Verify server response and decrypt
        if !srp_client.verify_server_proof(&complete_response.hamk) {
            return Err(anyhow!("Server proof verification failed"));
        }

        let decrypted = srp_client.decrypt_response(&complete_response.encrypted_response)?;

        // Parse decrypted response
        let login_response: plist::Value =
            plist::from_bytes(&decrypted).context("Failed to parse login response")?;

        let status_code = login_response
            .as_dictionary()
            .and_then(|d| d.get("status-code"))
            .and_then(|v| v.as_signed_integer())
            .unwrap_or(0);

        // Extract preliminary login data
        let dict = login_response
            .as_dictionary()
            .ok_or_else(|| anyhow!("Invalid login response format"))?;

        let adsid = dict
            .get("adsid")
            .and_then(|v| v.as_string())
            .map(|s| s.to_string())
            .unwrap_or_default();

        let idms_token = dict
            .get("GsIdmsToken")
            .and_then(|v| v.as_string())
            .map(|s| s.to_string())
            .unwrap_or_default();

        if status_code == 409 {
            // 2FA required - check the auth mode from "url" field
            let url = dict
                .get("url")
                .and_then(|v| v.as_string())
                .map(|s| s.to_string());

            let mode = match url.as_deref() {
                Some("trustedDeviceSecondaryAuth") => TwoFactorMode::TrustedDevice,
                Some("secondaryAuth") => TwoFactorMode::Sms,
                _ => TwoFactorMode::Auto,
            };

            return Err(TwoFactorRequired {
                adsid,
                idms_token,
                session_key: srp_client.session_key().to_vec(),
                mode,
            }
            .into());
        }

        if adsid.is_empty() || idms_token.is_empty() {
            return Err(anyhow!("Missing credentials in login response"));
        }

        let sk = dict
            .get("sk")
            .and_then(|v| v.as_data())
            .map(|d| d.to_vec())
            .ok_or_else(|| anyhow!("Missing session key (sk) in login response"))?;

        let cookie = dict
            .get("c")
            .and_then(|v| v.as_data())
            .map(|d| d.to_vec())
            .ok_or_else(|| anyhow!("Missing cookie (c) in login response"))?;

        Ok(GrandSlamLoginData {
            adsid,
            idms_token,
            sk,
            cookie,
        })
    }

    /// Request 2FA push notification to trusted device
    pub fn request_trusted_device_push(
        &self,
        partial_login: &TwoFactorRequired,
        device_info: &DeviceInfo,
        anisette: &AnisetteData,
    ) -> Result<()> {
        let endpoints = self
            .endpoints
            .as_ref()
            .ok_or_else(|| anyhow!("Endpoints not initialized"))?;

        // Build request matching xtool's GrandSlamTwoFactorRequest exactly
        let now = chrono::Utc::now();
        let response = http_agent()
            .get(&endpoints.trusted_device_secondary_auth)
            // From TwoFactorRequest.configure()
            .header("Accept", "application/x-buddyml")
            .header("Content-Type", "application/x-plist")
            .header("X-Apple-App-Info", "com.apple.gs.xcode.auth")
            .header("X-Xcode-Version", "14.2 (14C18)")
            .header("X-Apple-Identity-Token", partial_login.identity_token())
            // From GrandSlamClient.send() - X-Mme-Client-Info
            // Must match the client_info used in initial authentication
            .header("X-Mme-Client-Info", &device_info.client_info)
            // Anisette data headers (matching xtool's AnisetteData.dictionary)
            .header("X-Apple-Locale", "en_US")
            .header("X-Apple-I-TimeZone", "UTC")
            .header(
                "X-Apple-I-Client-Time",
                now.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            )
            .header("X-Apple-I-MD-RINFO", anisette.routing_info.to_string())
            .header("X-Apple-I-MD-M", &anisette.machine_id)
            .header("X-Apple-I-MD-LU", &anisette.local_user_id)
            .header("X-Apple-I-MD", &anisette.otp)
            .header("X-Mme-Device-Id", &anisette.device_id)
            .call();

        // Handle response
        let mut response =
            response.map_err(|e| anyhow!("Failed to send trusted device push request: {}", e))?;

        let response_body = response.body_mut().read_to_string()?;

        // Check for errors in response
        if let Ok(plist_response) = plist::from_bytes::<plist::Value>(response_body.as_bytes()) {
            if let Some(status) = plist_response
                .as_dictionary()
                .and_then(|d| d.get("Status"))
                .and_then(|v| v.as_dictionary())
            {
                let ec = status
                    .get("ec")
                    .and_then(|v| v.as_signed_integer())
                    .unwrap_or(0);
                if ec != 0 {
                    let em = status
                        .get("em")
                        .and_then(|v| v.as_string())
                        .unwrap_or("Unknown error");
                    return Err(anyhow!("Trusted device request failed: {} ({})", em, ec));
                }
            }
        }

        Ok(())
    }

    /// Validate 2FA code and complete authentication
    pub fn validate_2fa(
        &mut self,
        code: &str,
        partial_login: &TwoFactorRequired,
        device_info: &DeviceInfo,
        anisette_provider: &mut super::anisette::OmnisetteProvider,
    ) -> Result<()> {
        let endpoints = self
            .endpoints
            .as_ref()
            .ok_or_else(|| anyhow!("Endpoints not initialized"))?;

        // Fetch fresh anisette data for this request
        let anisette = anisette_provider
            .fetch_anisette_data()
            .context("Failed to refresh anisette data for 2FA validation")?;

        let now = chrono::Utc::now();

        // Build GET request matching xtool's GrandSlamValidateRequest exactly
        // Note: xtool uses GET with security-code header, not POST with body
        let response = http_agent()
            .get(&endpoints.validate_code)
            // From GrandSlamTwoFactorRequest.configure()
            .header("Accept", "application/x-buddyml")
            .header("Content-Type", "application/x-plist")
            .header("X-Apple-App-Info", "com.apple.gs.xcode.auth")
            .header("X-Xcode-Version", "14.2 (14C18)")
            .header("X-Apple-Identity-Token", partial_login.identity_token())
            // From GrandSlamClient.send() - X-Mme-Client-Info
            .header("X-Mme-Client-Info", &device_info.client_info)
            // Anisette data headers (matching xtool's AnisetteData.dictionary)
            .header("X-Apple-Locale", "en_US")
            .header("X-Apple-I-TimeZone", "UTC")
            .header(
                "X-Apple-I-Client-Time",
                now.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            )
            .header("X-Apple-I-MD-RINFO", anisette.routing_info.to_string())
            .header("X-Apple-I-MD-M", &anisette.machine_id)
            .header("X-Apple-I-MD-LU", &anisette.local_user_id)
            .header("X-Apple-I-MD", &anisette.otp)
            .header("X-Mme-Device-Id", &anisette.device_id)
            // Extra header from GrandSlamValidateRequest - security code
            .header("security-code", code)
            .call()
            .context("Failed to send 2FA validation request")?;

        // Check HTTP status
        if response.status() != 200 {
            return Err(anyhow!(
                "2FA validation failed with status: {}",
                response.status()
            ));
        }

        Ok(())
    }

    /// Auth Init request
    fn auth_init(
        &self,
        endpoint: &str,
        username: &str,
        public_key: &[u8],
        device_info: &DeviceInfo,
        anisette: &AnisetteData,
    ) -> Result<AuthInitResponse> {
        let protocols = vec!["s2k", "s2k_fo"];

        let client_data = self.build_client_data(device_info, anisette);

        let request_body: HashMap<String, plist::Value> = [
            (
                "Header".to_string(),
                plist::Value::Dictionary({
                    let mut h = plist::Dictionary::new();
                    h.insert("Version".to_string(), "1.0.1".into());
                    h
                }),
            ),
            (
                "Request".to_string(),
                plist::Value::Dictionary({
                    let mut r = plist::Dictionary::new();
                    r.insert("o".to_string(), "init".into());
                    r.insert("u".to_string(), username.into());
                    r.insert(
                        "ps".to_string(),
                        plist::Value::Array(protocols.iter().map(|s| (*s).into()).collect()),
                    );
                    r.insert("A2k".to_string(), plist::Value::Data(public_key.to_vec()));
                    r.insert("cpd".to_string(), client_data);
                    r
                }),
            ),
        ]
        .into_iter()
        .collect();

        let mut body_buf = Vec::new();
        plist::to_writer_xml(&mut body_buf, &request_body)
            .context("Failed to encode auth init request")?;
        let body = String::from_utf8(body_buf).context("Failed to convert body to string")?;

        let mut response = http_agent()
            .post(endpoint)
            .header("Content-Type", "text/x-xml-plist")
            .header("Accept", "*/*")
            .header("User-Agent", &device_info.user_agent)
            .header("X-Mme-Client-Info", &device_info.client_info)
            .send(body.as_bytes())
            .context("Failed to send auth init request")?;

        let response_body = response
            .body_mut()
            .read_to_string()
            .context("Failed to read auth init response")?;

        let plist_response: plist::Value = plist::from_bytes(response_body.as_bytes())
            .context("Failed to parse auth init response")?;

        self.check_error(&plist_response)?;

        let resp = plist_response
            .as_dictionary()
            .and_then(|d| d.get("Response"))
            .and_then(|v| v.as_dictionary())
            .ok_or_else(|| anyhow!("Invalid auth init response format"))?;

        let sp = resp
            .get("sp")
            .and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("Missing protocol in auth init response"))?;

        Ok(AuthInitResponse {
            is_legacy_protocol: sp == "s2k_fo",
            cookie: resp
                .get("c")
                .and_then(|v| v.as_string())
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow!("Missing cookie"))?,
            salt: resp
                .get("s")
                .and_then(|v| v.as_data())
                .map(|d| d.to_vec())
                .ok_or_else(|| anyhow!("Missing salt"))?,
            iterations: resp
                .get("i")
                .and_then(|v| v.as_signed_integer())
                .map(|i| i as u32)
                .ok_or_else(|| anyhow!("Missing iterations"))?,
            server_public_key: resp
                .get("B")
                .and_then(|v| v.as_data())
                .map(|d| d.to_vec())
                .ok_or_else(|| anyhow!("Missing server public key"))?,
        })
    }

    /// Auth Complete request
    fn auth_complete(
        &self,
        endpoint: &str,
        username: &str,
        cookie: &str,
        m1: &[u8],
        device_info: &DeviceInfo,
        anisette: &AnisetteData,
    ) -> Result<AuthCompleteResponse> {
        let client_data = self.build_client_data(device_info, anisette);

        let request_body: HashMap<String, plist::Value> = [
            (
                "Header".to_string(),
                plist::Value::Dictionary({
                    let mut h = plist::Dictionary::new();
                    h.insert("Version".to_string(), "1.0.1".into());
                    h
                }),
            ),
            (
                "Request".to_string(),
                plist::Value::Dictionary({
                    let mut r = plist::Dictionary::new();
                    r.insert("o".to_string(), "complete".into());
                    r.insert("u".to_string(), username.into());
                    r.insert("c".to_string(), cookie.into());
                    r.insert("M1".to_string(), plist::Value::Data(m1.to_vec()));
                    r.insert("cpd".to_string(), client_data);
                    r
                }),
            ),
        ]
        .into_iter()
        .collect();

        let mut body_buf = Vec::new();
        plist::to_writer_xml(&mut body_buf, &request_body)
            .context("Failed to encode auth complete request")?;
        let body = String::from_utf8(body_buf).context("Failed to convert body to string")?;

        let mut response = http_agent()
            .post(endpoint)
            .header("Content-Type", "text/x-xml-plist")
            .header("Accept", "*/*")
            .header("User-Agent", &device_info.user_agent)
            .header("X-Mme-Client-Info", &device_info.client_info)
            .send(body.as_bytes())
            .context("Failed to send auth complete request")?;

        let response_body = response
            .body_mut()
            .read_to_string()
            .context("Failed to read auth complete response")?;

        let plist_response: plist::Value = plist::from_bytes(response_body.as_bytes())
            .context("Failed to parse auth complete response")?;

        self.check_error(&plist_response)?;

        let resp = plist_response
            .as_dictionary()
            .and_then(|d| d.get("Response"))
            .and_then(|v| v.as_dictionary())
            .ok_or_else(|| anyhow!("Invalid auth complete response format"))?;

        Ok(AuthCompleteResponse {
            hamk: resp
                .get("M2")
                .and_then(|v| v.as_data())
                .map(|d| d.to_vec())
                .ok_or_else(|| anyhow!("Missing M2"))?,
            encrypted_response: resp
                .get("spd")
                .and_then(|v| v.as_data())
                .map(|d| d.to_vec())
                .ok_or_else(|| anyhow!("Missing encrypted response"))?,
        })
    }

    fn build_client_data(&self, device_info: &DeviceInfo, anisette: &AnisetteData) -> plist::Value {
        let mut cpd = plist::Dictionary::new();

        // Static fields
        cpd.insert("bootstrap".to_string(), true.into());
        cpd.insert("icscrec".to_string(), true.into());
        cpd.insert("pbe".to_string(), false.into());
        cpd.insert("prkgen".to_string(), true.into());
        cpd.insert("svct".to_string(), "iCloud".into());
        cpd.insert("loc".to_string(), "en_US".into());

        // Device info
        cpd.insert(
            "X-Mme-Device-Id".to_string(),
            device_info.device_id.clone().into(),
        );
        cpd.insert(
            "X-Apple-I-SRL-NO".to_string(),
            device_info.serial_number.clone().into(),
        );
        cpd.insert(
            "X-Apple-I-MLB".to_string(),
            device_info.mlb_serial.clone().into(),
        );
        cpd.insert(
            "X-Apple-I-ROM".to_string(),
            device_info.rom_address.clone().into(),
        );

        // Anisette data
        cpd.insert(
            "X-Apple-I-MD-M".to_string(),
            anisette.machine_id.clone().into(),
        );
        cpd.insert("X-Apple-I-MD".to_string(), anisette.otp.clone().into());
        cpd.insert(
            "X-Apple-I-MD-RINFO".to_string(),
            format!("{}", anisette.routing_info).into(),
        );
        cpd.insert(
            "X-Apple-I-MD-LU".to_string(),
            anisette.local_user_id.clone().into(),
        );
        cpd.insert("X-Apple-I-TimeZone".to_string(), "UTC".into());
        cpd.insert("X-Apple-Locale".to_string(), "en_US".into());
        cpd.insert(
            "X-Apple-I-Client-Time".to_string(),
            chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string()
                .into(),
        );

        plist::Value::Dictionary(cpd)
    }

    /// Fetch app-specific tokens required for Developer Services API.
    ///
    /// Returns the Xcode app token string used as `X-Apple-GS-Token`.
    pub fn fetch_app_tokens(
        &self,
        login_data: &GrandSlamLoginData,
        device_info: &DeviceInfo,
        anisette: &AnisetteData,
    ) -> Result<String> {
        let endpoints = self
            .endpoints
            .as_ref()
            .ok_or_else(|| anyhow!("Endpoints not initialized"))?;

        let app_key = "com.apple.gs.xcode.auth";

        // Compute HMAC-SHA256 checksum: HMAC(sk, "apptokens" + adsid + app)
        let checksum = {
            use hmac::{Hmac, Mac};
            use sha2::Sha256;
            let mut mac = Hmac::<Sha256>::new_from_slice(&login_data.sk)
                .expect("HMAC can take key of any size");
            mac.update(b"apptokens");
            mac.update(login_data.adsid.as_bytes());
            mac.update(app_key.as_bytes());
            mac.finalize().into_bytes().to_vec()
        };

        let client_data = self.build_client_data(device_info, anisette);

        let request_body: HashMap<String, plist::Value> = [
            (
                "Header".to_string(),
                plist::Value::Dictionary({
                    let mut h = plist::Dictionary::new();
                    h.insert("Version".to_string(), "1.0.1".into());
                    h
                }),
            ),
            (
                "Request".to_string(),
                plist::Value::Dictionary({
                    let mut r = plist::Dictionary::new();
                    r.insert("o".to_string(), "apptokens".into());
                    r.insert("u".to_string(), login_data.adsid.clone().into());
                    r.insert("app".to_string(), plist::Value::Array(vec![app_key.into()]));
                    r.insert(
                        "c".to_string(),
                        plist::Value::Data(login_data.cookie.clone()),
                    );
                    r.insert("t".to_string(), login_data.idms_token.clone().into());
                    r.insert("checksum".to_string(), plist::Value::Data(checksum));
                    r.insert("cpd".to_string(), client_data);
                    r
                }),
            ),
        ]
        .into_iter()
        .collect();

        let mut body = Vec::new();
        plist::to_writer_xml(&mut body, &request_body)
            .context("Failed to encode app tokens request")?;

        let mut response = http_agent()
            .post(&endpoints.gs_service)
            .header("Content-Type", "text/x-xml-plist")
            .header("Accept", "*/*")
            .header("User-Agent", &device_info.user_agent)
            .header("X-Mme-Client-Info", &device_info.client_info)
            .send(&body)
            .context("Failed to send app tokens request")?;

        let response_body = response
            .body_mut()
            .read_to_string()
            .context("Failed to read app tokens response")?;

        let plist_response: plist::Value = plist::from_bytes(response_body.as_bytes())
            .context("Failed to parse app tokens response")?;

        self.check_error(&plist_response)?;

        let resp = plist_response
            .as_dictionary()
            .and_then(|d| d.get("Response"))
            .and_then(|v| v.as_dictionary())
            .ok_or_else(|| anyhow!("Invalid app tokens response format"))?;

        let encrypted = resp
            .get("et")
            .and_then(|v| v.as_data())
            .ok_or_else(|| anyhow!("Missing encrypted token (et) in response"))?;

        // Decrypt with AES-256-GCM
        // Layout: AAD (3 bytes) | IV (16 bytes) | ciphertext | tag (16 bytes)
        if encrypted.len() < 3 + 16 + 16 {
            return Err(anyhow!("Encrypted token too short"));
        }

        let aad = &encrypted[..3];
        let iv = &encrypted[3..19];
        let tag = &encrypted[encrypted.len() - 16..];
        let ciphertext = &encrypted[19..encrypted.len() - 16];

        let decrypted = {
            use aes::Aes256;
            use aes_gcm::aead::generic_array::GenericArray;
            use aes_gcm::{AesGcm, KeyInit, aead::Aead};

            // Apple uses 16-byte nonce instead of the standard 12
            type Aes256Gcm16 =
                AesGcm<Aes256, aes_gcm::aead::consts::U16, aes_gcm::aead::consts::U16>;

            let cipher = Aes256Gcm16::new(GenericArray::from_slice(&login_data.sk));

            let mut combined = ciphertext.to_vec();
            combined.extend_from_slice(tag);

            cipher
                .decrypt(
                    GenericArray::from_slice(iv),
                    aes_gcm::aead::Payload {
                        msg: &combined,
                        aad,
                    },
                )
                .map_err(|_| anyhow!("AES-GCM decryption failed"))?
        };

        // Parse decrypted plist: {"t": {"com.apple.gs.xcode.auth": {"token": "...", "expiry": N}}}
        let token_plist: plist::Value =
            plist::from_bytes(&decrypted).context("Failed to parse decrypted app tokens")?;

        let token_value = token_plist
            .as_dictionary()
            .and_then(|d| d.get("t"))
            .and_then(|v| v.as_dictionary())
            .and_then(|d| d.get(app_key))
            .and_then(|v| v.as_dictionary())
            .and_then(|d| d.get("token"))
            .and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("Failed to extract app token from decrypted response"))?;

        Ok(token_value.to_string())
    }

    fn check_error(&self, response: &plist::Value) -> Result<()> {
        let status = response
            .as_dictionary()
            .and_then(|d| d.get("Response"))
            .and_then(|v| v.as_dictionary())
            .and_then(|d| d.get("Status"))
            .and_then(|v| v.as_dictionary());

        if let Some(status) = status {
            let error_code = status
                .get("ec")
                .and_then(|v| v.as_signed_integer())
                .unwrap_or(0);

            if error_code != 0 {
                let error_msg = status
                    .get("em")
                    .and_then(|v| v.as_string())
                    .unwrap_or("Unknown error");
                return Err(anyhow!("GrandSlam error {}: {}", error_code, error_msg));
            }
        }

        Ok(())
    }
}

/// Device info for authentication
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_id: String,
    pub serial_number: String,
    pub mlb_serial: String,
    pub rom_address: String,
    pub model_id: String,
    pub client_info: String,
    pub user_agent: String,
}

impl DeviceInfo {
    /// Create device info with reasonable defaults
    pub fn default_macos() -> Self {
        let device_id = uuid::Uuid::new_v4().to_string().to_uppercase();

        Self {
            device_id,
            serial_number: "C02XXXXXXXXX".to_string(), // Placeholder
            mlb_serial: "C02XXXXXXXXXXX".to_string(),  // Placeholder
            rom_address: "000000000000".to_string(),   // Placeholder
            model_id: "MacBookPro15,1".to_string(),
            client_info:
                "<MacBookPro15,1> <Mac OS X;14.3.1;23D60> <com.apple.AuthKit/1 (com.apple.akd/1.0)>"
                    .to_string(),
            user_agent: "akd/1.0 CFNetwork/978.0.7 Darwin/18.7.0".to_string(),
        }
    }
}

#[derive(Debug)]
struct AuthInitResponse {
    is_legacy_protocol: bool,
    cookie: String,
    salt: Vec<u8>,
    iterations: u32,
    server_public_key: Vec<u8>,
}

#[derive(Debug)]
struct AuthCompleteResponse {
    hamk: Vec<u8>,
    encrypted_response: Vec<u8>,
}
