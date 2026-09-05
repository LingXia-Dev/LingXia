//! Build-time injection of an optional private provider crate into a host's
//! native crate. The crate is referenced only while a `--with-provider` build
//! runs, then the manifest + lockfile are restored — so the committed tree stays
//! self-contained and no provider source is baked in.
//!
//! The CLI hardcodes nothing about any specific provider: the injected crate's
//! name and its workspace-shared deps are read from the provider crate itself,
//! and the cargo features to enable are declared by the *host* crate. Per
//! provider `<NAME>` (which is either an inert host feature `<NAME> = []` for
//! temporary injection, or an already-active feature that names the provider
//! dependency):
//!   - source, highest priority first:
//!       1. `--provider-path <dir>`
//!       2. `LINGXIA_PROVIDER_<NAME>_PATH`
//!       3. `LINGXIA_PROVIDER_<NAME>_GIT` (+ `_REV` to pin, else `_REF` branch)
//!   - extra cargo features (additive to the provider's defaults) come from the
//!     host crate's `[package.metadata.lingxia.providers.<NAME>] features = [..]`.

use anyhow::{Context, Result, anyhow, bail};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Active injection. Patched manifests + lockfiles are restored on drop, so a
/// failed, panicking, or interrupted build never leaves the tree dirty.
pub struct ProviderInjection {
    backups: Vec<(PathBuf, Vec<u8>)>,
    features: Vec<String>,
}

impl ProviderInjection {
    /// Host cargo features to add to the native build (the activated `<NAME>`).
    pub fn features(&self) -> &[String] {
        &self.features
    }
}

impl Drop for ProviderInjection {
    fn drop(&mut self) {
        // A standalone native manifest can receive both a dependency override
        // and a source-wide git patch. Restore repeated backups newest-first so
        // the original bytes win.
        for (path, original) in self.backups.drain(..).rev() {
            if let Err(err) = fs::write(&path, &original) {
                eprintln!("⚠ provider: failed to restore {}: {err}", path.display());
            }
        }
    }
}

/// A provider resolved entirely from its source crate + the host manifest.
struct ResolvedProvider {
    /// Host feature to enable; equal to the `--with-provider` name.
    feature: String,
    /// Injected crate's package name, read from its `[package].name`.
    crate_name: String,
    /// Local source directory (a `--provider-path`/env path, or a git clone).
    dir: PathBuf,
    /// The provider's dependency names — intersected with workspace members to
    /// derive the crates.io->local patches the provider needs to unify types.
    deps: Vec<String>,
    /// Extra cargo features the host asked for (additive to provider defaults).
    features: Vec<String>,
}

/// Inject the requested providers into the native crate at `rust_lib_dir`,
/// returning a guard that restores everything on drop plus the host features to
/// enable. Returns `Ok(None)` when no provider was requested.
pub fn inject(
    rust_lib_dir: &Path,
    with_provider: &[String],
    provider_path: Option<&str>,
) -> Result<Option<ProviderInjection>> {
    if with_provider.is_empty() {
        return Ok(None);
    }
    let mut guard = ProviderInjection {
        backups: Vec::new(),
        features: Vec::new(),
    };
    let member_root = workspace_member_root(rust_lib_dir);
    let members = match &member_root {
        Some(root) => workspace_members(root)?,
        None => BTreeMap::new(),
    };
    for name in with_provider {
        let resolved = resolve_provider(rust_lib_dir, name, provider_path)?;
        let provider_git_source = dependency_git_source(rust_lib_dir, &resolved.crate_name)?;
        guard.features.push(resolved.feature.clone());
        patch_native_manifest(rust_lib_dir, &resolved, &mut guard)?;
        if let Some(git_source) = provider_git_source {
            patch_git_dependency_source(
                member_root.as_deref().unwrap_or(rust_lib_dir),
                &git_source,
                &resolved,
                &mut guard,
            )?;
        }
        if let Some(root) = member_root.as_deref() {
            let patches: Vec<(String, PathBuf)> = resolved
                .deps
                .iter()
                .filter_map(|dep| members.get(dep).map(|dir| (dep.clone(), dir.clone())))
                .collect();
            patch_workspace_root(root, &patches, &mut guard)?;
        }
    }
    // Back up the lockfile cargo will rewrite, so injected entries never leak.
    let lock_dir = member_root.as_deref().unwrap_or(rust_lib_dir);
    backup_lock(lock_dir, &mut guard);
    verify_framework_not_duplicated(lock_dir)?;
    println!(
        "  \u{2022} Provider(s): {} (source: {})",
        with_provider.join(", "),
        describe_source(provider_path)
    );
    Ok(Some(guard))
}

fn dependency_git_source(dir: &Path, crate_name: &str) -> Result<Option<String>> {
    let manifest = dir.join("Cargo.toml");
    let original =
        fs::read_to_string(&manifest).with_context(|| format!("reading {}", manifest.display()))?;
    let parsed: toml::Value =
        toml::from_str(&original).with_context(|| format!("parsing {}", manifest.display()))?;
    Ok(parsed
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .and_then(|dependencies| dependencies.get(crate_name))
        .and_then(toml::Value::as_table)
        .and_then(|dependency| dependency.get("git"))
        .and_then(toml::Value::as_str)
        .map(str::to_string))
}

/// Override the same git package everywhere in the host dependency graph. A
/// product command crate may depend on the provider beside the native host;
/// changing only the direct dependency would link two provider singletons.
fn patch_git_dependency_source(
    root: &Path,
    git_source: &str,
    provider: &ResolvedProvider,
    guard: &mut ProviderInjection,
) -> Result<()> {
    let manifest = root.join("Cargo.toml");
    let original =
        fs::read_to_string(&manifest).with_context(|| format!("reading {}", manifest.display()))?;
    let mut parsed: toml::Table = toml::from_str(&original).context("parsing patch manifest")?;
    let patches = parsed
        .entry("patch".to_string())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .context("patch must be a table")?;
    let source = patches
        .entry(git_source.to_string())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .context("patch source must be a table")?;
    let mut dependency = toml::Table::new();
    dependency.insert(
        "path".to_string(),
        toml::Value::String(provider.dir.to_string_lossy().into_owned()),
    );
    source.insert(provider.crate_name.clone(), toml::Value::Table(dependency));
    let patched = toml::to_string(&parsed).context("serializing provider patch")?;
    guard
        .backups
        .push((manifest.clone(), original.into_bytes()));
    fs::write(&manifest, patched).with_context(|| format!("patching {}", manifest.display()))?;
    Ok(())
}

/// Fail when a framework crate resolves from crates.io beside the local copy.
///
/// Cargo drops a `[patch]` whose version does not satisfy a requirement — it
/// does not warn, it resolves the real crate from crates.io instead. A provider
/// pinning `lingxia-platform = "0.11"` against a 0.12 workspace therefore pulls
/// a second, older copy of the framework, and the build fails much later at
/// link time against a symbol that was renamed. Name the cause here instead.
fn verify_framework_not_duplicated(workspace_dir: &Path) -> Result<()> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .current_dir(workspace_dir)
        .output()
        .context("running cargo metadata to check the injected dependency graph")?;
    if !output.status.success() {
        // Resolution failed for its own reasons; let the build report that.
        return Ok(());
    }
    let meta: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("parsing cargo metadata output")?;
    duplicated_framework_crates(&meta).map_or(Ok(()), Err)
}

/// The graph half of [`verify_framework_not_duplicated`], split out so the
/// pairing rule can be tested without resolving a real workspace.
fn duplicated_framework_crates(meta: &serde_json::Value) -> Option<anyhow::Error> {
    let packages = meta.get("packages")?.as_array()?;

    let mut local: BTreeMap<&str, &str> = BTreeMap::new();
    let mut registry: BTreeMap<&str, &str> = BTreeMap::new();
    for package in packages {
        let (Some(name), Some(version)) = (
            package.get("name").and_then(|n| n.as_str()),
            package.get("version").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        if !name.starts_with("lingxia") {
            continue;
        }
        // A path dependency has no source; anything else came from a registry.
        if package.get("source").is_some_and(|s| !s.is_null()) {
            registry.insert(name, version);
        } else {
            local.insert(name, version);
        }
    }

    let duplicated: Vec<String> = registry
        .iter()
        .filter_map(|(name, version)| {
            local
                .get(name)
                .map(|local| format!("  {name}: {version} from crates.io, {local} local"))
        })
        .collect();
    if duplicated.is_empty() {
        return None;
    }
    Some(anyhow!(
        "the build resolved two copies of the framework:\n{}\n\n\
         A crate in the graph requires a version the local workspace no longer \
         satisfies, so cargo skipped its [patch] and took the published crate \
         instead. Widen that requirement — a crate built through [patch] should \
         not pin a framework minor.",
        duplicated.join("\n")
    ))
}

fn resolve_provider(
    host_dir: &Path,
    name: &str,
    provider_path: Option<&str>,
) -> Result<ResolvedProvider> {
    let dir = resolve_source_dir(name, provider_path)?;
    let (crate_name, deps) = provider_crate(&dir)?;
    Ok(ResolvedProvider {
        feature: name.to_string(),
        crate_name,
        dir,
        deps,
        features: host_requested_features(host_dir, name)?,
    })
}

/// The provider crate's package name and its resolved dependency package names.
/// `cargo metadata` resolves `workspace = true` and renamed deps (e.g. a key
/// `lxapp` whose package is `lingxia-lxapp`) to real crate names, so the
/// workspace-patch intersection below is correct.
fn provider_crate(dir: &Path) -> Result<(String, Vec<String>)> {
    let meta = cargo_metadata(dir)?;
    let manifest = dir.join("Cargo.toml");
    let pkg = meta["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| {
            p["manifest_path"]
                .as_str()
                .is_some_and(|m| same_path(Path::new(m), &manifest))
        })
        .ok_or_else(|| anyhow!("provider crate not found at {}", dir.display()))?;
    let crate_name = pkg["name"]
        .as_str()
        .ok_or_else(|| anyhow!("provider package at {} has no name", dir.display()))?
        .to_string();
    let deps = pkg["dependencies"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| d["name"].as_str().map(str::to_string))
        .collect();
    Ok((crate_name, deps))
}

/// `cargo metadata --no-deps` for the workspace/crate at `manifest_dir`.
fn cargo_metadata(manifest_dir: &Path) -> Result<serde_json::Value> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .arg("--manifest-path")
        .arg(manifest_dir.join("Cargo.toml"))
        .output()
        .context("running cargo metadata")?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout).context("parsing cargo metadata")
}

/// Path equality that tolerates symlinks (e.g. macOS `/var` -> `/private/var`).
fn same_path(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// Cargo features the host crate asked to enable on the provider, from
/// `[package.metadata.lingxia.providers.<name>] features = [..]`. Empty when the
/// host doesn't declare any (the provider's own default features then apply).
fn host_requested_features(host_dir: &Path, name: &str) -> Result<Vec<String>> {
    let manifest = host_dir.join("Cargo.toml");
    let text =
        fs::read_to_string(&manifest).with_context(|| format!("reading {}", manifest.display()))?;
    let value: toml::Value =
        toml::from_str(&text).with_context(|| format!("parsing {}", manifest.display()))?;
    let features = value
        .get("package")
        .and_then(|v| v.get("metadata"))
        .and_then(|v| v.get("lingxia"))
        .and_then(|v| v.get("providers"))
        .and_then(|v| v.get(name))
        .and_then(|v| v.get("features"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Ok(features)
}

enum Source {
    Path(PathBuf),
    Git {
        url: String,
        rev: Option<String>,
        branch: Option<String>,
    },
}

/// Resolve a provider to a local directory (cloning a git source if needed), so
/// its `Cargo.toml` can be read uniformly.
fn resolve_source_dir(name: &str, provider_path: Option<&str>) -> Result<PathBuf> {
    match resolve_source(name, provider_path)? {
        Source::Path(p) => fs::canonicalize(&p)
            .with_context(|| format!("provider path not found: {}", p.display())),
        Source::Git { url, rev, branch } => {
            clone_git(name, &url, rev.as_deref(), branch.as_deref())
        }
    }
}

fn resolve_source(name: &str, provider_path: Option<&str>) -> Result<Source> {
    let key = |suffix: &str| format!("LINGXIA_PROVIDER_{}_{suffix}", name.to_uppercase());
    let env = |suffix: &str| std::env::var(key(suffix)).ok().filter(|s| !s.is_empty());
    if let Some(p) = provider_path.filter(|s| !s.is_empty()) {
        return Ok(Source::Path(PathBuf::from(p)));
    }
    if let Some(p) = env("PATH") {
        return Ok(Source::Path(PathBuf::from(p)));
    }
    if let Some(url) = env("GIT") {
        return Ok(Source::Git {
            url,
            rev: env("REV"),
            branch: env("REF"),
        });
    }
    bail!(
        "provider '{name}' requested but no source given\n  \
         pass --provider-path <dir>, or set {} (local path) / {} (git url, + {} or {})",
        key("PATH"),
        key("GIT"),
        key("REV"),
        key("REF")
    )
}

/// Clone a git provider into a cache dir so its manifest can be read and a path
/// dep can point at it. Cached by provider name; re-checks out a pinned rev.
fn clone_git(name: &str, url: &str, rev: Option<&str>, branch: Option<&str>) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("lingxia-provider").join(name);
    if !dir.join("Cargo.toml").exists() {
        let _ = fs::remove_dir_all(&dir);
        if let Some(parent) = dir.parent() {
            fs::create_dir_all(parent).ok();
        }
        let mut cmd = Command::new("git");
        cmd.arg("clone");
        if let Some(b) = branch {
            cmd.args(["--branch", b]);
        }
        cmd.arg(url).arg(&dir);
        run(cmd, "git clone provider")?;
    }
    if let Some(r) = rev {
        let mut cmd = Command::new("git");
        cmd.current_dir(&dir).args(["checkout", r]);
        run(cmd, "git checkout provider rev")?;
    }
    Ok(dir)
}

fn run(mut cmd: Command, what: &str) -> Result<()> {
    let status = cmd.status().with_context(|| format!("running {what}"))?;
    if !status.success() {
        bail!("{what} failed");
    }
    Ok(())
}

fn describe_source(provider_path: Option<&str>) -> String {
    if let Some(p) = provider_path.filter(|s| !s.is_empty()) {
        return format!("path {p}");
    }
    // Don't print git URLs (may be a private host); just say it came from env.
    "env".to_string()
}

/// Workspace members of `root` via `cargo metadata`: package name -> crate dir.
fn workspace_members(root: &Path) -> Result<BTreeMap<String, PathBuf>> {
    let meta = cargo_metadata(root)?;
    let mut members = BTreeMap::new();
    for pkg in meta["packages"].as_array().into_iter().flatten() {
        if let (Some(name), Some(manifest_path)) =
            (pkg["name"].as_str(), pkg["manifest_path"].as_str())
            && let Some(dir) = Path::new(manifest_path).parent()
        {
            members.insert(name.to_string(), dir.to_path_buf());
        }
    }
    Ok(members)
}

fn patch_native_manifest(
    dir: &Path,
    provider: &ResolvedProvider,
    guard: &mut ProviderInjection,
) -> Result<()> {
    let manifest = dir.join("Cargo.toml");
    let original =
        fs::read_to_string(&manifest).with_context(|| format!("reading {}", manifest.display()))?;
    let parsed: toml::Value =
        toml::from_str(&original).with_context(|| format!("parsing {}", manifest.display()))?;
    let existing_dependency = parsed
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .and_then(|dependencies| dependencies.get(&provider.crate_name));
    let feature_values = parsed
        .get("features")
        .and_then(toml::Value::as_table)
        .and_then(|features| features.get(&provider.feature))
        .and_then(toml::Value::as_array);
    let dependency_feature = format!("dep:{}", provider.crate_name);
    let feature_is_inert = feature_values.is_some_and(Vec::is_empty);
    let feature_is_active = feature_values.is_some_and(|values| {
        values
            .iter()
            .any(|value| value.as_str() == Some(dependency_feature.as_str()))
    });
    if !feature_is_inert && !feature_is_active {
        bail!(
            "native crate {} must declare `{} = []` or activate `{}` with `{}`",
            manifest.display(),
            provider.feature,
            provider.feature,
            dependency_feature,
        );
    }
    let dependency_is_optional = existing_dependency
        .and_then(toml::Value::as_table)
        .and_then(|dependency| dependency.get("optional"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(existing_dependency.is_none());
    let mut patched = original.clone();
    if feature_is_inert && dependency_is_optional {
        // Valid TOML may use quoted keys, comments or different spacing.
        // The guard restores the original formatting after the build.
        let mut activated = parsed.clone();
        activated["features"][&provider.feature] =
            toml::Value::Array(vec![toml::Value::String(dependency_feature)]);
        patched = toml::to_string(&activated).context("activating provider feature")?;
    }
    if !patched.ends_with('\n') {
        patched.push('\n');
    }
    if let Some(dependency) = existing_dependency {
        patched = override_dependency_source(&patched, provider, dependency)?;
    } else {
        patched.push_str(&dep_table_toml(provider)?);
    }

    guard
        .backups
        .push((manifest.clone(), original.into_bytes()));
    fs::write(&manifest, patched).with_context(|| format!("patching {}", manifest.display()))?;
    Ok(())
}

/// Point a dependency already declared by the host at the requested provider
/// checkout. This is the normal product-app case: release builds keep a pinned
/// git dependency, while `--provider-path` substitutes a local checkout.
fn override_dependency_source(
    manifest: &str,
    provider: &ResolvedProvider,
    dependency: &toml::Value,
) -> Result<String> {
    let mut spec = dependency.as_table().cloned().unwrap_or_default();
    for source_key in [
        "version",
        "registry",
        "git",
        "branch",
        "tag",
        "rev",
        "path",
        "workspace",
    ] {
        spec.remove(source_key);
    }
    spec.insert(
        "path".to_string(),
        toml::Value::String(provider.dir.to_string_lossy().into_owned()),
    );
    if !provider.features.is_empty() {
        let features = spec
            .entry("features".to_string())
            .or_insert_with(|| toml::Value::Array(Vec::new()));
        let Some(features) = features.as_array_mut() else {
            bail!(
                "dependency {} has a non-array `features` value",
                provider.crate_name
            );
        };
        for feature in &provider.features {
            if !features.iter().any(|value| value.as_str() == Some(feature)) {
                features.push(toml::Value::String(feature.clone()));
            }
        }
    }

    let mut parsed: toml::Value = toml::from_str(manifest).context("parsing native manifest")?;
    parsed["dependencies"][&provider.crate_name] = toml::Value::Table(spec);
    toml::to_string(&parsed).context("serializing provider dependency override")
}

fn dep_table_toml(provider: &ResolvedProvider) -> Result<String> {
    let feats = provider
        .features
        .iter()
        .map(|f| format!("\"{f}\""))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "\n[dependencies.{}]\npath = {}\noptional = true\nfeatures = [{feats}]\n",
        provider.crate_name,
        toml_path(&provider.dir)
    ))
}

/// Quote a path as a TOML string — a literal (single-quoted) string so Windows
/// backslashes aren't read as escapes; basic-string fallback if it has a quote.
fn toml_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.contains('\'') {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        format!("'{s}'")
    }
}

/// Workspace root if `dir` is a member of an enclosing workspace; `None` for a
/// standalone crate (one declaring its own `[workspace]`/`[patch.crates-io]`).
fn workspace_member_root(dir: &Path) -> Option<PathBuf> {
    let own = fs::read_to_string(dir.join("Cargo.toml")).unwrap_or_default();
    if own.contains("[workspace]") || own.contains("[patch.crates-io]") {
        return None;
    }
    let mut cur = dir.parent();
    while let Some(d) = cur {
        if let Ok(s) = fs::read_to_string(d.join("Cargo.toml"))
            && s.contains("[workspace]")
        {
            return Some(d.to_path_buf());
        }
        cur = d.parent();
    }
    None
}

fn patch_workspace_root(
    root: &Path,
    patches: &[(String, PathBuf)],
    guard: &mut ProviderInjection,
) -> Result<()> {
    if patches.is_empty() {
        return Ok(());
    }
    let manifest = root.join("Cargo.toml");
    let original =
        fs::read_to_string(&manifest).with_context(|| format!("reading {}", manifest.display()))?;
    let Some(idx) = original.find("[patch.crates-io]") else {
        bail!(
            "workspace root {} has no [patch.crates-io] table to extend",
            manifest.display()
        );
    };
    let after_header = original[idx..]
        .find('\n')
        .map_or(original.len(), |n| idx + n + 1);
    let existing = &original[idx..];
    let mut insert = String::new();
    for (name, path) in patches {
        // Idempotent: don't duplicate an entry the root already declares.
        if existing
            .lines()
            .any(|l| l.trim_start().starts_with(&format!("{name} =")))
        {
            continue;
        }
        insert.push_str(&format!("{name} = {{ path = {} }}\n", toml_path(path)));
    }
    if insert.is_empty() {
        return Ok(());
    }
    let mut patched = String::with_capacity(original.len() + insert.len());
    patched.push_str(&original[..after_header]);
    patched.push_str(&insert);
    patched.push_str(&original[after_header..]);

    guard
        .backups
        .push((manifest.clone(), original.into_bytes()));
    fs::write(&manifest, patched).with_context(|| format!("patching {}", manifest.display()))?;
    Ok(())
}

fn backup_lock(workspace_dir: &Path, guard: &mut ProviderInjection) {
    let lock = workspace_dir.join("Cargo.lock");
    if let Ok(content) = fs::read(&lock) {
        guard.backups.push((lock, content));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProviderInjection, ResolvedProvider, dependency_git_source, duplicated_framework_crates,
        patch_git_dependency_source, patch_native_manifest,
    };
    use serde_json::json;
    use std::fs;

    fn package(name: &str, version: &str, source: Option<&str>) -> serde_json::Value {
        json!({ "name": name, "version": version, "source": source })
    }

    #[test]
    fn accepts_a_graph_patched_entirely_to_local_paths() {
        let meta = json!({ "packages": [
            package("lingxia-platform", "0.12.0", None),
            package("lingxia-lxapp", "0.12.0", None),
            package("serde", "1.0.0", Some("registry+https://github.com/rust-lang/crates.io-index")),
        ] });
        assert!(duplicated_framework_crates(&meta).is_none());
    }

    #[test]
    fn accepts_a_graph_that_only_uses_published_crates() {
        // An external host app builds against the release, with no patch at all.
        let registry = Some("registry+https://github.com/rust-lang/crates.io-index");
        let meta = json!({ "packages": [
            package("lingxia-platform", "0.12.0", registry),
            package("lingxia-lxapp", "0.12.0", registry),
        ] });
        assert!(duplicated_framework_crates(&meta).is_none());
    }

    #[test]
    fn reports_the_crate_cargo_took_from_the_registry_beside_the_local_one() {
        let registry = Some("registry+https://github.com/rust-lang/crates.io-index");
        let meta = json!({ "packages": [
            package("lingxia-platform", "0.12.0", None),
            package("lingxia-platform", "0.11.1", registry),
            package("lingxia-lxapp", "0.12.0", None),
            package("lingxia-lxapp", "0.11.1", registry),
        ] });
        let message = duplicated_framework_crates(&meta)
            .expect("a duplicated framework crate must fail the build")
            .to_string();
        assert!(message.contains("lingxia-platform: 0.11.1 from crates.io, 0.12.0 local"));
        assert!(message.contains("lingxia-lxapp: 0.11.1 from crates.io, 0.12.0 local"));
    }

    use super::toml_path;
    use std::path::Path;

    /// A Windows-style path must produce a TOML value that parses back to the same
    /// string — the old basic-string form turned `\s`/`\c` into invalid escapes.
    #[test]
    fn windows_path_round_trips_through_toml() {
        let rendered = format!("path = {}", toml_path(Path::new(r"C:\src\cloud")));
        let value: toml::Value = toml::from_str(&rendered).expect("valid TOML");
        assert_eq!(value["path"].as_str(), Some(r"C:\src\cloud"));
    }

    /// A path containing a single quote can't be a literal string, so it falls
    /// back to an escaped basic string — still round-trips.
    #[test]
    fn path_with_single_quote_falls_back_to_basic_string() {
        let weird = r"/tmp/it's \here";
        let rendered = format!("path = {}", toml_path(Path::new(weird)));
        let value: toml::Value = toml::from_str(&rendered).expect("valid TOML");
        assert_eq!(value["path"].as_str(), Some(weird));
    }

    #[test]
    fn local_provider_accepts_quoted_keys_and_commented_features() {
        let temp = tempfile::tempdir().unwrap();
        let manifest = temp.path().join("Cargo.toml");
        let original = r#"[package]
name = "host"
version = "0.1.0"
[features]
"cloud"=[] # opt in during the build
[dependencies] # private providers
"lingxia-cloud-client" = { path = "../old", optional = true }
[patch.'https://example.invalid/cloud.git'.lingxia-cloud-client]
path = "../old"
"#;
        fs::write(&manifest, original).unwrap();
        let provider = ResolvedProvider {
            feature: "cloud".into(),
            crate_name: "lingxia-cloud-client".into(),
            dir: temp.path().join("cloud"),
            deps: Vec::new(),
            features: vec!["dev".into()],
        };
        let mut guard = ProviderInjection {
            backups: Vec::new(),
            features: Vec::new(),
        };
        patch_native_manifest(temp.path(), &provider, &mut guard).unwrap();
        patch_git_dependency_source(
            temp.path(),
            "https://example.invalid/cloud.git",
            &provider,
            &mut guard,
        )
        .unwrap();
        let parsed: toml::Value = toml::from_str(&fs::read_to_string(&manifest).unwrap()).unwrap();
        assert_eq!(
            parsed["features"]["cloud"][0].as_str(),
            Some("dep:lingxia-cloud-client")
        );
        assert_eq!(
            parsed["dependencies"]["lingxia-cloud-client"]["path"].as_str(),
            provider.dir.to_str()
        );
        assert_eq!(
            parsed["patch"]["https://example.invalid/cloud.git"]["lingxia-cloud-client"]["path"]
                .as_str(),
            provider.dir.to_str()
        );
        drop(guard);
        assert_eq!(fs::read_to_string(manifest).unwrap(), original);
    }

    #[test]
    fn local_provider_overrides_an_existing_git_dependency() {
        let temp = tempfile::tempdir().expect("temp dir");
        let native = temp.path().join("native");
        let provider_dir = temp.path().join("cloud");
        fs::create_dir_all(&native).expect("native dir");
        fs::create_dir_all(&provider_dir).expect("provider dir");
        let manifest = native.join("Cargo.toml");
        let original = r#"[package]
name = "host"
version = "0.1.0"

[features]
cloud = []

[dependencies]
lingxia-cloud-client = { git = "https://example.invalid/cloud.git", rev = "abc", default-features = false, features = ["cloud"] }
"#;
        fs::write(&manifest, original).expect("write manifest");
        let provider = ResolvedProvider {
            feature: "cloud".to_string(),
            crate_name: "lingxia-cloud-client".to_string(),
            dir: provider_dir.clone(),
            deps: Vec::new(),
            features: vec!["dev".to_string()],
        };
        let mut guard = ProviderInjection {
            backups: Vec::new(),
            features: Vec::new(),
        };

        let git_source = dependency_git_source(&native, &provider.crate_name)
            .expect("read dependency source")
            .expect("git dependency");
        patch_native_manifest(&native, &provider, &mut guard).expect("patch manifest");
        patch_git_dependency_source(&native, &git_source, &provider, &mut guard)
            .expect("patch git source");
        let patched = fs::read_to_string(&manifest).expect("read patched manifest");
        let parsed: toml::Value = toml::from_str(&patched).expect("valid patched TOML");
        let dependency = parsed["dependencies"]["lingxia-cloud-client"]
            .as_table()
            .expect("dependency table");
        assert_eq!(
            dependency.get("path").and_then(toml::Value::as_str),
            provider_dir.to_str()
        );
        assert!(dependency.get("git").is_none());
        assert!(dependency.get("rev").is_none());
        assert_eq!(
            dependency.get("features").and_then(toml::Value::as_array),
            Some(&vec![
                toml::Value::String("cloud".to_string()),
                toml::Value::String("dev".to_string())
            ])
        );
        assert!(patched.contains("cloud = []"));
        assert!(!patched.contains("dep:lingxia-cloud-client"));
        assert_eq!(
            parsed["patch"]["https://example.invalid/cloud.git"]["lingxia-cloud-client"]["path"]
                .as_str(),
            provider_dir.to_str()
        );

        drop(guard);
        assert_eq!(
            fs::read_to_string(manifest).expect("restored manifest"),
            original
        );
    }

    #[test]
    fn local_provider_accepts_an_already_active_dependency() {
        let temp = tempfile::tempdir().expect("temp dir");
        let native = temp.path().join("native");
        let provider_dir = temp.path().join("cloud");
        fs::create_dir_all(&native).expect("native dir");
        fs::create_dir_all(&provider_dir).expect("provider dir");
        let manifest = native.join("Cargo.toml");
        let original = r#"[package]
name = "host"
version = "0.1.0"

[features]
cloud = ["dep:lingxia-cloud-client"]

[dependencies.lingxia-cloud-client]
path = "../checked-in-cloud"
optional = true
features = []
"#;
        fs::write(&manifest, original).expect("write manifest");
        let provider = ResolvedProvider {
            feature: "cloud".to_string(),
            crate_name: "lingxia-cloud-client".to_string(),
            dir: provider_dir.clone(),
            deps: Vec::new(),
            features: vec!["dev".to_string()],
        };
        let mut guard = ProviderInjection {
            backups: Vec::new(),
            features: Vec::new(),
        };

        patch_native_manifest(&native, &provider, &mut guard).expect("patch manifest");
        let patched = fs::read_to_string(&manifest).expect("read patched manifest");
        let parsed: toml::Value = toml::from_str(&patched).expect("valid patched TOML");
        assert_eq!(
            parsed["features"]["cloud"].as_array(),
            Some(&vec![toml::Value::String(
                "dep:lingxia-cloud-client".to_string()
            )])
        );
        let dependency = parsed["dependencies"]["lingxia-cloud-client"]
            .as_table()
            .expect("dependency table");
        assert_eq!(
            dependency.get("path").and_then(toml::Value::as_str),
            provider_dir.to_str()
        );
        assert_eq!(
            dependency.get("features").and_then(toml::Value::as_array),
            Some(&vec![toml::Value::String("dev".to_string())])
        );

        drop(guard);
        assert_eq!(fs::read_to_string(manifest).expect("restored"), original);
    }

    #[test]
    fn local_provider_replaces_an_existing_patch_entry() {
        let temp = tempfile::tempdir().expect("temp dir");
        let provider_dir = temp.path().join("new-cloud");
        fs::create_dir_all(&provider_dir).expect("provider dir");
        let manifest = temp.path().join("Cargo.toml");
        let original = r#"[workspace]

[patch."https://example.invalid/cloud.git"]
lingxia-cloud-client = { path = "../old-cloud" }
other = { path = "../other" }
"#;
        fs::write(&manifest, original).expect("write manifest");
        let provider = ResolvedProvider {
            feature: "cloud".to_string(),
            crate_name: "lingxia-cloud-client".to_string(),
            dir: provider_dir.clone(),
            deps: Vec::new(),
            features: Vec::new(),
        };
        let mut guard = ProviderInjection {
            backups: Vec::new(),
            features: Vec::new(),
        };

        patch_git_dependency_source(
            temp.path(),
            "https://example.invalid/cloud.git",
            &provider,
            &mut guard,
        )
        .expect("replace patch");

        let patched = fs::read_to_string(&manifest).expect("read patched manifest");
        let parsed: toml::Value = toml::from_str(&patched).expect("valid patched TOML");
        assert_eq!(
            parsed["patch"]["https://example.invalid/cloud.git"]["lingxia-cloud-client"]["path"]
                .as_str(),
            provider_dir.to_str()
        );
        assert_eq!(
            parsed["patch"]["https://example.invalid/cloud.git"]["other"]["path"].as_str(),
            Some("../other")
        );

        drop(guard);
        assert_eq!(fs::read_to_string(manifest).expect("restored"), original);
    }
}
