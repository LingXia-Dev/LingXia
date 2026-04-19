use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error,
    js_error_from_platform_error, js_internal_error, js_invalid_parameter_error,
};
use lingxia_platform::traits::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogFilter, FileService, OpenFileRequest,
};
use lxapp::{LxApp, lx};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, function::Optional};
use std::path::{Path, PathBuf};

mod download;
mod storage;
mod upload;

#[derive(FromJSObj)]
struct JSOpenFileOptions {
    #[rename = "filePath"]
    file_path: String,
    #[rename = "fileType"]
    file_type: Option<String>,
    mode: Option<String>,
    #[rename = "showMenu"]
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
                &format!("{api_name} requires mode to be auto, review, or external"),
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
            &format!("{api_name} requires filePath"),
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
            if let Err(review_error) = lxapp.runtime.review_file(request.clone()).await {
                match lxapp.runtime.open_external(request).await {
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
        OpenFileMode::Review => lxapp
            .runtime
            .review_file(request)
            .await
            .map_err(|e| js_error_from_platform_error(&e)),
        OpenFileMode::External => lxapp
            .runtime
            .open_external(request)
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

#[derive(FromJSObj, Clone, Default)]
struct JSFileDialogFilter {
    name: Option<String>,
    extensions: Option<Vec<String>>,
}

#[derive(FromJSObj, Clone, Default)]
struct JSChooseFileOptions {
    multiple: Option<bool>,
    filters: Option<Vec<JSFileDialogFilter>>,
    #[rename = "defaultPath"]
    default_path: Option<String>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct ChooseFileResultObj {
    canceled: bool,
    paths: Vec<String>,
}

#[derive(FromJSObj, Clone, Default)]
struct JSChooseDirectoryOptions {
    #[rename = "defaultPath"]
    default_path: Option<String>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct ChooseDirectoryResultObj {
    canceled: bool,
    path: Option<String>,
}

#[derive(FromJSObj)]
struct JSSaveFileOptions {
    #[rename = "tempFilePath"]
    temp_file_path: String,
    #[rename = "filePath"]
    file_path: Option<String>,
    overwrite: Option<bool>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSSaveFileResult {
    #[rename = "filePath"]
    file_path: String,
    size: u64,
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

    let result = lxapp
        .runtime
        .choose_file(ChooseFileRequest {
            multiple: opts.multiple.unwrap_or(false),
            filters,
            title: None,
            default_path,
        })
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

    let result = lxapp
        .runtime
        .choose_directory(ChooseDirectoryRequest {
            title: None,
            default_path,
        })
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

fn default_save_file_name(source: &Path) -> String {
    source
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("file")
        .to_string()
}

fn resolve_save_destination(
    lxapp: &LxApp,
    source: &Path,
    raw_file_path: Option<&str>,
) -> JSResult<PathBuf> {
    let Some(raw_file_path) = raw_file_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(lxapp.user_data_dir.join(default_save_file_name(source)));
    };

    if raw_file_path.starts_with("lx://") {
        let resolved = lxapp
            .resolve_accessible_path(raw_file_path)
            .map_err(|err| js_error_from_lxapp_error(&err))?;
        if !resolved.starts_with(&lxapp.user_data_dir) {
            return Err(js_invalid_parameter_error(
                "saveFile filePath must target lx://userdata",
            ));
        }
        return Ok(resolved);
    }

    let path = Path::new(raw_file_path);
    if path.is_absolute() || raw_file_path.contains(':') || raw_file_path.contains('\\') {
        return Err(js_invalid_parameter_error(
            "saveFile filePath must be relative or lx://userdata",
        ));
    }
    if raw_file_path
        .split('/')
        .any(|segment| segment == "." || segment == "..")
    {
        return Err(js_invalid_parameter_error(
            "saveFile filePath must not contain '.' or '..'",
        ));
    }
    Ok(lxapp
        .user_data_dir
        .join(raw_file_path.trim_start_matches('/')))
}

fn copy_file(source: &Path, destination: &Path, overwrite: bool) -> JSResult<u64> {
    if destination.exists() && !overwrite {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "saveFile destination already exists",
        ));
    }
    storage::copy_file_atomic(source, destination, overwrite)
        .map_err(|err| js_internal_error(format!("saveFile copy failed: {err}")))
}

fn ensure_userdata_quota(lxapp: &LxApp, destination: &Path, incoming_bytes: u64) -> JSResult<()> {
    storage::ensure_userdata_quota(&lxapp.user_data_dir, destination, incoming_bytes)
        .and_then(|()| {
            storage::ensure_app_storage_quota(
                &lxapp.user_data_dir,
                &lxapp.user_cache_dir,
                destination,
                incoming_bytes,
                false,
            )
        })
        .map_err(|err| err.into_js_error())?;
    Ok(())
}

async fn save_file(ctx: JSContext, options: JSSaveFileOptions) -> JSResult<JSSaveFileResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    if !options.temp_file_path.trim().starts_with("lx://temp/") {
        return Err(js_invalid_parameter_error(
            "saveFile tempFilePath must be a lx://temp URI",
        ));
    }
    let source = lxapp
        .resolve_accessible_path(options.temp_file_path.trim())
        .map_err(|err| js_error_from_lxapp_error(&err))?;
    if !source.is_file() {
        return Err(js_invalid_parameter_error(
            "saveFile tempFilePath must reference a file",
        ));
    }
    let destination = resolve_save_destination(&lxapp, &source, options.file_path.as_deref())?;
    let source_size = std::fs::metadata(&source)
        .map_err(|err| js_internal_error(format!("saveFile metadata failed: {err}")))?
        .len();
    ensure_userdata_quota(&lxapp, &destination, source_size)?;
    let size = copy_file(&source, &destination, options.overwrite.unwrap_or(true))?;
    let file_path = lxapp
        .to_uri(&destination)
        .ok_or_else(|| js_internal_error("saveFile failed to convert output path to lx:// uri"))?
        .into_string();
    Ok(JSSaveFileResult { file_path, size })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    lx::register_js_api(ctx, "openFile", JSFunc::new(ctx, open_file)?)?;
    lx::register_js_api(ctx, "chooseFile", JSFunc::new(ctx, choose_file)?)?;
    lx::register_js_api(ctx, "chooseDirectory", JSFunc::new(ctx, choose_directory)?)?;
    lx::register_js_api(ctx, "saveFile", JSFunc::new(ctx, save_file)?)?;
    download::init(ctx)?;
    upload::init(ctx)?;

    Ok(())
}
