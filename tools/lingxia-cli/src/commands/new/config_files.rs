use super::types::{LxAppInfo, Platform, ProjectConfig};
use crate::config::{AndroidConfig, HarmonyConfig, HostAppConfig, IosConfig, LingXiaConfig};
use crate::versions::LingXiaVersions;
use anyhow::Result;
use std::fs;
use std::io::Write;

pub(super) fn generate_config_file(
    config: &ProjectConfig,
    lxapp: &LxAppInfo,
    versions: &LingXiaVersions,
) -> Result<()> {
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
        })
    } else {
        None
    };

    let harmony = if config.platforms.contains(&Platform::Harmony) {
        Some(HarmonyConfig {
            bundle_name: config.package_id.clone(),
            compile_sdk_version: None,
            compatible_sdk_version: None,
        })
    } else {
        None
    };

    let lingxia_config = LingXiaConfig {
        app: Some(HostAppConfig {
            project_name: config.name.clone(),
            product_name: config.product_name.clone(),
            product_version: "0.0.1".to_string(),
            api_server: None,
            platforms: platforms.clone(),
            home_lxapp_id: lxapp.app_id.clone(),
            home_lxapp_version: "1.0.0".to_string(),
            sdk_version: Some(versions.sdk.clone()),
        }),
        android,
        ios,
        harmony,
        resources: None,
    };

    // Save config file
    lingxia_config.save(&config.target_dir)?;

    Ok(())
}

pub(super) fn ensure_root_gitignore(config: &ProjectConfig) -> Result<()> {
    let gitignore_path = config.target_dir.join(".gitignore");
    let android_assets = format!(
        "android/{}/",
        crate::platform::detector::ANDROID_ASSETS_REL_PATH
    );
    let standalone_assets = format!("{}/", crate::platform::detector::ANDROID_ASSETS_REL_PATH);
    let required = [android_assets.as_str(), standalone_assets.as_str()];

    let existing = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    let mut append = String::new();
    for line in required {
        if !existing.lines().any(|l| l.trim() == line) {
            append.push_str(line);
            append.push('\n');
        }
    }
    if append.is_empty() {
        return Ok(());
    }

    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gitignore_path)?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        f.write_all(b"\n")?;
    }
    f.write_all(append.as_bytes())?;
    Ok(())
}
