//! Theme steps on the wire: `[slot, times, plus]` resolved against the page's scales.
use super::{FIELDS, Record, SCALES, field, values};
use serde_json::Value;
use std::sync::{Arc, LazyLock};

/// One slot's value: a length or duration, an easing's control points, or none (`em`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slot {
    Number(f64),
    Easing([f64; 4]),
    None,
}

/// The scale values a page resolves its steps against, one per slot in wire order.
#[derive(Clone, Debug, PartialEq)]
pub struct Scales {
    pub revision: u64,
    slots: Vec<Slot>,
}

/// The step a resolved field came from, kept to resolve it again when the scales change.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Number {
        slot: usize,
        times: f64,
        plus: f64,
    },
    /// A transition as encoded, with its motion steps.
    Transition(String),
}

static BUILTIN: LazyLock<Arc<Scales>> = LazyLock::new(|| {
    Arc::new(Scales::from_pack(&lingxia_theme::default_pack(), 0).expect("default pack scales"))
});

/// A CSS timing function as cubic-bezier control points.
fn easing(css: &str) -> Option<[f64; 4]> {
    Some(match css {
        "linear" => [0.0, 0.0, 1.0, 1.0],
        "ease" => [0.25, 0.1, 0.25, 1.0],
        "ease-in" => [0.42, 0.0, 1.0, 1.0],
        "ease-out" => [0.0, 0.0, 0.58, 1.0],
        "ease-in-out" => [0.42, 0.0, 0.58, 1.0],
        _ => {
            let points: Vec<f64> = css
                .strip_prefix("cubic-bezier(")?
                .strip_suffix(')')?
                .split(',')
                .map(|p| p.trim().parse().ok())
                .collect::<Option<_>>()?;
            points.try_into().ok()?
        }
    })
}

impl Scales {
    /// The built-in pack's values, for a page whose host pushed none.
    pub fn builtin() -> Arc<Scales> {
        BUILTIN.clone()
    }

    /// The values `pack` gives each slot.
    pub fn from_pack(pack: &lingxia_theme::Pack, revision: u64) -> Result<Scales, String> {
        let pack = serde_json::to_value(lingxia_theme::complete(pack.clone()))
            .map_err(|e| e.to_string())?;
        let slots = SCALES
            .iter()
            .map(|(name, family, pointer)| {
                let value = (!pointer.is_empty())
                    .then(|| pack.pointer(pointer))
                    .flatten();
                let missing = || format!("UI_ENVIRONMENT: theme pack has no {name}");
                Ok(match (*family, value) {
                    ("em", _) => Slot::None,
                    ("ease", Some(Value::String(css))) => {
                        Slot::Easing(easing(css).ok_or_else(missing)?)
                    }
                    (_, Some(value)) => Slot::Number(
                        value
                            .as_f64()
                            .filter(|v| v.is_finite())
                            .ok_or_else(missing)?,
                    ),
                    (_, None) => return Err(missing()),
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Scales { revision, slots })
    }

    /// As the host pushes it in the environment's `scales` key.
    pub fn push(&self) -> Value {
        let values: Vec<Value> = self
            .slots
            .iter()
            .map(|slot| match slot {
                Slot::Number(value) => (*value).into(),
                Slot::Easing(points) => points.iter().copied().collect(),
                Slot::None => Value::Null,
            })
            .collect();
        serde_json::json!({ "revision": self.revision, "values": values })
    }

    /// The host's `scales` push: `{revision, values}`, one value per slot.
    pub fn decode(pushed: &Value) -> Result<Scales, String> {
        let invalid = || "UI_ENVIRONMENT: invalid scales".to_string();
        let revision = pushed["revision"].as_u64().ok_or_else(invalid)?;
        let values = pushed["values"]
            .as_array()
            .filter(|values| values.len() == SCALES.len())
            .ok_or_else(invalid)?;
        let slots = SCALES
            .iter()
            .zip(values)
            .map(|((_, family, _), value)| match (*family, value) {
                ("em", Value::Null) => Some(Slot::None),
                ("ease", Value::Array(points)) if points.len() == 4 => {
                    let points: Vec<f64> = points
                        .iter()
                        .map(|p| p.as_f64().filter(|p| p.is_finite()))
                        .collect::<Option<_>>()?;
                    ((0.0..=1.0).contains(&points[0]) && (0.0..=1.0).contains(&points[2]))
                        .then(|| Slot::Easing([points[0], points[1], points[2], points[3]]))
                }
                ("em" | "ease", _) => None,
                (_, value) => value
                    .as_f64()
                    .filter(|v| v.is_finite() && (0.0..=16384.0).contains(v))
                    .map(Slot::Number),
            })
            .collect::<Option<_>>()
            .ok_or_else(invalid)?;
        Ok(Scales { revision, slots })
    }

    /// The small control radius rings around text take.
    pub fn control_radius(&self) -> f32 {
        self.named("radius.sm").unwrap_or(4.0) as f32
    }

    /// The value of the slot named `name` (`radius.sm`).
    pub fn named(&self, name: &str) -> Option<f64> {
        self.number(SCALES.iter().position(|(slot, ..)| *slot == name)?)
    }

    fn number(&self, slot: usize) -> Option<f64> {
        match self.slots.get(slot) {
            Some(Slot::Number(value)) => Some(*value),
            _ => None,
        }
    }
}

/// `value` of field `id` with its steps resolved, and the step it came from; `None` when it
/// holds none. An `em` step stays for layout, which knows the node's font size.
pub(crate) fn resolve(
    tag: usize,
    id: usize,
    value: &Value,
    scales: &Scales,
) -> Result<Option<(Value, Step)>, String> {
    if id == field::TRANSITION {
        let Some(encoded) = value.as_str() else {
            return Ok(None);
        };
        return Ok(transition(encoded, scales)?.map(|resolved| {
            (
                Value::String(resolved),
                Step::Transition(encoded.to_owned()),
            )
        }));
    }
    let Some(step) = value.as_array() else {
        return Ok(None);
    };
    let (slot, times, plus) = values::scale_step(id, step)?;
    if SCALES[slot].1 == "em" {
        return Ok(None);
    }
    let step = Step::Number { slot, times, plus };
    Ok(Some((number(tag, id, &step, scales)?, step)))
}

fn number(tag: usize, id: usize, step: &Step, scales: &Scales) -> Result<Value, String> {
    let Step::Number { slot, times, plus } = *step else {
        unreachable!("a number step");
    };
    let base = scales
        .number(slot)
        .ok_or_else(|| format!("UI_SCALE: no value for {}", SCALES[slot].0))?;
    let resolved = times * base + plus;
    let signed = id == field::LETTER_SPACING || matches!(FIELDS[id].1, "margin" | "inset");
    if !signed && resolved < 0.0 {
        return Err(format!("UI_STYLE: {} resolves below 0", FIELDS[id].0));
    }
    let resolved = serde_json::Number::from_f64(resolved)
        .map(Value::Number)
        .ok_or("UI_STYLE: invalid length")?;
    values::check(tag, id, &resolved)?;
    Ok(resolved)
}

/// A transition with its motion steps replaced by their values; `None` when it has none.
fn transition(encoded: &str, scales: &Scales) -> Result<Option<String>, String> {
    if !encoded.contains('[') {
        return Ok(None);
    }
    let mut value: Value =
        serde_json::from_str(encoded).map_err(|_| "UI_TRANSITION: invalid transition")?;
    let slot = |value: &Value| match value.as_array().map(Vec::as_slice) {
        Some([slot, _, _]) => slot.as_u64().map(|slot| slot as usize),
        _ => None,
    };
    let mut changed = false;
    if let Some(slot) = slot(&value["durationMs"]) {
        let ms = scales
            .number(slot)
            .ok_or("UI_TRANSITION: invalid transition")?;
        value["durationMs"] = ms.into();
        changed = true;
    }
    if let Some(slot) = slot(&value["easing"]) {
        let Some(Slot::Easing(points)) = scales.slots.get(slot) else {
            return Err("UI_TRANSITION: invalid transition".into());
        };
        value["easing"] = points.iter().copied().collect();
        changed = true;
    }
    Ok(changed.then(|| value.to_string()))
}

/// `records` resolved again under `scales`; `None` when none holds a step.
pub(crate) fn rescale(
    records: &[Record],
    scales: &Scales,
) -> Result<Option<Arc<[Record]>>, String> {
    if records
        .iter()
        .all(|record| record.fields.steps().is_empty())
    {
        return Ok(None);
    }
    let mut rescaled = records.to_vec();
    for record in &mut rescaled {
        let steps = record.fields.steps().to_vec();
        if steps.is_empty() {
            continue;
        }
        let tag = record.tag;
        for (id, step) in steps {
            let value = match &step {
                Step::Number { .. } => number(tag, id, &step, scales)?,
                Step::Transition(encoded) => {
                    Value::String(transition(encoded, scales)?.unwrap_or_else(|| encoded.clone()))
                }
            };
            if let Some((_, old)) = record.fields.iter_mut().find(|(field, _)| *field == id) {
                *old = value;
            }
        }
    }
    Ok(Some(rescaled.into()))
}

/// An `em` step on field `id` of `record`: its times and plus, resolved at layout.
pub(crate) fn em(record: &Record, id: usize) -> Option<(f32, f32)> {
    let (_, value) = record.fields.iter().find(|(field, _)| *field == id)?;
    match value.as_array()?.as_slice() {
        [slot, times, plus] if slot.as_u64().and_then(|s| SCALES.get(s as usize))?.1 == "em" => {
            Some((times.as_f64()? as f32, plus.as_f64()? as f32))
        }
        _ => None,
    }
}
