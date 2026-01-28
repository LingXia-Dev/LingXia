use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const HOST_CONFIG_FILE: &str = "lingxia.config.json";
pub const LXAPP_BUILD_CONFIG_FILE: &str = "lxapp.config.json";

/// Host project configuration (native app project)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LingXiaConfig {
    pub project: ProjectConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub android: Option<AndroidConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ios: Option<IosConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harmony: Option<HarmonyConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lxapp: Option<LxAppConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub project_type: String, // "native-app" or "lxapp"
    pub platforms: Vec<String>, // ["android", "ios", "harmony"]
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
    pub deployment_target: Option<String>, // e.g., "14.0"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swift_version: Option<String>,
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
pub struct LxAppConfig {
    /// Path to LxApp project directory (relative to project root)
    pub source: String,
    /// Name to use in assets directory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_name: Option<String>,
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

impl LingXiaConfig {
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
            project: ProjectConfig {
                name: project_name.to_string(),
                project_type: "native-app".to_string(),
                platforms: vec!["android".to_string()],
            },
            android: Some(AndroidConfig {
                package_id: package_id.to_string(),
                min_sdk: Some(28),
                target_sdk: Some(35),
                compile_sdk: Some(35),
                ndk_version: None, // Auto-detect
                api_level: None,   // Derive from targetSdk
            }),
            ios: None,
            harmony: None,
            lxapp: None,
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
        assert_eq!(parsed.project.name, "my-app");
        assert_eq!(parsed.android.unwrap().package_id, "com.example.myapp");
    }
}
