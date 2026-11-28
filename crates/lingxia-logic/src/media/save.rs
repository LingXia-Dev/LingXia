use crate::i18n::js_error;
use lingxia_platform::traits::media_runtime::MediaRuntime;
use lingxia_service::media::SaveMediaRequest;
use lxapp::LxApp;
use rong::{JSContext, JSResult};

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace MediaApi = crate::media::media_namespace(ctx)?;
        fn saveToLibrary(ts_params = "path: ManagedPath", ts_return = "Promise<void>") = save_to_library;
    }
}

/// Save a picture or a video to the system photo library; which one comes from
/// the file's extension, else its leading bytes.
///
/// @supports media.saveToLibrary
/// @throws {E_NOT_FOUND} No file at the path.
/// @throws {E_INVALID_ARG} A path the lxapp may not read.
/// @throws {E_INVALID_DATA} The file is neither a picture nor a video the platform can read.
/// @throws {E_NOT_SUPPORTED} A desktop host, which has no system photo library.
async fn save_to_library(ctx: JSContext, path: String) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    // A path the lxapp may not read is refused everywhere, before the host's gate.
    lxapp
        .resolve_accessible_path(path.trim())
        .map_err(js_error)?;
    if !crate::capability::exposes(&ctx, "media.saveToLibrary") {
        return Err(rong::HostError::new(
            rong::error::E_NOT_SUPPORTED,
            "lx.media.saveToLibrary: this host has no photo library; check lx.supports('media.saveToLibrary') first",
        )
        .into());
    }
    let resolved = super::resolve_input_file(&lxapp, &path)?;
    let mime = crate::pick::mime_for_path(&resolved);
    let image = if mime.starts_with("image/") {
        true
    } else if mime.starts_with("video/") {
        false
    } else {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_DATA,
            format!("saveToLibrary takes a picture or a video; '{path}' is {mime}"),
        )
        .into());
    };
    let file_uri = resolved.to_string_lossy().into_owned();
    let request = SaveMediaRequest {
        file_uri: file_uri.clone(),
    };
    let saved = if image {
        lingxia_service::media::save_image_to_photos_album(&*lxapp.runtime, request).await
    } else {
        lingxia_service::media::save_video_to_photos_album(&*lxapp.runtime, request).await
    };
    saved.map_err(|error| {
        js_error(super::input_failure(error, || {
            if image {
                lxapp.runtime.get_image_info(&file_uri).map(drop)
            } else {
                lxapp.runtime.get_video_info(&file_uri).map(drop)
            }
        }))
    })
}
