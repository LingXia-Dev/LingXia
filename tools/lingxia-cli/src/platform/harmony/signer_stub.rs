#![allow(dead_code)]

use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SigningConfig {
    pub keystore_path: PathBuf,
    pub keystore_password: String,
    pub key_password: Option<String>,
    pub cert_path: PathBuf,
    pub profile_path: PathBuf,
    pub sign_algorithm: SignAlgorithm,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum SignAlgorithm {
    #[default]
    SHA256withECDSA,
}

impl std::fmt::Display for SignAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SHA256withECDSA => write!(f, "SHA256withECDSA"),
        }
    }
}

pub struct HarmonySigner;

impl HarmonySigner {
    pub fn new_native() -> Self {
        Self
    }

    pub fn sign_hap(
        &self,
        _config: &SigningConfig,
        _input_path: &Path,
        _output_path: &Path,
    ) -> Result<()> {
        Err(unsupported())
    }

    pub fn verify_hap(&self, _hap_path: &Path) -> Result<String> {
        Err(unsupported())
    }
}

fn unsupported() -> anyhow::Error {
    anyhow!(
        "Harmony native HAP signing is not supported by the Windows CLI build because it requires OpenSSL tooling."
    )
}
