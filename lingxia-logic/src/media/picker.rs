mod cache;
mod parser;
mod source_picker;
mod types;

use crate::i18n::err_code_message;
use cache::ensure_cached_media_path;
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::traits::app_runtime::AppRuntime;
#[cfg(not(target_os = "macos"))]
use lingxia_platform::traits::media_interaction::ChooseMediaMode;
use lingxia_platform::traits::media_interaction::{
    ChooseMediaRequest, MediaInteraction, MediaKind, MediaSource,
};
use lxapp::{LxApp, lx};
use parser::{parse_camera, parse_choose_mode, parse_sources};
use rong::{HostError, JSContext, JSFunc, JSResult, RongJSError, function::Optional};
use serde_json::Value;
use source_picker::present_source_picker;
use std::fs;
use std::path::{Path, PathBuf};
use types::{ChosenMediaEntry, JSChooseMediaOptions, MediaKey};

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let choose_media_func = JSFunc::new(ctx, |ctx, options| async move {
        choose_media(ctx, options).await
    })?;
    lx::register_js_api(ctx, "chooseMedia", choose_media_func)?;
    Ok(())
}

async fn choose_media(
    ctx: JSContext,
    options: Optional<JSChooseMediaOptions>,
) -> JSResult<Vec<ChosenMediaEntry>> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    let opts = options.as_ref().cloned().unwrap_or(JSChooseMediaOptions {
        count: None,
        media_type: None,
        source_type: None,
        camera: None,
        max_duration: None,
    });

    let mode = parse_choose_mode(opts.media_type)?;
    let sources = parse_sources(opts.source_type)?;
    let selected_source = if sources.len() > 1 {
        match present_source_picker(&lxapp, &sources).await? {
            Some(source) => source,
            None => return Ok(Vec::new()),
        }
    } else {
        sources.first().copied().unwrap_or(MediaSource::Album)
    };

    #[cfg(not(target_os = "macos"))]
    if matches!(selected_source, MediaSource::Camera) && matches!(mode, ChooseMediaMode::Mix) {
        return Err(HostError::new(
            rong::error::E_INTERNAL,
            "camera source does not support selecting both image and video; specify a single mediaType entry",
        ).into());
    }

    let (callback_id, receiver) = get_callback();
    let max_duration_seconds = opts
        .max_duration
        .filter(|v| !v.is_sign_negative())
        .map(|v| v.min(u32::MAX as f64).round() as u32);
    let source_types = vec![selected_source];

    let request = ChooseMediaRequest {
        max_count: opts.count.unwrap_or(9),
        mode,
        source_types,
        max_duration_seconds,
        camera_facing: parse_camera(opts.camera),
        callback_id,
    };

    lxapp.runtime.choose_media(request).map_err(|e| {
        HostError::new(
            rong::error::E_INTERNAL,
            format!("chooseMedia failed to start: {}", e),
        )
    })?;

    let result = receiver.await.map_err(|_| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "chooseMedia cancelled or failed",
        ))
    })?;

    let data = match result {
        CallbackResult::Success(data) => data,
        CallbackResult::Error(code) => {
            // 2000 = user cancelled, return empty result
            if code == 2000 {
                return Ok(Vec::new());
            }

            let message =
                err_code_message(code).unwrap_or_else(|| format!("chooseMedia error: {}", code));
            return Err(HostError::new(rong::error::E_INTERNAL, message).into());
        }
    };

    let parsed: Value = serde_json::from_str(&data).map_err(|_| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "chooseMedia invalid payload",
        ))
    })?;

    if parsed.is_null() {
        return Ok(Vec::new());
    }

    if parsed.as_object().is_some() {
        return Err(HostError::new(rong::error::E_INTERNAL, "chooseMedia invalid payload").into());
    }

    if !parsed.is_array() {
        return Err(HostError::new(rong::error::E_INTERNAL, "chooseMedia invalid payload").into());
    }

    let arr: Vec<MediaKey> = serde_json::from_str(&data).map_err(|_| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "chooseMedia invalid payload",
        ))
    })?;

    let mut out: Vec<ChosenMediaEntry> = Vec::new();
    for key in arr.into_iter() {
        let uri = key.uri.trim();
        if uri.is_empty() {
            continue;
        }
        let kind = key.kind.as_str();
        let ext = match kind {
            "video" => "mp4",
            _ => "jpg",
        };
        let is_original = key.is_original;

        // Only treat `file://...` URIs as local filesystem paths when the remainder is an absolute
        // path. Pickers may also return non-filesystem URIs (e.g. Android `content://...`, iOS
        // Photos identifiers, or Harmony `file://media/...`), which must be copied via the
        // platform runtime (copy_album_media_to_file).
        let local_path = if let Some(path_str) = uri.strip_prefix("file://") {
            Path::new(path_str)
                .is_absolute()
                .then(|| PathBuf::from(path_str))
        } else {
            Path::new(uri).is_absolute().then(|| PathBuf::from(uri))
        };

        let final_path: PathBuf = if let Some(source_path) = local_path {
            match lxapp.resolve_accessible_path(source_path.to_string_lossy().as_ref()) {
                Ok(path) if lxapp.to_uri(&path).is_some() => path,
                _ => ensure_cached_media_path(lxapp.as_ref(), &key, ext, |dest_path| {
                    fs::copy(&source_path, dest_path).map(|_| ()).map_err(|e| {
                        RongJSError::from(HostError::new(
                            rong::error::E_INTERNAL,
                            format!(
                                "chooseMedia failed to copy temp file into cache (src={}, dest={}): {}",
                                source_path.display(),
                                dest_path.display(),
                                e
                            ),
                        ))
                    })
                })?,
            }
        } else if let Ok(path) = lxapp.resolve_accessible_path(uri) {
            path
        } else {
            let media_kind = match kind {
                "video" => MediaKind::Video,
                "image" => MediaKind::Image,
                _ => MediaKind::Image,
            };
            ensure_cached_media_path(lxapp.as_ref(), &key, ext, |dest_path| {
                AppRuntime::copy_album_media_to_file(&*lxapp.runtime, uri, dest_path, media_kind)
                    .map_err(|err| {
                        RongJSError::from(HostError::new(
                            rong::error::E_INTERNAL,
                            format!("copyMedia failed: {}", err),
                        ))
                    })
            })?
        };

        let final_uri = lxapp
            .to_uri(&final_path)
            .ok_or_else(|| {
                RongJSError::from(HostError::new(
                    rong::error::E_INTERNAL,
                    "chooseMedia failed to convert output path to lx:// uri",
                ))
            })?
            .into_string();

        out.push(ChosenMediaEntry {
            path: final_uri,
            kind: key.kind,
            is_original,
        });
    }
    Ok(out)
}
