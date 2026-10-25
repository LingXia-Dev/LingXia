use crate::protocol::field;
pub mod app;
pub mod controller;
pub mod image_requests;
pub(crate) mod input_protocol;
pub mod lifecycle;
pub mod metrics;
pub mod state;

use crate::engine::decode_context_timed;
use crate::{DisplayList, Frame, Renderer};
use rong::{JSEngine, RongJS, Source};
use std::{cell::Cell, collections::HashMap, sync::Arc, time::Duration};
mod bridge;
mod editing;
mod environment;
mod events;
mod frame_animation;
mod images;
mod prepare;
#[cfg(test)]
pub(crate) mod tests;
use frame_animation::FrameAnimations;

/// Thread-confined View context. Candidates commit only after native validation and layout.
pub struct ViewSession {
    context: rong::JSContext,
    _runtime: rong::JSRuntime,
    renderer: Renderer,
    generation: lifecycle::CommitTicket,
    state: crate::session::state::StateReplica,
    budget: Arc<crate::engine::budget::Budget>,
    poisoned: Cell<bool>,
    environment: serde_json::Value,
    bridge: Option<ViewBridge>,
    retained: Option<Arc<crate::scene::retained::RetainedLayout>>,
    input_hits: Option<crate::interaction::input_hits::InputHitMap>,
    viewport: Option<(u16, u16, f32)>,
    scroll_offsets: HashMap<String, f32>,
    lists: crate::layout::list_layout::Lists,
    images: HashMap<String, crate::scene::image::Bitmap>,
    image_sources: std::collections::HashSet<String>,
    image_status: HashMap<crate::ImageSource, bool>,
    image_notified: std::collections::HashSet<(String, crate::ImageSource)>,
    inputs: crate::interaction::input_state::Inputs,
    input_records: Option<Vec<crate::protocol::Record>>,
    input_dirty: bool,
    input_rebuild: bool,
    reveal_input: bool,
    pending_inputs: Option<crate::interaction::input_state::Inputs>,
    metrics: crate::FrameMetrics,
    animations: crate::animation::Timeline,
    animation_surface: Option<lifecycle::SurfaceTicket>,
    pending_animation: Option<(
        lifecycle::CommitTicket,
        lifecycle::SurfaceTicket,
        crate::animation::Prepared,
    )>,
    animation_inventory: crate::animation::records::Inventory,
    animation_visible: std::collections::BTreeSet<String>,
    /// Native pointer state selecting `active:`/`hover:` paint deltas.
    pressed: Option<String>,
    hovered: Option<String>,
}

struct ViewBridge {
    nonce: String,
    session: Option<String>,
    ready: bool,
}

impl ViewSession {
    pub fn new(source: &str) -> Result<Self, String> {
        Self::new_with_logic(source, false)
    }

    pub fn new_with_logic(source: &str, logic: bool) -> Result<Self, String> {
        let runtime = RongJS::runtime();
        let budget = crate::engine::budget::Budget::new(&runtime);
        Self::with_runtime(runtime, budget, source, logic)
    }

    pub(crate) fn with_runtime(
        runtime: rong::JSRuntime,
        budget: Arc<crate::engine::budget::Budget>,
        source: &str,
        logic: bool,
    ) -> Result<Self, String> {
        crate::engine::compatibility::validate(source)?;
        let context = runtime.context();
        let initialized = budget.run(Duration::from_secs(1), || {
            crate::engine::compatibility::install(&context)?;
            <crate::engine::Active as crate::engine::Engine>::evaluate_module(&context, source)?;
            let valid: bool = context.eval(Source::from_bytes(
                "['Render','Prepare','Commit','Discard','Reset','Dispatch','CreateView'].every(name=>typeof globalThis['__lingxia'+name]==='function')"
            )).map_err(|e| e.to_string())?;
            if !valid {
                return Err("UI_MODULE: missing render or compiled UI runtime exports".into());
            }
            <crate::engine::Active as crate::engine::Engine>::drain_jobs(&context)
        });
        if let Err(error) = initialized {
            <crate::engine::Active as crate::engine::Engine>::discard_jobs(&context);
            return Err(error);
        }
        Ok(Self {
            context,
            _runtime: runtime,
            renderer: Renderer::default(),
            generation: lifecycle::CommitTicket::new(0),
            budget,
            poisoned: Cell::new(false),
            bridge: None,
            retained: None,
            input_hits: None,
            viewport: None,
            scroll_offsets: HashMap::new(),
            lists: HashMap::new(),
            images: HashMap::new(),
            image_sources: Default::default(),
            image_status: Default::default(),
            image_notified: Default::default(),
            inputs: Default::default(),
            input_records: None,
            animation_inventory: Default::default(),
            animation_visible: Default::default(),
            input_dirty: false,
            input_rebuild: false,
            reveal_input: false,
            pending_inputs: None,
            metrics: Default::default(),
            animations: Default::default(),
            animation_surface: None,
            pending_animation: None,
            pressed: None,
            hovered: None,
            environment: serde_json::json!({"theme":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":std::env::consts::OS,"formFactor":"phone","aside":false,"displayLanguage":"en","runner":true,"pointer":"coarse"}),
            state: if logic {
                crate::session::state::StateReplica::default()
            } else {
                crate::session::state::StateReplica::static_page()
            },
        })
    }

    /// The committed View description that input must target.
    pub fn generation(&self) -> lifecycle::CommitTicket {
        self.generation
    }

    fn advance(&mut self) {
        self.generation = lifecycle::CommitTicket::new(self.generation.get() + 1);
    }

    /// Keep module globals and replicated Logic data, but release local state and input handles.
    pub fn reset(&mut self) -> Result<(), String> {
        self.eval("__lingxiaReset()")?;
        self.animations.clear()?;
        self.pending_animation = None;
        self.renderer.clear_text_cache();
        self.retained = None;
        self.input_hits = None;
        self.viewport = None;
        self.scroll_offsets.clear();
        self.images.clear();
        self.image_sources.clear();
        self.image_status.clear();
        self.image_notified.clear();
        self.lists.clear();
        self.inputs.clear();
        self.input_records = None;
        self.animation_inventory = Default::default();
        self.animation_visible.clear();
        self.input_dirty = false;
        self.input_rebuild = false;
        self.reveal_input = false;
        self.pending_inputs = None;
        self.metrics = Default::default();
        self.advance();
        Ok(())
    }

    fn eval(&self, source: &str) -> Result<(), String> {
        self.execute(|| {
            self.context
                .eval::<()>(Source::from_bytes(source.as_bytes()))
                .map_err(|e| crate::engine::js_error(&self.context, e))
        })
    }

    fn execute<T>(&self, work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        if self.poisoned.get() {
            return Err("UI_TIMEOUT: discard the interrupted page before reusing it".into());
        }
        let result = self.budget.run(Duration::from_millis(100), || {
            let value = work()?;
            <crate::engine::Active as crate::engine::Engine>::drain_jobs(&self.context)?;
            Ok(value)
        });
        if result.is_err() {
            <crate::engine::Active as crate::engine::Engine>::discard_jobs(&self.context);
        }
        if result
            .as_ref()
            .is_err_and(|error| error.starts_with("UI_TIMEOUT:"))
        {
            self.poisoned.set(true);
        }
        result
    }
}
