//! Bounded compiler payload; constructors retain all numeric validation.
use super::{
    Definition, Effect, Timing, TransformTracks,
    interpolate::{Easing, Keyframe, Track},
};
use serde::Deserialize;
use std::{num::NonZeroU32, time::Duration};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Wire {
    name: String,
    duration_ms: f64,
    delay_ms: f64,
    iterations: Iterations,
    easing: Ease,
    opacity: Option<Vec<Key>>,
    transforms: Vec<Transform>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Iterations {
    Count(u32),
    Infinite(()),
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum Ease {
    Named(String),
    Bezier([f64; 4]),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    offset: f64,
    value: f64,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Transform {
    Scale { x: Vec<Key>, y: Vec<Key> },
    Translate { x: Vec<Key>, y: Vec<Key> },
}

fn track(keys: Vec<Key>, easing: Easing) -> Result<Track, String> {
    Track::new(
        keys.into_iter()
            .map(|key| Keyframe {
                offset: key.offset,
                value: key.value,
            })
            .collect(),
        easing,
    )
}

/// Decode one compiled animation; this does not enable authoring or scheduling.
pub fn decode_definition(source: &str) -> Result<Definition, String> {
    if source.len() > 32768 {
        return Err("UI_ANIMATION: wire budget exceeded".into());
    }
    let wire: Wire = serde_json::from_str(source)
        .map_err(|error| format!("UI_ANIMATION: invalid payload: {error}"))?;
    if wire.name.is_empty()
        || wire.name.len() > 128
        || !wire.name.bytes().enumerate().all(|(i, b)| {
            b.is_ascii_alphabetic() || b == b'-' || b == b'_' || (i > 0 && b.is_ascii_digit())
        })
    {
        return Err("UI_ANIMATION: invalid animation name".into());
    }
    let interval = |ms: f64| {
        if !ms.is_finite() || !(0.0..=86400000.0).contains(&ms) {
            return Err("UI_ANIMATION: invalid interval".to_owned());
        }
        Duration::try_from_secs_f64(ms / 1000.0)
            .map_err(|_| "UI_ANIMATION: invalid interval".to_owned())
    };
    let iterations = match wire.iterations {
        Iterations::Count(value) => {
            Some(NonZeroU32::new(value).ok_or("UI_ANIMATION: zero iterations")?)
        }
        Iterations::Infinite(()) => None,
    };
    let timing = Timing::new(
        interval(wire.duration_ms)?,
        interval(wire.delay_ms)?,
        iterations,
    )?;
    let easing = match wire.easing {
        Ease::Named(name) if name == "linear" => Easing::LINEAR,
        Ease::Named(name) if name == "step-end" => Easing::STEP_END,
        Ease::Bezier(points) => Easing::cubic_bezier(points)?,
        _ => return Err("UI_ANIMATION: unsupported easing".into()),
    };
    if wire.transforms.len() > 8 || (wire.opacity.is_none() && wire.transforms.is_empty()) {
        return Err("UI_ANIMATION: invalid effect tracks".into());
    }
    let opacity = wire.opacity.map(|keys| track(keys, easing)).transpose()?;
    let mut transforms = Vec::with_capacity(wire.transforms.len());
    for transform in wire.transforms {
        transforms.push(match transform {
            Transform::Scale { x, y } => {
                if x.iter().chain(&y).any(|key| key.value <= 0.0) {
                    return Err("UI_ANIMATION: singular or reflected scale unsupported".into());
                }
                TransformTracks::Scale {
                    x: track(x, easing)?,
                    y: track(y, easing)?,
                }
            }
            Transform::Translate { x, y } => TransformTracks::Translate {
                x: track(x, easing)?,
                y: track(y, easing)?,
            },
        });
    }
    let effect = Effect::new(opacity, transforms)?;
    if effect.storage_bytes() > 32768 {
        return Err("UI_ANIMATION: effect budget exceeded".into());
    }
    Ok(Definition {
        name: wire.name,
        timing,
        effect,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn fixtures() -> Value {
        serde_json::from_str(include_str!(
            "../../../../packages/lingxia-ui/tests/fixtures/animations.json"
        ))
        .unwrap()
    }

    #[test]
    fn compiler_fixtures_preserve_reference_timing_and_ordered_transforms() {
        let fixtures = fixtures();
        let blink = decode_definition(&fixtures["blink"].to_string()).unwrap();
        assert_eq!(blink.effect.sample(0.499).unwrap().opacity, Some(1.0));
        assert_eq!(blink.effect.sample(0.5).unwrap().opacity, Some(0.0));
        assert_eq!(
            blink.timing.sample(Duration::from_millis(900)).progress,
            0.0
        );
        let chart = decode_definition(&fixtures["chart-in"].to_string()).unwrap();
        let start = chart.effect.sample(0.0).unwrap();
        let matrix = start.matrix([50.0, 50.0]).unwrap().as_coeffs();
        assert_eq!(matrix[0], 0.96);
        assert!((matrix[5] - 9.68).abs() < 1e-12);
        assert_eq!(start.opacity, Some(0.0));
        assert_eq!(
            chart
                .effect
                .sample(1.0)
                .unwrap()
                .matrix([50.0, 50.0])
                .unwrap(),
            vello_cpu::kurbo::Affine::IDENTITY
        );
        assert_eq!(
            chart.timing.sample(Duration::from_millis(350)).phase,
            super::super::Phase::Finished
        );
    }

    #[test]
    fn invalid_wire_cannot_bypass_effect_or_timing_validation() {
        let original = fixtures()["chart-in"].clone();
        for (path, value) in [
            ("/name", json!("")),
            ("/name", json!("a".repeat(129))),
            ("/name", json!("two names")),
            ("/durationMs", json!(0)),
            ("/durationMs", json!(86400001)),
            ("/delayMs", json!(-1)),
            ("/iterations", json!(0)),
            ("/iterations", json!(1.5)),
            ("/iterations", json!(4294967296u64)),
            ("/easing", json!("step-start")),
            ("/easing", json!([2, 0, 1, 1])),
            ("/opacity/0/value", json!(1.1)),
            ("/opacity/0/offset", json!(0.1)),
            ("/transforms/0/x/0/value", json!(0)),
            ("/transforms/0/y/0/value", json!(-1)),
            ("/transforms/0/x/0/value", json!(17)),
            ("/transforms/1/y/0/value", json!(16385)),
            ("/transforms/0/x", json!([])),
        ] {
            let mut payload = original.clone();
            *payload.pointer_mut(path).unwrap() = value;
            assert!(
                decode_definition(&payload.to_string()).is_err(),
                "{path}: {payload}"
            );
        }
        let mut unknown = original.clone();
        unknown["fillMode"] = json!("forwards");
        assert!(decode_definition(&unknown.to_string()).is_err());
        let mut unknown = original.clone();
        unknown["opacity"][0]["ignored"] = json!(true);
        assert!(decode_definition(&unknown.to_string()).is_err());
        let mut empty = original.clone();
        empty["opacity"] = Value::Null;
        empty["transforms"] = json!([]);
        assert!(decode_definition(&empty.to_string()).is_err());
        let mut missing = original.clone();
        missing.as_object_mut().unwrap().remove("iterations");
        assert!(decode_definition(&missing.to_string()).is_err());
        assert!(decode_definition(&format!("{}{}", " ".repeat(32768), original)).is_err());
        assert!(decode_definition(&original.to_string().replacen("350", "1e999", 1)).is_err());
    }
}
