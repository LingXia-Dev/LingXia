use crate::protocol::Record;
use crate::protocol::field;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct SliderSnapshot {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub value: f64,
    pub inset: f32,
    pub enabled: bool,
    pub on_change: Option<u64>,
}

impl SliderSnapshot {
    pub(crate) fn from_record(record: &Record) -> Result<Self, String> {
        let number = |field, default| {
            record
                .fields
                .iter()
                .find(|(id, _)| *id == field)
                .and_then(|(_, v)| v.as_f64())
                .unwrap_or(default)
        };
        let min = number(43, 0.0);
        let max = number(44, 100.0);
        let step = number(45, 1.0);
        let value = number(42, min);
        if ![min, max, step, value]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1e9)
            || max <= min
            || step <= 0.0
            || step > max - min
            || (max - min) / step > 1e6
            || min + step == min
            || max - step == max
        {
            return Err("UI_SLIDER: invalid range or precision".into());
        }
        if record
            .string(field::ACCESSIBILITY_LABEL)
            .is_none_or(|label| label.trim().is_empty())
        {
            return Err("UI_SLIDER: accessibilityLabel is required".into());
        }
        let enabled = record.number(field::ENABLED) != Some(0.0);
        let mut result = Self {
            min,
            max,
            step,
            value,
            inset: record.number(field::PADDING).unwrap_or(12.0) + 12.0,
            enabled,
            on_change: enabled
                .then(|| {
                    record
                        .fields
                        .iter()
                        .find(|(id, _)| *id == field::ON_SLIDER_CHANGE)
                        .and_then(|(_, v)| v.as_u64())
                })
                .flatten(),
        };
        result.value = result.normalize(value);
        Ok(result)
    }

    pub(crate) fn normalize(&self, value: f64) -> f64 {
        let value = value.clamp(self.min, self.max);
        let snapped = (self.min + ((value - self.min) / self.step + 0.5).floor() * self.step)
            .clamp(self.min, self.max);
        if (self.max - value).abs() < (snapped - value).abs() {
            self.max
        } else {
            snapped
        }
    }
}
