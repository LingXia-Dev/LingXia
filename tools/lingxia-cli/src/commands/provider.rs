//! Build-time injection of an optional private provider crate into a host's
//! native crate. The crate is referenced only while a `--with-provider` build
//! runs, then the manifest + lockfile are restored — so the committed tree stays
//! self-contained and no provider source is baked in.
//!
//! The CLI hardcodes nothing about any specific provider: the injected crate's
//! name and its workspace-shared deps are read from the provider crate itself,
//! and the cargo features to enable are declared by the *host* crate. Per
//! provider `<NAME>` (which is also the inert host feature `<NAME> = []` to
//! activate):
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
        for (path, original) in self.backups.drain(..) {
            if let Err(err) = fs::write(&path, &original) {
                eprintln!("⚠ provider: failed to restore {}: {err}", path.display());
            }
        }
    }
}

/// A provider resolved entirely from its source crate + the host manifest.
struct ResolvedProvider {
    /// Inert host feature to activate; equal to the `--with-provider` name.
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
        guard.features.push(resolved.feature.clone());
        patch_native_manifest(rust_lib_dir, &resolved, &mut guard)?;
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
    println!(
        "  \u{2022} Provider(s): {} (source: {})",
        with_provider.join(", "),
        describe_source(provider_path)
    );
    Ok(Some(guard))
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
    if original.contains(&format!("[dependencies.{}]", provider.crate_name)) {
        bail!(
            "{} looks already-injected (stale state); run `git checkout -- {}`",
            manifest.display(),
            manifest.display()
        );
    }
    let inert = format!("{} = []", provider.feature);
    if !original.lines().any(|l| l.trim() == inert) {
        bail!(
            "native crate {} has no inert `{}` feature to activate",
            manifest.display(),
            inert
        );
    }
    let activated_line = format!("{} = [\"dep:{}\"]", provider.feature, provider.crate_name);
    let mut patched = original
        .lines()
        .map(|l| {
            if l.trim() == inert {
                activated_line.clone()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if !patched.ends_with('\n') {
        patched.push('\n');
    }
    patched.push_str(&dep_table_toml(provider)?);

    guard
        .backups
        .push((manifest.clone(), original.into_bytes()));
    fs::write(&manifest, patched).with_context(|| format!("patching {}", manifest.display()))?;
    Ok(())
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
}
