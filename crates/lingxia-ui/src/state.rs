use serde_json::{Value, json};

#[derive(Default)]
pub struct StateReplica {
    data: Option<Value>,
    revision: u64,
    recovery: Option<String>,
    next_request: u64,
}

#[derive(Default, Debug)]
pub struct StateUpdate {
    pub changed: bool,
    /// Send these before attempting a render: flush completion depends on state application.
    pub outbound: Vec<Value>,
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

    pub fn receive(&mut self, frame: &Value) -> Result<StateUpdate, String> {
        if frame["v"] != 2 {
            return Err("UI_STATE: expected LegacyV2".into());
        }
        match frame["kind"].as_str() {
            Some("state.snapshot") => self.snapshot(frame),
            Some("res")
                if self
                    .recovery
                    .as_deref()
                    .is_some_and(|id| frame["id"].as_str() == Some(id)) =>
            {
                if frame["ok"] == true {
                    self.snapshot(&frame["result"])
                } else {
                    Err("UI_STATE: snapshot recovery failed".into())
                }
            }
            Some("state.patch") => {
                if self.recovery.is_some() {
                    return Ok(StateUpdate::default());
                }
                let base = revision(&frame["baseRev"])?;
                let next = revision(&frame["rev"])?;
                if self.data.is_none() || base != self.revision || next <= base {
                    return Ok(self.recover(frame));
                }
                let mut candidate = self.data.clone().unwrap();
                if apply_patch(&mut candidate, &frame["ops"]).is_err() {
                    return Ok(self.recover(frame));
                }
                self.data = Some(candidate);
                self.revision = next;
                let mut update = StateUpdate {
                    changed: true,
                    outbound: Vec::new(),
                };
                if frame["ack"] == true {
                    let mut ack = json!({"v":2,"kind":"state.ack","rev":next});
                    if let Some(scope) = frame.get("scope") {
                        ack["scope"] = scope.clone();
                    }
                    update.outbound.push(ack);
                }
                Ok(update)
            }
            _ => Err("UI_STATE: unsupported frame".into()),
        }
    }

    fn snapshot(&mut self, frame: &Value) -> Result<StateUpdate, String> {
        let revision = revision(&frame["rev"])?;
        if !frame["state"].is_object() {
            return Err("UI_STATE: expected state object".into());
        }
        self.data = Some(frame["state"].clone());
        self.revision = revision;
        self.recovery = None;
        Ok(StateUpdate {
            changed: true,
            outbound: Vec::new(),
        })
    }

    fn recover(&mut self, frame: &Value) -> StateUpdate {
        self.next_request += 1;
        let id = format!("ui-state-{}", self.next_request);
        self.recovery = Some(id.clone());
        let mut request = json!({"v":2,"kind":"req","id":id,"method":"state.getSnapshot","cap":"state","params":{}});
        if let Some(scope) = frame.get("scope") {
            request["params"]["scope"] = scope.clone();
        }
        StateUpdate {
            changed: false,
            outbound: vec![request],
        }
    }
}

fn revision(value: &Value) -> Result<u64, String> {
    value
        .as_u64()
        .filter(|value| *value <= 9_007_199_254_740_991)
        .ok_or_else(|| "UI_STATE: invalid revision".into())
}

fn apply_patch(root: &mut Value, ops: &Value) -> Result<(), String> {
    for op in ops.as_array().ok_or("UI_PATCH: expected operations")? {
        let kind = op["op"].as_str().ok_or("UI_PATCH: missing operation")?;
        if !matches!(kind, "add" | "replace" | "remove") {
            return Err("UI_PATCH: unsupported operation".into());
        }
        let path = op["path"].as_str().ok_or("UI_PATCH: missing pointer")?;
        let value = if kind == "remove" {
            Value::Null
        } else {
            op.get("value").ok_or("UI_PATCH: missing value")?.clone()
        };
        if path.is_empty() {
            *root = if kind == "remove" {
                json!({})
            } else if value.is_object() {
                value
            } else {
                return Err("UI_PATCH: root must be object".into());
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
                    if kind != "remove" && !object.contains_key(segment) {
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
            Value::Object(object) => {
                if kind == "remove" {
                    object.remove(key);
                } else {
                    object.insert(key.clone(), value);
                }
            }
            Value::Array(array) => {
                let index = if key == "-" {
                    array.len()
                } else {
                    array_index(key)?
                };
                match kind {
                    "add" => array.insert(index.min(array.len()), value),
                    "remove" => {
                        if index < array.len() {
                            array.remove(index);
                        }
                    }
                    "replace" => {
                        // Native state patches use dense arrays; cap sparse expansion before allocating.
                        if index > 65535 {
                            return Err("UI_PATCH: array index too large".into());
                        }
                        if index >= array.len() {
                            array.resize(index + 1, Value::Null);
                        }
                        array[index] = value;
                    }
                    _ => unreachable!(),
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

    #[test]
    fn patch_ack_follows_atomic_application_and_pointer_rules() {
        let mut state = StateReplica::default();
        assert!(state.data().is_none());
        state
            .receive(
                &json!({"v":2,"kind":"state.snapshot","rev":7,"state":{"items":[1,2],"a/b":0}}),
            )
            .unwrap();
        let result=state.receive(&json!({"v":2,"kind":"state.patch","baseRev":7,"rev":8,"ack":true,"scope":"page","ops":[{"op":"add","path":"/items/1","value":3},{"op":"replace","path":"/a~1b","value":4},{"op":"replace","path":"/nested/name","value":"hello"}]})).unwrap();
        assert_eq!(
            state.data(),
            Some(&json!({"items":[1,3,2],"a/b":4,"nested":{"name":"hello"}}))
        );
        assert_eq!(
            result.outbound,
            vec![json!({"v":2,"kind":"state.ack","rev":8,"scope":"page"})]
        );
    }

    #[test]
    fn failed_patch_recovers_once_and_never_acknowledges_queued_revisions() {
        let mut state = StateReplica::static_page();
        let result=state.receive(&json!({"v":2,"kind":"state.patch","baseRev":0,"rev":1,"ack":true,"ops":[{"op":"add","path":"/value","value":3},{"op":"remove","path":"/missing/child"}]})).unwrap();
        assert_eq!(state.data(), Some(&json!({})));
        assert_eq!(state.revision(), 0);
        assert_eq!(result.outbound[0]["method"], "state.getSnapshot");
        let ignored = state
            .receive(&json!({"v":2,"kind":"state.patch","baseRev":1,"rev":2,"ack":true,"ops":[]}))
            .unwrap();
        assert!(ignored.outbound.is_empty());
        state.receive(&json!({"v":2,"kind":"res","id":result.outbound[0]["id"],"ok":true,"result":{"rev":2,"state":{"value":9}}})).unwrap();
        assert_eq!(state.data(), Some(&json!({"value":9})));
        assert_eq!(state.revision(), 2);
    }
}
