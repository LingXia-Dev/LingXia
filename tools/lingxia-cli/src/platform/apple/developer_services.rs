//! Apple Developer Services API client.
//!
//! Used to fetch developer team information after Apple ID authentication.

use anyhow::{Context, Result, anyhow};

use super::anisette::AnisetteData;
use super::grandslam::DeviceInfo;
use super::http_agent;

const DEVELOPER_SERVICES_BASE: &str = "https://developerservices2.apple.com/services";
const PROTOCOL_VERSION: &str = "QH65B2";
const CLIENT_ID: &str = "XABBG36SBA";

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
