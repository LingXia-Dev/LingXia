//! Composed controls: descendants of a button, checkbox or switch form one semantic node, except the
//! controls nested in it, which stay their own.
use crate::protocol::{Record, tag};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;

pub(crate) struct ControlGroups {
    /// The composed control that owns each record, if any.
    pub(crate) owners: Vec<Option<usize>>,
    /// A digest of each owner's content; gestures cannot survive a changed label.
    pub(crate) content: HashMap<usize, DefaultHasher>,
    /// Descendant text and image alternatives, in order: the name of an unlabeled control.
    pub(crate) names: HashMap<usize, String>,
}

/// Tags that take input of their own.
pub(crate) fn interactive(tag: usize) -> bool {
    matches!(
        tag,
        tag::BUTTON
            | tag::LINK
            | tag::LINK_BOX
            | tag::CHECKBOX
            | tag::SWITCH
            | tag::RADIO
            | tag::INPUT
            | tag::TEXTAREA
            | tag::SEARCH
            | tag::SELECT
            | tag::SLIDER
    )
}

pub(crate) fn control_groups(records: &[Record]) -> Result<ControlGroups, String> {
    let mut button_owners: Vec<Option<usize>> = Vec::with_capacity(records.len());
    let mut button_hashes: HashMap<usize, DefaultHasher> = HashMap::new();
    let mut names: HashMap<usize, String> = HashMap::new();
    // Hidden from assistive tech, by itself or by an ancestor: decoration that names nothing.
    let mut hidden: Vec<bool> = Vec::with_capacity(records.len());
    for record in records {
        hidden.push(
            record.number(crate::protocol::field::ACCESSIBILITY_HIDDEN) == Some(1.0)
                || record.parent.is_some_and(|parent| hidden[parent]),
        );
        // A control nested in a composed one is its own target and semantic node; the
        // outer control's name and content digest leave it out.
        let owner = record
            .parent
            .and_then(|parent| {
                if matches!(
                    records[parent].tag,
                    tag::BUTTON | tag::LINK_BOX | tag::CHECKBOX | tag::SWITCH
                ) {
                    Some(parent)
                } else {
                    button_owners[parent]
                }
            })
            .filter(|_| !interactive(record.tag));
        button_owners.push(owner);
        if let Some(owner) = owner {
            let part = if record.tag == tag::IMAGE {
                record
                    .string(crate::protocol::field::IMAGE_ALT)
                    .unwrap_or("")
            } else {
                record.text.as_str()
            };
            if !part.trim().is_empty() && !hidden[hidden.len() - 1] {
                let name = names.entry(owner).or_default();
                if !name.is_empty() {
                    name.push(' ');
                }
                name.push_str(part.trim());
            }
            // A gesture cannot survive a changed child label or image merely
            // because the outer button's identity and bounds stayed the same.
            let hash = button_hashes.entry(owner).or_default();
            record.tag.hash(hash);
            record.key.hash(hash);
            record.text.hash(hash);
            record.parent.hash(hash);
            serde_json::to_string(record.fields.as_slice())
                .map_err(|error| error.to_string())?
                .hash(hash);
        }
    }
    Ok(ControlGroups {
        owners: button_owners,
        content: button_hashes,
        names,
    })
}
