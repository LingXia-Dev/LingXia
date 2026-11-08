use super::*;

impl ViewSession {
    /// A typed Logic state write. It needs no acknowledged session: it arrives in process,
    /// guarded by its connection, and may overtake the handshake frames.
    pub fn write_state(
        &mut self,
        write: crate::session::state::StateWrite,
    ) -> Result<crate::session::state::StateUpdate, String> {
        let mut update = self.state.write(write)?;
        update.changed &= self.ready_to_render();
        Ok(update)
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
                    ..Default::default()
                })
            }
            _ if bridge.session.is_none() || frame.get("sessionId").is_some() => {
                Err("UI_BRIDGE: frame outside acknowledged LegacyV2 connection".into())
            }
            _ => Err("UI_BRIDGE: unsupported frame".into()),
        }
    }

    /// A typed message from Logic for this acknowledged connection: a reply, stream or channel
    /// message for the runtime, or a request the overlays answer.
    pub fn receive_logic(
        &mut self,
        message: crate::session::logic::ToView,
    ) -> Result<crate::session::state::StateUpdate, String> {
        use crate::session::logic::{ToLogic, ToView};
        if self
            .bridge
            .as_ref()
            .is_none_or(|bridge| bridge.session.is_none())
        {
            return Err("UI_BRIDGE: message outside an acknowledged connection".into());
        }
        let (changed, outbound) = self.execute(|| {
            let ctx = &self.context;
            let global = ctx.global();
            let fail = |e: rong::RongJSError| crate::engine::js_error(ctx, e);
            if let ToView::Request { id, request } = &message {
                let call = request.to_js(ctx, id).map_err(fail)?;
                let answer: rong::quickjs::JSObject = global
                    .get::<_, rong::quickjs::JSFunc>("__lingxiaViewCall")
                    .and_then(|handler| handler.call(None, (call,)))
                    .map_err(fail)?;
                let changed = answer.get::<_, bool>("changed").map_err(fail)?;
                let replies: rong::quickjs::JSArray = answer.get("replies").map_err(fail)?;
                let replies = (0..replies.len().map_err(fail)?)
                    .map(|index| replies.get_value(index).and_then(ToLogic::from_js))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(fail)?;
                return Ok((changed, replies));
            }
            let frame = message.to_js(ctx).map_err(fail)?;
            let changed = global
                .get::<_, rong::quickjs::JSFunc>("__lingxiaReceiveActionResponse")
                .and_then(|receive| receive.call::<_, bool>(None, (frame,)))
                .map_err(fail)?;
            Ok((changed, Vec::new()))
        })?;
        Ok(crate::session::state::StateUpdate {
            changed: changed && self.ready_to_render(),
            outbound,
            ..Default::default()
        })
    }

    /// A handshake frame, or a Logic message written as its frame.
    #[cfg(test)]
    pub(crate) fn receive_frame(
        &mut self,
        frame: &serde_json::Value,
    ) -> Result<crate::session::state::StateUpdate, String> {
        if matches!(frame["kind"].as_str(), Some("helloAck" | "ready")) || frame["v"] != 2 {
            self.receive_bridge(frame)
        } else {
            self.receive_logic(crate::session::logic::ToView::from_frame(frame))
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

    /// What the View queued for Logic since the last call, walked out of the isolate in order.
    pub fn take_action_requests(&self) -> Result<Vec<crate::session::logic::ToLogic>, String> {
        self.execute(|| {
            let queued: rong::quickjs::JSArray = self
                .context
                .eval(Source::from_bytes("__lingxiaTakeViewEvents()"))
                .map_err(|e| e.to_string())?;
            let len = queued.len().map_err(|e| e.to_string())?;
            (0..len)
                .map(|index| {
                    queued
                        .get_value(index)
                        .and_then(crate::session::logic::ToLogic::from_js)
                        .map_err(|e| e.to_string())
                })
                .collect()
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
        self.execute(|| {
            let ctx = &self.context;
            let fail = |e: rong::RongJSError| crate::engine::js_error(ctx, e);
            let frame = crate::session::js_tree::to_js(ctx, frame).map_err(fail)?;
            ctx.global()
                .get::<_, rong::quickjs::JSFunc>("__lingxiaReceiveActionResponse")
                .and_then(|receive| receive.call::<_, bool>(None, (frame,)))
                .map_err(fail)
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
