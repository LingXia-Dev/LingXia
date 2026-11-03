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
    /// `keep` when authored under `motion-reduce:`/`motion-safe:`; `base` shows the base
    /// style under reduced motion.
    when_reduced: WhenReduced,
    #[serde(default)]
    fill: FillMode,
    #[serde(default)]
    background_position: Option<Axes>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Axes {
    x: Vec<Key>,
    y: Vec<Key>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum FillMode {
    #[default]
    None,
    Forwards,
    Backwards,
    Both,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum WhenReduced {
    Base,
    Keep,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Iterations {
    Count(u32),
    Infinite(()),
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum Ease {
    Named(String),
    Bezier([f64; 4]),
    /// A transition's spring by bounce; keyframes refuse it.
    Spring {
        spring: f64,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    offset: f64,
    value: f64,
    /// Eases the segment that starts at this key.
    easing: Option<Ease>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Transform {
    Scale {
        x: Vec<Key>,
        y: Vec<Key>,
    },
    Translate {
        x: Vec<Key>,
        y: Vec<Key>,
        #[serde(default)]
        relative: bool,
    },
    Rotate {
        angle: Vec<Key>,
    },
}

pub(crate) fn easing(ease: Ease) -> Result<Easing, String> {
    match ease {
        Ease::Named(name) if name == "linear" => Ok(Easing::LINEAR),
        Ease::Named(name) if name == "step-end" => Ok(Easing::STEP_END),
        Ease::Bezier(points) => Easing::cubic_bezier(points),
        _ => Err("UI_ANIMATION: unsupported easing".into()),
    }
}

fn track(keys: Vec<Key>, default: Easing) -> Result<Track, String> {
    let mut easings = Vec::with_capacity(keys.len());
    let mut frames = Vec::with_capacity(keys.len());
    let last = keys.len().saturating_sub(1);
    for (index, key) in keys.into_iter().enumerate() {
        match key.easing {
            Some(_) if index == last => {
                return Err("UI_ANIMATION: final keyframe cannot ease".into());
            }
            Some(ease) => easings.push(easing(ease)?),
            None if index < last => easings.push(default),
            None => {}
        }
        frames.push(Keyframe {
            offset: key.offset,
            value: key.value,
        });
    }
    Track::segmented(frames, easings)
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
    )?
    .with_fill(super::Fill {
        backwards: matches!(wire.fill, FillMode::Backwards | FillMode::Both),
        forwards: matches!(wire.fill, FillMode::Forwards | FillMode::Both),
    });
    let easing = easing(wire.easing)?;
    if wire.transforms.len() > 8
        || (wire.opacity.is_none()
            && wire.transforms.is_empty()
            && wire.background_position.is_none())
    {
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
            Transform::Translate {
                x,
                y,
                relative: false,
            } => TransformTracks::Translate {
                x: track(x, easing)?,
                y: track(y, easing)?,
            },
            Transform::Translate { x, y, .. } => TransformTracks::Relative {
                x: track(x, easing)?,
                y: track(y, easing)?,
            },
            Transform::Rotate { angle } => TransformTracks::Rotate {
                angle: track(angle, easing)?,
            },
        });
    }
    let mut effect = Effect::new(opacity, transforms)?;
    if let Some(Axes { x, y }) = wire.background_position {
        effect = effect.with_background_position([track(x, easing)?, track(y, easing)?])?;
    }
    if effect.storage_bytes() > 32768 {
        return Err("UI_ANIMATION: effect budget exceeded".into());
    }
    Ok(Definition {
        keep_reduced: matches!(wire.when_reduced, WhenReduced::Keep),
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
            "../../../../packages/lingxia-view/tests/fixtures/animations.json"
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
    fn a_background_position_sweep_samples_between_its_keyframes() {
        let definition = decode_definition(
            &json!({"name":"shimmer","durationMs":1800,"delayMs":0,"iterations":null,"easing":"linear","whenReduced":"base",
                "opacity":null,"transforms":[],"backgroundPosition":{"x":[{"offset":0,"value":1},{"offset":1,"value":-1.5}],"y":[{"offset":0,"value":0},{"offset":1,"value":0}]}})
            .to_string(),
        )
        .unwrap();
        assert_eq!(
            definition.effect.sample(0.5).unwrap().background_position,
            Some([-0.25, 0.0])
        );
        let out_of_bounds = json!({"name":"far","durationMs":100,"delayMs":0,"iterations":1,"easing":"linear","whenReduced":"base",
            "opacity":null,"transforms":[],"backgroundPosition":{"x":[{"offset":0,"value":0},{"offset":1,"value":40}],"y":[{"offset":0,"value":0},{"offset":1,"value":0}]}});
        assert!(decode_definition(&out_of_bounds.to_string()).is_err());
    }

    /// The compiler's lowering of Tailwind's default `animate-*` keyframes.
    fn defaults() -> Value {
        json!({
            "spin":{"name":"spin","durationMs":1000,"delayMs":0,"iterations":null,"whenReduced":"base","easing":"linear","opacity":null,
                "transforms":[{"kind":"rotate","angle":[{"offset":0,"value":0},{"offset":1,"value":360}]}]},
            "ping":{"name":"ping","durationMs":1000,"delayMs":0,"iterations":null,"whenReduced":"base","easing":[0,0,0.2,1],
                "opacity":[{"offset":0,"value":1},{"offset":0.75,"value":0},{"offset":1,"value":0}],
                "transforms":[{"kind":"scale","x":[{"offset":0,"value":1},{"offset":0.75,"value":2},{"offset":1,"value":2}],
                    "y":[{"offset":0,"value":1},{"offset":0.75,"value":2},{"offset":1,"value":2}]}]},
            "pulse":{"name":"pulse","durationMs":2000,"delayMs":0,"iterations":null,"whenReduced":"base","easing":[0.4,0,0.6,1],
                "opacity":[{"offset":0,"value":1},{"offset":0.5,"value":0.5},{"offset":1,"value":1}],"transforms":[]},
            "bounce":{"name":"bounce","durationMs":1000,"delayMs":0,"iterations":null,"whenReduced":"base","easing":[0.25,0.1,0.25,1],"opacity":null,
                "transforms":[{"kind":"translate","relative":true,
                    "x":[{"offset":0,"value":0,"easing":[0.8,0,1,1]},{"offset":0.5,"value":0,"easing":[0,0,0.2,1]},{"offset":1,"value":0}],
                    "y":[{"offset":0,"value":-0.25,"easing":[0.8,0,1,1]},{"offset":0.5,"value":0,"easing":[0,0,0.2,1]},{"offset":1,"value":-0.25}]}]}
        })
    }

    #[test]
    fn default_tailwind_animations_rotate_translate_relatively_and_ease_per_key() {
        use vello_cpu::kurbo::Point;
        let defaults = defaults();
        let decode = |name: &str| decode_definition(&defaults[name].to_string()).unwrap();
        let spin = decode("spin");
        assert_eq!(spin.timing.iterations, None);
        let quarter = spin.effect.sample(0.25).unwrap();
        // A quarter turn clockwise about the center of a 20x20 box at the origin.
        let point = quarter.matrix_in([0.0, 0.0, 20.0, 20.0]).unwrap() * Point::new(20.0, 10.0);
        assert!(
            (point.x - 10.0).abs() < 1e-9 && (point.y - 20.0).abs() < 1e-9,
            "{point:?}"
        );
        assert_eq!(
            spin.effect.next_change(0.5).unwrap(),
            super::super::interpolate::NextChange::Continuous
        );

        let ping = decode("ping").effect;
        assert_eq!(ping.sample(0.8).unwrap().opacity, Some(0.0));
        assert_eq!(
            ping.sample(0.9).unwrap().matrix([0.0; 2]).unwrap(),
            vello_cpu::kurbo::Affine::scale(2.0)
        );
        let pulse = decode("pulse").effect;
        assert_eq!(pulse.sample(0.5).unwrap().opacity, Some(0.5));

        let bounce = decode("bounce").effect;
        let at = |progress: f64| {
            (bounce
                .sample(progress)
                .unwrap()
                .matrix_in([0.0, 0.0, 40.0, 40.0])
                .unwrap()
                * Point::ORIGIN)
                .y
        };
        // -25% of a 40px box at both ends, resting at the middle key.
        assert!((at(0.0) + 10.0).abs() < 1e-9);
        assert!(at(0.5).abs() < 1e-9);
        assert!((at(1.0) + 10.0).abs() < 1e-9);
        // The falling half eases in (0.8,0,1,1): still near the top a quarter in;
        // the rising half eases out (0,0,0.2,1): already high a quarter in.
        assert!(at(0.25) < -8.0, "{}", at(0.25));
        assert!(at(0.75) < -7.0, "{}", at(0.75));
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

        let defaults = defaults();
        for (name, path, value) in [
            ("spin", "/transforms/0/angle/1/value", json!(36001)),
            ("spin", "/transforms/0/angle/1/easing", json!("linear")),
            ("spin", "/transforms/0/angle/0/easing", json!("bounce")),
            ("spin", "/transforms/0/x", json!([])),
            ("bounce", "/transforms/0/y/0/value", json!(10.5)),
            ("bounce", "/transforms/0/relative", json!("yes")),
            ("ping", "/transforms/0/x/1/value", json!(-2)),
            ("ping", "/transforms/0/relative", json!(true)),
            ("pulse", "/whenReduced", json!("sometimes")),
            ("pulse", "/whenReduced", json!(null)),
        ] {
            let mut payload = defaults[name].clone();
            match payload.pointer_mut(path) {
                Some(slot) => *slot = value,
                None => {
                    let (parent, key) = path.rsplit_once('/').unwrap();
                    payload.pointer_mut(parent).unwrap()[key] = value;
                }
            }
            assert!(
                decode_definition(&payload.to_string()).is_err(),
                "{name}{path}: {payload}"
            );
        }
    }
}
