use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error,
    js_error_from_platform_error, js_internal_error, js_invalid_parameter_error,
};
use base64::{Engine as _, engine::general_purpose};
use futures::Stream;
use lingxia_service::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogFilter, OpenFileRequest,
};
use lxapp::LxApp;
use rong::{
    AnyJSTypedArray, Class, FromJSObject, HostError, IntoJSAsyncIteratorExt, IntoJSObject,
    IntoJSValue, JSArrayBuffer, JSContext, JSObject, JSResult, JSValue, RongJSError,
    function::Optional, js_class, js_method,
};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs as tokio_fs;

mod download;
mod network_security;
mod storage;
mod upload;

#[derive(FromJSObject)]
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

async fn open_file(ctx: JSContext, options: JSOpenFileOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let mode = OpenFileMode::parse(options.mode.as_deref(), "openFile")?;
    let request = resolve_open_file_request(&lxapp, &options, "openFile")?;
    open_file_with_mode(&lxapp, request, mode).await
}

#[derive(FromJSObject, Clone, Default)]
struct JSFileDialogFilter {
    name: Option<String>,
    extensions: Option<Vec<String>>,
}

#[derive(FromJSObject, Clone, Default)]
struct JSChooseFileOptions {
    multiple: Option<bool>,
    filters: Option<Vec<JSFileDialogFilter>>,
    #[js_name = "defaultPath"]
    default_path: Option<String>,
}

#[derive(Debug, Clone, IntoJSObject)]
struct ChooseFileResultObj {
    canceled: bool,
    paths: Vec<String>,
}

#[derive(FromJSObject, Clone, Default)]
struct JSChooseDirectoryOptions {
    #[js_name = "defaultPath"]
    default_path: Option<String>,
}

#[derive(Debug, Clone, IntoJSObject)]
struct ChooseDirectoryResultObj {
    canceled: bool,
    path: Option<String>,
}

#[derive(FromJSObject)]
struct JSFsPathOptions {
    path: String,
}

#[derive(FromJSObject)]
struct JSFsDirPathOptions {
    path: String,
}

#[derive(FromJSObject)]
struct JSMkdirOptions {
    path: String,
    recursive: Option<bool>,
}

#[derive(FromJSObject)]
struct JSReadFileOptions {
    #[js_name = "filePath"]
    file_path: String,
    encoding: Option<String>,
}

#[derive(FromJSObject)]
struct JSWriteFileOptions {
    #[js_name = "filePath"]
    file_path: String,
    data: JSValue,
    encoding: Option<String>,
    overwrite: Option<bool>,
}

#[derive(FromJSObject)]
struct JSCopyFileOptions {
    #[js_name = "srcPath"]
    src_path: String,
    #[js_name = "destPath"]
    dest_path: String,
    overwrite: Option<bool>,
}

#[derive(FromJSObject)]
struct JSRenameOptions {
    #[js_name = "oldPath"]
    old_path: String,
    #[js_name = "newPath"]
    new_path: String,
    overwrite: Option<bool>,
}

#[derive(FromJSObject)]
struct JSRemoveOptions {
    path: String,
    recursive: Option<bool>,
}

#[derive(Debug, Clone, IntoJSObject)]
struct JSFileStats {
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
struct JSFileManager {
    lxapp: Weak<LxApp>,
    user_data_dir: PathBuf,
}

impl JSFileManager {
    fn new(lxapp: &Arc<LxApp>) -> Self {
        Self {
            lxapp: Arc::downgrade(lxapp),
            user_data_dir: lxapp.user_data_dir.clone(),
        }
    }

    fn lxapp(&self) -> JSResult<Arc<LxApp>> {
        let lxapp = self
            .lxapp
            .upgrade()
            .ok_or_else(|| js_internal_error("FileManager owner LxApp has been released"))?;
        if lxapp.user_data_dir != self.user_data_dir {
            return Err(js_internal_error("FileManager owner LxApp changed"));
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
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use FileManager.readDir()",
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

type FileTypeFuture =
    Pin<Box<dyn futures::Future<Output = Result<std::fs::FileType, std::io::Error>> + Send>>;

struct DirEntryStream {
    entries: tokio_fs::ReadDir,
    current_entry: Option<tokio_fs::DirEntry>,
    current_file_type_fut: Option<FileTypeFuture>,
}

impl DirEntryStream {
    fn new(entries: tokio_fs::ReadDir) -> Self {
        Self {
            entries,
            current_entry: None,
            current_file_type_fut: None,
        }
    }
}

impl Stream for DirEntryStream {
    type Item = Result<JSDirEntry, RongJSError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(file_type_fut) = this.current_file_type_fut.as_mut() {
            match file_type_fut.as_mut().poll(cx) {
                Poll::Ready(Ok(file_type)) => {
                    this.current_file_type_fut.take();
                    if let Some(entry) = this.current_entry.take() {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        return Poll::Ready(Some(Ok(JSDirEntry {
                            name,
                            is_directory: file_type.is_dir(),
                            is_symlink: file_type.is_symlink(),
                        })));
                    }
                }
                Poll::Ready(Err(err)) => {
                    this.current_file_type_fut.take();
                    this.current_entry.take();
                    return Poll::Ready(Some(Err(js_internal_error(format!(
                        "readDir file type failed: {err}"
                    )))));
                }
                Poll::Pending => return Poll::Pending,
            }
        }

        match this.entries.poll_next_entry(cx) {
            Poll::Ready(Ok(Some(entry))) => {
                let path = entry.path();
                this.current_entry = Some(entry);
                this.current_file_type_fut = Some(Box::pin(async move {
                    tokio_fs::symlink_metadata(path)
                        .await
                        .map(|metadata| metadata.file_type())
                }));
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Poll::Ready(Ok(None)) => Poll::Ready(None),
            Poll::Ready(Err(err)) => Poll::Ready(Some(Err(js_internal_error(format!(
                "readDir entry failed: {err}"
            ))))),
            Poll::Pending => Poll::Pending,
        }
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

async fn choose_file(
    ctx: JSContext,
    options: Optional<JSChooseFileOptions>,
) -> JSResult<ChooseFileResultObj> {
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

    if !result.canceled && result.paths.is_empty() {
        return Err(js_internal_error(
            "chooseFile invalid payload: non-canceled result must include at least one path",
        ));
    }

    let paths = result
        .paths
        .iter()
        .map(|path| selected_file_path_to_uri(&lxapp, path))
        .collect::<JSResult<Vec<_>>>()?;

    Ok(ChooseFileResultObj {
        canceled: result.canceled,
        paths,
    })
}

async fn choose_directory(
    ctx: JSContext,
    options: Optional<JSChooseDirectoryOptions>,
) -> JSResult<ChooseDirectoryResultObj> {
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

    if !result.canceled && result.paths.len() != 1 {
        return Err(js_internal_error(
            "chooseDirectory invalid payload: non-canceled result must include exactly one path",
        ));
    }

    let path = result
        .paths
        .into_iter()
        .next()
        .map(|path| selected_directory_path_to_uri(&lxapp, &path))
        .transpose()?;

    Ok(ChooseDirectoryResultObj {
        canceled: result.canceled,
        path,
    })
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

    let relative = normalize_relative_path(path, api_name, field_name)?;
    Ok(ManagedPath {
        path: lxapp.user_data_dir.join(relative),
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

fn file_stats(metadata: std::fs::Metadata) -> JSFileStats {
    let file_type = metadata.file_type();
    JSFileStats {
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
            Err(_) => break,
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

fn bytes_to_read_file_result(
    ctx: &JSContext,
    bytes: Vec<u8>,
    encoding: Option<&str>,
) -> JSResult<JSObject> {
    let result = JSObject::new(ctx);
    match decode_encoding(encoding, "readFile")? {
        Some("base64") => {
            result.set("data", general_purpose::STANDARD.encode(bytes))?;
        }
        Some("utf8") => {
            let text = String::from_utf8(bytes).map_err(|err| {
                js_invalid_parameter_error(format!("readFile invalid utf8 data: {err}"))
            })?;
            result.set("data", text)?;
        }
        None => {
            let buffer = JSArrayBuffer::from_bytes_owned(ctx, bytes)?;
            result.set("data", buffer.into_js_value(ctx))?;
        }
        _ => unreachable!(),
    }
    Ok(result)
}

#[js_class(rename = "FileManager")]
impl JSFileManager {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.getFileManager()",
        )
        .into())
    }

    #[js_method]
    async fn exists(&self, _ctx: JSContext, options: JSFsPathOptions) -> JSResult<bool> {
        let lxapp = self.lxapp()?;
        match resolve_readable_path(&lxapp, &options.path, "exists", "path") {
            Ok(path) => {
                if ensure_no_symlink_ancestors(&lxapp, &path, "exists", "path").is_err() {
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

    #[js_method]
    async fn stat(&self, _ctx: JSContext, options: JSFsPathOptions) -> JSResult<JSFileStats> {
        let lxapp = self.lxapp()?;
        let path = resolve_readable_path(&lxapp, &options.path, "stat", "path")?;
        ensure_no_symlink_ancestors(&lxapp, &path, "stat", "path")?;
        let metadata = symlink_metadata(&path, "stat")?;
        mark_usercache_access(&path);
        Ok(file_stats(metadata))
    }

    #[js_method(rename = "readDir")]
    async fn read_dir(&self, ctx: JSContext, options: JSFsDirPathOptions) -> JSResult<JSObject> {
        let lxapp = self.lxapp()?;
        let path = resolve_readable_path(&lxapp, &options.path, "readDir", "path")?;
        ensure_no_symlink_ancestors(&lxapp, &path, "readDir", "path")?;
        if !symlink_metadata(&path, "readDir")?.file_type().is_dir() {
            return Err(js_invalid_parameter_error(
                "readDir path must reference a directory",
            ));
        }
        mark_usercache_access(&path);
        let entries = tokio_fs::read_dir(&path.path)
            .await
            .map_err(|err| js_internal_error(format!("readDir failed: {err}")))?;
        DirEntryStream::new(entries).to_js_async_iter(&ctx)
    }

    #[js_method]
    async fn mkdir(&self, _ctx: JSContext, options: JSMkdirOptions) -> JSResult<()> {
        let lxapp = self.lxapp()?;
        let path = resolve_writable_path(&lxapp, &options.path, "mkdir", "path")?;
        ensure_no_symlink_ancestors(&lxapp, &path, "mkdir", "path")?;
        if std::fs::symlink_metadata(&path.path)
            .map(|metadata| metadata.file_type().is_dir())
            .unwrap_or(false)
        {
            finish_write(&lxapp, &path);
            return Ok(());
        }
        if options.recursive.unwrap_or(false) {
            std::fs::create_dir_all(&path.path)
        } else {
            std::fs::create_dir(&path.path)
        }
        .map_err(|err| js_internal_error(format!("mkdir failed: {err}")))?;
        finish_write(&lxapp, &path);
        Ok(())
    }

    #[js_method(rename = "readFile")]
    async fn read_file(&self, ctx: JSContext, options: JSReadFileOptions) -> JSResult<JSObject> {
        let lxapp = self.lxapp()?;
        let path = resolve_readable_path(&lxapp, &options.file_path, "readFile", "filePath")?;
        ensure_no_symlink_ancestors(&lxapp, &path, "readFile", "filePath")?;
        if !symlink_metadata(&path, "readFile")?.file_type().is_file() {
            return Err(js_invalid_parameter_error(
                "readFile filePath must reference a file",
            ));
        }
        mark_usercache_access(&path);
        let bytes = std::fs::read(&path.path)
            .map_err(|err| js_internal_error(format!("readFile failed: {err}")))?;
        bytes_to_read_file_result(&ctx, bytes, options.encoding.as_deref())
    }

    #[js_method(rename = "writeFile")]
    async fn write_file(&self, _ctx: JSContext, options: JSWriteFileOptions) -> JSResult<()> {
        let lxapp = self.lxapp()?;
        let path = resolve_writable_path(&lxapp, &options.file_path, "writeFile", "filePath")?;
        ensure_no_symlink_ancestors(&lxapp, &path, "writeFile", "filePath")?;
        let overwrite = options.overwrite.unwrap_or(false);
        if !overwrite {
            ensure_not_exists(&path.path, "writeFile")?;
        }
        let bytes = js_value_to_bytes(options.data, options.encoding.as_deref(), "writeFile")?;
        ensure_write_quota(&lxapp, &path, bytes.len() as u64, None, false)?;
        storage::with_disk_pressure_recovery(
            &lxapp.user_cache_dir,
            bytes.len() as u64,
            &[path.path.as_path()],
            || storage::write_file_atomic(&bytes, &path.path, overwrite),
        )
        .map(|_| ())
        .map_err(|err| js_internal_error(format!("writeFile failed: {err}")))?;
        finish_write(&lxapp, &path);
        Ok(())
    }

    #[js_method(rename = "copyFile")]
    async fn copy_file(&self, _ctx: JSContext, options: JSCopyFileOptions) -> JSResult<()> {
        let lxapp = self.lxapp()?;
        let source = resolve_readable_path(&lxapp, &options.src_path, "copyFile", "srcPath")?;
        ensure_no_symlink_ancestors(&lxapp, &source, "copyFile", "srcPath")?;
        if !symlink_metadata(&source, "copyFile")?.file_type().is_file() {
            return Err(js_invalid_parameter_error(
                "copyFile srcPath must reference a file",
            ));
        }
        mark_usercache_access(&source);
        let destination =
            resolve_writable_path(&lxapp, &options.dest_path, "copyFile", "destPath")?;
        ensure_no_symlink_ancestors(&lxapp, &destination, "copyFile", "destPath")?;
        let overwrite = options.overwrite.unwrap_or(false);
        if !overwrite {
            ensure_not_exists(&destination.path, "copyFile")?;
        }
        let incoming = std::fs::symlink_metadata(&source.path)
            .map_err(|err| js_internal_error(format!("copyFile metadata failed: {err}")))?
            .len();
        ensure_write_quota(&lxapp, &destination, incoming, Some(&source), false)?;
        storage::with_disk_pressure_recovery(
            &lxapp.user_cache_dir,
            incoming,
            &[source.path.as_path(), destination.path.as_path()],
            || storage::copy_file_atomic_with_overwrite(&source.path, &destination.path, overwrite),
        )
        .map(|_| ())
        .map_err(|err| js_internal_error(format!("copyFile failed: {err}")))?;
        finish_write(&lxapp, &destination);
        Ok(())
    }

    #[js_method]
    async fn rename(&self, _ctx: JSContext, options: JSRenameOptions) -> JSResult<()> {
        let lxapp = self.lxapp()?;
        let old_path = resolve_managed_path(
            &lxapp,
            &options.old_path,
            "rename",
            "oldPath",
            true,
            true,
            true,
        )?;
        let new_path = resolve_writable_path(&lxapp, &options.new_path, "rename", "newPath")?;
        ensure_no_symlink_ancestors(&lxapp, &old_path, "rename", "oldPath")?;
        ensure_no_symlink_ancestors(&lxapp, &new_path, "rename", "newPath")?;
        let overwrite = options.overwrite.unwrap_or(false);
        if old_path.path == new_path.path {
            return Ok(());
        }
        if std::fs::symlink_metadata(&old_path.path).is_err() {
            return Err(js_error_from_business_code_with_detail(
                1003,
                "rename oldPath not found",
            ));
        }
        mark_usercache_access(&old_path);
        let incoming = storage::path_size(&old_path.path);
        ensure_write_quota(&lxapp, &new_path, incoming, Some(&old_path), true)?;
        if std::fs::symlink_metadata(&new_path.path).is_ok() {
            if !overwrite {
                return Err(js_error_from_business_code_with_detail(
                    1002,
                    "rename destination already exists",
                ));
            }
            if !(symlink_metadata(&old_path, "rename")?.file_type().is_file()
                && symlink_metadata(&new_path, "rename")?.file_type().is_file())
            {
                return Err(js_invalid_parameter_error(
                    "rename overwrite only supports file destinations",
                ));
            }
            storage::with_disk_pressure_recovery(
                &lxapp.user_cache_dir,
                incoming,
                &[old_path.path.as_path(), new_path.path.as_path()],
                || storage::move_file_atomic_with_overwrite(&old_path.path, &new_path.path, true),
            )
            .map_err(|err| js_internal_error(format!("rename failed: {err}")))?;
            finish_write(&lxapp, &new_path);
            return Ok(());
        }
        if let Some(parent) = new_path.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| js_internal_error(format!("rename create dir failed: {err}")))?;
        }
        storage::with_disk_pressure_recovery(
            &lxapp.user_cache_dir,
            incoming,
            &[old_path.path.as_path(), new_path.path.as_path()],
            || storage::move_file_atomic(&old_path.path, &new_path.path),
        )
        .map_err(|err| js_internal_error(format!("rename failed: {err}")))?;
        finish_write(&lxapp, &new_path);
        Ok(())
    }

    #[js_method]
    async fn remove(&self, _ctx: JSContext, options: JSRemoveOptions) -> JSResult<()> {
        let lxapp = self.lxapp()?;
        let path = resolve_writable_path(&lxapp, &options.path, "remove", "path")?;
        ensure_no_symlink_ancestors(&lxapp, &path, "remove", "path")?;
        let metadata = symlink_metadata(&path, "remove")?;
        if metadata.is_file() || metadata.file_type().is_symlink() {
            std::fs::remove_file(&path.path)
                .map_err(|err| js_internal_error(format!("remove file failed: {err}")))?;
        } else if metadata.is_dir() {
            if options.recursive.unwrap_or(false) {
                std::fs::remove_dir_all(&path.path)
            } else {
                std::fs::remove_dir(&path.path)
            }
            .map_err(|err| js_internal_error(format!("remove directory failed: {err}")))?;
        } else {
            return Err(js_invalid_parameter_error(
                "remove path must reference a file, symlink, or directory",
            ));
        }
        Ok(())
    }
}

fn get_file_manager(ctx: JSContext) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let class = Class::lookup::<JSFileManager>(&ctx)?;
    Ok(class.instance(JSFileManager::new(&lxapp)))
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_hidden_class::<JSDirEntry>()?;
    ctx.register_hidden_class::<JSFileManager>()?;
    register_file_api(ctx)?;
    download::init(ctx)?;
    upload::init(ctx)?;

    Ok(())
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
        fn getFileManager(ts_return = "PublicFileManager") = get_file_manager;
    }
}
