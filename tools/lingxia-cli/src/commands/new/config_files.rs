use super::RUST_LIB_DIR_NAME;
use super::types::{AppServiceMode, LxAppInfo, Platform, ProjectConfig};
use super::validation::swift_target_name_from_project_name;
use crate::config::HOST_CONFIG_FILE;
#[cfg(test)]
use crate::config::LingXiaConfig;
use anyhow::Result;
use std::{collections::HashMap, fs};

const HOST_CONFIG_TEMPLATE: &str = include_str!("../../../templates/host-config/lingxia.yaml");
const ANDROID_SECTION_TEMPLATE: &str =
    include_str!("../../../templates/host-config/sections/android.yaml");
const IOS_SECTION_TEMPLATE: &str = include_str!("../../../templates/host-config/sections/ios.yaml");
const MACOS_SECTION_TEMPLATE: &str =
    include_str!("../../../templates/host-config/sections/macos.yaml");
const UI_SECTION_TEMPLATE: &str = include_str!("../../../templates/host-config/sections/ui.yaml");
const HARMONY_SECTION_TEMPLATE: &str =
    include_str!("../../../templates/host-config/sections/harmony.yaml");
const WINDOWS_SECTION_TEMPLATE: &str =
    include_str!("../../../templates/host-config/sections/windows.yaml");
const APP_LINKS_SECTION_TEMPLATE: &str =
    include_str!("../../../templates/host-config/sections/app-links.yaml");

pub(super) fn generate_config_file(
    config: &ProjectConfig,
    lxapp: &LxAppInfo,
    app_service: AppServiceMode,
) -> Result<()> {
    let content = render_host_config(config, lxapp, app_service);
    fs::write(config.target_dir.join(HOST_CONFIG_FILE), content)?;
    Ok(())
}

fn render_host_config(
    config: &ProjectConfig,
    lxapp: &LxAppInfo,
    app_service: AppServiceMode,
) -> String {
    let swift_target_name = swift_target_name_from_project_name(&config.name);
    let mut vars = HashMap::new();
    vars.insert("PROJECT_NAME".to_string(), yaml_string(&config.name));
    vars.insert("RUST_LIB_DIR".to_string(), yaml_string(RUST_LIB_DIR_NAME));
    vars.insert(
        "PRODUCT_NAME".to_string(),
        yaml_string(&config.product_name),
    );
    vars.insert("PACKAGE_ID".to_string(), yaml_string(&config.package_id));
    vars.insert("HOME_APP_ID".to_string(), yaml_string(&lxapp.app_id));
    vars.insert("HOME_APP_PATH".to_string(), yaml_string(&lxapp.dir_name));
    vars.insert(
        "LINGXIA_ID".to_string(),
        yaml_string(&super::types::default_lingxia_id(&config.name)),
    );
    vars.insert(
        "SWIFT_TARGET_NAME".to_string(),
        yaml_string(&swift_target_name),
    );
    vars.insert("SWIFT_TARGET_LABEL".to_string(), swift_target_name);
    vars.insert("APP_SERVICE".to_string(), app_service.enabled().to_string());
    vars.insert("PLATFORMS".to_string(), render_platforms(&config.platforms));
    vars.insert("APP_LINK_HOSTS".to_string(), render_app_link_hosts(config));
    vars.insert("SHELL_SECTION".to_string(), String::new());
    vars.insert(
        "UI_SECTION".to_string(),
        super::template::substitute_variables(UI_SECTION_TEMPLATE, &vars),
    );
    vars.insert(
        "ANDROID_SECTION".to_string(),
        render_optional_section(
            config.platforms.contains(&Platform::Android),
            ANDROID_SECTION_TEMPLATE,
            &vars,
        ),
    );
    vars.insert(
        "IOS_SECTION".to_string(),
        render_optional_section(
            config.platforms.contains(&Platform::Ios),
            IOS_SECTION_TEMPLATE,
            &vars,
        ),
    );
    vars.insert(
        "MACOS_SECTION".to_string(),
        render_optional_section(
            config.platforms.contains(&Platform::Macos),
            MACOS_SECTION_TEMPLATE,
            &vars,
        ),
    );
    vars.insert(
        "HARMONY_SECTION".to_string(),
        render_optional_section(
            config.platforms.contains(&Platform::Harmony),
            HARMONY_SECTION_TEMPLATE,
            &vars,
        ),
    );
    vars.insert(
        "WINDOWS_SECTION".to_string(),
        render_optional_section(
            config.platforms.contains(&Platform::Windows),
            WINDOWS_SECTION_TEMPLATE,
            &vars,
        ),
    );
    vars.insert(
        "APP_LINKS_SECTION".to_string(),
        render_optional_section(
            !config.app_link_hosts.is_empty(),
            APP_LINKS_SECTION_TEMPLATE,
            &vars,
        ),
    );
    super::template::substitute_variables(HOST_CONFIG_TEMPLATE, &vars)
}

fn render_platforms(platforms: &[Platform]) -> String {
    platforms
        .iter()
        .map(|platform| format!("  - {}", yaml_string(platform.as_str())))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_app_link_hosts(config: &ProjectConfig) -> String {
    config
        .app_link_hosts
        .iter()
        .map(|host| format!("  - {}", yaml_string(host)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn yaml_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn render_optional_section(
    enabled: bool,
    template: &str,
    vars: &HashMap<String, String>,
) -> String {
    if enabled {
        super::template::substitute_variables(template, vars)
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn render_host_config_sets_home_app_and_storage_defaults() {
        let config = ProjectConfig {
            name: "demo".to_string(),
            product_name: "Demo: App".to_string(),
            project_type: super::super::types::ProjectType::NativeApp,
            platforms: vec![Platform::Android],
            package_id: "com.example.demo".to_string(),
            app_link_hosts: vec!["demo.example.com".to_string()],
            target_dir: PathBuf::from("/tmp/demo"),
        };
        let lxapp = LxAppInfo {
            app_id: "lingxia.lxapp.demo".to_string(),
            dir_name: "lxapp".to_string(),
        };

        let yaml = render_host_config(&config, &lxapp, AppServiceMode::Enabled);
        let lingxia: LingXiaConfig = serde_yaml_ng::from_str(&yaml).unwrap();
        let app = lingxia.app.as_ref().expect("app config should exist");
        assert_eq!(app.product_name, "Demo: App");
        assert_eq!(app.home_app_id, "lingxia.lxapp.demo");
        // lingxiaId defaults to the namespaced host publish id.
        assert_eq!(app.lingxia_id.as_deref(), Some("lingxia.app.demo"));
        let storage = lingxia
            .storage
            .as_ref()
            .expect("storage config should exist");
        assert_eq!(storage.temp_max_size_mb, Some(1024));
        assert_eq!(storage.cache_max_size_mb, Some(2048));
        assert_eq!(storage.data_max_size_mb, Some(4096));
        assert_eq!(storage.app_storage_max_size_mb, Some(16384));
        assert_eq!(
            lingxia.app_links.as_ref().unwrap().hosts,
            vec!["demo.example.com"]
        );
        assert!(lingxia.app_service_enabled());
        assert_eq!(
            lingxia.capabilities.as_ref().map(|c| c.notifications),
            Some(false)
        );
        let resources = lingxia
            .resources
            .as_ref()
            .expect("resources config should exist");
        assert_eq!(resources.bundles.len(), 1);
        // Bundle appId is the namespaced id; its path stays the on-disk dir name.
        assert_eq!(resources.bundles[0].app_id, "lingxia.lxapp.demo");
        assert_eq!(resources.bundles[0].path.as_deref(), Some("lxapp"));
    }

    #[test]
    fn build_lingxia_config_adds_default_surfaces_for_macos() {
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
            dir_name: "lxapp".to_string(),
        };

        let yaml = render_host_config(&config, &lxapp, AppServiceMode::Enabled);
        // v2 single-declaration template.
        assert!(yaml.contains("surfaces:"));
        assert!(yaml.contains("render: lxapp"));
        assert!(yaml.contains("role: main"));
        assert!(yaml.contains("launch: true"));
        let lingxia: LingXiaConfig = serde_yaml_ng::from_str(&yaml).unwrap();
        let surfaces = lingxia
            .surfaces
            .expect("macOS config should include default surfaces");
        assert_eq!(surfaces.len(), 1);
        assert_eq!(surfaces[0].id, "demo-home");
        assert!(surfaces[0].launch);
    }
}
