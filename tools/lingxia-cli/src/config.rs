use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const HOST_CONFIG_FILE: &str = "lingxia.config.json";
pub const SECRETS_CONFIG_FILE: &str = ".lingxia.secrets.json";
pub const LXAPP_BUILD_CONFIG_FILE: &str = "lxapp.config.ts";

/// Host project configuration (native app project)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LingXiaConfig {
    /// Host app settings used to generate `app.json` at build time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<HostAppConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub android: Option<AndroidConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ios: Option<IosConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub macos: Option<MacosConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harmony: Option<HarmonyConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesConfig>,
}

/// Host app settings (checked into git via `lingxia.config.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostAppConfig {
    /// Project name (technical identifier, used for Rust lib naming, e.g., "myapp" -> "myapp-lib")
    pub project_name: String,

    /// Product name (user-facing display name)
    pub product_name: String,
    pub product_version: String,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_server: Option<String>,

    /// Platforms to build for this app (e.g. ["android"]).
    pub platforms: Vec<String>,

    // Keep explicit spelling for "ID" (not "Id") to match runtime `app.json` schema.
    #[serde(rename = "homeLxAppID")]
    pub home_lxapp_id: String,
    // Keep explicit spelling for "App" (not "app") to match runtime `app.json` schema.
    #[serde(rename = "homeLxAppVersion")]
    pub home_lxapp_version: String,

    /// LingXia SDK version (e.g. "0.1.1")
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk_version: Option<String>,

    /// Maximum age in days for cache files before cleanup (default: 7)
    /// Set to 0 to disable automatic cache cleanup
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_max_age_days: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidConfig {
    pub package_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_sdk: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_sdk: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_sdk: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ndk_version: Option<String>,
    /// API level for NDK toolchain (e.g., 33 for android33-clang)
    /// If not specified, will be derived from targetSdk
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_level: Option<u32>,
}

impl AndroidConfig {
    /// Get the API level to use for NDK toolchain
    pub fn get_api_level(&self) -> u32 {
        // 1. Explicit API level takes priority
        if let Some(api) = self.api_level {
            return api;
        }

        // 2. Derive from targetSdk
        if let Some(target) = self.target_sdk {
            return target;
        }

        // 3. Default to 33
        33
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IosConfig {
    pub bundle_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_target: Option<String>, // e.g., "17.0"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swift_version: Option<String>,
    /// SwiftPM target name for resources lookup.
    /// If omitted, CLI will try app.projectName or infer from Sources/ when unambiguous.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacosConfig {
    /// Bundle identifier (e.g., "app.lingxia.example")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,

    /// Deployment target (e.g., "14.0")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_target: Option<String>,

    /// Executable product name (SwiftPM). If omitted, CLI will try a few
    /// reasonable defaults and fall back to "the only executable in bin dir".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_name: Option<String>,

    /// SwiftPM target name for resources lookup.
    /// If omitted, CLI will try app.projectName or infer from Sources/ when unambiguous.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarmonyConfig {
    pub bundle_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_sdk_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatible_sdk_version: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesConfig {
    /// Path to i18n resources (relative to project root)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub i18n: Option<String>,
    /// Path to icon resources (relative to project root)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<String>,
    /// Path to web runtime distribution (relative to project root)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
}

// =============================================================================
// Secrets Configuration (.lingxia.secrets.json)
// =============================================================================

// These types are intentionally kept even if some binaries don't currently use them.
// They are part of the CLI's configuration surface area.
#[allow(dead_code)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretsConfig {
    /// LingXia API key (for cloud services)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// LingXia API secret (for cloud services)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_secret: Option<String>,

    /// iOS-specific secrets (signing, team ID, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ios: Option<IosSecrets>,

    /// Android-specific secrets (keystore, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub android: Option<AndroidSecrets>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IosSecrets {
    /// Apple Developer Team ID (e.g., "AG98W7429S")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,

    /// Code signing identity (e.g., "Apple Development: xxx")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_identity: Option<String>,

    /// Path to provisioning profile (.mobileprovision)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioning_profile: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidSecrets {
    /// Path to keystore file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keystore_path: Option<String>,

    /// Keystore password
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keystore_password: Option<String>,

    /// Key alias
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_alias: Option<String>,

    /// Key password
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_password: Option<String>,
}

#[allow(dead_code)]
impl SecretsConfig {
    /// Load secrets from .lingxia.secrets.json in the given directory.
    ///
    /// Returns None if the file doesn't exist (secrets are optional).
    /// Returns an error if the file exists but is malformed.
    pub fn load(project_root: &Path) -> Result<Option<Self>> {
        let secrets_path = project_root.join(SECRETS_CONFIG_FILE);
        if !secrets_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&secrets_path)
            .with_context(|| format!("Failed to read {}", SECRETS_CONFIG_FILE))?;

        let config: SecretsConfig = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", SECRETS_CONFIG_FILE))?;

        Ok(Some(config))
    }

    /// Get API key from secrets or environment variable.
    ///
    /// Priority: LINGXIA_API_KEY env var > secrets file
    pub fn get_api_key(&self) -> Option<String> {
        if let Ok(key) = std::env::var("LINGXIA_API_KEY")
            && !key.is_empty()
        {
            return Some(key);
        }
        self.api_key.clone()
    }

    /// Get API secret from secrets or environment variable.
    ///
    /// Priority: LINGXIA_API_SECRET env var > secrets file
    pub fn get_api_secret(&self) -> Option<String> {
        if let Ok(secret) = std::env::var("LINGXIA_API_SECRET")
            && !secret.is_empty()
        {
            return Some(secret);
        }
        self.api_secret.clone()
    }

    /// Get iOS team ID from secrets or environment variable.
    ///
    /// Priority: LINGXIA_TEAM_ID env var > secrets file
    pub fn get_ios_team_id(&self) -> Option<String> {
        if let Ok(team_id) = std::env::var("LINGXIA_TEAM_ID")
            && !team_id.is_empty()
        {
            return Some(team_id);
        }
        self.ios.as_ref().and_then(|ios| ios.team_id.clone())
    }

    /// Get iOS signing identity from secrets or environment variable.
    ///
    /// Priority: LINGXIA_SIGNING_IDENTITY env var > secrets file
    pub fn get_ios_signing_identity(&self) -> Option<String> {
        if let Ok(identity) = std::env::var("LINGXIA_SIGNING_IDENTITY")
            && !identity.is_empty()
        {
            return Some(identity);
        }
        self.ios
            .as_ref()
            .and_then(|ios| ios.signing_identity.clone())
    }
}

impl LingXiaConfig {
    /// Get the project name from config
    pub fn get_project_name(&self) -> Option<&str> {
        self.app.as_ref().map(|app| app.project_name.as_str())
    }

    /// Get the Rust library directory name (e.g., "myproject-lib")
    pub fn get_rust_lib_name(&self) -> Option<String> {
        self.get_project_name().map(|name| format!("{}-lib", name))
    }

    /// Load config from lingxia.config.json in the given directory
    pub fn load(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join(HOST_CONFIG_FILE);

        if !config_path.exists() {
            anyhow::bail!(
                "{} not found in {}. Run 'lingxia new' to create a new project.",
                HOST_CONFIG_FILE,
                project_root.display()
            );
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {}", HOST_CONFIG_FILE))?;

        let config: LingXiaConfig = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", HOST_CONFIG_FILE))?;

        Ok(config)
    }

    /// Save config to lingxia.config.json in the given directory
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let config_path = project_root.join(HOST_CONFIG_FILE);

        let content = serde_json::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&config_path, content)
            .with_context(|| format!("Failed to write {}", HOST_CONFIG_FILE))?;

        Ok(())
    }

    /// Create a default Android config
    #[allow(dead_code)] // Used in tests
    pub fn new_android(project_name: &str, package_id: &str) -> Self {
        Self {
            app: Some(HostAppConfig {
                project_name: project_name.to_string(),
                product_name: project_name.to_string(),
                product_version: "0.0.1".to_string(),
                api_server: None,
                platforms: vec!["android".to_string()],
                home_lxapp_id: "homelxapp".to_string(),
                home_lxapp_version: "1.0.0".to_string(),
                sdk_version: None,
                cache_max_age_days: None,
            }),
            android: Some(AndroidConfig {
                package_id: package_id.to_string(),
                min_sdk: Some(28),
                target_sdk: Some(35),
                compile_sdk: Some(35),
                ndk_version: None, // Auto-detect
                api_level: None,   // Derive from targetSdk
            }),
            ios: None,
            macos: None,
            harmony: None,
            resources: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_android_api_level_derivation() {
        let config = AndroidConfig {
            package_id: "com.example.app".to_string(),
            min_sdk: Some(28),
            target_sdk: Some(35),
            compile_sdk: Some(35),
            ndk_version: None,
            api_level: None,
        };
        assert_eq!(config.get_api_level(), 35);

        let config_explicit = AndroidConfig {
            package_id: "com.example.app".to_string(),
            min_sdk: Some(28),
            target_sdk: Some(35),
            compile_sdk: Some(35),
            ndk_version: None,
            api_level: Some(33),
        };
        assert_eq!(config_explicit.get_api_level(), 33);
    }

    #[test]
    fn test_config_serialization() {
        let config = LingXiaConfig::new_android("my-app", "com.example.myapp");
        let json = serde_json::to_string_pretty(&config).unwrap();
        println!("{}", json);

        let parsed: LingXiaConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.app.unwrap().product_name, "my-app");
        assert_eq!(parsed.android.unwrap().package_id, "com.example.myapp");
    }
}
