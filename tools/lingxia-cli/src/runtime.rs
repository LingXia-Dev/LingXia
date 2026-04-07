use crate::config::LingXiaConfig;
use anyhow::{Context, Result, anyhow};
use flate2::read::GzDecoder;
use sha2::Digest;
use std::fs;
use std::io::Cursor;
use std::path::Component;
use std::path::{Path, PathBuf};

pub(crate) const DEFAULT_RUNTIME_PACKAGE: &str = "@lingxia/bridge";
const NPM_REGISTRY: &str = "https://registry.npmjs.org";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeEcmaTarget {
    Es5,
    Es2020,
}

impl RuntimeEcmaTarget {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RuntimeEcmaTarget::Es5 => "es5",
            RuntimeEcmaTarget::Es2020 => "es2020",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedRuntime {
    pub path: PathBuf,
    pub hash: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ScaffoldPackageVersions {
    pub bridge: String,
    pub types: String,
}

pub(crate) fn target_from_build_targets(build_targets: &[String]) -> RuntimeEcmaTarget {
    if build_targets.iter().any(|t| t.contains("armv7")) {
        RuntimeEcmaTarget::Es5
    } else {
        RuntimeEcmaTarget::Es2020
    }
}

pub(crate) fn resolve_runtime_js(
    project_root: &Path,
    config: &LingXiaConfig,
    ecma: RuntimeEcmaTarget,
) -> Result<ResolvedRuntime> {
    let runtime_override = config
        .resources
        .as_ref()
        .and_then(|r| r.runtime.as_ref())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    match runtime_override {
        Some(spec) => resolve_runtime_from_spec(project_root, spec, ecma),
        None => resolve_default_runtime(project_root, ecma),
    }
}

fn resolve_default_runtime(
    project_root: &Path,
    ecma: RuntimeEcmaTarget,
) -> Result<ResolvedRuntime> {
    if let Some(local_runtime_root) = find_repo_local_runtime_root(project_root)
        && let Ok(runtime) = resolve_runtime_from_local_path(&local_runtime_root, ecma)
    {
        return Ok(runtime);
    }

    resolve_runtime_from_npm(project_root, DEFAULT_RUNTIME_PACKAGE, None, ecma)
}

pub(crate) fn current_scaffold_versions() -> ScaffoldPackageVersions {
    let version = env!("CARGO_PKG_VERSION").to_string();
    ScaffoldPackageVersions {
        bridge: version.clone(),
        types: version,
    }
}

fn resolve_runtime_from_spec(
    project_root: &Path,
    spec: &str,
    ecma: RuntimeEcmaTarget,
) -> Result<ResolvedRuntime> {
    if let Some(spec) = spec.strip_prefix("npm:") {
        let (package, version) = parse_npm_package_spec(spec)?;
        return resolve_runtime_from_npm(project_root, &package, version.as_deref(), ecma);
    }

    if looks_like_version(spec) {
        return resolve_runtime_from_npm(project_root, DEFAULT_RUNTIME_PACKAGE, Some(spec), ecma);
    }

    let local_path = project_root.join(spec);
    if local_path.exists() {
        return resolve_runtime_from_local_path(&local_path, ecma);
    }

    let (package, version) = parse_npm_package_spec(spec)?;
    resolve_runtime_from_npm(project_root, &package, version.as_deref(), ecma)
}

fn resolve_runtime_from_local_path(
    path: &Path,
    ecma: RuntimeEcmaTarget,
) -> Result<ResolvedRuntime> {
    let runtime_path = resolve_runtime_file(path, ecma)?;
    let bytes = fs::read(&runtime_path)
        .with_context(|| format!("Failed to read runtime: {}", runtime_path.display()))?;
    Ok(ResolvedRuntime {
        path: runtime_path.clone(),
        hash: sha256_hex(&bytes),
        source: format!("local ({})", runtime_path.display()),
    })
}

fn resolve_runtime_from_npm(
    project_root: &Path,
    package: &str,
    version: Option<&str>,
    ecma: RuntimeEcmaTarget,
) -> Result<ResolvedRuntime> {
    let manifest = fetch_npm_manifest(package, version)?;
    let cache_root = resolve_runtime_cache_root(project_root);
    let cache_dir = cache_root
        .join("runtime")
        .join("npm")
        .join(safe_cache_segment(package))
        .join(&manifest.version);

    fs::create_dir_all(&cache_dir).with_context(|| {
        format!(
            "Failed to create runtime cache dir: {}",
            cache_dir.display()
        )
    })?;

    if resolve_runtime_file(&cache_dir, ecma).is_err() {
        let tarball = download_bytes(&manifest.tarball)
            .with_context(|| format!("Failed to download runtime tarball: {}", manifest.tarball))?;
        extract_runtime_dist_from_tarball(&tarball, &cache_dir).with_context(|| {
            format!(
                "Failed to extract runtime package {}@{}",
                package, manifest.version
            )
        })?;
    }

    let runtime_path = resolve_runtime_file(&cache_dir, ecma).with_context(|| {
        format!(
            "Runtime variant '{}' not found in {}@{}",
            ecma.as_str(),
            package,
            manifest.version
        )
    })?;
    let bytes = fs::read(&runtime_path)
        .with_context(|| format!("Failed to read runtime: {}", runtime_path.display()))?;

    Ok(ResolvedRuntime {
        path: runtime_path.clone(),
        hash: sha256_hex(&bytes),
        source: format!("npm:{}@{}", package, manifest.version),
    })
}

fn resolve_runtime_cache_root(project_root: &Path) -> PathBuf {
    if let Ok(explicit_target_dir) = std::env::var("CARGO_TARGET_DIR")
        && !explicit_target_dir.trim().is_empty()
    {
        let explicit_path = PathBuf::from(explicit_target_dir);
        return if explicit_path.is_absolute() {
            explicit_path
        } else {
            project_root.join(explicit_path)
        };
    }

    project_root.join("target")
}

fn find_repo_local_runtime_root(project_root: &Path) -> Option<PathBuf> {
    for dir in project_root.ancestors() {
        let candidate = dir.join("packages").join("lingxia-bridge");
        if resolve_runtime_file(&candidate, RuntimeEcmaTarget::Es2020).is_ok() {
            return Some(candidate);
        }
        // Stop at workspace/repo root to avoid leaking into unrelated parent directories.
        if dir.join(".git").exists() || dir.join("Cargo.lock").exists() {
            break;
        }
    }
    None
}

fn resolve_runtime_file(base_path: &Path, ecma: RuntimeEcmaTarget) -> Result<PathBuf> {
    let candidates: &[&str] = match ecma {
        RuntimeEcmaTarget::Es5 => &[
            "dist/runtime.es5.js",
            "runtime.es5.js",
            "dist/runtime.js",
            "runtime.js",
        ],
        RuntimeEcmaTarget::Es2020 => &[
            "dist/runtime.es2020.js",
            "runtime.es2020.js",
            "dist/runtime.js",
            "runtime.js",
        ],
    };

    if base_path.is_file() {
        return Ok(base_path.to_path_buf());
    }

    for rel in candidates {
        let candidate = base_path.join(rel);
        if candidate.exists() && candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err(anyhow!("No runtime.js found under {}", base_path.display()))
}

#[derive(Debug)]
struct NpmManifest {
    version: String,
    tarball: String,
}

fn fetch_npm_manifest(package: &str, version: Option<&str>) -> Result<NpmManifest> {
    let encoded = urlencoding::encode(package);
    let manifest_url = match version {
        Some(v) => format!("{NPM_REGISTRY}/{encoded}/{v}"),
        None => format!("{NPM_REGISTRY}/{encoded}/latest"),
    };
    let payload = fetch_json(&manifest_url)
        .with_context(|| format!("Failed to fetch npm package metadata: {manifest_url}"))?;

    let version = payload
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("npm metadata missing version field: {manifest_url}"))?
        .to_string();
    let tarball = payload
        .get("dist")
        .and_then(|d| d.get("tarball"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("npm metadata missing dist.tarball field: {manifest_url}"))?
        .to_string();

    Ok(NpmManifest { version, tarball })
}

fn fetch_json(url: &str) -> Result<serde_json::Value> {
    let mut resp = http_agent()
        .get(url)
        .header("User-Agent", "lingxia-cli")
        .call()
        .map_err(|e| anyhow!("Failed request to {url}: {e}"))?;
    if resp.status().as_u16() != 200 {
        return Err(anyhow!(
            "Request failed (HTTP {}) for {url}",
            resp.status().as_u16()
        ));
    }
    let body = resp
        .body_mut()
        .read_to_string()
        .context("Failed to read response body")?;
    serde_json::from_str(&body).context("Failed to parse JSON response")
}

fn download_bytes(url: &str) -> Result<Vec<u8>> {
    let mut resp = http_agent()
        .get(url)
        .header("User-Agent", "lingxia-cli")
        .call()
        .map_err(|e| anyhow!("Failed request to {url}: {e}"))?;
    if resp.status().as_u16() != 200 {
        return Err(anyhow!(
            "Download failed (HTTP {}) for {url}",
            resp.status().as_u16()
        ));
    }
    resp.body_mut()
        .read_to_vec()
        .context("Failed to read download body")
}

fn extract_runtime_dist_from_tarball(tarball: &[u8], out_root: &Path) -> Result<()> {
    let out_root_canon = out_root
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize {}", out_root.display()))?;
    let decoder = GzDecoder::new(Cursor::new(tarball));
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries().context("Failed to read tar entries")? {
        let mut entry = entry.context("Failed to read tar entry")?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().context("Failed to read tar path")?;
        if !path.starts_with("package/dist/") {
            continue;
        }
        let rel = path
            .strip_prefix("package")
            .context("Invalid tar package layout")?;
        if rel.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(anyhow!("Unsafe path in runtime tarball: {}", rel.display()));
        }
        let out_path = out_root.join(rel);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
            let parent_canon = parent
                .canonicalize()
                .with_context(|| format!("Failed to canonicalize {}", parent.display()))?;
            if !parent_canon.starts_with(&out_root_canon) {
                return Err(anyhow!(
                    "Refusing to unpack outside runtime cache: {}",
                    out_path.display()
                ));
            }
        }
        entry
            .unpack(&out_path)
            .with_context(|| format!("Failed to unpack {}", out_path.display()))?;
    }

    Ok(())
}

fn http_agent() -> ureq::Agent {
    crate::http_client::create_agent(60)
}

fn safe_cache_segment(package: &str) -> String {
    package
        .trim()
        .replace('@', "at-")
        .replace('/', "__")
        .replace(':', "_")
}

fn parse_npm_package_spec(spec: &str) -> Result<(String, Option<String>)> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err(anyhow!("Empty npm runtime spec"));
    }

    let version_sep = if spec.starts_with('@') {
        spec.rfind('@').filter(|idx| *idx > 0)
    } else {
        spec.find('@').filter(|idx| *idx > 0)
    };

    let (package, version) = if let Some(idx) = version_sep {
        let package = &spec[..idx];
        let version = &spec[idx + 1..];
        let version = if version.is_empty() {
            None
        } else {
            Some(version.to_string())
        };
        (package.to_string(), version)
    } else {
        (spec.to_string(), None)
    };

    if package.is_empty() {
        return Err(anyhow!("Invalid npm package spec: {spec}"));
    }
    Ok((package, version))
}

fn looks_like_version(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_digit() {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
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
