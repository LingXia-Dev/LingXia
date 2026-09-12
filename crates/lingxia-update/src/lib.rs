mod app;
mod config;
mod error;
mod lxapp;
mod signing;

use lingxia_provider::{BoxFuture, ProviderError};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

pub use app::{
    AppUpdateApply, AppUpdateEvent, AppUpdateEventReceiver, AppUpdateEventSender, AppUpdateHost,
    AppUpdateProgressReporter, AppUpdateStage, app_update_scope_key, check_app_update,
    ensure_app_update_candidate_version, send_app_update_event, send_app_update_failed,
    subscribe_app_update_events,
};
pub use config::{UpdateConfig, configure_update, update_config};
pub use error::UpdateError;
pub use lxapp::{
    LxAppUpdateHost, ensure_first_install as ensure_lxapp_first_install,
    ensure_force_update_for_installed as ensure_lxapp_force_update_for_installed,
    ensure_target_version_ready as ensure_lxapp_target_version_ready, lxapp_update_scope_key,
    spawn_background_update_check as spawn_lxapp_background_update_check,
};
pub use signing::{
    SignRequest, UpdateAuthentication, UpdateVerifyTarget, archive_sha256_hex,
    check_update_enabled, compact_manifest, decode_base64url, embedded_update_public_keys,
    encode_base64url, env_requires_signature, host_requires_signature, host_update_platform,
    load_signing_seed_file, public_key_base64url, sign_package, sign_package_from_key_file,
    verify_archive_bytes, verify_checked_update,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Release,
    Preview,
    Draft,
}

impl From<Channel> for lingxia_provider::LxAppChannel {
    fn from(channel: Channel) -> Self {
        match channel {
            Channel::Release => Self::Release,
            Channel::Preview => Self::Preview,
            Channel::Draft => Self::Draft,
        }
    }
}

/// Default lxapp channel for this host, derived from the host env:
/// `dev` → `draft`, `prod` → `release`. An open can pass an explicit
/// channel to override; the client does not forbid `draft` on a prod
/// host — the registry decides per-channel access.
///
/// Host self-update does **not** carry a channel: the host talks to the
/// server for its env.
pub fn default_channel() -> Channel {
    match lingxia_app_context::env() {
        lingxia_app_context::AppEnv::Dev => Channel::Draft,
        lingxia_app_context::AppEnv::Prod => Channel::Release,
    }
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Preview => "preview",
            Self::Draft => "draft",
        }
    }

    pub fn parse(tag: &str) -> Result<Self, String> {
        match tag.trim() {
            "release" => Ok(Self::Release),
            "preview" => Ok(Self::Preview),
            "draft" => Ok(Self::Draft),
            value => Err(format!("invalid channel: {value}")),
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A semantic version representation (`major.minor.patch`) shared by update policy
/// and lxapp metadata persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn parse(version_str: &str) -> Result<Self, VersionError> {
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() != 3 {
            return Err(VersionError::InvalidFormat);
        }

        let major = parts[0]
            .parse()
            .map_err(|_| VersionError::InvalidComponent)?;
        let minor = parts.get(1).map_or(Ok(0), |s| {
            s.parse().map_err(|_| VersionError::InvalidComponent)
        })?;
        let patch = parts.get(2).map_or(Ok(0), |s| {
            s.parse().map_err(|_| VersionError::InvalidComponent)
        })?;

        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

impl FromStr for Version {
    type Err = VersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => match self.minor.cmp(&other.minor) {
                Ordering::Equal => self.patch.cmp(&other.patch),
                ordering => ordering,
            },
            ordering => ordering,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VersionError {
    #[error("invalid version format, expected 'major.minor.patch'")]
    InvalidFormat,
    #[error("invalid version component, expected unsigned integer")]
    InvalidComponent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemanticVersion {
    pub fn from_version(version: &Version) -> Self {
        Self {
            major: version.major,
            minor: version.minor,
            patch: version.patch,
        }
    }

    pub fn to_version_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LxAppUpdateQuery {
    Latest { current_version: Option<String> },
    TargetVersion(String),
}

impl LxAppUpdateQuery {
    pub fn latest(current_version: Option<impl Into<String>>) -> Self {
        Self::Latest {
            current_version: current_version.map(Into::into),
        }
    }

    pub fn target_version(version: impl Into<String>) -> Self {
        Self::TargetVersion(version.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateTarget {
    App {
        current_version: Option<String>,
    },
    LxApp {
        id: String,
        channel: Channel,
        query: LxAppUpdateQuery,
    },
    Plugin {
        id: String,
        version: String,
        channel: Channel,
    },
}

impl UpdateTarget {
    pub fn app(current_version: Option<impl Into<String>>) -> Self {
        Self::App {
            current_version: current_version.map(Into::into),
        }
    }

    pub fn lxapp(id: impl Into<String>, channel: Channel, query: LxAppUpdateQuery) -> Self {
        Self::LxApp {
            id: id.into(),
            channel,
            query,
        }
    }

    pub fn plugin(id: impl Into<String>, version: impl Into<String>, channel: Channel) -> Self {
        Self::Plugin {
            id: id.into(),
            version: version.into(),
            channel,
        }
    }

    /// Stable routing key for dedupe, metrics, and diagnostics.
    pub fn scope_key(&self) -> String {
        match self {
            Self::App { .. } => "app".to_string(),
            Self::LxApp { id, channel, .. } => format!("lxapp:{id}@{}", channel.as_str()),
            Self::Plugin {
                id,
                version,
                channel,
            } => {
                format!("plugin:{id}@{version}@{}", channel.as_str())
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct UpdatePackageInfo {
    pub version: String,
    pub url: String,
    pub checksum_sha256: String,
    pub size: Option<u64>,
    pub release_notes: Option<Vec<String>>,
    pub is_force_update: bool,
    pub required_runtime_version: Option<String>,
    pub authentication: Option<UpdateAuthentication>,
}

impl UpdatePackageInfo {
    pub fn should_replace_version(
        candidate_version: &str,
        installed_version: Option<&str>,
    ) -> bool {
        installed_version != Some(candidate_version)
    }

    pub fn should_replace_installed_version(&self, installed_version: Option<&str>) -> bool {
        Self::should_replace_version(&self.version, installed_version)
    }

    /// Whether this package should replace what is already installed.
    ///
    /// `release` / `preview` compare versions only. `draft` also treats a
    /// same-version package as an update when `checksum_sha256` differs, so a
    /// republish does not need a version bump. A draft install with no
    /// stored checksum is treated as different so the first OTA after a
    /// bundled/sideload install still picks up a same-version republish.
    pub fn should_replace(
        &self,
        channel: Channel,
        installed_version: Option<&str>,
        installed_checksum: Option<&str>,
    ) -> bool {
        if channel != Channel::Draft {
            return Self::should_replace_version(&self.version, installed_version);
        }
        if Self::should_replace_version(&self.version, installed_version) {
            return true;
        }
        let Some(server) = normalize_checksum(&self.checksum_sha256) else {
            return false;
        };
        match installed_checksum.and_then(normalize_checksum) {
            Some(local) => !local.eq_ignore_ascii_case(server),
            None => true,
        }
    }

    pub fn required_runtime_version_trimmed(&self) -> Option<&str> {
        self.required_runtime_version
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub fn ensure_runtime_compatible(
        &self,
        current_runtime_version: &str,
        target_name: &str,
    ) -> Result<(), RuntimeCompatibilityError> {
        let Some(required_runtime_version) = self.required_runtime_version_trimmed() else {
            return Ok(());
        };

        let current = Version::parse(current_runtime_version).map_err(|_| {
            RuntimeCompatibilityError::InvalidCurrentRuntimeVersion {
                runtime_version: current_runtime_version.to_string(),
            }
        })?;
        let required = Version::parse(required_runtime_version).map_err(|_| {
            RuntimeCompatibilityError::InvalidRequiredRuntimeVersion {
                target: target_name.to_string(),
                update_version: self.version.clone(),
                runtime_version: required_runtime_version.to_string(),
            }
        })?;

        if current < required {
            return Err(RuntimeCompatibilityError::RequiresRuntimeUpgrade {
                target: target_name.to_string(),
                update_version: self.version.clone(),
                required_runtime_version: required.to_string(),
                current_runtime_version: current.to_string(),
            });
        }

        Ok(())
    }
}

fn normalize_checksum(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() { None } else { Some(value) }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeCompatibilityError {
    #[error("invalid SDK runtime version '{runtime_version}'")]
    InvalidCurrentRuntimeVersion { runtime_version: String },
    #[error(
        "invalid minRuntimeVersion '{runtime_version}' from update metadata for {target}@{update_version}"
    )]
    InvalidRequiredRuntimeVersion {
        target: String,
        update_version: String,
        runtime_version: String,
    },
    #[error(
        "{target} update {update_version} requires runtime >= {required_runtime_version}, current SDK runtime is {current_runtime_version}; update host app first"
    )]
    RequiresRuntimeUpgrade {
        target: String,
        update_version: String,
        required_runtime_version: String,
        current_runtime_version: String,
    },
}

/// Update contract shared by app and lxapp update implementations.
pub trait UpdateProvider: Send + Sync + 'static {
    /// Returns `Some(package)` when an update package exists and `None` when the target
    /// is already up to date or no matching package is available.
    fn check_update<'a>(
        &'a self,
        target: UpdateTarget,
    ) -> BoxFuture<'a, Result<Option<UpdatePackageInfo>, ProviderError>>;
}

#[cfg(test)]
mod tests {
    use super::{Channel, UpdatePackageInfo, UpdateTarget, Version};

    fn package(version: &str, checksum: &str) -> UpdatePackageInfo {
        UpdatePackageInfo {
            version: version.to_string(),
            url: "https://example.test/pkg".to_string(),
            checksum_sha256: checksum.to_string(),
            size: None,
            release_notes: None,
            is_force_update: false,
            required_runtime_version: None,
            authentication: None,
        }
    }

    #[test]
    fn version_parse_accepts_full_semver_only() {
        assert!(Version::parse("1.2.3").is_ok());
        assert!(Version::parse("1").is_err());
        assert!(Version::parse("1.2").is_err());
        assert!(Version::parse("1.2.3.4").is_err());
    }

    #[test]
    fn release_and_preview_ignore_checksum_when_version_matches() {
        let pkg = package("1.0.0", "aaa");
        assert!(!pkg.should_replace(Channel::Release, Some("1.0.0"), Some("bbb")));
        assert!(!pkg.should_replace(Channel::Preview, Some("1.0.0"), Some("bbb")));
        assert!(pkg.should_replace(Channel::Release, Some("0.9.0"), Some("aaa")));
    }

    #[test]
    fn draft_replaces_same_version_when_checksum_differs() {
        let pkg = package("1.0.0", "bbb");
        assert!(pkg.should_replace(Channel::Draft, Some("1.0.0"), Some("aaa")));
        assert!(!pkg.should_replace(Channel::Draft, Some("1.0.0"), Some("BBB")));
        assert!(pkg.should_replace(Channel::Draft, Some("1.0.0"), None));
        assert!(pkg.should_replace(Channel::Draft, Some("0.9.0"), Some("bbb")));
    }

    #[test]
    fn plugin_target_carries_the_requested_channel() {
        let target = UpdateTarget::plugin("plug", "1.0.0", Channel::Draft);
        assert_eq!(target.scope_key(), "plugin:plug@1.0.0@draft");
    }
}
