use crate::i18n::{js_error_from_business_code_with_detail, js_internal_error};
use lxapp::{LxApp, lx};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult};

#[derive(FromJSObj)]
struct JSDownloadOptions {
    url: String,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSDownloadResult {
    #[rename = "tempFilePath"]
    temp_file_path: String,
    #[rename = "fileName"]
    file_name: String,
    #[rename = "mimeType"]
    mime_type: Option<String>,
    size: u64,
}

async fn download_file(ctx: JSContext, options: JSDownloadOptions) -> JSResult<JSDownloadResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let url = options.url.trim().to_string();
    if url.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "downloadFile requires url",
        ));
    }

    let request = lxapp::download_manager::UserCacheDownloadRequest {
        url,
        headers: Vec::new(),
    };
    let persistence = lxapp::download_manager::DownloadPersistence::new(
        lxapp.app_data_dir(),
        lxapp::download_manager::download_request_task_id(&request),
        lxapp::download_manager::DownloadOwner {
            kind: lxapp::download_manager::DownloadOwnerKind::LxApp,
            appid: lxapp.appid.clone(),
            page_path: None,
            tab_id: None,
        },
        true,
    );

    let result = lxapp::download_manager::download_to_user_cache(
        Some(persistence),
        &lxapp.user_cache_dir,
        request,
        Some(rong::get_user_agent()),
        |_| {},
    )
    .await
    .map_err(|e| js_internal_error(format!("download failed: {}", e.error)))?;

    let temp_file_path = lxapp
        .to_uri(&result.temp_path)
        .ok_or_else(|| js_internal_error("download failed to convert output path to lx:// uri"))?
        .into_string();

    Ok(JSDownloadResult {
        temp_file_path,
        file_name: result.file_name,
        mime_type: result.mime_type,
        size: result.size,
    })
}

pub(super) fn init(ctx: &JSContext) -> JSResult<()> {
    lx::register_js_api(ctx, "downloadFile", JSFunc::new(ctx, download_file)?)?;
    Ok(())
}
