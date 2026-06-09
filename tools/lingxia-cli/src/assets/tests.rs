use super::{
    any_path_bundle_targets_es5, build_app_json_from_config, build_ui_json_from_config,
    collect_view_target_warnings, is_png_path, prepare_app_ui_icons, validate_app_ui_svg_icon,
};
use crate::config::{EnvVersion, HostAppConfig, LingXiaConfig, LingxiaServer, ResolvedEnv};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn test_resolved_env() -> ResolvedEnv {
    ResolvedEnv {
        version: EnvVersion::Release,
        lingxia_server: "https://api.example.com".to_string(),
        package_id_suffix: None,
    }
}

#[test]
fn png_path_check_accepts_png_case_insensitively() {
    assert!(is_png_path(Path::new("splash.png")));
    assert!(is_png_path(Path::new("SPLASH.PNG")));
    assert!(is_png_path(Path::new("assets/launch.PnG")));
}

#[test]
fn png_path_check_rejects_non_png_extensions() {
    assert!(!is_png_path(Path::new("splash.jpg")));
    assert!(!is_png_path(Path::new("splash.jpeg")));
    assert!(!is_png_path(Path::new("splash.webp")));
    assert!(!is_png_path(Path::new("splash")));
}

#[test]
fn generated_app_json_excludes_ui_fields() {
    let config = LingXiaConfig {
        app: Some(HostAppConfig {
            project_name: "demo".into(),
            product_name: "Demo".into(),
            product_version: "1.2.3".into(),
            lingxia_server: Some(LingxiaServer::Single("http://127.0.0.1:8080".into())),
            lingxia_id: Some("demo".into()),
            package_id_suffix: None,
            platforms: vec!["macos".into()],
            home_app_id: "demo-home".into(),
        }),
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: None,
        shell: None,
        ui: Some(serde_json::json!({
            "launch": { "initialSurface": "main" },
            "surfaces": [],
            "activators": []
        })),
        app_links: None,
        storage: None,
        resources: None,
    };

    let app_json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();

    assert!(value.get("ui").is_none());
    assert!(value.get("panels").is_none());
    assert!(value.get("splashTimeout").is_none());
}

#[test]
fn generated_app_json_includes_dev_ws_url_when_configured() {
    let config = LingXiaConfig {
        app: Some(HostAppConfig {
            project_name: "demo".into(),
            product_name: "Demo".into(),
            product_version: "1.2.3".into(),
            lingxia_server: None,
            lingxia_id: None,
            package_id_suffix: None,
            platforms: vec!["android".into()],
            home_app_id: "demo-home".into(),
        }),
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: None,
        shell: None,
        ui: None,
        app_links: None,
        storage: None,
        resources: None,
    };

    let app_json = build_app_json_from_config(
        &config,
        None,
        Some("ws://127.0.0.1:12345"),
        &test_resolved_env(),
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();

    assert_eq!(value["devWsUrl"], "ws://127.0.0.1:12345");
}

#[test]
fn generated_app_json_includes_app_link_hosts() {
    let config = LingXiaConfig {
        app: Some(HostAppConfig {
            project_name: "demo".into(),
            product_name: "Demo".into(),
            product_version: "1.2.3".into(),
            lingxia_server: None,
            lingxia_id: None,
            package_id_suffix: None,
            platforms: vec!["android".into()],
            home_app_id: "demo-home".into(),
        }),
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: None,
        shell: None,
        ui: None,
        app_links: Some(crate::config::AppLinksConfig {
            hosts: vec!["www.example.com".into()],
        }),
        storage: None,
        resources: None,
    };

    let app_json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();

    assert_eq!(value["appLinks"]["hosts"][0], "www.example.com");
}

#[test]
fn generated_app_json_includes_capabilities() {
    let config = LingXiaConfig {
        app: Some(HostAppConfig {
            project_name: "demo".into(),
            product_name: "Demo".into(),
            product_version: "1.2.3".into(),
            lingxia_server: None,
            lingxia_id: None,
            package_id_suffix: None,
            platforms: vec!["android".into()],
            home_app_id: "demo-home".into(),
        }),
        android: None,
        ios: None,
        macos: None,
        harmony: None,
        windows: None,
        features: None,
        capabilities: Some(crate::config::CapabilitiesConfig {
            notifications: true,
            terminal: true,
        }),
        shell: None,
        ui: None,
        app_links: None,
        storage: None,
        resources: None,
    };

    let app_json = build_app_json_from_config(&config, None, None, &test_resolved_env()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();

    assert_eq!(value["capabilities"]["notifications"], true);
    assert_eq!(value["capabilities"]["terminal"], true);
}

#[test]
fn generated_ui_json_matches_ui_section() {
    let ui = serde_json::json!({
        "launch": { "initialSurface": "main" },
        "surfaces": [{
            "id": "main",
            "presentation": { "style": "window" },
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
        shell: None,
        ui: Some(ui.clone()),
        app_links: None,
        storage: None,
        resources: None,
    };

    let temp = TempDir::new().unwrap();
    let icons = prepare_app_ui_icons(temp.path(), &config).unwrap();
    let ui_json = build_ui_json_from_config(&config, &icons).unwrap().unwrap();
    let value: serde_json::Value = serde_json::from_str(&ui_json).unwrap();
    println!("{}", serde_json::to_string_pretty(&value).unwrap());
    assert_eq!(value, ui);
}

#[test]
fn generated_ui_json_rewrites_app_ui_icons() {
    let ui = serde_json::json!({
        "launch": { "initialSurface": "main" },
        "surfaces": [],
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
        shell: None,
        ui: Some(ui),
        app_links: None,
        storage: None,
        resources: None,
    };
    let icons = vec![super::PreparedAppUiIcon {
        relative_path: "icons/browser-deadbeef.pdf".to_string(),
        source_path: "icons/browser.svg".to_string(),
        bytes: Vec::new(),
        hash: "deadbeef".to_string(),
    }];

    let ui_json = build_ui_json_from_config(&config, &icons).unwrap().unwrap();
    let value: serde_json::Value = serde_json::from_str(&ui_json).unwrap();
    assert_eq!(value["activators"][0]["icon"], "icons/browser-deadbeef.pdf");
}

#[test]
fn generated_ui_json_adds_terminal_for_capability() {
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
            terminal: true,
        }),
        shell: None,
        ui: Some(serde_json::json!({
            "launch": { "initialSurface": "main" },
            "surfaces": [{
                "id": "main",
                "presentation": { "kind": "window" },
                "content": { "kind": "lxapp", "appId": "demo-home" }
            }],
            "activators": []
        })),
        app_links: None,
        storage: None,
        resources: None,
    };

    let temp = TempDir::new().unwrap();
    let icons = prepare_app_ui_icons(temp.path(), &config).unwrap();
    let ui_json = build_ui_json_from_config(&config, &icons).unwrap().unwrap();
    let value: serde_json::Value = serde_json::from_str(&ui_json).unwrap();

    assert_eq!(value["surfaces"][1]["id"], "terminal");
    assert_eq!(value["surfaces"][1]["presentation"]["attachTo"], "main");
    assert_eq!(value["surfaces"][1]["presentation"]["edge"], "bottom");
    assert_eq!(value["surfaces"][1]["content"]["kind"], "terminal");
    assert!(value["surfaces"][1]["content"].get("backend").is_none());
    assert_eq!(value["activators"][0]["id"], "terminalSidebar");
    assert_eq!(value["activators"][0]["hostSurface"], "main");
    assert!(
        value["activators"][0]["icon"]
            .as_str()
            .unwrap()
            .starts_with("icons/terminal-")
    );
    assert_eq!(value["activators"][0]["action"]["surface"], "terminal");
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
            terminal: false,
        }),
        shell: None,
        ui: Some(serde_json::json!({
            "launch": { "initialSurface": "main" },
            "surfaces": [{
                "id": "main",
                "presentation": { "kind": "window" },
                "content": { "kind": "lxapp", "appId": "demo-home" }
            }, {
                "id": "terminal",
                "presentation": {
                    "kind": "attachPanel",
                    "attachTo": "main",
                    "edge": "bottom"
                },
                "content": { "kind": "terminal" }
            }],
            "activators": []
        })),
        app_links: None,
        storage: None,
        resources: None,
    };

    let err = build_ui_json_from_config(&config, &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("capabilities.terminal is not enabled"));
}

#[test]
fn generated_ui_json_adds_terminal_activators_when_missing() {
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
            terminal: true,
        }),
        shell: None,
        ui: Some(serde_json::json!({
            "launch": { "initialSurface": "main" },
            "surfaces": [{
                "id": "main",
                "presentation": { "kind": "window" },
                "content": { "kind": "lxapp", "appId": "demo-home" }
            }]
        })),
        app_links: None,
        storage: None,
        resources: None,
    };

    let temp = TempDir::new().unwrap();
    let icons = prepare_app_ui_icons(temp.path(), &config).unwrap();
    let ui_json = build_ui_json_from_config(&config, &icons).unwrap().unwrap();
    let value: serde_json::Value = serde_json::from_str(&ui_json).unwrap();

    assert_eq!(value["activators"][0]["id"], "terminalSidebar");
    assert_eq!(value["activators"][0]["hostSurface"], "main");
}

#[test]
fn generated_ui_json_attaches_terminal_to_initial_root_surface() {
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
            terminal: true,
        }),
        shell: None,
        ui: Some(serde_json::json!({
            "launch": { "initialSurface": "mainPanel" },
            "surfaces": [{
                "id": "secondary",
                "presentation": { "kind": "window" },
                "content": { "kind": "lxapp", "appId": "secondary-home" }
            }, {
                "id": "mainPanel",
                "presentation": { "kind": "panel", "anchor": "activator" },
                "content": { "kind": "lxapp", "appId": "main-home" }
            }],
            "activators": []
        })),
        app_links: None,
        storage: None,
        resources: None,
    };

    let temp = TempDir::new().unwrap();
    let icons = prepare_app_ui_icons(temp.path(), &config).unwrap();
    let ui_json = build_ui_json_from_config(&config, &icons).unwrap().unwrap();
    let value: serde_json::Value = serde_json::from_str(&ui_json).unwrap();

    assert_eq!(
        value["surfaces"][2]["presentation"]["attachTo"],
        "mainPanel"
    );
    assert_eq!(value["activators"][0]["hostSurface"], "mainPanel");
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
        shell: None,
        ui: Some(serde_json::json!({
            "launch": { "initialSurface": "main" },
            "surfaces": [],
            "activators": [{
                "id": "browser",
                "kind": "sidebarItem",
                "icon": "icons/browser.png",
                "action": { "kind": "toggleSurface", "surface": "main" }
            }]
        })),
        app_links: None,
        storage: None,
        resources: None,
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
            package_id: "com.example.demo".to_string(),
            min_sdk,
            target_sdk: Some(35),
            compile_sdk: Some(35),
            ndk_version: None,
            api_level: None,
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
