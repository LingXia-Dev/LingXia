use lingxia_platform::{
    MediaInteraction, SaveMediaRequest, ToastIcon, ToastOptions, ToastPosition, UserFeedback,
};
use lxapp::{LxApp, lx};
use rong::{FromJSObj, JSContext, JSFunc, JSResult, RongJSError};

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

fn save_image_to_photos_album(ctx: JSContext, options: JSSaveMediaOptions) -> JSResult<()> {
    save_media(ctx, options, true)
}

fn save_video_to_photos_album(ctx: JSContext, options: JSSaveMediaOptions) -> JSResult<()> {
    save_media(ctx, options, false)
}

fn save_media(ctx: JSContext, options: JSSaveMediaOptions, image: bool) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let runtime = &lxapp.runtime;

    let request = SaveMediaRequest {
        file_uri: options.file_path,
    };

    let op = if image {
        runtime.save_image_to_photos_album(request)
    } else {
        runtime.save_video_to_photos_album(request)
    };

    match op {
        Ok(()) => Ok(()),
        Err(e) => {
            // Surface a user-visible hint when saving fails, typically due to
            // missing photo library permissions or storage issues.
            let _ = runtime.show_toast(ToastOptions {
                title: "保存失败，请检查照片权限或可用空间".to_string(),
                icon: ToastIcon::Error,
                image: None,
                duration: 2.0,
                mask: false,
                position: ToastPosition::Center,
            });

            let name = if image {
                "saveImageToPhotosAlbum"
            } else {
                "saveVideoToPhotosAlbum"
            };
            Err(RongJSError::Error(format!("{} failed: {}", name, e)))
        }
    }
}
