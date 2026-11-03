use super::*;

impl ViewInteraction {
    pub(super) fn propose_number_node(
        &mut self,
        node: &NodeSnapshot,
        value: f64,
        page: PageTicket,
    ) -> Vec<Effect> {
        if self.slider.propose(node, value, page) {
            vec![Effect::Number]
        } else {
            Vec::new()
        }
    }

    /// Propose a value for page `page`; the adapter dispatches it with `take_number`.
    pub fn propose_number(&mut self, index: usize, value: f64, page: PageTicket) -> Vec<Effect> {
        let Some(node) = self
            .frame
            .as_ref()
            .and_then(|frame| frame.node(index))
            .cloned()
        else {
            return Vec::new();
        };
        if !frame::slider_enabled(&node)
            || (!frame::visible(&node)
                && self
                    .pending_focus
                    .as_ref()
                    .is_none_or(|p| p.identity != node.identity))
        {
            return Vec::new();
        }
        if self.slider.propose(&node, value, page) {
            vec![Effect::Number]
        } else {
            Vec::new()
        }
    }

    /// The next slider value to send: commit, callback handle, value and page.
    pub fn take_number(&mut self) -> Option<(CommitTicket, u64, f64, Proposal)> {
        if self.pending_focus.is_some() {
            return None;
        }
        let frame = self.frame.clone()?;
        let proposal = self.slider.begin_flight()?;
        let node = frame.nodes.iter().find(|node| proposal.matches(node))?;
        let range = node.range.as_ref()?;
        if !frame::visible(node) || proposal.value == range.value {
            self.slider.end_flight();
            return None;
        }
        Some((frame.commit, range.on_change?, proposal.value, proposal))
    }

    pub fn number_done(&mut self) {
        self.slider.end_flight();
    }

    pub fn number_current(&self, proposal: &Proposal) -> bool {
        self.slider.is_current(proposal)
    }

    // ---- keyboard ------------------------------------------------------
}
