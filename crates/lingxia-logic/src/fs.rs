use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_platform_error, js_internal_error,
};
use lingxia_platform::traits::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogFilter, FileService, OpenFileRequest,
};
use lxapp::{LxApp, lx};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, function::Optional};

mod download;
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

    Ok(ChooseFileResultObj {
        canceled: result.canceled,
        paths: result.paths,
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

    Ok(ChooseDirectoryResultObj {
        canceled: result.canceled,
        path: result.paths.into_iter().next(),
    })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    lx::register_js_api(ctx, "openFile", JSFunc::new(ctx, open_file)?)?;
    lx::register_js_api(ctx, "chooseFile", JSFunc::new(ctx, choose_file)?)?;
    lx::register_js_api(ctx, "chooseDirectory", JSFunc::new(ctx, choose_directory)?)?;
    download::init(ctx)?;
    upload::init(ctx)?;

    Ok(())
}
