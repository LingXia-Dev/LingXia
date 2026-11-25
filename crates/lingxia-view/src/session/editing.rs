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
            log::info!(
                target: "lingxia_view",
                "editor hit test {identity} refused: frame current {}, input unsettled {}, pending {}",
                self.presented_layout_commit(generation),
                self.input_dirty,
                self.pending_inputs.is_some()
            );
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
        if self.input_commit(generation, identity, true).is_none() {
            return Ok(None);
        }
        self.focus_now(identity)
    }

    /// The next focus of `identity` arrives by keyboard: a single-line editor then selects its
    /// text, a multiline one puts the caret at the end.
    pub(crate) fn enter_editor_by_keyboard(&mut self, identity: String, select_all: bool) {
        self.keyboard_entry = Some((identity, select_all));
    }

    fn focus_now(&mut self, identity: &str) -> Result<Option<crate::InputReply>, String> {
        let entry = self
            .keyboard_entry
            .take()
            .filter(|(entry, _)| entry == identity);
        let mut inputs = self.inputs.clone();
        let left = inputs.focused().map(str::to_owned);
        if !inputs.focus(identity, &mut self.renderer)? {
            return Ok(None);
        }
        if let Some((_, select_all)) = entry
            && left.as_deref() != Some(identity)
        {
            inputs.place_entry_selection(identity, select_all, &mut self.renderer)?;
        }
        inputs.hold_keyboard(false);
        // The editor focus moved from hears `onBlur` before this one hears `onFocus`.
        if left.as_deref() != Some(identity) {
            if let Some(left) = left {
                self.focus_event(&left, false)?;
            }
            self.focus_event(identity, true)?;
            // A keyboard already up does not change again: the newly focused field clears it now.
            self.reveal_input = true;
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

    /// A tap or activation of a date or time editor opens the View's picker instead of an input method.
    pub fn open_picker(
        &mut self,
        generation: lifecycle::CommitTicket,
        identity: &str,
    ) -> Result<bool, String> {
        self.check_input_active()?;
        if self.input_commit(generation, identity, true).is_none() {
            return Ok(false);
        }
        self.open_picker_now(identity, None)
    }

    /// Open the View's picker on editor `identity` of the last layout, as a tap would; `today`
    /// (ISO) replaces the clock's date, for renders that must not change from day to day.
    pub fn open_picker_now(&mut self, identity: &str, today: Option<&str>) -> Result<bool, String> {
        self.check_input_active()?;
        // A read-only date or time field shows its value and opens no picker.
        if self.read_only(identity) {
            return Ok(false);
        }
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
        if self.input_commit(generation, identity, true).is_none() {
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
            self.choose_editor_value(&identity, value)?;
        }
        Ok(changed)
    }

    /// Give date or time editor `identity` `value` as its picker's choice would: focus, the value
    /// replacing its text with input and submit callbacks, then blur. False when it is gone.
    pub fn choose_editor_value(&mut self, identity: &str, value: String) -> Result<bool, String> {
        use crate::text::input::Edit;
        let Some(focused) = self.focus_now(identity)? else {
            return Ok(false);
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
        self.edit_input(focused.token, identity, &edits, true)?;
        self.blur_input(focused.token, identity)?;
        Ok(true)
    }

    /// Automation's `fill` on editor `identity` while no input method holds it: replace its text
    /// as typing would, with its input callback. The focus it needs lasts only this call, as a
    /// picker's choice does, so no frame shows it and no keyboard rises. False when it is gone.
    pub fn fill_editor(&mut self, identity: &str, text: String) -> Result<bool, String> {
        use crate::text::input::Edit;
        if self.read_only(identity) {
            return Err("is read-only".into());
        }
        let held = self.inputs.focused() == Some(identity);
        let Some(focused) = self.focus_now(identity)? else {
            return Ok(false);
        };
        let edits = [
            Edit::SelectAll,
            Edit::Replace {
                text,
                cursor: 1,
                composing: false,
            },
        ];
        self.edit_input(focused.token, identity, &edits, false)?;
        if !held {
            self.blur_input(focused.token, identity)?;
        }
        Ok(true)
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

    /// Blur whichever editor holds focus, as the platform's blur does. False when none does.
    pub fn blur_focused_editor(&mut self) -> Result<bool, String> {
        let Some(identity) = self.inputs.focused().map(str::to_owned) else {
            return Ok(false);
        };
        self.blur_editor(&identity)
    }

    /// Blur editor `identity` when it holds focus, as the platform's blur does. False otherwise.
    pub fn blur_editor(&mut self, identity: &str) -> Result<bool, String> {
        if self.inputs.focused() != Some(identity) {
            return Ok(false);
        }
        let token = self.inputs.reply(identity, false).token;
        self.blur_input(token, identity)
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
        // A read-only editor keeps its text: the caret and the selection still move.
        if self.read_only(identity) {
            use crate::text::input::Edit;
            edits.retain(|edit| {
                matches!(
                    edit,
                    Edit::Focus | Edit::Blur | Edit::Selection { .. } | Edit::SelectAll
                )
            });
        }
        if submit {
            edits.push(crate::text::input::Edit::FinishComposition);
        }
        let committed = inputs.edit(identity, &edits, &mut self.renderer)?;
        // Typing into an editor scrolled out of view brings it back, as native fields do.
        if !edits.is_empty() && !inputs.visible(identity) {
            self.reveal_input = true;
        }
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

    /// A paste into an editor with `onPaste`: the page hears it first, and the plain text is
    /// inserted unless its handler called `preventDefault()`.
    pub fn paste_input(
        &mut self,
        token: u64,
        identity: &str,
        text: String,
        html: Option<String>,
        plain: bool,
        html_omitted: bool,
    ) -> Result<Option<crate::InputReply>, String> {
        use crate::text::input::Edit;
        self.check_input_active()?;
        if !self.inputs.valid(identity, token) || self.read_only(identity) {
            return Ok(None);
        }
        if text.len() > crate::text::input::MAX_TEXT_BYTES {
            return Err("UI_INPUT: paste exceeds the editor's text limit".into());
        }
        let Some((handle, multiline, secure, [start, end])) = self.inputs.paste_target(identity)
        else {
            return Ok(None);
        };
        let mut handled = false;
        if let Some(handle) = handle.filter(|_| self.presented_commit(self.generation)) {
            let mut omitted = Vec::new();
            if html_omitted && !plain && !secure {
                omitted.push("html");
            }
            let html = html
                .filter(|_| !plain && !secure && !html_omitted)
                .filter(|html| {
                    let fits = html.len() <= crate::protocol::limits::PASTE_HTML_BYTES;
                    if !fits {
                        omitted.push("html");
                    }
                    fits
                });
            let payload = serde_json::json!({
                "text": text,
                "html": html,
                "selection": [start, end],
                "plain": plain,
                "omitted": omitted,
            })
            .to_string();
            let encoded = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
            let time = self.event_time();
            let outcome: i32 = self.execute(|| {
                self.context
                    .eval(Source::from_bytes(
                        format!(
                            "(r => r === 'prevented' ? 2 : r ? 1 : 0)(__lingxiaDispatch({handle},{encoded},{time}))"
                        )
                        .as_bytes(),
                    ))
                    .map_err(|e| crate::engine::js_error(&self.context, e))
            })?;
            if outcome == 2 {
                let mut reply = self.edit_input(token, identity, &[], false)?;
                if let Some(reply) = &mut reply {
                    reply.needs_view_render = true;
                }
                self.input_rebuild = true;
                return Ok(reply);
            }
            handled = outcome == 1;
        }
        // A single-line editor takes line breaks as spaces, as typing cannot enter them.
        let text = if multiline {
            text.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            text.replace("\r\n", " ").replace(['\n', '\r'], " ")
        };
        let edits = [
            Edit::FinishComposition,
            Edit::Replace {
                text,
                cursor: 1,
                composing: false,
            },
        ];
        let mut reply = self.edit_input(token, identity, &edits, false)?;
        if handled && let Some(reply) = &mut reply {
            reply.needs_view_render = true;
            self.input_rebuild = true;
        }
        Ok(reply)
    }

    pub fn blur_input(&mut self, token: u64, identity: &str) -> Result<bool, String> {
        self.check_input_active()?;
        if !self.inputs.valid(identity, token) {
            return Ok(false);
        }
        let mut inputs = self.inputs.clone();
        inputs.blur(&mut self.renderer)?;
        inputs.hold_keyboard(true);
        self.focus_event(identity, false)?;
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

    fn read_only(&self, identity: &str) -> bool {
        self.retained
            .as_ref()
            .and_then(|retained| retained.nodes.iter().find(|node| node.identity == identity))
            .and_then(|node| node.input.as_ref())
            .is_some_and(|input| input.attributes.read_only)
    }

    /// Calls the editor's `onFocus` or `onBlur`; the View renders again if one ran.
    fn focus_event(&mut self, identity: &str, focused: bool) -> Result<(), String> {
        let handle = self
            .retained
            .as_ref()
            .and_then(|retained| retained.nodes.iter().find(|node| node.identity == identity))
            .and_then(|node| node.on_focus[usize::from(!focused)]);
        if let Some(handle) = handle
            && self.dispatch_handle(handle)?
        {
            self.input_rebuild = true;
        }
        Ok(())
    }

    pub(super) fn check_input_active(&self) -> Result<(), String> {
        if self.poisoned.get() {
            Err("UI_TIMEOUT: discard the interrupted page before reusing it".into())
        } else {
            Ok(())
        }
    }
}
