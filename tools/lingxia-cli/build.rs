use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use toml::{Table as TomlTable, Value as TomlValue};

const WINDOWS_DESIGN_ICON_PNG_SIZE: u32 = 64;

#[derive(Debug)]
struct ComponentVersions {
    bridge: String,
    polyfills: String,
    rong: String,
    rust_crate: String,
    sdk: String,
}

fn main() {
    if let Err(err) = run() {
        panic!("failed to prepare embedded bridge runtime: {err}");
    }
}

fn run() -> Result<(), String> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(|e| e.to_string())?);
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "failed to resolve repo root".to_string())?;
    let bridge_dir = repo_root.join("packages").join("lingxia-bridge");
    let polyfills_dir = repo_root.join("packages").join("lingxia-polyfills");
    let bridge_package_json = bridge_dir.join("package.json");
    let polyfills_package_json = polyfills_dir.join("package.json");
    let component_versions = read_component_versions(
        &manifest_dir.join("Cargo.toml"),
        &repo_root.join("Cargo.toml"),
    )?;

    emit_rerun_markers(&manifest_dir, repo_root, &bridge_dir, &polyfills_dir)?;
    emit_component_version_env(&component_versions);
    emit_build_metadata_env(repo_root);

    let actual_bridge_version = read_npm_package_version(&bridge_package_json)?;
    if actual_bridge_version != component_versions.bridge {
        return Err(format!(
            "configured @lingxia/bridge version {} does not match package.json version {}",
            component_versions.bridge, actual_bridge_version
        ));
    }
    let actual_polyfills_version = read_npm_package_version(&polyfills_package_json)?;
    if actual_polyfills_version != component_versions.polyfills {
        return Err(format!(
            "configured @lingxia/polyfills version {} does not match package.json version {}",
            component_versions.polyfills, actual_polyfills_version
        ));
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|e| e.to_string())?);
    let cache_root = embedded_js_cache_root(&out_dir).join("lingxia-embedded-js");
    let workspace_lock = repo_root.join("packages").join("package-lock.json");
    prepare_embedded_js(
        &EmbeddedJsPackage {
            name: "lingxia-bridge",
            dir: &bridge_dir,
            outputs: &["bridge-runtime.es2020.js", "bridge-runtime.es5.js"],
            bins: &["rolldown", "tsc"],
        },
        &workspace_lock,
        &cache_root,
        &out_dir,
    )?;
    prepare_embedded_js(
        &EmbeddedJsPackage {
            name: "lingxia-polyfills",
            dir: &polyfills_dir,
            outputs: &["polyfills.es5.js"],
            bins: &["terser"],
        },
        &workspace_lock,
        &cache_root,
        &out_dir,
    )?;
    let es2020_out = out_dir.join("bridge-runtime.es2020.js");
    let es5_out = out_dir.join("bridge-runtime.es5.js");
    let polyfills_out = out_dir.join("polyfills.es5.js");
    generate_windows_design_icons(repo_root, &out_dir)?;

    println!(
        "cargo:rustc-env=LINGXIA_BRIDGE_RUNTIME_ES2020={}",
        es2020_out.display()
    );
    println!(
        "cargo:rustc-env=LINGXIA_BRIDGE_RUNTIME_ES5={}",
        es5_out.display()
    );
    println!(
        "cargo:rustc-env=LINGXIA_POLYFILLS_ES5={}",
        polyfills_out.display()
    );

    Ok(())
}

fn generate_windows_design_icons(repo_root: &Path, out_dir: &Path) -> Result<(), String> {
    let svg_dir = repo_root.join("design").join("icons").join("svg");
    let png_dir = out_dir.join("windows-design-icons");
    fs::create_dir_all(&png_dir)
        .map_err(|e| format!("failed to create {}: {e}", png_dir.display()))?;

    let mut svg_paths = Vec::new();
    for entry in
        fs::read_dir(&svg_dir).map_err(|e| format!("failed to read {}: {e}", svg_dir.display()))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
        {
            svg_paths.push(path);
        }
    }
    svg_paths.sort();

    let generated_rs = out_dir.join("windows-design-icons.rs");
    let mut rust =
        String::from("pub(crate) static WINDOWS_DESIGN_ICONS: &[(&str, &str, &[u8])] = &[\n");
    for svg_path in svg_paths {
        let stem = svg_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("invalid SVG icon name: {}", svg_path.display()))?;
        let svg = fs::read_to_string(&svg_path)
            .map_err(|e| format!("failed to read {}: {e}", svg_path.display()))?;
        let png = svg_to_png_bytes(&svg, WINDOWS_DESIGN_ICON_PNG_SIZE)
            .map_err(|e| format!("failed to convert {} to PNG: {e}", svg_path.display()))?;
        let png_path = png_dir.join(format!("{stem}.png"));
        fs::write(&png_path, png)
            .map_err(|e| format!("failed to write {}: {e}", png_path.display()))?;
        let source_path = format!("design/icons/svg/{stem}.svg");
        let relative_path = format!("icons/design/{stem}.png");
        rust.push_str("    (");
        rust.push_str(&rust_string_literal(&relative_path));
        rust.push_str(", ");
        rust.push_str(&rust_string_literal(&source_path));
        rust.push_str(", include_bytes!(");
        rust.push_str(&rust_string_literal(&png_path.to_string_lossy()));
        rust.push_str(")),\n");
    }
    rust.push_str("];\n");
    write_if_changed(&generated_rs, rust.as_bytes())
        .map_err(|e| format!("failed to write {}: {e}", generated_rs.display()))?;
    Ok(())
}

fn svg_to_png_bytes(svg_content: &str, target_size: u32) -> Result<Vec<u8>, String> {
    let tree = usvg::Tree::from_str(svg_content, &usvg::Options::default())
        .map_err(|e| format!("failed to parse SVG: {e}"))?;
    let source_size = tree.size();
    let max_side = source_size.width().max(source_size.height());
    if max_side <= 0.0 {
        return Err("SVG has an empty viewport".to_string());
    }

    let scale = target_size as f32 / max_side;
    let offset_x = (target_size as f32 - source_size.width() * scale) / 2.0;
    let offset_y = (target_size as f32 - source_size.height() * scale) / 2.0;
    let mut pixmap = tiny_skia::Pixmap::new(target_size, target_size)
        .ok_or_else(|| "failed to allocate icon pixmap".to_string())?;
    let transform = tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, offset_x, offset_y);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap
        .encode_png()
        .map_err(|e| format!("failed to encode rendered SVG as PNG: {e}"))
}

fn rust_string_literal(value: &str) -> String {
    format!("{value:?}")
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if fs::read(path).ok().as_deref() == Some(bytes) {
        return Ok(());
    }
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)
}

fn read_component_versions(
    manifest: &Path,
    workspace_manifest: &Path,
) -> Result<ComponentVersions, String> {
    let content = fs::read_to_string(manifest)
        .map_err(|e| format!("failed to read {}: {e}", manifest.display()))?;
    let value: TomlTable = content
        .parse()
        .map_err(|e| format!("failed to parse {}: {e}", manifest.display()))?;
    let table = value
        .get("package")
        .and_then(|value| value.get("metadata"))
        .and_then(|value| value.get("lingxia"))
        .and_then(TomlValue::as_table)
        .ok_or_else(|| {
            format!(
                "missing [package.metadata.lingxia] in {}",
                manifest.display()
            )
        })?;

    let get = |key: &str| -> Result<String, String> {
        table
            .get(key)
            .and_then(TomlValue::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                format!(
                    "missing non-empty package.metadata.lingxia.{key} in {}",
                    manifest.display()
                )
            })
    };

    Ok(ComponentVersions {
        bridge: get("bridge-version")?,
        polyfills: get("polyfills-version")?,
        // Read from the workspace rather than a metadata key beside it: rong is
        // a third-party version this release does not set, so a copy here is a
        // mirror that nothing updates when the dependency moves.
        rong: read_workspace_dependency_version(workspace_manifest, "rong")?,
        rust_crate: get("rust-crate-version")?,
        sdk: get("sdk-version")?,
    })
}

/// The version the workspace resolves for a `[workspace.dependencies]` entry,
/// whether it is written as a bare string or a table with a `version` key.
fn read_workspace_dependency_version(manifest: &Path, name: &str) -> Result<String, String> {
    let content = fs::read_to_string(manifest)
        .map_err(|e| format!("failed to read {}: {e}", manifest.display()))?;
    let value: TomlTable = content
        .parse()
        .map_err(|e| format!("failed to parse {}: {e}", manifest.display()))?;
    let entry = value
        .get("workspace")
        .and_then(|value| value.get("dependencies"))
        .and_then(|value| value.get(name))
        .ok_or_else(|| {
            format!(
                "missing [workspace.dependencies].{name} in {}",
                manifest.display()
            )
        })?;
    let version = match entry {
        TomlValue::String(version) => Some(version.as_str()),
        other => other.get("version").and_then(TomlValue::as_str),
    };
    version
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "[workspace.dependencies].{name} in {} has no version",
                manifest.display()
            )
        })
}

fn emit_component_version_env(versions: &ComponentVersions) {
    println!("cargo:rustc-env=LINGXIA_BRIDGE_VERSION={}", versions.bridge);
    println!(
        "cargo:rustc-env=LINGXIA_POLYFILLS_VERSION={}",
        versions.polyfills
    );
    println!("cargo:rustc-env=LINGXIA_RONG_VERSION={}", versions.rong);
    println!(
        "cargo:rustc-env=LINGXIA_RUST_CRATE_VERSION={}",
        versions.rust_crate
    );
    println!("cargo:rustc-env=LINGXIA_SDK_VERSION={}", versions.sdk);
}

fn emit_build_metadata_env(repo_root: &Path) {
    println!(
        "cargo:rustc-env=LINGXIA_BUILD_HOST={}",
        env::var("HOST").unwrap_or_else(|_| "unknown".to_string())
    );
    println!(
        "cargo:rustc-env=LINGXIA_COMMIT_HASH={}",
        git_output(repo_root, &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_string())
    );
    println!(
        "cargo:rustc-env=LINGXIA_COMMIT_DATE={}",
        git_output(repo_root, &["show", "-s", "--format=%cs", "HEAD"])
            .unwrap_or_else(|| "unknown".to_string())
    );
    // Set by the release pipeline: this build's Runner is its version's
    // release asset (see `runner_cache::CliBuild`).
    println!("cargo:rerun-if-env-changed=LINGXIA_RELEASE_BUILD");
    println!(
        "cargo:rustc-env=LINGXIA_RELEASE_BUILD={}",
        match env::var("LINGXIA_RELEASE_BUILD") {
            Ok(value) if !value.is_empty() && value != "0" => "1",
            _ => "",
        }
    );
    let dirty = git_tree_dirty(repo_root);
    println!(
        "cargo:rustc-env=LINGXIA_COMMIT_DIRTY={}",
        if dirty { "1" } else { "" }
    );
    // `lingxia --version` / `lingxia version`: the release version alone cannot
    // tell two builds apart, so stamp the commit (and `-dirty`) it came from.
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let stamp = match (
        git_output(repo_root, &["rev-parse", "--short=9", "HEAD"]),
        git_output(repo_root, &["show", "-s", "--format=%cs", "HEAD"]),
    ) {
        (Some(hash), Some(date)) => {
            format!(
                "{version} ({hash}{} {date})",
                if dirty { "-dirty" } else { "" }
            )
        }
        _ => version,
    };
    println!("cargo:rustc-env=LINGXIA_BUILD_VERSION={stamp}");
}

fn git_tree_dirty(repo_root: &Path) -> bool {
    Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .current_dir(repo_root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .is_some_and(|output| !output.stdout.iter().all(u8::is_ascii_whitespace))
}

fn git_output(repo_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn run_npm_build(package_dir: &Path) -> Result<(), String> {
    let status = Command::new(npm_command())
        .arg("run")
        .arg("build")
        .current_dir(package_dir)
        .status()
        .map_err(|e| {
            format!(
                "failed to start npm run build in {}: {e}",
                package_dir.display()
            )
        })?;
    if !status.success() {
        return Err(format!(
            "npm run build failed in {} with status {}",
            package_dir.display(),
            status
        ));
    }
    Ok(())
}

fn ensure_npm_available() -> Result<(), String> {
    match Command::new(npm_command()).arg("--version").status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!(
            "npm is required to build the embedded @lingxia/bridge runtime, but `npm --version` exited with status {}.\n\
Install Node.js/npm, then retry `cargo build -p lingxia-cli`.",
            status
        )),
        Err(err) => Err(format!(
            "npm is required to build the embedded @lingxia/bridge runtime, but it is not available: {err}\n\
Install Node.js/npm, then retry `cargo build -p lingxia-cli`."
        )),
    }
}

fn npm_command() -> &'static str {
    if cfg!(windows) { "npm.cmd" } else { "npm" }
}

fn ensure_npm_bin_installed(package_dir: &Path, bin_name: &str) -> Result<(), String> {
    let bin_leaf = if cfg!(windows) {
        format!("{bin_name}.cmd")
    } else {
        bin_name.to_string()
    };

    // The bin may live in the package's own node_modules (a per-package install)
    // or be hoisted to the npm-workspace root, so search upward for either.
    let mut dir = Some(package_dir);
    while let Some(current) = dir {
        if current
            .join("node_modules")
            .join(".bin")
            .join(&bin_leaf)
            .is_file()
        {
            return Ok(());
        }
        dir = current.parent();
    }

    Err(format!(
        "npm build tooling (`{bin_name}`) is not installed.\n\
Run `npm install` in the `packages/` workspace (it links the in-repo @lingxia/* packages \
and hoists their dev tooling), then retry `cargo build -p lingxia-cli`.",
    ))
}

fn read_npm_package_version(package_json: &Path) -> Result<String, String> {
    let content = fs::read_to_string(package_json)
        .map_err(|e| format!("failed to read {}: {e}", package_json.display()))?;
    let value: JsonValue = serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {e}", package_json.display()))?;
    value
        .get("version")
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("missing version in {}", package_json.display()))
}

fn emit_rerun_markers(
    manifest_dir: &Path,
    repo_root: &Path,
    bridge_dir: &Path,
    polyfills_dir: &Path,
) -> Result<(), String> {
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("build.rs").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("Cargo.toml").display()
    );
    emit_rerun_for_dir(&manifest_dir.join("templates"))?;
    emit_rerun_for_dir(&repo_root.join("design").join("icons").join("svg"))?;
    // The agent skill is embedded with include_dir!, which expands to one
    // include_bytes! per file. Editing a file retriggers on its own, but adding
    // or removing one does not re-expand the macro -- the binary would keep
    // shipping the previous file list.
    emit_rerun_for_dir(&repo_root.join("docs").join("skill"))?;
    // A worktree's `.git` is a file, so ask git where HEAD, the branch ref and
    // the index live. Source directories are watched so an unstaged edit
    // refreshes the `-dirty` stamp in the version line.
    for name in ["HEAD", "index", "packed-refs"] {
        if let Some(path) = git_output(repo_root, &["rev-parse", "--git-path", name]) {
            println!("cargo:rerun-if-changed={}", repo_root.join(path).display());
        }
    }
    if let Some(reference) = git_output(repo_root, &["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git_output(repo_root, &["rev-parse", "--git-path", &reference])
    {
        println!("cargo:rerun-if-changed={}", repo_root.join(path).display());
    }
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        repo_root.join("crates").display()
    );

    println!(
        "cargo:rerun-if-changed={}",
        repo_root
            .join("packages")
            .join("package-lock.json")
            .display()
    );
    for package_dir in [bridge_dir, polyfills_dir] {
        for name in NPM_PACKAGE_CONFIG_FILES {
            let path = package_dir.join(name);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
        // Watching the directories themselves catches files being added or removed.
        for sub in ["src", "scripts"] {
            let dir = package_dir.join(sub);
            if dir.exists() {
                println!("cargo:rerun-if-changed={}", dir.display());
            }
            emit_rerun_for_dir(&dir)?;
        }
    }
    Ok(())
}

fn emit_rerun_for_dir(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|e| format!("failed to read {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            emit_rerun_for_dir(&path)?;
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    Ok(())
}

/// Config files that, besides `src/` and `scripts/`, determine a package's build output.
const NPM_PACKAGE_CONFIG_FILES: &[&str] = &[
    "package.json",
    "package-lock.json",
    "rolldown.config.js",
    "tsconfig.json",
    "tsconfig.modules.json",
    "tsconfig.modules.legacy.json",
];

struct EmbeddedJsPackage<'a> {
    name: &'a str,
    dir: &'a Path,
    outputs: &'a [&'a str],
    bins: &'a [&'a str],
}

/// Copies a package's dist outputs into OUT_DIR, from the content-addressed cache when it
/// holds the current input hash, otherwise via `npm run build` (which then fills the cache).
fn prepare_embedded_js(
    package: &EmbeddedJsPackage,
    workspace_lock: &Path,
    cache_root: &Path,
    out_dir: &Path,
) -> Result<(), String> {
    let hash = hash_npm_package_inputs(package.dir, workspace_lock)?;
    let package_cache = cache_root.join(package.name);
    let entry = package_cache.join(&hash);
    let source_dir = if package
        .outputs
        .iter()
        .all(|name| entry.join(name).is_file())
    {
        entry
    } else {
        ensure_npm_available()?;
        for bin in package.bins {
            ensure_npm_bin_installed(package.dir, bin)?;
        }
        run_npm_build(package.dir)?;
        let dist = package.dir.join("dist");
        for name in package.outputs {
            let file = dist.join(name);
            if !file.is_file() {
                return Err(format!("missing runtime asset: {}", file.display()));
            }
        }
        // The cache is only an accelerator; a failed store must not fail the build.
        if let Err(err) = store_in_cache(&dist, package.outputs, &package_cache, &hash) {
            println!(
                "cargo:warning=failed to cache {} build output: {err}",
                package.name
            );
        }
        dist
    };
    for name in package.outputs {
        let src = source_dir.join(name);
        fs::copy(&src, out_dir.join(name))
            .map_err(|e| format!("failed to copy {}: {e}", src.display()))?;
    }
    Ok(())
}

fn store_in_cache(
    dist: &Path,
    outputs: &[&str],
    package_cache: &Path,
    hash: &str,
) -> Result<(), String> {
    let entry = package_cache.join(hash);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let temp = package_cache.join(format!(".tmp-{hash}-{}-{nanos}", std::process::id()));
    let result = (|| {
        fs::create_dir_all(&temp).map_err(|e| format!("create {}: {e}", temp.display()))?;
        for name in outputs {
            fs::copy(dist.join(name), temp.join(name))
                .map_err(|e| format!("copy {name} into {}: {e}", temp.display()))?;
        }
        if entry.is_dir() {
            let _ = fs::remove_dir_all(&entry);
        }
        match fs::rename(&temp, &entry) {
            Ok(()) => Ok(()),
            // A concurrent build stored the same hash first.
            Err(_) if outputs.iter().all(|name| entry.join(name).is_file()) => Ok(()),
            Err(e) => Err(format!("rename into {}: {e}", entry.display())),
        }
    })();
    let _ = fs::remove_dir_all(&temp);
    result
}

/// The cargo target root, shared by every worktree that builds into it.
fn embedded_js_cache_root(out_dir: &Path) -> PathBuf {
    if let Some(dir) = env::var_os("CARGO_TARGET_DIR").map(PathBuf::from)
        && dir.is_absolute()
    {
        return dir;
    }
    // Cargo tags both `target/` and a cross build's `target/<triple>/`; prefer the outer one.
    if let Some(tagged) = out_dir
        .ancestors()
        .find(|dir| dir.join("CACHEDIR.TAG").is_file())
    {
        return match tagged.parent() {
            Some(parent) if parent.join("CACHEDIR.TAG").is_file() => parent.to_path_buf(),
            _ => tagged.to_path_buf(),
        };
    }
    // OUT_DIR is `<target>/<profile>/build/<pkg-hash>/out`.
    out_dir.ancestors().nth(4).unwrap_or(out_dir).to_path_buf()
}

fn hash_npm_package_inputs(package_dir: &Path, workspace_lock: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    for sub in ["src", "scripts"] {
        collect_files(&package_dir.join(sub), &mut files)?;
    }
    for name in NPM_PACKAGE_CONFIG_FILES {
        let path = package_dir.join(name);
        if path.is_file() {
            files.push(path);
        }
    }
    let mut entries = files
        .into_iter()
        .map(|path| {
            let rel = path
                .strip_prefix(package_dir)
                .map_err(|e| e.to_string())?
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            Ok((rel, path))
        })
        .collect::<Result<Vec<_>, String>>()?;
    entries.sort();
    if workspace_lock.is_file() {
        entries.push((
            "<workspace>/package-lock.json".to_string(),
            workspace_lock.to_path_buf(),
        ));
    }

    let mut hasher = Sha256::new();
    hasher.update(b"lingxia-embedded-js-v1\0");
    for (rel, path) in entries {
        let bytes =
            fs::read(&path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        // Length-prefix both fields so distinct file sets can never hash alike.
        hasher.update((rel.len() as u64).to_le_bytes());
        hasher.update(rel.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|e| format!("failed to read {}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            collect_files(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}
