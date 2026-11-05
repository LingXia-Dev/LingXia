use super::*;

impl ViewSession {
    pub fn receive_state(
        &mut self,
        frame: &serde_json::Value,
    ) -> Result<crate::session::state::StateUpdate, String> {
        self.state.receive(frame)
    }

    /// Called by native after attaching this context to its owning page endpoint.
    pub fn begin_bridge(&mut self, nonce: &str) -> Result<serde_json::Value, String> {
        if nonce.is_empty() || nonce.len() > 256 {
            return Err("UI_BRIDGE: invalid nonce".into());
        }
        self.reset()?;
        self.state = crate::session::state::StateReplica::default();
        self.bridge = Some(ViewBridge {
            nonce: nonce.into(),
            session: None,
            ready: false,
        });
        Ok(
            serde_json::json!({"v":2,"kind":"hello","nonce":nonce,"role":"view","protocolsSupported":[2]}),
        )
    }

    pub fn ready_to_render(&self) -> bool {
        self.state.data().is_some() && self.bridge.as_ref().is_none_or(|bridge| bridge.ready)
    }

    pub fn receive_bridge(
        &mut self,
        frame: &serde_json::Value,
    ) -> Result<crate::session::state::StateUpdate, String> {
        if frame["v"] != 2 {
            return Err("UI_BRIDGE: expected LegacyV2".into());
        }
        let bridge = self
            .bridge
            .as_ref()
            .ok_or("UI_BRIDGE: context is not connected")?;
        let kind = frame["kind"].as_str().ok_or("UI_BRIDGE: missing kind")?;
        match kind {
            "helloAck" => {
                let session = frame["sessionId"]
                    .as_str()
                    .filter(|s| !s.is_empty() && s.len() <= 256)
                    .ok_or("UI_BRIDGE: invalid session")?;
                if bridge.session.is_some()
                    || frame["nonce"] != bridge.nonce
                    || frame["protocol"] != 2
                {
                    return Err("UI_BRIDGE: invalid hello acknowledgement".into());
                }
                self.bridge.as_mut().unwrap().session = Some(session.into());
                Ok(crate::session::state::StateUpdate::default())
            }
            "ready" => {
                if bridge
                    .session
                    .as_deref()
                    .is_none_or(|id| frame["sessionId"] != id)
                    || bridge.ready
                {
                    return Err("UI_BRIDGE: invalid ready".into());
                }
                require_bounded_jobs()?;
                self.eval("__lingxiaConfigureActions(globalThis.__lingxiaActionCatalog)")?;
                self.bridge.as_mut().unwrap().ready = true;
                Ok(crate::session::state::StateUpdate {
                    changed: self.ready_to_render(),
                    outbound: Vec::new(),
                })
            }
            _ if bridge.session.is_none() || frame.get("sessionId").is_some() => {
                Err("UI_BRIDGE: frame outside acknowledged LegacyV2 connection".into())
            }
            "state.snapshot" | "state.patch" => {
                let mut update = self.receive_state(frame)?;
                update.changed &= self.ready_to_render();
                Ok(update)
            }
            "res"
                if frame["id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with("ui-state-")) =>
            {
                let mut update = self.receive_state(frame)?;
                update.changed &= self.ready_to_render();
                Ok(update)
            }
            "res" | "event" | "ch.ack" | "ch.data" | "ch.close" => {
                let changed = self.receive_action_response(frame)?;
                let outbound = self
                    .take_action_requests()?
                    .iter()
                    .map(|frame| serde_json::from_str(frame).map_err(|e| e.to_string()))
                    .collect::<Result<_, _>>()?;
                Ok(crate::session::state::StateUpdate {
                    changed: changed && self.ready_to_render(),
                    outbound,
                })
            }
            // A Logic call into the View (`call_view`): the overlays answer `ui.*`; any other
            // method is refused at once, so Logic never waits on a GPU page.
            "req" => {
                let json = serde_json::to_string(&frame.to_string()).map_err(|e| e.to_string())?;
                let answer: String = self.execute(|| {
                    self.context
                        .eval(Source::from_bytes(
                            format!("__lingxiaViewRequest(JSON.parse({json}))").as_bytes(),
                        ))
                        .map_err(|e| e.to_string())
                })?;
                let answer: serde_json::Value =
                    serde_json::from_str(&answer).map_err(|e| e.to_string())?;
                let outbound = answer["replies"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|reply| serde_json::from_str(reply.as_str()?).ok())
                    .collect();
                Ok(crate::session::state::StateUpdate {
                    changed: answer["changed"] == true && self.ready_to_render(),
                    outbound,
                })
            }
            _ => Err("UI_BRIDGE: unsupported frame".into()),
        }
    }

    /// Toast deadlines the overlays asked for since the last call: `(token, milliseconds)`.
    pub fn take_overlay_timers(&self) -> Result<Vec<(u64, u64)>, String> {
        let json: String = self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    "typeof __lingxiaTakeOverlayTimers === 'function' ? __lingxiaTakeOverlayTimers() : '[]'",
                ))
                .map_err(|e| e.to_string())
        })?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    /// `measureFrames` windows the View asked for since the last call: `(token, milliseconds)`.
    pub(crate) fn take_frame_measures(&self) -> Result<Vec<(u64, u64)>, String> {
        let json: String = self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    "typeof __lingxiaTakeFrameMeasures === 'function' ? __lingxiaTakeFrameMeasures() : '[]'",
                ))
                .map_err(|e| e.to_string())
        })?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    /// Resolve the `measureFrames` window `token` with `stats`.
    pub(crate) fn frame_measured(
        &self,
        token: u64,
        stats: &serde_json::Value,
    ) -> Result<bool, String> {
        let json = serde_json::to_string(&stats.to_string()).map_err(|e| e.to_string())?;
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!("typeof __lingxiaFrameMeasured === 'function' && __lingxiaFrameMeasured({token}, JSON.parse({json}))").as_bytes(),
                ))
                .map_err(|e| e.to_string())
        })
    }

    /// Escape: the innermost overlay goes (a sheet cancels, a modal cancels, a toast hides).
    pub fn escape(&self) -> Result<bool, String> {
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    "typeof __lingxiaEscape === 'function' && __lingxiaEscape()",
                ))
                .map_err(|e| e.to_string())
        })
    }

    /// Open the View's own picker for a date or time editor: `{identity, kind, value, min, max, step}`.
    pub fn open_view_picker(&self, request: &serde_json::Value) -> Result<bool, String> {
        let request = serde_json::to_string(&request.to_string()).map_err(|e| e.to_string())?;
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!("typeof __lingxiaOpenPicker === 'function' && __lingxiaOpenPicker(JSON.parse({request}))").as_bytes(),
                ))
                .map_err(|e| e.to_string())
        })
    }

    /// A key for the open picker (`ArrowUp`, `PageDown`, `Enter`, `Escape`, a digit); true when used.
    pub fn view_picker_key(&self, key: &str) -> Result<bool, String> {
        let key = serde_json::to_string(key).map_err(|e| e.to_string())?;
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!(
                        "typeof __lingxiaPickerKey === 'function' && __lingxiaPickerKey({key})"
                    )
                    .as_bytes(),
                ))
                .map_err(|e| e.to_string())
        })
    }

    /// Choices the picker made since the last call.
    pub(crate) fn take_view_picker_results(&self) -> Result<Vec<PickerChoice>, String> {
        let json: String = self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    "typeof __lingxiaTakePickerResults === 'function' ? __lingxiaTakePickerResults() : '[]'",
                ))
                .map_err(|e| e.to_string())
        })?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    /// A toast deadline passed; true when it hid the toast still showing.
    pub fn overlay_timer(&self, token: u64) -> Result<bool, String> {
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!("__lingxiaOverlayTimer({token})").as_bytes(),
                ))
                .map_err(|e| e.to_string())
        })
    }

    pub fn configure_actions(&self, metadata: &serde_json::Value) -> Result<(), String> {
        require_bounded_jobs()?;
        let json = serde_json::to_string(&metadata.to_string()).map_err(|e| e.to_string())?;
        self.eval(&format!("__lingxiaConfigureActions(JSON.parse({json}))"))
    }

    pub fn request_leave(&self, reason: &str) -> Result<(), String> {
        if !matches!(reason, "back" | "home") {
            return Err("UI_LEAVE: invalid reason".into());
        }
        let encoded = serde_json::to_string(reason).map_err(|e| e.to_string())?;
        self.eval(&format!("__lingxiaRequestLeave({encoded})"))
    }

    pub fn has_local_updates(&self) -> Result<bool, String> {
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    "typeof __lingxiaHasLocalUpdates === 'function' && __lingxiaHasLocalUpdates()",
                ))
                .map_err(|e| e.to_string())
        })
    }

    pub fn take_action_requests(&self) -> Result<Vec<String>, String> {
        self.execute(|| {
            self.context
                .eval(Source::from_bytes("__lingxiaTakeActionRequests()"))
                .map_err(|e| e.to_string())
        })
    }

    pub fn take_action_errors(&self) -> Result<Vec<String>, String> {
        self.execute(|| {
            self.context
                .eval(Source::from_bytes("__lingxiaTakeActionErrors()"))
                .map_err(|e| e.to_string())
        })
    }

    /// Call page action `name` with `payload` as the View would; its outcome arrives through
    /// [`Self::take_automation_results`] once Logic answers.
    pub fn automation_action(
        &self,
        call: u64,
        name: &str,
        payload: Option<&serde_json::Value>,
    ) -> Result<bool, String> {
        let name = serde_json::to_string(name).map_err(|e| e.to_string())?;
        let payload = match payload {
            Some(payload) => {
                let json =
                    serde_json::to_string(&payload.to_string()).map_err(|e| e.to_string())?;
                format!("JSON.parse({json})")
            }
            None => "undefined".into(),
        };
        self.eval(&format!(
            "__lingxiaAutomationAction({call}, {name}, {payload})"
        ))?;
        Ok(true)
    }

    /// Settled automation action calls: `(call, value or "CODE: message")`.
    pub fn take_automation_results(&self) -> Result<Vec<AutomationOutcome>, String> {
        let json: String = self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    "typeof __lingxiaTakeAutomationResults === 'function' ? __lingxiaTakeAutomationResults() : '[]'",
                ))
                .map_err(|e| e.to_string())
        })?;
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(&json).map_err(|e| e.to_string())?;
        Ok(entries
            .into_iter()
            .filter_map(|entry| {
                let call = entry["call"].as_u64()?;
                Some(match entry.get("error") {
                    Some(error) => (
                        call,
                        Err(format!(
                            "{}: {}",
                            error["code"].as_str().unwrap_or("UI_ACTION"),
                            error["message"].as_str().unwrap_or_default()
                        )),
                    ),
                    None => (call, Ok(entry["value"].clone())),
                })
            })
            .collect())
    }

    /// The page's current `view.data`, as Logic last set it.
    pub fn automation_data(&self) -> serde_json::Value {
        self.state
            .data()
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    }

    pub fn receive_action_response(&self, frame: &serde_json::Value) -> Result<bool, String> {
        let json = serde_json::to_string(&frame.to_string()).map_err(|e| e.to_string())?;
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!("__lingxiaReceiveActionResponse(JSON.parse({json}))").as_bytes(),
                ))
                .map_err(|e| e.to_string())
        })
    }
}

/// A settled automation call: its id and the action's value or `CODE: message`.
pub type AutomationOutcome = (u64, Result<serde_json::Value, String>);

// Page actions resolve through microtasks; an unbounded continuation would hang the worker.
fn require_bounded_jobs() -> Result<(), String> {
    if crate::engine::supports(crate::engine::Feature::BoundedJobs) {
        Ok(())
    } else {
        Err("UI_ACTION: this engine cannot bound View microtasks".into())
    }
}

/// One choice of the View's picker: the control's native identity and its value, `None` for a
/// cancel; a select's choice carries its option index.
#[derive(serde::Deserialize)]
pub(crate) struct PickerChoice {
    pub identity: String,
    pub value: Option<String>,
    pub index: Option<usize>,
}
