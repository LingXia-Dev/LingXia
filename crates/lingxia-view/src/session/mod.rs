use crate::protocol::field;
pub mod app;
pub mod host;
pub mod image_requests;
pub(crate) mod input_protocol;
pub mod lifecycle;
pub mod metrics;
pub mod state;

use crate::engine::{RongJS, decode_buffer};
use crate::{DisplayList, Frame, Renderer};
use rong::{JSEngine, Source};
use std::{cell::Cell, collections::HashMap, sync::Arc, time::Duration};
mod bridge;
mod editing;
mod environment;
mod events;
mod fonts;
mod frame_animation;
#[cfg(feature = "hot-swap")]
mod hot_swap;
mod images;
mod prepare;
mod selection;
#[cfg(test)]
pub(crate) mod tests;
use frame_animation::FrameAnimations;

/// Thread-confined View context. Candidates commit only after native validation and layout.
pub struct ViewSession {
    context: crate::engine::JSContext,
    _runtime: crate::engine::JSRuntime,
    renderer: Renderer,
    generation: lifecycle::CommitTicket,
    state: crate::session::state::StateReplica,
    budget: Arc<crate::engine::budget::Budget>,
    poisoned: Cell<bool>,
    /// The next frame is the module's first since it loaded or was swapped in: it also warms
    /// the engine, so it gets the warm-up budget.
    warm: Cell<bool>,
    /// Frames in a row that finished past their budget; each was dropped.
    overruns: Cell<u32>,
    environment: serde_json::Value,
    bridge: Option<ViewBridge>,
    retained: Option<Arc<crate::scene::retained::RetainedLayout>>,
    /// Bumped whenever a frame commits a different retained layout.
    layout_revision: u64,
    /// The first commit of the current layout revision.
    /// The last commit the host presented: the frame the user saw.
    seen_commit: lifecycle::CommitTicket,
    layout_commit: lifecycle::CommitTicket,
    /// Clicks since the host last collected them.
    clicks: Vec<ClickRecord>,
    /// The first commit of the current retained layout (editor geometry is valid from here).
    retained_commit: lifecycle::CommitTicket,
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
    /// The last buffer the View encoded and the host decoded; the next may reference it.
    wire: std::cell::RefCell<crate::protocol::Wire>,
    input_dirty: bool,
    /// The View's last run read a host-context key (`env.sizeClass`, `env.canDock`).
    reads_host_context: bool,
    input_rebuild: bool,
    reveal_input: bool,
    pending_inputs: Option<crate::interaction::input_state::Inputs>,
    metrics: crate::FrameMetrics,
    animations: crate::animation::Timeline,
    animation_surface: Option<lifecycle::SurfaceTicket>,
    /// An unpresented frame that a newer, prepared-ahead frame builds on.
    presenting_animation: Option<(
        lifecycle::CommitTicket,
        lifecycle::SurfaceTicket,
        crate::animation::Prepared,
    )>,
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
    /// A non-editor control holding keyboard focus with a `focus:` paint delta (a `<select>`).
    focused_control: Option<String>,
    /// Limits this page has come near, each warned once; drained by the host.
    budget_warned: std::collections::BTreeSet<&'static str>,
    budget_warnings: Vec<String>,
}

/// The error a frame build returns when the presenter declines the frame; the candidate is
/// dropped but the View's pending state survives for the next frame.
pub const FRAME_DECLINED: &str = "frame declined by the target";

/// One click the session handled, for the host to log and count.
#[derive(Clone, Debug, PartialEq)]
pub struct ClickRecord {
    /// The frame the click was made on and the session's commit when it ran.
    pub commit: lifecycle::CommitTicket,
    pub current: lifecycle::CommitTicket,
    /// It ran the same control's current callback because its frame was superseded.
    pub reresolved: bool,
    /// Whether a callback ran, or why the click was refused.
    pub outcome: Result<bool, &'static str>,
}

struct ViewBridge {
    nonce: String,
    session: Option<String>,
    ready: bool,
}

/// Prefix of a frame dropped for finishing past its budget; the page keeps running.
pub const OVERRUN: &str = "UI_OVERRUN:";
/// A frame asked to build ahead while two frames already wait under it.
pub(crate) const AHEAD_REFUSED: &str = "UI_ANIMATION: two frames already wait for presentation";

/// The CPU-time budget for one View evaluation; unoptimized builds get a multiple.
pub(crate) fn eval_budget() -> Duration {
    use crate::protocol::limits::{EVAL_BUDGET_DEBUG_MULTIPLIER, EVAL_BUDGET_MS};
    let scale = if cfg!(debug_assertions) {
        EVAL_BUDGET_DEBUG_MULTIPLIER
    } else {
        1
    };
    Duration::from_millis((EVAL_BUDGET_MS * scale) as u64)
}

impl ViewSession {
    /// Timings and counters of the last prepared frame.
    pub fn metrics(&self) -> &crate::FrameMetrics {
        &self.metrics
    }

    pub fn new(source: &str) -> Result<Self, String> {
        Self::new_with_logic(source, false)
    }

    pub fn new_with_logic(source: &str, logic: bool) -> Result<Self, String> {
        let runtime = RongJS::runtime();
        let budget = crate::engine::budget::Budget::new(&runtime);
        Self::with_runtime(runtime, budget, source, logic)
    }

    pub(crate) fn with_runtime(
        runtime: crate::engine::JSRuntime,
        budget: Arc<crate::engine::budget::Budget>,
        source: &str,
        logic: bool,
    ) -> Result<Self, String> {
        crate::engine::compatibility::validate(source)?;
        let context = runtime.context();
        let (initialized, _) = budget.run(Duration::from_secs(1), Duration::from_secs(1), || {
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
            warm: Cell::new(true),
            overruns: Cell::new(0),
            bridge: None,
            retained: None,
            layout_revision: 0,
            seen_commit: lifecycle::CommitTicket::new(0),
            layout_commit: lifecycle::CommitTicket::new(0),
            clicks: Vec::new(),
            retained_commit: lifecycle::CommitTicket::new(0),
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
            wire: Default::default(),
            animation_inventory: Default::default(),
            animation_visible: Default::default(),
            input_dirty: false,
            reads_host_context: true,
            input_rebuild: false,
            reveal_input: false,
            pending_inputs: None,
            metrics: Default::default(),
            animations: Default::default(),
            animation_surface: None,
            pending_animation: None,
            presenting_animation: None,
            pressed: None,
            hovered: None,
            focused_control: None,
            budget_warned: Default::default(),
            budget_warnings: Vec::new(),
            environment: serde_json::json!({"theme":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"safeArea":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":std::env::consts::OS,"formFactor":"phone","canDock":false,"displayLanguage":"en","pointer":"coarse"}),
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
        *self.wire.borrow_mut() = Default::default();
        self.animations.clear()?;
        self.pending_animation = None;
        self.presenting_animation = None;
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
        self.layout_commit = self.generation;
        self.retained_commit = self.generation;
        Ok(())
    }

    fn eval(&self, source: &str) -> Result<(), String> {
        self.execute(|| {
            self.context
                .eval::<()>(Source::from_bytes(source.as_bytes()))
                .map_err(|e| crate::engine::js_error(&self.context, e))
        })
    }

    /// A callback or other evaluation: one that finishes past its budget keeps its effects.
    fn execute<T>(&self, work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        self.evaluate(false, work)
    }

    /// One frame's View render: past its budget the frame is dropped (`UI_OVERRUN`), and
    /// the page is discarded after enough of those in a row.
    pub(super) fn execute_frame<T>(
        &self,
        work: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        self.evaluate(true, work)
    }

    fn evaluate<T>(
        &self,
        frame: bool,
        work: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        use crate::protocol::limits::{
            EVAL_LIMIT_MULTIPLIER, EVAL_OVERRUN_FRAMES, EVAL_WARMUP_MULTIPLIER,
        };
        if self.poisoned.get() {
            return Err("UI_TIMEOUT: discard the interrupted page before reusing it".into());
        }
        let budget = if frame && self.warm.get() {
            eval_budget() * EVAL_WARMUP_MULTIPLIER as u32
        } else {
            eval_budget()
        };
        let limit = eval_budget() * EVAL_LIMIT_MULTIPLIER as u32;
        let (result, overrun) = self.budget.run(budget, limit, || {
            let value = work()?;
            <crate::engine::Active as crate::engine::Engine>::drain_jobs(&self.context)?;
            Ok(value)
        });
        let result = match (frame, overrun, result) {
            (true, Some(overrun), Ok(_)) => {
                let run = self.overruns.get() + 1;
                self.overruns.set(run);
                if run as usize >= EVAL_OVERRUN_FRAMES {
                    // Each run completed, so no element is mid-render to name.
                    self.poisoned.set(true);
                    return Err(format!(
                        "UI_TIMEOUT: View JavaScript overran its budget on {run} frames in a row ({}); discard this page",
                        overrun.detail
                    ));
                } else {
                    Err(format!(
                        "{OVERRUN} View JavaScript overran its budget ({}); the frame is dropped",
                        overrun.detail
                    ))
                }
            }
            (true, None, Ok(value)) => {
                self.overruns.set(0);
                self.warm.set(false);
                Ok(value)
            }
            (_, _, result) => result,
        };
        if result.is_err() {
            <crate::engine::Active as crate::engine::Engine>::discard_jobs(&self.context);
        }
        if result
            .as_ref()
            .is_err_and(|error| error.starts_with("UI_TIMEOUT:"))
        {
            self.poisoned.set(true);
            // A render that ran out of time names the element it was building.
            let site = self
                .context
                .eval::<String>(Source::from_bytes(
                    "typeof __lingxiaSite === 'function' ? __lingxiaSite() : ''",
                ))
                .unwrap_or_default();
            if !site.is_empty() {
                return result.map_err(|error| format!("{error}\n  while rendering {site}"));
            }
        }
        result
    }
}
