use super::*;

impl ViewSession {
    /// Ignore input from frames superseded while the event was queued.
    pub fn click(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
    ) -> Result<bool, String> {
        if generation != self.generation || handle == 0 {
            return Ok(false);
        }
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!("__lingxiaDispatch({handle})").as_bytes(),
                ))
                .map_err(|e| e.to_string())
        })
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
        let (Some(option), Some(handle)) = (select.options.get(index), select.on_select) else {
            return Ok(false);
        };
        if !option.enabled || select.selected == Some(index) {
            return Ok(false);
        }
        self.text_event(generation, handle, &option.value)
    }

    pub fn text_event(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
        text: &str,
    ) -> Result<bool, String> {
        if generation != self.generation || handle == 0 {
            return Ok(false);
        }
        if text.len() > crate::text::input::MAX_TEXT_BYTES {
            return Err("UI_INPUT: text too long".into());
        }
        let encoded = serde_json::to_string(text).map_err(|e| e.to_string())?;
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!("__lingxiaDispatch({handle},{encoded})").as_bytes(),
                ))
                .map_err(|e| e.to_string())
        })
    }

    /// Resolve and dispatch selection against committed geometry and callback identity.
    pub fn select_plot(
        &mut self,
        generation: lifecycle::CommitTicket,
        identity: &str,
        selection: crate::PlotSelection,
    ) -> Result<bool, String> {
        self.check_input_active()?;
        if generation != self.generation || self.input_dirty || self.pending_inputs.is_some() {
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

    /// Propose a finite scalar to a numeric control from the displayed generation.
    pub fn number_event(
        &mut self,
        generation: lifecycle::CommitTicket,
        handle: u64,
        value: f64,
    ) -> Result<bool, String> {
        if generation != self.generation || handle == 0 {
            return Ok(false);
        }
        if !value.is_finite() || value.abs() > 1e9 {
            return Err("UI_SLIDER: invalid proposed value".into());
        }
        let encoded = serde_json::to_string(&value).map_err(|e| e.to_string())?;
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!("__lingxiaDispatch({handle},{encoded})").as_bytes(),
                ))
                .map_err(|e| e.to_string())
        })
    }
}
