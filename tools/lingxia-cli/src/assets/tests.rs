use super::{
    any_path_bundle_targets_es5, build_app_json_from_config, build_ui_json_from_config,
    build_windows_ui_json_from_config, collect_view_target_warnings, prepare_app_ui_icons,
    validate_app_ui_svg_icon,
};
use crate::config::{
    AppEnv, AppLinkHosts, AppLinksConfig, AppStoreConfig, HostAppConfig, IosConfig, LingXiaConfig,
    LingxiaServer, MacosConfig, PerEnvHosts, PerEnvServer, ResolvedEnv, SettingsDestination,
    ThemeConfig, UpdateSigningConfig,
};
use lingxia_app_context::{ThemeColor, ThemeStyle};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn test_resolved_env() -> ResolvedEnv {
    ResolvedEnv {
        version: AppEnv::Prod,
        lingxia_server: "https://api.example.com".to_string(),
        package_id_suffix: None,
        app_link_hosts: Vec::new(),
    }
}

#[test]
fn lingxia_id_is_not_suffixed_by_env() {
    let config = LingXiaConfig {
        app: Some(HostAppConfig {
            project_name: "demo".into(),
            rust_lib_dir: None,
            package_id: "com.example.demo".into(),
            product_name: "Demo".into(),
            product_names: Default::default(),
            product_version: "1.2.3".into(),
            lingxia_server: None,
            lingxia_id: Some("app.lingxia.demo".into()),
            platforms: vec!["macos".into()],
            home_app_id: Some("demo-home".into()),
        }),
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: None,
        theme: None,
        settings_destination: None,
        browser: None,
        generated_ui: None,
        surfaces: None,
        app_links: None,
        storage: None,
        resources: None,
        splash: None,
        assets: None,
        update: None,
    };
    // An active package-id suffix must not leak into lingxiaId.
    let dev_env = ResolvedEnv {
        version: AppEnv::Dev,
        lingxia_server: String::new(),
        package_id_suffix: Some(".dev".to_string()),
        app_link_hosts: Vec::new(),
    };
    let app_json = build_app_json_from_config(&config, None, None, &dev_env).unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();
    assert_eq!(
        value.get("lingxiaId").and_then(|v| v.as_str()),
        Some("app.lingxia.demo")
    );
    assert!(value.get("windowsAppId").is_none());
}

#[test]
fn windows_app_id_is_only_emitted_for_windows_hosts() {
    let android = LingXiaConfig::new_android("demo", "com.example.demo", "home");
    let json = build_app_json_from_config(&android, None, None, &test_resolved_env()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(value.get("windowsAppId").is_none());

    let mut windows = android;
    windows.app.as_mut().unwrap().platforms = vec!["windows".into()];
    let json = build_app_json_from_config(&windows, None, None, &test_resolved_env()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        value.get("windowsAppId").and_then(|v| v.as_str()),
        Some("com.example.demo")
    );
}

#[test]
fn showcase_yaml_declares_update_public_key() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/lingxia-showcase");
    let config = LingXiaConfig::load(&repo).unwrap();
    assert_eq!(
        config.update.unwrap().trusted_public_keys,
        vec!["6kpsY-KcUgq-9VB7Ey7F-ZVHdq6-vnuSQh7qaRRG0iw".to_string()]
    );
}

#[test]
fn omitted_update_table_does_not_embed_keys() {
    let config = LingXiaConfig::new_android("demo", "com.example.demo", "home");
    assert!(config.update.is_none());
    let app_json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();
    assert!(value.get("updateTrustedPublicKeys").is_none());
}

#[test]
fn update_table_allows_at_most_two_keys() {
    // A direct host without keys is refused by `LingXiaConfig::validate`.
    let mut config = LingXiaConfig::new_android("demo", "com.example.demo", "home");
    config.update = Some(UpdateSigningConfig {
        trusted_public_keys: vec!["a".into(), "b".into(), "c".into()],
        ..Default::default()
    });
    let err = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap_err();
    assert!(err.to_string().contains("at most two keys"), "{err}");
}

#[test]
fn browser_bookmarks_survives_yaml_to_runtime_config() {
    let mut config = LingXiaConfig::new_android("demo", "com.example.demo", "home");
    for bookmarks in [true, false] {
        config.browser = Some(crate::config::BrowserConfig {
            bookmarks,
            webui: None,
        });
        let json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
        let runtime = lingxia_app_context::AppConfig::parse_and_validate(&json).unwrap();
        assert_eq!(runtime.browser.bookmarks, bookmarks);
    }
    config.browser = None;
    let json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    assert!(
        lingxia_app_context::AppConfig::parse_and_validate(&json)
            .unwrap()
            .browser
            .bookmarks
    );
}

#[test]
fn generated_app_json_embeds_update_trusted_public_keys() {
    let mut config = LingXiaConfig::new_android("demo", "com.example.demo", "home");
    config.update = Some(UpdateSigningConfig {
        trusted_public_keys: vec!["6kpsY-KcUgq-9VB7Ey7F-ZVHdq6-vnuSQh7qaRRG0iw".into()],
        ..Default::default()
    });
    let app_json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();
    assert_eq!(
        value["updateTrustedPublicKeys"][0],
        "6kpsY-KcUgq-9VB7Ey7F-ZVHdq6-vnuSQh7qaRRG0iw"
    );
    let parsed = lingxia_app_context::AppConfig::parse_and_validate(&app_json)
        .expect("runtime app.json parse");
    assert_eq!(
        parsed.update_trusted_public_keys,
        vec!["6kpsY-KcUgq-9VB7Ey7F-ZVHdq6-vnuSQh7qaRRG0iw".to_string()]
    );
    assert!(parsed.update_channel.is_none());
    assert!(parsed.update_channels.is_empty());
}

#[test]
fn generated_app_json_embeds_update_channels() {
    use lingxia_app_context::UpdateChannel;
    use std::collections::BTreeMap;

    let mut config = LingXiaConfig::new_android("demo", "com.example.demo", "home");
    let mut platforms = BTreeMap::new();
    platforms.insert("android".into(), UpdateChannel::Direct);
    platforms.insert("ios".into(), UpdateChannel::Store);
    config.update = Some(UpdateSigningConfig {
        trusted_public_keys: vec!["6kpsY-KcUgq-9VB7Ey7F-ZVHdq6-vnuSQh7qaRRG0iw".into()],
        channel: Some(UpdateChannel::Direct),
        platforms,
    });
    let app_json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();
    assert_eq!(value["updateChannel"], "direct");
    assert_eq!(value["updateChannels"]["android"], "direct");
    assert_eq!(value["updateChannels"]["ios"], "store");
    let parsed = lingxia_app_context::AppConfig::parse_and_validate(&app_json)
        .expect("runtime app.json parse");
    assert_eq!(parsed.update_channel, Some(UpdateChannel::Direct));
    assert_eq!(
        parsed.update_channels.get("ios").copied(),
        Some(UpdateChannel::Store)
    );
}

#[test]
fn generated_app_json_embeds_store_listing_ids() {
    let mut config = LingXiaConfig::new_android("demo", "com.example.demo", "home");
    config.ios = Some(IosConfig {
        bundle_id: None,
        team_id: None,
        deployment_target: None,
        swift_version: None,
        target_name: None,
        store: Some(AppStoreConfig {
            app_id: Some("1234567890".into()),
        }),
    });
    config.macos = Some(MacosConfig {
        bundle_id: None,
        team_id: None,
        deployment_target: None,
        executable_name: None,
        target_name: None,
        store: Some(AppStoreConfig {
            app_id: Some("987654321".into()),
        }),
    });
    let app_json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();
    assert_eq!(value["storeListingIds"]["ios"], "1234567890");
    assert_eq!(value["storeListingIds"]["macos"], "987654321");
    let parsed = lingxia_app_context::AppConfig::parse_and_validate(&app_json)
        .expect("runtime app.json parse");
    assert_eq!(
        parsed.store_listing_ids.get("ios").map(String::as_str),
        Some("1234567890")
    );
}

#[test]
fn a_store_only_update_table_needs_no_keys() {
    use lingxia_app_context::UpdateChannel;

    let mut config = LingXiaConfig::new_android("demo", "com.example.demo", "home");
    if let Some(app) = config.app.as_mut() {
        app.platforms = vec!["ios".into()];
    }
    config.update = Some(UpdateSigningConfig {
        trusted_public_keys: vec![],
        channel: Some(UpdateChannel::Store),
        ..Default::default()
    });
    // A store version signal carries no package to verify.
    let app_json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();
    assert!(value.get("updateTrustedPublicKeys").is_none());
    assert_eq!(value["updateChannel"], "store");
}

#[test]
fn store_channel_without_listing_id_is_reported() {
    use lingxia_app_context::UpdateChannel;
    use std::collections::{BTreeMap, HashMap};

    let mut platforms = BTreeMap::new();
    platforms.insert("macos".into(), UpdateChannel::Store);
    platforms.insert("android".into(), UpdateChannel::Store);
    let update = UpdateSigningConfig {
        trusted_public_keys: vec!["6kpsY-KcUgq-9VB7Ey7F-ZVHdq6-vnuSQh7qaRRG0iw".into()],
        channel: None,
        platforms,
    };
    let app_platforms = vec![
        "ios".to_string(),
        "macos".to_string(),
        "windows".to_string(),
        "android".to_string(),
    ];
    let mut listing_ids = HashMap::new();
    listing_ids.insert("ios".to_string(), "1234567890".to_string());

    // ios has an id, windows defaults to direct, android needs no id.
    assert_eq!(
        super::json::store_platforms_missing_listing_id(&app_platforms, &update, &listing_ids),
        vec!["macos".to_string()]
    );
}

#[test]
fn generated_app_json_excludes_ui_fields() {
    let config = LingXiaConfig {
        app: Some(HostAppConfig {
            project_name: "demo".into(),
            rust_lib_dir: None,
            package_id: "com.example.demo".into(),
            product_name: "Demo".into(),
            product_names: Default::default(),
            product_version: "1.2.3".into(),
            lingxia_server: Some(LingxiaServer::Single("http://127.0.0.1:8080".into())),
            lingxia_id: Some("demo".into()),
            platforms: vec!["macos".into()],
            home_app_id: Some("demo-home".into()),
        }),
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: None,
        theme: None,
        settings_destination: None,
        browser: None,
        generated_ui: Some(serde_json::json!({
            "launch": { "initialSurface": "main" },
            "surfaces": [],
            "activators": []
        })),
        surfaces: None,
        app_links: None,
        storage: None,
        resources: None,
        splash: None,
        assets: None,
        update: None,
    };

    let app_json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();

    assert!(value.get("ui").is_none());
    assert!(value.get("panels").is_none());
}

#[test]
fn generated_app_json_omits_home_identity_for_native_host() {
    let mut config = LingXiaConfig::new_android("demo", "com.example.demo", "demo-home");
    let app = config.app.as_mut().unwrap();
    app.platforms = vec!["windows".into()];
    app.home_app_id = None;

    let app_json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();

    assert!(value.get("homeAppId").is_none());
    assert!(value.get("homeAppVersion").is_none());
}

#[test]
fn generated_app_json_emits_settings_destination_only_when_configured() {
    let mut config = LingXiaConfig::new_android("demo", "com.example.demo", "demo-home");
    let without = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let without: serde_json::Value = serde_json::from_str(&without).unwrap();
    assert!(without.get("settingsDestination").is_none());

    config.settings_destination = Some(SettingsDestination::BrowserControlPage {
        route: "/settings/privacy".to_string(),
        query: Some(std::collections::BTreeMap::from([
            ("source".to_string(), serde_json::json!("sidebar")),
            ("highlight".to_string(), serde_json::json!(true)),
        ])),
    });
    let with = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let with: serde_json::Value = serde_json::from_str(&with).unwrap();
    assert_eq!(
        with["settingsDestination"],
        serde_json::json!({
            "kind": "browserControlPage",
            "route": "/settings/privacy",
            "query": { "highlight": true, "source": "sidebar" }
        })
    );
}

#[test]
fn showcase_generated_app_json_uses_the_static_browser_settings_page() {
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lingxia-showcase");
    let config = LingXiaConfig::load(&project).unwrap();
    let app_json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let generated: serde_json::Value = serde_json::from_str(&app_json).unwrap();

    assert_eq!(
        generated["settingsDestination"],
        serde_json::json!({
            "kind": "browserControlPage",
            "route": "/settings"
        })
    );
}

#[test]
fn generated_app_json_includes_dev_ws_url_when_configured() {
    let config = LingXiaConfig {
        app: Some(HostAppConfig {
            project_name: "demo".into(),
            rust_lib_dir: None,
            package_id: "com.example.demo".into(),
            product_name: "Demo".into(),
            product_names: Default::default(),
            product_version: "1.2.3".into(),
            lingxia_server: None,
            lingxia_id: None,
            platforms: vec!["android".into()],
            home_app_id: Some("demo-home".into()),
        }),
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: None,
        theme: None,
        settings_destination: None,
        browser: None,
        generated_ui: None,
        surfaces: None,
        app_links: None,
        storage: None,
        resources: None,
        splash: None,
        assets: None,
        update: None,
    };

    let app_json = build_app_json_from_config(
        &config,
        None,
        Some("ws://192.168.1.20:12345/?token=abc"),
        &test_resolved_env(),
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();

    assert_eq!(value["devWsUrl"], "ws://192.168.1.20:12345/?token=abc");
    assert_eq!(
        value["devBundleBaseUrl"],
        "http://192.168.1.20:12345/__lingxia/dev?token=abc"
    );
}

#[test]
fn generated_app_json_includes_app_link_hosts() {
    let config = LingXiaConfig {
        app: Some(HostAppConfig {
            project_name: "demo".into(),
            rust_lib_dir: None,
            package_id: "com.example.demo".into(),
            product_name: "Demo".into(),
            product_names: Default::default(),
            product_version: "1.2.3".into(),
            lingxia_server: None,
            lingxia_id: None,
            platforms: vec!["android".into()],
            home_app_id: Some("demo-home".into()),
        }),
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: None,
        theme: None,
        settings_destination: None,
        browser: None,
        generated_ui: None,
        surfaces: None,
        app_links: Some(AppLinksConfig {
            hosts: AppLinkHosts::Single(vec!["www.example.com".into()]),
        }),
        storage: None,
        resources: None,
        splash: None,
        assets: None,
        update: None,
    };

    let mut env = test_resolved_env();
    env.app_link_hosts = vec!["www.example.com".into()];
    let app_json = build_app_json_from_config(&config, None, None, &env).unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();

    assert_eq!(value["appLinks"]["hosts"][0], "www.example.com");
    assert_eq!(value["appLinks"]["hostsByEnv"]["dev"][0], "www.example.com");
    assert_eq!(
        value["appLinks"]["hostsByEnv"]["prod"][0],
        "www.example.com"
    );
}

#[test]
fn generated_app_json_selects_per_env_app_link_hosts() {
    let config = LingXiaConfig {
        app: Some(HostAppConfig {
            project_name: "demo".into(),
            rust_lib_dir: None,
            package_id: "com.example.demo".into(),
            product_name: "Demo".into(),
            product_names: Default::default(),
            product_version: "1.2.3".into(),
            lingxia_server: None,
            lingxia_id: None,
            platforms: vec!["android".into()],
            home_app_id: Some("demo-home".into()),
        }),
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: None,
        theme: None,
        settings_destination: None,
        browser: None,
        generated_ui: None,
        surfaces: None,
        app_links: Some(AppLinksConfig {
            hosts: AppLinkHosts::PerEnv(PerEnvHosts {
                dev: Some(vec!["app-dev.example.com".into()]),
                prod: Some(vec!["app.example.com".into()]),
            }),
        }),
        storage: None,
        resources: None,
        splash: None,
        assets: None,
        update: None,
    };

    let dev = config.resolve_env(AppEnv::Dev).unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&build_app_json_from_config(&config, None, None, &dev).unwrap())
            .unwrap();
    assert_eq!(value["appLinks"]["hosts"][0], "app-dev.example.com");
    assert_eq!(value["appLinks"]["hosts"].as_array().unwrap().len(), 1);
    assert_eq!(
        value["appLinks"]["hostsByEnv"]["dev"][0],
        "app-dev.example.com"
    );
    assert_eq!(
        value["appLinks"]["hostsByEnv"]["prod"][0],
        "app.example.com"
    );

    let prod = config.resolve_env(AppEnv::Prod).unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&build_app_json_from_config(&config, None, None, &prod).unwrap())
            .unwrap();
    assert_eq!(value["appLinks"]["hosts"][0], "app.example.com");
    assert_eq!(
        value["appLinks"]["hostsByEnv"]["dev"][0],
        "app-dev.example.com"
    );
    assert_eq!(
        value["appLinks"]["hostsByEnv"]["prod"][0],
        "app.example.com"
    );
}

#[test]
fn generated_app_json_keeps_both_lingxia_servers() {
    let mut config = LingXiaConfig {
        app: Some(HostAppConfig {
            project_name: "demo".into(),
            rust_lib_dir: None,
            package_id: "com.example.demo".into(),
            product_name: "Demo".into(),
            product_names: Default::default(),
            product_version: "1.2.3".into(),
            lingxia_server: Some(LingxiaServer::PerEnv(PerEnvServer {
                dev: Some("https://dev.example".into()),
                prod: Some("https://prod.example".into()),
            })),
            lingxia_id: None,
            platforms: vec!["macos".into()],
            home_app_id: Some("demo-home".into()),
        }),
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: None,
        theme: None,
        settings_destination: None,
        browser: None,
        generated_ui: None,
        surfaces: None,
        app_links: None,
        storage: None,
        resources: None,
        splash: None,
        assets: None,
        update: None,
    };

    let prod = config.resolve_env(AppEnv::Prod).unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&build_app_json_from_config(&config, None, None, &prod).unwrap())
            .unwrap();
    assert_eq!(value["env"], "prod");
    assert_eq!(value["lingxiaServer"], "https://prod.example");
    assert_eq!(value["lingxiaServers"]["dev"], "https://dev.example");
    assert_eq!(value["lingxiaServers"]["prod"], "https://prod.example");

    config.app.as_mut().unwrap().lingxia_server =
        Some(LingxiaServer::Single("https://shared.example".into()));
    let dev = config.resolve_env(AppEnv::Dev).unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&build_app_json_from_config(&config, None, None, &dev).unwrap())
            .unwrap();
    assert_eq!(value["lingxiaServers"]["dev"], "https://shared.example");
    assert_eq!(value["lingxiaServers"]["prod"], "https://shared.example");
}

#[test]
fn generated_app_json_includes_capabilities() {
    let config = LingXiaConfig {
        app: Some(HostAppConfig {
            project_name: "demo".into(),
            rust_lib_dir: None,
            package_id: "com.example.demo".into(),
            product_name: "Demo".into(),
            product_names: Default::default(),
            product_version: "1.2.3".into(),
            lingxia_server: None,
            lingxia_id: None,
            platforms: vec!["android".into()],
            home_app_id: Some("demo-home".into()),
        }),
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: Some(crate::config::CapabilitiesConfig {
            notifications: true,
            browser: false,
            terminal: true,
            proxy: false,
            process: true,
            autostart: false,
            app_use: false,
            computer_use: false,
            browser_use: false,
            media_capture: Default::default(),
        }),
        theme: None,
        settings_destination: None,
        browser: None,
        generated_ui: None,
        surfaces: None,
        app_links: None,
        storage: None,
        resources: None,
        splash: None,
        assets: None,
        update: None,
    };

    let app_json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();

    assert_eq!(value["capabilities"]["notifications"], true);
    assert_eq!(value["capabilities"]["terminal"], true);
    assert_eq!(value["capabilities"]["process"], true);
}

#[test]
fn generated_app_json_includes_normalized_theme() {
    let mut config = LingXiaConfig::new_android("demo", "com.example.demo", "demo-home");
    config.theme = Some(ThemeConfig {
        light: Some(ThemeStyle {
            accent_color: Some(ThemeColor::parse("#a1b2c3").unwrap()),
            ..ThemeStyle::default()
        }),
        dark: Some(ThemeStyle {
            separator_color: Some(ThemeColor::parse("#343840").unwrap()),
            ..ThemeStyle::default()
        }),
        default_appearance: Some(lingxia_app_context::AppearancePreference::Dark),
    });

    let app_json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();

    assert_eq!(value["theme"]["light"]["accentColor"], "#A1B2C3");
    assert_eq!(value["theme"]["dark"]["separatorColor"], "#343840");
    assert_eq!(value["theme"]["defaultAppearance"], "dark");
}

#[test]
fn generated_ui_json_preserves_generated_ui_config() {
    let ui = serde_json::json!({
        "launch": { "initialSurface": "main" },
        "surfaces": [{
            "id": "main",
            "role": "main",
            "content": { "kind": "lxapp", "appId": "demo-home" }
        }],
        "activators": []
    });
    let config = LingXiaConfig {
        app: None,
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: None,
        theme: None,
        settings_destination: None,
        browser: None,
        generated_ui: Some(ui.clone()),
        surfaces: None,
        app_links: None,
        storage: None,
        resources: None,
        splash: None,
        assets: None,
        update: None,
    };

    let temp = TempDir::new().unwrap();
    let icons = prepare_app_ui_icons(temp.path(), &config).unwrap();
    let ui_json = build_ui_json_from_config(&config, &icons, "macos")
        .unwrap()
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&ui_json).unwrap();
    println!("{}", serde_json::to_string_pretty(&value).unwrap());
    assert_eq!(value, ui);
}

#[test]
fn generated_ui_json_rewrites_app_ui_icons() {
    let ui = serde_json::json!({
        "launch": { "initialSurface": "main" },
        "surfaces": [{
            "id": "main",
            "role": "main",
            "content": { "kind": "lxapp", "appId": "demo-home" }
        }],
        "activators": [{
            "id": "browser",
            "kind": "sidebarItem",
            "icon": "icons/browser.svg",
            "action": { "kind": "toggleSurface", "surface": "main" }
        }]
    });
    let config = LingXiaConfig {
        app: None,
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: None,
        theme: None,
        settings_destination: None,
        browser: None,
        generated_ui: Some(ui),
        surfaces: None,
        app_links: None,
        storage: None,
        resources: None,
        splash: None,
        assets: None,
        update: None,
    };
    let icons = vec![super::PreparedAppUiIcon {
        relative_path: "icons/browser-deadbeef.pdf".to_string(),
        windows_relative_path: "icons/browser-deadbeef.png".to_string(),
        source_path: "icons/browser.svg".to_string(),
        bytes: Vec::new(),
        windows_bytes: Vec::new(),
        hash: "deadbeef".to_string(),
        windows_hash: "deadbeef".to_string(),
    }];

    let ui_json = build_ui_json_from_config(&config, &icons, "macos")
        .unwrap()
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&ui_json).unwrap();
    assert_eq!(value["activators"][0]["icon"], "icons/browser-deadbeef.pdf");
}

#[test]
fn generated_windows_ui_json_rewrites_app_ui_icons_to_png() {
    let ui = serde_json::json!({
        "launch": { "initialSurface": "main" },
        "surfaces": [{
            "id": "main",
            "role": "main",
            "content": { "kind": "lxapp", "appId": "demo-home" }
        }],
        "activators": [{
            "id": "browser",
            "kind": "sidebarItem",
            "icon": "icons/browser.svg",
            "action": { "kind": "toggleSurface", "surface": "main" }
        }]
    });
    let config = LingXiaConfig {
        app: None,
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: None,
        theme: None,
        settings_destination: None,
        browser: None,
        generated_ui: Some(ui),
        surfaces: None,
        app_links: None,
        storage: None,
        resources: None,
        splash: None,
        assets: None,
        update: None,
    };
    let icons = vec![super::PreparedAppUiIcon {
        relative_path: "icons/browser-deadbeef.pdf".to_string(),
        windows_relative_path: "icons/browser-cafebabe.png".to_string(),
        source_path: "icons/browser.svg".to_string(),
        bytes: Vec::new(),
        windows_bytes: Vec::new(),
        hash: "deadbeef".to_string(),
        windows_hash: "cafebabe".to_string(),
    }];

    let ui_json = build_windows_ui_json_from_config(&config, &icons)
        .unwrap()
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&ui_json).unwrap();
    assert_eq!(value["activators"][0]["icon"], "icons/browser-cafebabe.png");
}

#[test]
fn surfaces_end_to_end_maps_terminal_when_explicitly_declared() {
    // Mirrors the migrated showcase: main lxapp + aside lxapp (right) + native
    // terminal (bottom). Loading must map `surfaces:` into the internal ui;
    // capabilities.terminal only enables the runtime, it does not inject UI.
    let temp = TempDir::new().unwrap();
    fs::write(
        temp.path().join("lingxia.yaml"),
        r#"
app:
  projectName: demo
  packageId: app.demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [macos]
  homeAppId: home
macos: {}
capabilities:
  terminal: true
surfaces:
  - lxapp: home
    role: main
    launch: true
    tray:
      icon: icons/tray.svg
      label: Demo
      action: activate
  - lxapp: chat
    role: aside
    edge: right
  - native: terminal
    role: aside
    edge: bottom
"#,
    )
    .unwrap();
    fs::create_dir_all(temp.path().join("icons")).unwrap();
    fs::write(
        temp.path().join("icons/chat.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64"><rect x="8" y="8" width="48" height="48" rx="8" fill="#000"/></svg>"##,
    )
    .unwrap();
    fs::write(
        temp.path().join("icons/tray.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64"><circle cx="32" cy="32" r="22" fill="#000"/></svg>"##,
    )
    .unwrap();

    let config = LingXiaConfig::load(temp.path()).unwrap();
    let icons = prepare_app_ui_icons(temp.path(), &config).unwrap();
    let ui_json = build_ui_json_from_config(&config, &icons, "macos")
        .unwrap()
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&ui_json).unwrap();

    assert_eq!(value["launch"]["initialSurface"], "home");

    let surfaces = value["surfaces"].as_array().unwrap();
    assert_eq!(surfaces.len(), 3);
    let terminal_count = surfaces
        .iter()
        .filter(|surface| {
            surface["content"]["kind"] == "native" && surface["content"]["name"] == "terminal"
        })
        .count();
    assert_eq!(terminal_count, 1);

    // main -> role main / lxapp(home)
    assert_eq!(surfaces[0]["id"], "home");
    assert_eq!(surfaces[0]["role"], "main");
    assert_eq!(surfaces[0]["content"]["appId"], "home");
    // aside right -> role aside / edge right
    assert_eq!(surfaces[1]["id"], "chat");
    assert_eq!(surfaces[1]["role"], "aside");
    assert_eq!(surfaces[1]["attachTo"], "home");
    assert_eq!(surfaces[1]["edge"], "right");
    // native terminal -> terminal surface, bottom, with size
    assert_eq!(surfaces[2]["id"], "terminal");
    assert_eq!(surfaces[2]["edge"], "bottom");
    assert_eq!(surfaces[2]["size"]["height"], 320);
    // Only the tray entry: persistent sidebar entries come from the runtime
    // activator API, never from YAML.
    let activators = value["activators"].as_array().unwrap();
    assert_eq!(activators.len(), 1);
    assert_eq!(activators[0]["id"], "homeTray");
    assert_eq!(activators[0]["kind"], "menuBarItem");
    assert_eq!(activators[0]["label"], "Demo");
    assert_eq!(activators[0]["action"]["kind"], "openSurface");
    assert_eq!(activators[0]["action"]["surface"], "home");
}

#[test]
fn generated_ui_json_rejects_terminal_when_capability_disabled() {
    let config = LingXiaConfig {
        app: None,
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: Some(crate::config::CapabilitiesConfig {
            notifications: false,
            browser: false,
            terminal: false,
            proxy: false,
            process: false,
            autostart: false,
            app_use: false,
            computer_use: false,
            browser_use: false,
            media_capture: Default::default(),
        }),
        theme: None,
        settings_destination: None,
        browser: None,
        generated_ui: Some(serde_json::json!({
            "launch": { "initialSurface": "main" },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "content": { "kind": "lxapp", "appId": "demo-home" }
            }, {
                "id": "terminal",
                "role": "aside",
                "attachTo": "main",
                "edge": "bottom",
                "content": { "kind": "native", "name": "terminal" }
            }],
            "activators": []
        })),
        surfaces: None,
        app_links: None,
        storage: None,
        resources: None,
        splash: None,
        assets: None,
        update: None,
    };

    let err = build_ui_json_from_config(&config, &[], "macos")
        .unwrap_err()
        .to_string();
    assert!(err.contains("capabilities.terminal is not enabled"));
}

#[test]
fn generated_ui_json_prunes_surfaces_for_target_platform() {
    let config = LingXiaConfig {
        app: None,
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: None,
        theme: None,
        settings_destination: None,
        browser: None,
        generated_ui: Some(serde_json::json!({
            "launch": { "initialSurface": "main" },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "platforms": ["macos"],
                "content": { "kind": "lxapp", "appId": "main-home" }
            }, {
                "id": "windowsSide",
                "role": "aside",
                "attachTo": "main",
                "edge": "right",
                "platforms": ["windows"],
                "content": { "kind": "lxapp", "appId": "win-side" }
            }],
            "activators": [{
                "id": "windowsSideButton",
                "kind": "sidebarItem",
                "hostSurface": "main",
                "action": { "kind": "toggleSurface", "surface": "windowsSide" }
            }]
        })),
        surfaces: None,
        app_links: None,
        storage: None,
        resources: None,
        splash: None,
        assets: None,
        update: None,
    };

    let temp = TempDir::new().unwrap();
    let icons = prepare_app_ui_icons(temp.path(), &config).unwrap();
    let ui_json = build_ui_json_from_config(&config, &icons, "macos")
        .unwrap()
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&ui_json).unwrap();

    let surfaces = value["surfaces"].as_array().unwrap();
    assert_eq!(surfaces.len(), 1);
    assert_eq!(surfaces[0]["id"], "main");
    assert!(surfaces[0].get("platforms").is_none());
    assert_eq!(value["activators"].as_array().unwrap().len(), 0);
}

#[test]
fn app_ui_svg_icon_validation_rejects_non_square() {
    let err = validate_app_ui_svg_icon(
        "wide.svg",
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32" viewBox="0 0 64 32"><rect width="64" height="32"/></svg>"#,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("must be square"));
}

#[test]
fn app_ui_icon_preparation_requires_svg() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("icons")).unwrap();
    fs::write(temp.path().join("icons/browser.png"), b"not really png").unwrap();
    let config = LingXiaConfig {
        app: None,
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: None,
        theme: None,
        settings_destination: None,
        browser: None,
        generated_ui: Some(serde_json::json!({
            "launch": { "initialSurface": "main" },
            "surfaces": [],
            "activators": [{
                "id": "browser",
                "kind": "sidebarItem",
                "icon": "icons/browser.png",
                "action": { "kind": "toggleSurface", "surface": "main" }
            }]
        })),
        surfaces: None,
        app_links: None,
        storage: None,
        resources: None,
        splash: None,
        assets: None,
        update: None,
    };

    let err = prepare_app_ui_icons(temp.path(), &config)
        .unwrap_err()
        .to_string();
    assert!(err.contains("only SVG source icons"));
}

mod view_target_warnings {
    use super::*;
    use crate::config::{AndroidConfig, ResourceBundleConfig, ResourceBundleType, ResourcesConfig};

    fn android_config_with(min_sdk: Option<u32>) -> AndroidConfig {
        AndroidConfig {
            package_id: Some("com.example.demo".to_string()),
            min_sdk,
            target_sdk: Some(35),
            compile_sdk: Some(35),
            ndk_version: None,
            api_level: None,
            google_play_store: None,
            xiaomi_store: None,
            oppo_store: None,
            honor_store: None,
        }
    }

    fn host_config(min_sdk: Option<u32>, bundle_path: &str, bundle_app_id: &str) -> LingXiaConfig {
        let mut config = LingXiaConfig::new_android("demo", "com.example.demo", bundle_app_id);
        config.android = Some(android_config_with(min_sdk));
        config.resources = Some(ResourcesConfig {
            bundles: vec![ResourceBundleConfig {
                bundle_type: ResourceBundleType::Lxapp,
                app_id: bundle_app_id.to_string(),
                path: Some(bundle_path.to_string()),
                package: None,
                version: None,
            }],
        });
        config
    }

    fn write_lxapp_config(root: &Path, bundle_path: &str, contents: &str) {
        let dir = root.join(bundle_path);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("lxapp.config.ts"), contents).unwrap();
    }

    #[test]
    fn warns_when_min_sdk_low_and_target_es2015() {
        let temp = TempDir::new().unwrap();
        write_lxapp_config(
            temp.path(),
            "muke",
            "export default { view: { target: 'es2015' } };",
        );
        let config = host_config(Some(21), "muke", "muke");
        let warnings = collect_view_target_warnings(temp.path(), &config, Some(21));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("muke/lxapp.config.ts"));
        assert!(warnings[0].contains("'es2015'"));
        assert!(warnings[0].contains("minSdk = 21"));
    }

    #[test]
    fn warns_when_no_lxapp_config_present() {
        // Default (no view.target) routes through the modern pipeline,
        // which is exactly the dangerous case on old WebView.
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join("muke")).unwrap();
        let config = host_config(Some(21), "muke", "muke");
        let warnings = collect_view_target_warnings(temp.path(), &config, Some(21));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("(default, modern)"));
    }

    #[test]
    fn no_warning_when_target_es5() {
        let temp = TempDir::new().unwrap();
        write_lxapp_config(
            temp.path(),
            "muke",
            "export default { view: { target: \"ES5\" } };", // case-insensitive
        );
        let config = host_config(Some(21), "muke", "muke");
        assert!(collect_view_target_warnings(temp.path(), &config, Some(21)).is_empty());
    }

    #[test]
    fn no_warning_when_min_sdk_modern() {
        let temp = TempDir::new().unwrap();
        write_lxapp_config(
            temp.path(),
            "muke",
            "export default { view: { target: 'es2015' } };",
        );
        let config = host_config(Some(28), "muke", "muke");
        assert!(collect_view_target_warnings(temp.path(), &config, Some(28)).is_empty());
    }

    #[test]
    fn no_warning_when_min_sdk_unset() {
        let temp = TempDir::new().unwrap();
        write_lxapp_config(
            temp.path(),
            "muke",
            "export default { view: { target: 'es2015' } };",
        );
        let config = host_config(None, "muke", "muke");
        assert!(collect_view_target_warnings(temp.path(), &config, None).is_empty());
    }
}

mod polyfills_asset_decision {
    use super::*;
    use crate::config::{ResourceBundleConfig, ResourceBundleType, ResourcesConfig};

    fn config_with_bundle(bundle_path: &str) -> LingXiaConfig {
        let mut config = LingXiaConfig::new_android("demo", "com.example.demo", "muke");
        config.resources = Some(ResourcesConfig {
            bundles: vec![ResourceBundleConfig {
                bundle_type: ResourceBundleType::Lxapp,
                app_id: "muke".to_string(),
                path: Some(bundle_path.to_string()),
                package: None,
                version: None,
            }],
        });
        config
    }

    fn write_lxapp_config(root: &Path, bundle_path: &str, contents: &str) {
        let dir = root.join(bundle_path);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("lxapp.config.ts"), contents).unwrap();
    }

    #[test]
    fn true_when_bundle_view_target_is_es5() {
        let temp = TempDir::new().unwrap();
        write_lxapp_config(
            temp.path(),
            "muke",
            "export default { view: { target: 'es5' } };",
        );
        assert!(any_path_bundle_targets_es5(
            temp.path(),
            &config_with_bundle("muke"),
        ));
    }

    #[test]
    fn case_insensitive_match() {
        let temp = TempDir::new().unwrap();
        write_lxapp_config(
            temp.path(),
            "muke",
            "export default { view: { target: \"ES5\" } };",
        );
        assert!(any_path_bundle_targets_es5(
            temp.path(),
            &config_with_bundle("muke"),
        ));
    }

    #[test]
    fn false_when_bundle_view_target_is_modern() {
        let temp = TempDir::new().unwrap();
        write_lxapp_config(
            temp.path(),
            "muke",
            "export default { view: { target: 'es2015' } };",
        );
        assert!(!any_path_bundle_targets_es5(
            temp.path(),
            &config_with_bundle("muke"),
        ));
    }

    #[test]
    fn false_when_bundle_has_no_lxapp_config() {
        // No lxapp.config.ts ⇒ default (modern) pipeline, no polyfills script.
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join("muke")).unwrap();
        assert!(!any_path_bundle_targets_es5(
            temp.path(),
            &config_with_bundle("muke"),
        ));
    }

    #[test]
    fn false_when_no_resources() {
        let temp = TempDir::new().unwrap();
        let mut config = LingXiaConfig::new_android("demo", "com.example.demo", "muke");
        config.resources = None;
        assert!(!any_path_bundle_targets_es5(temp.path(), &config));
    }
}
