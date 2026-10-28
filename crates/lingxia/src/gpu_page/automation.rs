//! Test automation for a live GPU page: requests travel to its native shell,
//! which answers on its UI thread after the presented frame.
use super::NativePage;
use base64::Engine;
use lingxia_automation::gpu::{GpuAction, GpuFuture, GpuPageAutomation};
use serde_json::{Value, json};
use std::sync::Weak;
use tokio::sync::oneshot;

const INACTIVE: &str = "page is not active: the GPU page was disposed";
/// How often an act's result is polled (about once per frame), and for how long:
/// inside the 5 s locator action budget, so a stuck act reports itself.
const ACT_POLL: std::time::Duration = std::time::Duration::from_millis(16);
const ACT_SETTLE: std::time::Duration = std::time::Duration::from_secs(4);

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
        let native = self.native.clone();
        let css = css.to_owned();
        Box::pin(async move {
            // The act names a call; it has happened once the worker applied it and the frame
            // settled. A drop (`E_ACT_DROPPED`) is an error the locator retries on.
            let Some(call) = reply.await?["call"].as_u64() else {
                return Ok(());
            };
            let poll = GpuAutomation { native };
            let deadline = std::time::Instant::now() + ACT_SETTLE;
            loop {
                let answer = tokio::time::timeout_at(
                    deadline.into(),
                    poll.request(json!({"op": "result", "call": call})),
                )
                .await;
                match act_progress(answer) {
                    ActProgress::Done => return Ok(()),
                    ActProgress::Disposed => {
                        // The act's handler navigated away and the dispatching page went.
                        log::info!("act on {css} counted done: its page was disposed");
                        return Ok(());
                    }
                    ActProgress::Failed(error) => return Err(error),
                    ActProgress::Pending => {}
                }
                if std::time::Instant::now() >= deadline {
                    return Err(format!(
                        "E_TIMEOUT: the act on {css} did not settle within {}ms",
                        ACT_SETTLE.as_millis()
                    ));
                }
                tokio::time::sleep(ACT_POLL).await;
            }
        })
    }

    fn action(&self, name: &str, args: Option<Value>) -> GpuFuture<u64> {
        let reply = self.request(json!({"op": "action", "name": name, "args": args}));
        Box::pin(async move {
            reply.await?["call"]
                .as_u64()
                .ok_or_else(|| "E_UNSUPPORTED: page action answered no call".to_owned())
        })
    }

    fn result(&self, call: u64) -> GpuFuture<Option<Value>> {
        let reply = self.request(json!({"op": "result", "call": call}));
        Box::pin(async move {
            let mut reply = reply.await?;
            Ok((reply["done"] == true).then(|| reply["value"].take()))
        })
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

#[derive(Debug, PartialEq)]
pub(super) enum ActProgress {
    Done,
    Pending,
    /// The page that dispatched the act is gone.
    Disposed,
    Failed(String),
}

/// One poll of an act's result on the page instance that dispatched it.
pub(super) fn act_progress(
    answer: Result<Result<Value, String>, tokio::time::error::Elapsed>,
) -> ActProgress {
    match answer {
        Err(_) => ActProgress::Pending,
        Ok(Err(error)) if error.starts_with("page is not active") => ActProgress::Disposed,
        Ok(Err(error)) => ActProgress::Failed(error),
        Ok(Ok(reply)) if reply["done"] == true => ActProgress::Done,
        Ok(Ok(_)) => ActProgress::Pending,
    }
}
