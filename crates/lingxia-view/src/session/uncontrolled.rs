//! Controls the page leaves unbound (no `value`, `checked` or `bind`) own their state here, as
//! the web's uncontrolled controls do: a re-render or a dev hot-swap keeps it, unmounting drops
//! it, and the control's change callback still hears each new value. A control the page binds
//! is controlled exactly as before and keeps nothing here.
use crate::protocol::{Record, field, identities, tag};
use std::collections::HashMap;

/// A control's own value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ControlValue {
    Checked(bool),
}

impl ControlValue {
    /// The value as its change callback receives it.
    fn json(&self) -> String {
        match self {
            Self::Checked(on) => on.to_string(),
        }
    }
}

/// One unbound control in the current records.
#[derive(Clone, Debug)]
struct Unbound {
    /// Its change callback in these records; a press dispatches it with the new value.
    handle: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Uncontrolled {
    values: HashMap<String, ControlValue>,
    unbound: HashMap<String, Unbound>,
}

/// The field holding a control's value, when the tag is one with its own state.
fn value_field(tag: usize) -> Option<usize> {
    match tag {
        tag::CHECKBOX | tag::SWITCH => Some(field::CHECKED),
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
            let Some(value) = value_field(record.tag) else {
                continue;
            };
            if record.fields.iter().any(|(field, _)| *field == value) {
                continue;
            }
            let handle = record
                .fields
                .iter()
                .find(|(field, _)| *field == field::ON_CHECKED_CHANGE)
                .and_then(|(_, handle)| handle.as_u64())
                .filter(|handle| *handle != 0);
            self.unbound.insert(id, Unbound { handle });
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
            let (Some(value), Some(field)) = (self.values.get(&id), value_field(record.tag)) else {
                continue;
            };
            if !self.unbound.contains_key(&id) {
                continue;
            }
            let ControlValue::Checked(on) = value;
            record
                .fields
                .push((field, serde_json::Value::from(u8::from(*on))));
        }
    }

    /// A press ran `handle`: when it is an unbound control's, that control takes its next value
    /// and the encoded value its callback receives is returned.
    pub(crate) fn press(&mut self, handle: u64) -> Option<String> {
        let (id, _) = self
            .unbound
            .iter()
            .find(|(_, unbound)| unbound.handle == Some(handle))?;
        // Unchecked until pressed; a mixed box checks on its first press, as in the DOM.
        let next = match self.values.get(id) {
            Some(ControlValue::Checked(on)) => ControlValue::Checked(!on),
            None => ControlValue::Checked(true),
        };
        let json = next.json();
        self.values.insert(id.clone(), next);
        Some(json)
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
}
