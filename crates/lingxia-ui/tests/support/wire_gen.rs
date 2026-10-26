//! Wire buffers from arbitrary bytes, shared by the fuzz targets and the property tests.
//! Most inputs follow the admission tables so they reach layout; the rest break headers,
//! ids and payloads on purpose.
#![allow(dead_code)]

use lingxia_ui::protocol::{
    Children, FIELD_ADMISSION, FIELDS, TAG_ADMISSION, TAGS, field, options, tag,
};
use serde_json::{Value, json};

/// Bytes read front to back; exhausted input reads as zeros.
pub struct Bytes<'a>(pub &'a [u8]);

impl Bytes<'_> {
    pub fn byte(&mut self) -> u8 {
        let Some((&first, rest)) = self.0.split_first() else {
            return 0;
        };
        self.0 = rest;
        first
    }
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            usize::from(u16::from_le_bytes([self.byte(), self.byte()])) % n
        }
    }
    pub fn chance(&mut self, percent: u8) -> bool {
        self.byte() % 100 < percent
    }
    pub fn pick<'t, T>(&mut self, items: &'t [T]) -> &'t T {
        &items[self.below(items.len())]
    }
    pub fn done(&self) -> bool {
        self.0.is_empty()
    }
}

const NUMBERS: &[f64] = &[
    0.0,
    1.0,
    -1.0,
    0.5,
    2.0,
    8.0,
    12.5,
    16.0,
    44.0,
    100.0,
    320.0,
    1000.0,
    -40.0,
    1e6,
    -1e6,
    1e30,
    -1e30,
    1e-9,
    65535.0,
    4294967296.0,
];
const WORDS: &[&str] = &[
    "",
    "auto",
    "100%",
    "50%",
    "-10%",
    "1fr",
    "repeat(3,1fr)",
    "minmax(0,1fr) auto",
    "span 2",
    "1 / 3",
    "accent",
    "text/40",
    "#ff000080",
    "#12",
    "surface",
    "none",
    "center",
    "start",
    "stretch",
    "space-between",
    "row",
    "column",
    "wrap",
    "hidden",
    "absolute",
    "relative",
    "sticky",
    "Hello wörld",
    "مرحبا",
    "日本語のテキスト",
    "a\u{0}b",
    "{\"x\":1}",
    "M0 0L10 10Z",
    "1fr 2fr",
    "ellipsis",
    "pre-wrap",
    "break-all",
    "999999999999999999999",
];
const KEYS: &[&str] = &[
    "name",
    "durationMs",
    "delayMs",
    "iterations",
    "whenReduced",
    "easing",
    "opacity",
    "transforms",
    "properties",
    "kind",
    "offset",
    "value",
    "x",
    "y",
    "color",
    "blur",
    "spread",
    "inset",
    "stops",
    "angle",
    "label",
    "disabled",
    "unknown",
];

pub fn number(b: &mut Bytes) -> Value {
    let n = *b.pick(NUMBERS);
    if b.chance(30) {
        json!(n.trunc() as i64)
    } else {
        json!(n * (1.0 + f64::from(b.byte()) / 64.0))
    }
}

pub fn chaos(b: &mut Bytes, depth: u8) -> Value {
    match b.below(if depth > 2 { 5 } else { 7 }) {
        0 => Value::Null,
        1 => json!(b.chance(50)),
        2 | 3 => number(b),
        4 => json!(*b.pick(WORDS)),
        5 => Value::Array((0..b.below(6)).map(|_| chaos(b, depth + 1)).collect()),
        _ => Value::Object(
            (0..b.below(5))
                .map(|_| ((*b.pick(KEYS)).to_owned(), chaos(b, depth + 1)))
                .collect(),
        ),
    }
}

/// A well-formed payload for the field, or `None` when the kind has no generator here.
pub fn valid(b: &mut Bytes, id: usize) -> Option<Value> {
    let (name, kind) = FIELDS[id];
    if let Some(choices) = options(kind) {
        return Some(json!(*b.pick(choices)));
    }
    let length = |b: &mut Bytes| json!(f64::from(b.byte()) * 2.0 * f64::from(b.below(3) as u8));
    Some(match kind {
        _ if name.contains("opacity") => json!(f64::from(b.byte()) / 255.0),
        _ if name == "fontWeight" => json!(100 * (1 + b.below(9))),
        _ if name == "fontSize" || name == "lineHeight" => json!(1 + b.below(64)),
        "string" => json!(*b.pick(&[
            "accent",
            "surface",
            "text",
            "text/40",
            "line",
            "on-accent",
            "#ff000080"
        ])),
        "dimension-percent" => json!(b.below(101)),
        "dimension-ch" => json!(1 + b.below(80)),
        "aspect-ratio" => json!(0.25 + f64::from(b.byte()) / 64.0),
        "hidden-flag" => json!(1),
        _ if name.starts_with("backdrop") => json!(b.below(16)),
        "z-index" | "order" => json!(b.below(20) as i64 - 10),
        "margin" | "inset" => json!(f64::from(b.byte()) - 128.0),
        "number" | "slider-number" => length(b),
        "flex-basis" => {
            if b.chance(70) {
                length(b)
            } else {
                json!(format!("{}%", b.below(101)))
            }
        }
        "flag" | "input-flag" | "plot-flag" | "check-flag" | "semantic-state" => json!(b.below(2)),
        "handle" | "input-handle" | "image-handle" | "reach-handle" | "check-handle"
        | "slider-handle" | "select-handle" | "plot-handle" => json!(b.below(1000) + 1),
        "textarea-rows" => json!(1 + b.below(8)),
        "list-count" => json!(b.below(50)),
        "list-extent" => json!(f64::from(b.byte())),
        "image-src" | "image-source" => json!(*b.pick(&["a.png", "images/b.jpg"])),
        "list-id" => json!(format!("list{}", b.below(4))),
        "grid-template" => json!(*b.pick(&[
            "1fr 1fr",
            "repeat(3,1fr)",
            "100px auto",
            "minmax(0,1fr) 2fr"
        ])),
        "input-text" | "image-alt" => json!(*b.pick(&WORDS[26..30])),
        _ => return None,
    })
}

/// A payload shaped for the field's kind, or chaos.
pub fn payload(b: &mut Bytes, id: usize) -> Value {
    if !b.chance(15)
        && let Some(value) = valid(b, id)
    {
        return value;
    }
    if b.chance(50) {
        json!(*b.pick(WORDS))
    } else {
        chaos(b, 0)
    }
}

fn admitted(id: usize, tag: usize, parent: Option<usize>) -> bool {
    FIELDS[id].1 != "reserved"
        && FIELD_ADMISSION[id]
            .tags
            .is_none_or(|tags| tags.contains(&tag))
        && FIELD_ADMISSION[id].parent.is_none_or(|p| Some(p) == parent)
}

/// A wire buffer: usually admitted structure with shaped payloads, sometimes raw chaos.
pub fn buffer(data: &[u8]) -> Vec<Value> {
    let mut b = Bytes(data);
    let raw = b.chance(10);
    // Hostile buffers bend payloads; the rest stay well formed so they reach layout.
    let hostile = b.chance(35);
    let mut wire: Vec<Value> = Vec::new();
    // Tag, wire offset, and whether it carries inline text (then it takes no children).
    let mut nodes: Vec<(usize, i64, bool)> = Vec::new();
    let records = 1 + b.below(24);
    for index in 0..records {
        if raw {
            for _ in 0..6 + b.below(10) {
                wire.push(chaos(&mut b, 0));
            }
            continue;
        }
        let (parent, tag) = if index == 0 {
            (
                None,
                *b.pick(&[
                    tag::COLUMN,
                    tag::PAGE,
                    tag::ROW,
                    tag::STACK,
                    tag::GRID,
                    tag::SCROLL,
                ]),
            )
        } else {
            let containers: Vec<usize> = (0..nodes.len())
                .filter(|&n| {
                    !nodes[n].2
                        && matches!(
                            TAG_ADMISSION[nodes[n].0].children,
                            Children::Any | Children::Runs | Children::Content
                        )
                })
                .collect();
            if containers.is_empty() {
                break;
            }
            let parent = containers[b.below(containers.len())];
            let owner = nodes[parent].0;
            let tag = match TAG_ADMISSION[owner].children {
                Children::Runs => *b.pick(&[tag::SPAN, tag::LINK]),
                Children::Content if !hostile => {
                    *b.pick(&[tag::TEXT, tag::IMAGE, tag::COLUMN, tag::ROW])
                }
                _ if !hostile => *b.pick(&[
                    tag::COLUMN,
                    tag::ROW,
                    tag::TEXT,
                    tag::STACK,
                    tag::GRID,
                    tag::SCROLL,
                    tag::BUTTON,
                    tag::SPACER,
                    tag::INPUT,
                    tag::CHECKBOX,
                    tag::SLIDER,
                    tag::IMAGE,
                ]),
                _ => b.below(TAGS.len()),
            };
            (Some(parent), tag)
        };
        let value = |b: &mut Bytes, id| {
            if hostile {
                payload(b, id)
            } else {
                valid(b, id).unwrap_or_else(|| payload(b, id))
            }
        };
        let mut fields: Vec<(usize, Value)> = TAG_ADMISSION[tag]
            .requires
            .iter()
            .map(|&id| (id, value(&mut b, id)))
            .collect();
        // Controls a screen reader announces need a name before they are admitted.
        if !hostile
            && matches!(
                tag,
                tag::SLIDER
                    | tag::INPUT
                    | tag::TEXTAREA
                    | tag::SEARCH
                    | tag::RADIO
                    | tag::PLOT
                    | tag::SELECT
            )
        {
            fields.push((field::ACCESSIBILITY_LABEL, json!("Name")));
        }
        let parent_tag = parent.map(|p| nodes[p].0);
        for _ in 0..b.below(if hostile { 10 } else { 6 }) {
            let id = b.below(FIELDS.len());
            let shaped = hostile || valid(&mut Bytes(&[]), id).is_some();
            if shaped
                && (admitted(id, tag, parent_tag) || (hostile && b.chance(5)))
                && fields.iter().all(|(old, _)| *old != id)
            {
                fields.push((id, value(&mut b, id)));
            }
        }
        let text = match TAG_ADMISSION[tag].children {
            Children::Text | Children::Content if b.chance(70) => *b.pick(&WORDS[26..30]),
            _ => "",
        };
        let key = if TAG_ADMISSION[tag].keyed || b.chance(20) {
            format!("k{index}")
        } else {
            String::new()
        };
        let offset = wire.len() as i64;
        let parent_offset = parent.map(|p| nodes[p].1).unwrap_or(-1);
        wire.extend([
            json!(tag),
            json!(parent_offset),
            json!(key),
            json!(format!("n{index}")),
            json!(text),
            json!(fields.len()),
        ]);
        for (id, value) in fields {
            wire.push(json!(id));
            wire.push(value);
        }
        nodes.push((tag, offset, !text.is_empty()));
    }
    wire
}

/// Viewport for a buffer: odd sizes and scales included.
pub fn viewport(data: &[u8]) -> (u16, u16, f32) {
    let mut b = Bytes(data.get(data.len().saturating_sub(3)..).unwrap_or_default());
    let width = *b.pick(&[1u16, 64, 200, 393, 820, 1600]);
    let height = *b.pick(&[1u16, 64, 300, 852, 1180]);
    (width, height, *b.pick(&[1.0f32, 2.0, 3.0, 1.5]))
}
