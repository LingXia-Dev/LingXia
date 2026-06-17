use super::ffi;
use crate::AssetFileEntry;
use crate::error::PlatformError;
use crate::traits::app_runtime::AppRuntime;
use crate::traits::app_runtime::LxAppOpenMode;
use crate::traits::media_runtime::MediaRuntime;
use crate::traits::share::{ShareRequest, ShareResult, ShareService};
#[cfg(target_os = "macos")]
#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::fs::OpenOptions;
use std::io::{Cursor, Read};
#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "macos")]
use std::process::Stdio;
#[cfg(target_os = "macos")]
use std::thread;
#[cfg(target_os = "macos")]
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::{SystemTime, UNIX_EPOCH};

/// Platform implementation for Apple platforms (iOS/macOS)
#[derive(Clone)]
pub struct Platform {
    pub data_dir: String,
    pub cache_dir: String,
    pub locale: String,
    pub(crate) market_name: String,
}

unsafe impl Send for Platform {}
unsafe impl Sync for Platform {}

impl crate::traits::update::UpdateService for Platform {
    fn self_update_supported(&self) -> bool {
        // macOS ships outside the App Store and swaps its own bundle; iOS must
        // update through the App Store.
        cfg!(target_os = "macos")
    }

    fn install_update(
        &self,
        package_path: &Path,
        is_force_update: bool,
    ) -> Result<(), PlatformError> {
        #[cfg(target_os = "macos")]
        {
            install_update_on_macos(self, package_path, is_force_update)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (package_path, is_force_update);
            Err(PlatformError::NotSupported(
                "install_update is only supported on macOS".to_string(),
            ))
        }
    }
}

impl Platform {
    /// Create a new Platform instance
    pub fn new(data_dir: String, cache_dir: String, locale: String) -> Result<Self, PlatformError> {
        Ok(Platform {
            data_dir,
            cache_dir,
            locale,
            market_name: super::device::load_platform_market_name(),
        })
    }
}

impl AppRuntime for Platform {
    fn app_data_dir(&self) -> PathBuf {
        PathBuf::from(&self.data_dir)
    }

    fn app_cache_dir(&self) -> PathBuf {
        PathBuf::from(&self.cache_dir)
    }

    fn get_app_identifier(&self) -> Result<String, PlatformError> {
        use objc2_foundation::NSBundle;
        let bundle = NSBundle::mainBundle();
        if let Some(identifier) = bundle.bundleIdentifier() {
            Ok(identifier.to_string())
        } else {
            Err(PlatformError::Platform(
                "Failed to get bundle identifier".to_string(),
            ))
        }
    }

    /// Copy album media to a local file path
    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &std::path::Path,
        kind: crate::traits::media_interaction::MediaKind,
    ) -> Result<(), PlatformError> {
        MediaRuntime::copy_album_media_to_file(self, uri, dest_path, kind)
    }

    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, PlatformError> {
        let data = super::resources::read_asset_data(path);

        if data.is_empty() {
            Err(PlatformError::AssetNotFound(path.to_string()))
        } else {
            Ok(Box::new(Cursor::new(data)))
        }
    }

    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, PlatformError>> + 'a> {
        let entries = self.collect_files_recursively(asset_dir);
        Box::new(entries.into_iter())
    }

    fn get_system_locale(&self) -> &str {
        &self.locale
    }

    fn show_lxapp(
        &self,
        appid: String,
        path: String,
        session_id: u64,
        open_mode: LxAppOpenMode,
        panel_id: String,
    ) -> Result<(), PlatformError> {
        if ffi::open_lxapp(&appid, &path, session_id, open_mode as i32, &panel_id) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to show lxapp: appid={}, path={}, session_id={}, open_mode={:?}, panel_id={}",
                appid, path, session_id, open_mode, panel_id
            )))
        }
    }

    fn hide_lxapp(&self, appid: String, session_id: u64) -> Result<(), PlatformError> {
        if ffi::close_lxapp(&appid, session_id) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to hide lxapp: appid={}, session_id={}",
                appid, session_id
            )))
        }
    }

    fn exit(&self) -> Result<(), PlatformError> {
        if ffi::exit_app() {
            Ok(())
        } else {
            Err(PlatformError::Platform("Failed to exit app".to_string()))
        }
    }

    fn navigate(
        &self,
        appid: String,
        path: String,
        animation_type: crate::traits::app_runtime::AnimationType,
    ) -> Result<(), PlatformError> {
        if ffi::navigate(&appid, &path, animation_type as i32) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to navigate: appid={}, path={}, animation_type={:?}",
                appid, path, animation_type
            )))
        }
    }

    fn open_url(
        &self,
        req: crate::traits::app_runtime::OpenUrlRequest,
    ) -> Result<(), PlatformError> {
        if ffi::open_url(
            &req.owner_appid,
            req.owner_session_id,
            &req.url,
            req.target as i32,
        ) {
            Ok(())
        } else {
            Err(PlatformError::Platform(format!(
                "Failed to open URL: owner_appid={}, owner_session_id={}, url={}, target={:?}",
                req.owner_appid, req.owner_session_id, req.url, req.target
            )))
        }
    }

    async fn get_capsule_rect(&self) -> Result<String, PlatformError> {
        #[cfg(target_os = "ios")]
        {
            crate::rt::native_call(|callback_id| {
                ffi::get_capsule_rect(callback_id);
                Ok(())
            })
            .await
        }
        #[cfg(not(target_os = "ios"))]
        {
            Err(PlatformError::Platform(
                "getCapsuleRect is only supported on iOS".to_string(),
            ))
        }
    }
}

impl ShareService for Platform {
    async fn share(&self, request: ShareRequest) -> Result<ShareResult, PlatformError> {
        let files_json = serde_json::to_string(&request.files)
            .map_err(|e| PlatformError::Platform(format!("Failed to encode share files: {}", e)))?;

        let data = crate::rt::native_call(|callback_id| {
            if ffi::share(
                request.title.as_deref().unwrap_or_default(),
                request.text.as_deref().unwrap_or_default(),
                request.url.as_deref().unwrap_or_default(),
                &files_json,
                callback_id,
            ) {
                Ok(())
            } else {
                Err(PlatformError::Platform(
                    "Failed to present share sheet".to_string(),
                ))
            }
        })
        .await?;

        serde_json::from_str(&data)
            .map_err(|e| PlatformError::Platform(format!("share returned invalid payload: {}", e)))
    }
}

impl Platform {
    /// Recursively collect all files from a directory
    fn collect_files_recursively<'a>(
        &'a self,
        dir_path: &str,
    ) -> Vec<Result<AssetFileEntry<'a>, PlatformError>> {
        let mut all_files = Vec::new();
        let mut dirs_to_process = vec![dir_path.to_string()];

        while let Some(current_dir) = dirs_to_process.pop() {
            let contents = super::resources::list_asset_directory(&current_dir);

            for name in contents {
                let full_path = if current_dir.is_empty() || current_dir == "/" {
                    name.clone()
                } else {
                    format!("{}/{}", current_dir.trim_end_matches('/'), name)
                };

                // Try to read as file first
                let data = super::resources::read_asset_data(&full_path);

                if !data.is_empty() {
                    // It's a file, add it to results
                    let reader: Box<dyn Read + 'a> = Box::new(Cursor::new(data));
                    all_files.push(Ok(AssetFileEntry {
                        path: full_path,
                        reader,
                    }));
                } else {
                    // It might be a directory, try to list it
                    let sub_contents = super::resources::list_asset_directory(&full_path);
                    if !sub_contents.is_empty() {
                        // It's a directory with contents, add it to processing queue
                        dirs_to_process.push(full_path);
                    }
                }
            }
        }

        all_files
    }
}

#[cfg(target_os = "macos")]
fn install_update_on_macos(
    platform: &Platform,
    package_path: &Path,
    is_force_update: bool,
) -> Result<(), PlatformError> {
    if !package_path.exists() {
        return Err(PlatformError::InvalidParameter(format!(
            "Update package does not exist: {}",
            package_path.display()
        )));
    }

    let current_app = current_macos_app_bundle_path()?;
    let prepared = prepare_macos_update_source(platform, package_path, &current_app)?;
    let helper = write_macos_update_helper(platform, &current_app, &prepared)?;
    let staged = StagedMacosUpdate {
        helper,
        pid: std::process::id(),
        bundle_id: platform.get_app_identifier().ok(),
    };

    // The download already finished silently. Ask the shell to surface the
    // post-download prompt and wait for the user to click before swapping the
    // bundle: a dismissible "ready to update" callout for normal updates, or a
    // blocking "must update" modal when the update is forced. If there is no
    // shell (headless run), restart immediately.
    let state = if is_force_update { "ready-force" } else { "ready" };
    let has_ui = ffi::notify_app_update_ready(state);
    if has_ui {
        if let Ok(mut slot) = staged_macos_update_slot().lock() {
            *slot = Some(staged);
        }
    } else {
        spawn_and_exit_macos_update(staged)?;
    }
    Ok(())
}

/// A prepared host-app update awaiting the user's confirmation click. Held in
/// `STAGED_MACOS_UPDATE` between `install_update` (which stages it and shows
/// the sidebar callout) and `apply_staged_macos_update` (the click handler).
#[cfg(target_os = "macos")]
struct StagedMacosUpdate {
    helper: MacosUpdateHelper,
    pid: u32,
    bundle_id: Option<String>,
}

#[cfg(target_os = "macos")]
fn staged_macos_update_slot() -> &'static std::sync::Mutex<Option<StagedMacosUpdate>> {
    static SLOT: std::sync::OnceLock<std::sync::Mutex<Option<StagedMacosUpdate>>> =
        std::sync::OnceLock::new();
    SLOT.get_or_init(|| std::sync::Mutex::new(None))
}

#[cfg(target_os = "macos")]
fn spawn_and_exit_macos_update(staged: StagedMacosUpdate) -> Result<(), PlatformError> {
    spawn_macos_update_helper(&staged.helper)?;
    request_current_process_exit(staged.pid, staged.bundle_id);
    Ok(())
}

/// Apply the update staged by `install_update_on_macos`: launch the swap
/// helper and quit so it can replace the running bundle. Returns `false`
/// when nothing is staged (e.g. the click arrived twice). Invoked from the
/// sidebar "click to restart" callout via the `UpdateRestartClick` app event.
pub fn apply_staged_macos_update() -> bool {
    #[cfg(target_os = "macos")]
    {
        let staged = staged_macos_update_slot()
            .lock()
            .ok()
            .and_then(|mut slot| slot.take());
        match staged {
            Some(staged) => {
                if let Err(error) = spawn_and_exit_macos_update(staged) {
                    log::warn!("Failed to apply staged macOS update: {error}");
                    false
                } else {
                    true
                }
            }
            None => false,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
struct PreparedMacosUpdate {
    source_app: PathBuf,
    cleanup_path: Option<PathBuf>,
}

#[cfg(target_os = "macos")]
struct MacosUpdateHelper {
    script_path: PathBuf,
    log_path: PathBuf,
}

#[cfg(target_os = "macos")]
fn current_macos_app_bundle_path() -> Result<PathBuf, PlatformError> {
    let current_exe = std::env::current_exe().map_err(|e| {
        PlatformError::Platform(format!("Failed to resolve current executable path: {}", e))
    })?;

    for ancestor in current_exe.ancestors() {
        if is_app_bundle(ancestor) {
            return fs::canonicalize(ancestor).map_err(|e| {
                PlatformError::Platform(format!(
                    "Failed to resolve current app bundle path {}: {}",
                    ancestor.display(),
                    e
                ))
            });
        }
    }

    Err(PlatformError::NotSupported(format!(
        "Current process is not running from a macOS app bundle: {}",
        current_exe.display()
    )))
}

/// Returns true if the file begins with the ZIP local-file-header magic
/// (`PK\x03\x04`). Used to recognize a downloaded update package that was
/// saved without a `.zip` extension.
#[cfg(target_os = "macos")]
fn file_has_zip_magic(path: &Path) -> bool {
    use std::io::Read as _;
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 4];
    matches!(file.read_exact(&mut magic), Ok(())) && magic == [0x50, 0x4B, 0x03, 0x04]
}

#[cfg(target_os = "macos")]
fn prepare_macos_update_source(
    platform: &Platform,
    package_path: &Path,
    current_app: &Path,
) -> Result<PreparedMacosUpdate, PlatformError> {
    if is_app_bundle(package_path) {
        return Ok(PreparedMacosUpdate {
            source_app: fs::canonicalize(package_path).map_err(|e| {
                PlatformError::Platform(format!(
                    "Failed to resolve update app bundle {}: {}",
                    package_path.display(),
                    e
                ))
            })?,
            cleanup_path: None,
        });
    }

    // Accept the package as a zip by extension OR by content. The downloader
    // saves the verified package without an extension (e.g. `app_0.2.7_0.2.7`),
    // so sniff the zip magic (`PK\x03\x04`) rather than relying on the name.
    let ext = package_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());
    let is_zip = matches!(ext.as_deref(), Some("zip")) || file_has_zip_magic(package_path);
    if !is_zip {
        return Err(PlatformError::InvalidParameter(format!(
            "Unsupported macOS update package {}. Expected a signed .zip or a staged .app bundle.",
            package_path.display()
        )));
    }

    let stamp = unique_update_stamp();
    let staging_root = platform
        .app_cache_dir()
        .join("LingXia")
        .join("app_updates")
        .join("staged")
        .join(stamp);
    let extract_root = staging_root.join("expanded");
    fs::create_dir_all(&extract_root).map_err(|e| {
        PlatformError::Platform(format!(
            "Failed to create update staging directory {}: {}",
            extract_root.display(),
            e
        ))
    })?;

    let status = Command::new("/usr/bin/ditto")
        .args(["-x", "-k"])
        .arg(package_path)
        .arg(&extract_root)
        .status()
        .map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to extract update archive {}: {}",
                package_path.display(),
                e
            ))
        })?;
    if !status.success() {
        return Err(PlatformError::Platform(format!(
            "Failed to extract update archive {}: {}",
            package_path.display(),
            status
        )));
    }

    Ok(PreparedMacosUpdate {
        source_app: find_single_app_bundle(&extract_root, current_app.file_name())?,
        cleanup_path: Some(staging_root),
    })
}

#[cfg(target_os = "macos")]
fn find_single_app_bundle(
    root: &Path,
    preferred_bundle_name: Option<&std::ffi::OsStr>,
) -> Result<PathBuf, PlatformError> {
    let mut candidates = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to inspect extracted update bundle {}: {}",
                dir.display(),
                e
            ))
        })?;

        for entry in entries {
            let path = entry
                .map_err(|e| {
                    PlatformError::Platform(format!(
                        "Failed to inspect extracted update entry in {}: {}",
                        dir.display(),
                        e
                    ))
                })?
                .path();

            if is_app_bundle(&path) {
                candidates.push(path);
                continue;
            }

            if path.is_dir() {
                stack.push(path);
            }
        }
    }

    if candidates.is_empty() {
        return Err(PlatformError::InvalidParameter(format!(
            "Update archive does not contain a .app bundle: {}",
            root.display()
        )));
    }

    let selected = if candidates.len() == 1 {
        candidates.remove(0)
    } else if let Some(bundle_name) = preferred_bundle_name {
        let mut matching = candidates
            .into_iter()
            .filter(|path| path.file_name() == Some(bundle_name))
            .collect::<Vec<_>>();
        match matching.len() {
            1 => matching.remove(0),
            0 => {
                return Err(PlatformError::InvalidParameter(format!(
                    "Update archive contains multiple .app bundles and none matches current app '{}': {}",
                    bundle_name.to_string_lossy(),
                    root.display()
                )));
            }
            _ => {
                return Err(PlatformError::InvalidParameter(format!(
                    "Update archive contains multiple .app bundles matching current app '{}': {}",
                    bundle_name.to_string_lossy(),
                    root.display()
                )));
            }
        }
    } else {
        return Err(PlatformError::InvalidParameter(format!(
            "Update archive contains multiple .app bundles: {}",
            root.display()
        )));
    };

    fs::canonicalize(&selected).map_err(|e| {
        PlatformError::Platform(format!(
            "Failed to resolve extracted app bundle {}: {}",
            selected.display(),
            e
        ))
    })
}

#[cfg(target_os = "macos")]
fn write_macos_update_helper(
    platform: &Platform,
    current_app: &Path,
    prepared: &PreparedMacosUpdate,
) -> Result<MacosUpdateHelper, PlatformError> {
    let helper_dir = platform
        .app_cache_dir()
        .join("LingXia")
        .join("app_updates")
        .join("helper");
    fs::create_dir_all(&helper_dir).map_err(|e| {
        PlatformError::Platform(format!(
            "Failed to create update helper directory {}: {}",
            helper_dir.display(),
            e
        ))
    })?;

    let stamp = unique_update_stamp();
    let script_path = helper_dir.join(format!("apply-macos-update-{stamp}.sh"));
    let log_path = helper_dir.join(format!("apply-macos-update-{stamp}.log"));
    let backup_path = current_app.with_file_name(format!(
        "{}.lingxia-updating-{}",
        current_app
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("app"),
        stamp
    ));
    let privileged_apply_command = format!(
        "/bin/sh {} --apply >> {} 2>&1",
        shell_quote(&script_path),
        shell_quote(&log_path)
    );
    let privileged_apply_osascript = shell_quote_str(&format!(
        "do shell script {} with administrator privileges",
        apple_script_quote_str(&privileged_apply_command)
    ));

    let script = format!(
        r#"#!/bin/sh
set -eu

PID={pid}
TARGET_APP={target_app}
SOURCE_APP={source_app}
BACKUP_APP={backup_app}
CLEANUP_PATH={cleanup_path}
HELPER_PATH={helper_path}
MODE="${{1:-}}"

wait_for_target_exit() {{
  attempt=0
  while kill -0 "$PID" 2>/dev/null; do
    attempt=$((attempt + 1))
    if [ "$attempt" -eq 120 ]; then
      kill -TERM "$PID" 2>/dev/null || true
    fi
    if [ "$attempt" -eq 160 ]; then
      kill -KILL "$PID" 2>/dev/null || true
    fi
    sleep 0.5
  done
}}

apply_update() {{
  if [ -e "$BACKUP_APP" ]; then
    rm -rf "$BACKUP_APP"
  fi

  if [ -d "$TARGET_APP" ]; then
    mv "$TARGET_APP" "$BACKUP_APP"
  fi

  if ! /usr/bin/ditto "$SOURCE_APP" "$TARGET_APP"; then
    rm -rf "$TARGET_APP"
    if [ -d "$BACKUP_APP" ]; then
      mv "$BACKUP_APP" "$TARGET_APP"
    fi
    return 1
  fi

  if [ -d "$BACKUP_APP" ]; then
    rm -rf "$BACKUP_APP"
  fi
}}

cleanup_update_artifacts() {{
  if [ -n "$CLEANUP_PATH" ] && [ -e "$CLEANUP_PATH" ]; then
    rm -rf "$CLEANUP_PATH"
  fi
}}

finish_success() {{
  /usr/bin/open "$TARGET_APP"
  cleanup_update_artifacts
  rm -f "$HELPER_PATH"
}}

can_replace_directly() {{
  [ -w "$(dirname "$TARGET_APP")" ]
}}

if [ "$MODE" = "--apply" ]; then
  apply_update
  finish_success
  exit 0
fi

wait_for_target_exit

if can_replace_directly; then
  apply_update
  finish_success
  exit 0
fi

if ! /usr/bin/osascript -e {privileged_apply_osascript}; then
  exit 1
fi

"#,
        pid = std::process::id(),
        target_app = shell_quote(current_app),
        source_app = shell_quote(&prepared.source_app),
        backup_app = shell_quote(&backup_path),
        cleanup_path = shell_quote_optional(prepared.cleanup_path.as_deref()),
        helper_path = shell_quote(&script_path),
        privileged_apply_osascript = privileged_apply_osascript,
    );

    fs::write(&script_path, script).map_err(|e| {
        PlatformError::Platform(format!(
            "Failed to write update helper script {}: {}",
            script_path.display(),
            e
        ))
    })?;

    let mut perms = fs::metadata(&script_path)
        .map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to inspect update helper script {}: {}",
                script_path.display(),
                e
            ))
        })?
        .permissions();
    perms.set_mode(0o700);
    fs::set_permissions(&script_path, perms).map_err(|e| {
        PlatformError::Platform(format!(
            "Failed to mark update helper script executable {}: {}",
            script_path.display(),
            e
        ))
    })?;

    Ok(MacosUpdateHelper {
        script_path,
        log_path,
    })
}

#[cfg(target_os = "macos")]
fn spawn_macos_update_helper(helper: &MacosUpdateHelper) -> Result<(), PlatformError> {
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&helper.log_path)
        .map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to create update helper log {}: {}",
                helper.log_path.display(),
                e
            ))
        })?;
    let stderr = stdout.try_clone().map_err(|e| {
        PlatformError::Platform(format!(
            "Failed to duplicate update helper log handle {}: {}",
            helper.log_path.display(),
            e
        ))
    })?;

    Command::new("nohup")
        .arg("/bin/sh")
        .arg(&helper.script_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to launch update helper {}: {}",
                helper.script_path.display(),
                e
            ))
        })?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn request_current_process_exit(pid: u32, bundle_id: Option<String>) {
    let _ = thread::Builder::new()
        .name("lingxia-macos-update-exit".to_string())
        .spawn(move || {
            if let Some(bundle_id) = bundle_id {
                let script = format!(
                    "tell application id \"{}\" to quit",
                    bundle_id.replace('\\', "\\\\").replace('"', "\\\"")
                );
                let _ = Command::new("/usr/bin/osascript")
                    .arg("-e")
                    .arg(script)
                    .status();
            }

            thread::sleep(Duration::from_millis(800));
            let _ = Command::new("/bin/kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .status();
        });
}

#[cfg(target_os = "macos")]
fn unique_update_stamp() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{}-{}", millis, std::process::id())
}

#[cfg(target_os = "macos")]
fn is_app_bundle(path: &Path) -> bool {
    path.is_dir()
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
}

#[cfg(target_os = "macos")]
fn shell_quote(path: &Path) -> String {
    shell_quote_str(&path.to_string_lossy())
}

#[cfg(target_os = "macos")]
fn shell_quote_optional(path: Option<&Path>) -> String {
    path.map(shell_quote).unwrap_or_else(|| "''".to_string())
}

#[cfg(target_os = "macos")]
fn shell_quote_str(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'\''"#))
}

#[cfg(target_os = "macos")]
fn apple_script_quote_str(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
