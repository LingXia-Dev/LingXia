use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

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
    let package_json = bridge_dir.join("package.json");
    let expected_version = env::var("CARGO_PKG_VERSION").map_err(|e| e.to_string())?;

    emit_rerun_markers(&bridge_dir)?;

    let actual_version = read_bridge_version(&package_json)?;
    if actual_version != expected_version {
        return Err(format!(
            "lingxia-cli version {} does not match @lingxia/bridge version {}",
            expected_version, actual_version
        ));
    }

    let es2020_src = bridge_dir.join("dist").join("bridge-runtime.es2020.js");
    let es5_src = bridge_dir.join("dist").join("bridge-runtime.es5.js");
    if should_build_bridge(&bridge_dir, &[&es2020_src, &es5_src])? {
        ensure_npm_available()?;
        ensure_bridge_tooling_installed(&bridge_dir)?;
        let status = Command::new("npm")
            .arg("run")
            .arg("build")
            .current_dir(&bridge_dir)
            .status()
            .map_err(|e| {
                format!(
                    "failed to start npm run build in {}: {e}",
                    bridge_dir.display()
                )
            })?;
        if !status.success() {
            return Err(format!(
                "npm run build failed in {} with status {}",
                bridge_dir.display(),
                status
            ));
        }
    }

    for file in [&es2020_src, &es5_src] {
        if !file.is_file() {
            return Err(format!("missing bridge runtime bundle: {}", file.display()));
        }
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|e| e.to_string())?);
    let es2020_out = out_dir.join("bridge-runtime.es2020.js");
    let es5_out = out_dir.join("bridge-runtime.es5.js");

    fs::copy(&es2020_src, &es2020_out)
        .map_err(|e| format!("failed to copy {}: {e}", es2020_src.display()))?;
    fs::copy(&es5_src, &es5_out)
        .map_err(|e| format!("failed to copy {}: {e}", es5_src.display()))?;

    println!(
        "cargo:rustc-env=LINGXIA_BRIDGE_RUNTIME_ES2020={}",
        es2020_out.display()
    );
    println!(
        "cargo:rustc-env=LINGXIA_BRIDGE_RUNTIME_ES5={}",
        es5_out.display()
    );

    Ok(())
}

fn ensure_npm_available() -> Result<(), String> {
    match Command::new("npm").arg("--version").status() {
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

fn ensure_bridge_tooling_installed(bridge_dir: &Path) -> Result<(), String> {
    let node_modules = bridge_dir.join("node_modules");
    let rolldown_bin = bridge_dir
        .join("node_modules")
        .join(".bin")
        .join(if cfg!(windows) {
            "rolldown.cmd"
        } else {
            "rolldown"
        });
    let tsc_bin = bridge_dir
        .join("node_modules")
        .join(".bin")
        .join(if cfg!(windows) { "tsc.cmd" } else { "tsc" });

    if node_modules.is_dir() && rolldown_bin.is_file() && tsc_bin.is_file() {
        return Ok(());
    }

    let install_cmd = if bridge_dir.join("package-lock.json").is_file() {
        "npm ci"
    } else {
        "npm install"
    };
    Err(format!(
        "@lingxia/bridge build tooling is not installed in {}.\n\
Run `cd {} && {}` first, then retry `cargo build -p lingxia-cli`.",
        node_modules.display(),
        bridge_dir.display(),
        install_cmd
    ))
}

fn read_bridge_version(package_json: &Path) -> Result<String, String> {
    let content = fs::read_to_string(package_json)
        .map_err(|e| format!("failed to read {}: {e}", package_json.display()))?;
    let value: Value = serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {e}", package_json.display()))?;
    value
        .get("version")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("missing version in {}", package_json.display()))
}

fn emit_rerun_markers(bridge_dir: &Path) -> Result<(), String> {
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

fn should_build_bridge(bridge_dir: &Path, outputs: &[&Path]) -> Result<bool, String> {
    if outputs.iter().any(|path| !path.is_file()) {
        return Ok(true);
    }

    let latest_input = latest_modified(bridge_dir.join("src"))?
        .max(latest_modified(bridge_dir.join("scripts"))?)
        .max(file_mtime(&bridge_dir.join("package.json"))?)
        .max(optional_file_mtime(&bridge_dir.join("package-lock.json"))?)
        .max(file_mtime(&bridge_dir.join("rolldown.config.js"))?)
        .max(file_mtime(&bridge_dir.join("tsconfig.modules.json"))?)
        .max(file_mtime(
            &bridge_dir.join("tsconfig.modules.legacy.json"),
        )?);

    let oldest_output = outputs
        .iter()
        .map(|path| file_mtime(path))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .min()
        .ok_or_else(|| "no bridge outputs found".to_string())?;

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
