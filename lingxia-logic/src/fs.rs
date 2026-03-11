use crate::i18n::{
    js_error_from_business_code, js_error_from_business_code_with_detail,
    js_error_from_lxapp_error, js_error_from_platform_error, js_internal_error, js_timeout_error,
};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::traits::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogFilter, FileInteraction,
    OpenDocumentRequest,
};
use lxapp::{LxApp, lx};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, function::Optional};
use serde::Deserialize;

#[derive(FromJSObj)]
struct JSOpenDocumentOptions {
    #[rename = "filePath"]
    file_path: String,
    #[rename = "fileType"]
    file_type: Option<String>,
    /// Controls share/menu button visibility.
    #[rename = "showMenu"]
    show_menu: Option<bool>,
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

fn open_document(ctx: JSContext, options: JSOpenDocumentOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    if options.file_path.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "openDocument requires filePath",
        ));
    }

    let resolved_path = lxapp
        .resolve_accessible_path(&options.file_path)
        .map_err(|err| js_error_from_lxapp_error(&err))?;

    lxapp
        .runtime
        .open_document(OpenDocumentRequest {
            file_path: resolved_path.to_string_lossy().into_owned(),
            mime_type: map_file_type_to_mime(options.file_type),
            show_menu: options.show_menu,
        })
        .map_err(|e| js_error_from_platform_error(&e))
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
    title: Option<String>,
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
    title: Option<String>,
    #[rename = "defaultPath"]
    default_path: Option<String>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct ChooseDirectoryResultObj {
    canceled: bool,
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DialogCallbackPayload {
    canceled: bool,
    paths: Vec<String>,
}

fn normalize_extensions(raw: Option<Vec<String>>) -> Vec<String> {
    raw.unwrap_or_default()
        .into_iter()
        .map(|ext| ext.trim().trim_start_matches('.').to_lowercase())
        .filter(|ext| !ext.is_empty())
        .collect()
}

fn parse_dialog_payload(data: &str, api_name: &str) -> JSResult<DialogCallbackPayload> {
    serde_json::from_str::<DialogCallbackPayload>(data)
        .map_err(|e| js_internal_error(format!("{} invalid payload: {}", api_name, e)))
}

async fn choose_file(
    ctx: JSContext,
    options: Optional<JSChooseFileOptions>,
) -> JSResult<ChooseFileResultObj> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let opts = options.as_ref().cloned().unwrap_or_default();
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
    let (callback_id, receiver) = get_callback();

    lxapp
        .runtime
        .choose_file(ChooseFileRequest {
            multiple: opts.multiple.unwrap_or(false),
            filters,
            title: opts.title,
            default_path: opts.default_path,
            callback_id,
        })
        .map_err(|e| js_error_from_platform_error(&e))?;

    let data = match receiver
        .await
        .map_err(|_| js_timeout_error("chooseFile callback timed out"))?
    {
        CallbackResult::Success(data) => data,
        CallbackResult::Error(code) => return Err(js_error_from_business_code(code)),
    };
    let payload = parse_dialog_payload(&data, "chooseFile")?;
    if !payload.canceled && payload.paths.is_empty() {
        return Err(js_internal_error(
            "chooseFile invalid payload: non-canceled result must include at least one path",
        ));
    }

    Ok(ChooseFileResultObj {
        canceled: payload.canceled,
        paths: payload.paths,
    })
}

async fn choose_directory(
    ctx: JSContext,
    options: Optional<JSChooseDirectoryOptions>,
) -> JSResult<ChooseDirectoryResultObj> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let opts = options.as_ref().cloned().unwrap_or_default();
    let (callback_id, receiver) = get_callback();

    lxapp
        .runtime
        .choose_directory(ChooseDirectoryRequest {
            title: opts.title,
            default_path: opts.default_path,
            callback_id,
        })
        .map_err(|e| js_error_from_platform_error(&e))?;

    let data = match receiver
        .await
        .map_err(|_| js_timeout_error("chooseDirectory callback timed out"))?
    {
        CallbackResult::Success(data) => data,
        CallbackResult::Error(code) => return Err(js_error_from_business_code(code)),
    };
    let payload = parse_dialog_payload(&data, "chooseDirectory")?;
    if !payload.canceled && payload.paths.len() != 1 {
        return Err(js_internal_error(
            "chooseDirectory invalid payload: non-canceled result must include exactly one path",
        ));
    }
    let path = payload.paths.into_iter().next();

    Ok(ChooseDirectoryResultObj {
        canceled: payload.canceled,
        path,
    })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    lx::register_js_api(ctx, "openDocument", JSFunc::new(ctx, open_document)?)?;
    lx::register_js_api(ctx, "chooseFile", JSFunc::new(ctx, choose_file)?)?;
    lx::register_js_api(ctx, "chooseDirectory", JSFunc::new(ctx, choose_directory)?)?;
    Ok(())
}
