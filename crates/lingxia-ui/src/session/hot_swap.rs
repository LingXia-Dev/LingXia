//! Dev sessions: replace the page module in place. The new isolate takes the
//! committed local state; scroll, focus and selection stay on the native side.
use super::*;

impl ViewSession {
    /// Evaluate `source` as this page's module and render it with the old state.
    /// On any failure the previous module keeps running and the error is returned.
    pub fn swap_module(&mut self, source: &str) -> Result<(), String> {
        let state: String = self
            .context
            .eval(Source::from_bytes(
                "typeof __lingxiaExportState === 'function' ? __lingxiaExportState() : '[]'",
            ))
            .map_err(|e| crate::engine::js_error(&self.context, e))?;
        let runtime = RongJS::runtime();
        let budget = crate::engine::budget::Budget::new(&runtime);
        let mut fresh = Self::with_runtime(runtime, budget, source, false)?;
        self.exchange_isolate(&mut fresh);
        let imported = (|| {
            let state = serde_json::to_string(&state).map_err(|e| e.to_string())?;
            self.eval(&format!(
                "typeof __lingxiaImportState === 'function' && __lingxiaImportState({state})"
            ))?;
            if self.bridge.as_ref().is_some_and(|bridge| bridge.ready) {
                self.eval("__lingxiaConfigureActions(globalThis.__lingxiaActionCatalog)")?;
            }
            // Render once now so a module that cannot render never replaces one that can.
            if let Some((width, height, scale)) = self.viewport {
                self.prepare_native(width, height, scale, true, None, |_| Ok(()))?;
            }
            Ok(())
        })();
        if let Err(error) = imported {
            self.exchange_isolate(&mut fresh);
            return Err(error);
        }
        Ok(())
    }

    fn exchange_isolate(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.context, &mut other.context);
        std::mem::swap(&mut self._runtime, &mut other._runtime);
        std::mem::swap(&mut self.budget, &mut other.budget);
        let poisoned = self.poisoned.get();
        self.poisoned.set(other.poisoned.get());
        other.poisoned.set(poisoned);
    }
}
