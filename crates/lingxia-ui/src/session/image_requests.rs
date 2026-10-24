use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, serde::Serialize)]
pub struct ImageRequest {
    pub source: String,
    pub retry_key: String,
    pub token: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ImageSource {
    pub source: String,
    #[serde(default)]
    pub retry_key: String,
}

struct Entry {
    request: ImageRequest,
    started: bool,
}

/// A surface owner's queue, retained across page reloads until the owner closes.
#[derive(Default)]
pub struct ImageRequests {
    wanted: Vec<Entry>,
    inflight: HashMap<u64, String>,
    next: u64,
}

impl ImageRequests {
    pub fn update(&mut self, sources: &[String]) -> Result<(), String> {
        self.update_resources(
            &sources
                .iter()
                .map(|source| ImageSource {
                    source: source.clone(),
                    retry_key: String::new(),
                })
                .collect::<Vec<_>>(),
        )
    }

    pub fn update_resources(&mut self, sources: &[ImageSource]) -> Result<(), String> {
        let wanted: HashSet<(&str, &str)> = sources
            .iter()
            .map(|s| (s.source.as_str(), s.retry_key.as_str()))
            .collect();
        let existing: HashSet<(&str, &str)> = self
            .wanted
            .iter()
            .map(|entry| {
                (
                    entry.request.source.as_str(),
                    entry.request.retry_key.as_str(),
                )
            })
            .collect();
        let added = wanted.difference(&existing).count() as u64;
        if self
            .next
            .checked_add(added)
            .is_none_or(|next| next > i64::MAX as u64)
        {
            return Err("UI_IMAGE: request identity exhausted".into());
        }
        self.wanted.retain(|entry| {
            wanted.contains(&(
                entry.request.source.as_str(),
                entry.request.retry_key.as_str(),
            ))
        });
        let current: HashSet<u64> = self
            .wanted
            .iter()
            .map(|entry| entry.request.token)
            .collect();
        self.inflight.retain(|token, _| current.contains(token));
        let mut present: HashSet<(String, String)> = self
            .wanted
            .iter()
            .map(|entry| {
                (
                    entry.request.source.clone(),
                    entry.request.retry_key.clone(),
                )
            })
            .collect();
        for source in sources {
            if !present.insert((source.source.clone(), source.retry_key.clone())) {
                continue;
            }
            self.next += 1;
            self.wanted.push(Entry {
                request: ImageRequest {
                    source: source.source.clone(),
                    retry_key: source.retry_key.clone(),
                    token: self.next,
                },
                started: false,
            });
        }
        Ok(())
    }

    /// The adapter cancels obsolete transports when their tokens become stale.
    pub fn clear(&mut self) {
        self.wanted.clear();
        self.inflight.clear();
    }

    pub fn start(&mut self) -> Vec<ImageRequest> {
        let mut batch = Vec::new();
        for entry in &mut self.wanted {
            if self.inflight.len() >= 4 {
                break;
            }
            if entry.started {
                continue;
            }
            entry.started = true;
            self.inflight
                .insert(entry.request.token, entry.request.source.clone());
            batch.push(entry.request.clone());
        }
        batch
    }

    pub fn current(&self, token: u64) -> bool {
        self.current_source(token).is_some()
    }

    pub fn current_source(&self, token: u64) -> Option<&str> {
        let source = self.inflight.get(&token)?;
        self.wanted
            .iter()
            .any(|entry| entry.request.token == token && entry.request.source == *source)
            .then_some(source.as_str())
    }

    /// Finish success or failure exactly once; only a current result may bind pixels.
    pub fn complete(&mut self, token: u64) -> bool {
        let current = self.current(token);
        self.inflight.remove(&token);
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sources(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }

    #[test]
    fn explicit_retry_changes_identity_without_restarting_settled_work() {
        let mut queue = ImageRequests::default();
        let mut source = ImageSource {
            source: "photo".into(),
            retry_key: "0".into(),
        };
        queue
            .update_resources(&[source.clone(), source.clone()])
            .unwrap();
        let old = queue.start();
        assert_eq!(old.len(), 1);
        queue.complete(old[0].token);
        queue.update_resources(&[source.clone()]).unwrap();
        assert!(queue.start().is_empty());
        source.retry_key = "1".into();
        queue.update_resources(&[source]).unwrap();
        let next = queue.start();
        assert_eq!(next.len(), 1);
        assert_ne!(old[0].token, next[0].token);
        assert!(!queue.complete(old[0].token));
        assert!(queue.current(next[0].token));
    }

    #[test]
    fn bounded_deduplicated_and_finished_sources_do_not_restart() {
        let mut queue = ImageRequests::default();
        let wanted = sources(&["a", "a", "b", "c", "d", "e"]);
        queue.update(&wanted).unwrap();
        let first = queue.start();
        assert_eq!(first.len(), 4);
        assert_eq!(first[0].source, "a");
        queue.update(&wanted).unwrap();
        assert!(queue.start().is_empty());
        assert!(!queue.complete(999));
        assert!(queue.complete(first[0].token));
        assert!(!queue.complete(first[0].token));
        assert_eq!(queue.start()[0].source, "e");
        assert!(queue.start().is_empty());
    }

    #[test]
    fn reload_same_url_rejects_old_completion_and_releases_obsolete_slots() {
        let mut queue = ImageRequests::default();
        queue.update(&sources(&["a", "b", "c", "d"])).unwrap();
        let old = queue.start();
        queue.clear();
        queue.update(&sources(&["a"])).unwrap();
        assert!(!queue.current(old[0].token));
        assert!(!queue.complete(old[0].token));
        let new = queue.start().remove(0);
        assert_ne!(new.token, old[0].token);
        assert!(!queue.complete(old[0].token));
        assert!(queue.current(new.token));
        assert!(queue.complete(new.token));
        assert!(!queue.current(new.token));
        assert!(queue.start().is_empty());
    }

    #[test]
    fn failed_or_removed_sources_retry_only_after_leaving_the_wanted_set() {
        let mut queue = ImageRequests::default();
        queue.update(&sources(&["a", "b"])).unwrap();
        let old = queue.start();
        assert!(queue.complete(old[0].token));
        queue.update(&sources(&["b"])).unwrap();
        queue.update(&sources(&["a", "b"])).unwrap();
        let new = queue.start();
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].source, "a");
        assert_ne!(new[0].token, old[0].token);
        assert!(queue.complete(old[1].token));
    }
}
