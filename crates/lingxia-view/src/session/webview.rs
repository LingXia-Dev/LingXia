//! `<webview>` event delivery into the View runtime.
use super::ViewSession;

impl ViewSession {
    pub(crate) fn webview_scope(&self) -> u64 {
        self.renderer.webviews.scope()
    }

    /// Delivers batched `<webview>` events (`(web_id, events)`) to every mounted `<webview>`
    /// node of `web_id` that registered `onWebEvent`. Returns `true` when any callback ran or
    /// invalidated the page.
    pub fn dispatch_webview_events(
        &mut self,
        batches: &[(String, Vec<serde_json::Value>)],
    ) -> Result<bool, String> {
        let generation = self.generation;
        let targets: Vec<(u64, Vec<serde_json::Value>)> = {
            let Some(retained) = &self.retained else {
                return Ok(false);
            };
            let mut list = Vec::new();
            for (web_id, events) in batches {
                if events.is_empty() {
                    continue;
                }
                for handle in retained.nodes.iter().filter_map(|node| {
                    node.webview
                        .as_ref()
                        .filter(|wv| wv.web_id == *web_id)
                        .and_then(|wv| wv.on_event)
                        .filter(|handle| *handle != 0)
                }) {
                    list.push((handle, events.clone()));
                }
            }
            list
        };
        let mut changed = false;
        for (handle, events) in targets {
            let payload = serde_json::json!({ "events": events }).to_string();
            changed |= self.text_event(generation, handle, &payload)?;
        }
        Ok(changed)
    }
}
