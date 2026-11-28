//! `lx.media`: preview, inspect, transcode and save pictures and videos.

mod image;
mod picker;
mod preview;
mod save;
mod scan;
mod video;
pub(crate) use video::{JSVideoContext as VideoContextHandle, for_page as video_for_page};
mod video_file;

use rong::{JSContext, JSObject, JSResult};

/// The file an API reads from: refused before the platform sees it, so a missing
/// file is `E_NOT_FOUND` everywhere rather than whatever the decoder says.
pub(crate) fn resolve_input_file(lxapp: &lxapp::LxApp, path: &str) -> JSResult<std::path::PathBuf> {
    let resolved = lxapp
        .resolve_accessible_path(path.trim())
        .map_err(crate::i18n::js_error)?;
    if !resolved.exists() {
        return Err(crate::i18n::js_resource_not_found_error(format!(
            "no such file: {path}"
        )));
    }
    Ok(resolved)
}

/// A platform failure on an input file is the input's fault when the platform cannot read
/// it as that media at all: `probe` asks, and its `E_INVALID_DATA` replaces an internal error.
pub(crate) fn input_failure<T>(
    error: lingxia_error::Error,
    probe: impl FnOnce() -> Result<T, lingxia_error::Error>,
) -> lingxia_error::Error {
    if !error.is(lingxia_error::code::INTERNAL) {
        return error;
    }
    match probe() {
        Err(content) if content.is(lingxia_error::code::INVALID_DATA) => content,
        _ => error,
    }
}

pub(crate) fn media_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("media") {
        Ok(namespace) => Ok(namespace),
        Err(_) => {
            let namespace = JSObject::new(ctx);
            lx.set("media", namespace.clone())?;
            Ok(namespace)
        }
    }
}

rong::js_api! {
    fn register_media_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const media: "MediaApi" = media_namespace(ctx)?;
    }
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    register_media_property(ctx)?;
    preview::init(ctx)?;
    save::init(ctx)?;
    image::init(ctx)?;
    video_file::init(ctx)?;
    picker::init(ctx)?;
    scan::init(ctx)?;
    video::init(ctx)?;
    Ok(())
}
