use crate::protocol::field;
pub mod app;
pub mod host;
pub mod image_requests;
pub(crate) mod input_protocol;
pub(crate) mod js_tree;
pub mod lifecycle;
pub mod logic;
pub mod metrics;
pub(crate) mod probe;
pub mod state;

use crate::engine::{RongJS, decode_buffer};
use crate::{DisplayList, Frame, Renderer};
use rong::{JSEngine, Source};
use std::{cell::Cell, collections::HashMap, sync::Arc, time::Duration};
mod bridge;
mod chrome;
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
mod video;
mod webview;
use frame_animation::FrameAnimations;

/// Thread-confined View context. Candidates commit only after native validation and layout.
pub struct ViewSession {
    context: crate::engine::JSContext,
    _runtime: crate::engine::JSRuntime,
    renderer: Renderer,
    generation: lifecycle::CommitTicket,
    state: crate::session::state::StateReplica,
    budget: Arc<crate::engine::budget::Budget>,
    /// Kept so a hot-swapped module logs to the same place.
    #[cfg_attr(not(feature = "hot-swap"), allow(dead_code))]
    console: Option<crate::engine::console::ConsoleSink>,
    poisoned: Cell<bool>,
    /// The next frame is the module's first since it loaded or was swapped in: it also warms
    /// the engine, so it gets the warm-up budget.
    warm: Cell<bool>,
    /// Whether the last render was at the compact size class.
    compact: Option<bool>,
    /// Frames prepared since this isolate last collected garbage.
    uncollected: Cell<u32>,
    /// The page opted its dev clock into eval phases (`__lingxiaClock.phases = true`).
    eval_phases: bool,
    /// The module's cache key, under which its lowered styles persist for the next isolate.
    module: crate::engine::bytecode::Key,
    /// The first commit ran and its lowered styles are not yet offered to the cache.
    lowered_due: Cell<bool>,
    /// Lowered styles were offered once; later frames only add what the seed never serves.
    lowered_taken: Cell<bool>,
    /// Frames in a row that finished past their budget; each was dropped.
    overruns: Cell<u32>,
    environment: serde_json::Value,
    /// Connected to Logic: its page actions are configured.
    connected: bool,
    retained: Option<Arc<crate::scene::retained::RetainedLayout>>,
    /// Bumped whenever a frame commits a different retained layout.
    layout_revision: u64,
    /// The content size last reported to each `onLayout` element, by identity.
    boxes: HashMap<String, [f32; 2]>,
    /// Sizes that changed in the committed layout and wait for delivery.
    boxes_due: Vec<(String, [f32; 2])>,
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
    /// Offsets a reveal moved, held while the host's scroll revision stays the one the reveal
    /// was made under: a frame requested before the host saw the reveal carries the old offset
    /// and must not put the field back, while any scroll input since wins.
    revealed_offsets: HashMap<String, f32>,
    revealed_revision: Option<u64>,
    /// The host's scroll input revision for the frame being prepared, when it has one.
    offsets_revision: Option<u64>,
    lists: crate::layout::list_layout::Lists,
    images: HashMap<String, crate::scene::image::Bitmap>,
    image_sources: std::collections::HashSet<String>,
    image_status: HashMap<crate::ImageSource, bool>,
    image_notified: std::collections::HashSet<(String, crate::ImageSource)>,
    /// Each player's state as its views last heard it.
    video_reported: HashMap<String, crate::scene::video::Status>,
    inputs: crate::interaction::input_state::Inputs,
    input_records: Option<std::sync::Arc<[crate::protocol::Record]>>,
    /// The records `retained` was laid out from: `input_records` as painted, with input values
    /// and state styles applied.
    paint_records: Option<Vec<crate::protocol::Record>>,
    /// The last buffer the View encoded and the host decoded; the next may reference it.
    wire: std::cell::RefCell<crate::protocol::Wire>,
    /// The theme scales the page's steps resolve against; the wire carries the same.
    scales: std::sync::Arc<crate::protocol::scales::Scales>,
    input_dirty: bool,
    /// A press or hover changed paint only: the next frame repaints, while editor geometry
    /// and hit tests stay those of the presented frame.
    pointer_dirty: bool,
    /// The View's last run read a host-context key (`env.sizeClass`, `env.canDock`).
    reads_host_context: bool,
    /// The View depends on the appearance: its code read `env.theme` or `env.highContrast`, or a
    /// class has a theme condition. True until it has run.
    reads_appearance: bool,
    input_rebuild: bool,
    reveal_input: bool,
    /// The editor keyboard focus is starting, and whether it selects all its text on arrival.
    keyboard_entry: Option<(String, bool)>,
    pending_inputs: Option<crate::interaction::input_state::Inputs>,
    input_serial: u64,
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
    animation_visible: std::collections::BTreeSet<crate::scene::node::Identity>,
    /// Native pointer state selecting `active:`/`hover:` paint deltas.
    pressed: Option<String>,
    hovered: Option<String>,
    /// A non-editor control holding keyboard focus with a `focus:` paint delta (a `<select>`).
    focused_control: Option<String>,
    /// Limits this page has come near, each warned once; drained by the host.
    budget_warned: std::collections::BTreeSet<&'static str>,
    budget_warnings: Vec<String>,
    /// The host's bars, drawn under the top layer while the page is raised over them.
    chrome: chrome::ChromeSnapshots,
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

/// Prefix of a frame dropped for finishing past its budget; the page keeps running.
pub const OVERRUN: &str = "UI_OVERRUN:";
/// Prefix of a frame asked for before Logic sent the connection's first snapshot.
pub(crate) const NOT_READY: &str = "UI_STATE: waiting for first snapshot";
/// Prefix of a failure that was a caught panic in native code.
pub(crate) const PANIC: &str = "UI_RENDER: native panic";
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

/// Seed the runtime with lowerings an earlier isolate of this module persisted. Only a cache:
/// a seed the runtime refuses or fails on is dropped, never failing the page.
fn seed_lowered(context: &crate::engine::JSContext, module: &crate::engine::bytecode::Key) {
    let Some(text) = crate::engine::bytecode::load_lowered(module) else {
        return;
    };
    let accepted = serde_json::to_string(&text)
        .map_err(|e| e.to_string())
        .and_then(|literal| {
            context
                .eval::<bool>(Source::from_bytes(format!(
                    "typeof __lingxiaSeedLowered === 'function' && __lingxiaSeedLowered({literal})"
                )))
                .map_err(|e| crate::engine::js_error(context, e))
        });
    if !matches!(accepted, Ok(true)) {
        crate::engine::bytecode::forget_lowered(module);
    }
}

/// Frames of garbage a page builds before the host collects it between frames.
const FRAMES_PER_COLLECTION: u32 = 8;

impl ViewSession {
    /// Timings and counters of the last prepared frame.
    /// Collects garbage when enough frames have built up since the last collection; returns
    /// whether it did. The host calls it on the worker between frames.
    pub fn collect_between_frames(&self) -> bool {
        if self.lowered_due.replace(false) && !self.lowered_taken.replace(true) {
            self.persist_lowered();
        }
        if self.uncollected.get() < FRAMES_PER_COLLECTION {
            return false;
        }
        self.uncollected.set(0);
        crate::engine::collect_between_frames(&self.context);
        true
    }

    /// Hand styles lowered since the last take to the cache, so the next isolate starts with them.
    fn persist_lowered(&self) {
        // Not `execute`: draining the page's microtasks outside a frame would run its updates
        // where no frame follows them.
        if self.poisoned.get() {
            return;
        }
        let budget = eval_budget();
        let (taken, _) = self.budget.run(budget, budget, || {
            self.context
                .eval::<String>(Source::from_bytes(
                    "typeof __lingxiaTakeLowered === 'function' ? __lingxiaTakeLowered() : ''",
                ))
                .map_err(|e| crate::engine::js_error(&self.context, e))
        });
        match taken {
            Ok(text) if !text.is_empty() => {
                crate::engine::bytecode::store_lowered(self.module, text)
            }
            Ok(_) => {}
            Err(error) => {
                log::warn!(target: "lingxia_view", "BYTECODE_CACHE: cannot take lowered styles: {error}")
            }
        }
    }

    /// Hold a host bar's snapshot to draw under the top layer while it stays open; see
    /// [`crate::ViewHost::set_chrome_snapshot`].
    pub fn set_chrome_snapshot(
        &mut self,
        edge: crate::scene::display::ChromeEdge,
        generation: u64,
        rgba: &[u8],
        width: u32,
        height: u32,
        rect: [f32; 4],
    ) -> Result<(), String> {
        self.chrome.set(edge, generation, rgba, width, height, rect)
    }

    pub fn clear_chrome_snapshots(&mut self) {
        self.chrome.clear();
    }

    pub fn metrics(&self) -> &crate::FrameMetrics {
        &self.metrics
    }

    pub fn new(source: &str) -> Result<Self, String> {
        Self::new_with_logic(source, false)
    }

    pub fn new_with_logic(source: &str, logic: bool) -> Result<Self, String> {
        let runtime = RongJS::runtime();
        let budget = crate::engine::budget::Budget::new(&runtime);
        Self::with_runtime(runtime, budget, source, logic, None)
    }

    pub(crate) fn with_runtime(
        runtime: crate::engine::JSRuntime,
        budget: Arc<crate::engine::budget::Budget>,
        source: &str,
        logic: bool,
        console: Option<crate::engine::console::ConsoleSink>,
    ) -> Result<Self, String> {
        crate::engine::compatibility::validate(source)?;
        let context = runtime.context();
        let renderer = Renderer::default();
        let video_scope = renderer.videos.scope();
        let video_orientation = renderer.videos.orientation_writer();
        let (initialized, _) = budget.run(Duration::from_secs(1), Duration::from_secs(1), || {
            crate::engine::compatibility::install(&context)?;
            crate::engine::install_dev_globals(&context)?;
            crate::engine::console::install(&context, console.clone())?;
            video::install(&context, video_scope, video_orientation)?;
            crate::engine::defer_collections(&context);
            let module =
                <crate::engine::Active as crate::engine::Engine>::evaluate_module(&context, source)?;
            let valid: bool = context.eval(Source::from_bytes(
                "['Render','Prepare','Commit','Discard','Reset','Dispatch','CreateView'].every(name=>typeof globalThis['__lingxia'+name]==='function')"
            )).map_err(|e| e.to_string())?;
            if !valid {
                return Err("UI_MODULE: missing render or compiled UI runtime exports".into());
            }
            <crate::engine::Active as crate::engine::Engine>::drain_jobs(&context)?;
            seed_lowered(&context, &module);
            let phases = context
                .eval::<bool>(Source::from_bytes(
                    "globalThis.__lingxiaClock?.phases === true",
                ))
                .map_err(|e| e.to_string())?;
            Ok((module, phases))
        });
        let (module, eval_phases) = match initialized {
            Ok(initialized) => initialized,
            Err(error) => {
                <crate::engine::Active as crate::engine::Engine>::discard_jobs(&context);
                return Err(error);
            }
        };
        Ok(Self {
            context,
            _runtime: runtime,
            renderer,
            generation: lifecycle::CommitTicket::new(0),
            budget,
            console,
            poisoned: Cell::new(false),
            warm: Cell::new(true),
            compact: None,
            uncollected: Cell::new(0),
            eval_phases,
            module,
            lowered_due: Cell::new(false),
            lowered_taken: Cell::new(false),
            overruns: Cell::new(0),
            connected: false,
            retained: None,
            layout_revision: 0,
            boxes: HashMap::new(),
            boxes_due: Vec::new(),
            seen_commit: lifecycle::CommitTicket::new(0),
            layout_commit: lifecycle::CommitTicket::new(0),
            clicks: Vec::new(),
            retained_commit: lifecycle::CommitTicket::new(0),
            input_hits: None,
            viewport: None,
            scroll_offsets: HashMap::new(),
            revealed_offsets: HashMap::new(),
            revealed_revision: None,
            offsets_revision: None,
            lists: HashMap::new(),
            images: HashMap::new(),
            image_sources: Default::default(),
            image_status: Default::default(),
            image_notified: Default::default(),
            video_reported: Default::default(),
            inputs: Default::default(),
            input_records: None,
            paint_records: None,
            wire: Default::default(),
            scales: crate::protocol::scales::Scales::builtin(),
            animation_inventory: Default::default(),
            animation_visible: Default::default(),
            input_dirty: false,
            pointer_dirty: false,
            reads_host_context: true,
            reads_appearance: true,
            input_rebuild: false,
            reveal_input: false,
            keyboard_entry: None,
            pending_inputs: None,
            input_serial: 0,
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
            chrome: Default::default(),
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

    /// Counts the platform's focus, blur and edit requests; frames record the count they were
    /// built after.
    pub fn input_serial(&self) -> u64 {
        self.input_serial
    }

    fn advance(&mut self) {
        self.generation = lifecycle::CommitTicket::new(self.generation.get() + 1);
    }

    /// Keep module globals and replicated Logic data, but release local state and input handles.
    pub fn reset(&mut self) -> Result<(), String> {
        self.eval("__lingxiaReset()")?;
        *self.wire.borrow_mut() = Default::default();
        self.wire.borrow_mut().set_scales(self.scales.clone())?;
        self.animations.clear()?;
        self.pending_animation = None;
        self.presenting_animation = None;
        self.renderer.clear_text_cache();
        self.renderer.webviews.clear();
        self.retained = None;
        self.input_hits = None;
        self.viewport = None;
        self.scroll_offsets.clear();
        self.revealed_offsets.clear();
        self.images.clear();
        self.image_sources.clear();
        self.image_status.clear();
        self.image_notified.clear();
        self.lists.clear();
        self.inputs.clear();
        self.input_records = None;
        self.paint_records = None;
        self.animation_inventory = Default::default();
        self.animation_visible.clear();
        self.input_dirty = false;
        self.pointer_dirty = false;
        self.input_rebuild = false;
        self.reveal_input = false;
        self.pending_inputs = None;
        self.metrics = Default::default();
        self.advance();
        self.layout_commit = self.generation;
        self.retained_commit = self.generation;
        Ok(())
    }

    /// Opts this page's dev clock into eval phases, as a page can itself; needs dev globals.
    pub fn enable_eval_phases(&mut self) -> Result<(), String> {
        self.eval_phases = self.execute(|| {
            self.context
                .eval::<bool>(Source::from_bytes(
                    "!!globalThis.__lingxiaClock && (globalThis.__lingxiaClock.phases = true)",
                ))
                .map_err(|e| crate::engine::js_error(&self.context, e))
        })?;
        Ok(())
    }

    /// Eval phase totals since the last take; `None` without the runtime's phase export.
    pub(crate) fn take_eval_phases(
        &self,
    ) -> Result<Option<crate::session::metrics::ViewPhases>, String> {
        let spans: String = self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    "typeof __lingxiaEvalSpans === 'function' ? __lingxiaEvalSpans() : ''",
                ))
                .map_err(|e| crate::engine::js_error(&self.context, e))
        })?;
        if spans.is_empty() {
            return Ok(None);
        }
        serde_json::from_str(&spans)
            .map(Some)
            .map_err(|e| e.to_string())
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

    /// The error the frame's render threw itself, read under its own budget so a hung
    /// isolate yields nothing.
    fn recorded_throw(&self) -> Option<String> {
        let budget = eval_budget();
        let (thrown, _) = self.budget.run(budget, budget, || {
            self.context
                .eval::<String>(Source::from_bytes(
                    "typeof __lingxiaThrown === 'function' ? __lingxiaThrown() : ''",
                ))
                .map_err(|e| e.to_string())
        });
        thrown.ok().filter(|thrown| !thrown.is_empty())
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
            // A render that threw on its own and only then passed the limit is that throw.
            if frame && let Some(thrown) = self.recorded_throw() {
                return Err(thrown);
            }
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
