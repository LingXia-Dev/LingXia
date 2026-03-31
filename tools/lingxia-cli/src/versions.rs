//! Local version metadata used by LingXia project templates.

/// LingXia component versions used in project templates
#[derive(Debug, Clone)]
pub struct LingXiaVersions {
    /// @lingxia/rong NPM package version
    pub rong: String,
    /// lingxia Rust crate version
    pub lingxia_crate: String,
    /// Native SDK version (Android/iOS/HarmonyOS)
    pub sdk: String,
}

pub fn current_versions() -> LingXiaVersions {
    let version = env!("CARGO_PKG_VERSION").to_string();
    LingXiaVersions {
        rong: version.clone(),
        lingxia_crate: version.clone(),
        sdk: version,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_versions_are_aligned_with_cli_version() {
        let versions = current_versions();
        let cli_version = env!("CARGO_PKG_VERSION");
        assert_eq!(versions.sdk, cli_version);
        assert_eq!(versions.rong, cli_version);
        assert_eq!(versions.lingxia_crate, cli_version);
    }
}
