use super::types::{LxAppInfo, Platform, ProjectConfig};
use super::validation::swift_target_name_from_project_name;
use crate::config::{
    AndroidConfig, AppLinksConfig, DEFAULT_CACHE_MAX_AGE_DAYS, HarmonyConfig, HostAppConfig,
    IosConfig, LingXiaConfig, MacosConfig, ResourceBundleConfig, ResourceBundleDetail,
    ResourceBundleType, ResourcesConfig, StorageConfig,
};
use anyhow::Result;
use serde_json::Value;

const DEFAULT_MACOS_UI_TEMPLATE: &str =
    include_str!("../../../templates/host-ui/macos-default.json");

pub(super) fn generate_config_file(config: &ProjectConfig, lxapp: &LxAppInfo) -> Result<()> {
    let lingxia_config = build_lingxia_config(config, lxapp);

    // Save config file
    lingxia_config.save_with_comments(&config.target_dir)?;

    Ok(())
}

fn build_lingxia_config(config: &ProjectConfig, lxapp: &LxAppInfo) -> LingXiaConfig {
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
            lingxia_id: None,
            platforms: platforms.clone(),
            home_lxapp_id: Some(lxapp.app_id.clone()),
            cache_max_age_days: None,
            cache_max_size_mb: None,
        }),
        android,
        ios,
        macos,
        harmony,
        ui: default_ui_config(config, lxapp),
        app_links: (!config.app_link_hosts.is_empty()).then(|| AppLinksConfig {
            hosts: config.app_link_hosts.clone(),
        }),
        storage: Some(StorageConfig {
            temp_max_size_mb: Some(1024),
            cache_max_age_days: Some(DEFAULT_CACHE_MAX_AGE_DAYS),
            cache_max_size_mb: Some(2048),
            data_max_size_mb: Some(4096),
            app_storage_max_size_mb: Some(16384),
        }),
        resources: Some(ResourcesConfig {
            i18n: None,
            icons: None,
            bundles: Some(vec![ResourceBundleConfig::Detailed(ResourceBundleDetail {
                bundle_type: ResourceBundleType::Lxapp,
                path: lxapp.app_id.clone(),
                target: None,
            })]),
        }),
    }
}

fn default_ui_config(config: &ProjectConfig, lxapp: &LxAppInfo) -> Option<serde_json::Value> {
    if !config.platforms.contains(&Platform::Macos) {
        return None;
    }

    let mut ui: Value = serde_json::from_str(DEFAULT_MACOS_UI_TEMPLATE)
        .expect("built-in macOS App UI template must be valid JSON");
    ui["surfaces"][0]["content"]["appId"] = Value::String(lxapp.app_id.clone());
    Some(ui)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn build_lingxia_config_sets_bundle_and_storage_defaults() {
        let config = ProjectConfig {
            name: "demo".to_string(),
            product_name: "Demo".to_string(),
            project_type: super::super::types::ProjectType::NativeApp,
            platforms: vec![Platform::Android],
            package_id: "com.example.demo".to_string(),
            app_link_hosts: vec!["demo.example.com".to_string()],
            target_dir: PathBuf::from("/tmp/demo"),
        };
        let lxapp = LxAppInfo {
            app_id: "demo".to_string(),
        };

        let lingxia = build_lingxia_config(&config, &lxapp);
        let app = lingxia.app.expect("app config should exist");
        let resources = lingxia.resources.expect("resources config should exist");

        assert_eq!(app.cache_max_age_days, None);
        assert_eq!(app.cache_max_size_mb, None);
        let storage = lingxia
            .storage
            .as_ref()
            .expect("storage config should exist");
        assert_eq!(storage.temp_max_size_mb, Some(1024));
        assert_eq!(storage.cache_max_age_days, Some(7));
        assert_eq!(storage.cache_max_size_mb, Some(2048));
        assert_eq!(storage.data_max_size_mb, Some(4096));
        assert_eq!(storage.app_storage_max_size_mb, Some(16384));
        assert_eq!(
            lingxia.app_links.as_ref().unwrap().hosts,
            vec!["demo.example.com"]
        );
        assert!(matches!(
            resources.bundles.as_deref(),
            Some([ResourceBundleConfig::Detailed(detail)])
                if detail.bundle_type == ResourceBundleType::Lxapp
                    && detail.path == "demo"
                    && detail.target.is_none()
        ));
    }

    #[test]
    fn build_lingxia_config_adds_default_ui_for_macos() {
        let config = ProjectConfig {
            name: "demo".to_string(),
            product_name: "Demo".to_string(),
            project_type: super::super::types::ProjectType::NativeApp,
            platforms: vec![Platform::Macos],
            package_id: "com.example.demo".to_string(),
            app_link_hosts: vec!["demo.example.com".to_string()],
            target_dir: PathBuf::from("/tmp/demo"),
        };
        let lxapp = LxAppInfo {
            app_id: "demo-home".to_string(),
        };

        let lingxia = build_lingxia_config(&config, &lxapp);
        let ui = lingxia.ui.expect("macOS config should include default ui");

        assert_eq!(ui["launch"]["initialSurface"], "main");
        assert_eq!(ui["surfaces"][0]["id"], "main");
        assert_eq!(ui["surfaces"][0]["content"]["appId"], "demo-home");
        assert!(ui["surfaces"][0]["presentation"].get("size").is_none());
        assert_eq!(ui["activators"].as_array().unwrap().len(), 0);
    }
}
