//! Composed controls: descendants of a button or checkbox form one semantic node.
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

pub(crate) fn control_groups(records: &[Record]) -> Result<ControlGroups, String> {
    let mut button_owners: Vec<Option<usize>> = Vec::with_capacity(records.len());
    let mut button_hashes: HashMap<usize, DefaultHasher> = HashMap::new();
    let mut names: HashMap<usize, String> = HashMap::new();
    for record in records {
        let owner = record.parent.and_then(|parent| {
            if matches!(records[parent].tag, tag::BUTTON | tag::CHECKBOX) {
                Some(parent)
            } else {
                button_owners[parent]
            }
        });
        button_owners.push(owner);
        if let Some(owner) = owner {
            let part = if record.tag == tag::IMAGE {
                record
                    .string(crate::protocol::field::IMAGE_ALT)
                    .unwrap_or("")
            } else {
                record.text.as_str()
            };
            if !part.trim().is_empty() {
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
            serde_json::to_string(&record.fields)
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
