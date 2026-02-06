//! Anisette data provider using Omnisette service.
//!
//! Anisette data is device fingerprinting required by Apple's authentication.
//! We use the public Omnisette service (sidestore.io) to generate this data.

use super::http_agent;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::net::TcpStream;
use tungstenite::{Message, WebSocket, connect, stream::MaybeTlsStream};
use uuid::Uuid;

const OMNISETTE_URL: &str = "https://ani.sidestore.io";
const OMNISETTE_WS_URL: &str = "wss://ani.sidestore.io/v3/provisioning_session";

/// Anisette data required for Apple authentication
#[derive(Debug, Clone)]
pub struct AnisetteData {
    /// Machine ID (X-Apple-I-MD-M)
    pub machine_id: String,
    /// One-time password (X-Apple-I-MD)
    pub otp: String,
    /// Routing info (X-Apple-I-MD-RINFO)
    pub routing_info: u64,
    /// Local user ID (X-Apple-I-MD-LU)
    pub local_user_id: String,
    /// Device ID (X-Mme-Device-Id)
    pub device_id: String,
}

/// Omnisette Anisette provider
pub struct OmnisetteProvider {
    url: String,
    local_user_uid: Uuid,
    provisioning_data: Option<ProvisioningData>,
    client_info: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProvisioningData {
    adi_pb: Vec<u8>,
    routing_info: u64,
}

impl OmnisetteProvider {
    /// Create a new Omnisette provider
    pub fn new() -> Self {
        Self {
            url: OMNISETTE_URL.to_string(),
            local_user_uid: Uuid::new_v4(),
            provisioning_data: None,
            client_info: None,
        }
    }

    /// Load provisioning data from cache file
    pub fn load_cached(&mut self) -> Result<bool> {
        let cache_path = Self::cache_path()?;
        if cache_path.exists() {
            let data = std::fs::read_to_string(&cache_path)
                .context("Failed to read provisioning cache")?;

            #[derive(Deserialize)]
            struct CacheData {
                local_user_uid: String,
                provisioning_data: ProvisioningData,
            }

            if let Ok(cache) = serde_json::from_str::<CacheData>(&data)
                && let Ok(uid) = Uuid::parse_str(&cache.local_user_uid)
            {
                self.local_user_uid = uid;
                self.provisioning_data = Some(cache.provisioning_data);
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Save provisioning data to cache file
    fn save_cached(&self) -> Result<()> {
        if let Some(ref prov) = self.provisioning_data {
            let cache_path = Self::cache_path()?;
            if let Some(parent) = cache_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            #[derive(Serialize)]
            struct CacheData<'a> {
                local_user_uid: String,
                provisioning_data: &'a ProvisioningData,
            }

            let cache = CacheData {
                local_user_uid: self.local_user_uid.to_string(),
                provisioning_data: prov,
            };

            let data = serde_json::to_string_pretty(&cache)?;
            std::fs::write(&cache_path, data)?;
        }
        Ok(())
    }

    fn cache_path() -> Result<std::path::PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        Ok(home.join(".lingxia").join("anisette_cache.json"))
    }

    fn get_client_info(&mut self) -> Result<String> {
        if let Some(ref info) = self.client_info {
            return Ok(info.clone());
        }

        #[derive(Deserialize)]
        struct ClientInfoResponse {
            client_info: String,
        }

        let url = format!("{}/v3/client_info", self.url);
        let mut response = http_agent()
            .get(&url)
            .call()
            .context("Failed to fetch client info from Omnisette")?;

        let body = response
            .body_mut()
            .read_to_string()
            .context("Failed to read client info response")?;
        let info: ClientInfoResponse =
            serde_json::from_str(&body).context("Failed to parse client info response")?;

        self.client_info = Some(info.client_info.clone());
        Ok(info.client_info)
    }

    /// Fetch anisette data (provision if needed)
    pub fn fetch_anisette_data(&mut self) -> Result<AnisetteData> {
        // Ensure client info is available (required by Apple GSA provisioning calls).
        let _ = self.get_client_info()?;

        // Compute local user ID (SHA256 hash of UUID)
        let local_user_id = compute_local_user_id(&self.local_user_uid);

        // Try to load cached provisioning data
        if self.provisioning_data.is_none() {
            let _ = self.load_cached();
        }

        // Try to get OTP if we have provisioning data
        if let Some(ref prov) = self.provisioning_data {
            match self.request_otp(prov) {
                Ok((machine_id, otp, routing_info)) => {
                    return Ok(AnisetteData {
                        machine_id,
                        otp,
                        routing_info,
                        local_user_id,
                        device_id: self.local_user_uid.to_string().to_uppercase(),
                    });
                }
                Err(e) => {
                    eprintln!("  OTP request failed, re-provisioning: {}", e);
                    self.provisioning_data = None;
                }
            }
        }

        // Need to provision
        eprintln!("  Provisioning with Omnisette (this may take a moment)...");
        self.provision()?;

        // Try again with new provisioning data
        let prov = self
            .provisioning_data
            .as_ref()
            .ok_or_else(|| anyhow!("Provisioning failed"))?;

        let (machine_id, otp, routing_info) = self.request_otp(prov)?;

        Ok(AnisetteData {
            machine_id,
            otp,
            routing_info,
            local_user_id,
            device_id: self.local_user_uid.to_string().to_uppercase(),
        })
    }

    /// Provision with Omnisette service via WebSocket
    fn provision(&mut self) -> Result<()> {
        // Connect to WebSocket
        let (mut socket, _response) =
            connect(OMNISETTE_WS_URL).context("Failed to connect to Omnisette WebSocket")?;

        // Step 1: Receive "GiveIdentifier"
        let msg = receive_message(&mut socket)?;
        if msg.result != "GiveIdentifier" {
            return Err(anyhow!("Expected GiveIdentifier, got {}", msg.result));
        }

        // Send identifier (UUID bytes)
        let identifier = self.local_user_uid.as_bytes().to_vec();
        send_json(
            &mut socket,
            &serde_json::json!({
                "identifier": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &identifier)
            }),
        )?;

        // Step 2: Receive "GiveStartProvisioningData"
        let msg = receive_message(&mut socket)?;
        if msg.result != "GiveStartProvisioningData" {
            return Err(anyhow!(
                "Expected GiveStartProvisioningData, got {}",
                msg.result
            ));
        }

        // We need to get SPIM from Apple's GSA endpoint
        let spim = self.fetch_start_provisioning()?;

        // Send SPIM
        send_json(
            &mut socket,
            &serde_json::json!({
                "spim": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &spim)
            }),
        )?;

        // Step 3: Receive "GiveEndProvisioningData" with CPIM
        let msg = receive_message(&mut socket)?;
        if msg.result != "GiveEndProvisioningData" {
            return Err(anyhow!(
                "Expected GiveEndProvisioningData, got {}",
                msg.result
            ));
        }

        let cpim = msg
            .data
            .get("cpim")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing cpim in response"))?;
        let cpim_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, cpim)
            .context("Failed to decode cpim")?;

        // Step 4: Send CPIM to Apple to get PTM/TK
        let (ptm, tk, routing_info) = self.fetch_end_provisioning(&cpim_bytes)?;

        // Send PTM and TK back to Omnisette
        send_json(
            &mut socket,
            &serde_json::json!({
                "ptm": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &ptm),
                "tk": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &tk)
            }),
        )?;

        // Step 5: Receive "ProvisioningSuccess" with adi_pb
        let msg = receive_message(&mut socket)?;
        if msg.result != "ProvisioningSuccess" {
            return Err(anyhow!("Provisioning failed: {}", msg.result));
        }

        let adi_pb = msg
            .data
            .get("adi_pb")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing adi_pb in response"))?;
        let adi_pb_bytes =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, adi_pb)
                .context("Failed to decode adi_pb")?;

        // Save provisioning data
        self.provisioning_data = Some(ProvisioningData {
            adi_pb: adi_pb_bytes,
            routing_info,
        });

        // Cache for future use
        let _ = self.save_cached();

        socket.close(None).ok();

        Ok(())
    }

    /// Fetch start provisioning data from Apple GSA
    fn fetch_start_provisioning(&self) -> Result<Vec<u8>> {
        let client_info = self
            .client_info
            .as_ref()
            .ok_or_else(|| anyhow!("Client info not available"))?;
        let local_user_id = compute_local_user_id(&self.local_user_uid);

        // Build request
        let request_body = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Header</key>
    <dict/>
    <key>Request</key>
    <dict/>
</dict>
</plist>"#;

        // First, lookup the endpoint
        let mut lookup_response = http_agent()
            .get("https://gsa.apple.com/grandslam/GsService2/lookup")
            .header("X-Mme-Client-Info", client_info)
            .header(
                "X-Mme-Device-Id",
                &self.local_user_uid.to_string().to_uppercase(),
            )
            .call()
            .context("Failed to lookup GSA endpoints")?;

        let lookup_body = lookup_response.body_mut().read_to_string()?;
        let lookup_plist: plist::Value = plist::from_bytes(lookup_body.as_bytes())?;

        let start_url = lookup_plist
            .as_dictionary()
            .and_then(|d| d.get("urls"))
            .and_then(|v| v.as_dictionary())
            .and_then(|d| d.get("midStartProvisioning"))
            .and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("midStartProvisioning endpoint not found"))?;

        // Send start provisioning request
        let mut response = http_agent()
            .post(start_url)
            .header("Content-Type", "text/x-xml-plist")
            .header("X-Mme-Client-Info", client_info)
            .header(
                "X-Mme-Device-Id",
                &self.local_user_uid.to_string().to_uppercase(),
            )
            .header("X-Apple-I-MD-LU", &local_user_id)
            .send(request_body.as_bytes())
            .context("Failed to start provisioning")?;

        let body = response.body_mut().read_to_string()?;
        let plist_response: plist::Value = plist::from_bytes(body.as_bytes())?;

        // Check for error
        if let Some(status) = plist_response
            .as_dictionary()
            .and_then(|d| d.get("Response"))
            .and_then(|v| v.as_dictionary())
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
                return Err(anyhow!("Start provisioning failed: {} ({})", em, ec));
            }
        }

        let spim = plist_response
            .as_dictionary()
            .and_then(|d| d.get("Response"))
            .and_then(|v| v.as_dictionary())
            .and_then(|d| d.get("spim"))
            .and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("Missing spim in response"))?;

        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, spim)
            .context("Failed to decode spim")
    }

    /// Fetch end provisioning data from Apple GSA
    fn fetch_end_provisioning(&self, cpim: &[u8]) -> Result<(Vec<u8>, Vec<u8>, u64)> {
        let client_info = self
            .client_info
            .as_ref()
            .ok_or_else(|| anyhow!("Client info not available"))?;
        let local_user_id = compute_local_user_id(&self.local_user_uid);

        let cpim_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, cpim);

        // Build request
        let request_body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Header</key>
    <dict/>
    <key>Request</key>
    <dict>
        <key>cpim</key>
        <string>{}</string>
    </dict>
</dict>
</plist>"#,
            cpim_b64
        );

        // Lookup endpoint
        let mut lookup_response = http_agent()
            .get("https://gsa.apple.com/grandslam/GsService2/lookup")
            .header("X-Mme-Client-Info", client_info)
            .header(
                "X-Mme-Device-Id",
                &self.local_user_uid.to_string().to_uppercase(),
            )
            .call()
            .context("Failed to lookup GSA endpoints")?;

        let lookup_body = lookup_response.body_mut().read_to_string()?;
        let lookup_plist: plist::Value = plist::from_bytes(lookup_body.as_bytes())?;

        let end_url = lookup_plist
            .as_dictionary()
            .and_then(|d| d.get("urls"))
            .and_then(|v| v.as_dictionary())
            .and_then(|d| d.get("midFinishProvisioning"))
            .and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("midFinishProvisioning endpoint not found"))?;

        // Send end provisioning request
        let mut response = http_agent()
            .post(end_url)
            .header("Content-Type", "text/x-xml-plist")
            .header("X-Mme-Client-Info", client_info)
            .header(
                "X-Mme-Device-Id",
                &self.local_user_uid.to_string().to_uppercase(),
            )
            .header("X-Apple-I-MD-LU", &local_user_id)
            .send(request_body.as_bytes())
            .context("Failed to end provisioning")?;

        let body = response.body_mut().read_to_string()?;
        let plist_response: plist::Value = plist::from_bytes(body.as_bytes())?;

        // Check for error
        if let Some(status) = plist_response
            .as_dictionary()
            .and_then(|d| d.get("Response"))
            .and_then(|v| v.as_dictionary())
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
                return Err(anyhow!("End provisioning failed: {} ({})", em, ec));
            }
        }

        let resp = plist_response
            .as_dictionary()
            .and_then(|d| d.get("Response"))
            .and_then(|v| v.as_dictionary())
            .ok_or_else(|| anyhow!("Invalid end provisioning response"))?;

        let ptm = resp
            .get("ptm")
            .and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("Missing ptm"))?;
        let tk = resp
            .get("tk")
            .and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("Missing tk"))?;
        let rinfo = resp
            .get("X-Apple-I-MD-RINFO")
            .and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("Missing routing info"))?;

        let ptm_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, ptm)?;
        let tk_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, tk)?;
        let routing_info = rinfo.parse::<u64>()?;

        Ok((ptm_bytes, tk_bytes, routing_info))
    }

    /// Request OTP using provisioning data
    fn request_otp(&self, prov: &ProvisioningData) -> Result<(String, String, u64)> {
        #[derive(Serialize)]
        struct Request {
            identifier: String,
            adi_pb: String,
        }

        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "X-Apple-I-MD-M")]
            machine_id: String,
            #[serde(rename = "X-Apple-I-MD")]
            otp: String,
            #[serde(rename = "X-Apple-I-MD-RINFO")]
            routing_info: String,
        }

        let identifier_bytes: [u8; 16] = *self.local_user_uid.as_bytes();
        let request = Request {
            identifier: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                identifier_bytes,
            ),
            adi_pb: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &prov.adi_pb,
            ),
        };

        let url = format!("{}/v3/get_headers", self.url);
        let body = serde_json::to_string(&request)?;

        let mut response = http_agent()
            .post(&url)
            .header("Content-Type", "application/json")
            .send(body.as_bytes())
            .context("Failed to request OTP from Omnisette")?;

        let response_body = response
            .body_mut()
            .read_to_string()
            .context("Failed to read OTP response")?;

        let resp: Response =
            serde_json::from_str(&response_body).context("Failed to parse OTP response")?;

        let routing_info = resp
            .routing_info
            .parse::<u64>()
            .context("Invalid routing info")?;

        Ok((resp.machine_id, resp.otp, routing_info))
    }
}

/// Compute local user ID from UUID (SHA256 hash, uppercase hex)
fn compute_local_user_id(uuid: &Uuid) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(uuid.to_string().as_bytes());
    hash.iter().map(|b| format!("{:02X}", b)).collect()
}

/// WebSocket message wrapper
#[derive(Debug, Deserialize)]
struct WsMessage {
    result: String,
    #[serde(flatten)]
    data: serde_json::Map<String, serde_json::Value>,
}

fn receive_message(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>) -> Result<WsMessage> {
    loop {
        match socket.read()? {
            Message::Text(text) => {
                return serde_json::from_str(&text).context("Failed to parse WebSocket message");
            }
            Message::Binary(data) => {
                return serde_json::from_slice(&data)
                    .context("Failed to parse WebSocket binary message");
            }
            Message::Ping(data) => {
                socket.send(Message::Pong(data))?;
            }
            Message::Pong(_) => {}
            Message::Close(_) => {
                return Err(anyhow!("WebSocket connection closed"));
            }
            Message::Frame(_) => {}
        }
    }
}

fn send_json(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    value: &serde_json::Value,
) -> Result<()> {
    let text = serde_json::to_string(value)?;
    socket.send(Message::Text(text.into()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_local_user_id() {
        let uuid = Uuid::parse_str("12345678-1234-1234-1234-123456789012").unwrap();
        let id = compute_local_user_id(&uuid);
        assert_eq!(id.len(), 64); // SHA256 = 32 bytes = 64 hex chars
    }
}
