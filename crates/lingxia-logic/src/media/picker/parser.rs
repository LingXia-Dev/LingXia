use crate::i18n::js_invalid_parameter_error;
use lingxia_platform::traits::media_interaction::{CameraFacing, PickMediaMode};
use lingxia_service::media::MediaSource;
use rong::{JSObject, JSResult, JSValue, RongJSError, error::HostError};

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

fn parse_camera(s: Option<String>) -> Option<CameraFacing> {
    s.map(|v| match v.to_lowercase().as_str() {
        "front" => CameraFacing::Front,
        _ => CameraFacing::Back,
    })
}

/// `sources` in the order given, without repeats; absent means the library only.
pub(super) fn parse_sources(values: Option<Vec<String>>) -> JSResult<Vec<MediaSource>> {
    let Some(values) = values else {
        return Ok(vec![MediaSource::Album]);
    };
    let mut sources = Vec::new();
    for value in values {
        let source = match value.as_str() {
            "library" => MediaSource::Album,
            "camera" => MediaSource::Camera,
            other => {
                return Err(refused(
                    format!("pick.media sources entry {other:?} is not 'library' or 'camera'"),
                    "sources",
                ));
            }
        };
        if !sources.contains(&source) {
            sources.push(source);
        }
    }
    if sources.is_empty() {
        return Err(refused(
            "pick.media sources needs at least one source".into(),
            "sources",
        ));
    }
    Ok(sources)
}

/// How the camera records: `facing` and the video limit in whole seconds, rounded up.
#[derive(Clone, Copy, Default)]
pub(super) struct CameraSettings {
    pub(super) facing: Option<CameraFacing>,
    pub(super) max_duration_seconds: Option<u32>,
}

impl CameraSettings {
    pub(super) fn new(facing: Option<String>, max_duration_ms: Option<f64>) -> Self {
        Self {
            facing: parse_camera(facing),
            max_duration_seconds: max_duration_ms
                .filter(|v| !v.is_sign_negative())
                .map(|v| (v / 1000.0).ceil().min(u32::MAX as f64) as u32),
        }
    }

    /// `pick.media`'s `camera` object, read with the same fields as `pick.capture`'s options.
    pub(super) fn from_media_option(camera: Option<JSObject>) -> JSResult<Self> {
        let Some(camera) = camera else {
            return Ok(Self::default());
        };
        // A missing field reads as absent; strict `get` would refuse it.
        let field = |name: &'static str| -> JSResult<Option<JSValue>> {
            Ok(camera
                .get::<_, JSValue>(name)
                .ok()
                .filter(|value| !value.is_undefined() && !value.is_null()))
        };
        let at =
            |error: RongJSError, name: &str| error.at_field(name).at_field("camera").at_argument(1);
        let facing = field("facing")?
            .map(|v| v.to_rust::<String>().map_err(|e| at(e, "facing")))
            .transpose()?;
        let max_duration_ms = field("maxDurationMs")?
            .map(|v| v.to_rust::<f64>().map_err(|e| at(e, "maxDurationMs")))
            .transpose()?;
        Ok(Self::new(facing, max_duration_ms))
    }
}

fn refused(message: String, field: &str) -> RongJSError {
    RongJSError::from(HostError::new(rong::error::E_INVALID_ARG, message))
        .at_field(field)
        .at_argument(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rong::JSEngine;

    #[test]
    fn sources_default_to_the_library_and_keep_the_order_given() {
        assert_eq!(parse_sources(None).unwrap(), vec![MediaSource::Album]);
        let both = vec!["camera".into(), "library".into(), "camera".into()];
        assert_eq!(
            parse_sources(Some(both)).unwrap(),
            vec![MediaSource::Camera, MediaSource::Album]
        );
        for refused in [vec![], vec!["album".to_string()]] {
            let error = parse_sources(Some(refused)).unwrap_err();
            assert_eq!(error.code(), Some(rong::error::E_INVALID_ARG));
        }
    }

    /// `media`'s `camera` reads like `capture`'s options, and a wrong field is refused where it is.
    #[test]
    fn camera_settings_round_up_and_refuse_a_wrong_field() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let camera = JSObject::new(&ctx);
        camera.set("facing", "front").unwrap();
        camera.set("maxDurationMs", 1500.0).unwrap();
        let settings = CameraSettings::from_media_option(Some(camera)).unwrap();
        assert!(matches!(settings.facing, Some(CameraFacing::Front)));
        assert_eq!(settings.max_duration_seconds, Some(2));
        let wrong = JSObject::new(&ctx);
        wrong.set("maxDurationMs", "long").unwrap();
        let error = CameraSettings::from_media_option(Some(wrong))
            .err()
            .unwrap();
        assert_eq!(error.code(), Some(rong::error::E_INVALID_ARG));
    }
}
