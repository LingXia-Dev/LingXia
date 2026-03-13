use super::types::{LxAppInfo, Platform, ProjectConfig};
use super::validation::swift_target_name_from_project_name;
use crate::config::{
    AndroidConfig, DEFAULT_CACHE_MAX_AGE_DAYS, DEFAULT_CACHE_MAX_SIZE_MB, HarmonyConfig,
    HostAppConfig, IosConfig, LingXiaConfig, MacosConfig, ResourcesConfig,
};
use anyhow::Result;

pub(super) fn generate_config_file(
    config: &ProjectConfig,
    lxapp: &LxAppInfo,
    core_version: &str,
) -> Result<()> {
    let lingxia_config = build_lingxia_config(config, lxapp, core_version);

    // Save config file
    lingxia_config.save(&config.target_dir)?;

    Ok(())
}

fn build_lingxia_config(
    config: &ProjectConfig,
    lxapp: &LxAppInfo,
    core_version: &str,
) -> LingXiaConfig {
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

    LingXiaConfig {
        app: Some(HostAppConfig {
            project_name: config.name.clone(),
            product_name: config.product_name.clone(),
            product_version: "0.0.1".to_string(),
            api_server: None,
            client_id: None,
            platforms: platforms.clone(),
            home_lxapp_id: lxapp.app_id.clone(),
            cache_max_age_days: Some(DEFAULT_CACHE_MAX_AGE_DAYS),
            cache_max_size_mb: Some(DEFAULT_CACHE_MAX_SIZE_MB),
        }),
        android,
        ios,
        macos,
        harmony,
        resources: Some(ResourcesConfig {
            i18n: None,
            icons: None,
            runtime: Some(format!("npm:@lingxia/core@{core_version}")),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DEFAULT_CACHE_MAX_AGE_DAYS, DEFAULT_CACHE_MAX_SIZE_MB};
    use std::path::PathBuf;

    #[test]
    fn build_lingxia_config_sets_runtime_and_cache_defaults() {
        let config = ProjectConfig {
            name: "demo".to_string(),
            product_name: "Demo".to_string(),
            project_type: super::super::types::ProjectType::NativeApp,
            platforms: vec![Platform::Android],
            package_id: "com.example.demo".to_string(),
            target_dir: PathBuf::from("/tmp/demo"),
        };
        let lxapp = LxAppInfo {
            app_id: "homelxapp".to_string(),
        };

        let lingxia = build_lingxia_config(&config, &lxapp, "0.2.0");
        let app = lingxia.app.expect("app config should exist");
        let resources = lingxia.resources.expect("resources config should exist");

        assert_eq!(app.cache_max_age_days, Some(DEFAULT_CACHE_MAX_AGE_DAYS));
        assert_eq!(app.cache_max_size_mb, Some(DEFAULT_CACHE_MAX_SIZE_MB));
        assert_eq!(
            resources.runtime.as_deref(),
            Some("npm:@lingxia/core@0.2.0")
        );
    }
}
