use crate::i18n::{
    js_error_from_lxapp_error, js_error_from_platform_error, js_invalid_parameter_error,
};
use lingxia_service::media::SaveMediaRequest;
use lxapp::LxApp;
use rong::{JSContext, JSResult};

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace MediaApi = crate::media::media_namespace(ctx)?;
        fn saveToLibrary(ts_params = "path: string", ts_return = "Promise<void>") = save_to_library;
    }
}

/// Save a picture or a video to the system photo library; which one comes from
/// the file's extension, else its leading bytes.
///
/// @throws {E_INVALID_ARG} The file is neither.
async fn save_to_library(ctx: JSContext, path: String) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let resolved = lxapp
        .resolve_accessible_path(&path)
        .map_err(|err| js_error_from_lxapp_error(&err))?;
    let mime = crate::pick::mime_for_path(&resolved);
    let image = if mime.starts_with("image/") {
        true
    } else if mime.starts_with("video/") {
        false
    } else {
        return Err(js_invalid_parameter_error(format!(
            "saveToLibrary takes a picture or a video; '{path}' is {mime}"
        )));
    };
    let request = SaveMediaRequest {
        file_uri: resolved.to_string_lossy().into_owned(),
    };
    let saved = if image {
        lingxia_service::media::save_image_to_photos_album(&*lxapp.runtime, request).await
    } else {
        lingxia_service::media::save_video_to_photos_album(&*lxapp.runtime, request).await
    };
    saved.map_err(|e| js_error_from_platform_error(&e))
}
