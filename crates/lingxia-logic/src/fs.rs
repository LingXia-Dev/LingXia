use crate::dismissal::{canceled, completed};
use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error,
    js_error_from_platform_error, js_internal_error, js_invalid_parameter_error,
};
use base64::{Engine as _, engine::general_purpose};
use lingxia_service::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogFilter, OpenFileRequest,
};
use lxapp::LxApp;
use rong::{
    AnyJSTypedArray, Class, FromJSObject, HostError, IntoJSObject, JSArrayBuffer, JSContext,
    JSObject, JSResult, JSTypedArray, JSValue, JsonToJSValue, function::Optional, js_class,
    js_method,
};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs as tokio_fs;

mod download;
mod network_security;
mod storage;
mod upload;

const READ_FILE_MAX_BYTES: u64 = 16 * 1024 * 1024;

#[derive(FromJSObject)]
#[ts_skip]
struct JSOpenFileOptions {
    #[js_name = "filePath"]
    file_path: String,
    #[js_name = "fileType"]
    file_type: Option<String>,
    mode: Option<String>,
    #[js_name = "showMenu"]
    show_menu: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenFileMode {
    Auto,
    Review,
    External,
}

impl OpenFileMode {
    fn parse(raw: Option<&str>, api_name: &'static str) -> JSResult<Self> {
        match raw.map(str::trim).filter(|value| !value.is_empty()) {
            None | Some("auto") => Ok(Self::Auto),
            Some("review") => Ok(Self::Review),
            Some("external") => Ok(Self::External),
            Some(_) => Err(js_error_from_business_code_with_detail(
                1002,
                format!("{api_name} requires mode to be auto, review, or external"),
            )),
        }
    }
}

fn map_file_type_to_mime(file_type: Option<String>) -> Option<String> {
    match file_type.unwrap_or_default().to_lowercase().as_str() {
        "pdf" => Some("application/pdf".to_string()),
        "doc" => Some("application/msword".to_string()),
        "docx" => Some(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string(),
        ),
        "ppt" => Some("application/vnd.ms-powerpoint".to_string()),
        "pptx" => Some(
            "application/vnd.openxmlformats-officedocument.presentationml.presentation".to_string(),
        ),
        "xls" => Some("application/vnd.ms-excel".to_string()),
        "xlsx" => {
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string())
        }
        "zip" => Some("application/zip".to_string()),
        _ => None,
    }
}

fn resolve_open_file_request(
    lxapp: &LxApp,
    options: &JSOpenFileOptions,
    api_name: &'static str,
) -> JSResult<OpenFileRequest> {
    if options.file_path.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            format!("{api_name} requires filePath"),
        ));
    }

    let resolved_path = lxapp
        .resolve_accessible_path(&options.file_path)
        .map_err(|err| crate::i18n::js_error_from_lxapp_error(&err))?;

    Ok(OpenFileRequest {
        path: resolved_path.to_string_lossy().into_owned(),
        mime_type: map_file_type_to_mime(options.file_type.clone()),
        show_menu: options.show_menu,
    })
}

async fn open_file_with_mode(
    lxapp: &LxApp,
    request: OpenFileRequest,
    mode: OpenFileMode,
) -> JSResult<()> {
    match mode {
        OpenFileMode::Auto => {
            if let Err(review_error) =
                lingxia_service::file::review_file(&*lxapp.runtime, request.clone()).await
            {
                match lingxia_service::file::open_external(&*lxapp.runtime, request).await {
                    Ok(()) => Ok(()),
                    Err(open_external_error) => {
                        let _ = review_error;
                        Err(js_error_from_platform_error(&open_external_error))
                    }
                }
            } else {
                Ok(())
            }
        }
        OpenFileMode::Review => lingxia_service::file::review_file(&*lxapp.runtime, request)
            .await
            .map_err(|e| js_error_from_platform_error(&e)),
        OpenFileMode::External => lingxia_service::file::open_external(&*lxapp.runtime, request)
            .await
            .map_err(|e| js_error_from_platform_error(&e)),
    }
}

/// Open a local file with the requested strategy.
///
/// Use `mode: "review"` when the UX requires in-app preview; otherwise prefer
/// `mode: "auto"`.
async fn open_file(ctx: JSContext, options: JSOpenFileOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let mode = OpenFileMode::parse(options.mode.as_deref(), "openFile")?;
    let request = resolve_open_file_request(&lxapp, &options, "openFile")?;
    open_file_with_mode(&lxapp, request, mode).await
}

#[derive(FromJSObject, Clone, Default)]
#[ts_skip]
struct JSFileDialogFilter {
    name: Option<String>,
    extensions: Option<Vec<String>>,
}

#[derive(FromJSObject, Clone, Default)]
#[ts_skip]
struct JSChooseFileOptions {
    multiple: Option<bool>,
    filters: Option<Vec<JSFileDialogFilter>>,
    #[js_name = "defaultPath"]
    default_path: Option<String>,
}

#[derive(FromJSObject, Clone, Default)]
#[ts_skip]
struct JSChooseDirectoryOptions {
    #[js_name = "defaultPath"]
    default_path: Option<String>,
}

#[derive(Default, FromJSObject)]
#[ts_skip]
struct JSFsMkdirOptions {
    recursive: Option<bool>,
}

#[derive(Default, FromJSObject)]
#[ts_skip]
struct JSFsWriteOptions {
    encoding: Option<String>,
    overwrite: Option<bool>,
}

#[derive(Default, FromJSObject)]
#[ts_skip]
struct JSFsOverwriteOptions {
    overwrite: Option<bool>,
}

#[derive(Default, FromJSObject)]
#[ts_skip]
struct JSFsRemoveOptions {
    recursive: Option<bool>,
}

#[derive(Debug, Clone, IntoJSObject)]
struct FileStats {
    #[js_name = "isFile"]
    is_file: bool,
    #[js_name = "isDirectory"]
    is_directory: bool,
    #[js_name = "isSymlink"]
    is_symlink: bool,
    size: u64,
    #[js_name = "lastModifiedTime"]
    last_modified_time: Option<u64>,
    #[js_name = "lastAccessedTime"]
    last_accessed_time: Option<u64>,
    #[js_name = "createTime"]
    create_time: Option<u64>,
}

#[js_class(clone)]
struct JSLxFile {
    lxapp: Weak<LxApp>,
    user_data_dir: PathBuf,
    path: String,
}

impl JSLxFile {
    fn new(lxapp: &Arc<LxApp>, path: String) -> Self {
        Self {
            lxapp: Arc::downgrade(lxapp),
            user_data_dir: lxapp.user_data_dir.clone(),
            path,
        }
    }

    fn lxapp(&self) -> JSResult<Arc<LxApp>> {
        let lxapp = self
            .lxapp
            .upgrade()
            .ok_or_else(|| js_internal_error("LxFile owner LxApp has been released"))?;
        if lxapp.user_data_dir != self.user_data_dir {
            return Err(js_internal_error("LxFile owner LxApp changed"));
        }
        Ok(lxapp)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ManagedPathKind {
    Temp,
    UserData,
    UserCache,
    /// A transient grant to a file/directory the user explicitly picked
    /// (chooseFile / chooseDirectory / chooseMedia on desktop). Read-only:
    /// the readable resolver accepts it, the writable resolver does not.
    Granted,
}

#[derive(Clone, Debug)]
struct ManagedPath {
    path: PathBuf,
    kind: ManagedPathKind,
}

impl ManagedPathKind {
    fn is_app_storage(self) -> bool {
        matches!(self, Self::UserData | Self::UserCache)
    }

    fn label(self) -> &'static str {
        match self {
            Self::Temp => "lx://temp",
            Self::UserData => "lx://userdata",
            Self::UserCache => "lx://usercache",
            Self::Granted => "granted file",
        }
    }
}

fn managed_root(lxapp: &LxApp, kind: ManagedPathKind) -> Option<&Path> {
    match kind {
        ManagedPathKind::Temp | ManagedPathKind::Granted => None,
        ManagedPathKind::UserData => Some(&lxapp.user_data_dir),
        ManagedPathKind::UserCache => Some(&lxapp.user_cache_dir),
    }
}

#[js_class(clone)]
struct JSDirEntry {
    name: String,
    is_directory: bool,
    is_symlink: bool,
}

#[js_class(rename = "DirEntry")]
impl JSDirEntry {
    #[js_method(constructor, private)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.fs.readDir(path)",
        )
        .into())
    }

    #[js_method(getter)]
    fn name(&self) -> String {
        self.name.clone()
    }

    #[js_method(getter, rename = "isFile")]
    fn is_file(&self) -> bool {
        !self.is_directory && !self.is_symlink
    }

    #[js_method(getter, rename = "isDirectory")]
    fn is_directory(&self) -> bool {
        self.is_directory
    }

    #[js_method(getter, rename = "isSymlink")]
    fn is_symlink(&self) -> bool {
        self.is_symlink
    }
}

fn normalize_extensions(raw: Option<Vec<String>>) -> Vec<String> {
    raw.unwrap_or_default()
        .into_iter()
        .map(|ext| ext.trim().trim_start_matches('.').to_lowercase())
        .filter(|ext| !ext.is_empty())
        .collect()
}

fn resolve_dialog_default_path(lxapp: &LxApp, raw_path: &str) -> JSResult<String> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    let resolved = lxapp
        .resolve_accessible_path(trimmed)
        .map_err(|err| crate::i18n::js_error_from_lxapp_error(&err))?;

    Ok(resolved.to_string_lossy().into_owned())
}

fn selected_file_path_to_uri(lxapp: &LxApp, raw_path: &str) -> JSResult<String> {
    let path = raw_path.trim();
    if path.is_empty() {
        return Err(js_internal_error("chooseFile returned an empty path"));
    }

    if is_platform_file_reference(path) {
        return lxapp.grant_transient_file_reference(path).map_err(|err| {
            js_internal_error(format!("chooseFile failed to grant file access: {err}"))
        });
    }

    if let Ok(resolved) = lxapp.resolve_accessible_path(path)
        && let Some(uri) = lxapp.to_uri(&resolved)
    {
        return Ok(uri.into_string());
    }

    let path_ref = Path::new(path);
    if path_ref.is_absolute() {
        return lxapp
            .grant_transient_file_access(path_ref)
            .map(|uri| uri.into_string())
            .map_err(|err| {
                js_internal_error(format!(
                    "chooseFile failed to grant temporary file access for {}: {}",
                    path_ref.display(),
                    err
                ))
            });
    }

    Err(js_internal_error(format!(
        "chooseFile returned an inaccessible path: {}",
        path
    )))
}

fn is_platform_file_reference(path: &str) -> bool {
    let Some((scheme, _)) = path.split_once(':') else {
        return false;
    };
    matches!(
        scheme.to_ascii_lowercase().as_str(),
        "content" | "datashare" | "file"
    )
}

fn selected_directory_path_to_uri(lxapp: &LxApp, raw_path: &str) -> JSResult<String> {
    let path = raw_path.trim();
    if path.is_empty() {
        return Err(js_internal_error("chooseDirectory returned an empty path"));
    }

    if let Ok(resolved) = lxapp.resolve_accessible_path(path)
        && let Some(uri) = lxapp.to_uri(&resolved)
    {
        return Ok(uri.into_string());
    }

    let path_ref = Path::new(path);
    if path_ref.is_absolute() {
        return lxapp
            .grant_transient_directory_access(path_ref)
            .map(|uri| uri.into_string())
            .map_err(|err| {
                js_internal_error(format!(
                    "chooseDirectory failed to grant temporary directory access for {}: {}",
                    path_ref.display(),
                    err
                ))
            });
    }

    Err(js_internal_error(format!(
        "chooseDirectory returned an inaccessible path: {}",
        path
    )))
}

/// Opens a file picker.
///
/// Resolves `{ canceled: true }` only when the user dismisses the picker. A
/// completed selection resolves `{ canceled: false, paths }` with at least one
/// path. Rejects when the picker fails or returns an invalid payload.
async fn choose_file(ctx: JSContext, options: Optional<JSChooseFileOptions>) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let opts = options.as_ref().cloned().unwrap_or_default();
    let default_path = opts
        .default_path
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| resolve_dialog_default_path(&lxapp, value))
        .transpose()?
        .filter(|path| !path.is_empty());

    let filters = opts
        .filters
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            let extensions = normalize_extensions(item.extensions);
            if extensions.is_empty() {
                return None;
            }
            Some(FileDialogFilter {
                name: item.name,
                extensions,
            })
        })
        .collect();

    let result = lingxia_service::file::choose_file(
        &*lxapp.runtime,
        ChooseFileRequest {
            multiple: opts.multiple.unwrap_or(false),
            filters,
            title: None,
            default_path,
        },
    )
    .await
    .map_err(|e| js_error_from_platform_error(&e))?;

    if result.canceled {
        return canceled(&ctx);
    }
    if result.paths.is_empty() {
        return Err(js_internal_error(
            "chooseFile invalid payload: non-canceled result must include at least one path",
        ));
    }

    let paths = result
        .paths
        .iter()
        .map(|path| selected_file_path_to_uri(&lxapp, path))
        .collect::<JSResult<Vec<_>>>()?;

    let chosen = completed(&ctx)?;
    chosen.set("paths", paths)?;
    Ok(chosen)
}

/// Opens a directory picker.
///
/// Resolves `{ canceled: true }` only when the user dismisses the picker. A
/// completed selection resolves `{ canceled: false, path }`. Rejects when the
/// picker fails or returns an invalid payload.
async fn choose_directory(
    ctx: JSContext,
    options: Optional<JSChooseDirectoryOptions>,
) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let opts = options.as_ref().cloned().unwrap_or_default();
    let default_path = opts
        .default_path
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| resolve_dialog_default_path(&lxapp, value))
        .transpose()?
        .filter(|path| !path.is_empty());

    let result = lingxia_service::file::choose_directory(
        &*lxapp.runtime,
        ChooseDirectoryRequest {
            title: None,
            default_path,
        },
    )
    .await
    .map_err(|e| js_error_from_platform_error(&e))?;

    if result.canceled {
        return canceled(&ctx);
    }
    // Same tripwire as chooseFile: the union removed the type, not the state.
    if result.paths.len() != 1 {
        return Err(js_internal_error(
            "chooseDirectory invalid payload: non-canceled result must include exactly one path",
        ));
    }
    let path = result.paths.into_iter().next().ok_or_else(|| {
        js_internal_error(
            "chooseDirectory invalid payload: non-canceled result must include exactly one path",
        )
    })?;

    let chosen = completed(&ctx)?;
    chosen.set("path", selected_directory_path_to_uri(&lxapp, &path)?)?;
    Ok(chosen)
}

fn system_time_millis(value: std::io::Result<SystemTime>) -> Option<u64> {
    value
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}

fn normalize_relative_path<'a>(
    raw_path: &'a str,
    api_name: &'static str,
    field_name: &'static str,
) -> JSResult<&'a str> {
    let path = raw_path.trim();
    if path.is_empty() {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} requires {field_name}"
        )));
    }
    let path_ref = Path::new(path);
    if path_ref.is_absolute() || path.contains(':') || path.contains('\\') {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} {field_name} must be a clean relative path or supported lx:// URI"
        )));
    }
    if path
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} {field_name} must not contain empty, '.' or '..' segments"
        )));
    }
    Ok(path)
}

fn resolve_relative_managed_path(
    user_data_dir: &Path,
    raw_path: &str,
    api_name: &'static str,
    field_name: &'static str,
) -> JSResult<PathBuf> {
    let relative = normalize_relative_path(raw_path, api_name, field_name)?;
    Ok(user_data_dir.join(relative))
}

fn classify_managed_path(lxapp: &LxApp, path: &Path) -> Option<ManagedPathKind> {
    fn path_starts_with_root(path: &Path, root: &Path) -> bool {
        if root.as_os_str().is_empty() {
            return false;
        }
        if path.starts_with(root) {
            return true;
        }
        if let Ok(canonical_root) = std::fs::canonicalize(root) {
            return path.starts_with(canonical_root);
        }
        false
    }

    if path_starts_with_root(path, &lxapp.temp_dir) {
        Some(ManagedPathKind::Temp)
    } else if path_starts_with_root(path, &lxapp.user_data_dir) {
        Some(ManagedPathKind::UserData)
    } else if path_starts_with_root(path, &lxapp.user_cache_dir) {
        Some(ManagedPathKind::UserCache)
    } else {
        None
    }
}

fn is_storage_root(lxapp: &LxApp, path: &ManagedPath) -> bool {
    match path.kind {
        ManagedPathKind::Temp | ManagedPathKind::Granted => false,
        ManagedPathKind::UserData => path.path == lxapp.user_data_dir,
        ManagedPathKind::UserCache => path.path == lxapp.user_cache_dir,
    }
}

fn ensure_managed_path_kind_allowed(
    kind: ManagedPathKind,
    api_name: &'static str,
    field_name: &'static str,
    allow_temp: bool,
    allow_usercache: bool,
) -> JSResult<()> {
    if kind == ManagedPathKind::Temp && !allow_temp {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} {field_name} must not target lx://temp"
        )));
    }
    if kind == ManagedPathKind::UserCache && !allow_usercache {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} {field_name} must not target lx://usercache"
        )));
    }
    Ok(())
}

fn resolve_managed_path(
    lxapp: &LxApp,
    raw_path: &str,
    api_name: &'static str,
    field_name: &'static str,
    allow_temp: bool,
    allow_usercache: bool,
    require_child: bool,
) -> JSResult<ManagedPath> {
    // Read-only callers accept transient grants; writers never do.
    let allow_granted = !require_child;
    let path = raw_path.trim();
    if path.starts_with("lx://") {
        let resolved = lxapp
            .resolve_accessible_path(path)
            .map_err(|err| js_error_from_lxapp_error(&err))?;
        let kind = match classify_managed_path(lxapp, &resolved) {
            Some(kind) => kind,
            // The URI resolved (so the lxapp may access this path) but it is
            // not under managed storage: a transient grant to a file the
            // user explicitly picked (chooseFile / chooseMedia on desktop).
            None if allow_granted => ManagedPathKind::Granted,
            None => {
                return Err(js_invalid_parameter_error(format!(
                    "{api_name} {field_name} must target LingXia-managed storage"
                )));
            }
        };
        ensure_managed_path_kind_allowed(kind, api_name, field_name, allow_temp, allow_usercache)?;
        let path = ManagedPath {
            path: resolved,
            kind,
        };
        if require_child && is_storage_root(lxapp, &path) {
            return Err(js_invalid_parameter_error(format!(
                "{api_name} {field_name} must reference a path under {}",
                kind.label()
            )));
        }
        return Ok(path);
    }

    Ok(ManagedPath {
        path: resolve_relative_managed_path(&lxapp.user_data_dir, path, api_name, field_name)?,
        kind: ManagedPathKind::UserData,
    })
}

fn resolve_readable_path(
    lxapp: &LxApp,
    raw_path: &str,
    api_name: &'static str,
    field_name: &'static str,
) -> JSResult<ManagedPath> {
    let path = raw_path.trim();
    if path.is_empty() {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} requires {field_name}"
        )));
    }
    resolve_managed_path(lxapp, path, api_name, field_name, true, true, false)
}

fn resolve_writable_path(
    lxapp: &LxApp,
    raw_path: &str,
    api_name: &'static str,
    field_name: &'static str,
) -> JSResult<ManagedPath> {
    resolve_managed_path(lxapp, raw_path, api_name, field_name, false, true, true)
}

pub(crate) fn resolve_writable_file_path(
    lxapp: &LxApp,
    raw_path: &str,
    api_name: &'static str,
    field_name: &'static str,
    allow_temp: bool,
) -> JSResult<PathBuf> {
    let path = resolve_managed_path(
        lxapp, raw_path, api_name, field_name, allow_temp, true, true,
    )?;
    ensure_no_symlink_ancestors(lxapp, &path, api_name, field_name)?;
    match std::fs::symlink_metadata(&path.path) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(js_invalid_parameter_error(format!(
                "{api_name} {field_name} must not target a symlink"
            )));
        }
        Ok(_) => {
            return Err(js_invalid_parameter_error(format!(
                "{api_name} {field_name} must target a regular file"
            )));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(js_internal_error(format!(
                "{api_name} failed to inspect {field_name}: {err}"
            )));
        }
    }
    Ok(path.path)
}

fn file_stats(metadata: std::fs::Metadata) -> FileStats {
    let file_type = metadata.file_type();
    FileStats {
        is_file: file_type.is_file(),
        is_directory: file_type.is_dir(),
        is_symlink: file_type.is_symlink(),
        size: metadata.len(),
        last_modified_time: system_time_millis(metadata.modified()),
        last_accessed_time: system_time_millis(metadata.accessed()),
        create_time: system_time_millis(metadata.created()),
    }
}

fn ensure_not_exists(path: &Path, api_name: &'static str) -> JSResult<()> {
    if std::fs::symlink_metadata(path).is_ok() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            format!("{api_name} destination already exists"),
        ));
    }
    Ok(())
}

fn ensure_no_symlink_ancestors(
    lxapp: &LxApp,
    managed: &ManagedPath,
    api_name: &'static str,
    field_name: &'static str,
) -> JSResult<()> {
    let Some(root) = managed_root(lxapp, managed.kind) else {
        return Ok(());
    };
    let Ok(relative) = managed.path.strip_prefix(root) else {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} {field_name} must stay inside {}",
            managed.kind.label()
        )));
    };
    let mut current = root.to_path_buf();
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        if components.peek().is_none() {
            break;
        }
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(js_invalid_parameter_error(format!(
                    "{api_name} {field_name} must not pass through a symlink"
                )));
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => break,
            Err(err) => {
                return Err(js_internal_error(format!(
                    "{api_name} failed to inspect {field_name}: {err}"
                )));
            }
        }
    }
    Ok(())
}

fn symlink_metadata(managed: &ManagedPath, api_name: &'static str) -> JSResult<std::fs::Metadata> {
    std::fs::symlink_metadata(&managed.path)
        .map_err(|err| js_internal_error(format!("{api_name} stat failed: {err}")))
}

fn mark_usercache_access(path: &ManagedPath) {
    if path.kind == ManagedPathKind::UserCache {
        lxapp::touch_access_time(&path.path);
    }
}

fn cleanup_usercache_preserving(lxapp: &LxApp, preserve: Option<&Path>) {
    lingxia_service::storage::cleanup_usercache_preserving(&lxapp.user_cache_dir, preserve);
}

fn finish_write(lxapp: &LxApp, destination: &ManagedPath) {
    if destination.kind == ManagedPathKind::UserCache {
        mark_usercache_access(destination);
        cleanup_usercache_preserving(lxapp, Some(&destination.path));
    }
}

fn ensure_write_quota(
    lxapp: &LxApp,
    destination: &ManagedPath,
    incoming_bytes: u64,
    source: Option<&ManagedPath>,
    is_move: bool,
) -> JSResult<()> {
    let same_storage_move = is_move && source.is_some_and(|source| source.kind == destination.kind);
    let removed_source = if is_move {
        source.map(|source| source.path.as_path())
    } else {
        None
    };
    if !same_storage_move {
        match destination.kind {
            ManagedPathKind::UserData => storage::ensure_userdata_quota_with_removed(
                &lxapp.user_data_dir,
                &destination.path,
                incoming_bytes,
                removed_source,
            ),
            ManagedPathKind::UserCache => match source
                .filter(|source| source.kind == ManagedPathKind::UserCache)
                .map(|source| source.path.as_path())
            {
                Some(source_path) => storage::ensure_usercache_quota_preserving(
                    &lxapp.user_cache_dir,
                    &destination.path,
                    incoming_bytes,
                    removed_source,
                    &[source_path],
                ),
                None => storage::ensure_usercache_quota(
                    &lxapp.user_cache_dir,
                    &destination.path,
                    incoming_bytes,
                    removed_source,
                ),
            },
            ManagedPathKind::Temp => Err(storage::StorageQuotaError::Temp),
            // The writable resolver never yields grants (allow_granted is
            // derived from !require_child), so a granted destination cannot
            // reach quota accounting.
            ManagedPathKind::Granted => unreachable!("granted paths are read-only"),
        }
        .map_err(storage::quota_error_to_js)?;
    }

    let app_storage_incoming = if is_move
        && source.is_some_and(|source| source.kind.is_app_storage())
        && destination.kind.is_app_storage()
    {
        0
    } else {
        incoming_bytes
    };
    if app_storage_incoming > 0 {
        let mut keep_cache_paths = Vec::with_capacity(2);
        if destination.kind == ManagedPathKind::UserCache {
            keep_cache_paths.push(destination.path.as_path());
        }
        if let Some(source) = source.filter(|source| source.kind == ManagedPathKind::UserCache) {
            keep_cache_paths.push(source.path.as_path());
        }
        storage::ensure_app_storage_quota_preserving_many(
            &lxapp.user_data_dir,
            &lxapp.user_cache_dir,
            &destination.path,
            app_storage_incoming,
            &keep_cache_paths,
        )
        .map_err(storage::quota_error_to_js)?;
    }
    Ok(())
}

fn decode_encoding(raw: Option<&str>, api_name: &'static str) -> JSResult<Option<&'static str>> {
    match raw.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(None),
        Some("utf8") | Some("utf-8") => Ok(Some("utf8")),
        Some("base64") => Ok(Some("base64")),
        Some(_) => Err(js_invalid_parameter_error(format!(
            "{api_name} encoding must be utf8 or base64"
        ))),
    }
}

fn js_value_to_bytes(
    value: JSValue,
    encoding: Option<&str>,
    api_name: &'static str,
) -> JSResult<Vec<u8>> {
    if value.is_string() {
        let text = value
            .to_rust::<String>()
            .map_err(|_| js_invalid_parameter_error(format!("{api_name} data must be a string")))?;
        return match decode_encoding(encoding, api_name)? {
            Some("base64") => general_purpose::STANDARD.decode(text).map_err(|err| {
                js_invalid_parameter_error(format!("{api_name} invalid base64 data: {err}"))
            }),
            _ => Ok(text.into_bytes()),
        };
    }
    if encoding.is_some() {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} encoding is only valid for string data"
        )));
    }
    if value.is_array_buffer() {
        let buffer = value.to_rust::<JSArrayBuffer>().map_err(|_| {
            js_invalid_parameter_error(format!("{api_name} data must be ArrayBuffer"))
        })?;
        return Ok(buffer.as_bytes().to_vec());
    }
    if let Some(obj) = value.into_object()
        && let Some(typed_array) = AnyJSTypedArray::from_object(obj)
        && let Some(bytes) = typed_array.as_bytes()
    {
        return Ok(bytes.to_vec());
    }
    Err(js_invalid_parameter_error(format!(
        "{api_name} data must be string, ArrayBuffer, or TypedArray"
    )))
}

fn ensure_read_file_size(size: u64, api_name: &'static str) -> JSResult<()> {
    if size > READ_FILE_MAX_BYTES {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} file exceeds the {} MiB limit",
            READ_FILE_MAX_BYTES / 1024 / 1024
        )));
    }
    Ok(())
}

fn read_file_bytes(path: &Path, expected_size: u64, api_name: &'static str) -> JSResult<Vec<u8>> {
    ensure_read_file_size(expected_size, api_name)?;
    let file = std::fs::File::open(path)
        .map_err(|err| js_internal_error(format!("{api_name} failed: {err}")))?;
    let mut reader = file.take(READ_FILE_MAX_BYTES + 1);
    let mut bytes = Vec::with_capacity(expected_size as usize);
    reader
        .read_to_end(&mut bytes)
        .map_err(|err| js_internal_error(format!("{api_name} failed: {err}")))?;
    ensure_read_file_size(bytes.len() as u64, api_name)?;
    Ok(bytes)
}

fn read_managed_file(lxapp: &LxApp, raw_path: &str, api_name: &'static str) -> JSResult<Vec<u8>> {
    let path = resolve_readable_path(lxapp, raw_path, api_name, "path")?;
    ensure_no_symlink_ancestors(lxapp, &path, api_name, "path")?;
    let metadata = symlink_metadata(&path, api_name)?;
    if !metadata.file_type().is_file() {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} path must reference a file"
        )));
    }
    mark_usercache_access(&path);
    read_file_bytes(&path.path, metadata.len(), api_name)
}

#[js_class(rename = "LxFile")]
impl JSLxFile {
    #[js_method(constructor, private)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(rong::error::E_ILLEGAL_CONSTRUCTOR, "Use lx.fs.file(path)").into())
    }

    /// The path supplied to `lx.fs.file`.
    #[js_method(getter)]
    fn path(&self) -> String {
        self.path.clone()
    }

    /// Read the complete file as strict UTF-8 text.
    #[js_method]
    async fn text(&self) -> JSResult<String> {
        let lxapp = self.lxapp()?;
        let bytes = read_managed_file(&lxapp, &self.path, "LxFile.text")?;
        String::from_utf8(bytes).map_err(|err| {
            js_invalid_parameter_error(format!("LxFile.text invalid utf8 data: {err}"))
        })
    }

    /// Read and parse the complete file as JSON. Stays `unknown`: a class
    /// method cannot carry a type parameter through the binding, so unlike
    /// `lx.getStorage().get<T>()` the assertion is spelled `as` at the call
    /// site rather than passed in.
    #[js_method(ts_return = "Promise<unknown>")]
    async fn json(&self, ctx: JSContext) -> JSResult<JSValue> {
        let lxapp = self.lxapp()?;
        let bytes = read_managed_file(&lxapp, &self.path, "LxFile.json")?;
        let text = String::from_utf8(bytes).map_err(|err| {
            js_invalid_parameter_error(format!("LxFile.json invalid utf8 data: {err}"))
        })?;
        text.as_str().json_to_js_value(&ctx)
    }

    /// Read the complete file as a Base64 string.
    #[js_method]
    async fn base64(&self) -> JSResult<String> {
        let lxapp = self.lxapp()?;
        let bytes = read_managed_file(&lxapp, &self.path, "LxFile.base64")?;
        Ok(general_purpose::STANDARD.encode(bytes))
    }

    /// Read the complete file as bytes.
    #[js_method]
    async fn bytes(&self, ctx: JSContext) -> JSResult<JSTypedArray> {
        let lxapp = self.lxapp()?;
        let bytes = read_managed_file(&lxapp, &self.path, "LxFile.bytes")?;
        let len = bytes.len();
        let buffer = JSArrayBuffer::from_bytes_owned(&ctx, bytes)?;
        JSTypedArray::from_array_buffer(&ctx, buffer, 0, Some(len))
    }

    /// Read the complete file as an ArrayBuffer.
    #[js_method(rename = "arrayBuffer")]
    async fn array_buffer(&self, ctx: JSContext) -> JSResult<JSArrayBuffer> {
        let lxapp = self.lxapp()?;
        let bytes = read_managed_file(&lxapp, &self.path, "LxFile.arrayBuffer")?;
        JSArrayBuffer::from_bytes_owned(&ctx, bytes)
    }

    /// Test whether this managed path currently exists.
    #[js_method]
    async fn exists(&self) -> JSResult<bool> {
        let lxapp = self.lxapp()?;
        match resolve_readable_path(&lxapp, &self.path, "LxFile.exists", "path") {
            Ok(path) => {
                if ensure_no_symlink_ancestors(&lxapp, &path, "LxFile.exists", "path").is_err() {
                    return Ok(false);
                }
                let exists = std::fs::symlink_metadata(&path.path).is_ok();
                if exists {
                    mark_usercache_access(&path);
                }
                Ok(exists)
            }
            Err(_) => Ok(false),
        }
    }

    /// Read metadata for this managed path.
    #[js_method]
    async fn stat(&self) -> JSResult<FileStats> {
        let lxapp = self.lxapp()?;
        let path = resolve_readable_path(&lxapp, &self.path, "LxFile.stat", "path")?;
        ensure_no_symlink_ancestors(&lxapp, &path, "LxFile.stat", "path")?;
        let metadata = symlink_metadata(&path, "LxFile.stat")?;
        mark_usercache_access(&path);
        Ok(file_stats(metadata))
    }
}

/// LingXia-managed file access, isolated to this lxapp's storage and
/// explicitly granted paths.
fn fs_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("fs") {
        Ok(namespace) => Ok(namespace),
        Err(_) => {
            let namespace = JSObject::new(ctx);
            lx.set("fs", namespace.clone())?;
            Ok(namespace)
        }
    }
}

/// Create a lazy reference to a LingXia-managed path.
///
/// Relative paths resolve under `lx.env.USER_DATA_PATH`. Creating a reference
/// does not require the path to exist.
fn fs_file(ctx: JSContext, path: String) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    // Validate the namespace and traversal rules now; existence is deliberately
    // checked only by operations on the lazy reference.
    let _ = resolve_readable_path(&lxapp, &path, "fs.file", "path")?;
    let class = Class::lookup::<JSLxFile>(&ctx)?;
    Ok(class.instance(JSLxFile::new(&lxapp, path)))
}

/// Test whether a managed path currently exists.
async fn fs_exists(ctx: JSContext, path: String) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    match resolve_readable_path(&lxapp, &path, "fs.exists", "path") {
        Ok(path) => {
            if ensure_no_symlink_ancestors(&lxapp, &path, "fs.exists", "path").is_err() {
                return Ok(false);
            }
            let exists = std::fs::symlink_metadata(&path.path).is_ok();
            if exists {
                mark_usercache_access(&path);
            }
            Ok(exists)
        }
        Err(_) => Ok(false),
    }
}

/// Read metadata for a managed path.
async fn fs_stat(ctx: JSContext, path: String) -> JSResult<FileStats> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let path = resolve_readable_path(&lxapp, &path, "fs.stat", "path")?;
    ensure_no_symlink_ancestors(&lxapp, &path, "fs.stat", "path")?;
    let metadata = symlink_metadata(&path, "fs.stat")?;
    mark_usercache_access(&path);
    Ok(file_stats(metadata))
}

/// The direct children of a managed directory.
async fn fs_read_dir(ctx: JSContext, path: String) -> JSResult<Vec<JSDirEntry>> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let path = resolve_readable_path(&lxapp, &path, "fs.readDir", "path")?;
    ensure_no_symlink_ancestors(&lxapp, &path, "fs.readDir", "path")?;
    if !symlink_metadata(&path, "fs.readDir")?.file_type().is_dir() {
        return Err(js_invalid_parameter_error(
            "fs.readDir path must reference a directory",
        ));
    }
    mark_usercache_access(&path);
    let mut entries = tokio_fs::read_dir(&path.path)
        .await
        .map_err(|err| js_internal_error(format!("fs.readDir failed: {err}")))?;
    let mut children = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|err| js_internal_error(format!("fs.readDir entry failed: {err}")))?
    {
        let file_type = tokio_fs::symlink_metadata(entry.path())
            .await
            .map(|metadata| metadata.file_type())
            .map_err(|err| js_internal_error(format!("fs.readDir file type failed: {err}")))?;
        children.push(JSDirEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            is_directory: file_type.is_dir(),
            is_symlink: file_type.is_symlink(),
        });
    }
    Ok(children)
}

/// Create a managed directory.
async fn fs_mkdir(
    ctx: JSContext,
    path: String,
    options: Optional<JSFsMkdirOptions>,
) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let path = resolve_writable_path(&lxapp, &path, "fs.mkdir", "path")?;
    ensure_no_symlink_ancestors(&lxapp, &path, "fs.mkdir", "path")?;
    if std::fs::symlink_metadata(&path.path)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
    {
        finish_write(&lxapp, &path);
        return Ok(());
    }
    if options.0.unwrap_or_default().recursive.unwrap_or(false) {
        std::fs::create_dir_all(&path.path)
    } else {
        std::fs::create_dir(&path.path)
    }
    .map_err(|err| js_internal_error(format!("fs.mkdir failed: {err}")))?;
    finish_write(&lxapp, &path);
    Ok(())
}

/// Write UTF-8 text or bytes to a managed file.
async fn fs_write(
    ctx: JSContext,
    path: String,
    data: JSValue,
    options: Optional<JSFsWriteOptions>,
) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let path = resolve_writable_path(&lxapp, &path, "fs.write", "path")?;
    ensure_no_symlink_ancestors(&lxapp, &path, "fs.write", "path")?;
    let options = options.0.unwrap_or_default();
    let overwrite = options.overwrite.unwrap_or(false);
    if !overwrite {
        ensure_not_exists(&path.path, "fs.write")?;
    }
    let bytes = js_value_to_bytes(data, options.encoding.as_deref(), "fs.write")?;
    ensure_write_quota(&lxapp, &path, bytes.len() as u64, None, false)?;
    storage::with_disk_pressure_recovery(
        &lxapp.user_cache_dir,
        bytes.len() as u64,
        &[path.path.as_path()],
        || storage::write_file_atomic(&bytes, &path.path, overwrite),
    )
    .map(|_| ())
    .map_err(|err| js_internal_error(format!("fs.write failed: {err}")))?;
    finish_write(&lxapp, &path);
    Ok(())
}

/// Copy a managed file.
async fn fs_copy(
    ctx: JSContext,
    source: String,
    destination: String,
    options: Optional<JSFsOverwriteOptions>,
) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let source = resolve_readable_path(&lxapp, &source, "fs.copy", "source")?;
    ensure_no_symlink_ancestors(&lxapp, &source, "fs.copy", "source")?;
    if !symlink_metadata(&source, "fs.copy")?.file_type().is_file() {
        return Err(js_invalid_parameter_error(
            "fs.copy source must reference a file",
        ));
    }
    mark_usercache_access(&source);
    let destination = resolve_writable_path(&lxapp, &destination, "fs.copy", "destination")?;
    ensure_no_symlink_ancestors(&lxapp, &destination, "fs.copy", "destination")?;
    let overwrite = options.0.unwrap_or_default().overwrite.unwrap_or(false);
    if !overwrite {
        ensure_not_exists(&destination.path, "fs.copy")?;
    }
    let incoming = std::fs::symlink_metadata(&source.path)
        .map_err(|err| js_internal_error(format!("fs.copy metadata failed: {err}")))?
        .len();
    ensure_write_quota(&lxapp, &destination, incoming, Some(&source), false)?;
    storage::with_disk_pressure_recovery(
        &lxapp.user_cache_dir,
        incoming,
        &[source.path.as_path(), destination.path.as_path()],
        || storage::copy_file_atomic_with_overwrite(&source.path, &destination.path, overwrite),
    )
    .map(|_| ())
    .map_err(|err| js_internal_error(format!("fs.copy failed: {err}")))?;
    finish_write(&lxapp, &destination);
    Ok(())
}

/// Rename or move a managed file or directory.
async fn fs_rename(
    ctx: JSContext,
    source: String,
    destination: String,
    options: Optional<JSFsOverwriteOptions>,
) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let source = resolve_managed_path(&lxapp, &source, "fs.rename", "source", true, true, true)?;
    let destination = resolve_writable_path(&lxapp, &destination, "fs.rename", "destination")?;
    ensure_no_symlink_ancestors(&lxapp, &source, "fs.rename", "source")?;
    ensure_no_symlink_ancestors(&lxapp, &destination, "fs.rename", "destination")?;
    let overwrite = options.0.unwrap_or_default().overwrite.unwrap_or(false);
    if source.path == destination.path {
        return Ok(());
    }
    if std::fs::symlink_metadata(&source.path).is_err() {
        return Err(js_error_from_business_code_with_detail(
            1003,
            "fs.rename source not found",
        ));
    }
    mark_usercache_access(&source);
    let incoming = storage::path_size(&source.path);
    ensure_write_quota(&lxapp, &destination, incoming, Some(&source), true)?;
    if std::fs::symlink_metadata(&destination.path).is_ok() {
        if !overwrite {
            return Err(js_error_from_business_code_with_detail(
                1002,
                "fs.rename destination already exists",
            ));
        }
        if !(symlink_metadata(&source, "fs.rename")?
            .file_type()
            .is_file()
            && symlink_metadata(&destination, "fs.rename")?
                .file_type()
                .is_file())
        {
            return Err(js_invalid_parameter_error(
                "fs.rename overwrite only supports file destinations",
            ));
        }
        storage::with_disk_pressure_recovery(
            &lxapp.user_cache_dir,
            incoming,
            &[source.path.as_path(), destination.path.as_path()],
            || storage::move_file_atomic_with_overwrite(&source.path, &destination.path, true),
        )
        .map_err(|err| js_internal_error(format!("fs.rename failed: {err}")))?;
        finish_write(&lxapp, &destination);
        return Ok(());
    }
    if let Some(parent) = destination.path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| js_internal_error(format!("fs.rename create dir failed: {err}")))?;
    }
    storage::with_disk_pressure_recovery(
        &lxapp.user_cache_dir,
        incoming,
        &[source.path.as_path(), destination.path.as_path()],
        || storage::move_file_atomic(&source.path, &destination.path),
    )
    .map_err(|err| js_internal_error(format!("fs.rename failed: {err}")))?;
    finish_write(&lxapp, &destination);
    Ok(())
}

/// Remove a managed file or directory.
async fn fs_remove(
    ctx: JSContext,
    path: String,
    options: Optional<JSFsRemoveOptions>,
) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let path = resolve_writable_path(&lxapp, &path, "fs.remove", "path")?;
    ensure_no_symlink_ancestors(&lxapp, &path, "fs.remove", "path")?;
    let metadata = symlink_metadata(&path, "fs.remove")?;
    if metadata.is_file() || metadata.file_type().is_symlink() {
        std::fs::remove_file(&path.path)
            .map_err(|err| js_internal_error(format!("fs.remove file failed: {err}")))?;
    } else if metadata.is_dir() {
        if options.0.unwrap_or_default().recursive.unwrap_or(false) {
            std::fs::remove_dir_all(&path.path)
        } else {
            std::fs::remove_dir(&path.path)
        }
        .map_err(|err| js_internal_error(format!("fs.remove directory failed: {err}")))?;
    } else {
        return Err(js_invalid_parameter_error(
            "fs.remove path must reference a file, symlink, or directory",
        ));
    }
    Ok(())
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_hidden_class::<JSDirEntry>()?;
    ctx.register_hidden_class::<JSLxFile>()?;
    register_file_api(ctx)?;
    register_fs_property(ctx)?;
    register_fs_api(ctx)?;
    download::init(ctx)?;
    upload::init(ctx)?;

    Ok(())
}

#[cfg(feature = "terminal")]
pub(crate) fn init_download(ctx: &JSContext) -> JSResult<()> {
    download::init(ctx)
}

rong::js_api! {
    fn register_file_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn openFile(ts_params = "options: OpenFileOptions", ts_return = "void") = open_file;
        fn chooseFile(
            ts_params = "options?: ChooseFileOptions",
            ts_return = "Promise<ChooseFileResult>"
        ) = choose_file;
        fn chooseDirectory(
            ts_params = "options?: ChooseDirectoryOptions",
            ts_return = "Promise<ChooseDirectoryResult>"
        ) = choose_directory;
    }
}

rong::js_api! {
    fn register_fs_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const fs: "FileSystemApi" = fs_namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_fs_api(ctx) {
        namespace FileSystemApi = fs_namespace(ctx)?;
        fn file(ts_params = "path: string", ts_return = "LxFile") = fs_file;
        fn exists(ts_params = "path: string") = fs_exists;
        fn stat(ts_params = "path: string") = fs_stat;
        fn readDir(
            ts_params = "path: string",
            ts_return = "Promise<DirEntry[]>"
        ) = fs_read_dir;
        fn mkdir(ts_params = "path: string, options?: FsMkdirOptions") = fs_mkdir;
        // Text only here; the byte overload is merged in `lingxia-types`, where
        // a second signature can express that bytes take no `encoding`.
        fn write(
            ts_params = "path: string, data: string, options?: FsWriteOptions"
        ) = fs_write;
        fn copy(
            ts_params = "source: string, destination: string, options?: FsCopyOptions"
        ) = fs_copy;
        fn rename(
            ts_params = "source: string, destination: string, options?: FsRenameOptions"
        ) = fs_rename;
        fn remove(ts_params = "path: string, options?: FsRemoveOptions") = fs_remove;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ManagedPathKind, READ_FILE_MAX_BYTES, ensure_managed_path_kind_allowed,
        ensure_read_file_size, resolve_relative_managed_path,
    };
    use std::path::Path;

    #[test]
    fn writable_relative_paths_resolve_under_userdata() {
        let user_data_dir = Path::new("sandbox").join("userdata");
        let resolved = resolve_relative_managed_path(
            &user_data_dir,
            "pages/home/index.js",
            "compressVideo",
            "outputPath",
        )
        .unwrap();

        assert_eq!(resolved, user_data_dir.join("pages/home/index.js"));
    }

    #[test]
    fn writable_relative_paths_reject_escape_syntax() {
        let user_data_dir = Path::new("sandbox").join("userdata");
        for path in [
            "../bundle.js",
            "pages/../../bundle.js",
            "..\\bundle.js",
            "C:\\bundle.js",
            "/bundle.js",
        ] {
            assert!(
                resolve_relative_managed_path(&user_data_dir, path, "compressVideo", "outputPath",)
                    .is_err(),
                "path should be rejected: {path}"
            );
        }
    }

    #[test]
    fn media_writable_policy_can_preserve_explicit_temp_outputs() {
        assert!(
            ensure_managed_path_kind_allowed(
                ManagedPathKind::Temp,
                "compressVideo",
                "outputPath",
                true,
                true,
            )
            .is_ok()
        );
        assert!(
            ensure_managed_path_kind_allowed(
                ManagedPathKind::Temp,
                "writeFile",
                "filePath",
                false,
                true,
            )
            .is_err()
        );
    }

    #[test]
    fn read_file_size_is_bounded() {
        assert!(ensure_read_file_size(READ_FILE_MAX_BYTES, "LxFile.bytes").is_ok());
        assert!(ensure_read_file_size(READ_FILE_MAX_BYTES + 1, "LxFile.bytes").is_err());
        assert!(ensure_read_file_size(u64::MAX, "LxFile.bytes").is_err());
    }
}
