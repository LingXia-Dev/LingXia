use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

/// A semantic version representation (major.minor.patch)
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// Parse a version string in the format "major.minor.patch"
    pub fn parse(version_str: &str) -> Result<Self, VersionError> {
        let parts: Vec<&str> = version_str.split('.').collect();

        if parts.is_empty() || parts.len() > 3 {
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

    /// Compare this version with another version
    pub fn compare(&self, other: &Version) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => match self.minor.cmp(&other.minor) {
                Ordering::Equal => self.patch.cmp(&other.patch),
                ordering => ordering,
            },
            ordering => ordering,
        }
    }
}

/// Determine whether an update is required given an installed (optional) and a required/remote version string.
/// Fallback policy:
/// - If `required` cannot be parsed, return false (reject update on parse error).
/// - If `installed` is None or cannot be parsed, return true.
/// - Otherwise, return installed < required.
pub(crate) fn need_update(installed: Option<&str>, required: &str) -> bool {
    let required = match Version::parse(required) {
        Ok(v) => v,
        Err(_) => return false,
    };
    match installed {
        None => true,
        Some(s) => match Version::parse(s) {
            Ok(inst) => inst < required,
            Err(_) => true,
        },
    }
}

impl FromStr for Version {
    type Err = VersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Version::parse(s)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(std::cmp::Ord::cmp(self, other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
    }
}

/// Errors that can occur when parsing a version string
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    InvalidFormat,
    InvalidComponent,
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionError::InvalidFormat => {
                write!(f, "Invalid version format, expected 'major.minor.patch'")
            }
            VersionError::InvalidComponent => {
                write!(f, "Invalid version component, expected unsigned integer")
            }
        }
    }
}

impl std::error::Error for VersionError {}
