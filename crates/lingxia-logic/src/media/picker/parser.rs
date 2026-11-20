use crate::i18n::js_invalid_parameter_error;
use lingxia_platform::traits::media_interaction::{CameraFacing, MediaSource, PickMediaMode};
use rong::JSResult;

pub(super) fn parse_pick_mode(values: Option<Vec<String>>) -> JSResult<PickMediaMode> {
    let raw = values.unwrap_or_else(|| vec!["image".to_string(), "video".to_string()]);
    let mut has_image = false;
    let mut has_video = false;

    for token in raw {
        match token.to_lowercase().as_str() {
            "image" => has_image = true,
            "video" => has_video = true,
            other => {
                return Err(js_invalid_parameter_error(format!(
                    "pickMedia invalid mediaType token \"{}\"",
                    other
                )));
            }
        }
    }

    if !has_image && !has_video {
        has_image = true;
        has_video = true;
    }

    Ok(match (has_image, has_video) {
        (true, true) => PickMediaMode::Mix,
        (true, false) => PickMediaMode::Images,
        (false, true) => PickMediaMode::Videos,
        _ => PickMediaMode::Images,
    })
}

pub(super) fn parse_sources(values: Option<Vec<String>>) -> JSResult<Vec<MediaSource>> {
    let raw = values.unwrap_or_else(|| vec!["album".to_string()]);
    let mut out: Vec<MediaSource> = Vec::new();

    for token in raw {
        let source = match token.to_lowercase().as_str() {
            "album" => MediaSource::Album,
            "camera" => MediaSource::Camera,
            other => {
                return Err(js_invalid_parameter_error(format!(
                    "pickMedia invalid sourceType token \"{}\"",
                    other
                )));
            }
        };

        if !out.contains(&source) {
            out.push(source);
        }
    }

    if out.is_empty() {
        out.push(MediaSource::Album);
    }

    Ok(out)
}

pub(super) fn parse_camera(s: Option<String>) -> Option<CameraFacing> {
    s.map(|v| match v.to_lowercase().as_str() {
        "front" => CameraFacing::Front,
        _ => CameraFacing::Back,
    })
}
