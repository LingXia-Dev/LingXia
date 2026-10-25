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
            _ => Err("UI_BRIDGE: unsupported frame".into()),
        }
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

// Page actions resolve through microtasks; an unbounded continuation would hang the worker.
fn require_bounded_jobs() -> Result<(), String> {
    if crate::engine::supports(crate::engine::Feature::BoundedJobs) {
        Ok(())
    } else {
        Err("UI_ACTION: this engine cannot bound View microtasks".into())
    }
}
