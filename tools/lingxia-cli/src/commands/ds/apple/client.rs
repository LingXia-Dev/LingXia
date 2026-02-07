//! Common utilities for Apple Developer Services commands.

use crate::platform::apple::anisette::OmnisetteProvider;
use crate::platform::apple::asc::AppStoreConnectClient;
use crate::platform::apple::auth::{AuthCredentials, CredentialStorage};
use crate::platform::apple::developer_services;
use crate::platform::apple::developer_services::DeveloperServicesClient;
use crate::platform::apple::grandslam::DeviceInfo;
use anyhow::{Context, Result};

pub enum AppleDsClient {
    Private(Box<PrivateDsAuth>),
    ApiKey {
        team_id: String,
        client: AppStoreConnectClient,
    },
}

pub struct PrivateDsAuth {
    adsid: String,
    app_token: String,
    team_id: String,
    device_info: DeviceInfo,
    anisette: crate::platform::apple::anisette::AnisetteData,
}

impl AppleDsClient {
    pub fn list_teams(&self) -> Result<Vec<developer_services::DeveloperTeam>> {
        match self {
            Self::Private(auth) => developer_services::list_teams(
                &auth.adsid,
                &auth.app_token,
                &auth.device_info,
                &auth.anisette,
            ),
            Self::ApiKey { team_id, .. } => Ok(vec![developer_services::DeveloperTeam {
                id: team_id.clone(),
                name: "App Store Connect Team".to_string(),
                memberships: vec![developer_services::Membership {
                    name: "Apple Developer Program".to_string(),
                    platform: "ios".to_string(),
                }],
            }]),
        }
    }

    pub fn list_devices(&self) -> Result<Vec<developer_services::RegisteredDevice>> {
        match self {
            Self::Private(auth) => {
                let client = DeveloperServicesClient::new(
                    &auth.adsid,
                    &auth.app_token,
                    &auth.team_id,
                    &auth.device_info,
                    &auth.anisette,
                );
                client.list_devices()
            }
            Self::ApiKey { client, .. } => {
                let devices = client.list_devices()?;
                Ok(devices
                    .into_iter()
                    .map(|d| developer_services::RegisteredDevice {
                        id: d.id,
                        name: d.attributes.name,
                        udid: d.attributes.udid.unwrap_or_default(),
                        platform: d.attributes.platform,
                        status: d.attributes.status,
                        device_class: d.attributes.device_class,
                        model: d.attributes.model,
                        added_date: None,
                    })
                    .collect())
            }
        }
    }

    pub fn list_certificates(&self) -> Result<Vec<developer_services::DeveloperCertificate>> {
        match self {
            Self::Private(auth) => {
                let client = DeveloperServicesClient::new(
                    &auth.adsid,
                    &auth.app_token,
                    &auth.team_id,
                    &auth.device_info,
                    &auth.anisette,
                );
                client.list_certificates()
            }
            Self::ApiKey { client, .. } => {
                let certs = client.list_certificates()?;
                Ok(certs
                    .into_iter()
                    .map(|c| developer_services::DeveloperCertificate {
                        id: c.id,
                        name: c.attributes.name,
                        display_name: c.attributes.display_name,
                        status: None,
                        type_string: c.attributes.certificate_type,
                        serial_number: c.attributes.serial_number,
                        expiration_date: c.attributes.expiration_date,
                        certificate_content: c.attributes.certificate_content,
                    })
                    .collect())
            }
        }
    }

    pub fn list_app_ids(&self) -> Result<Vec<developer_services::AppId>> {
        match self {
            Self::Private(auth) => {
                let client = DeveloperServicesClient::new(
                    &auth.adsid,
                    &auth.app_token,
                    &auth.team_id,
                    &auth.device_info,
                    &auth.anisette,
                );
                client.list_app_ids()
            }
            Self::ApiKey { client, .. } => {
                let app_ids = client.list_bundle_ids()?;
                Ok(app_ids
                    .into_iter()
                    .map(|b| developer_services::AppId {
                        id: b.id,
                        name: b.attributes.name,
                        identifier: b.attributes.identifier.unwrap_or_default(),
                        platform: b.attributes.platform,
                    })
                    .collect())
            }
        }
    }

    pub fn list_provisioning_profiles(
        &self,
    ) -> Result<Vec<developer_services::ProvisioningProfile>> {
        match self {
            Self::Private(auth) => {
                let client = DeveloperServicesClient::new(
                    &auth.adsid,
                    &auth.app_token,
                    &auth.team_id,
                    &auth.device_info,
                    &auth.anisette,
                );
                client.list_provisioning_profiles()
            }
            Self::ApiKey { team_id, client } => {
                let profiles = client.list_profiles()?;
                Ok(profiles
                    .into_iter()
                    .map(|p| developer_services::ProvisioningProfile {
                        id: p.id,
                        name: p.attributes.name.unwrap_or_default(),
                        platform: None,
                        status: p.attributes.profile_state,
                        profile_type: p.attributes.profile_type,
                        uuid: p.attributes.uuid,
                        expiration_date: p.attributes.expiration_date,
                        team_identifier: Some(team_id.clone()),
                        entitlements: None,
                    })
                    .collect())
            }
        }
    }
}

/// Helper to execute a closure with an authenticated DeveloperServicesClient.
///
/// This handles loading credentials, refreshing anisette data, and creating
/// the properly authenticated client.
pub fn with_client<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&AppleDsClient) -> Result<T>,
{
    let storage = CredentialStorage::new()?;
    let credentials = storage
        .load()?
        .ok_or_else(|| anyhow::anyhow!("Not logged in. Run 'lingxia auth apple login' first."))?;

    let client = match credentials {
        AuthCredentials::AppleId {
            adsid,
            app_token,
            team_id,
            ..
        } => {
            // Get fresh anisette data
            let mut anisette_provider = OmnisetteProvider::new();
            let anisette = anisette_provider
                .fetch_anisette_data()
                .context("Failed to get anisette data")?;
            let device_info = DeviceInfo::default_macos();
            AppleDsClient::Private(Box::new(PrivateDsAuth {
                adsid,
                app_token,
                team_id,
                device_info,
                anisette,
            }))
        }
        AuthCredentials::AppStoreConnect {
            key_id,
            issuer_id,
            private_key_pem,
            team_id,
            ..
        } => {
            let client =
                AppStoreConnectClient::new(&key_id, &issuer_id, &private_key_pem, &team_id)
                    .context("Failed to initialize App Store Connect API client")?;
            AppleDsClient::ApiKey { team_id, client }
        }
    };

    f(&client)
}
