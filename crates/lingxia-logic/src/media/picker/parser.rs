use crate::i18n::js_invalid_parameter_error;
use lingxia_platform::traits::media_interaction::{CameraFacing, PickMediaMode};
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
                    "pick.media invalid types entry \"{}\"",
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

/// The one kind `lx.pick.capture` records.
pub(super) fn parse_capture_kind(value: &str) -> JSResult<PickMediaMode> {
    match value {
        "image" => Ok(PickMediaMode::Images),
        "video" => Ok(PickMediaMode::Videos),
        other => Err(js_invalid_parameter_error(format!(
            "pick.capture invalid type \"{other}\" (expected image or video)"
        ))),
    }
}

pub(super) fn parse_camera(s: Option<String>) -> Option<CameraFacing> {
    s.map(|v| match v.to_lowercase().as_str() {
        "front" => CameraFacing::Front,
        _ => CameraFacing::Back,
    })
}
