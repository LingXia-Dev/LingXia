use crate::i18n::err_code_message;
use lingxia_messaging::{CallbackResult, get_callback, remove_callback};
use lingxia_platform::traits::media_interaction::{MediaInteraction, SaveMediaRequest};
use lxapp::{LxApp, lx};
use rong::{FromJSObj, JSContext, JSFunc, JSResult, RongJSError, error::HostError};

#[derive(FromJSObj)]
struct JSSaveMediaOptions {
    #[rename = "filePath"]
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
    let runtime = &lxapp.runtime;

    let resolved = lxapp
        .resolve_accessible_path(&options.file_path)
        .map_err(|err| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                format!("saveMedia path error: {}", err),
            ))
        })?;

    let (callback_id, receiver) = get_callback();
    let request = SaveMediaRequest {
        file_uri: resolved.to_string_lossy().into_owned(),
        callback_id,
    };

    let op = if image {
        runtime.save_image_to_photos_album(request)
    } else {
        runtime.save_video_to_photos_album(request)
    };

    if let Err(e) = op {
        let _ = remove_callback(callback_id);
        return Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            format!("saveMedia failed to start: {}", e),
        )));
    }

    let result = receiver.await.map_err(|_| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "saveMedia cancelled or failed",
        ))
    })?;

    match result {
        CallbackResult::Success(_) => Ok(()),
        CallbackResult::Error(code) => Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            err_code_message(code).unwrap_or_else(|| format!("saveMedia error: {}", code)),
        ))),
    }
}
