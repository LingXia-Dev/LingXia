use super::*;

impl ViewSession {
    /// Ignore input from frames superseded while the event was queued.
    pub fn click(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
    ) -> Result<bool, String> {
        if handle == 0 {
            return Ok(false);
        }
        if !self.presented_commit(generation) {
            self.record_click(
                generation,
                false,
                Err("its frame was superseded before the click ran"),
            );
            return Ok(false);
        }
        let ran = self.dispatch_handle(handle)?;
        self.record_click(generation, false, Ok(ran));
        Ok(ran)
    }

    /// A non-input event callback (image load, scroll edge): runs like a click but is not one,
    /// so it never reaches the host's per-activation input log.
    pub(crate) fn event(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
    ) -> Result<bool, String> {
        if handle == 0 || !self.presented_commit(generation) {
            return Ok(false);
        }
        self.dispatch_handle(handle)
    }

    /// A tap on `identity` in the presented frame. If a newer View superseded that frame,
    /// the same control's current callback runs: a stale frame never costs a tap. Only a
    /// control that is gone or no longer clickable drops it.
    pub fn click_control(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
        identity: &str,
    ) -> Result<bool, String> {
        if handle == 0 || self.presented_commit(generation) || identity.is_empty() {
            return self.click(generation, handle);
        }
        let current = self.retained.as_ref().and_then(|retained| {
            retained
                .nodes
                .iter()
                .find(|node| node.identity == identity)
                .map(|node| node.on_click)
        });
        match current {
            Some(Some(handle)) if handle != 0 => {
                let ran = self.dispatch_handle(handle)?;
                self.record_click(generation, true, Ok(ran));
                Ok(ran)
            }
            Some(_) => {
                self.record_click(generation, true, Err("the control is no longer clickable"));
                Ok(false)
            }
            None => {
                self.record_click(generation, true, Err("the control is gone"));
                Ok(false)
            }
        }
    }

    /// An `onScrollEdge` from a presented frame. A newer View that superseded the frame
    /// takes it through the same scroll's current callback: the tracker reports each change
    /// once, so a dropped edge would leave the page's state wrong until the next change.
    pub(crate) fn scroll_edge(
        &mut self,
        generation: lifecycle::CommitTicket,
        edge: &crate::interaction::reach::Edge,
    ) -> Result<bool, String> {
        if self.presented_commit(generation) {
            return self.text_event(generation, edge.handle, &edge.payload);
        }
        let current = self.retained.as_ref().and_then(|retained| {
            retained
                .nodes
                .iter()
                .find(|node| node.identity == edge.identity.as_str())
                .and_then(|node| node.scroll.as_ref()?.on_scroll_edge)
        });
        match current {
            Some(handle) if handle != 0 => self.text_event(self.generation, handle, &edge.payload),
            _ => Ok(false),
        }
    }

    /// `outcome`: whether a callback ran, or why the click was refused.
    fn record_click(
        &mut self,
        commit: lifecycle::CommitTicket,
        reresolved: bool,
        outcome: Result<bool, &'static str>,
    ) {
        if self.clicks.len() < 16 {
            self.clicks.push(ClickRecord {
                commit,
                current: self.generation,
                reresolved,
                outcome,
            });
        }
    }

    /// When an event reached the page, in epoch ms: its handler's `timeStamp`.
    pub(crate) fn event_time(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as u64)
    }

    pub(super) fn dispatch_handle(&mut self, handle: u64) -> Result<bool, String> {
        // An unbound control's press changes its own value; its callback hears the new one.
        let pressed = self.uncontrolled.press(handle);
        let call = match &pressed {
            Some(value) => format!("__lingxiaDispatch({handle},{value})"),
            None => format!(
                "__lingxiaDispatch({handle},undefined,{})",
                self.event_time()
            ),
        };
        self.send_loudness()?;
        let ran: bool = self.execute(|| {
            self.context
                .eval(Source::from_bytes(call.as_bytes()))
                .map_err(|e| crate::engine::js_error(&self.context, e))
        })?;
        self.take_handler_failure()?;
        if pressed.is_some() {
            // The control repaints with its own value whether or not the page re-renders.
            self.input_dirty = true;
        }
        Ok(ran || pressed.is_some())
    }

    /// Only text callbacks accept this payload; stale displayed-frame events are ignored.
    /// A `<select>` chose option `index` (picker, arrow key or automation); false when stale or unchanged.
    pub fn select_option(
        &mut self,
        generation: lifecycle::CommitTicket,
        identity: &str,
        index: usize,
    ) -> Result<bool, String> {
        let Some(select) = self.retained.as_ref().and_then(|retained| {
            retained
                .nodes
                .iter()
                .find(|node| node.identity == identity)
                .and_then(|node| node.select.clone())
        }) else {
            return Ok(false);
        };
        // A choice that arrives another way (automation, a key) closes this select's picker.
        let closed = self.close_view_picker_on(identity)?;
        let (Some(option), Some(handle)) = (select.options.get(index), select.on_select) else {
            return Ok(closed);
        };
        if !option.enabled || select.selected == Some(index) {
            return Ok(closed);
        }
        Ok(self.text_event(generation, handle, &option.value)? || closed)
    }

    /// A View-side choice (automation, arrow keys) made on a frame the View has since
    /// re-rendered finds its option again by value, as a click finds its control by identity.
    pub fn choose_option(
        &mut self,
        generation: lifecycle::CommitTicket,
        identity: &str,
        index: usize,
        value: &str,
    ) -> Result<bool, String> {
        if self.presented_commit(generation) {
            return self.select_option(generation, identity, index);
        }
        let current = self.retained.as_ref().and_then(|retained| {
            let node = retained
                .nodes
                .iter()
                .find(|node| node.identity == identity)?;
            node.select
                .as_ref()?
                .options
                .iter()
                .position(|option| option.value == value)
        });
        match current {
            Some(index) => self.select_option(self.generation, identity, index),
            None => {
                // Reported like a refused click: the host warns and fails the automation call.
                self.record_click(generation, true, Err("its option is gone"));
                Ok(false)
            }
        }
    }

    pub fn text_event(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
        text: &str,
    ) -> Result<bool, String> {
        if !self.presented_commit(generation) || handle == 0 {
            return Ok(false);
        }
        if text.len() > crate::text::input::MAX_TEXT_BYTES {
            return Err("UI_INPUT: text too long".into());
        }
        // An unbound select takes the choice itself; its callback hears only a change.
        let took = match self.uncontrolled.choose(handle, text) {
            uncontrolled::Proposal::Unchanged => return Ok(false),
            uncontrolled::Proposal::Changed(_) => {
                self.input_dirty = true;
                true
            }
            uncontrolled::Proposal::Bound => false,
        };
        let encoded = serde_json::to_string(text).map_err(|e| e.to_string())?;
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!("__lingxiaDispatch({handle},{encoded})").as_bytes(),
                ))
                .map_err(|e| crate::engine::js_error(&self.context, e))
        })
        .map(|ran: bool| ran || took)
    }

    /// Collects the `onLayout` elements whose content size differs from the one last reported.
    pub(crate) fn note_boxes(&mut self) {
        let Some(retained) = &self.retained else {
            return;
        };
        // An eighth of a point: a re-render that nudges the size settles.
        let size = |node: &crate::scene::node::NodeSnapshot| {
            let [min, max] = node.intrinsic.unwrap_or_default();
            [node.content_box[2], node.content_box[3], min, max].map(|v| (v * 8.0).round() / 8.0)
        };
        let observed: HashMap<&str, [f32; 4]> = retained
            .nodes
            .iter()
            .filter(|node| node.on_layout.is_some())
            .map(|node| (&*node.identity, size(node)))
            .collect();
        self.boxes
            .retain(|identity, _| observed.contains_key(identity.as_str()));
        self.boxes_due = observed
            .into_iter()
            .filter(|(identity, size)| self.boxes.get(*identity) != Some(size))
            .map(|(identity, size)| (identity.to_owned(), size))
            .collect();
        self.boxes_due.sort_by(|a, b| a.0.cmp(&b.0));
    }

    /// A frame left `onLayout` or `onSelectionChange` calls to make once it lands.
    pub(crate) fn frame_events_due(&self) -> bool {
        !self.boxes_due.is_empty() || self.inputs.selection_due()
    }

    /// Calls `onSelectionChange` for the focused editor's selection the frame moved or showed
    /// elsewhere, then `onLayout` for each changed box through the element's current callback;
    /// true when one ran.
    pub fn report_frame_events(&mut self) -> Result<bool, String> {
        let mut ran = false;
        for (handle, payload) in self.inputs.take_selection_changes() {
            ran |= self.text_event(self.generation, handle, &payload)?;
        }
        for (identity, size) in std::mem::take(&mut self.boxes_due) {
            let handle = self
                .retained
                .as_ref()
                .and_then(|retained| {
                    retained
                        .nodes
                        .iter()
                        .find(|node| *node.identity == *identity)
                })
                .and_then(|node| node.on_layout);
            let Some(handle) = handle else {
                continue;
            };
            self.boxes.insert(identity, size);
            let payload = format!(
                r#"{{"width":{},"height":{},"minContentWidth":{},"maxContentWidth":{}}}"#,
                size[0], size[1], size[2], size[3]
            );
            ran |= self.text_event(self.generation, handle, &payload)?;
        }
        Ok(ran)
    }

    /// A `drag` element's event. One a newer render overtook goes to the element's current
    /// handler: a flick released before its start's render lands must still end.
    pub fn drag_event(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
        identity: &str,
        payload: &str,
    ) -> Result<bool, String> {
        if self.presented_commit(generation) || identity.is_empty() {
            return self.text_event(generation, handle, payload);
        }
        let current = self
            .retained
            .as_ref()
            .and_then(|retained| retained.nodes.iter().find(|node| node.identity == identity))
            .and_then(|node| node.drag.as_ref())
            .map(|drag| drag.handle);
        match current {
            Some(handle) => self.text_event(self.generation, handle, payload),
            None => Ok(false),
        }
    }

    /// A slider's press began or ended. A press runs the element's current callback, as a
    /// re-render renumbers them; a release needs no callback, so a slider gone since still hears it.
    pub fn slider_press(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
        identity: &str,
        pressed: bool,
    ) -> Result<bool, String> {
        if !pressed {
            return self.execute(|| {
                self.context
                    .eval(Source::from_bytes(
                        "typeof __lingxiaReleasePress === 'function' && __lingxiaReleasePress()",
                    ))
                    .map_err(|e| crate::engine::js_error(&self.context, e))
            });
        }
        let current = self
            .retained
            .as_ref()
            .and_then(|retained| retained.nodes.iter().find(|node| node.identity == identity))
            .map(|node| node.on_press);
        match current {
            Some(Some(handle)) => self.dispatch_handle(handle),
            Some(None) => Ok(false),
            None if self.presented_commit(generation) => self.dispatch_handle(handle),
            None => Ok(false),
        }
    }

    /// A fine pointer crossed `identity`'s box. A re-render renumbers callbacks without a new
    /// commit, so the element's current callback runs; the frame's handle only if it is gone.
    pub fn pointer_crossing(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
        identity: &str,
        enter: bool,
    ) -> Result<bool, String> {
        let current = self
            .retained
            .as_ref()
            .and_then(|retained| retained.nodes.iter().find(|node| node.identity == identity))
            .map(|node| node.on_pointer[usize::from(!enter)]);
        match current {
            Some(Some(handle)) => self.dispatch_handle(handle),
            Some(None) => Ok(false),
            None if self.presented_commit(generation) => self.dispatch_handle(handle),
            None => Ok(false),
        }
    }

    /// A key the editor `identity` lists in `onKeyDown`, through its current callback.
    pub fn editor_key(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
        identity: &str,
        key: &str,
    ) -> Result<bool, String> {
        let current = self
            .retained
            .as_ref()
            .and_then(|retained| retained.nodes.iter().find(|node| node.identity == identity))
            .map(|node| node.key_map.as_ref().map(|(handle, _)| *handle));
        match current {
            Some(Some(handle)) => self.text_event(self.generation, handle, key),
            Some(None) => Ok(false),
            None => self.text_event(generation, handle, key),
        }
    }

    /// A grid's `onPage` for `identity`, through its current callback.
    pub fn grid_page_event(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
        identity: &str,
        next: bool,
    ) -> Result<bool, String> {
        let direction = if next { "next" } else { "previous" };
        let current = self
            .retained
            .as_ref()
            .and_then(|retained| retained.nodes.iter().find(|node| node.identity == identity))
            .map(|node| node.on_page);
        match current {
            Some(Some(handle)) => self.text_event(self.generation, handle, direction),
            Some(None) => Ok(false),
            None => self.text_event(generation, handle, direction),
        }
    }

    /// `onContextMenu` for `identity`, through the element's current callback.
    pub fn context_menu_event(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
        identity: &str,
        payload: &str,
    ) -> Result<bool, String> {
        let current = self
            .retained
            .as_ref()
            .and_then(|retained| retained.nodes.iter().find(|node| node.identity == identity))
            .map(|node| node.on_context_menu);
        match current {
            Some(Some(handle)) => self.text_event(self.generation, handle, payload),
            Some(None) => Ok(false),
            None => self.text_event(generation, handle, payload),
        }
    }

    /// `onPointer` for `identity`, through the element's current callback as a re-render
    /// renumbers them; the frame's handle only if the element is gone.
    pub fn pointer_event(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
        identity: &str,
        payload: &str,
    ) -> Result<bool, String> {
        let current = self
            .retained
            .as_ref()
            .and_then(|retained| retained.nodes.iter().find(|node| node.identity == identity))
            .map(|node| node.on_pointer_event);
        match current {
            Some(Some((handle, _))) => self.text_event(self.generation, handle, payload),
            Some(None) => Ok(false),
            None => self.text_event(generation, handle, payload),
        }
    }

    /// Resolve and dispatch selection against committed geometry and callback identity.
    pub fn select_plot(
        &mut self,
        generation: lifecycle::CommitTicket,
        identity: &str,
        selection: crate::PlotSelection,
    ) -> Result<bool, String> {
        self.check_input_active()?;
        let Some(generation) = self.input_commit(generation, identity, false) else {
            return Ok(false);
        };
        if self.input_dirty || self.pending_inputs.is_some() {
            return Ok(false);
        }
        let Some(node) = self
            .input_hits
            .as_ref()
            .and_then(|hits| hits.plot_node(identity))
        else {
            return Ok(false);
        };
        let plot = node.plot.as_ref().unwrap();
        let Some(handle) = plot.on_select.filter(|handle| *handle != 0) else {
            return Ok(false);
        };
        let index = match selection {
            crate::PlotSelection::Point(point) => {
                if point.iter().any(|value| !value.is_finite()) {
                    return Ok(false);
                }
                node.hit_test_plot(point).map_or(-1, |index| index as i64)
            }
            crate::PlotSelection::Next => plot.next_selection().map_or(-1, |index| index as i64),
            crate::PlotSelection::Index(index) => index,
        };
        if !(index == -1 && plot.selected_index >= 0)
            && !usize::try_from(index)
                .ok()
                .is_some_and(|index| plot.selectable(index))
        {
            return Ok(false);
        }
        self.number_event(generation, handle, index as f64)
    }

    /// A slider's value from the presented frame; one a newer render overtook goes to the
    /// control's current callback.
    pub fn number_control(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
        identity: &str,
        value: f64,
    ) -> Result<bool, String> {
        if self.presented_commit(generation) || identity.is_empty() {
            return self.number_event(generation, handle, value);
        }
        let current = self
            .retained
            .as_ref()
            .and_then(|retained| retained.nodes.iter().find(|node| node.identity == identity))
            .and_then(|node| node.range.as_ref())
            .and_then(|range| range.on_change);
        match current {
            Some(handle) => self.number_event(self.generation, handle, value),
            None => Ok(false),
        }
    }

    /// A bound snapping scroll's resting index; one a newer render overtook goes to the
    /// region's current callback.
    pub fn snap_event(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
        region: &str,
        index: usize,
    ) -> Result<bool, String> {
        if self.presented_commit(generation) || region.is_empty() {
            return self.text_event(generation, handle, &index.to_string());
        }
        let current = self
            .retained
            .as_ref()
            .and_then(|retained| {
                retained
                    .nodes
                    .iter()
                    .find_map(|node| node.scroll.as_ref().filter(|scroll| scroll.id == region))
            })
            .and_then(|scroll| scroll.on_snap);
        match current {
            Some(handle) => self.text_event(self.generation, handle, &index.to_string()),
            None => Ok(false),
        }
    }

    /// Propose a finite scalar to a numeric control from the displayed generation.
    pub fn number_event(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
        value: f64,
    ) -> Result<bool, String> {
        if !self.presented_commit(generation) || handle == 0 {
            return Ok(false);
        }
        if !value.is_finite() || value.abs() > 1e9 {
            return Err("UI_SLIDER: invalid proposed value".into());
        }
        // An unbound slider takes the value itself; its callback hears only a change.
        let (encoded, took) = match self.uncontrolled.propose(handle, value) {
            uncontrolled::Proposal::Unchanged => return Ok(false),
            uncontrolled::Proposal::Changed(encoded) => {
                self.input_dirty = true;
                (encoded, true)
            }
            uncontrolled::Proposal::Bound => (
                serde_json::to_string(&value).map_err(|e| e.to_string())?,
                false,
            ),
        };
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!("__lingxiaDispatch({handle},{encoded})").as_bytes(),
                ))
                .map_err(|e| crate::engine::js_error(&self.context, e))
        })
        .map(|ran: bool| ran || took)
    }
}

impl ViewSession {
    /// Automation: the off-screen list row that would hold `test_id`, as its list's scroll
    /// id and the offset that brings it to the top. Off-screen rows are rendered only here.
    pub fn probe_list_row(&mut self, test_id: &str) -> Result<Option<(String, f32)>, String> {
        let encoded = serde_json::to_string(test_id).map_err(|e| e.to_string())?;
        let found: String = self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!("typeof __lingxiaProbeListRow === 'function' ? __lingxiaProbeListRow({encoded}) : 'null'").as_bytes(),
                ))
                .map_err(|e| crate::engine::js_error(&self.context, e))
        })?;
        #[derive(serde::Deserialize)]
        struct Found {
            list: String,
            index: usize,
        }
        let Some(found) =
            serde_json::from_str::<Option<Found>>(&found).map_err(|e| e.to_string())?
        else {
            return Ok(None);
        };
        let viewport = self
            .retained
            .as_ref()
            .and_then(|retained| {
                retained
                    .nodes
                    .iter()
                    .find(|node| node.scroll.as_ref().is_some_and(|s| s.id == found.list))
            })
            .map(|node| node.bounds[3]);
        Ok(viewport.and_then(|viewport| {
            crate::layout::list_layout::row_offset(&self.lists, &found.list, found.index, viewport)
                .map(|offset| (found.list, offset))
        }))
    }
}
