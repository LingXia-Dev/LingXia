//! Map `app.productVersion` onto the integers / marketing strings each OS
//! package format requires.
//!
//! One field in `lingxia.yaml` is the source of truth. Platform project files
//! may still contain scaffold placeholders; `lingxia build` overwrites them.

use anyhow::{Result, anyhow};
use semver::Version;

/// User-facing marketing version plus the monotonic integer stores need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsPackageVersion {
    /// `major.minor.patch` (prerelease / build metadata stripped).
    pub marketing: String,
    /// `major * 1_000_000 + minor * 1_000 + patch`.
    ///
    /// Matches the Harmony template's `1.0.0` → `1000000` convention and stays
    /// inside Android's 2_100_000_000 `versionCode` ceiling for majors < 2100.
    pub build: u32,
}

/// Derive OS package versions from a validated `app.productVersion`.
pub fn os_package_version(product_version: &str) -> Result<OsPackageVersion> {
    let parsed = Version::parse(product_version.trim()).map_err(|_| {
        anyhow!("app.productVersion must be a semantic version (major.minor.patch)")
    })?;
    let build = parsed
        .major
        .checked_mul(1_000_000)
        .and_then(|value| {
            parsed
                .minor
                .checked_mul(1_000)
                .and_then(|minor| value.checked_add(minor))
        })
        .and_then(|value| value.checked_add(parsed.patch))
        .ok_or_else(|| {
            anyhow!("app.productVersion is too large to encode as a platform versionCode")
        })?;
    if build > 2_100_000_000 {
        return Err(anyhow!(
            "app.productVersion encodes versionCode {build}, which exceeds Android's 2100000000 limit"
        ));
    }
    Ok(OsPackageVersion {
        marketing: format!("{}.{}.{}", parsed.major, parsed.minor, parsed.patch),
        build: u32::try_from(build).expect("build checked against 2_100_000_000"),
    })
}

#[cfg(test)]
mod tests {
    use super::os_package_version;

    #[test]
    fn encodes_the_harmony_1_0_0_convention() {
        let version = os_package_version("1.0.0").unwrap();
        assert_eq!(version.marketing, "1.0.0");
        assert_eq!(version.build, 1_000_000);
    }

    #[test]
    fn encodes_a_pre_1_0_release() {
        let version = os_package_version("0.2.7").unwrap();
        assert_eq!(version.marketing, "0.2.7");
        assert_eq!(version.build, 2_007);
    }

    #[test]
    fn strips_prerelease_from_the_os_package_version() {
        let version = os_package_version("1.2.3-beta.1+exp.sha").unwrap();
        assert_eq!(version.marketing, "1.2.3");
        assert_eq!(version.build, 1_002_003);
    }

    #[test]
    fn rejects_a_major_that_would_overflow_android_version_code() {
        let err = os_package_version("2100.0.1").unwrap_err().to_string();
        assert!(err.contains("2100000000"), "{err}");
    }
}
