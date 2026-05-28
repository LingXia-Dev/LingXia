use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error,
    js_error_from_platform_error, js_invalid_parameter_error,
};
use lingxia_platform::traits::share::{
    ShareRequest, ShareResult as PlatformShareResult, ShareService,
};
use lxapp::{LxApp, LxAppError, lx};
use rong::{IntoJSObj, JSArray, JSContext, JSFunc, JSObject, JSResult, JSValue};
use serde_json::Value;

struct JSShareOptions {
    title: Option<String>,
    text: Option<String>,
    page: Option<JSSharePage>,
    files: Vec<String>,
}

struct JSSharePage {
    query: Option<JSObject>,
}

#[derive(IntoJSObj)]
struct JSShareResult {
    completed: Option<bool>,
}

async fn share(ctx: JSContext, options: JSValue) -> JSResult<JSShareResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let options = parse_share_options(options)?;
    let title = non_empty(options.title);
    let text = non_empty(options.text);
    if !options.files.is_empty() && options.page.is_some() {
        return Err(js_invalid_parameter_error(
            "share page and files cannot be used together",
        ));
    }
    if !options.files.is_empty() && text.is_some() {
        return Err(js_invalid_parameter_error(
            "share files does not support text; share text separately",
        ));
    }
    let files = resolve_share_files(&lxapp, options.files)?;
    let url = options
        .page
        .map(|page| build_page_share_url(&lxapp, page))
        .transpose()?;

    if title.is_none() && text.is_none() && url.is_none() && files.is_empty() {
        return Err(js_invalid_parameter_error(
            "share requires title, text, page, or files",
        ));
    }

    let result = lxapp
        .runtime
        .share(ShareRequest {
            title,
            text,
            url,
            files,
        })
        .await
        .map_err(|e| js_error_from_platform_error(&e))?;

    Ok(result.into())
}

fn parse_share_options(value: JSValue) -> JSResult<JSShareOptions> {
    let Some(obj) = value.into_object() else {
        return Err(js_invalid_parameter_error(
            "share options must be an object",
        ));
    };
    if get_present_property(&obj, "query").is_some() {
        return Err(js_invalid_parameter_error(
            "share page query must be nested under page.query",
        ));
    }
    Ok(JSShareOptions {
        title: read_optional_string(&obj, "title")?,
        text: read_optional_string(&obj, "text")?,
        page: read_optional_page_target(&obj)?,
        files: read_optional_files(&obj)?,
    })
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn build_page_share_url(lxapp: &LxApp, page: JSSharePage) -> JSResult<String> {
    let current_page = lxapp.peek_current_page().ok_or_else(|| {
        js_error_from_lxapp_error(&LxAppError::Runtime("No current page found".to_string()))
    })?;
    let page_url = append_query(current_page, page.query.as_ref())
        .map_err(|e| js_error_from_lxapp_error(&e))?;
    lxapp
        .ensure_page_exists(&page_url)
        .map_err(|e| js_error_from_lxapp_error(&e))?;

    let host = first_app_link_host()?;
    let (path, raw_query) = lxapp::startup::split_path_query(&page_url);
    validate_share_page_query(raw_query.as_deref())?;
    let mut pairs = vec![
        format!("appId={}", urlencoding::encode(&lxapp.appid)),
        format!("path={}", urlencoding::encode(path.trim_start_matches('/'))),
    ];
    if let Some(env_version) = app_link_env_version(lxapp) {
        pairs.push(format!("envVersion={env_version}"));
    }
    if let Some(raw_query) = raw_query.filter(|value| !value.is_empty()) {
        pairs.push(raw_query);
    }
    Ok(format!("https://{host}/lxapp/open?{}", pairs.join("&")))
}

fn get_present_property(obj: &JSObject, field: &str) -> Option<JSValue> {
    obj.get::<_, JSValue>(field)
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
}

fn read_optional_string(obj: &JSObject, field: &str) -> JSResult<Option<String>> {
    let Some(value) = get_present_property(obj, field) else {
        return Ok(None);
    };
    if !value.is_string() {
        return Err(js_invalid_parameter_error(format!(
            "share {field} must be a string"
        )));
    }
    value
        .to_rust::<String>()
        .map(Some)
        .map_err(|_| js_invalid_parameter_error(format!("share {field} must be a string")))
}

fn read_optional_page_target(obj: &JSObject) -> JSResult<Option<JSSharePage>> {
    let Some(value) = get_present_property(obj, "page") else {
        return Ok(None);
    };

    if value.is_boolean() {
        let enabled = value
            .to_rust::<bool>()
            .map_err(|_| js_invalid_parameter_error("share page must be true or an object"))?;
        return Ok(enabled.then_some(JSSharePage { query: None }));
    }

    let Some(page_obj) = value.into_object() else {
        return Err(js_invalid_parameter_error(
            "share page must be true or an object",
        ));
    };
    if get_present_property(&page_obj, "path").is_some()
        || get_present_property(&page_obj, "name").is_some()
    {
        return Err(js_invalid_parameter_error(
            "share page only supports the current page; use page.query for query parameters",
        ));
    }
    let query = read_optional_query(&page_obj)?;
    Ok(Some(JSSharePage { query }))
}

fn read_optional_query(obj: &JSObject) -> JSResult<Option<JSObject>> {
    let Some(value) = get_present_property(obj, "query") else {
        return Ok(None);
    };
    value
        .into_object()
        .map(Some)
        .ok_or_else(|| js_invalid_parameter_error("share page.query must be an object"))
}

fn read_optional_files(obj: &JSObject) -> JSResult<Vec<String>> {
    let Some(value) = get_present_property(obj, "files") else {
        return Ok(Vec::new());
    };
    let values: Vec<JSValue> = value
        .into_object()
        .and_then(JSArray::from_object)
        .ok_or_else(|| js_invalid_parameter_error("share files must be an array"))?
        .iter_values()?
        .collect::<JSResult<Vec<_>>>()?;
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            if !value.is_string() {
                return Err(js_invalid_parameter_error(format!(
                    "share files[{index}] must be a string"
                )));
            }
            value.to_rust::<String>().map_err(|_| {
                js_invalid_parameter_error(format!("share files[{index}] must be a string"))
            })
        })
        .collect()
}

fn append_query(path: String, query: Option<&JSObject>) -> Result<String, LxAppError> {
    let Some(query) = query else {
        return Ok(path);
    };
    let query_json = query.to_json_string().map_err(LxAppError::from)?;
    let query: Value = serde_json::from_str(&query_json)?;
    lxapp::append_page_query(path, &query).map_err(LxAppError::InvalidParameter)
}

fn validate_share_page_query(raw_query: Option<&str>) -> JSResult<()> {
    let Some(raw_query) = raw_query else {
        return Ok(());
    };
    for pair in raw_query.split('&').filter(|pair| !pair.is_empty()) {
        let raw_key = pair.split_once('=').map(|(key, _)| key).unwrap_or(pair);
        let key = urlencoding::decode(raw_key)
            .map(|value| value.to_string())
            .map_err(|_| js_invalid_parameter_error("share page query has invalid encoding"))?;
        if matches!(key.as_str(), "appId" | "appid" | "path" | "envVersion") {
            return Err(js_invalid_parameter_error(format!(
                "share page query key is reserved: {key}"
            )));
        }
    }
    Ok(())
}

fn app_link_env_version(lxapp: &LxApp) -> Option<&'static str> {
    match lxapp.release_type() {
        lxapp::ReleaseType::Release => None,
        lxapp::ReleaseType::Preview => Some("preview"),
        lxapp::ReleaseType::Developer => Some("develop"),
    }
}

fn first_app_link_host() -> JSResult<String> {
    let config = lingxia_app_context::app_config().ok_or_else(|| share_page_unsupported_error())?;
    let host = config
        .app_links
        .as_ref()
        .and_then(|links| links.hosts.iter().find(|host| !host.trim().is_empty()))
        .map(|host| host.trim().to_string())
        .ok_or_else(share_page_unsupported_error)?;
    Ok(host)
}

fn share_page_unsupported_error() -> rong::RongJSError {
    js_error_from_business_code_with_detail(
        6000,
        "share page requires appLinks.hosts configuration",
    )
}

fn resolve_share_files(lxapp: &LxApp, files: Vec<String>) -> JSResult<Vec<String>> {
    files
        .into_iter()
        .map(|file| {
            let path = file.trim();
            if path.is_empty() {
                return Err(js_invalid_parameter_error("share file path is required"));
            }
            if is_platform_file_reference(path) && lxapp.has_transient_file_reference(path) {
                return Ok(path.to_string());
            }
            if is_platform_file_reference(path) {
                return Err(js_invalid_parameter_error(
                    "share file path must be returned by lx.chooseFile/lx.chooseMedia",
                ));
            }
            if !path.starts_with("lx://") {
                return Err(js_invalid_parameter_error(
                    "share file path must be returned by lx.chooseFile/lx.chooseMedia",
                ));
            }
            let resolved = lxapp
                .resolve_accessible_path(path)
                .map_err(|e| js_error_from_lxapp_error(&e))?;
            let metadata = std::fs::metadata(&resolved)
                .map_err(|e| js_invalid_parameter_error(e.to_string()))?;
            if !metadata.is_file() {
                return Err(js_invalid_parameter_error(format!(
                    "share file path is not a file: {}",
                    resolved.display()
                )));
            }
            Ok(resolved.to_string_lossy().to_string())
        })
        .collect()
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

impl From<PlatformShareResult> for JSShareResult {
    fn from(value: PlatformShareResult) -> Self {
        Self {
            completed: value.completed,
        }
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let share_func = JSFunc::new(ctx, |ctx, options| async move { share(ctx, options).await })?;
    lx::register_js_api(ctx, "share", share_func)?;
    Ok(())
}
