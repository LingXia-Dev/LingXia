use super::*;

struct Fixture {
    root: tempfile::TempDir,
    manager: LxApps,
    appid: String,
    channel: Channel,
}

impl Fixture {
    fn new() -> Self {
        // Metadata is process-wide; keep its backing file alive across fixtures.
        static METADATA: OnceLock<tempfile::TempDir> = OnceLock::new();
        METADATA.get_or_init(|| {
            let root = tempfile::tempdir().unwrap();
            metadata::init(root.path().join("metadata.redb")).unwrap();
            root
        });
        let root = tempfile::tempdir().unwrap();
        let runtime = Platform::new(
            root.path().join("data").display().to_string(),
            root.path().join("cache").display().to_string(),
            "en-US".to_string(),
        )
        .unwrap();
        Self {
            root,
            manager: LxApps::new(runtime, LxAppWorkers::init(1), 1),
            appid: format!("app.lingxia.update-recovery.{}", Uuid::new_v4()),
            channel: crate::default_channel(),
        }
    }

    fn install_previous(&self) -> PathBuf {
        self.install_channel(self.channel)
    }

    fn install_channel(&self, channel: Channel) -> PathBuf {
        let path = self.root.path().join(format!("installed-{channel}"));
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("lxapp.json"),
            serde_json::json!({
                "appId": self.appid,
                "version": "1.0.0",
                "logic": false,
                "pages": [{"name": "home", "path": "pages/home/index"}]
            })
            .to_string(),
        )
        .unwrap();
        metadata::upsert(&metadata::LxAppRecord::new(
            &self.appid,
            channel,
            lingxia_update::SemanticVersion::from_version(&Version::parse("1.0.0").unwrap()),
            lxapp_fingermark(&self.appid, channel),
            path.display().to_string(),
            0,
        ))
        .unwrap();
        path
    }

    fn stage_update(&self, manifest: Option<serde_json::Value>) {
        let archive = self.root.path().join("update.lxapp");
        if let Some(manifest) = manifest {
            let source = self.root.path().join("candidate");
            fs::create_dir_all(&source).unwrap();
            fs::write(source.join("lxapp.json"), manifest.to_string()).unwrap();
            let encoder =
                zstd::stream::write::Encoder::new(fs::File::create(&archive).unwrap(), 0).unwrap();
            let mut builder = tar::Builder::new(encoder);
            builder.append_dir_all(".", source).unwrap();
            builder.into_inner().unwrap().finish().unwrap();
        } else {
            fs::write(&archive, b"invalid zstd download").unwrap();
        }
        metadata::downloaded_upsert(&self.appid, self.channel, "1.1.0", &archive, None).unwrap();
    }

    fn assert_download_discarded(&self) {
        assert!(
            metadata::downloaded_get(&self.appid, self.channel)
                .unwrap()
                .is_none()
        );
        assert!(!self.root.path().join("update.lxapp").exists());
    }
}

#[test]
fn explicit_channel_switch_replaces_live_and_closed_sessions() {
    #[cfg(target_vendor = "apple")]
    let _host = crate::apple_host_stubs::headless_lifecycle();
    for home in [false, true] {
        for closed in [false, true] {
            let f = Fixture::new();
            let release_path = f.install_channel(Channel::Release);
            let draft_path = f.install_channel(Channel::Draft);
            let open = |channel| {
                if home {
                    f.manager.ensure_lxapp_with_session_class(
                        f.appid.clone(),
                        channel,
                        AppSessionClass::ControlApp,
                    )
                } else {
                    f.manager.ensure_lxapp(f.appid.clone(), channel)
                }
                .unwrap()
            };
            let release = if home {
                f.manager.initialize_home_lxapp(f.appid.clone()).unwrap()
            } else {
                open(Channel::Release)
            };
            release.set_status(if closed {
                LxAppSessionStatus::Closed
            } else {
                LxAppSessionStatus::Opened
            });
            let draft = open(Channel::Draft);
            assert_eq!(draft.release_type(), Channel::Draft);
            assert_eq!(draft.lxapp_dir, draft_path);
            assert_ne!(draft.storage_file_path, release.storage_file_path);
            assert_eq!(draft.app_session_class(), release.app_session_class());
            assert!(draft.resource_grants_sealed_for_test());
            assert_ne!(draft.session_id(), release.session_id());
            assert!(release.session.is_retired());
            release.session.revive();
            assert!(release.session.is_cancelled());
            assert!(Arc::ptr_eq(&draft, &open(Channel::Draft)));
            let restored = open(Channel::Release);
            assert_eq!(restored.release_type(), Channel::Release);
            assert_eq!(restored.lxapp_dir, release_path);
            assert_eq!(restored.storage_file_path, release.storage_file_path);
            assert_eq!(restored.app_session_class(), release.app_session_class());
            assert!(draft.session.is_retired());
            f.manager.retire_lxapp(&f.appid).unwrap();
        }
    }
}

#[test]
fn omitted_channel_keeps_the_live_draft_session() {
    #[cfg(target_vendor = "apple")]
    let _host = crate::apple_host_stubs::headless_lifecycle();
    let f = Fixture::new();
    f.install_channel(Channel::Release);
    f.install_channel(Channel::Draft);
    assert_eq!(
        f.manager.resolve_open_channel(&f.appid, None),
        Channel::Release
    );
    let draft = f
        .manager
        .ensure_lxapp(f.appid.clone(), Channel::Draft)
        .unwrap();
    draft.set_status(LxAppSessionStatus::Opened);
    // A pin click, notification tap or navigateToApp without `channel`.
    let resolved = f.manager.resolve_open_channel(&f.appid, None);
    assert_eq!(resolved, Channel::Draft);
    let again = f.manager.ensure_lxapp(f.appid.clone(), resolved).unwrap();
    assert!(Arc::ptr_eq(&draft, &again));
    assert!(!draft.session.is_retired());
    assert_eq!(
        f.manager
            .resolve_open_channel(&f.appid, Some(Channel::Release)),
        Channel::Release
    );
    f.manager.retire_lxapp(&f.appid).unwrap();
}

#[test]
fn explicit_channel_switch_applies_the_pending_target_package() {
    #[cfg(target_vendor = "apple")]
    let _host = crate::apple_host_stubs::headless_lifecycle();
    let mut f = Fixture::new();
    register_builtin_asset_bundle(f.appid.clone());
    let release_path = f.install_channel(Channel::Release);
    let release = f
        .manager
        .ensure_lxapp(f.appid.clone(), Channel::Release)
        .unwrap();
    release.set_status(LxAppSessionStatus::Opened);
    f.channel = Channel::Draft;
    f.stage_update(Some(serde_json::json!({
        "appId": f.appid, "version": "1.1.0", "logic": false,
        "pages": [{"name": "home", "path": "pages/home/index"}]
    })));
    let draft = f
        .manager
        .ensure_lxapp(f.appid.clone(), Channel::Draft)
        .unwrap();
    assert_eq!(draft.release_type(), Channel::Draft);
    assert_eq!(draft.current_version(), "1.1.0");
    assert!(release.session.is_retired());
    assert!(release_path.join("lxapp.json").exists());
    assert!(matches!(
        lxapp_bundle_source_for(&f.appid),
        Some(LxAppBundleSource::BuiltinAssets)
    ));
    assert!(
        metadata::downloaded_get(&f.appid, Channel::Draft)
            .unwrap()
            .is_none()
    );
    f.manager.retire_lxapp(&f.appid).unwrap();
}

#[test]
fn installed_draft_ignores_a_registered_release_asset_bundle() {
    let f = Fixture::new();
    let draft_path = f.install_channel(Channel::Draft);
    register_builtin_asset_bundle(f.appid.clone());
    let draft = f
        .manager
        .ensure_lxapp(f.appid.clone(), Channel::Draft)
        .unwrap();
    assert!(matches!(draft.bundle_source, LxAppBundleSource::Installed));
    assert_eq!(draft.lxapp_dir, draft_path);
    assert_eq!(draft.release_type(), Channel::Draft);
}

#[test]
fn downloaded_draft_cannot_inherit_control_surface_authority() {
    let f = Fixture::new();
    f.install_channel(Channel::Release);
    f.install_channel(Channel::Draft);
    let surface = Arc::new(
        LxApp::new_with_session_class_for_test(
            f.appid.clone(),
            f.manager.runtime.clone(),
            f.manager.executor.clone(),
            AppSessionClass::ControlSurface,
        )
        .unwrap(),
    );
    f.manager.lxapps.insert(f.appid.clone(), surface.clone());
    assert!(matches!(
        f.manager.ensure_lxapp(f.appid.clone(), Channel::Draft),
        Err(LxAppError::InvalidParameter(_))
    ));
    assert!(!surface.session.is_retired());
    assert!(Arc::ptr_eq(
        f.manager.lxapps.get(&f.appid).unwrap().value(),
        &surface
    ));
}

#[test]
fn dev_path_reuses_its_draft_session_for_default_opens() {
    #[cfg(target_vendor = "apple")]
    let _host = crate::apple_host_stubs::headless_lifecycle();
    let f = Fixture::new();
    let root = f.install_previous();
    register_dev_bundle_source(f.appid.clone(), root);
    let app = f
        .manager
        .ensure_lxapp(f.appid.clone(), Channel::Release)
        .unwrap();
    assert_eq!(app.release_type(), Channel::Draft);
    let recalled = f
        .manager
        .ensure_lxapp(f.appid.clone(), Channel::Release)
        .unwrap();
    assert!(Arc::ptr_eq(&app, &recalled));
    f.manager.retire_lxapp(&f.appid).unwrap();
}

#[tokio::test]
async fn failed_pending_update_recreates_previous_install_after_teardown() {
    #[cfg(target_vendor = "apple")]
    let _host = crate::apple_host_stubs::headless_lifecycle();
    for home in [false, true] {
        for invalid_manifest in [false, true] {
            let f = Fixture::new();
            let installed = f.install_previous();
            let manifest_before = fs::read(installed.join("lxapp.json")).unwrap();
            let old = if home {
                f.manager.initialize_home_lxapp(f.appid.clone())
            } else {
                f.manager.ensure_lxapp(f.appid.clone(), f.channel)
            }
            .unwrap();
            old.set_status(LxAppSessionStatus::Opened);
            f.stage_update(invalid_manifest.then(|| {
                serde_json::json!({
                    "appId": "another.app",
                    "version": "1.1.0"
                })
            }));

            // A Later/open while live must leave both the session and download alone.
            let recalled = f.manager.ensure_lxapp(f.appid.clone(), f.channel).unwrap();
            assert!(Arc::ptr_eq(&old, &recalled));
            assert!(
                metadata::downloaded_get(&f.appid, f.channel)
                    .unwrap()
                    .is_some()
            );

            let replacement = f
                .manager
                .recreate_lxapp(f.appid.clone(), f.channel, old.session_id())
                .await
                .expect("failed update must reopen the previous install");
            assert!(old.session.is_retired());
            assert_ne!(replacement.session_id(), old.session_id());
            assert!(!replacement.session.is_cancelled());
            assert_eq!(replacement.app_session_class(), old.app_session_class());
            assert!(replacement.resource_grants_sealed_for_test());
            assert_eq!(replacement.lxapp_dir, installed);
            assert_eq!(replacement.config().version, "1.0.0");
            assert_eq!(replacement.current_version(), "1.0.0");
            assert_eq!(
                fs::read(installed.join("lxapp.json")).unwrap(),
                manifest_before
            );
            f.assert_download_discarded();
            f.manager.retire_lxapp(&f.appid).unwrap();
        }
    }
}

#[test]
fn failed_pending_update_on_cold_open_uses_previous_install() {
    let f = Fixture::new();
    let installed = f.install_previous();
    f.stage_update(None);
    let app = f.manager.ensure_lxapp(f.appid.clone(), f.channel).unwrap();
    assert_eq!(app.lxapp_dir, installed);
    assert_eq!(app.current_version(), "1.0.0");
    f.assert_download_discarded();
}

#[test]
fn failed_first_install_does_not_publish_a_session() {
    let f = Fixture::new();
    f.stage_update(None);
    assert!(f.manager.ensure_lxapp(f.appid.clone(), f.channel).is_err());
    assert!(!f.manager.lxapps.contains_key(&f.appid));
    assert!(metadata::get(&f.appid, f.channel).unwrap().is_none());
    f.assert_download_discarded();
}
