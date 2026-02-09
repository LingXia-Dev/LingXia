use super::{AgcApiCredentials, AgcConnectClient, AgcCredentialStorage, AgcToken};
use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarmonyTokenState {
    Valid,
    Expired,
    NotCached,
}

impl HarmonyTokenState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Expired => "expired",
            Self::NotCached => "not cached",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HarmonyAuthStatus {
    pub client_id: String,
    pub token_state: HarmonyTokenState,
    pub storage_path: PathBuf,
}

pub struct HarmonyAuthService {
    storage: AgcCredentialStorage,
    client: AgcConnectClient,
}

impl HarmonyAuthService {
    pub fn new() -> Result<Self> {
        Ok(Self {
            storage: AgcCredentialStorage::new()?,
            client: AgcConnectClient::new(),
        })
    }

    pub fn storage_path(&self) -> &Path {
        self.storage.path()
    }

    pub fn load_credentials(&self) -> Result<Option<AgcApiCredentials>> {
        self.storage.load()
    }

    pub fn authenticate(&self, client_id: &str, client_secret: &str) -> Result<AgcApiCredentials> {
        let token: AgcToken = self.client.get_token(client_id, client_secret)?;
        Ok(AgcApiCredentials {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            token: Some(token),
        })
    }

    pub fn save_credentials(&self, credentials: &AgcApiCredentials) -> Result<()> {
        self.storage.save(credentials)
    }

    pub fn clear_credentials(&self) -> Result<bool> {
        if self.storage.load()?.is_some() {
            self.storage.clear()?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn status(&self) -> Result<Option<HarmonyAuthStatus>> {
        let Some(credentials) = self.storage.load()? else {
            return Ok(None);
        };

        let token_state = credentials
            .token
            .as_ref()
            .map(|token| {
                if AgcConnectClient::is_token_expired(token) {
                    HarmonyTokenState::Expired
                } else {
                    HarmonyTokenState::Valid
                }
            })
            .unwrap_or(HarmonyTokenState::NotCached);

        Ok(Some(HarmonyAuthStatus {
            client_id: credentials.client_id,
            token_state,
            storage_path: self.storage.path().to_path_buf(),
        }))
    }
}
