#![allow(dead_code)]

use super::signer::SigningConfig;
use anyhow::{Result, anyhow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigningMode {
    Debug,
    Release,
}

impl SigningMode {
    pub fn cert_type(self) -> i32 {
        match self {
            Self::Debug => 1,
            Self::Release => 2,
        }
    }

    pub fn provision_type(self) -> i32 {
        match self {
            Self::Debug => 1,
            Self::Release => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }
}

pub struct ProvisioningManager;

impl ProvisioningManager {
    pub fn from_storage() -> Result<Self> {
        Err(unsupported())
    }

    pub fn prepare_signing_config(
        &mut self,
        _bundle_name: &str,
        _mode: SigningMode,
        _target_udids: &[String],
        _acl_permissions: &[String],
    ) -> Result<SigningConfig> {
        Err(unsupported())
    }
}

fn unsupported() -> anyhow::Error {
    anyhow!(
        "Harmony provisioning is not supported by the Windows CLI build because it requires OpenSSL tooling."
    )
}
