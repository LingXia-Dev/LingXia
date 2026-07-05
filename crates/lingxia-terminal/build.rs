use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_REPO: &str = "https://github.com/ghostty-org/ghostty.git";
/// Pinned Ghostty revision for libghostty-vt. Newer revs pin a themes tarball
/// that 404s on ghostty's deps server (the fetch aborts and the lib is silently
/// dropped); this is a recent rev whose deps still resolve.
const DEFAULT_REV: &str = "ca7516bea60190ee2e9a4f9182b61d318d107c6e";

fn main() {
    emit_rerun_env();
    emit_check_cfg();

    match prepare_ghostty_vt() {
        Ok(prepared) => emit_prepared(&prepared),
        Err(err) if env_flag("LINGXIA_GHOSTTY_REQUIRED") => {
            panic!("failed to prepare libghostty-vt: {err}");
        }
        Err(err) => emit_unavailable(&err),
    }
}

fn emit_rerun_env() {
    println!("cargo:rerun-if-changed=build.rs");
    for key in [
        "LINGXIA_GHOSTTY_SOURCE_DIR",
        "LINGXIA_GHOSTTY_CACHE_DIR",
        "LINGXIA_GHOSTTY_REPO",
        "LINGXIA_GHOSTTY_REV",
        "LINGXIA_GHOSTTY_ZIG",
        "LINGXIA_GHOSTTY_ZIG_ARGS",
        "LINGXIA_GHOSTTY_VT_STEP",
        "LINGXIA_GHOSTTY_VT_SIMD",
        "LINGXIA_GHOSTTY_OPTIMIZE",
        "LINGXIA_GHOSTTY_REQUIRED",
        "ZIG_GLOBAL_CACHE_DIR",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
}

fn emit_check_cfg() {
    println!("cargo:rustc-check-cfg=cfg(lingxia_ghostty_vt_available)");
}

fn emit_prepared(prepared: &PreparedGhostty) {
    println!("cargo:rustc-cfg=lingxia_ghostty_vt_available");
    println!("cargo:rustc-env=LINGXIA_GHOSTTY_AVAILABLE=1");
    println!(
        "cargo:rustc-env=LINGXIA_GHOSTTY_SOURCE_DIR={}",
        prepared.source_dir.display()
    );
    println!(
        "cargo:rustc-env=LINGXIA_GHOSTTY_LIB_DIR={}",
        prepared.lib_dir.display()
    );
    println!("cargo:rustc-env=LINGXIA_GHOSTTY_STATUS={}", prepared.status);
    println!(
        "cargo:rustc-link-search=native={}",
        prepared.lib_dir.display()
    );
    println!("cargo:rustc-link-lib=static={}", prepared.link_name);
}

fn emit_unavailable(reason: &str) {
    println!("cargo:warning=libghostty-vt unavailable: {reason}");
    println!("cargo:rustc-env=LINGXIA_GHOSTTY_AVAILABLE=0");
    println!(
        "cargo:rustc-env=LINGXIA_GHOSTTY_STATUS={}",
        sanitize_status(reason)
    );
}

fn prepare_ghostty_vt() -> Result<PreparedGhostty, String> {
    let zig = env_os("LINGXIA_GHOSTTY_ZIG").unwrap_or_else(|| OsString::from("zig"));
    probe_zig(&zig)?;

    let source_dir = if let Some(source_dir) = env_path("LINGXIA_GHOSTTY_SOURCE_DIR") {
        if !source_dir.is_dir() {
            return Err(format!(
                "LINGXIA_GHOSTTY_SOURCE_DIR does not exist: {}",
                source_dir.display()
            ));
        }
        source_dir
    } else {
        fetch_git_checkout()?
    };

    build_vt(&source_dir, &zig)?;
    let (lib_path, link_name) = find_vt_lib(&source_dir)?;
    let lib_dir = lib_path
        .parent()
        .ok_or_else(|| format!("missing parent for {}", lib_path.display()))?
        .to_path_buf();

    Ok(PreparedGhostty {
        source_dir,
        lib_dir,
        link_name,
        status: "libghostty-vt prepared".to_string(),
    })
}

fn fetch_git_checkout() -> Result<PathBuf, String> {
    let rev = env_string("LINGXIA_GHOSTTY_REV").unwrap_or_else(|| DEFAULT_REV.to_string());
    let repo = env_string("LINGXIA_GHOSTTY_REPO").unwrap_or_else(|| DEFAULT_REPO.to_string());
    let cache_dir = cache_dir()?.join(format!("git-{}", sanitize(&rev)));

    if cache_dir.join(".git").is_dir() {
        if current_git_rev(&cache_dir).as_deref() != Some(&rev) {
            run(
                "git",
                [
                    OsStr::new("fetch"),
                    OsStr::new("--tags"),
                    OsStr::new("--force"),
                ],
                Some(&cache_dir),
            )?;
        }
    } else {
        if let Some(parent) = cache_dir.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
        }
        run(
            "git",
            [
                OsStr::new("clone"),
                OsStr::new("--filter=blob:none"),
                OsStr::new(&repo),
                cache_dir.as_os_str(),
            ],
            None,
        )?;
    }

    run(
        "git",
        [OsStr::new("checkout"), OsStr::new(&rev)],
        Some(&cache_dir),
    )?;
    Ok(cache_dir)
}

fn build_vt(source_dir: &Path, zig: &OsStr) -> Result<(), String> {
    let invocation = pick_vt_invocation(zig, source_dir)?;
    let optimize =
        env_string("LINGXIA_GHOSTTY_OPTIMIZE").unwrap_or_else(|| "ReleaseFast".to_string());
    let simd = if env_flag("LINGXIA_GHOSTTY_VT_SIMD") {
        "-Dsimd=true"
    } else {
        "-Dsimd=false"
    };

    let mut args = vec!["build".to_string()];
    args.extend(invocation);
    if let Some(target) = zig_target_arg() {
        args.push(format!("-Dtarget={target}"));
    }
    args.push(format!("-Doptimize={optimize}"));
    args.push(simd.to_string());
    if let Some(extra) = env_string("LINGXIA_GHOSTTY_ZIG_ARGS") {
        args.extend(extra.split_whitespace().map(ToOwned::to_owned));
    }

    run_os(zig, args.iter().map(OsStr::new), Some(source_dir))
}

fn probe_zig(zig: &OsStr) -> Result<(), String> {
    let output = Command::new(zig)
        .arg("version")
        .output()
        .map_err(|e| format!("failed to start {} version: {e}", zig.to_string_lossy()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} version failed with status {}: {}",
            zig.to_string_lossy(),
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn pick_vt_invocation(_zig: &OsStr, source_dir: &Path) -> Result<Vec<String>, String> {
    if let Some(step) = env_string("LINGXIA_GHOSTTY_VT_STEP") {
        return Ok(vec![step]);
    }

    let config_zig = source_dir.join("src/build/Config.zig");
    if fs::read_to_string(&config_zig)
        .map(|text| text.contains("\"emit-lib-vt\""))
        .unwrap_or(false)
    {
        return Ok(vec!["-Demit-lib-vt=true".to_string()]);
    }

    let help = fs::read_to_string(source_dir.join("build.zig")).unwrap_or_default();
    for candidate in [
        "ghostty-vt-static",
        "libghostty-vt-static",
        "vt-static",
        "libghostty-vt",
        "ghostty-vt",
    ] {
        if help.lines().any(|line| {
            let line = line.trim_start();
            line == candidate || line.starts_with(&format!("{candidate} "))
        }) {
            return Ok(vec![candidate.to_string()]);
        }
    }

    Err("Ghostty checkout does not expose a libghostty-vt build option or step".to_string())
}

fn find_vt_lib(source_dir: &Path) -> Result<(PathBuf, String), String> {
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let candidates: &[(&str, &str)] = if target_env == "msvc" {
        &[
            ("ghostty-vt-static.lib", "ghostty-vt-static"),
            ("ghostty-vt.lib", "ghostty-vt"),
        ]
    } else {
        &[
            ("libghostty-vt-static.a", "ghostty-vt-static"),
            ("libghostty-vt.a", "ghostty-vt"),
        ]
    };

    for (file, link) in candidates {
        if let Some(path) = try_find_file(source_dir, file) {
            return Ok((path, (*link).to_string()));
        }
    }

    Err(format!(
        "could not find libghostty-vt archive under {} after zig build",
        source_dir.display()
    ))
}

fn try_find_file(root: &Path, file_name: &str) -> Option<PathBuf> {
    let zig_cache = root.join(".zig-cache");
    let mut candidates = Vec::new();
    if zig_cache.is_dir() {
        collect_files_named(&zig_cache, file_name, &mut candidates);
    }

    for relative in ["zig-out/lib", "zig-out\\lib", "macos/build/Debug", "lib"] {
        let candidate = root.join(relative).join(file_name);
        if candidate.is_file() {
            candidates.push(candidate);
        }
    }

    candidates.sort_by(|a, b| {
        let a_time = fs::metadata(a).and_then(|m| m.modified()).ok();
        let b_time = fs::metadata(b).and_then(|m| m.modified()).ok();
        b_time.cmp(&a_time)
    });
    candidates
        .into_iter()
        .find(|path| archive_matches_target(path))
}

fn collect_files_named(root: &Path, file_name: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_files_named(&path, file_name, out);
        } else if file_type.is_file() && path.file_name() == Some(OsStr::new(file_name)) {
            out.push(path);
        }
    }
}

fn run<I, S>(program: &str, args: I, cwd: Option<&Path>) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_os(OsStr::new(program), args, cwd)
}

fn run_os<I, S>(program: &OsStr, args: I, cwd: Option<&Path>) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(program);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    if let Some(cache) = env_os("ZIG_GLOBAL_CACHE_DIR") {
        command.env("ZIG_GLOBAL_CACHE_DIR", cache);
    }
    // Capture instead of inheriting stdout: inherited child output goes
    // straight to cargo and could be misinterpreted as `cargo:` directives.
    let output = command
        .output()
        .map_err(|e| format!("failed to start {}: {e}", program.to_string_lossy()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} failed with status {}: {}",
            program.to_string_lossy(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn cache_dir() -> Result<PathBuf, String> {
    if let Some(path) = env_path("LINGXIA_GHOSTTY_CACHE_DIR") {
        return Ok(path);
    }
    // Default inside OUT_DIR — CARGO_MANIFEST_DIR/../../target breaks
    // for the published crate (it would write outside the registry
    // checkout). LINGXIA_GHOSTTY_CACHE_DIR overrides for shared caches.
    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|e| e.to_string())?);
    if let Some(cache) = windows_same_drive_cache(&out_dir) {
        return Ok(cache);
    }
    Ok(out_dir.join("ghostty-cache"))
}

/// zig 0.15's build runner asserts when a tool path cannot be expressed
/// relative to the build cwd, which on Windows happens whenever the
/// ghostty checkout sits on a different drive than the zig compiler
/// (e.g. project on D:, zig under C:\Users). When OUT_DIR and zig
/// disagree on the drive, fall back to a per-user build cache on zig's drive
/// (under the profile root, not AppData — AV products are quick to eat
/// freshly linked unsigned executables like zig's build runner there).
fn windows_same_drive_cache(out_dir: &Path) -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let zig = env_os("LINGXIA_GHOSTTY_ZIG").unwrap_or_else(|| OsString::from("zig"));
    let zig_path = which_zig(&zig)?;
    let zig_drive = windows_drive(&zig_path)?;
    if windows_drive(out_dir) == Some(zig_drive) {
        return None;
    }
    let profile = env_path("USERPROFILE")?;
    if windows_drive(&profile) != Some(zig_drive) {
        return None;
    }
    Some(
        profile
            .join(".cache")
            .join("lingxia")
            .join("build")
            .join("ghostty-vt"),
    )
}

fn which_zig(zig: &OsStr) -> Option<PathBuf> {
    let candidate = Path::new(zig);
    if candidate.is_absolute() {
        return Some(candidate.to_path_buf());
    }
    let output = Command::new("where.exe").arg(zig).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(PathBuf::from)
}

fn windows_drive(path: &Path) -> Option<char> {
    match path.components().next() {
        Some(std::path::Component::Prefix(prefix)) => match prefix.kind() {
            std::path::Prefix::Disk(letter) | std::path::Prefix::VerbatimDisk(letter) => {
                Some(letter.to_ascii_uppercase() as char)
            }
            _ => None,
        },
        _ => None,
    }
}

fn current_git_rev(repo_dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_string())
}

fn zig_target_arg() -> Option<&'static str> {
    let os = env::var("CARGO_CFG_TARGET_OS").ok()?;
    let arch = env::var("CARGO_CFG_TARGET_ARCH").ok()?;
    match (os.as_str(), arch.as_str()) {
        ("macos", "aarch64") => Some("aarch64-macos"),
        ("macos", "x86_64") => Some("x86_64-macos"),
        ("linux", "aarch64") => Some("aarch64-linux-gnu"),
        ("linux", "x86_64") => Some("x86_64-linux-gnu"),
        ("windows", "aarch64") => Some("aarch64-windows-msvc"),
        ("windows", "x86_64") => Some("x86_64-windows-msvc"),
        _ => None,
    }
}

fn archive_matches_target(path: &Path) -> bool {
    if env::var("CARGO_CFG_TARGET_OS").ok().as_deref() != Some("macos") {
        return true;
    }
    let Ok(arch) = env::var("CARGO_CFG_TARGET_ARCH") else {
        return true;
    };
    let lipo_arch = match arch.as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        _ => return true,
    };
    let Ok(output) = Command::new("lipo").arg("-info").arg(path).output() else {
        return true;
    };
    if !output.status.success() {
        return true;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace().any(|part| part == lipo_arch)
}

fn env_flag(key: &str) -> bool {
    env::var(key)
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

fn env_string(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_os(key: &str) -> Option<OsString> {
    env::var_os(key).filter(|value| !value.is_empty())
}

fn env_path(key: &str) -> Option<PathBuf> {
    env_string(key).map(PathBuf::from)
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn sanitize_status(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\r' | '\n' => ' ',
            ch if ch.is_control() => ' ',
            ch => ch,
        })
        .collect()
}

struct PreparedGhostty {
    source_dir: PathBuf,
    lib_dir: PathBuf,
    link_name: String,
    status: String,
}
