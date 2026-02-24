use super::types::{LxAppInfo, Platform, ProjectConfig};
use super::validation::swift_target_name_from_project_name;
use crate::config::{
    AndroidConfig, HarmonyConfig, HostAppConfig, IosConfig, LingXiaConfig, MacosConfig,
    ResourcesConfig,
};
use anyhow::Result;

pub(super) fn generate_config_file(
    config: &ProjectConfig,
    lxapp: &LxAppInfo,
    web_runtime_version: &str,
) -> Result<()> {
    let swift_target_name = swift_target_name_from_project_name(&config.name);

    let platforms = config
        .platforms
        .iter()
        .map(|p| p.as_str().to_string())
        .collect::<Vec<_>>();

    let android = if config.platforms.contains(&Platform::Android) {
        Some(AndroidConfig {
            package_id: config.package_id.clone(),
            min_sdk: Some(29),
            target_sdk: Some(35),
            compile_sdk: Some(35),
            ndk_version: None,
            api_level: None,
        })
    } else {
        None
    };

    let ios = if config.platforms.contains(&Platform::Ios) {
        Some(IosConfig {
            bundle_id: config.package_id.clone(),
            deployment_target: None,
            swift_version: None,
            target_name: Some(swift_target_name.clone()),
        })
    } else {
        None
    };

    let macos = if config.platforms.contains(&Platform::Macos) {
        Some(MacosConfig {
            bundle_id: Some(config.package_id.clone()),
            deployment_target: None,
            executable_name: Some(swift_target_name.clone()),
            target_name: Some(swift_target_name.clone()),
        })
    } else {
        None
    };

    let harmony = if config.platforms.contains(&Platform::Harmony) {
        Some(HarmonyConfig {
            bundle_name: config.package_id.clone(),
            compatible_sdk_version: None,
            target_sdk_version: None,
        })
    } else {
        None
    };

    let lingxia_config = LingXiaConfig {
        app: Some(HostAppConfig {
            project_name: config.name.clone(),
            product_name: config.product_name.clone(),
            product_version: "0.0.1".to_string(),
            api_server: None,
            platforms: platforms.clone(),
            home_lxapp_id: lxapp.app_id.clone(),
            cache_max_age_days: None,
            cache_max_size_mb: None,
        }),
        android,
        ios,
        macos,
        harmony,
        resources: Some(ResourcesConfig {
            i18n: None,
            icons: None,
            runtime: Some(format!("npm:@lingxia/web-runtime@{web_runtime_version}")),
        }),
    };

    // Save config file
    lingxia_config.save(&config.target_dir)?;

    Ok(())
}
