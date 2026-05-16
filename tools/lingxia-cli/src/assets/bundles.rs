use super::cache::{HostAssetsCache, LxAppBuildStamp};
use super::hash::{hash_tree, path_key, sha256_hex};
use super::shell_webui::{
    APP_ID as SHELL_WEBUI_APP_ID, resolve_lxapp_package, resolve_shell_webui_dir,
};
use crate::config::{
    HOST_CONFIG_FILE, LXAPP_BUILD_CONFIG_FILE, LingXiaConfig, ResourceBundleConfig,
};
use crate::lxapp;
use crate::lxapp::ProjectFramework;
use crate::platform::BuildProfile;
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use semver::Version;
use sha2::Digest;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) struct PreparedResourceBundle {
    pub(super) dist_dir: PathBuf,
    pub(super) asset_name: String,
    pub(super) dist_hash: String,
    pub(super) version: String,
}

struct ResourceBundlePlan {
    bundle_dir: PathBuf,
    asset_name: String,
    output_dir: PathBuf,
    version: String,
    framework_override: Option<ProjectFramework>,
    build: bool,
}

pub(super) fn prepare_home_app_bundle(
    project_root: &Path,
    config: &LingXiaConfig,
    build_profile: BuildProfile,
    framework_override: Option<ProjectFramework>,
    progress_override: Option<&str>,
    cache: &mut HostAssetsCache,
) -> Result<PreparedResourceBundle> {
    let app = config
        .app
        .as_ref()
        .ok_or_else(|| anyhow!("Missing app settings in {}", HOST_CONFIG_FILE))?;
    if let Some(bundle) = resource_bundle_for_app_id(config, &app.home_app_id)
        && resource_bundle_has_source(bundle)
    {
        println!("{}", "Preparing home LxApp bundle...".bold());
        return prepare_resource_bundle(
            project_root,
            bundle,
            "home-lxapp",
            build_profile,
            framework_override,
            progress_override,
            cache,
        );
    }

    Err(anyhow!(
        "app.homeAppId '{}' requires a matching resources.bundles entry with path or package",
        app.home_app_id
    ))
}

pub(super) fn prepare_resource_lxapp_bundles(
    project_root: &Path,
    config: &LingXiaConfig,
    build_profile: BuildProfile,
    framework_override: Option<ProjectFramework>,
    progress_override: Option<&str>,
    cache: &mut HostAssetsCache,
) -> Result<Vec<PreparedResourceBundle>> {
    let home_app_id = config
        .app
        .as_ref()
        .map(|app| app.home_app_id.as_str())
        .unwrap_or_default();
    let Some(resources) = config.resources.as_ref() else {
        return Ok(Vec::new());
    };

    let mut bundles = Vec::new();
    for bundle in &resources.bundles {
        if bundle.app_id == home_app_id || bundle.app_id == SHELL_WEBUI_APP_ID {
            continue;
        }
        if !resource_bundle_has_source(bundle) {
            continue;
        }
        println!(
            "{}",
            format!("Preparing resource bundle: {}", bundle.app_id).bold()
        );
        bundles.push(prepare_resource_bundle(
            project_root,
            bundle,
            "resource-lxapp",
            build_profile,
            framework_override,
            progress_override,
            cache,
        )?);
    }
    Ok(bundles)
}

fn resource_bundle_for_app_id<'a>(
    config: &'a LingXiaConfig,
    app_id: &str,
) -> Option<&'a ResourceBundleConfig> {
    config
        .resources
        .as_ref()?
        .bundles
        .iter()
        .find(|bundle| bundle.app_id == app_id)
}

fn resource_bundle_has_source(bundle: &ResourceBundleConfig) -> bool {
    bundle
        .path
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
        || bundle
            .package
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
}

fn prepare_resource_bundle(
    project_root: &Path,
    bundle: &ResourceBundleConfig,
    cache_kind: &str,
    build_profile: BuildProfile,
    framework_override: Option<ProjectFramework>,
    progress_override: Option<&str>,
    cache: &mut HostAssetsCache,
) -> Result<PreparedResourceBundle> {
    let source = resolve_resource_bundle_source(project_root, bundle)?;
    prepare_lxapp_bundle_dir(
        source.bundle_dir,
        &bundle.app_id,
        &format!("resources.bundles[{}]", bundle.app_id),
        cache_kind,
        build_profile,
        framework_override,
        progress_override,
        source.build,
        cache,
    )
}

struct ResourceBundleSource {
    bundle_dir: PathBuf,
    build: bool,
}

fn resolve_resource_bundle_source(
    project_root: &Path,
    bundle: &ResourceBundleConfig,
) -> Result<ResourceBundleSource> {
    if let Some(path) = bundle
        .path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return Ok(ResourceBundleSource {
            bundle_dir: project_root.join(path),
            build: true,
        });
    }

    let package = bundle
        .package
        .as_deref()
        .map(str::trim)
        .filter(|package| !package.is_empty())
        .ok_or_else(|| {
            anyhow!(
                "resources.bundles[{}] must set path or package",
                bundle.app_id
            )
        })?;
    let version = bundle
        .version
        .as_deref()
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .unwrap_or(env!("CARGO_PKG_VERSION"));
    Ok(ResourceBundleSource {
        bundle_dir: resolve_lxapp_package(project_root, package, version)?,
        build: false,
    })
}

fn prepare_lxapp_bundle_dir(
    bundle_dir: PathBuf,
    expected_app_id: &str,
    config_key: &str,
    cache_kind: &str,
    build_profile: BuildProfile,
    framework_override: Option<ProjectFramework>,
    progress_override: Option<&str>,
    build: bool,
    cache: &mut HostAssetsCache,
) -> Result<PreparedResourceBundle> {
    if !bundle_dir.exists() {
        return Err(anyhow!(
            "Configured {config_key}.path not found: {}",
            bundle_dir.display()
        ));
    }
    let lxapp_json = bundle_dir.join("lxapp.json");
    let lxapp_build_config = bundle_dir.join(LXAPP_BUILD_CONFIG_FILE);
    if !lxapp_json.exists() {
        return Err(anyhow!(
            "Configured {config_key} must contain lxapp.json: {}",
            bundle_dir.display()
        ));
    }
    if build && !lxapp_build_config.exists() {
        return Err(anyhow!(
            "Configured {config_key} source bundle must contain {}: {}",
            LXAPP_BUILD_CONFIG_FILE,
            bundle_dir.display()
        ));
    }
    let metadata = read_lxapp_metadata(&lxapp_json)?;
    if metadata.app_id != expected_app_id {
        return Err(anyhow!(
            "{config_key}.appId '{}' does not match {} appId '{}'",
            expected_app_id,
            lxapp_json.display(),
            metadata.app_id
        ));
    }

    let plan = ResourceBundlePlan {
        bundle_dir: bundle_dir.clone(),
        asset_name: metadata.app_id,
        output_dir: bundle_dir.join("dist"),
        version: metadata.version,
        framework_override,
        build,
    };

    prepare_lxapp_plan(plan, build_profile, progress_override, cache_kind, cache)
}

pub(super) fn prepare_shell_webui_bundle(
    project_root: &Path,
    config: &LingXiaConfig,
    build_profile: BuildProfile,
    cache: &mut HostAssetsCache,
) -> Result<PreparedResourceBundle> {
    if let Some(bundle) = resource_bundle_for_app_id(config, SHELL_WEBUI_APP_ID)
        && resource_bundle_has_source(bundle)
    {
        return prepare_resource_bundle(
            project_root,
            bundle,
            "shell-webui",
            build_profile,
            None,
            None,
            cache,
        );
    }

    let source = resolve_shell_webui_dir(project_root, config)?;
    let lxapp_json = source.bundle_dir.join("lxapp.json");
    if !lxapp_json.exists() {
        return Err(anyhow!(
            "Configured shell.webui path must contain lxapp.json: {}",
            source.bundle_dir.display()
        ));
    }
    let metadata = read_lxapp_metadata(&lxapp_json)?;
    if metadata.app_id != SHELL_WEBUI_APP_ID {
        return Err(anyhow!(
            "shell.webui appId must be '{}', got '{}' in {}",
            SHELL_WEBUI_APP_ID,
            metadata.app_id,
            lxapp_json.display()
        ));
    }

    let plan = ResourceBundlePlan {
        bundle_dir: source.bundle_dir,
        asset_name: metadata.app_id,
        output_dir: lxapp_json
            .parent()
            .expect("lxapp.json has parent")
            .join("dist"),
        version: metadata.version,
        framework_override: None,
        build: source.build,
    };

    prepare_lxapp_plan(plan, build_profile, None, "shell-webui", cache)
}

fn prepare_lxapp_plan(
    plan: ResourceBundlePlan,
    build_profile: BuildProfile,
    progress_override: Option<&str>,
    kind: &str,
    cache: &mut HostAssetsCache,
) -> Result<PreparedResourceBundle> {
    let cache_key = format!(
        "{}|{}|{}|framework={}",
        path_key(&plan.bundle_dir),
        build_profile.as_str(),
        kind,
        plan.framework_override
            .map(|framework| framework.as_str())
            .unwrap_or("auto")
    );
    let inputs_hash = hash_resource_bundle_inputs(&plan)?;
    let mut needs_build = plan.build;

    if plan.build
        && let Some(stamp) = cache.lxapp_builds.get(&cache_key)
        && stamp.inputs_hash == inputs_hash
        && plan.output_dir.exists()
    {
        needs_build = false;
    }

    if needs_build {
        println!(
            "  {} {}",
            "Building bundle...".cyan(),
            plan.bundle_dir.display()
        );
        let mut args = vec!["build".to_string()];
        if matches!(build_profile, BuildProfile::Release) {
            args.push("--release".to_string());
        }
        if let Some(framework) = plan.framework_override {
            args.push("--framework".to_string());
            args.push(framework.as_str().to_string());
        }
        if let Some(progress) = progress_override {
            args.push("--progress".to_string());
            args.push(progress.to_string());
        }
        lxapp::run_in_dir(&args, &plan.bundle_dir)?;
    } else {
        let message = if plan.build {
            "bundle unchanged, skip build"
        } else {
            "using prebuilt bundle"
        };
        println!(
            "  {} {}: {}",
            "✓".green(),
            message,
            plan.bundle_dir.display()
        );
    }

    if !plan.output_dir.exists() {
        return Err(anyhow!(
            "Bundle build output not found: {}",
            plan.output_dir.display()
        ));
    }

    let dist_hash = hash_tree(&plan.output_dir, &[])?;
    cache.lxapp_builds.insert(
        cache_key,
        LxAppBuildStamp {
            inputs_hash,
            dist_hash: dist_hash.clone(),
            asset_name: plan.asset_name.clone(),
        },
    );

    Ok(PreparedResourceBundle {
        dist_dir: plan.output_dir,
        asset_name: plan.asset_name,
        dist_hash,
        version: plan.version,
    })
}

struct LxAppMetadata {
    app_id: String,
    version: String,
}

fn read_lxapp_metadata(path: &Path) -> Result<LxAppMetadata> {
    let content = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&content)?;
    let app_id = value
        .get("appId")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("appId missing in {}", path.display()))?;
    let version = value
        .get("version")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("version missing in {}", path.display()))?;
    Version::parse(version).map_err(|_| {
        anyhow!(
            "version in {} must be a semantic version (major.minor.patch)",
            path.display()
        )
    })?;

    Ok(LxAppMetadata {
        app_id: app_id.to_string(),
        version: version.to_string(),
    })
}

fn hash_resource_bundle_inputs(plan: &ResourceBundlePlan) -> Result<String> {
    let mut hasher = sha2::Sha256::new();
    hasher.update(b"bundle");
    hasher.update(path_key(&plan.bundle_dir).as_bytes());
    hasher.update(hash_tree(
        &plan.bundle_dir,
        &["dist", "node_modules", ".git", ".lingxia"],
    )?);

    Ok(sha256_hex(&hasher.finalize()))
}

pub(super) fn bundle_hashes(bundles: &[PreparedResourceBundle]) -> BTreeMap<String, String> {
    bundles
        .iter()
        .map(|bundle| (bundle.asset_name.clone(), bundle.dist_hash.clone()))
        .collect()
}

pub(super) fn sync_resource_bundles(
    target_root: &Path,
    bundles: &[PreparedResourceBundle],
    prev_hashes: Option<&BTreeMap<String, String>>,
) -> Result<bool> {
    let desired_hashes = bundle_hashes(bundles);
    let mut changed = false;

    if let Some(prev_hashes) = prev_hashes {
        for prev_name in prev_hashes.keys() {
            if !desired_hashes.contains_key(prev_name) {
                let stale_dir = target_root.join(prev_name);
                if stale_dir.exists() {
                    fs::remove_dir_all(&stale_dir)
                        .with_context(|| format!("Failed to remove {}", stale_dir.display()))?;
                    changed = true;
                }
            }
        }
    }

    for bundle in bundles {
        let target_dir = target_root.join(&bundle.asset_name);
        let prev_hash = prev_hashes.and_then(|hashes| hashes.get(&bundle.asset_name));
        if prev_hash == Some(&bundle.dist_hash) && target_dir.exists() {
            continue;
        }
        if target_dir.exists() {
            fs::remove_dir_all(&target_dir)
                .with_context(|| format!("Failed to remove {}", target_dir.display()))?;
        }
        copy_dir_recursive(&bundle.dist_dir, &target_dir)?;
        println!(
            "  {} bundle {} → {}",
            "✓".green(),
            bundle.asset_name,
            target_dir.display()
        );
        changed = true;
    }

    Ok(changed)
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    if !dest.exists() {
        fs::create_dir_all(dest)?;
    }

    let mut entries: Vec<_> = fs::read_dir(src)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        if is_apple_junk_entry(&entry.file_name()) {
            continue;
        }
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn is_apple_junk_entry(name: &std::ffi::OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    name == ".DS_Store" || name == "__MACOSX" || name.starts_with("._")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prebuilt_package_bundle_does_not_require_lxapp_build_config() {
        let temp = tempfile::tempdir().unwrap();
        let bundle_dir = temp.path().join("pkg");
        fs::create_dir_all(bundle_dir.join("dist")).unwrap();
        fs::write(
            bundle_dir.join("lxapp.json"),
            r#"{"appId":"pkg-app","version":"1.2.3","logic":false,"pages":[]}"#,
        )
        .unwrap();
        fs::write(bundle_dir.join("dist").join("asset.txt"), "ok").unwrap();

        let mut cache = HostAssetsCache::default();
        let prepared = prepare_lxapp_bundle_dir(
            bundle_dir,
            "pkg-app",
            "resources.bundles[pkg-app]",
            "resource-lxapp",
            BuildProfile::Debug,
            None,
            None,
            false,
            &mut cache,
        )
        .unwrap();

        assert_eq!(prepared.asset_name, "pkg-app");
        assert_eq!(prepared.version, "1.2.3");
    }
}
