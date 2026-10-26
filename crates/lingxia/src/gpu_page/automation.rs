//! Test automation for a live GPU page: requests travel to its native shell,
//! which answers on its UI thread after the presented frame.
use super::NativePage;
use base64::Engine;
use lingxia_automation::gpu::{GpuAction, GpuFuture, GpuPageAutomation};
use serde_json::{Value, json};
use std::sync::Weak;
use tokio::sync::oneshot;

const INACTIVE: &str = "page is not active: the GPU page was disposed";

pub(super) struct GpuAutomation {
    pub native: Weak<dyn NativePage>,
}

impl GpuAutomation {
    fn request(&self, request: Value) -> GpuFuture<Value> {
        let native = self.native.upgrade();
        Box::pin(async move {
            let native = native.ok_or(INACTIVE)?;
            let (tx, rx) = oneshot::channel();
            native.automation(
                request,
                Box::new(move |reply| {
                    let _ = tx.send(reply);
                }),
            );
            drop(native);
            rx.await.map_err(|_| INACTIVE.to_owned())?
        })
    }
}

impl GpuPageAutomation for GpuAutomation {
    fn query(
        &self,
        css: &str,
        index: Option<usize>,
        all: bool,
        max_text: Option<usize>,
    ) -> GpuFuture<Value> {
        self.request(
            json!({"op": "query", "css": css, "index": index, "all": all, "maxText": max_text}),
        )
    }

    fn act(&self, css: &str, index: Option<usize>, action: GpuAction) -> GpuFuture<()> {
        let action = match action {
            GpuAction::Click { force } => json!({"kind": "click", "force": force}),
            GpuAction::Fill { text, force } => {
                json!({"kind": "fill", "text": text, "force": force})
            }
            GpuAction::Type { text } => json!({"kind": "type", "text": text}),
            GpuAction::ScrollIntoView => json!({"kind": "scroll"}),
            GpuAction::Press { key } => json!({"kind": "press", "key": key}),
            GpuAction::SelectOption { value } => json!({"kind": "select", "value": value}),
            GpuAction::DoubleClick => json!({"kind": "dblclick"}),
        };
        let reply =
            self.request(json!({"op": "act", "css": css, "index": index, "action": action}));
        Box::pin(async move { reply.await.map(|_| ()) })
    }

    fn tree(&self) -> GpuFuture<Value> {
        self.request(json!({"op": "tree"}))
    }

    fn frame(&self) -> GpuFuture<Value> {
        self.request(json!({"op": "frame"}))
    }

    fn screenshot(&self) -> GpuFuture<Vec<u8>> {
        let reply = self.request(json!({"op": "screenshot"}));
        Box::pin(async move {
            let value = reply.await?;
            let png = value["png"]
                .as_str()
                .ok_or("E_PAGE_NOT_READY: no presented frame")?;
            base64::engine::general_purpose::STANDARD
                .decode(png)
                .map_err(|e| format!("GPU screenshot: {e}"))
        })
    }
}
