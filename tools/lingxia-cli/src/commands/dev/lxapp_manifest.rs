use crate::config::LingXiaConfig;
use anyhow::{Context, Result, anyhow};
use lingxia_control_protocol::dev_session::ViewWire;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const MANIFEST_FILE: &str = "manifest.json";
/// Published file bytes by sha256, and the manifests that list them by dist hash: a hot swap
/// rewrites the build in place, so bytes are served from here, not from the build.
const BLOBS_DIR: &str = "blobs";
const SNAPSHOTS_DIR: &str = "snapshots";
const KEPT_SNAPSHOTS: usize = 8;
/// A blob younger than this may belong to a publish still writing its snapshot.
const BLOB_GRACE: std::time::Duration = std::time::Duration::from_secs(600);

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

/// Each path bundle of the host, from the `dist` its embed build wrote: copied into the
/// session's own build, then published from there.
pub(crate) fn publish_configured(
    project_root: &Path,
    config: &LingXiaConfig,
    output: &super::session_output::SessionOutput,
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
        // A bundle this session built is already in its directory; a prebuilt one is copied in.
        let session_built = output.build_dir(&bundle.app_id);
        let built = if session_built.join("lxapp.json").is_file() {
            session_built
        } else {
            let dist_dir = project_root.join(bundle_path).join("dist");
            if !dist_dir.join("lxapp.json").is_file() {
                continue;
            }
            output.seed(&bundle.app_id, &dist_dir)?
        };
        let manifest = build_manifest(&built)
            .with_context(|| format!("Failed to build dev manifest for {}", built.display()))?;
        if manifest.app_id != bundle.app_id {
            output.remove_app(&bundle.app_id);
            return Err(anyhow!(
                "resources.bundles[{}].appId does not match its built lxapp.json appId '{}'",
                bundle.app_id,
                manifest.app_id
            ));
        }
        let manifest = capture(&output.store(), &built, manifest)?;
        write_manifest(&output.store(), &manifest)?;
        written.push(manifest);
    }
    Ok(written)
}

/// The View wire the compiled GPU pages in `dist_dir` need: their UI ABI and the newest field
/// any header declares. `None` for an lxapp without compiled GPU pages.
pub(crate) fn view_requirement(dist_dir: &Path) -> Option<ViewWire> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(dist_dir.join("lxapp.json")).ok()?).ok()?;
    let pages: Vec<&str> = match manifest.get("pages")? {
        serde_json::Value::Array(pages) => pages
            .iter()
            .filter_map(|page| page.get("path").and_then(serde_json::Value::as_str))
            .collect(),
        serde_json::Value::Object(pages) => pages
            .values()
            .filter_map(serde_json::Value::as_str)
            .collect(),
        _ => return None,
    };
    let mut need: Option<ViewWire> = None;
    for page in pages {
        let Some(header) = module_header(&dist_dir.join(page)) else {
            continue;
        };
        let (Some(ui_abi), Some(field)) = (header["uiAbi"].as_u64(), header["minField"].as_u64())
        else {
            continue;
        };
        let (ui_abi, field) = (ui_abi as u32, field as u32);
        need = Some(match need {
            Some(seen) => ViewWire {
                ui_abi: seen.ui_abi.max(ui_abi),
                max_field: seen.max_field.max(field),
            },
            None => ViewWire {
                ui_abi,
                max_field: field,
            },
        });
    }
    need
}

/// The capability manifest on a compiled GPU page's first line.
fn module_header(page: &Path) -> Option<serde_json::Value> {
    use std::io::Read;
    let mut head = [0u8; 4096];
    let read = fs::File::open(page).ok()?.read(&mut head).ok()?;
    let head = String::from_utf8_lossy(&head[..read]);
    let line = head.lines().next()?;
    let json = line.strip_prefix(crate::lxapp::UI_MODULE_HEADER)?;
    serde_json::from_str(json).ok()
}

/// Whether this session publishes `app_id`: its manifest has been written.
pub(crate) fn is_published(store: &Path, app_id: &str) -> bool {
    manifest_path(store, app_id).is_file()
}

pub(crate) fn load_manifest(store: &Path, app_id: &str) -> Result<DevLxAppManifest> {
    let path = manifest_path(store, app_id);
    let bytes = fs::read(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("Invalid JSON in {}", path.display()))
}

/// A listed file from the session's build of `app_id`, when its captured bytes are gone.
pub(crate) fn resolve_published_file(
    store: &Path,
    app_id: &str,
    relative_path: &str,
    dist_dir: &Path,
) -> Result<PathBuf> {
    validate_relative_path(relative_path)?;
    let manifest = load_manifest(store, app_id)?;
    if !manifest.files.iter().any(|file| file.path == relative_path) {
        return Err(anyhow!(
            "file is not listed in dev manifest: {relative_path}"
        ));
    }
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

pub(crate) fn publish_dist(store: &Path, dist_dir: &Path) -> Result<DevLxAppManifest> {
    let manifest = build_manifest(dist_dir)
        .with_context(|| format!("Failed to build dev manifest for {}", dist_dir.display()))?;
    let manifest = capture(store, dist_dir, manifest)?;
    write_manifest(store, &manifest)?;
    Ok(manifest)
}

pub(crate) fn remove_manifest(store: &Path, app_id: &str) -> Result<()> {
    let path = manifest_path(store, app_id);
    if path.is_file() {
        fs::remove_file(&path).with_context(|| format!("Failed to remove {}", path.display()))?;
    }
    Ok(())
}

/// Why this lxapp cannot be attached: the host already ships it.
pub(crate) fn declared_bundle_conflict(
    project_root: &Path,
    source: &Path,
    app_id: &str,
) -> Result<Option<String>> {
    if !project_root.join("lingxia.yaml").is_file() {
        return Ok(None);
    }
    let config = LingXiaConfig::load(project_root)?;
    let Some(resources) = config.resources.as_ref() else {
        return Ok(None);
    };
    let source = source
        .canonicalize()
        .unwrap_or_else(|_| source.to_path_buf());
    for bundle in &resources.bundles {
        if bundle.app_id == app_id {
            return Ok(Some(format!(
                "{app_id} is already declared in this host's lingxia.yaml. Attaching it would \
                 replace that bundle."
            )));
        }
        let Some(bundle_path) = bundle
            .path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Ok(bundle_dir) = project_root.join(bundle_path).canonicalize() else {
            continue;
        };
        if bundle_dir == source {
            return Ok(Some(format!(
                "{} is already the host bundle {}. Attaching it would replace that bundle.",
                source.display(),
                bundle.app_id
            )));
        }
    }
    Ok(None)
}

fn write_manifest(store: &Path, manifest: &DevLxAppManifest) -> Result<()> {
    let path = manifest_path(store, &manifest.app_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(manifest)?;
    let snapshots = app_dir(store, &manifest.app_id).join(SNAPSHOTS_DIR);
    fs::create_dir_all(&snapshots)
        .with_context(|| format!("Failed to create {}", snapshots.display()))?;
    write_atomic(
        &snapshots.join(format!("{}.json", manifest.dist_hash)),
        &bytes,
    )?;
    if fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
        write_atomic(&path, &bytes)?;
    }
    prune(store, &manifest.app_id);
    Ok(())
}

fn app_dir(store: &Path, app_id: &str) -> PathBuf {
    store.join(sanitize_component(app_id))
}

fn manifest_path(store: &Path, app_id: &str) -> PathBuf {
    app_dir(store, app_id).join(MANIFEST_FILE)
}

/// Copy each listed file into the blob store. A file rewritten since it was hashed is
/// listed as the bytes captured, so the manifest always describes what will be served.
fn capture(
    store: &Path,
    dist_dir: &Path,
    mut manifest: DevLxAppManifest,
) -> Result<DevLxAppManifest> {
    let blobs = app_dir(store, &manifest.app_id).join(BLOBS_DIR);
    fs::create_dir_all(&blobs).with_context(|| format!("Failed to create {}", blobs.display()))?;
    let mut moved = false;
    for file in &mut manifest.files {
        if blobs.join(&file.hash).is_file() {
            continue;
        }
        let source = dist_dir.join(&file.path);
        let bytes =
            fs::read(&source).with_context(|| format!("Failed to read {}", source.display()))?;
        let hash = sha256_hex(&bytes);
        if hash != file.hash || bytes.len() as u64 != file.size {
            file.hash = hash;
            file.size = bytes.len() as u64;
            moved = true;
        }
        write_atomic(&blobs.join(&file.hash), &bytes)?;
    }
    if moved {
        manifest.dist_hash = hash_file_list(&manifest.files);
    }
    Ok(manifest)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary, bytes)
        .with_context(|| format!("Failed to write {}", temporary.display()))?;
    fs::rename(&temporary, path).with_context(|| format!("Failed to publish {}", path.display()))
}

/// Keep the newest snapshots and the blobs they list; best effort.
fn prune(store: &Path, app_id: &str) {
    let dir = app_dir(store, app_id);
    let Ok(entries) = fs::read_dir(dir.join(SNAPSHOTS_DIR)) else {
        return;
    };
    let mut snapshots: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    snapshots.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    for (_, stale) in snapshots.iter().skip(KEPT_SNAPSHOTS) {
        let _ = fs::remove_file(stale);
    }
    let mut listed = BTreeSet::new();
    for (_, path) in snapshots.iter().take(KEPT_SNAPSHOTS) {
        let Some(manifest) = fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<DevLxAppManifest>(&bytes).ok())
        else {
            continue;
        };
        listed.extend(manifest.files.into_iter().map(|file| file.hash));
    }
    let Ok(blobs) = fs::read_dir(dir.join(BLOBS_DIR)) else {
        return;
    };
    for blob in blobs.filter_map(|entry| entry.ok()) {
        let name = blob.file_name().to_string_lossy().into_owned();
        let old = blob
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > BLOB_GRACE);
        if old && !listed.contains(&name) {
            let _ = fs::remove_file(blob.path());
        }
    }
}

/// The manifest published as `dist_hash`, while its snapshot is kept.
pub(crate) fn load_snapshot(
    store: &Path,
    app_id: &str,
    dist_hash: &str,
) -> Result<Option<DevLxAppManifest>> {
    if !is_sha256(dist_hash) {
        return Ok(None);
    }
    let path = app_dir(store, app_id)
        .join(SNAPSHOTS_DIR)
        .join(format!("{dist_hash}.json"));
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .with_context(|| format!("Invalid JSON in {}", path.display())),
        Err(_) => Ok(None),
    }
}

/// The captured bytes of a published file, by its sha256.
pub(crate) fn read_blob(store: &Path, app_id: &str, hash: &str) -> Option<Vec<u8>> {
    if !is_sha256(hash) {
        return None;
    }
    fs::read(app_dir(store, app_id).join(BLOBS_DIR).join(hash)).ok()
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
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
        let (hash, size) = file_hash(&path)?;
        files.push(DevLxAppManifestFile {
            path: rel,
            hash,
            size,
        });
    }
    Ok(files)
}

/// A file's sha256 and size; a dev session rehashes only files whose size or mtime moved.
fn file_hash(path: &Path) -> Result<(String, u64)> {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};
    type Stamp = (u64, std::time::SystemTime);
    static HASHES: LazyLock<Mutex<HashMap<PathBuf, (Stamp, String)>>> =
        LazyLock::new(Default::default);
    let meta = fs::metadata(path).with_context(|| format!("Failed to read {}", path.display()))?;
    // A file written in the last two seconds is rehashed: a coarse mtime could hide a rewrite.
    let stamp = meta
        .modified()
        .ok()
        .filter(|modified| modified.elapsed().is_ok_and(|age| age.as_secs() >= 2))
        .map(|modified| (meta.len(), modified));
    if let Some(stamp) = stamp
        && let Some((held, hash)) = HASHES.lock().unwrap_or_else(|p| p.into_inner()).get(path)
        && *held == stamp
    {
        return Ok((hash.clone(), stamp.0));
    }
    let bytes = fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let hash = sha256_hex(&bytes);
    if let Some(stamp) = stamp.filter(|(size, _)| *size == bytes.len() as u64) {
        HASHES
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(path.to_path_buf(), (stamp, hash.clone()));
    }
    Ok((hash, bytes.len() as u64))
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
    fn view_requirement_is_the_newest_field_any_compiled_page_declares() {
        let temp = tempfile::tempdir().unwrap();
        let dist = temp.path();
        fs::create_dir_all(dist.join("pages/a")).unwrap();
        fs::create_dir_all(dist.join("pages/b")).unwrap();
        let header = |field: u32| {
            format!(
                "{}{{\"uiAbi\":1,\"minField\":{field},\"requiredFeatures\":[],\"compiler\":\"0.28.0\"}}\n(function(){{}})()",
                crate::lxapp::UI_MODULE_HEADER
            )
        };
        fs::write(dist.join("pages/a/index.js"), header(224)).unwrap();
        fs::write(dist.join("pages/b/index.js"), header(230)).unwrap();
        fs::write(
            dist.join("lxapp.json"),
            r#"{"appId":"demo","renderer":"gpu","pages":[{"name":"a","path":"pages/a/index.js"},{"name":"b","path":"pages/b/index.js"}]}"#,
        )
        .unwrap();
        assert_eq!(
            view_requirement(dist),
            Some(ViewWire {
                ui_abi: 1,
                max_field: 230
            })
        );

        let web = temp.path().join("web");
        fs::create_dir_all(web.join("pages/home")).unwrap();
        fs::write(web.join("pages/home/index.html"), "<!doctype html>").unwrap();
        fs::write(
            web.join("lxapp.json"),
            r#"{"appId":"web","pages":{"home":"pages/home/index.html"}}"#,
        )
        .unwrap();
        assert_eq!(view_requirement(&web), None, "WebView pages need no View");
    }

    #[test]
    fn a_path_bundle_is_published_from_the_sessions_copy_of_its_dist() {
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
            theme: None,
            settings_destination: None,
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
            splash: None,
            assets: None,
            update: None,
        };

        let output = super::super::session_output::SessionOutput::at(project.join("session"));
        let manifests = publish_configured(project, &config, &output).unwrap();
        assert_eq!(manifests.len(), 1);
        assert!(output.store().join("demo/manifest.json").is_file());
        assert!(output.build_dir("demo").join("logic.js").is_file());
        assert!(!dist.join("manifest.json").exists());
        assert!(!project.join(".lingxia/dev/lxapp").exists());
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

    #[test]
    fn attach_refuses_an_app_id_or_directory_the_host_already_declares() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path();
        fs::write(
            project.join("lingxia.yaml"),
            "app:\n  projectName: demo-host\n  packageId: app.example.demohost\n  productName: Demo Host\n  productVersion: 1.0.0\n  platforms: [windows]\n  homeAppId: demo\nresources:\n  bundles:\n    - type: lxapp\n      appId: demo\n      path: home\n",
        )
        .unwrap();
        fs::create_dir_all(project.join("home")).unwrap();
        let home = project.join("home").canonicalize().unwrap();
        let other = project.join("other");
        fs::create_dir_all(&other).unwrap();
        let other = other.canonicalize().unwrap();

        let by_id = declared_bundle_conflict(project, &other, "demo")
            .unwrap()
            .unwrap();
        assert!(by_id.contains("already declared"));
        let by_dir = declared_bundle_conflict(project, &home, "someone-else")
            .unwrap()
            .unwrap();
        assert!(by_dir.contains("already the host bundle"));
        assert!(
            declared_bundle_conflict(project, &other, "someone-else")
                .unwrap()
                .is_none()
        );
    }
}
