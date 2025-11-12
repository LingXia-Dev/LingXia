use crate::lxapp::LxApp;
// no core UpdateManager needed here; navigate_to handles apply-downloaded
use rong::{JSContext, JSFunc, JSResult, JSValue, RongJSError, js_class, js_export, js_method};
use std::sync::Arc;

/// JS-exposed Update Manager wrapper for AppService layer
/// Stores pending update info set by native events, and applies it on demand.
#[js_export]
pub(crate) struct JSUpdateManager {
    on_ready: Option<JSFunc>,
    on_failed: Option<JSFunc>,
}

impl JSUpdateManager {
    pub fn new() -> Self {
        Self {
            on_ready: None,
            on_failed: None,
        }
    }

    pub(crate) async fn notify_update_ready(&self) {
        if let Some(cb) = &self.on_ready {
            let _ = cb.call_async::<_, ()>(None, ()).await;
        }
    }

    pub(crate) async fn notify_update_failed(&self) {
        if let Some(cb) = &self.on_failed {
            let _ = cb.call_async::<_, ()>(None, ()).await;
        }
    }
}

#[js_class]
impl JSUpdateManager {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(RongJSError::TypeError(
            "UpdateManager cannot be directly constructed".to_string(),
        ))
    }

    /// Apply update: simply restart the app.
    /// The actual apply of a downloaded package occurs inside LxApp.navigate_to
    /// which is called by restart().
    #[js_method(rename = "applyUpdate")]
    fn apply_update(&self, ctx: JSContext) -> JSResult<()> {
        let lxapp = ctx
            .get_user_data::<Arc<LxApp>>()
            .ok_or_else(|| RongJSError::Error("LxApp not found in context".to_string()))?;
        if let Err(e) = lxapp.restart() {
            let this = self.clone();
            rong::spawn(async move {
                this.notify_update_failed().await;
            });
            return Err(RongJSError::Error(e.to_string()));
        }
        Ok(())
    }

    #[js_method(rename = "onUpdateReady")]
    fn on_update_ready(&mut self, cb: JSFunc) -> JSResult<()> {
        self.on_ready = Some(cb);
        Ok(())
    }

    #[js_method(rename = "onUpdateFailed")]
    fn on_update_failed(&mut self, cb: JSFunc) -> JSResult<()> {
        self.on_failed = Some(cb);
        Ok(())
    }

    #[js_method(gc_mark)]
    fn gc_mark_with<F>(&self, mut mark_fn: F)
    where
        F: FnMut(&JSValue),
    {
        if let Some(cb) = &self.on_ready {
            mark_fn(cb.as_jsvalue());
        }
        if let Some(cb) = &self.on_failed {
            mark_fn(cb.as_jsvalue());
        }
    }
}
