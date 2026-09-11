use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone)]
struct TestHost {
    channel: ReleaseType,
    package: UpdatePackageInfo,
    checksum: Option<String>,
    checksum_error: bool,
    pending_checksum: Option<String>,
    downloads: Arc<AtomicUsize>,
    exact_checks: Arc<AtomicUsize>,
}

impl TestHost {
    fn new(channel: ReleaseType) -> Self {
        Self {
            channel,
            package: UpdatePackageInfo {
                version: "1.0.0".into(),
                url: "https://example.test/package.tar.zst".into(),
                checksum_sha256: "new".into(),
                size: None,
                release_notes: None,
                is_force_update: true,
                required_runtime_version: None,
            },
            checksum: Some("old".into()),
            checksum_error: false,
            pending_checksum: None,
            downloads: Arc::new(AtomicUsize::new(0)),
            exact_checks: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl LxAppUpdateHost for TestHost {
    fn spawn_detached(&self, _: BoxFuture<'static, ()>) {
        unreachable!()
    }
    fn target_appid(&self) -> &str {
        "test-app"
    }
    fn channel(&self) -> ReleaseType {
        self.channel
    }
    fn is_ota_managed(&self) -> bool {
        true
    }
    fn runtime_version(&self) -> &str {
        "1.0.0"
    }
    fn current_version_hint(&self) -> Option<String> {
        Some("1.0.0".into())
    }
    fn installed_version(&self) -> BoxFuture<'_, Result<Option<String>, UpdateError>> {
        Box::pin(async { Ok(Some("1.0.0".into())) })
    }
    fn installed_checksum(&self) -> BoxFuture<'_, Result<Option<String>, UpdateError>> {
        Box::pin(async {
            if self.checksum_error {
                Err(UpdateError::io("checksum unavailable"))
            } else {
                Ok(self.checksum.clone())
            }
        })
    }
    fn is_installed(&self) -> BoxFuture<'_, Result<bool, UpdateError>> {
        Box::pin(async { Ok(true) })
    }
    fn check_latest_update<'a>(
        &'a self,
        _: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Option<UpdatePackageInfo>, UpdateError>> {
        Box::pin(async { Ok(Some(self.package.clone())) })
    }
    fn check_exact_update<'a>(
        &'a self,
        _: &'a str,
    ) -> BoxFuture<'a, Result<Option<UpdatePackageInfo>, UpdateError>> {
        self.exact_checks.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(Some(self.package.clone())) })
    }
    fn has_downloaded_update<'a>(
        &'a self,
        version: &'a str,
        checksum: &'a str,
    ) -> BoxFuture<'a, Result<bool, UpdateError>> {
        Box::pin(async move {
            Ok(version == self.package.version
                && self.pending_checksum.as_deref() == Some(checksum))
        })
    }
    fn download_update<'a>(
        &'a self,
        _: &'a UpdatePackageInfo,
    ) -> BoxFuture<'a, Result<(), UpdateError>> {
        self.downloads.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
    fn wait_for_or_start_force_download<'a>(
        &'a self,
        package: &'a UpdatePackageInfo,
    ) -> BoxFuture<'a, Result<(), UpdateError>> {
        self.download_update(package)
    }
    fn emit_update_ready(&self, _: &str, _: bool) -> Result<(), UpdateError> {
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

#[tokio::test]
async fn exact_developer_version_downloads_republished_package() {
    let mut host = TestHost::new(ReleaseType::Developer);
    host.pending_checksum = Some("old".into());
    ensure_target_version_ready(&host, "1.0.0").await.unwrap();
    assert_eq!(host.downloads.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn exact_developer_version_skips_installed_or_pending_checksum() {
    let mut host = TestHost::new(ReleaseType::Developer);
    host.checksum = Some("new".into());
    ensure_target_version_ready(&host, "1.0.0").await.unwrap();
    assert_eq!(host.exact_checks.load(Ordering::SeqCst), 1);
    assert_eq!(host.downloads.load(Ordering::SeqCst), 0);

    host.checksum = None;
    host.pending_checksum = Some("new".into());
    ensure_target_version_ready(&host, "1.0.0").await.unwrap();
    assert_eq!(host.downloads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn exact_release_and_preview_keep_version_only_shortcut() {
    for channel in [ReleaseType::Release, ReleaseType::Preview] {
        let host = TestHost::new(channel);
        ensure_target_version_ready(&host, "1.0.0").await.unwrap();
        assert_eq!(host.exact_checks.load(Ordering::SeqCst), 0);
        assert_eq!(host.downloads.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn force_gate_requires_republished_developer_checksum() {
    let mut host = TestHost::new(ReleaseType::Developer);
    host.pending_checksum = Some("old".into());
    ensure_force_update_for_installed(&host).await.unwrap();
    assert_eq!(host.downloads.load(Ordering::SeqCst), 1);
    host.checksum = Some("new".into());
    ensure_force_update_for_installed(&host).await.unwrap();
    assert_eq!(host.downloads.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn force_gate_does_not_read_checksum_for_optional_or_release_packages() {
    let mut host = TestHost::new(ReleaseType::Developer);
    host.checksum_error = true;
    host.package.is_force_update = false;
    ensure_force_update_for_installed(&host).await.unwrap();
    for channel in [ReleaseType::Release, ReleaseType::Preview] {
        host.channel = channel;
        host.package.is_force_update = true;
        ensure_force_update_for_installed(&host).await.unwrap();
    }
    assert_eq!(host.downloads.load(Ordering::SeqCst), 0);
}
