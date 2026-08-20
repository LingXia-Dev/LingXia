//! The CLI knows one compatibility line, not a catalog of package versions.
//!
//! What is baked into the binary:
//! - the workspace/base line (`LINGXIA_RUST_CRATE_VERSION`) — crates, SDK zip names
//! - embedded `@lingxia/bridge` / `@lingxia/polyfills` (the JS the CLI ships)
//! - Rong (a different product version)
//!
//! What is not: `@lingxia/react|vue|html|types`, browser-shell-webui, or
//! terminal-settings. Those resolve to the latest published patch on this
//! line (`~M.m.0`) at `lingxia new` / first fetch. Scaffolded crate deps use
//! the same idea with the base patch as the floor (`~M.m.P`).

/// LingXia component versions used in project templates
#[derive(Debug, Clone)]
pub struct LingXiaVersions {
    /// Rong runtime product release version
    pub rong: String,
    /// lingxia Rust crate version (exact workspace line; SDK zips use this)
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

/// Tilde range that tracks this CLI's major.minor (`~0.11.0`).
/// `npm install` / `npm pack` then pick up the latest published patch.
pub fn npm_compat_range() -> String {
    minor_tilde_range(env!("LINGXIA_RUST_CRATE_VERSION"))
}

/// Cargo requirement floored at this CLI's base patch (`~0.11.2`).
/// crates.io then supplies the latest published patch. Unlike npm, the whole
/// crate workspace publishes in lockstep, so nothing can lag the base — and the
/// floor keeps the crates from resolving older than the SDK zip they pair with.
pub fn cargo_compat_req() -> String {
    format!("~{}", env!("LINGXIA_RUST_CRATE_VERSION"))
}

/// `~M.m.0` from a full semver. Used by scaffolds so an older framework
/// patch still resolves after a base-only bump.
pub fn minor_tilde_range(version: &str) -> String {
    let (major, minor) = major_minor(version);
    format!("~{major}.{minor}.0")
}

fn major_minor(version: &str) -> (&str, &str) {
    let mut parts = version.split('.');
    (
        parts.next().filter(|part| !part.is_empty()).unwrap_or("0"),
        parts.next().filter(|part| !part.is_empty()).unwrap_or("0"),
    )
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

    /// The reported Rong version drifted a full minor behind the workspace for
    /// a month while it was a hand-kept key in the CLI manifest. It is derived
    /// now; this fails if anything reintroduces a copy.
    #[test]
    fn rong_version_matches_the_workspace_dependency() {
        let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");
        let workspace = std::fs::read_to_string(manifest).expect("read the workspace manifest");
        let declared = workspace
            .lines()
            .find_map(|line| line.strip_prefix("rong = "))
            .expect("workspace declares a rong dependency");
        assert!(
            declared.contains(env!("LINGXIA_RONG_VERSION")),
            "CLI reports rong {}, workspace declares {declared}",
            env!("LINGXIA_RONG_VERSION"),
        );
    }

    #[test]
    fn minor_tilde_range_floors_to_the_compat_line() {
        assert_eq!(minor_tilde_range("0.11.2"), "~0.11.0");
        assert_eq!(minor_tilde_range("1.2.3"), "~1.2.0");
        assert_eq!(
            npm_compat_range(),
            minor_tilde_range(env!("LINGXIA_RUST_CRATE_VERSION"))
        );
    }

    #[test]
    fn cargo_compat_req_floors_at_the_base_patch() {
        assert_eq!(
            cargo_compat_req(),
            format!("~{}", env!("LINGXIA_RUST_CRATE_VERSION"))
        );
    }
}
