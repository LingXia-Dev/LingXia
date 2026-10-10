use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use lingxia_app_context::{AppConfig, AppEnv, service_env, set_app_config};
use lingxia_update::{
    BoxFuture, Channel, LxAppUpdateHost, UpdateError, UpdatePackageInfo, UpdateVerifyTarget,
    blocks_dev_service_packages, check_update_enabled, default_channel, ensure_lxapp_first_install,
    ensure_lxapp_target_version_ready, spawn_lxapp_background_update_check, verify_checked_update,
};

#[derive(Clone)]
struct CountingHost {
    installed: bool,
    channel: Channel,
    version: Option<String>,
    checks: Arc<AtomicUsize>,
    downloads: Arc<AtomicUsize>,
    spawned: Arc<AtomicUsize>,
}

impl CountingHost {
    fn new(installed: bool) -> Self {
        Self::with_channel(installed, Channel::Release)
    }

    fn draft() -> Self {
        Self::with_channel(true, Channel::Draft)
    }

    fn with_channel(installed: bool, channel: Channel) -> Self {
        Self {
            installed,
            channel,
            version: installed.then(|| "1.0.0".to_string()),
            checks: Arc::new(AtomicUsize::new(0)),
            downloads: Arc::new(AtomicUsize::new(0)),
            spawned: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl LxAppUpdateHost for CountingHost {
    fn spawn_detached(&self, _: BoxFuture<'static, ()>) {
        self.spawned.fetch_add(1, Ordering::SeqCst);
    }
    fn target_appid(&self) -> &str {
        "guest"
    }
    fn channel(&self) -> Channel {
        self.channel
    }
    fn is_ota_managed(&self) -> bool {
        true
    }
    fn runtime_version(&self) -> &str {
        "1.0.0"
    }
    fn current_version_hint(&self) -> Option<String> {
        self.version.clone()
    }
    fn installed_version(&self) -> BoxFuture<'_, Result<Option<String>, UpdateError>> {
        let version = self.version.clone();
        Box::pin(async move { Ok(version) })
    }
    fn installed_checksum(&self) -> BoxFuture<'_, Result<Option<String>, UpdateError>> {
        Box::pin(async { Ok(None) })
    }
    fn is_installed(&self) -> BoxFuture<'_, Result<bool, UpdateError>> {
        let installed = self.installed;
        Box::pin(async move { Ok(installed) })
    }
    fn check_latest_update<'a>(
        &'a self,
        _: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Option<UpdatePackageInfo>, UpdateError>> {
        self.checks.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(None) })
    }
    fn check_exact_update<'a>(
        &'a self,
        _: &'a str,
    ) -> BoxFuture<'a, Result<Option<UpdatePackageInfo>, UpdateError>> {
        self.checks.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(None) })
    }
    fn has_downloaded_update<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
    ) -> BoxFuture<'a, Result<bool, UpdateError>> {
        Box::pin(async { Ok(false) })
    }
    fn download_update<'a>(
        &'a self,
        _: &'a UpdatePackageInfo,
    ) -> BoxFuture<'a, Result<(), UpdateError>> {
        self.downloads.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
    fn emit_update_ready(&self, _: &str) -> Result<(), UpdateError> {
        Ok(())
    }
    fn emit_update_failed(&self, _: &UpdatePackageInfo, _: &str) -> Result<(), UpdateError> {
        Ok(())
    }
    fn is_bundled_available(&self) -> bool {
        false
    }
    fn register_builtin_bundle(&self) -> Result<(), UpdateError> {
        unreachable!()
    }
    fn has_update_provider(&self) -> bool {
        true
    }
    fn log_warning(&self, _: &str) {}
}

#[test]
fn prod_host_on_the_dev_service_does_not_install_its_packages() {
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
    assert!(!blocks_dev_service_packages());
    assert!(!check_update_enabled(&[]));
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
    assert!(blocks_dev_service_packages());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let missing = CountingHost::new(false);
    let err = runtime
        .block_on(ensure_lxapp_first_install(&missing))
        .unwrap_err();
    assert!(
        err.to_string()
            .contains(lingxia_update::DEV_SERVICE_PACKAGE_REFUSAL),
        "{err}"
    );
    assert_eq!(missing.checks.load(Ordering::SeqCst), 0);
    assert_eq!(missing.downloads.load(Ordering::SeqCst), 0);
    let installed = CountingHost::new(true);
    runtime
        .block_on(ensure_lxapp_first_install(&installed))
        .unwrap();
    let err = runtime
        .block_on(ensure_lxapp_target_version_ready(&installed, "2.0.0"))
        .unwrap_err();
    assert!(
        err.to_string()
            .contains(lingxia_update::DEV_SERVICE_PACKAGE_REFUSAL),
        "{err}"
    );
    let draft = CountingHost::draft();
    runtime
        .block_on(ensure_lxapp_target_version_ready(&draft, "1.0.0"))
        .unwrap();
    let err = runtime
        .block_on(ensure_lxapp_target_version_ready(&draft, "2.0.0"))
        .unwrap_err();
    assert!(
        err.to_string()
            .contains(lingxia_update::DEV_SERVICE_PACKAGE_REFUSAL),
        "{err}"
    );
    assert_eq!(draft.checks.load(Ordering::SeqCst), 0);
    assert_eq!(draft.downloads.load(Ordering::SeqCst), 0);
    spawn_lxapp_background_update_check(installed.clone(), Some("1.0.0".into()));
    assert_eq!(installed.checks.load(Ordering::SeqCst), 0);
    assert_eq!(installed.downloads.load(Ordering::SeqCst), 0);
    assert_eq!(installed.spawned.load(Ordering::SeqCst), 0);
    // The dev service waives nothing: a prod build still refuses unsigned
    // packages of every kind and channel.
    assert!(!check_update_enabled(&[]));
    for kind in ["lxapp", "lxplugin", "app"] {
        target.kind = kind.into();
        for channel in ["draft", "release", ""] {
            target.channel = channel.into();
            assert!(verify_checked_update(package.clone(), &target, &[]).is_err());
        }
    }
    std::fs::remove_dir_all(data_dir).unwrap();
}
