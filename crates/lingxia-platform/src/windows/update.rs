//! Windows host-app update install flow.
//!
//! Mirrors the macOS updater (`apple/app.rs`): the shared service layer
//! downloads and SHA256-verifies the package, then calls [`install_update`],
//! which stages the new build and hands off to a detached helper that waits for
//! this process to exit, swaps the install directory, and relaunches the app.
//!
//! The signed feed archive contains the runnable payload plus optional NSIS /
//! portable installers. Installed hosts run Setup; portable hosts replace the
//! outer launcher. Legacy directory installs mirror the payload. MSIX stays
//! under OS deployment control.

mod installer;

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::Platform;
use super::app::request_windows_app_exit;
use crate::error::PlatformError;
use crate::traits::app_runtime::AppRuntime;
use crate::traits::update::UpdateService;

/// `CREATE_NO_WINDOW` — run a child as a hidden background console process
/// (no console window). Used both for tar and for the detached update helper.
const NO_WINDOW: u32 = 0x0800_0000;

fn windows_installed_from_store() -> bool {
    use windows::ApplicationModel::{Package, PackageSignatureKind};
    let Ok(package) = Package::Current() else {
        return false;
    };
    matches!(package.SignatureKind(), Ok(PackageSignatureKind::Store))
}

impl UpdateService for Platform {
    fn self_update_supported(&self) -> bool {
        // Store and sideloaded MSIX packages are both OS-managed.
        windows::ApplicationModel::Package::Current().is_err()
    }

    fn installed_from_store(&self) -> bool {
        windows_installed_from_store()
    }

    fn open_update_store(&self, info_json: &str) -> Result<bool, PlatformError> {
        Ok(open_windows_store_listing(info_json))
    }

    fn present_store_update(&self, info_json: &str) -> Result<bool, PlatformError> {
        present_windows_store_update(self, info_json);
        Ok(true)
    }

    fn install_update(&self, package_path: &Path, info_json: &str) -> Result<(), PlatformError> {
        install_update_on_windows(self, package_path, info_json)
    }
}

fn open_windows_store_listing(info_json: &str) -> bool {
    let Some(url) = crate::traits::update::store_url_in_update_info(info_json) else {
        log::warn!("[lingxia] store update has no listing URL");
        return false;
    };
    if open_shell_url(&url) {
        return true;
    }
    if let Some(id) = url
        .split_once("ProductId=")
        .map(|(_, id)| id.trim())
        .filter(|id| !id.is_empty())
    {
        return open_shell_url(&format!("https://apps.microsoft.com/detail/{id}"));
    }
    false
}

fn open_shell_url(url: &str) -> bool {
    match super::file::open_with_shell_detached(url) {
        Ok(()) => true,
        Err(error) => {
            log::warn!("[lingxia] failed to open store listing: {error}");
            false
        }
    }
}

fn present_windows_store_update(platform: &Platform, info_json: &str) {
    let mut info = parse_card_info(info_json);
    info.product_name = platform.product_name().to_string();
    info.logo_path = resolve_brand_logo(platform.asset_dir());
    info.locale = platform.get_system_locale().to_string();
    info.open_store = true;
    info.store_url = crate::traits::update::store_url_in_update_info(info_json);
    super::update_callout::set_context(platform.product_name(), platform.get_system_locale());
    super::update_card::present_ready(info);
}

/// A prepared host-app update awaiting the user's confirmation click. Held
/// between [`install_update_on_windows`] (which stages it and surfaces the
/// callout) and [`apply_staged_windows_update`] (the click handler).
struct StagedWindowsUpdate {
    helper: WindowsUpdateHelper,
}

struct WindowsUpdateHelper {
    script_path: PathBuf,
    log_path: PathBuf,
}

struct PreparedWindowsUpdate {
    /// Directory containing the new `.exe` + assets to mirror into the install.
    source_dir: PathBuf,
    cleanup_path: Option<PathBuf>,
}

fn staged_windows_update_slot() -> &'static Mutex<Option<StagedWindowsUpdate>> {
    static SLOT: OnceLock<Mutex<Option<StagedWindowsUpdate>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// Stage the already-downloaded package and surface the single post-download
/// moment: the "ready to update" UI. The package downloaded silently; this is
/// where the user first sees the update. A forced update shows the card (with
/// release notes + Restart) directly; otherwise a dismissible bottom-left
/// callout offers it, and clicking the callout opens the card.
fn install_update_on_windows(
    platform: &Platform,
    package_path: &Path,
    info_json: &str,
) -> Result<(), PlatformError> {
    if windows::ApplicationModel::Package::Current().is_ok() {
        return Err(PlatformError::NotSupported(
            "MSIX updates must use Microsoft Store or App Installer".into(),
        ));
    }
    if !package_path.exists() {
        return Err(PlatformError::InvalidParameter(format!(
            "Update package does not exist: {}",
            package_path.display()
        )));
    }

    let current_exe = std::env::current_exe().map_err(|e| {
        PlatformError::Platform(format!("Failed to resolve current executable path: {e}"))
    })?;
    let install_dir = current_exe
        .parent()
        .ok_or_else(|| {
            PlatformError::Platform(format!(
                "Current executable has no parent directory: {}",
                current_exe.display()
            ))
        })?
        .to_path_buf();

    let prepared = prepare_windows_update_source(platform, package_path, &current_exe)?;
    let helper = match fs::read_to_string(install_dir.join(".lingxia-distribution")) {
        Ok(kind) if matches!(kind.as_str(), "nsis" | "portable") => {
            write_packaged_update_helper(platform, &kind, &current_exe, &prepared, info_json)?
        }
        Ok(_) => {
            return Err(PlatformError::InvalidParameter(
                "Unknown Windows distribution kind".into(),
            ));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            write_windows_update_helper(platform, &install_dir, &current_exe, &prepared)?
        }
        Err(e) => {
            return Err(PlatformError::Platform(format!(
                "Failed to read distribution marker: {e}"
            )));
        }
    };
    let staged = StagedWindowsUpdate { helper };

    if let Ok(mut slot) = staged_windows_update_slot().lock() {
        *slot = Some(staged);
    }

    // Build the "ready to update" card model: version / release notes come
    // from `info_json` (`{version, releaseNotes}`); the size comes from the
    // downloaded package, and the logo from the app assets.
    let mut info = parse_card_info(info_json);
    info.product_name = platform.product_name().to_string();
    info.logo_path = resolve_brand_logo(platform.asset_dir());
    info.size_bytes = std::fs::metadata(package_path).ok().map(|m| m.len());
    info.locale = platform.get_system_locale().to_string();
    super::update_callout::set_context(platform.product_name(), platform.get_system_locale());
    super::update_card::present_ready(info);

    Ok(())
}

/// Parse the install hand-off JSON (`{"version","releaseNotes":[...]}`) into
/// the card model.
fn parse_card_info(info_json: &str) -> super::update_card::CardInfo {
    let mut info = super::update_card::CardInfo::default();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(info_json) else {
        return info;
    };
    if let Some(v) = value.get("version").and_then(serde_json::Value::as_str) {
        info.version = v.to_string();
    }
    info.size_bytes = value.get("size").and_then(serde_json::Value::as_u64);
    if let Some(arr) = value
        .get("releaseNotes")
        .and_then(serde_json::Value::as_array)
    {
        info.release_notes = arr
            .iter()
            .filter_map(|n| n.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    info.open_store = value
        .get("openStore")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    info.store_url = value
        .get("storeUrl")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    info
}

/// Resolve the brand logo PNG for the update card, preferring the clean brand
/// icon over the badged dev launcher icon.
fn resolve_brand_logo(asset_dir: &Path) -> Option<PathBuf> {
    [
        asset_dir.join("AppIcon.png"),
        asset_dir
            .join("app.lingxia.browser")
            .join("public")
            .join("LingXia.png"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// Apply the update staged by [`install_update_on_windows`]: launch the swap
/// helper and quit so it can replace the running install. Returns `false` when
/// nothing is staged (e.g. the click arrived twice). Invoked from the product
/// shell's "click to install" callout.
pub fn apply_staged_windows_update() -> bool {
    let staged = staged_windows_update_slot()
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    let Some(staged) = staged else {
        return false;
    };
    if let Err(error) = spawn_windows_update_helper(&staged.helper) {
        log::warn!("Failed to apply staged Windows update: {error}");
        // Re-stage so a retry is possible.
        if let Ok(mut slot) = staged_windows_update_slot().lock() {
            *slot = Some(staged);
        }
        return false;
    }
    request_windows_app_exit();
    true
}

/// Returns true if the file begins with the ZIP local-file-header magic
/// (`PK\x03\x04`). The downloader saves the verified package without an
/// extension, so recognize it by content, not name.
fn file_has_zip_magic(path: &Path) -> bool {
    use std::io::Read as _;
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 4];
    matches!(file.read_exact(&mut magic), Ok(())) && magic == [0x50, 0x4B, 0x03, 0x04]
}

fn prepare_windows_update_source(
    platform: &Platform,
    package_path: &Path,
    current_exe: &Path,
) -> Result<PreparedWindowsUpdate, PlatformError> {
    let exe_name = current_exe.file_name();

    if package_path.is_dir() {
        let source_dir = find_install_root(package_path, exe_name)?;
        return Ok(PreparedWindowsUpdate {
            source_dir,
            cleanup_path: None,
        });
    }

    let ext = package_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());
    let is_zip = matches!(ext.as_deref(), Some("zip")) || file_has_zip_magic(package_path);
    if !is_zip {
        return Err(PlatformError::InvalidParameter(format!(
            "Unsupported Windows update package {}. Expected a .zip of the install directory.",
            package_path.display()
        )));
    }

    let stamp = unique_update_stamp();
    let staging_root = update_root(platform).join("staged").join(&stamp);
    let extract_root = staging_root.join("expanded");
    fs::create_dir_all(&extract_root).map_err(|e| {
        PlatformError::Platform(format!(
            "Failed to create update staging directory {}: {e}",
            extract_root.display()
        ))
    })?;

    // `tar` (bsdtar, in System32 on Windows 10+) reads zip by content, so the
    // extension-less verified package extracts fine.
    let status = Command::new("tar.exe")
        .arg("-xf")
        .arg(package_path)
        .arg("-C")
        .arg(&extract_root)
        .creation_flags(NO_WINDOW)
        .status()
        .map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to extract update archive {}: {e}",
                package_path.display()
            ))
        })?;
    if !status.success() {
        return Err(PlatformError::Platform(format!(
            "Failed to extract update archive {}: exit {status}",
            package_path.display()
        )));
    }

    let source_dir = find_install_root(&extract_root, exe_name)?;
    Ok(PreparedWindowsUpdate {
        source_dir,
        cleanup_path: Some(staging_root),
    })
}

/// Finds the directory in `root` that holds the new install (the one containing
/// the app `.exe`). Prefers the directory holding an exe whose name matches the
/// running executable; falls back to the sole `.exe` found.
fn find_install_root(
    root: &Path,
    preferred_exe: Option<&std::ffi::OsStr>,
) -> Result<PathBuf, PlatformError> {
    let mut exe_dirs: Vec<PathBuf> = Vec::new();
    let mut preferred: Option<PathBuf> = None;
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to inspect extracted update {}: {e}",
                dir.display()
            ))
        })?;
        for entry in entries {
            let path = entry
                .map_err(|e| {
                    PlatformError::Platform(format!(
                        "Failed to read extracted update entry in {}: {e}",
                        dir.display()
                    ))
                })?
                .path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let is_exe = path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
            if !is_exe {
                continue;
            }
            if let Some(parent) = path.parent() {
                if preferred_exe.is_some() && path.file_name() == preferred_exe {
                    preferred = Some(parent.to_path_buf());
                }
                exe_dirs.push(parent.to_path_buf());
            }
        }
    }

    if let Some(dir) = preferred {
        if !dir.join("assets/app.json").is_file() {
            return Err(PlatformError::InvalidParameter(
                "Update payload is missing assets/app.json".into(),
            ));
        }
        return Ok(dir);
    }
    exe_dirs.sort();
    exe_dirs.dedup();
    match exe_dirs.len() {
        1 => Ok(exe_dirs.remove(0)),
        0 => Err(PlatformError::InvalidParameter(format!(
            "Update archive does not contain an .exe: {}",
            root.display()
        ))),
        _ => Err(PlatformError::InvalidParameter(format!(
            "Update archive contains multiple .exe directories and none matches the current app: {}",
            root.display()
        ))),
    }
}

fn write_windows_update_helper(
    platform: &Platform,
    install_dir: &Path,
    current_exe: &Path,
    prepared: &PreparedWindowsUpdate,
) -> Result<WindowsUpdateHelper, PlatformError> {
    let helper_dir = update_root(platform).join("helper");
    fs::create_dir_all(&helper_dir).map_err(|e| {
        PlatformError::Platform(format!(
            "Failed to create update helper directory {}: {e}",
            helper_dir.display()
        ))
    })?;

    let stamp = unique_update_stamp();
    let script_path = helper_dir.join(format!("apply-windows-update-{stamp}.ps1"));
    let log_path = helper_dir.join(format!("apply-windows-update-{stamp}.log"));

    let script = render_helper_script(
        std::process::id(),
        install_dir,
        &prepared.source_dir,
        current_exe,
        prepared.cleanup_path.as_deref(),
        &script_path,
    );

    fs::write(&script_path, script).map_err(|e| {
        PlatformError::Platform(format!(
            "Failed to write update helper script {}: {e}",
            script_path.display()
        ))
    })?;

    Ok(WindowsUpdateHelper {
        script_path,
        log_path,
    })
}

/// Renders the PowerShell swap helper. It waits for this process to exit,
/// mirrors the new tree over the install dir with `robocopy /MIR`, relaunches
/// the app, then deletes the staging tree and itself. If the install dir is not
/// writable (e.g. under `Program Files`) it relaunches itself elevated.
fn render_helper_script(
    pid: u32,
    install_dir: &Path,
    source_dir: &Path,
    current_exe: &Path,
    cleanup_path: Option<&Path>,
    script_path: &Path,
) -> String {
    let target = ps_single_quote(&install_dir.to_string_lossy());
    let source = ps_single_quote(&source_dir.to_string_lossy());
    let exe = ps_single_quote(&current_exe.to_string_lossy());
    let cleanup = cleanup_path
        .map(|p| ps_single_quote(&p.to_string_lossy()))
        .unwrap_or_else(|| "''".to_string());
    let self_path = ps_single_quote(&script_path.to_string_lossy());

    let mut head = format!(
        r#"$ErrorActionPreference = 'Continue'
$procId = {pid}
$target = {target}
$source = {source}
$exe = {exe}
$cleanup = {cleanup}
$self = {self_path}
$log = "$self.log"
function L($m) {{ try {{ Add-Content -Path $log -Value ((Get-Date).ToString('HH:mm:ss.fff') + ' ' + $m) }} catch {{}} }}
L "start pid=$procId target=$target"

# Wait for the host app to exit so its files unlock.
$deadline = (Get-Date).AddSeconds(90)
while (Get-Process -Id $procId -ErrorAction SilentlyContinue) {{
  if ((Get-Date) -gt $deadline) {{ Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue; break }}
  Start-Sleep -Milliseconds 300
}}
Start-Sleep -Milliseconds 400
L "app exited"

# Elevate if the install directory is not writable (e.g. Program Files).
$canWrite = $false
try {{
  $probe = Join-Path $target (".lxupd-" + [System.IO.Path]::GetRandomFileName())
  Set-Content -Path $probe -Value 'x' -ErrorAction Stop
  Remove-Item -Path $probe -Force -ErrorAction SilentlyContinue
  $canWrite = $true
}} catch {{ $canWrite = $false }}
L "canWrite=$canWrite"

if (-not $canWrite -and $args[0] -ne '--elevated') {{
  L "elevating"
  Start-Process -FilePath 'powershell.exe' -Verb RunAs -WindowStyle Hidden -ArgumentList @(
    '-NoProfile','-ExecutionPolicy','Bypass','-WindowStyle','Hidden','-File',$self,'--elevated')
  exit 0
}}

# Mirror the new build over the install dir. Invoke robocopy directly (a
# detached, console-less helper can't reliably Start-Process a console app).
# robocopy exit codes 0-7 are success.
L "robocopy start: $source -> $target"
& robocopy.exe $source $target /MIR /XD .lingxia-update /R:3 /W:1 /NJH /NJS /NP /NDL /NFL /NC *>$null
$rc = $LASTEXITCODE
L "robocopy exit=$rc"
if ($rc -ge 8) {{ L "robocopy FAILED"; exit 1 }}
"#,
        pid = pid,
        target = target,
        source = source,
        exe = exe,
        cleanup = cleanup,
        self_path = self_path,
    );
    // Appended as a raw string so the C# `Add-Type` block's braces don't need
    // `format!` escaping. It references PowerShell vars set in `head`.
    head.push_str(RELAUNCH_AND_PROMOTE);
    format!("\u{feff}{head}")
}

/// PowerShell appended to the swap helper: relaunch the updated app. A
/// background helper can't grant the freshly launched window foreground, and the
/// app would otherwise open inert in the taskbar at the default cascade
/// position. Rather than race the app from outside, we tag the relaunch with
/// `LINGXIA_UPDATE_RELAUNCH` (inherited by the child) so the app centers and
/// force-foregrounds its own main window from its UI thread as it is shown.
const RELAUNCH_AND_PROMOTE: &str = r#"
# Drop a marker next to the exe so the relaunched app centers + foregrounds its
# own main window. A marker file is used instead of an env var or argument: a
# detached, console-less helper can't reliably hand a child its modified
# environment block, and the app's arg parsing is not ours to extend. The app
# deletes the marker as it reads it. Written after robocopy so /MIR won't wipe it.
L "relaunching $exe"
$marker = Join-Path $target '.lx-update-relaunch'
Set-Content -Path $marker -Value '1' -ErrorAction SilentlyContinue
Start-Process -FilePath $exe -WorkingDirectory $target
L "relaunched"

# Clean up staging and the helper itself.
if ($cleanup -and (Test-Path $cleanup)) { Remove-Item -Recurse -Force $cleanup -ErrorAction SilentlyContinue }
L "done"
Remove-Item -Force $self -ErrorAction SilentlyContinue
"#;

fn spawn_windows_update_helper(helper: &WindowsUpdateHelper) -> Result<(), PlatformError> {
    // `CREATE_BREAKAWAY_FROM_JOB`: if a launcher placed the app in a
    // kill-on-close job object, the helper must escape it to outlive this
    // process. Not all jobs permit breakaway, so fall back without it.
    const BREAKAWAY: u32 = 0x0100_0000;

    let spawn_with = |flags: u32| -> std::io::Result<()> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&helper.log_path)?;
        let log_err = log.try_clone()?;
        // `CREATE_NO_WINDOW` runs the helper as a hidden background console
        // process that survives this app's exit. (Do NOT use DETACHED_PROCESS:
        // a console-less PowerShell here failed to execute the `-File` script.)
        installer::command(&helper.script_path)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err))
            .creation_flags(flags)
            .spawn()
            .map(|_| ())
    };

    spawn_with(NO_WINDOW | BREAKAWAY)
        .or_else(|_| spawn_with(NO_WINDOW))
        .map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to launch update helper {}: {e}",
                helper.script_path.display()
            ))
        })
}

fn update_root(platform: &Platform) -> PathBuf {
    platform.app_cache_dir().join("LingXia").join("app_updates")
}

fn unique_update_stamp() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{millis}-{}", std::process::id())
}

/// Escapes a string for a PowerShell single-quoted literal (double the quote).
fn ps_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn write_packaged_update_helper(
    platform: &Platform,
    kind: &str,
    current_exe: &Path,
    prepared: &PreparedWindowsUpdate,
    info_json: &str,
) -> Result<WindowsUpdateHelper, PlatformError> {
    use self::installer::Installer;
    let fail = |message: String| PlatformError::InvalidParameter(message);
    let update_dir = prepared.source_dir.join(".lingxia-update");
    let metadata = fs::read(update_dir.join("manifest.json"))
        .map_err(|e| fail(format!("Update has no distribution manifest: {e}. Publish a package built with --format {kind}.")))?;
    let metadata: serde_json::Value = serde_json::from_slice(&metadata)
        .map_err(|e| fail(format!("Invalid distribution manifest: {e}")))?;
    let architecture = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        arch => arch,
    };
    let info: serde_json::Value =
        serde_json::from_str(info_json).map_err(|e| fail(e.to_string()))?;
    validate_update_manifest(
        &metadata,
        platform.app_identifier(),
        architecture,
        current_exe
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| fail("Invalid executable name".into()))?,
        info["version"]
            .as_str()
            .ok_or_else(|| fail("Missing verified update version".into()))?,
    )
    .map_err(fail)?;
    let package = update_dir.join(if kind == "nsis" {
        "setup.exe"
    } else {
        "portable.exe"
    });
    if !package.is_file() {
        return Err(fail(format!(
            "Update does not include {kind}; publish all supported formats together"
        )));
    }
    let helper_dir = update_root(platform).join("helper");
    fs::create_dir_all(&helper_dir).map_err(|e| fail(e.to_string()))?;
    let stamp = unique_update_stamp();
    let script_path = helper_dir.join(format!("apply-windows-update-{stamp}.ps1"));
    let log_path = helper_dir.join(format!("apply-windows-update-{stamp}.log"));
    let cleanup = prepared
        .cleanup_path
        .as_deref()
        .ok_or_else(|| fail("Packaged updates require a staged archive".into()))?;
    let install_root;
    let launcher;
    let installer = if kind == "nsis" {
        install_root = current_exe
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| fail("Invalid NSIS install root".into()))?
            .to_path_buf();
        let owner = fs::read_to_string(install_root.join(".lingxia-install-id"))
            .map_err(|e| fail(e.to_string()))?;
        if owner != platform.app_identifier() {
            return Err(fail(
                "NSIS install owner does not match app identity".into(),
            ));
        }
        Installer::Nsis {
            setup: &package,
            install_root: &install_root,
            executable: current_exe,
        }
    } else {
        launcher = std::env::var_os("LINGXIA_PORTABLE_EXECUTABLE")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute() && path.is_file())
            .ok_or_else(|| fail("Portable launcher path is unavailable".into()))?;
        let launcher_pid = std::env::var("LINGXIA_PORTABLE_LAUNCHER_PID")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|pid| *pid > 0 && *pid != std::process::id())
            .ok_or_else(|| fail("Portable launcher process is unavailable".into()))?;
        Installer::Portable {
            source: &package,
            launcher: &launcher,
            launcher_pid,
        }
    };
    let script = installer::render(std::process::id(), installer, cleanup, &script_path);
    fs::write(&script_path, script).map_err(|e| fail(e.to_string()))?;
    Ok(WindowsUpdateHelper {
        script_path,
        log_path,
    })
}

/// Bind the signed archive's installer to this installation and verified feed version.
fn validate_update_manifest(
    value: &serde_json::Value,
    identity: &str,
    architecture: &str,
    executable: &str,
    version: &str,
) -> Result<(), String> {
    if value["schemaVersion"].as_u64() != Some(1)
        || value["appId"].as_str() != Some(identity)
        || value["architecture"].as_str() != Some(architecture)
        || value["executable"].as_str() != Some(executable)
        || value["version"].as_str() != Some(version)
    {
        return Err("Windows update identity, architecture, executable or version does not match this installation/update".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_wrong_installer_target_or_feed_version() {
        let manifest = serde_json::json!({"schemaVersion": 1, "appId": "com.example.app", "architecture": "x64", "executable": "demo.exe", "version": "2.0.0"});
        let validate = |value: &serde_json::Value| {
            validate_update_manifest(value, "com.example.app", "x64", "demo.exe", "2.0.0")
        };
        assert!(validate(&manifest).is_ok());
        for (field, replacement) in [
            ("appId", "other.app"),
            ("architecture", "arm64"),
            ("executable", "other.exe"),
            ("version", "1.0.0"),
        ] {
            let mut value = manifest.clone();
            value[field] = replacement.into();
            assert!(validate(&value).is_err(), "{field}");
        }
        let mut newer_schema = manifest;
        newer_schema["schemaVersion"] = 2.into();
        assert!(validate(&newer_schema).is_err());
    }
}
