//! Local version metadata used by LingXia project templates.

/// LingXia component versions used in project templates
#[derive(Debug, Clone)]
pub struct LingXiaVersions {
    /// Rong runtime product release version
    pub rong: String,
    /// lingxia Rust crate version
    pub lingxia_crate: String,
    /// Native SDK version (Android/iOS/HarmonyOS)
    pub sdk: String,
}

pub fn current_versions() -> LingXiaVersions {
    LingXiaVersions {
        rong: env!("LINGXIA_RONG_VERSION").to_string(),
        lingxia_crate: env!("LINGXIA_RUST_CRATE_VERSION").to_string(),
        sdk: env!("LINGXIA_SDK_VERSION").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_versions_use_configured_component_versions() {
        let versions = current_versions();
        assert_eq!(versions.sdk, env!("LINGXIA_SDK_VERSION"));
        assert_eq!(versions.rong, env!("LINGXIA_RONG_VERSION"));
        assert_eq!(versions.lingxia_crate, env!("LINGXIA_RUST_CRATE_VERSION"));
    }
}
