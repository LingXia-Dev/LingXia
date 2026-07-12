use crate::i18n::{js_error_from_lxapp_error, js_error_from_platform_error};
use lingxia_service::media::SaveMediaRequest;
use lxapp::LxApp;
use rong::{FromJSObject, JSContext, JSResult};

#[derive(FromJSObject)]
struct JSSaveMediaOptions {
    #[js_name = "filePath"]
    file_path: String,
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn saveImageToPhotosAlbum(ts_params = "options: SaveMediaOptions") = save_image_to_photos_album;
        fn saveVideoToPhotosAlbum(ts_params = "options: SaveMediaOptions") = save_video_to_photos_album;
    }
}

async fn save_image_to_photos_album(ctx: JSContext, options: JSSaveMediaOptions) -> JSResult<()> {
    save_media(ctx, options, true).await
}

async fn save_video_to_photos_album(ctx: JSContext, options: JSSaveMediaOptions) -> JSResult<()> {
    save_media(ctx, options, false).await
}

async fn save_media(ctx: JSContext, options: JSSaveMediaOptions, image: bool) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let resolved = lxapp
        .resolve_accessible_path(&options.file_path)
        .map_err(|err| js_error_from_lxapp_error(&err))?;

    let request = SaveMediaRequest {
        file_uri: resolved.to_string_lossy().into_owned(),
    };

    if image {
        lingxia_service::media::save_image_to_photos_album(&*lxapp.runtime, request)
            .await
            .map_err(|e| js_error_from_platform_error(&e))
    } else {
        lingxia_service::media::save_video_to_photos_album(&*lxapp.runtime, request)
            .await
            .map_err(|e| js_error_from_platform_error(&e))
    }
}
