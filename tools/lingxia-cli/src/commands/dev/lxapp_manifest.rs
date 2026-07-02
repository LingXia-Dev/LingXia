use crate::config::LingXiaConfig;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const DEV_MANIFEST_DIR: &str = ".lingxia/dev/lxapp";
const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DevLxAppManifest {
    pub app_id: String,
    pub version: String,
    pub dist_hash: String,
    pub files: Vec<DevLxAppManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DevLxAppManifestFile {
    pub path: String,
    pub hash: String,
    pub size: u64,
}

pub(crate) fn write_configured_manifests(
    project_root: &Path,
    config: &LingXiaConfig,
) -> Result<Vec<DevLxAppManifest>> {
    let Some(resources) = config.resources.as_ref() else {
        return Ok(Vec::new());
    };

    let mut written = Vec::new();
    for bundle in &resources.bundles {
        let Some(bundle_path) = bundle
            .path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let dist_dir = project_root.join(bundle_path).join("dist");
        if !dist_dir.join("lxapp.json").is_file() {
            continue;
        }
        let manifest = build_manifest(&dist_dir)
            .with_context(|| format!("Failed to build dev manifest for {}", dist_dir.display()))?;
        if manifest.app_id != bundle.app_id {
            return Err(anyhow!(
                "resources.bundles[{}].appId does not match dist/lxapp.json appId '{}'",
                bundle.app_id,
                manifest.app_id
            ));
        }
        write_manifest(project_root, &manifest)?;
        written.push(manifest);
    }
    Ok(written)
}

pub(crate) fn load_manifest(project_root: &Path, app_id: &str) -> Result<DevLxAppManifest> {
    let path = manifest_path(project_root, app_id);
    let bytes = fs::read(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("Invalid JSON in {}", path.display()))
}

pub(crate) fn resolve_dist_file(
    project_root: &Path,
    app_id: &str,
    relative_path: &str,
) -> Result<PathBuf> {
    validate_relative_path(relative_path)?;
    let manifest = load_manifest(project_root, app_id)?;
    if !manifest.files.iter().any(|file| file.path == relative_path) {
        return Err(anyhow!(
            "file is not listed in dev manifest: {relative_path}"
        ));
    }
    let dist_dir = configured_dist_dir(project_root, app_id)?;
    let file_path = dist_dir.join(relative_path);
    let dist_root = dist_dir
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize {}", dist_dir.display()))?;
    let resolved = file_path
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize {}", file_path.display()))?;
    if !resolved.starts_with(&dist_root) || !resolved.is_file() {
        return Err(anyhow!("invalid dev file path: {}", relative_path));
    }
    Ok(resolved)
}

fn write_manifest(project_root: &Path, manifest: &DevLxAppManifest) -> Result<()> {
    let path = manifest_path(project_root, &manifest.app_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(manifest)?;
    if fs::read(&path).ok().as_deref() == Some(bytes.as_slice()) {
        return Ok(());
    }
    fs::write(&path, bytes).with_context(|| format!("Failed to write {}", path.display()))
}

fn manifest_path(project_root: &Path, app_id: &str) -> PathBuf {
    project_root
        .join(DEV_MANIFEST_DIR)
        .join(sanitize_component(app_id))
        .join(MANIFEST_FILE)
}

fn configured_dist_dir(project_root: &Path, app_id: &str) -> Result<PathBuf> {
    let config = LingXiaConfig::load(project_root)?;
    let bundle = config
        .resources
        .as_ref()
        .and_then(|resources| {
            resources
                .bundles
                .iter()
                .find(|bundle| bundle.app_id == app_id)
        })
        .ok_or_else(|| anyhow!("No configured resource bundle for appId '{}'", app_id))?;
    let path = bundle
        .path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("Resource bundle '{}' is not path-based", app_id))?;
    Ok(project_root.join(path).join("dist"))
}

fn build_manifest(dist_dir: &Path) -> Result<DevLxAppManifest> {
    let lxapp_json = dist_dir.join("lxapp.json");
    let manifest_json: serde_json::Value = serde_json::from_slice(
        &fs::read(&lxapp_json)
            .with_context(|| format!("Failed to read {}", lxapp_json.display()))?,
    )
    .with_context(|| format!("Invalid JSON in {}", lxapp_json.display()))?;
    let app_id = manifest_field(&manifest_json, &lxapp_json, "appId")?;
    let version = manifest_field(&manifest_json, &lxapp_json, "version")?;
    let files = collect_files(dist_dir)?;
    let dist_hash = hash_file_list(&files);

    Ok(DevLxAppManifest {
        app_id,
        version,
        dist_hash,
        files,
    })
}

fn manifest_field(value: &serde_json::Value, path: &Path, field: &str) -> Result<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("Missing or empty '{}' in {}", field, path.display()))
}

fn collect_files(dist_dir: &Path) -> Result<Vec<DevLxAppManifestFile>> {
    let mut rel_paths = BTreeSet::new();
    collect_file_paths(dist_dir, dist_dir, &mut rel_paths)?;
    let mut files = Vec::with_capacity(rel_paths.len());
    for rel in rel_paths {
        validate_relative_path(&rel)?;
        let path = dist_dir.join(&rel);
        let bytes =
            fs::read(&path).with_context(|| format!("Failed to read {}", path.display()))?;
        files.push(DevLxAppManifestFile {
            path: rel,
            hash: sha256_hex(&bytes),
            size: bytes.len() as u64,
        });
    }
    Ok(files)
}

fn collect_file_paths(root: &Path, current: &Path, out: &mut BTreeSet<String>) -> Result<()> {
    let mut entries = fs::read_dir(current)
        .with_context(|| format!("Failed to read {}", current.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".DS_Store" || name.starts_with("._") {
            continue;
        }
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_file_paths(root, &path, out)?;
        } else if file_type.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel);
        }
    }
    Ok(())
}

fn validate_relative_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(anyhow!("invalid relative dev file path: {}", path));
    }
    Ok(())
}

fn sanitize_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '_',
        })
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hex_lower(&hasher.finalize())
}

fn hash_file_list(files: &[DevLxAppManifestFile]) -> String {
    let mut hasher = sha2::Sha256::new();
    for file in files {
        hasher.update(file.path.as_bytes());
        hasher.update([0]);
        hasher.update(file.size.to_le_bytes());
        hasher.update([0]);
        hasher.update(file.hash.as_bytes());
        hasher.update([0]);
    }
    hex_lower(&hasher.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LingXiaConfig, ResourceBundleConfig, ResourcesConfig};

    #[test]
    fn writes_manifest_outside_dist_for_path_bundle() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path();
        let dist = project.join("app").join("dist");
        fs::create_dir_all(dist.join("pages/home")).unwrap();
        fs::write(
            dist.join("lxapp.json"),
            r#"{"appId":"demo","version":"1.2.3","pages":{"home":"pages/home/index.html"}}"#,
        )
        .unwrap();
        fs::write(dist.join("logic.js"), "Page({})").unwrap();
        fs::write(dist.join("pages/home/index.html"), "<html></html>").unwrap();

        let config = LingXiaConfig {
            app: None,
            android: None,
            ios: None,
            macos: None,
            harmony: None,
            windows: None,
            features: None,
            capabilities: None,
            browser: None,
            generated_ui: None,
            surfaces: None,
            app_links: None,
            storage: None,
            resources: Some(ResourcesConfig {
                bundles: vec![ResourceBundleConfig {
                    bundle_type: Default::default(),
                    app_id: "demo".to_string(),
                    path: Some("app".to_string()),
                    package: None,
                    version: None,
                }],
            }),
        };

        let manifests = write_configured_manifests(project, &config).unwrap();
        assert_eq!(manifests.len(), 1);
        assert!(
            project
                .join(".lingxia/dev/lxapp/demo/manifest.json")
                .is_file()
        );
        assert!(!dist.join("manifest.json").exists());
        assert!(
            manifests[0]
                .files
                .iter()
                .any(|file| file.path == "logic.js")
        );
        assert!(
            manifests[0]
                .files
                .iter()
                .any(|file| file.path == "pages/home/index.html")
        );
    }

    #[test]
    fn rejects_unsafe_relative_paths() {
        assert!(validate_relative_path("pages/home/index.html").is_ok());
        assert!(validate_relative_path("../secret").is_err());
        assert!(validate_relative_path("/secret").is_err());
        assert!(validate_relative_path("pages\\home\\index.html").is_err());
        assert!(validate_relative_path("pages//home").is_err());
    }
}
