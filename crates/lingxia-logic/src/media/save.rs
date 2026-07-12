use crate::i18n::{js_error_from_lxapp_error, js_error_from_platform_error};
use lingxia_service::media::SaveMediaRequest;
use lxapp::{LxApp, lx};
use rong::{FromJSObject, JSContext, JSFunc, JSResult};

#[derive(FromJSObject)]
struct JSSaveMediaOptions {
    #[js_name = "filePath"]
    file_path: String,
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let save_image_func = JSFunc::new(ctx, save_image_to_photos_album)?;
    lx::register_js_api(ctx, "saveImageToPhotosAlbum", save_image_func)?;

    let save_video_func = JSFunc::new(ctx, save_video_to_photos_album)?;
    lx::register_js_api(ctx, "saveVideoToPhotosAlbum", save_video_func)?;
    Ok(())
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
