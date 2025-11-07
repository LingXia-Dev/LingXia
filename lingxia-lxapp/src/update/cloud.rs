use crate::app::AppConfig;
use crate::error::LxAppError;
use crate::lxapp::ReleaseType;
use crate::update::{UpdateCheckResult, UpdateManager, UpdatePackageInfo};
use rong::service_executor;
use serde::{Deserialize, Serialize};

impl UpdateManager {
    /// Check with the cloud whether a newer package is available.
    ///
    /// - `current_version`: None means client requests the latest package regardless of current state (first install).
    /// - Returns `UpdateCheckResult` with a package URL and checksum when an update is available.
    pub async fn check_update(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
        current_version: Option<&str>,
    ) -> Result<UpdateCheckResult, LxAppError> {
        let config = AppConfig::global().ok_or_else(|| {
            LxAppError::Runtime("App configuration not loaded; cannot perform check-update".into())
        })?;

        let base = config
            .api_server
            .as_deref()
            .ok_or_else(|| LxAppError::Runtime("apiServer not configured in app.json".into()))?;

        let endpoint = format!(
            "{}/api/v1/lxapps/{}/check-update",
            base.trim_end_matches('/'),
            lxappid
        );

        let api_key = config.api_key.as_deref();

        let request_body = CloudCheckUpdateRequest { current_version };

        let response = perform_check_update_request(&endpoint, api_key, &request_body).await?;

        convert_cloud_response(response, lxappid, release_type)
    }
}

async fn perform_check_update_request(
    url: &str,
    api_key: Option<&str>,
    body: &CloudCheckUpdateRequest<'_>,
) -> Result<CloudCheckUpdateResponse, LxAppError> {
    let payload = serde_json::to_vec(body).map_err(|e| {
        LxAppError::Runtime(format!(
            "failed to serialize check-update request body: {}",
            e
        ))
    })?;

    let header_pairs = api_key.map(|key| [("X-API-Key", key)]);
    let headers = header_pairs.as_ref().map(|pairs| pairs.as_slice());

    let (status, body_bytes) = service_executor::post_json(url, &payload, headers)
        .await
        .map_err(|e| LxAppError::Runtime(format!("check-update request failed: {}", e)))?;

    if !status.is_success() {
        return Err(LxAppError::Runtime(format!(
            "check-update http status: {}",
            status
        )));
    }

    let parsed: CloudCheckUpdateResponse = serde_json::from_slice(&body_bytes).map_err(|e| {
        LxAppError::Runtime(format!("invalid check-update response payload: {}", e))
    })?;

    Ok(parsed)
}

fn convert_cloud_response(
    response: CloudCheckUpdateResponse,
    lxappid: &str,
    _release_type: ReleaseType,
) -> Result<UpdateCheckResult, LxAppError> {
    if response.code != 200 {
        let message = response
            .message
            .unwrap_or_else(|| "check-update failed".to_string());
        return Err(LxAppError::Runtime(format!(
            "check-update returned code {}: {}",
            response.code, message
        )));
    }

    let data = match response.data {
        Some(data) => data,
        None => {
            return Ok(UpdateCheckResult {
                has_update: false,
                package: None,
            });
        }
    };

    if !data.has_update {
        return Ok(UpdateCheckResult {
            has_update: false,
            package: None,
        });
    }

    let info = data.update_info.ok_or_else(|| {
        LxAppError::Runtime("check-update response missing updateInfo".to_string())
    })?;

    if info.lx_app_id != lxappid {
        return Err(LxAppError::Runtime(format!(
            "check-update returned mismatched lxAppId: expected {}, got {}",
            lxappid, info.lx_app_id
        )));
    }

    if info.package_url.is_empty() {
        return Err(LxAppError::Runtime(
            "check-update response missing packageUrl".to_string(),
        ));
    }
    if info.version.is_empty() {
        return Err(LxAppError::Runtime(
            "check-update response missing version".to_string(),
        ));
    }
    if info.sha256.trim().is_empty() {
        return Err(LxAppError::Runtime(
            "check-update response missing sha256 checksum".to_string(),
        ));
    }

    Ok(UpdateCheckResult {
        has_update: true,
        package: Some(UpdatePackageInfo {
            version: info.version,
            url: info.package_url,
            checksum_sha256: info.sha256,
        }),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CloudCheckUpdateRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    current_version: Option<&'a str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudCheckUpdateResponse {
    code: i32,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    data: Option<CloudCheckUpdateData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudCheckUpdateData {
    has_update: bool,
    #[serde(default)]
    update_info: Option<CloudUpdateInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudUpdateInfo {
    #[serde(rename = "lxAppId")]
    lx_app_id: String,
    version: String,
    package_url: String,
    #[serde(rename = "sha256")]
    sha256: String,
}
