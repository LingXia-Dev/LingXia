use serde_json::Value as JsonValue;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;
use toml::{Table as TomlTable, Value as TomlValue};

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
    let node_modules = package_dir.join("node_modules");
    let bin_file = node_modules.join(".bin").join(if cfg!(windows) {
        format!("{bin_name}.cmd")
    } else {
        bin_name.to_string()
    });

    if node_modules.is_dir() && bin_file.is_file() {
        return Ok(());
    }

    let install_cmd = if package_dir.join("package-lock.json").is_file() {
        "npm ci"
    } else {
        "npm install"
    };
    Err(format!(
        "npm build tooling (`{bin_name}`) is not installed in {}.\n\
Run `cd {} && {install_cmd}` first, then retry `cargo build -p lingxia-cli`.",
        node_modules.display(),
        package_dir.display(),
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
