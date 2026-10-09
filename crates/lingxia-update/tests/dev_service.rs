use lingxia_app_context::{AppConfig, AppEnv, service_env, set_app_config};
use lingxia_update::{
    Channel, UpdatePackageInfo, UpdateVerifyTarget, check_package_update_enabled,
    check_update_enabled, default_channel, verify_checked_update,
};

#[test]
fn prod_host_restored_to_dev_opens_unsigned_drafts_without_update_keys() {
    // This integration test has its own process: app config and service env
    // are immutable OnceLocks, so unit tests must not mutate their globals.
    let config: AppConfig = serde_json::from_value(serde_json::json!({
        "productName": "Test",
        "productVersion": "1.0.0",
        "env": "prod",
        "lingxiaServer": "https://prod.example",
        "lingxiaServers": {
            "dev": "https://dev.example",
            "prod": "https://prod.example"
        }
    }))
    .unwrap();
    set_app_config(config.clone()).unwrap();
    let package = UpdatePackageInfo {
        version: "1.0.0".into(),
        url: "https://dev.example/package.tar.zst".into(),
        checksum_sha256: lingxia_update::archive_sha256_hex(b"package"),
        size: None,
        release_notes: None,
        min_runtime: None,
        authentication: None,
    };
    let mut target = UpdateVerifyTarget {
        kind: "lxapp".into(),
        target_id: "guest".into(),
        channel: "draft".into(),
        platform: "any".into(),
        exact_version: None,
    };
    assert!(!check_package_update_enabled("lxapp", "draft", &[]));
    assert!(verify_checked_update(package.clone(), &target, &[]).is_err());

    let data_dir = std::env::temp_dir().join(format!("lingxia-dev-service-{}", std::process::id()));
    std::fs::create_dir_all(data_dir.join("app_state")).unwrap();
    std::fs::write(
        data_dir.join("app_state/service-env.json"),
        r#"{"serviceEnv":"dev"}"#,
    )
    .unwrap();
    lingxia_app_context::service_env::install(&data_dir, &config).unwrap();
    assert_eq!(lingxia_app_context::env(), AppEnv::Prod);
    assert_eq!(service_env(), AppEnv::Dev);
    assert_eq!(default_channel(), Channel::Release);
    for kind in ["lxapp", "lxplugin"] {
        target.kind = kind.into();
        assert!(check_package_update_enabled(kind, "draft", &[]));
        verify_checked_update(package.clone(), &target, &[]).unwrap();
        target.channel = "release".into();
        assert!(!check_package_update_enabled(kind, "release", &[]));
        assert!(verify_checked_update(package.clone(), &target, &[]).is_err());
        target.channel = "draft".into();
    }
    target.kind = "app".into();
    target.channel.clear();
    assert!(!check_update_enabled(&[]));
    assert!(verify_checked_update(package, &target, &[]).is_err());
    std::fs::remove_dir_all(data_dir).unwrap();
}
