use serde_json::Value as JsonValue;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;
use toml::{Table as TomlTable, Value as TomlValue};

const WINDOWS_DESIGN_ICON_PNG_SIZE: u32 = 64;

#[derive(Debug)]
struct ComponentVersions {
    bridge: String,
    polyfills: String,
    types: String,
    rong: String,
    rust_crate: String,
    sdk: String,
    browser_shell_webui: String,
    resource_bundle: String,
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
    let component_versions = read_component_versions(&manifest_dir.join("Cargo.toml"))?;

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

    let es2020_src = bridge_dir.join("dist").join("bridge-runtime.es2020.js");
    let es5_src = bridge_dir.join("dist").join("bridge-runtime.es5.js");
    if should_rebuild_npm_package(&bridge_dir, &[&es2020_src, &es5_src])? {
        ensure_npm_available()?;
        ensure_npm_bin_installed(&bridge_dir, "rolldown")?;
        ensure_npm_bin_installed(&bridge_dir, "tsc")?;
        run_npm_build(&bridge_dir)?;
    }
    let polyfills_src = polyfills_dir.join("dist").join("polyfills.es5.js");
    if should_rebuild_npm_package(&polyfills_dir, &[&polyfills_src])? {
        ensure_npm_available()?;
        ensure_npm_bin_installed(&polyfills_dir, "terser")?;
        run_npm_build(&polyfills_dir)?;
    }

    for file in [&es2020_src, &es5_src, &polyfills_src] {
        if !file.is_file() {
            return Err(format!("missing runtime asset: {}", file.display()));
        }
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|e| e.to_string())?);
    let es2020_out = out_dir.join("bridge-runtime.es2020.js");
    let es5_out = out_dir.join("bridge-runtime.es5.js");
    let polyfills_out = out_dir.join("polyfills.es5.js");

    fs::copy(&es2020_src, &es2020_out)
        .map_err(|e| format!("failed to copy {}: {e}", es2020_src.display()))?;
    fs::copy(&es5_src, &es5_out)
        .map_err(|e| format!("failed to copy {}: {e}", es5_src.display()))?;
    fs::copy(&polyfills_src, &polyfills_out)
        .map_err(|e| format!("failed to copy {}: {e}", polyfills_src.display()))?;
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

fn read_component_versions(manifest: &Path) -> Result<ComponentVersions, String> {
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
        types: get("types-version")?,
        rong: get("rong-version")?,
        rust_crate: get("rust-crate-version")?,
        sdk: get("sdk-version")?,
        browser_shell_webui: get("browser-shell-webui-version")?,
        resource_bundle: get("resource-bundle-version")?,
    })
}

fn emit_component_version_env(versions: &ComponentVersions) {
    println!("cargo:rustc-env=LINGXIA_BRIDGE_VERSION={}", versions.bridge);
    println!(
        "cargo:rustc-env=LINGXIA_POLYFILLS_VERSION={}",
        versions.polyfills
    );
    println!("cargo:rustc-env=LINGXIA_TYPES_VERSION={}", versions.types);
    println!("cargo:rustc-env=LINGXIA_RONG_VERSION={}", versions.rong);
    println!(
        "cargo:rustc-env=LINGXIA_RUST_CRATE_VERSION={}",
        versions.rust_crate
    );
    println!("cargo:rustc-env=LINGXIA_SDK_VERSION={}", versions.sdk);
    println!(
        "cargo:rustc-env=LINGXIA_BROWSER_SHELL_WEBUI_VERSION={}",
        versions.browser_shell_webui
    );
    println!(
        "cargo:rustc-env=LINGXIA_RESOURCE_BUNDLE_VERSION={}",
        versions.resource_bundle
    );
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
    let git_head = repo_root.join(".git").join("HEAD");
    if git_head.exists() {
        println!("cargo:rerun-if-changed={}", git_head.display());
        if let Ok(head) = fs::read_to_string(&git_head)
            && let Some(reference) = head.trim().strip_prefix("ref: ")
        {
            let git_ref = repo_root.join(".git").join(reference);
            if git_ref.exists() {
                println!("cargo:rerun-if-changed={}", git_ref.display());
            }
        }
    }

    for path in [
        bridge_dir.join("package.json"),
        bridge_dir.join("package-lock.json"),
        bridge_dir.join("rolldown.config.js"),
        bridge_dir.join("tsconfig.json"),
        bridge_dir.join("tsconfig.modules.json"),
        bridge_dir.join("tsconfig.modules.legacy.json"),
    ] {
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    emit_rerun_for_dir(&bridge_dir.join("src"))?;
    emit_rerun_for_dir(&bridge_dir.join("scripts"))?;

    for path in [
        polyfills_dir.join("package.json"),
        polyfills_dir.join("package-lock.json"),
    ] {
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    emit_rerun_for_dir(&polyfills_dir.join("src"))?;
    emit_rerun_for_dir(&polyfills_dir.join("scripts"))?;
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

fn should_rebuild_npm_package(package_dir: &Path, outputs: &[&Path]) -> Result<bool, String> {
    if outputs.iter().any(|path| !path.is_file()) {
        return Ok(true);
    }

    let latest_input = latest_modified(package_dir.join("src"))?
        .max(latest_modified(package_dir.join("scripts"))?)
        .max(file_mtime(&package_dir.join("package.json"))?)
        .max(optional_file_mtime(&package_dir.join("package-lock.json"))?)
        .max(optional_file_mtime(
            &package_dir.join("rolldown.config.js"),
        )?)
        .max(optional_file_mtime(
            &package_dir.join("tsconfig.modules.json"),
        )?)
        .max(optional_file_mtime(
            &package_dir.join("tsconfig.modules.legacy.json"),
        )?);

    let oldest_output = outputs
        .iter()
        .map(|path| file_mtime(path))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .min()
        .ok_or_else(|| "no outputs found".to_string())?;

    Ok(latest_input > oldest_output)
}

fn latest_modified(path: PathBuf) -> Result<SystemTime, String> {
    if !path.exists() {
        return Ok(SystemTime::UNIX_EPOCH);
    }
    if path.is_file() {
        return file_mtime(&path);
    }

    let mut latest = SystemTime::UNIX_EPOCH;
    for entry in
        fs::read_dir(&path).map_err(|e| format!("failed to read {}: {e}", path.display()))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let entry_path = entry.path();
        let entry_time = if entry_path.is_dir() {
            latest_modified(entry_path)?
        } else {
            file_mtime(&entry_path)?
        };
        if entry_time > latest {
            latest = entry_time;
        }
    }
    Ok(latest)
}

fn file_mtime(path: &Path) -> Result<SystemTime, String> {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .map_err(|e| format!("failed to read mtime for {}: {e}", path.display()))
}

fn optional_file_mtime(path: &Path) -> Result<SystemTime, String> {
    if path.exists() {
        file_mtime(path)
    } else {
        Ok(SystemTime::UNIX_EPOCH)
    }
}
