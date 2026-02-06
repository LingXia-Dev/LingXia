//! Apple Developer Services API client.
//!
//! Used for Apple Developer Services operations after Apple ID authentication.
//! This API is used for free developer accounts (via Apple ID + GrandSlam auth).
//!
//! For paid accounts, use the App Store Connect API (`asc.rs`) instead.

use anyhow::{Context, Result, anyhow};

use super::anisette::AnisetteData;
use super::grandslam::DeviceInfo;
use super::http_agent;

const DEVELOPER_SERVICES_BASE: &str = "https://developerservices2.apple.com/services";
const PROTOCOL_VERSION: &str = "QH65B2";
const CLIENT_ID: &str = "XABBG36SBA";

// =============================================================================
// Client
// =============================================================================

/// Developer Services API client for Apple ID authenticated requests.
///
/// This client is used for free developer accounts that authenticate via
/// Apple ID (GrandSlam). For paid accounts, use `AppStoreConnectClient` instead.
pub struct DeveloperServicesClient<'a> {
    pub adsid: &'a str,
    pub app_token: &'a str,
    pub team_id: &'a str,
    pub device_info: &'a DeviceInfo,
    pub anisette: &'a AnisetteData,
}

impl<'a> DeveloperServicesClient<'a> {
    /// Create a new Developer Services client
    pub fn new(
        adsid: &'a str,
        app_token: &'a str,
        team_id: &'a str,
        device_info: &'a DeviceInfo,
        anisette: &'a AnisetteData,
    ) -> Self {
        Self {
            adsid,
            app_token,
            team_id,
            device_info,
            anisette,
        }
    }

    /// Make an authenticated request to Developer Services API
    fn request(&self, action: &str, extra_params: &[(&str, plist::Value)]) -> Result<plist::Value> {
        let url = format!(
            "{}/{}/{}.action?clientId={}",
            DEVELOPER_SERVICES_BASE, PROTOCOL_VERSION, action, CLIENT_ID
        );

        // Build request body
        let request_id = uuid::Uuid::new_v4().to_string();
        let mut params = plist::Dictionary::new();
        params.insert("requestId".to_string(), request_id.into());
        params.insert("clientId".to_string(), CLIENT_ID.into());
        params.insert("protocolVersion".to_string(), PROTOCOL_VERSION.into());
        params.insert(
            "userLocale".to_string(),
            plist::Value::Array(vec!["en_US".into()]),
        );
        params.insert("teamId".to_string(), self.team_id.into());

        // Add extra parameters
        for (key, value) in extra_params {
            params.insert(key.to_string(), value.clone());
        }

        let mut body_buf = Vec::new();
        plist::to_writer_xml(&mut body_buf, &plist::Value::Dictionary(params))
            .context("Failed to encode request body")?;
        let body = String::from_utf8(body_buf).context("Failed to convert body to string")?;

        let now = chrono::Utc::now();

        let mut response = http_agent()
            .post(&url)
            .header("Accept", "text/x-xml-plist")
            .header("Content-Type", "text/x-xml-plist")
            .header("User-Agent", "Xcode")
            .header("X-Xcode-Version", "14.2 (14C18)")
            .header("X-Apple-App-Info", "com.apple.gs.xcode.auth")
            // Auth
            .header("X-Apple-I-Identity-Id", self.adsid)
            .header("X-Apple-GS-Token", self.app_token)
            // Device
            .header("X-Mme-Client-Info", &self.device_info.client_info)
            .header("X-Mme-Device-Id", &self.anisette.device_id)
            // Anisette
            .header("X-Apple-Locale", "en_US")
            .header("X-Apple-I-TimeZone", "UTC")
            .header(
                "X-Apple-I-Client-Time",
                now.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            )
            .header("X-Apple-I-MD-RINFO", self.anisette.routing_info.to_string())
            .header("X-Apple-I-MD-M", &self.anisette.machine_id)
            .header("X-Apple-I-MD-LU", &self.anisette.local_user_id)
            .header("X-Apple-I-MD", &self.anisette.otp)
            .send(body.as_bytes())
            .with_context(|| format!("Failed to call {} API", action))?;

        let response_body = response
            .body_mut()
            .read_to_string()
            .with_context(|| format!("Failed to read {} response", action))?;

        let plist: plist::Value = plist::from_bytes(response_body.as_bytes())
            .with_context(|| format!("Failed to parse {} response", action))?;

        // Check result code
        let dict = plist
            .as_dictionary()
            .ok_or_else(|| anyhow!("Invalid {} response format", action))?;

        let result_code = dict
            .get("resultCode")
            .and_then(|v| v.as_signed_integer())
            .unwrap_or(-1);

        if result_code != 0 {
            let msg = dict
                .get("userString")
                .or_else(|| dict.get("resultString"))
                .and_then(|v| v.as_string())
                .unwrap_or("Unknown error");
            return Err(anyhow!("{} failed ({}): {}", action, result_code, msg));
        }

        Ok(plist)
    }

    /// Make a platform-specific request (iOS)
    fn ios_request(
        &self,
        sub_action: &str,
        extra_params: &[(&str, plist::Value)],
    ) -> Result<plist::Value> {
        let action = format!("ios/{}", sub_action);
        let mut params: Vec<(&str, plist::Value)> = extra_params.to_vec();
        params.push(("DTDK_Platform", "ios".into()));
        self.request(&action, &params)
    }

    // =========================================================================
    // Device Management
    // =========================================================================

    /// List all registered devices for the team
    pub fn list_devices(&self) -> Result<Vec<RegisteredDevice>> {
        let response = self.ios_request("listDevices", &[])?;
        parse_devices_response(&response)
    }

    /// Register a new device
    pub fn add_device(&self, udid: &str, name: &str) -> Result<RegisteredDevice> {
        let params = vec![
            ("deviceNumber", plist::Value::String(udid.to_string())),
            ("name", plist::Value::String(name.to_string())),
        ];
        let response = self.ios_request("addDevice", &params)?;

        let dict = response
            .as_dictionary()
            .ok_or_else(|| anyhow!("Invalid addDevice response"))?;

        let device = dict
            .get("device")
            .and_then(|v| v.as_dictionary())
            .ok_or_else(|| anyhow!("Missing device in addDevice response"))?;

        parse_device(device)
    }

    // =========================================================================
    // Certificate Management
    // =========================================================================

    /// List all certificates for the team
    pub fn list_certificates(&self) -> Result<Vec<DeveloperCertificate>> {
        let response = self.ios_request("listAllDevelopmentCerts", &[])?;
        parse_certificates_response(&response)
    }

    /// Submit a Certificate Signing Request (CSR) to create a new certificate
    pub fn submit_development_csr(&self, csr_content: &str) -> Result<DeveloperCertificate> {
        let params = vec![
            ("csrContent", plist::Value::String(csr_content.to_string())),
            (
                "machineId",
                plist::Value::String(self.anisette.device_id.clone()),
            ),
            (
                "machineName",
                plist::Value::String("LingXia CLI".to_string()),
            ),
        ];
        let response = self.ios_request("submitDevelopmentCSR", &params)?;

        let dict = response
            .as_dictionary()
            .ok_or_else(|| anyhow!("Invalid submitDevelopmentCSR response"))?;

        let cert = dict
            .get("certRequest")
            .and_then(|v| v.as_dictionary())
            .ok_or_else(|| anyhow!("Missing certRequest in response"))?;

        let cert = parse_certificate(cert)?;

        // If the certificate content is not in the response, fetch it separately
        if cert.certificate_content.is_none() {
            let certs = self.list_certificates()?;
            let full_cert = certs
                .into_iter()
                .find(|c| c.id == cert.id)
                .ok_or_else(|| anyhow!("Certificate {} not found after submission", cert.id))?;
            return Ok(full_cert);
        }

        Ok(cert)
    }

    /// List all App IDs for the team
    pub fn list_app_ids(&self) -> Result<Vec<AppId>> {
        let response = self.ios_request("listAppIds", &[])?;
        parse_app_ids_response(&response)
    }

    /// Create a new App ID (Bundle ID)
    pub fn add_app_id(&self, identifier: &str, name: &str) -> Result<AppId> {
        let params = vec![
            ("identifier", plist::Value::String(identifier.to_string())),
            ("name", plist::Value::String(name.to_string())),
        ];
        let response = self.ios_request("addAppId", &params)?;

        let dict = response
            .as_dictionary()
            .ok_or_else(|| anyhow!("Invalid addAppId response"))?;

        let app_id = dict
            .get("appId")
            .and_then(|v| v.as_dictionary())
            .ok_or_else(|| anyhow!("Missing appId in response"))?;

        parse_app_id(app_id)
    }
    /// List all provisioning profiles for the team
    pub fn list_provisioning_profiles(&self) -> Result<Vec<ProvisioningProfile>> {
        let response = self.ios_request("listProvisioningProfiles", &[])?;
        parse_profiles_response(&response)
    }

    /// Download a provisioning profile by ID
    pub fn download_provisioning_profile(&self, profile_id: &str) -> Result<Vec<u8>> {
        let params = vec![(
            "provisioningProfileId",
            plist::Value::String(profile_id.to_string()),
        )];
        let response = self.ios_request("downloadProvisioningProfile", &params)?;

        let dict = response
            .as_dictionary()
            .ok_or_else(|| anyhow!("Invalid downloadProvisioningProfile response"))?;

        let profile = dict
            .get("provisioningProfile")
            .and_then(|v| v.as_dictionary())
            .ok_or_else(|| anyhow!("Missing provisioningProfile in response"))?;

        let encoded_profile = profile
            .get("encodedProfile")
            .and_then(|v| v.as_data())
            .ok_or_else(|| anyhow!("Missing encodedProfile in response"))?;

        Ok(encoded_profile.to_vec())
    }

    /// Create a new development provisioning profile
    pub fn create_provisioning_profile(
        &self,
        name: &str,
        app_id_id: &str,
        certificate_ids: &[&str],
        device_ids: &[&str],
    ) -> Result<ProvisioningProfile> {
        let cert_array: Vec<plist::Value> = certificate_ids
            .iter()
            .map(|id| plist::Value::String(id.to_string()))
            .collect();
        let device_array: Vec<plist::Value> = device_ids
            .iter()
            .map(|id| plist::Value::String(id.to_string()))
            .collect();

        let params = vec![
            (
                "provisioningProfileName",
                plist::Value::String(name.to_string()),
            ),
            ("appIdId", plist::Value::String(app_id_id.to_string())),
            ("certificateIds", plist::Value::Array(cert_array)),
            ("deviceIds", plist::Value::Array(device_array)),
            (
                "distributionType",
                plist::Value::String("limited".to_string()),
            ),
        ];
        let response = self.ios_request("createProvisioningProfile", &params)?;

        let dict = response
            .as_dictionary()
            .ok_or_else(|| anyhow!("Invalid createProvisioningProfile response"))?;

        let profile = dict
            .get("provisioningProfile")
            .and_then(|v| v.as_dictionary())
            .ok_or_else(|| anyhow!("Missing provisioningProfile in response"))?;

        parse_profile(profile)
    }

    /// Delete a provisioning profile
    pub fn delete_provisioning_profile(&self, profile_id: &str) -> Result<()> {
        let params = vec![(
            "provisioningProfileId",
            plist::Value::String(profile_id.to_string()),
        )];
        self.ios_request("deleteProvisioningProfile", &params)?;
        Ok(())
    }
}

/// A registered device in the developer portal
#[derive(Debug, Clone)]
pub struct RegisteredDevice {
    pub id: String,
    pub name: Option<String>,
    pub udid: String,
    pub platform: Option<String>,
    pub status: Option<String>,
    pub device_class: Option<String>,
    pub model: Option<String>,
    pub added_date: Option<String>,
}

/// A developer certificate
#[derive(Debug, Clone)]
pub struct DeveloperCertificate {
    pub id: String,
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub status: Option<String>,
    pub type_string: Option<String>,
    pub serial_number: Option<String>,
    pub date_created: Option<String>,
    pub expiration_date: Option<String>,
    /// Base64-encoded certificate content
    pub certificate_content: Option<String>,
}

/// An App ID (Bundle ID) in the developer portal
#[derive(Debug, Clone)]
pub struct AppId {
    pub id: String,
    pub name: Option<String>,
    pub identifier: String,
    pub platform: Option<String>,
}

/// A provisioning profile
#[derive(Debug, Clone)]
pub struct ProvisioningProfile {
    pub id: String,
    pub name: String,
    pub platform: Option<String>,
    pub status: Option<String>,
    pub profile_type: Option<String>,
    pub uuid: Option<String>,
    pub expiration_date: Option<String>,
    /// Team identifier from appId.prefix
    pub team_identifier: Option<String>,
    /// Application identifier from appId
    pub application_identifier: Option<String>,
    /// Entitlements (constructed from appId features)
    pub entitlements: Option<String>,
}

/// A developer team returned by listTeams
#[derive(Debug, Clone)]
pub struct DeveloperTeam {
    pub id: String,
    pub name: String,
    pub memberships: Vec<Membership>,
}

impl DeveloperTeam {
    /// Whether this is a free (non-paid) developer account
    pub fn is_free(&self) -> bool {
        !self
            .memberships
            .iter()
            .any(|m| m.platform == "ios" && m.name.contains("Apple Developer Program"))
    }

    /// Human-readable account type
    pub fn account_type(&self) -> &str {
        if self.is_free() { "Free" } else { "Paid" }
    }
}

/// A membership entry within a team
#[derive(Debug, Clone)]
pub struct Membership {
    pub name: String,
    pub platform: String,
}

/// Fetch the list of developer teams for an authenticated Apple ID.
///
/// `adsid` is the Apple ID Services ID and `app_token` is the Xcode
/// app-specific token obtained from `GrandSlamClient::fetch_app_tokens`.
pub fn list_teams(
    adsid: &str,
    app_token: &str,
    device_info: &DeviceInfo,
    anisette: &AnisetteData,
) -> Result<Vec<DeveloperTeam>> {
    let url = format!(
        "{}/{}/listTeams.action?clientId={}",
        DEVELOPER_SERVICES_BASE, PROTOCOL_VERSION, CLIENT_ID
    );

    // Build request body
    let request_id = uuid::Uuid::new_v4().to_string();
    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>requestId</key>
    <string>{}</string>
    <key>clientId</key>
    <string>{}</string>
    <key>protocolVersion</key>
    <string>{}</string>
    <key>userLocale</key>
    <array>
        <string>en_US</string>
    </array>
</dict>
</plist>"#,
        request_id, CLIENT_ID, PROTOCOL_VERSION
    );

    let now = chrono::Utc::now();

    let mut response = http_agent()
        .post(&url)
        .header("Accept", "text/x-xml-plist")
        .header("Content-Type", "text/x-xml-plist")
        .header("User-Agent", "Xcode")
        .header("X-Xcode-Version", "14.2 (14C18)")
        .header("X-Apple-App-Info", "com.apple.gs.xcode.auth")
        // Auth
        .header("X-Apple-I-Identity-Id", adsid)
        .header("X-Apple-GS-Token", app_token)
        // Device
        .header("X-Mme-Client-Info", &device_info.client_info)
        .header("X-Mme-Device-Id", &anisette.device_id)
        // Anisette
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
        .send(body.as_bytes())
        .context("Failed to call listTeams API")?;

    let response_body = response
        .body_mut()
        .read_to_string()
        .context("Failed to read listTeams response")?;

    parse_teams_response(&response_body)
}

/// Parse the plist response from listTeams.action
fn parse_teams_response(body: &str) -> Result<Vec<DeveloperTeam>> {
    let plist: plist::Value =
        plist::from_bytes(body.as_bytes()).context("Failed to parse listTeams response")?;

    let dict = plist
        .as_dictionary()
        .ok_or_else(|| anyhow!("Invalid listTeams response format"))?;

    // Check result code
    let result_code = dict
        .get("resultCode")
        .and_then(|v| v.as_signed_integer())
        .unwrap_or(-1);

    if result_code != 0 {
        let msg = dict
            .get("userString")
            .or_else(|| dict.get("resultString"))
            .and_then(|v| v.as_string())
            .unwrap_or("Unknown error");
        return Err(anyhow!("listTeams failed ({}): {}", result_code, msg));
    }

    let teams_array = dict
        .get("teams")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("Missing 'teams' in response"))?;

    let mut teams = Vec::new();
    for team_value in teams_array {
        let team_dict = match team_value.as_dictionary() {
            Some(d) => d,
            None => continue,
        };

        let id = team_dict
            .get("teamId")
            .and_then(|v| v.as_string())
            .unwrap_or("")
            .to_string();
        let name = team_dict
            .get("name")
            .and_then(|v| v.as_string())
            .unwrap_or("")
            .to_string();
        let mut memberships = Vec::new();
        if let Some(arr) = team_dict.get("memberships").and_then(|v| v.as_array()) {
            for m in arr {
                if let Some(md) = m.as_dictionary() {
                    let mname = md
                        .get("name")
                        .and_then(|v| v.as_string())
                        .unwrap_or("")
                        .to_string();
                    let platform = md
                        .get("platform")
                        .and_then(|v| v.as_string())
                        .unwrap_or("")
                        .to_lowercase();
                    memberships.push(Membership {
                        name: mname,
                        platform,
                    });
                }
            }
        }

        teams.push(DeveloperTeam {
            id,
            name,
            memberships,
        });
    }

    Ok(teams)
}

/// Parse devices from a listDevices response
fn parse_devices_response(response: &plist::Value) -> Result<Vec<RegisteredDevice>> {
    let dict = response
        .as_dictionary()
        .ok_or_else(|| anyhow!("Invalid listDevices response"))?;

    let devices_array = dict
        .get("devices")
        .and_then(|v| v.as_array())
        .unwrap_or(&Vec::new())
        .clone();

    let mut devices = Vec::new();
    for device_value in devices_array {
        if let Some(device_dict) = device_value.as_dictionary() {
            if let Ok(device) = parse_device(device_dict) {
                devices.push(device);
            }
        }
    }

    Ok(devices)
}

/// Parse a single device from a plist dictionary
fn parse_device(dict: &plist::Dictionary) -> Result<RegisteredDevice> {
    let id = dict
        .get("deviceId")
        .or_else(|| dict.get("deviceNumber"))
        .and_then(|v| v.as_string())
        .unwrap_or("")
        .to_string();

    let name = dict
        .get("name")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let udid = dict
        .get("deviceNumber")
        .and_then(|v| v.as_string())
        .unwrap_or("")
        .to_string();

    let platform = dict
        .get("devicePlatform")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let status = dict
        .get("status")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let device_class = dict
        .get("deviceClass")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let model = dict
        .get("model")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let added_date = dict
        .get("dateCreated")
        .or_else(|| dict.get("addedDate"))
        .or_else(|| dict.get("registrationDate"))
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    Ok(RegisteredDevice {
        id,
        name,
        udid,
        platform,
        status,
        device_class,
        model,
        added_date,
    })
}

/// Parse certificates from a listAllDevelopmentCerts response
fn parse_certificates_response(response: &plist::Value) -> Result<Vec<DeveloperCertificate>> {
    let dict = response
        .as_dictionary()
        .ok_or_else(|| anyhow!("Invalid listAllDevelopmentCerts response"))?;

    let certs_array = dict
        .get("certificates")
        .or_else(|| dict.get("certRequests"))
        .and_then(|v| v.as_array())
        .unwrap_or(&Vec::new())
        .clone();

    let mut certs = Vec::new();
    for cert_value in certs_array {
        if let Some(cert_dict) = cert_value.as_dictionary() {
            if let Ok(cert) = parse_certificate(cert_dict) {
                certs.push(cert);
            }
        }
    }

    Ok(certs)
}

/// Parse a single certificate from a plist dictionary
fn parse_certificate(dict: &plist::Dictionary) -> Result<DeveloperCertificate> {
    let id = dict
        .get("certRequestId")
        .or_else(|| dict.get("certificateId"))
        .and_then(|v| v.as_string())
        .unwrap_or("")
        .to_string();

    let name = dict
        .get("name")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let display_name = dict
        .get("ownerName")
        .or_else(|| dict.get("displayName"))
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let status = dict
        .get("status")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let type_string = dict
        .get("typeString")
        .or_else(|| dict.get("certificateType"))
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let serial_number = dict
        .get("serialNumber")
        .or_else(|| dict.get("serialNum"))
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let date_created = dict
        .get("dateCreated")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let expiration_date = dict
        .get("expirationDate")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let certificate_content_value = [
        "certContent",
        "certificateContent",
        "certData",
        "certificateData",
    ]
    .into_iter()
    .find_map(|k| dict.get(k));

    let certificate_content = match certificate_content_value {
        Some(plist::Value::String(s)) => Some(s.to_string()),
        Some(plist::Value::Data(data)) => {
            use base64::Engine;
            Some(base64::engine::general_purpose::STANDARD.encode(data))
        }
        _ => None,
    };

    Ok(DeveloperCertificate {
        id,
        name,
        display_name,
        status,
        type_string,
        serial_number,
        date_created,
        expiration_date,
        certificate_content,
    })
}

/// Parse App IDs from a listAppIds response
fn parse_app_ids_response(response: &plist::Value) -> Result<Vec<AppId>> {
    let dict = response
        .as_dictionary()
        .ok_or_else(|| anyhow!("Invalid listAppIds response"))?;

    let app_ids_array = dict
        .get("appIds")
        .and_then(|v| v.as_array())
        .unwrap_or(&Vec::new())
        .clone();

    let mut app_ids = Vec::new();
    for app_id_value in app_ids_array {
        if let Some(app_id_dict) = app_id_value.as_dictionary() {
            if let Ok(app_id) = parse_app_id(app_id_dict) {
                app_ids.push(app_id);
            }
        }
    }

    Ok(app_ids)
}

/// Parse a single App ID from a plist dictionary
fn parse_app_id(dict: &plist::Dictionary) -> Result<AppId> {
    let id = dict
        .get("appIdId")
        .and_then(|v| v.as_string())
        .unwrap_or("")
        .to_string();

    let name = dict
        .get("name")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let identifier = dict
        .get("identifier")
        .and_then(|v| v.as_string())
        .unwrap_or("")
        .to_string();

    let platform = dict
        .get("appIdPlatform")
        .or_else(|| dict.get("platform"))
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    Ok(AppId {
        id,
        name,
        identifier,
        platform,
    })
}

/// Parse provisioning profiles from a listProvisioningProfiles response
fn parse_profiles_response(response: &plist::Value) -> Result<Vec<ProvisioningProfile>> {
    let dict = response
        .as_dictionary()
        .ok_or_else(|| anyhow!("Invalid listProvisioningProfiles response"))?;

    let profiles_array = dict
        .get("provisioningProfiles")
        .and_then(|v| v.as_array())
        .unwrap_or(&Vec::new())
        .clone();

    let mut profiles = Vec::new();
    for profile_value in profiles_array {
        if let Some(profile_dict) = profile_value.as_dictionary() {
            if let Ok(profile) = parse_profile(profile_dict) {
                profiles.push(profile);
            }
        }
    }

    Ok(profiles)
}

/// Parse a single provisioning profile from a plist dictionary
fn parse_profile(dict: &plist::Dictionary) -> Result<ProvisioningProfile> {
    let id = dict
        .get("provisioningProfileId")
        .and_then(|v| v.as_string())
        .unwrap_or("")
        .to_string();

    let name = dict
        .get("name")
        .and_then(|v| v.as_string())
        .unwrap_or("")
        .to_string();

    let platform = dict
        .get("devicePlatform")
        .or_else(|| dict.get("platform"))
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let status = dict
        .get("status")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let profile_type = dict
        .get("type")
        .or_else(|| dict.get("profileType"))
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let uuid = dict
        .get("UUID")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let expiration_date = dict
        .get("dateExpire")
        .or_else(|| dict.get("expirationDate"))
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    // Extract team identifier and entitlements from appId dictionary
    let (team_identifier, application_identifier, entitlements) =
        if let Some(plist::Value::Dictionary(app_id)) = dict.get("appId") {
            let team_id = app_id
                .get("prefix")
                .and_then(|v| v.as_string())
                .map(|s| s.to_string());

            let identifier = app_id
                .get("identifier")
                .and_then(|v| v.as_string())
                .map(|s| s.to_string());

            let app_identifier = if let (Some(prefix), Some(id)) = (&team_id, &identifier) {
                Some(format!("{}.{}", prefix, id))
            } else {
                None
            };

            // Construct entitlements from features
            let ents = if let Some(plist::Value::Dictionary(features)) = app_id.get("features") {
                let mut ents_map = serde_json::Map::new();

                // Standard entitlements
                if let Some(ref team) = team_id {
                    ents_map.insert(
                        "com.apple.developer.team-identifier".to_string(),
                        serde_json::Value::String(team.clone()),
                    );
                }

                if let Some(ref app_id) = app_identifier {
                    ents_map.insert(
                        "application-identifier".to_string(),
                        serde_json::Value::String(app_id.clone()),
                    );
                }

                if let Some(ref team) = team_id {
                    let keychain_groups = vec![
                        serde_json::Value::String(format!("{}.*", team)),
                        serde_json::Value::String("com.apple.token".to_string()),
                    ];
                    ents_map.insert(
                        "keychain-access-groups".to_string(),
                        serde_json::Value::Array(keychain_groups),
                    );
                }

                // Add aps-environment if push is enabled
                if let Some(plist::Value::Boolean(true)) = features.get("push") {
                    ents_map.insert(
                        "aps-environment".to_string(),
                        serde_json::Value::String("development".to_string()),
                    );
                }

                // Add get-task-allow for development profiles
                if profile_type.as_deref() == Some("iOS Development") {
                    ents_map.insert("get-task-allow".to_string(), serde_json::Value::Bool(true));
                }

                // Add associated domains if enabled (SKC3T5S89Y is associated domains capability)
                if let Some(plist::Value::Boolean(true)) = features.get("SKC3T5S89Y") {
                    ents_map.insert(
                        "com.apple.developer.associated-domains".to_string(),
                        serde_json::Value::String("*".to_string()),
                    );
                }

                Some(serde_json::to_string(&ents_map).unwrap_or_default())
            } else {
                None
            };

            (team_id, app_identifier, ents)
        } else {
            (None, None, None)
        };

    Ok(ProvisioningProfile {
        id,
        name,
        platform,
        status,
        profile_type,
        uuid,
        expiration_date,
        team_identifier,
        application_identifier,
        entitlements,
    })
}
