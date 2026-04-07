use lingxia_provider::{BoxFuture, ProviderError};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseType {
    #[default]
    Release,
    Preview,
    Developer,
}

impl ReleaseType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Preview => "preview",
            Self::Developer => "developer",
        }
    }
}

impl fmt::Display for ReleaseType {
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
        release_type: ReleaseType,
        query: LxAppUpdateQuery,
    },
    Plugin {
        id: String,
        version: String,
    },
}

impl UpdateTarget {
    pub fn app(current_version: Option<impl Into<String>>) -> Self {
        Self::App {
            current_version: current_version.map(Into::into),
        }
    }

    pub fn lxapp(
        id: impl Into<String>,
        release_type: ReleaseType,
        query: LxAppUpdateQuery,
    ) -> Self {
        Self::LxApp {
            id: id.into(),
            release_type,
            query,
        }
    }

    pub fn plugin(id: impl Into<String>, version: impl Into<String>) -> Self {
        Self::Plugin {
            id: id.into(),
            version: version.into(),
        }
    }

    /// Stable routing key for cooldowns, metrics, and diagnostics.
    pub fn scope_key(&self) -> String {
        match self {
            Self::App { .. } => "app".to_string(),
            Self::LxApp {
                id, release_type, ..
            } => format!("lxapp:{id}@{}", release_type.as_str()),
            Self::Plugin { id, version } => format!("plugin:{id}@{version}"),
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
    use super::Version;

    #[test]
    fn version_parse_accepts_full_semver_only() {
        assert!(Version::parse("1.2.3").is_ok());
        assert!(Version::parse("1").is_err());
        assert!(Version::parse("1.2").is_err());
        assert!(Version::parse("1.2.3.4").is_err());
    }
}
