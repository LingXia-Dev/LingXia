use serde_json::Value;
use std::sync::Arc;

mod admission;
mod decode;
pub(crate) mod scales;
mod values;

#[rustfmt::skip]
mod generated;
pub use generated::{
    Children, ENVIRONMENT_KEYS, FIELD_ADMISSION, FIELDS, FieldAdmission, SCALED_FIELDS, SCALES,
    TAG_ADMISSION, TAGS, TagAdmission, UI_ABI, check_environment, field, limits, options, tag,
};

pub use decode::{Wire, decode, decode_wire};
pub use values::color;
pub(crate) use values::{flex_basis, focus_style, image_position, length_or_percent, part_style};

/// Absolute dimension fields paired with their percentage forms.
pub const DIMENSIONS: [(usize, usize); 6] = [
    (field::WIDTH, field::WIDTH_PERCENT),
    (field::HEIGHT, field::HEIGHT_PERCENT),
    (field::MIN_WIDTH, field::MIN_WIDTH_PERCENT),
    (field::MIN_HEIGHT, field::MIN_HEIGHT_PERCENT),
    (field::MAX_WIDTH, field::MAX_WIDTH_PERCENT),
    (field::MAX_HEIGHT, field::MAX_HEIGHT_PERCENT),
];
/// Top, right, bottom, left.
pub const PADDING_EDGES: [usize; 4] = [
    field::PADDING_TOP,
    field::PADDING_RIGHT,
    field::PADDING_BOTTOM,
    field::PADDING_LEFT,
];
/// Top, right, bottom, left.
pub const BORDER_EDGES: [usize; 4] = [
    field::BORDER_TOP_WIDTH,
    field::BORDER_RIGHT_WIDTH,
    field::BORDER_BOTTOM_WIDTH,
    field::BORDER_LEFT_WIDTH,
];
/// Top-left, top-right, bottom-right, bottom-left.
pub const RADIUS_CORNERS: [usize; 4] = [
    field::RADIUS_TOP_LEFT,
    field::RADIUS_TOP_RIGHT,
    field::RADIUS_BOTTOM_RIGHT,
    field::RADIUS_BOTTOM_LEFT,
];
/// The slider parts, in drawing order.
pub const PARTS: [usize; 4] = [
    field::TRACK_STYLE,
    field::SECONDARY_STYLE,
    field::RANGE_STYLE,
    field::THUMB_STYLE,
];
/// What a slider part record may carry.
pub const PART_FIELDS: [usize; 16] = [
    field::BACKGROUND,
    field::WIDTH,
    field::HEIGHT,
    field::RADIUS,
    field::RADIUS_TOP_LEFT,
    field::RADIUS_TOP_RIGHT,
    field::RADIUS_BOTTOM_RIGHT,
    field::RADIUS_BOTTOM_LEFT,
    field::BORDER_TOP_WIDTH,
    field::BORDER_RIGHT_WIDTH,
    field::BORDER_BOTTOM_WIDTH,
    field::BORDER_LEFT_WIDTH,
    field::BORDER_COLOR,
    field::BOX_SHADOW,
    field::RING,
    field::TRANSFORM,
];
/// Paint-only fields a state delta may carry; a slider's parts change only its drawing.
pub const FOCUS_PAINT_FIELDS: [usize; 13] = [
    field::COLOR,
    field::BACKGROUND,
    field::OPACITY,
    field::BORDER_COLOR,
    field::BACKGROUND_GRADIENT,
    field::BOX_SHADOW,
    field::RING,
    field::PLACEHOLDER_COLOR,
    field::TRANSFORM,
    field::TRACK_STYLE,
    field::RANGE_STYLE,
    field::SECONDARY_STYLE,
    field::THUMB_STYLE,
];

#[derive(Clone, Debug)]
pub struct Record {
    pub tag: usize,
    pub parent: Option<usize>,
    pub key: String,
    pub test_id: String,
    pub text: String,
    pub fields: Fields,
}

/// A record's fields, shared by every frame that reuses the record until one edits them; with
/// the theme step each resolved field came from, when any did.
#[derive(Clone, Default, PartialEq)]
pub struct Fields(
    Arc<Vec<(usize, Value)>>,
    Option<Arc<[(usize, scales::Step)]>>,
);

impl Fields {
    pub(crate) fn with_steps(
        fields: Vec<(usize, Value)>,
        steps: Vec<(usize, scales::Step)>,
    ) -> Self {
        Self(Arc::new(fields), (!steps.is_empty()).then(|| steps.into()))
    }
    /// The theme steps resolved into these fields, by field id.
    pub(crate) fn steps(&self) -> &[(usize, scales::Step)] {
        self.1.as_deref().unwrap_or_default()
    }
    /// Same allocation: the fields were carried over, not decoded again.
    pub fn ptr_eq(a: &Self, b: &Self) -> bool {
        Arc::ptr_eq(&a.0, &b.0)
    }
    pub fn into_vec(self) -> Vec<(usize, Value)> {
        Arc::try_unwrap(self.0).unwrap_or_else(|shared| (*shared).clone())
    }
}

impl std::ops::Deref for Fields {
    type Target = Vec<(usize, Value)>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Copies on first write so a frame's overrides never reach the decoded records it shares.
impl std::ops::DerefMut for Fields {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}

impl From<Vec<(usize, Value)>> for Fields {
    fn from(fields: Vec<(usize, Value)>) -> Self {
        Self(Arc::new(fields), None)
    }
}

impl std::fmt::Debug for Fields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<'a> IntoIterator for &'a Fields {
    type Item = &'a (usize, Value);
    type IntoIter = std::slice::Iter<'a, (usize, Value)>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut Fields {
    type Item = &'a mut (usize, Value);
    type IntoIter = std::slice::IterMut<'a, (usize, Value)>;
    fn into_iter(self) -> Self::IntoIter {
        Arc::make_mut(&mut self.0).iter_mut()
    }
}

impl IntoIterator for Fields {
    type Item = (usize, Value);
    type IntoIter = std::vec::IntoIter<(usize, Value)>;
    fn into_iter(self) -> Self::IntoIter {
        self.into_vec().into_iter()
    }
}

impl Record {
    pub fn number(&self, field: usize) -> Option<f32> {
        self.fields
            .iter()
            .find(|(id, _)| *id == field)?
            .1
            .as_f64()
            .map(|v| v as f32)
    }
    pub fn string(&self, field: usize) -> Option<&str> {
        self.fields.iter().find(|(id, _)| *id == field)?.1.as_str()
    }
}

/// Whether `record` carries the anchor `name`: an element that anchors several popovers
/// carries every name, a line each.
pub(crate) fn anchors(record: &Record, name: &str) -> bool {
    record
        .string(field::ANCHOR_NAME)
        .is_some_and(|names| names.split('\n').any(|own| own == name))
}

pub(crate) fn identities(records: &[Record]) -> Vec<String> {
    let mut identities: Vec<String> = Vec::with_capacity(records.len());
    let mut children = vec![0; records.len()];
    for record in records {
        let ordinal = record
            .parent
            .map(|p| {
                let n = children[p];
                children[p] += 1;
                n
            })
            .unwrap_or(0);
        let segment = if let Some(slot) = record
            .string(field::CHILD_SLOT)
            .filter(|_| record.key.is_empty())
        {
            // The authored position, holes included: a sibling appearing before it changes nothing.
            format!("i:{slot}")
        } else if record.key.is_empty() {
            format!("i:{ordinal}")
        } else {
            format!("k:{}:{}", record.key.len(), record.key)
        };
        identities.push(format!(
            "{}/{segment}:{}",
            record.parent.map(|p| identities[p].as_str()).unwrap_or(""),
            record.tag
        ));
    }
    identities
}

#[cfg(test)]
pub(crate) mod tests;
