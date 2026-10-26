use serde_json::Value;

mod admission;
mod decode;
mod values;

#[rustfmt::skip]
mod generated;
pub use generated::{
    Children, ENVIRONMENT_KEYS, FIELD_ADMISSION, FIELDS, FieldAdmission, TAG_ADMISSION, TAGS,
    TagAdmission, UI_ABI, check_environment, field, limits, options, tag,
};

pub(crate) use decode::MAX_BUFFER_VALUES;
pub use decode::decode;
pub use values::color;
pub(crate) use values::{flex_basis, focus_style, image_position, length_or_percent};

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
/// Paint-only fields an editor focus delta may carry.
pub const FOCUS_PAINT_FIELDS: [usize; 8] = [
    field::COLOR,
    field::BACKGROUND,
    field::OPACITY,
    field::BORDER_COLOR,
    field::BACKGROUND_GRADIENT,
    field::BOX_SHADOW,
    field::PLACEHOLDER_COLOR,
    field::TRANSFORM,
];

#[derive(Clone, Debug)]
pub struct Record {
    pub tag: usize,
    pub parent: Option<usize>,
    pub key: String,
    pub test_id: String,
    pub text: String,
    pub fields: Vec<(usize, Value)>,
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
        let segment = if record.key.is_empty() {
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
mod tests;
