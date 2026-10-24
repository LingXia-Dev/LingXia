//! Shared fixtures for display-pipeline tests.
pub(crate) use crate::interaction::input_hits::*;
pub(crate) use crate::layout::Renderer;
pub(crate) use crate::protocol::Record;
#[cfg(feature = "gpu")]
pub(crate) use crate::scene::display::*;
pub(crate) use crate::scene::node::*;
pub(crate) use crate::scene::paint::*;
pub(crate) use crate::text::editing::*;
pub(crate) use crate::text::shape::*;
pub(crate) use serde_json::json;
pub(crate) use std::{collections::HashMap, sync::Arc};

pub(crate) fn node(
    tag: usize,
    parent: Option<usize>,
    key: &str,
    fields: &[(usize, serde_json::Value)],
) -> Record {
    Record {
        tag,
        parent,
        key: key.into(),
        test_id: key.into(),
        text: String::new(),
        fields: fields.to_vec(),
    }
}

pub(crate) fn page() -> Vec<Record> {
    vec![
        node(0, None, "root", &[(6, json!("#ffffff"))]),
        node(4, Some(0), "header", &[(1, json!(20))]),
        node(5, Some(0), "feed", &[(1, json!(60))]),
        node(
            3,
            Some(2),
            "first",
            &[
                (1, json!(50)),
                (6, json!("#ff0000")),
                (10, json!(0)),
                (8, json!(1)),
            ],
        ),
        node(
            3,
            Some(2),
            "second",
            &[
                (1, json!(50)),
                (6, json!("#0000ff")),
                (10, json!(0)),
                (8, json!(2)),
            ],
        ),
        node(4, Some(0), "footer", &[(1, json!(20))]),
    ]
}
