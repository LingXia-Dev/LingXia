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
        if !self.presented_layout_commit(generation)
            || self.input_dirty
            || self.pending_inputs.is_some()
        {
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
            || !self.presented_layout_commit(generation)
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
        if !self.presented_layout_commit(generation) {
            return Ok(None);
        }
        self.focus_now(identity)
    }

    fn focus_now(&mut self, identity: &str) -> Result<Option<crate::InputReply>, String> {
        let mut inputs = self.inputs.clone();
        if !inputs.focus(identity, &mut self.renderer)? {
            return Ok(None);
        }
        inputs.hold_keyboard(false);
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

    /// A tap or activation of a date or time editor opens the View's picker instead of an input method.
    pub fn open_picker(
        &mut self,
        generation: lifecycle::CommitTicket,
        identity: &str,
    ) -> Result<bool, String> {
        self.check_input_active()?;
        if !self.presented_layout_commit(generation) {
            return Ok(false);
        }
        self.open_picker_now(identity, None)
    }

    /// Open the View's picker on editor `identity` of the last layout, as a tap would; `today`
    /// (ISO) replaces the clock's date, for renders that must not change from day to day.
    pub fn open_picker_now(&mut self, identity: &str, today: Option<&str>) -> Result<bool, String> {
        self.check_input_active()?;
        match self.inputs.picker_request(identity) {
            Some(mut request) => {
                if let Some(today) = today {
                    request["today"] = today.into();
                }
                request["anchorWidth"] = self.anchor_width(identity).into();
                self.open_view_picker(&request)
            }
            None => Ok(false),
        }
    }

    /// A tap or activation of a `<select>` opens the View's picker with its options.
    pub fn open_select(
        &mut self,
        generation: lifecycle::CommitTicket,
        identity: &str,
    ) -> Result<bool, String> {
        self.check_input_active()?;
        if !self.presented_layout_commit(generation) {
            return Ok(false);
        }
        self.open_select_now(identity)
    }

    /// Open the View's picker on `<select>` `identity` of the last layout, as a tap would.
    pub fn open_select_now(&mut self, identity: &str) -> Result<bool, String> {
        let Some(request) = self.retained.as_ref().and_then(|retained| {
            let node = retained
                .nodes
                .iter()
                .find(|node| node.identity == identity)?;
            let select = node.select.as_ref().filter(|s| s.on_select.is_some())?;
            Some(serde_json::json!({
                "identity": identity, "kind": "select", "options": select.options,
                "selected": select.selected, "label": node.accessibility_label,
                "anchorWidth": node.bounds[2],
            }))
        }) else {
            return Ok(false);
        };
        self.open_view_picker(&request)
    }

    /// The laid-out width of the control a popover opens from, logical pixels.
    fn anchor_width(&self, identity: &str) -> f32 {
        self.retained
            .as_ref()
            .and_then(|retained| retained.nodes.iter().find(|node| node.identity == identity))
            .map_or(0.0, |node| node.bounds[2])
    }

    /// A key while the picker is open; a choice it made applies at once.
    pub fn picker_key(&mut self, key: &str) -> Result<bool, String> {
        self.check_input_active()?;
        let used = self.view_picker_key(key)?;
        Ok(self.apply_picker_results()? || used)
    }

    /// Choices of the View's picker go through the editor as a platform picker's would: focus,
    /// the chosen value replacing its text with input and submit callbacks, then blur.
    pub fn apply_picker_results(&mut self) -> Result<bool, String> {
        use crate::text::input::Edit;
        let mut changed = false;
        for crate::session::bridge::PickerChoice {
            identity,
            value,
            index,
        } in self.take_view_picker_results()?
        {
            changed = true;
            let Some(value) = value else { continue };
            if index.is_some() {
                // A select's choice, found again by value in the View rendered since it opened.
                let current = self.retained.as_ref().and_then(|retained| {
                    let node = retained
                        .nodes
                        .iter()
                        .find(|node| node.identity == identity)?;
                    node.select
                        .as_ref()?
                        .options
                        .iter()
                        .position(|o| o.value == value)
                });
                if let Some(current) = current {
                    self.select_option(self.generation, &identity, current)?;
                }
                continue;
            }
            let Some(focused) = self.focus_now(&identity)? else {
                continue;
            };
            let all = focused.state.text.encode_utf16().count() as u32;
            let edits = [
                Edit::Selection {
                    anchor: 0,
                    focus: all,
                },
                Edit::Replace {
                    text: value,
                    cursor: 1,
                    composing: false,
                },
            ];
            self.edit_input(focused.token, &identity, &edits, true)?;
            self.blur_input(focused.token, &identity)?;
        }
        Ok(changed)
    }

    /// Submit an editor as its input method action would, focusing it first.
    pub fn submit_editor(
        &mut self,
        generation: lifecycle::CommitTicket,
        identity: &str,
    ) -> Result<bool, String> {
        let Some(focused) = self.focus_input(generation, identity)? else {
            return Ok(false);
        };
        Ok(self
            .edit_input(focused.token, identity, &[], true)?
            .is_some())
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
        inputs.hold_keyboard(true);
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
