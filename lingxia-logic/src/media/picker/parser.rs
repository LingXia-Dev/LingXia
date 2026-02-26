use crate::i18n::js_invalid_parameter_error;
use lingxia_platform::traits::media_interaction::{CameraFacing, ChooseMediaMode, MediaSource};
use rong::JSResult;

pub(super) fn parse_choose_mode(values: Option<Vec<String>>) -> JSResult<ChooseMediaMode> {
    let raw = values.unwrap_or_else(|| vec!["image".to_string(), "video".to_string()]);
    let mut has_image = false;
    let mut has_video = false;

    for token in raw {
        match token.to_lowercase().as_str() {
            "image" => has_image = true,
            "video" => has_video = true,
            other => {
                return Err(js_invalid_parameter_error(format!(
                    "chooseMedia invalid mediaType token \"{}\"",
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
        (true, true) => ChooseMediaMode::Mix,
        (true, false) => ChooseMediaMode::Images,
        (false, true) => ChooseMediaMode::Videos,
        _ => ChooseMediaMode::Images,
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
                    "chooseMedia invalid sourceType token \"{}\"",
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
