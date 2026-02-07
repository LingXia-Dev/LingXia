//! Common utilities for Apple Developer Services commands.

use crate::platform::apple::anisette::OmnisetteProvider;
use crate::platform::apple::auth::CredentialStorage;
use crate::platform::apple::developer_services::DeveloperServicesClient;
use crate::platform::apple::grandslam::DeviceInfo;
use anyhow::{Context, Result, anyhow};

/// Helper to execute a closure with an authenticated DeveloperServicesClient.
///
/// This handles loading credentials, refreshing anisette data, and creating
/// the properly authenticated client.
pub fn with_client<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&DeveloperServicesClient) -> Result<T>,
{
    let storage = CredentialStorage::new()?;
    let credentials = storage
        .load()?
        .ok_or_else(|| anyhow!("Not logged in. Run 'lingxia auth apple login' first."))?;

    // Currently only AppleId credentials support Developer Services
    let (adsid, app_token, team_id) = match &credentials {
        crate::platform::apple::auth::AuthCredentials::AppleId {
            adsid,
            app_token,
            team_id,
            ..
        } => (adsid.clone(), app_token.clone(), team_id.clone()),
        crate::platform::apple::auth::AuthCredentials::AppStoreConnect { .. } => {
            return Err(anyhow!(
                "App Store Connect API keys are not supported for this command.\n\
                 Run 'lingxia auth apple login' and choose Password mode instead."
            ));
        }
    };

    // Get fresh anisette data
    let mut anisette_provider = OmnisetteProvider::new();
    let anisette = anisette_provider
        .fetch_anisette_data()
        .context("Failed to get anisette data")?;

    let device_info = DeviceInfo::default_macos();

    let client =
        DeveloperServicesClient::new(&adsid, &app_token, &team_id, &device_info, &anisette);

    f(&client)
}
