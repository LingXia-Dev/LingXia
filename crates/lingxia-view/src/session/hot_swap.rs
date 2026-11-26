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
        let mut fresh = Self::with_runtime(
            runtime,
            budget,
            source,
            false,
            self.console.clone(),
            self.chunks.clone(),
        )?;
        self.exchange_isolate(&mut fresh);
        let imported = (|| {
            let state = serde_json::to_string(&state).map_err(|e| e.to_string())?;
            self.eval(&format!(
                "typeof __lingxiaImportState === 'function' && __lingxiaImportState({state})"
            ))?;
            let language = self.environment["language"]
                .as_str()
                .unwrap_or("")
                .to_owned();
            self.ensure_language_catalogue(&language)?;
            if self.connected {
                self.eval("__lingxiaConfigureActions(globalThis.__lingxiaActionCatalog)")?;
            }
            // Render once now so a module that cannot render never replaces one that can.
            if let Some((width, height, scale)) = self.viewport {
                self.prepare_trial(width, height, scale)?;
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
        std::mem::swap(&mut self.chunk_build_id, &mut other.chunk_build_id);
        std::mem::swap(&mut self.catalogue_chunk, &mut other.catalogue_chunk);
        std::mem::swap(&mut self.catalogue_source, &mut other.catalogue_source);
        std::mem::swap(
            &mut self.catalogue_languages,
            &mut other.catalogue_languages,
        );
        std::mem::swap(&mut self.loaded_languages, &mut other.loaded_languages);
        crate::engine::intl::update(&self.context, &self.intl);
        // The buffers each isolate referenced are its own.
        self.wire.swap(&other.wire);
        // Each keeps resolving against its own page's scales.
        let _ = self.wire.borrow_mut().set_scales(self.scales.clone());
        let _ = other.wire.borrow_mut().set_scales(other.scales.clone());
        // A swapped-in module's first frame warms the engine, as a loaded one's does.
        self.warm.swap(&other.warm);
        // Until the new module has run, it is taken to read everything.
        self.reads_host_context = true;
        self.reads_appearance = true;
        self.overruns.swap(&other.overruns);
        let poisoned = self.poisoned.get();
        self.poisoned.set(other.poisoned.get());
        other.poisoned.set(poisoned);
    }
}
