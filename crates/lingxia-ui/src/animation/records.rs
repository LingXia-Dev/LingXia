use super::Definition;
use crate::schema::{Record, identities};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone)]
struct Owner {
    list: String,
    key: String,
}

#[derive(Clone)]
struct Entry {
    definition: Definition,
    owners: Vec<Owner>,
}

#[derive(Clone, Default)]
pub(crate) struct Inventory(BTreeMap<String, Entry>);

struct List {
    keys: Option<HashSet<String>>,
    realized: HashSet<String>,
}

impl Inventory {
    pub(crate) fn definitions(&self) -> impl Iterator<Item = (String, Definition)> + '_ {
        self.0
            .iter()
            .map(|(id, entry)| (id.clone(), entry.definition.clone()))
    }

    /// Reconcile a fully measured View candidate, never an intermediate list window.
    pub(crate) fn reconcile(&self, records: &[Record]) -> Result<Self, String> {
        if self.0.is_empty() && !records.iter().any(|record| record.string(101).is_some()) {
            return Ok(Self::default());
        }
        let ids = identities(records);
        let mut lists: HashMap<String, List> = HashMap::new();
        let mut owners: Vec<Vec<Owner>> = Vec::with_capacity(records.len());
        for (index, record) in records.iter().enumerate() {
            let mut path = record.parent.map(|p| owners[p].clone()).unwrap_or_default();
            if let Some(parent) = record.parent.filter(|p| records[*p].tag == 6) {
                lists
                    .get_mut(&ids[parent])
                    .unwrap()
                    .realized
                    .insert(record.key.clone());
                path.push(Owner {
                    list: ids[parent].clone(),
                    key: record.key.clone(),
                });
            }
            owners.push(path);
            if record.tag == 6 {
                let keys = record
                    .string(102)
                    .map(crate::list::decode_item_keys)
                    .transpose()?
                    .map(|keys| keys.iter().map(|key| crate::list::item_key(key)).collect());
                lists.insert(
                    ids[index].clone(),
                    List {
                        keys,
                        realized: HashSet::new(),
                    },
                );
            }
        }
        let mut next = BTreeMap::new();
        for (id, entry) in &self.0 {
            for owner in &entry.owners {
                let Some(list) = lists.get(&owner.list) else {
                    break;
                };
                let keys = list
                    .keys
                    .as_ref()
                    .ok_or("UI_ANIMATION: virtual rows require itemKeys")?;
                if !keys.contains(&owner.key) {
                    break;
                }
                if !list.realized.contains(&owner.key) {
                    // An evicted outer row hides its nested lists as well.
                    next.insert(id.clone(), entry.clone());
                    break;
                }
            }
        }
        for (index, record) in records.iter().enumerate() {
            let Some(source) = record.string(101) else {
                continue;
            };
            for owner in &owners[index] {
                if lists[&owner.list].keys.is_none() {
                    return Err("UI_ANIMATION: virtual rows require itemKeys".into());
                }
            }
            next.insert(
                ids[index].clone(),
                Entry {
                    definition: super::decode_definition(source)?,
                    owners: owners[index].clone(),
                },
            );
        }
        Ok(Self(next))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(
        tag: usize,
        parent: Option<usize>,
        key: &str,
        fields: Vec<(usize, serde_json::Value)>,
    ) -> Record {
        Record {
            tag,
            parent,
            key: key.into(),
            test_id: String::new(),
            text: String::new(),
            fields,
        }
    }
    #[test]
    fn nested_inventories_preserve_evicted_descendants_and_prune_visible_removals() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../packages/lingxia-ui/tests/fixtures/animations.json"
        ))
        .unwrap();
        let mut records = vec![
            record(6, None, "", vec![(102, serde_json::json!("[\"outer\"]"))]),
            record(0, Some(0), "key:\"outer\"", vec![]),
            record(
                6,
                Some(1),
                "",
                vec![(102, serde_json::json!("[\"inner\"]"))],
            ),
            record(0, Some(2), "key:\"inner\"", vec![]),
            record(
                0,
                Some(3),
                "chart",
                vec![(101, serde_json::json!(fixture["chart-in"].to_string()))],
            ),
        ];
        let original = Inventory::default().reconcile(&records).unwrap();
        assert_eq!(original.definitions().count(), 1);
        assert_eq!(
            original
                .reconcile(&records[..1])
                .unwrap()
                .definitions()
                .count(),
            1
        );
        assert_eq!(
            original
                .reconcile(&records[..3])
                .unwrap()
                .definitions()
                .count(),
            1
        );
        assert_eq!(
            original
                .reconcile(&records[..2])
                .unwrap()
                .definitions()
                .count(),
            0
        );
        records[2].fields[0].1 = serde_json::json!("[]");
        assert_eq!(
            original
                .reconcile(&records[..3])
                .unwrap()
                .definitions()
                .count(),
            0
        );
        records[0].fields[0].1 = serde_json::json!("[]");
        assert_eq!(
            original
                .reconcile(&records[..1])
                .unwrap()
                .definitions()
                .count(),
            0
        );
        records[0].fields.clear();
        assert!(original.reconcile(&records[..1]).is_err());
        assert_eq!(
            original.definitions().count(),
            1,
            "failed collection leaves prior inventory intact"
        );
    }
}
