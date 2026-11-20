//! Controls the page leaves unbound (no `value`, `checked` or `bind`) own their state here, as
//! the web's uncontrolled controls do: a re-render or a dev hot-swap keeps it, unmounting drops
//! it, and the control's change callback still hears each new value. A control the page binds
//! is controlled exactly as before and keeps nothing here.
use crate::protocol::{Record, field, identities, tag};
use crate::scene::slider::SliderSnapshot;
use std::collections::HashMap;

/// A control's own value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ControlValue {
    Checked(bool),
    Number(f64),
}

impl ControlValue {
    /// The value as its change callback receives it.
    fn json(&self) -> String {
        match self {
            Self::Checked(on) => on.to_string(),
            Self::Number(value) => serde_json::Value::from(*value).to_string(),
        }
    }

    fn field(&self) -> serde_json::Value {
        match self {
            Self::Checked(on) => serde_json::Value::from(u8::from(*on)),
            Self::Number(value) => serde_json::Value::from(*value),
        }
    }
}

/// One unbound control in the current records.
#[derive(Clone, Debug)]
struct Unbound {
    /// Its change callback in these records; a change dispatches it with the new value.
    handle: Option<u64>,
    /// A slider's range, which snaps a proposed value as the slider draws it.
    range: Option<SliderSnapshot>,
}

/// What a proposed value did to an unbound control.
pub(crate) enum Proposal {
    /// Not an unbound control's handle: the page decides, as for any bound control.
    Bound,
    /// The control already holds that value; its callback does not run.
    Unchanged,
    /// The control took the value; its callback receives this encoding of it.
    Changed(String),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Uncontrolled {
    values: HashMap<String, ControlValue>,
    unbound: HashMap<String, Unbound>,
}

/// The field holding a control's value and the one naming its change callback, when the tag is
/// one with its own state.
fn fields(tag: usize) -> Option<(usize, usize)> {
    match tag {
        tag::CHECKBOX | tag::SWITCH => Some((field::CHECKED, field::ON_CHECKED_CHANGE)),
        tag::SLIDER => Some((field::SLIDER_VALUE, field::ON_SLIDER_CHANGE)),
        _ => None,
    }
}

impl Uncontrolled {
    /// Takes the unbound controls of `records`. A control that left, or that the page now
    /// binds, drops its own value.
    pub(crate) fn reconcile(&mut self, records: &[Record]) {
        self.unbound.clear();
        let ids = identities(records);
        for (record, id) in records.iter().zip(ids) {
            let Some((value, callback)) = fields(record.tag) else {
                continue;
            };
            if record.fields.iter().any(|(field, _)| *field == value) {
                continue;
            }
            let handle = record
                .fields
                .iter()
                .find(|(field, _)| *field == callback)
                .and_then(|(_, handle)| handle.as_u64())
                .filter(|handle| *handle != 0);
            let range = (record.tag == tag::SLIDER)
                .then(|| SliderSnapshot::from_record(record).ok())
                .flatten();
            self.unbound.insert(id, Unbound { handle, range });
        }
        self.values.retain(|id, _| self.unbound.contains_key(id));
    }

    /// Writes each unbound control's own value into `records` for layout and paint.
    pub(crate) fn apply(&self, records: &mut [Record]) {
        if self.values.is_empty() {
            return;
        }
        let ids = identities(records);
        for (record, id) in records.iter_mut().zip(ids) {
            let (Some(value), Some((field, _))) = (self.values.get(&id), fields(record.tag)) else {
                continue;
            };
            if self.unbound.contains_key(&id) {
                record.fields.push((field, value.field()));
            }
        }
    }

    /// A press ran `handle`: when it is an unbound checkbox's or switch's, that control takes
    /// its next value and the encoded value its callback receives is returned.
    pub(crate) fn press(&mut self, handle: u64) -> Option<String> {
        let (id, _) = self
            .unbound
            .iter()
            .find(|(_, unbound)| unbound.handle == Some(handle) && unbound.range.is_none())?;
        // Unchecked until pressed; a mixed box checks on its first press, as in the DOM.
        let next = match self.values.get(id) {
            Some(ControlValue::Checked(on)) => ControlValue::Checked(!on),
            _ => ControlValue::Checked(true),
        };
        let json = next.json();
        self.values.insert(id.clone(), next);
        Some(json)
    }

    /// A drag proposed `value` through `handle`: an unbound slider snaps it to its range and
    /// takes it.
    pub(crate) fn propose(&mut self, handle: u64, value: f64) -> Proposal {
        let Some((id, range)) = self.unbound.iter().find_map(|(id, unbound)| {
            (unbound.handle == Some(handle))
                .then_some(unbound.range.as_ref())
                .flatten()
                .map(|range| (id, range))
        }) else {
            return Proposal::Bound;
        };
        let next = range.normalize(value);
        let current = match self.values.get(id) {
            Some(ControlValue::Number(current)) => *current,
            _ => range.value,
        };
        if next == current {
            return Proposal::Unchanged;
        }
        let next = ControlValue::Number(next);
        let json = next.json();
        self.values.insert(id.clone(), next);
        Proposal::Changed(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn record(tag: usize, parent: Option<usize>, fields: Vec<(usize, Value)>) -> Record {
        Record {
            tag,
            parent,
            key: String::new(),
            test_id: String::new(),
            text: String::new(),
            fields: fields.into(),
        }
    }

    fn checked(records: &[Record], state: &Uncontrolled) -> Vec<Option<f32>> {
        let mut painted = records.to_vec();
        state.apply(&mut painted);
        painted
            .iter()
            .map(|record| record.number(field::CHECKED))
            .collect()
    }

    fn page(boxes: Vec<Record>) -> Vec<Record> {
        let mut records = vec![record(tag::COLUMN, None, vec![])];
        records.extend(boxes);
        records
    }

    #[test]
    fn an_unbound_checkbox_toggles_and_keeps_its_value_across_renders() {
        let records = page(vec![record(
            tag::CHECKBOX,
            Some(0),
            vec![(field::ON_CHECKED_CHANGE, json!(7))],
        )]);
        let mut state = Uncontrolled::default();
        state.reconcile(&records);
        assert_eq!(checked(&records, &state)[1], None);
        assert_eq!(state.press(7).as_deref(), Some("true"));
        assert_eq!(checked(&records, &state)[1], Some(1.0));
        // The same control in a re-render, under a new callback handle.
        let rerender = page(vec![record(
            tag::CHECKBOX,
            Some(0),
            vec![(field::ON_CHECKED_CHANGE, json!(9))],
        )]);
        state.reconcile(&rerender);
        assert_eq!(checked(&rerender, &state)[1], Some(1.0));
        assert_eq!(state.press(7), None);
        assert_eq!(state.press(9).as_deref(), Some("false"));
        assert_eq!(checked(&rerender, &state)[1], Some(0.0));
    }

    #[test]
    fn a_bound_control_keeps_nothing_and_an_unmounted_one_forgets() {
        let bound = page(vec![record(
            tag::SWITCH,
            Some(0),
            vec![
                (field::CHECKED, json!(0)),
                (field::ON_CHECKED_CHANGE, json!(3)),
            ],
        )]);
        let mut state = Uncontrolled::default();
        state.reconcile(&bound);
        assert_eq!(state.press(3), None);
        assert_eq!(checked(&bound, &state)[1], Some(0.0));

        let unbound = page(vec![record(
            tag::SWITCH,
            Some(0),
            vec![(field::ON_CHECKED_CHANGE, json!(4))],
        )]);
        state.reconcile(&unbound);
        assert_eq!(state.press(4).as_deref(), Some("true"));
        state.reconcile(&page(vec![]));
        state.reconcile(&unbound);
        assert_eq!(checked(&unbound, &state)[1], None);
    }

    #[test]
    fn a_mixed_unbound_checkbox_checks_on_its_first_press() {
        let records = page(vec![record(
            tag::CHECKBOX,
            Some(0),
            vec![
                (field::MIXED, json!(1)),
                (field::ON_CHECKED_CHANGE, json!(5)),
            ],
        )]);
        let mut state = Uncontrolled::default();
        state.reconcile(&records);
        assert_eq!(state.press(5).as_deref(), Some("true"));
        assert_eq!(state.press(5).as_deref(), Some("false"));
    }

    fn slider(handle: u64, step: f64) -> Record {
        record(
            tag::SLIDER,
            Some(0),
            vec![
                (field::ACCESSIBILITY_LABEL, json!("Volume")),
                (field::SLIDER_MIN, json!(0)),
                (field::SLIDER_MAX, json!(10)),
                (field::SLIDER_STEP, json!(step)),
                (field::ON_SLIDER_CHANGE, json!(handle)),
            ],
        )
    }

    fn proposed(state: &mut Uncontrolled, handle: u64, value: f64) -> Option<String> {
        match state.propose(handle, value) {
            Proposal::Changed(json) => Some(json),
            Proposal::Unchanged => None,
            Proposal::Bound => Some("bound".into()),
        }
    }

    fn slider_value(records: &[Record], state: &Uncontrolled) -> Option<f32> {
        let mut painted = records.to_vec();
        state.apply(&mut painted);
        painted[1].number(field::SLIDER_VALUE)
    }

    #[test]
    fn an_unbound_slider_snaps_takes_and_keeps_a_dragged_value() {
        let records = page(vec![slider(7, 3.0)]);
        let mut state = Uncontrolled::default();
        state.reconcile(&records);
        assert_eq!(slider_value(&records, &state), None);
        // Snapped as drawn; the minimum it starts at is no change.
        assert_eq!(proposed(&mut state, 7, 0.4), None);
        assert_eq!(proposed(&mut state, 7, 9.6).as_deref(), Some("10.0"));
        assert_eq!(proposed(&mut state, 7, 9.9), None);
        assert_eq!(slider_value(&records, &state), Some(10.0));
        let rerender = page(vec![slider(9, 3.0)]);
        state.reconcile(&rerender);
        assert_eq!(slider_value(&rerender, &state), Some(10.0));
        assert_eq!(proposed(&mut state, 9, 4.0).as_deref(), Some("3.0"));
        // A click on a slider's handle is not a checkbox press.
        assert_eq!(state.press(9), None);
    }

    #[test]
    fn a_bound_slider_is_left_to_the_page() {
        let mut bound = slider(7, 1.0);
        bound.fields.push((field::SLIDER_VALUE, json!(4)));
        let records = page(vec![bound]);
        let mut state = Uncontrolled::default();
        state.reconcile(&records);
        assert_eq!(proposed(&mut state, 7, 6.0).as_deref(), Some("bound"));
        assert_eq!(slider_value(&records, &state), Some(4.0));
    }
}
