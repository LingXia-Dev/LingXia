//! Stable virtual accessibility ids: they follow node identity across
//! reordering and are never reused after removal or reset.
use std::collections::HashMap;

#[derive(Default)]
pub struct SemanticIds {
    by_identity: HashMap<String, u32>,
    by_id: HashMap<u32, usize>,
    ids: Vec<Option<u32>>,
    next: u32,
}

impl SemanticIds {
    pub fn clear(&mut self) {
        self.by_identity.clear();
        self.by_id.clear();
        self.ids.clear();
    }

    pub fn update<'a>(&mut self, identities: impl IntoIterator<Item = &'a str>) {
        let mut retained = HashMap::new();
        self.by_id.clear();
        self.ids.clear();
        for (index, identity) in identities.into_iter().enumerate() {
            let id = if identity.is_empty() {
                None
            } else if let Some(id) = self.by_identity.get(identity) {
                Some(*id)
            } else if self.next < i32::MAX as u32 {
                self.next += 1;
                Some(self.next)
            } else {
                None
            };
            if let Some(id) = id {
                retained.insert(identity.to_owned(), id);
                self.by_id.insert(id, index);
            }
            self.ids.push(id);
        }
        self.by_identity = retained;
    }

    pub fn id(&self, index: usize) -> Option<u32> {
        self.ids.get(index).copied().flatten()
    }

    pub fn index(&self, id: u32) -> Option<usize> {
        self.by_id.get(&id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_follow_identity_and_are_never_reused() {
        let mut ids = SemanticIds::default();
        ids.update(["a", "b", ""]);
        let (a, b) = (ids.id(0).unwrap(), ids.id(1).unwrap());
        assert_eq!(ids.id(2), None);
        ids.update(["b", "a"]);
        assert_eq!((ids.id(0), ids.id(1)), (Some(b), Some(a)));
        assert_eq!(ids.index(a), Some(1));
        ids.update(["b"]);
        ids.update(["b", "a"]);
        assert_ne!(ids.id(1), Some(a), "a removed identity gets a fresh id");
        ids.clear();
        ids.update(["b"]);
        assert_ne!(ids.id(0), Some(b));
    }
}
