use serde_json::{Value, json};

/// One JSON Patch operation on page data; `path` is a JSON Pointer.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum PatchOp {
    Add { path: String, value: Value },
    Replace { path: String, value: Value },
    Remove { path: String },
}

/// A Logic state write, delivered in process and in write order.
#[derive(Debug)]
pub enum StateWrite {
    /// The whole page data, an object.
    Snapshot { rev: u64, state: Value },
    /// `ack`: Logic waits to hear that this revision reached a frame.
    Patch {
        rev: u64,
        ops: Vec<PatchOp>,
        ack: bool,
    },
}

/// What the View tells Logic about its state writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateSignal {
    /// Revision `rev` is in a frame committed for display, or will never be.
    Acked(u64),
    /// A patch did not apply; send a snapshot. Writes before it are dropped.
    Resync,
}

#[derive(Default)]
pub struct StateReplica {
    data: Option<Value>,
    revision: u64,
    resyncing: bool,
    /// Ops applied since the View isolate's copy was last synced at `journal_from`; `None`
    /// when that copy needs the whole tree.
    journal: Vec<PatchOp>,
    journal_from: Option<u64>,
}

/// Past this many ops the isolate takes the whole tree instead of a replay.
const JOURNAL_LIMIT: usize = 1024;

#[derive(Default, Debug)]
pub struct StateUpdate {
    pub changed: bool,
    /// Messages for Logic, sent before attempting a render.
    pub outbound: Vec<crate::session::logic::ToLogic>,
    /// The revision to acknowledge once a frame shows it.
    pub ack: Option<u64>,
    pub resync: bool,
}

impl StateReplica {
    pub fn static_page() -> Self {
        Self {
            data: Some(json!({})),
            ..Self::default()
        }
    }
    pub fn data(&self) -> Option<&Value> {
        self.data.as_ref()
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn write(&mut self, write: StateWrite) -> Result<StateUpdate, String> {
        match write {
            StateWrite::Snapshot { rev, state } => {
                if !state.is_object() {
                    return Err("UI_STATE: expected state object".into());
                }
                self.data = Some(state);
                self.revision = rev;
                self.resyncing = false;
                self.forget_journal();
                Ok(StateUpdate {
                    changed: true,
                    ..StateUpdate::default()
                })
            }
            StateWrite::Patch { rev, ops, ack } => {
                if self.resyncing {
                    return Ok(StateUpdate::default());
                }
                let Some(data) = self.data.as_mut().filter(|_| rev > self.revision) else {
                    return Ok(self.resync());
                };
                if self.journal_from.is_some() {
                    if self.journal.len() + ops.len() > JOURNAL_LIMIT {
                        self.journal_from = None;
                        self.journal.clear();
                    } else {
                        self.journal.extend(ops.iter().cloned());
                    }
                }
                // In place: a failed op may leave earlier ones applied until the snapshot
                // it asks for replaces the whole tree.
                if apply_patch(data, ops).is_err() {
                    return Ok(self.resync());
                }
                self.revision = rev;
                Ok(StateUpdate {
                    changed: true,
                    ack: ack.then_some(rev),
                    ..StateUpdate::default()
                })
            }
        }
    }

    /// The isolate's synced revision and the ops since, when a replay can bring it current.
    pub fn journal(&self) -> Option<(u64, &[PatchOp])> {
        self.journal_from
            .map(|from| (from, self.journal.as_slice()))
    }

    /// The isolate's copy now matches the current revision.
    pub fn mark_synced(&mut self) {
        self.journal.clear();
        self.journal_from = self.data.is_some().then_some(self.revision);
    }

    fn forget_journal(&mut self) {
        self.journal.clear();
        self.journal_from = None;
    }

    fn resync(&mut self) -> StateUpdate {
        self.resyncing = true;
        self.forget_journal();
        StateUpdate {
            resync: true,
            ..StateUpdate::default()
        }
    }
}

fn apply_patch(root: &mut Value, ops: Vec<PatchOp>) -> Result<(), String> {
    for op in ops {
        let (remove, path, value) = match op {
            PatchOp::Add { path, value } => (false, path, Some((false, value))),
            PatchOp::Replace { path, value } => (false, path, Some((true, value))),
            PatchOp::Remove { path } => (true, path, None),
        };
        if path.is_empty() {
            *root = match value {
                None => json!({}),
                Some((_, value)) if value.is_object() => value,
                Some(_) => return Err("UI_PATCH: root must be object".into()),
            };
            continue;
        }
        if !path.starts_with('/') {
            return Err("UI_PATCH: invalid pointer".into());
        }
        let segments: Vec<String> = path[1..]
            .split('/')
            .map(|s| s.replace("~1", "/").replace("~0", "~"))
            .collect();
        if segments.len() > 128 {
            return Err("UI_PATCH: excessive depth".into());
        }
        let mut current = &mut *root;
        for (index, segment) in segments[..segments.len() - 1].iter().enumerate() {
            current = match current {
                Value::Object(object) => {
                    if !remove && !object.contains_key(segment) {
                        let next = &segments[index + 1];
                        object.insert(
                            segment.clone(),
                            if !next.is_empty() && next.bytes().all(|c| c.is_ascii_digit()) {
                                json!([])
                            } else {
                                json!({})
                            },
                        );
                    }
                    object
                        .get_mut(segment)
                        .ok_or("UI_PATCH: missing container")?
                }
                Value::Array(array) => array
                    .get_mut(array_index(segment)?)
                    .ok_or("UI_PATCH: missing array item")?,
                _ => return Err("UI_PATCH: invalid container".into()),
            };
        }
        let key = segments.last().unwrap();
        match current {
            Value::Object(object) => match value {
                None => {
                    object.remove(key);
                }
                Some((_, value)) => {
                    object.insert(key.clone(), value);
                }
            },
            Value::Array(array) => {
                let index = if key == "-" {
                    array.len()
                } else {
                    array_index(key)?
                };
                match value {
                    None => {
                        if index < array.len() {
                            array.remove(index);
                        }
                    }
                    Some((false, value)) => array.insert(index.min(array.len()), value),
                    Some((true, value)) => {
                        // Native state patches use dense arrays; cap sparse expansion before allocating.
                        if index > 65535 {
                            return Err("UI_PATCH: array index too large".into());
                        }
                        if index >= array.len() {
                            array.resize(index + 1, Value::Null);
                        }
                        array[index] = value;
                    }
                }
            }
            _ => return Err("UI_PATCH: invalid container".into()),
        }
    }
    Ok(())
}

fn array_index(value: &str) -> Result<usize, String> {
    if value.is_empty() || !value.bytes().all(|c| c.is_ascii_digit()) {
        return Err("UI_PATCH: invalid array index".into());
    }
    value
        .parse()
        .map_err(|_| "UI_PATCH: invalid array index".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replace(path: &str, value: Value) -> PatchOp {
        PatchOp::Replace {
            path: path.into(),
            value,
        }
    }

    #[test]
    fn malformed_patch_operations_are_ui_patch() {
        let mut root = json!({"n": 1});
        for ops in [
            vec![PatchOp::Add {
                path: "a".into(),
                value: json!(1),
            }],
            vec![replace("", json!(3))],
            vec![replace("/n/x", json!(1))],
        ] {
            assert!(
                apply_patch(&mut root, ops)
                    .unwrap_err()
                    .starts_with("UI_PATCH:")
            );
        }
    }

    #[test]
    fn patches_apply_in_place_and_acknowledge_their_revision() {
        let mut state = StateReplica::default();
        assert!(state.data().is_none());
        state
            .write(StateWrite::Snapshot {
                rev: 7,
                state: json!({"items":[1,2],"a/b":0}),
            })
            .unwrap();
        let result = state
            .write(StateWrite::Patch {
                rev: 8,
                ack: true,
                ops: vec![
                    PatchOp::Add {
                        path: "/items/1".into(),
                        value: json!(3),
                    },
                    replace("/a~1b", json!(4)),
                    replace("/nested/name", json!("hello")),
                ],
            })
            .unwrap();
        assert_eq!(
            state.data(),
            Some(&json!({"items":[1,3,2],"a/b":4,"nested":{"name":"hello"}}))
        );
        assert_eq!(
            (result.changed, result.ack, result.resync),
            (true, Some(8), false)
        );
    }

    #[test]
    fn a_failed_patch_resyncs_once_and_never_acknowledges_dropped_revisions() {
        let mut state = StateReplica::static_page();
        let result = state
            .write(StateWrite::Patch {
                rev: 1,
                ack: true,
                ops: vec![PatchOp::Remove {
                    path: "/missing/child".into(),
                }],
            })
            .unwrap();
        assert_eq!(state.revision(), 0);
        assert!(result.resync && result.ack.is_none() && !result.changed);
        let ignored = state
            .write(StateWrite::Patch {
                rev: 2,
                ack: true,
                ops: vec![],
            })
            .unwrap();
        assert!(!ignored.resync && ignored.ack.is_none());
        state
            .write(StateWrite::Snapshot {
                rev: 2,
                state: json!({"value":9}),
            })
            .unwrap();
        assert_eq!(state.data(), Some(&json!({"value":9})));
        let result = state
            .write(StateWrite::Patch {
                rev: 3,
                ack: true,
                ops: vec![replace("/value", json!(10))],
            })
            .unwrap();
        assert_eq!(result.ack, Some(3));
    }

    #[test]
    fn a_patch_before_the_first_snapshot_or_out_of_order_resyncs() {
        let mut state = StateReplica::default();
        let early = StateWrite::Patch {
            rev: 1,
            ack: false,
            ops: vec![],
        };
        assert!(state.write(early).unwrap().resync);
        state
            .write(StateWrite::Snapshot {
                rev: 4,
                state: json!({}),
            })
            .unwrap();
        let stale = StateWrite::Patch {
            rev: 4,
            ack: false,
            ops: vec![],
        };
        assert!(state.write(stale).unwrap().resync);
    }

    #[test]
    fn the_journal_holds_ops_since_the_isolate_synced() {
        let op = |n: i64| PatchOp::Replace {
            path: "/n".into(),
            value: json!(n),
        };
        let patch = |rev, n| StateWrite::Patch {
            rev,
            ops: vec![op(n)],
            ack: false,
        };
        let mut replica = StateReplica::default();
        replica
            .write(StateWrite::Snapshot {
                rev: 1,
                state: json!({"n": 0}),
            })
            .unwrap();
        assert!(
            replica.journal().is_none(),
            "a snapshot needs the whole tree"
        );
        replica.mark_synced();
        replica.write(patch(2, 1)).unwrap();
        replica.write(patch(3, 2)).unwrap();
        assert_eq!(replica.journal(), Some((1, &[op(1), op(2)][..])));
        replica.mark_synced();
        assert_eq!(replica.journal(), Some((3, &[][..])));
        replica.write(patch(3, 9)).unwrap();
        assert!(replica.journal().is_none(), "a resync forgets the journal");
        replica
            .write(StateWrite::Snapshot {
                rev: 4,
                state: json!({"n": 0}),
            })
            .unwrap();
        replica.mark_synced();
        for rev in 0..=JOURNAL_LIMIT as u64 {
            replica.write(patch(5 + rev, 1)).unwrap();
        }
        assert!(
            replica.journal().is_none(),
            "a long replay takes the whole tree"
        );
    }
}
