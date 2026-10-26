use super::*;

impl ViewSession {
    /// Ignore input from frames superseded while the event was queued.
    pub fn hit_test_input(
        &self,
        generation: lifecycle::CommitTicket,
        identity: &str,
        point: [f32; 2],
        word: bool,
    ) -> Result<Option<[u32; 2]>, String> {
        self.check_input_active()?;
        if generation != self.generation || self.input_dirty || self.pending_inputs.is_some() {
            return Ok(None);
        }
        Ok(self
            .input_hits
            .as_ref()
            .and_then(|hits| hits.hit(identity, point, word)))
    }

    /// Ignore input from frames superseded while the event was queued.
    pub fn drag_hit_test_input(
        &self,
        generation: lifecycle::CommitTicket,
        identity: &str,
        token: u64,
        point: [f32; 2],
        edge: [i8; 2],
    ) -> Result<Option<[u32; 2]>, String> {
        self.check_input_active()?;
        if !self.inputs.valid(identity, token)
            || generation != self.generation
            || self.input_dirty
            || self.pending_inputs.is_some()
        {
            return Ok(None);
        }
        Ok(self
            .input_hits
            .as_ref()
            .and_then(|hits| hits.drag_hit(identity, point, edge)))
    }

    /// Ignore input from frames superseded while the event was queued.
    pub fn focus_input(
        &mut self,
        generation: lifecycle::CommitTicket,
        identity: &str,
    ) -> Result<Option<crate::InputReply>, String> {
        self.check_input_active()?;
        if generation != self.generation {
            return Ok(None);
        }
        let mut inputs = self.inputs.clone();
        if !inputs.focus(identity, &mut self.renderer)? {
            return Ok(None);
        }
        for (handle, text) in inputs.take_committed() {
            self.text_event(self.generation, handle, &text)?;
            self.input_rebuild = true;
        }
        let reply = inputs.reply(identity, self.input_rebuild);
        if self.pending_inputs.is_none() {
            self.pending_inputs = Some(self.inputs.clone());
        }
        self.inputs = inputs;
        self.input_dirty = true;
        Ok(Some(reply))
    }

    /// A focus token survives presentation; it cannot target a later input connection.
    pub fn input_state(&self, token: u64, identity: &str) -> Option<crate::InputReply> {
        self.inputs
            .valid(identity, token)
            .then(|| self.inputs.reply(identity, self.input_rebuild))
    }

    /// A focus token survives presentation; it cannot target a later input connection.
    pub fn edit_input(
        &mut self,
        token: u64,
        identity: &str,
        edits: &[crate::text::input::Edit],
        submit: bool,
    ) -> Result<Option<crate::InputReply>, String> {
        self.check_input_active()?;
        if !self.inputs.valid(identity, token) {
            return Ok(None);
        }
        let mut inputs = self.inputs.clone();
        let mut edits = edits.to_vec();
        if submit {
            edits.push(crate::text::input::Edit::FinishComposition);
        }
        let committed = inputs.edit(identity, &edits, &mut self.renderer)?;
        let mut rebuild = self.input_rebuild;
        if let (Some(text), Some(handle)) = (committed, inputs.handler(identity, false)) {
            if !self.text_event(self.generation, handle, &text)? {
                return Err("UI_INPUT: unavailable input callback".into());
            }
            rebuild = true;
        }
        // `enterKeyHint` next/previous moves to the adjacent editor instead of submitting;
        // the last editor in that direction submits.
        let moved_to = if submit {
            inputs.adjacent(identity)
        } else {
            None
        };
        if let Some(next) = &moved_to {
            inputs.focus(next, &mut self.renderer)?;
        } else if submit {
            inputs.commit(identity, &mut self.renderer)?;
        }
        for (handle, text) in inputs.take_committed() {
            self.text_event(self.generation, handle, &text)?;
            rebuild = true;
        }
        if let Some(next) = moved_to {
            let mut reply = inputs.reply(&next, rebuild);
            reply.moved_to = Some(next);
            if self.pending_inputs.is_none() {
                self.pending_inputs = Some(self.inputs.clone());
            }
            self.inputs = inputs;
            self.input_dirty = true;
            self.input_rebuild = rebuild;
            return Ok(Some(reply));
        }
        if submit && let Some(handle) = inputs.handler(identity, true) {
            let text = inputs.reply(identity, false).state.text;
            if !self.text_event(self.generation, handle, &text)? {
                return Err("UI_INPUT: unavailable submit callback".into());
            }
            rebuild = true;
        }
        let reply = inputs.reply(identity, rebuild);
        if self.pending_inputs.is_none() {
            self.pending_inputs = Some(self.inputs.clone());
        }
        self.inputs = inputs;
        self.input_dirty = true;
        self.input_rebuild = rebuild;
        Ok(Some(reply))
    }

    pub fn blur_input(&mut self, token: u64, identity: &str) -> Result<bool, String> {
        self.check_input_active()?;
        if !self.inputs.valid(identity, token) {
            return Ok(false);
        }
        let mut inputs = self.inputs.clone();
        inputs.blur(&mut self.renderer)?;
        for (handle, text) in inputs.take_committed() {
            self.text_event(self.generation, handle, &text)?;
            self.input_rebuild = true;
        }
        if self.pending_inputs.is_none() {
            self.pending_inputs = Some(self.inputs.clone());
        }
        self.inputs = inputs;
        self.input_dirty = true;
        Ok(true)
    }

    pub(super) fn check_input_active(&self) -> Result<(), String> {
        if self.poisoned.get() {
            Err("UI_TIMEOUT: discard the interrupted page before reusing it".into())
        } else {
            Ok(())
        }
    }
}
