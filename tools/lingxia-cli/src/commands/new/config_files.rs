use super::RUST_LIB_DIR_NAME;
use super::types::{AppServiceMode, LxAppInfo, MainSurface, Platform, ProjectConfig};
use super::validation::swift_target_name_from_project_name;
use crate::config::HOST_CONFIG_FILE;
#[cfg(test)]
use crate::config::LingXiaConfig;
use anyhow::Result;
use lingxia_app_context::UpdateChannel;
use lingxia_app_context::update::default_update_channel;
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
    lxapp: Option<&LxAppInfo>,
    main: MainSurface,
    app_service: AppServiceMode,
) -> Result<()> {
    let content = render_host_config(config, lxapp, main, app_service);
    fs::write(config.target_dir.join(HOST_CONFIG_FILE), content)?;
    Ok(())
}

fn render_host_config(
    config: &ProjectConfig,
    lxapp: Option<&LxAppInfo>,
    main: MainSurface,
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
    vars.insert(
        "BROWSER_CAPABILITY".to_string(),
        (main == MainSurface::Browser).to_string(),
    );
    vars.insert(
        "TERMINAL_CAPABILITY".to_string(),
        (main == MainSurface::Terminal).to_string(),
    );
    vars.insert(
        "HOME_APP_FIELD".to_string(),
        lxapp
            .map(|lxapp| {
                format!(
                    "  homeAppId: {}   # bundled control lxapp; source is resources.bundles",
                    yaml_string(&lxapp.app_id)
                )
            })
            .unwrap_or_default(),
    );
    vars.insert(
        "MAIN_SURFACE_CONTENT".to_string(),
        render_main_surface(main, lxapp),
    );
    vars.insert(
        "RESOURCES_SECTION".to_string(),
        render_resources_section(lxapp),
    );
    vars.insert("PLATFORMS".to_string(), render_platforms(&config.platforms));
    vars.insert(
        "UPDATE_PLATFORM_CHANNELS".to_string(),
        render_update_platform_channels(&config.platforms),
    );
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

fn render_main_surface(main: MainSurface, lxapp: Option<&LxAppInfo>) -> String {
    match main {
        MainSurface::LxApp => format!(
            "lxapp: {}   # main screen: your lxapp",
            yaml_string(&lxapp.expect("lxapp main requires lxapp control").app_id)
        ),
        MainSurface::Terminal => "native: terminal   # main screen: built-in terminal".into(),
        MainSurface::Browser => "native: browser   # main screen: built-in browser".into(),
    }
}

fn render_resources_section(lxapp: Option<&LxAppInfo>) -> String {
    let Some(lxapp) = lxapp else {
        return String::new();
    };
    format!(
        "# Bundled lxapps. homeAppId and lxapp surfaces reference appId.\nresources:\n  bundles:\n    - type: lxapp\n      appId: {}\n      path: {}",
        yaml_string(&lxapp.app_id),
        yaml_string(&lxapp.dir_name)
    )
}

fn render_platforms(platforms: &[Platform]) -> String {
    platforms
        .iter()
        .map(|platform| format!("  - {}", yaml_string(platform.as_str())))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Commented `update.platforms` entries for the selected platforms. Values come
/// from the runtime compile defaults so the scaffold cannot drift from them.
fn render_update_platform_channels(platforms: &[Platform]) -> String {
    platforms
        .iter()
        .map(|platform| {
            let channel = match default_update_channel(platform.as_str()) {
                UpdateChannel::Store => "store",
                UpdateChannel::Direct => "direct",
            };
            format!("#     {}: {channel}", platform.as_str())
        })
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

        let yaml = render_host_config(
            &config,
            Some(&lxapp),
            MainSurface::LxApp,
            AppServiceMode::Enabled,
        );
        let lingxia: LingXiaConfig = serde_yaml_ng::from_str(&yaml).unwrap();
        let app = lingxia.app.as_ref().expect("app config should exist");
        assert_eq!(app.product_name, "Demo: App");
        assert_eq!(app.package_id, "com.example.demo");
        assert_eq!(
            lingxia
                .android
                .as_ref()
                .and_then(|a| a.package_id.as_deref()),
            None
        );
        assert_eq!(app.home_app_id.as_deref(), Some("lingxia.lxapp.demo"));
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

        let all_platforms = render_host_config(
            &ProjectConfig {
                name: config.name.clone(),
                product_name: config.product_name.clone(),
                project_type: config.project_type,
                platforms: vec![
                    Platform::Android,
                    Platform::Ios,
                    Platform::Macos,
                    Platform::Harmony,
                    Platform::Windows,
                ],
                package_id: config.package_id.clone(),
                app_link_hosts: config.app_link_hosts.clone(),
                target_dir: config.target_dir.clone(),
            },
            Some(&lxapp),
            MainSurface::LxApp,
            AppServiceMode::Enabled,
        );
        let all: LingXiaConfig = serde_yaml_ng::from_str(&all_platforms).unwrap();
        let app = all.app.as_ref().unwrap();
        assert_eq!(app.package_id, "com.example.demo");
        assert!(all.android.is_some() && all.ios.is_some() && all.macos.is_some());
        assert!(all.harmony.is_some() && all.windows.is_some());
        assert_eq!(
            all.android.as_ref().and_then(|c| c.package_id.as_deref()),
            None
        );
        assert_eq!(all.ios.as_ref().and_then(|c| c.bundle_id.as_deref()), None);
        assert_eq!(
            all.macos.as_ref().and_then(|c| c.bundle_id.as_deref()),
            None
        );
        assert_eq!(
            all.harmony.as_ref().and_then(|c| c.bundle_name.as_deref()),
            None
        );
        assert_eq!(all.windows.as_ref().and_then(|c| c.app_id.as_deref()), None);
        assert_eq!(
            lingxia.app_links.as_ref().unwrap().hosts,
            crate::config::AppLinkHosts::Single(vec!["demo.example.com".to_string()])
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

    /// Uncomment the template's `update:` block and plug in a key, so the
    /// scaffolded comment is checked against the real schema.
    fn activate_update_block(yaml: &str) -> String {
        let mut inside = false;
        yaml.lines()
            .map(|line| {
                inside = inside && line.starts_with('#') || line.starts_with("# update:");
                if !inside {
                    return line.to_string();
                }
                line.strip_prefix("# ")
                    .or_else(|| line.strip_prefix('#'))
                    .unwrap_or(line)
                    .replace("[ ... ]", "['6kpsY-KcUgq-9VB7Ey7F-ZVHdq6-vnuSQh7qaRRG0iw']")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn template_update_block_parses_once_uncommented() {
        let config = ProjectConfig {
            name: "demo".to_string(),
            product_name: "Demo".to_string(),
            project_type: super::super::types::ProjectType::NativeApp,
            platforms: vec![
                Platform::Android,
                Platform::Ios,
                Platform::Macos,
                Platform::Harmony,
                Platform::Windows,
            ],
            package_id: "com.example.demo".to_string(),
            app_link_hosts: Vec::new(),
            target_dir: PathBuf::from("/tmp/demo"),
        };
        let lxapp = LxAppInfo {
            app_id: "lingxia.lxapp.demo".to_string(),
            dir_name: "lxapp".to_string(),
        };

        let yaml = render_host_config(
            &config,
            Some(&lxapp),
            MainSurface::LxApp,
            AppServiceMode::Enabled,
        );
        // Scaffolds stay valid without a key: the block ships commented out.
        let scaffolded: LingXiaConfig = serde_yaml_ng::from_str(&yaml).unwrap();
        assert!(scaffolded.update.is_none());

        let activated: LingXiaConfig = serde_yaml_ng::from_str(&activate_update_block(&yaml))
            .expect("uncommented update block should parse");
        let update = activated.update.expect("update table should be present");
        update
            .validate()
            .expect("template block must satisfy validate");
        assert_eq!(update.platforms.len(), 5);
        for (platform, channel) in [
            ("ios", UpdateChannel::Store),
            ("harmony", UpdateChannel::Store),
            ("android", UpdateChannel::Direct),
            ("macos", UpdateChannel::Direct),
            ("windows", UpdateChannel::Direct),
        ] {
            assert_eq!(update.platforms.get(platform), Some(&channel), "{platform}");
        }
    }

    #[test]
    fn update_platform_channels_follow_the_selection() {
        let rendered = render_update_platform_channels(&[Platform::Macos, Platform::Ios]);
        assert_eq!(rendered, "#     macos: direct\n#     ios: store");
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

        let yaml = render_host_config(
            &config,
            Some(&lxapp),
            MainSurface::LxApp,
            AppServiceMode::Enabled,
        );
        // v2 single-declaration template, content-key form.
        assert!(yaml.contains("surfaces:"));
        assert!(yaml.contains("lxapp:"));
        assert!(yaml.contains("role: main"));
        assert!(yaml.contains("launch: true"));
        let lingxia: LingXiaConfig = serde_yaml_ng::from_str(&yaml).unwrap();
        let surfaces = lingxia
            .surfaces
            .expect("macOS config should include default surfaces");
        assert_eq!(surfaces.len(), 1);
        assert_eq!(surfaces[0].lxapp.as_deref(), Some("demo-home"));
        assert!(surfaces[0].launch);
    }

    #[test]
    fn native_terminal_main_omits_control_lxapp() {
        let config = ProjectConfig {
            name: "terminal-host".to_string(),
            product_name: "Terminal Host".to_string(),
            project_type: super::super::types::ProjectType::NativeApp,
            platforms: vec![Platform::Macos, Platform::Windows],
            package_id: "com.example.terminal".to_string(),
            app_link_hosts: Vec::new(),
            target_dir: PathBuf::from("/tmp/terminal-host"),
        };

        let yaml = render_host_config(
            &config,
            None,
            MainSurface::Terminal,
            AppServiceMode::Disabled,
        );
        let lingxia: LingXiaConfig = serde_yaml_ng::from_str(&yaml).unwrap();
        let app = lingxia.app.as_ref().unwrap();
        assert_eq!(app.home_app_id, None);
        assert!(!lingxia.app_service_enabled());
        assert!(lingxia.resources.is_none());
        let capabilities = lingxia.capabilities.as_ref().unwrap();
        assert!(capabilities.terminal);
        assert!(!capabilities.browser);
        let surfaces = lingxia.surfaces.as_ref().unwrap();
        assert_eq!(surfaces.len(), 1);
        assert_eq!(surfaces[0].native.as_deref(), Some("terminal"));
        assert!(surfaces[0].launch);
    }

    #[test]
    fn native_browser_main_can_keep_embedded_control_lxapp() {
        let config = ProjectConfig {
            name: "browser-host".to_string(),
            product_name: "Browser Host".to_string(),
            project_type: super::super::types::ProjectType::NativeApp,
            platforms: vec![Platform::Windows],
            package_id: "com.example.browser".to_string(),
            app_link_hosts: Vec::new(),
            target_dir: PathBuf::from("/tmp/browser-host"),
        };
        let lxapp = LxAppInfo {
            app_id: "lingxia.lxapp.browser-control".to_string(),
            dir_name: "lxapp".to_string(),
        };

        let yaml = render_host_config(
            &config,
            Some(&lxapp),
            MainSurface::Browser,
            AppServiceMode::Enabled,
        );
        let lingxia: LingXiaConfig = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(
            lingxia.app.as_ref().unwrap().home_app_id.as_deref(),
            Some("lingxia.lxapp.browser-control")
        );
        assert!(lingxia.capabilities.as_ref().unwrap().browser);
        assert_eq!(
            lingxia.surfaces.as_ref().unwrap()[0].native.as_deref(),
            Some("browser")
        );
        assert_eq!(lingxia.resources.as_ref().unwrap().bundles.len(), 1);
    }
}
