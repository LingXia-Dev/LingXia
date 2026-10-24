use crate::{DisplayList, Frame, Renderer, decode_context_timed};
use rong::{JSEngine, RongJS, Source};
use std::{cell::Cell, collections::HashMap, sync::Arc, time::Duration};
#[cfg(test)]
#[path = "flex_direction_tests.rs"]
mod flex_direction_tests;
#[path = "session_frame_animation.rs"]
mod frame_animation;
#[path = "session_images.rs"]
mod image_lifecycle;
use frame_animation::FrameAnimations;

/// Thread-confined View context. Candidates commit only after native validation and layout.
pub struct ViewSession {
    context: rong::JSContext,
    _runtime: rong::JSRuntime,
    renderer: Renderer,
    generation: u64,
    state: crate::state::StateReplica,
    budget: Arc<crate::budget::Budget>,
    poisoned: Cell<bool>,
    environment: serde_json::Value,
    bridge: Option<ViewBridge>,
    retained: Option<Arc<crate::render::RetainedLayout>>,
    input_hits: Option<crate::render::InputHitMap>,
    viewport: Option<(u16, u16, f32)>,
    scroll_offsets: HashMap<String, f32>,
    lists: crate::list_layout::Lists,
    images: HashMap<String, crate::image::Bitmap>,
    image_sources: std::collections::HashSet<String>,
    image_status: HashMap<crate::ImageSource, bool>,
    image_notified: std::collections::HashSet<(String, crate::ImageSource)>,
    inputs: crate::input_state::Inputs,
    input_records: Option<Vec<crate::schema::Record>>,
    input_dirty: bool,
    input_rebuild: bool,
    reveal_input: bool,
    pending_inputs: Option<crate::input_state::Inputs>,
    metrics: crate::FrameMetrics,
    animations: crate::animation::Timeline,
    animation_surface: Option<u64>,
    pending_animation: Option<(u64, u64, crate::animation::Prepared)>,
    animation_inventory: crate::animation::records::Inventory,
    animation_visible: std::collections::BTreeSet<String>,
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
        let budget = crate::budget::Budget::new(&runtime);
        Self::with_runtime(runtime, budget, source, logic)
    }

    pub(crate) fn with_runtime(
        runtime: rong::JSRuntime,
        budget: Arc<crate::budget::Budget>,
        source: &str,
        logic: bool,
    ) -> Result<Self, String> {
        crate::compatibility::validate(source)?;
        let context = runtime.context();
        let initialized = budget.run(Duration::from_secs(1), || {
            crate::compatibility::install(&context)?;
            crate::module::evaluate(&context, source)?;
            let valid: bool = context.eval(Source::from_bytes(
                "['Render','Prepare','Commit','Discard','Reset','Dispatch','CreateView'].every(name=>typeof globalThis['__lingxia'+name]==='function')"
            )).map_err(|e| e.to_string())?;
            if !valid {
                return Err("UI_MODULE: missing render or compiled UI runtime exports".into());
            }
            crate::module::drain_jobs(&context)
        });
        if let Err(error) = initialized {
            crate::module::discard_jobs(&context);
            return Err(error);
        }
        Ok(Self {
            context,
            _runtime: runtime,
            renderer: Renderer::default(),
            generation: 0,
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
            environment: serde_json::json!({"theme":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":std::env::consts::OS,"formFactor":"phone","aside":false,"displayLanguage":"en","runner":true}),
            state: if logic {
                crate::state::StateReplica::default()
            } else {
                crate::state::StateReplica::static_page()
            },
        })
    }

    pub fn image_sources(&self) -> Vec<String> {
        self.image_sources.iter().cloned().collect()
    }
    pub fn load_image(&mut self, src: &str, bytes: &[u8]) -> Result<bool, String> {
        if !self.image_sources.contains(src) {
            return Ok(false);
        }
        let bitmap = crate::image::Bitmap::decode(bytes)?;
        self.bind_image(src, bitmap)
    }
    pub(crate) fn bind_image(
        &mut self,
        src: &str,
        bitmap: crate::image::Bitmap,
    ) -> Result<bool, String> {
        if !self.image_sources.contains(src) {
            return Ok(false);
        }
        let retained_bytes: usize = self
            .images
            .iter()
            .filter(|(key, _)| key.as_str() != src)
            .map(|(_, value)| value.byte_len())
            .sum();
        if retained_bytes + bitmap.byte_len() > 64 * 1024 * 1024 {
            return Err("UI_IMAGE: page image budget exceeded".into());
        }
        self.images.insert(src.to_owned(), bitmap);
        Ok(true)
    }
    fn commit_images(&mut self, wanted: std::collections::HashSet<String>) {
        self.images.retain(|src, _| wanted.contains(src));
        self.image_sources = wanted;
        self.retain_image_lifecycle();
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Native measurements are sampled with the frame; viewport stays renderer-owned.
    pub fn set_environment(&mut self, value: serde_json::Value) -> Result<(), String> {
        let valid = value.is_object()
            && matches!(value["theme"].as_str(), Some("light" | "dark"))
            && ["top", "right", "bottom", "left"].iter().all(|edge| {
                value["insets"][edge]
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && n >= 0.0 && n <= 16384.0)
            })
            && value["keyboard"]["height"]
                .as_f64()
                .is_some_and(|n| n.is_finite() && n >= 0.0 && n <= 16384.0)
            && ["os", "formFactor", "displayLanguage"].iter().all(|key| {
                value[key]
                    .as_str()
                    .is_some_and(|s| !s.is_empty() && s.len() <= 128)
            })
            && (value.get("sizeClass").is_none()
                || matches!(value["sizeClass"].as_str(), Some("compact" | "regular")))
            && value["aside"].is_boolean()
            && value["runner"].is_boolean();
        if !valid {
            return Err("UI_ENVIRONMENT: invalid native snapshot".into());
        }
        let text_scale = crate::text_scale::TextScale::decode(value.get("textScale"))?;
        if self.renderer.text_scale != text_scale {
            self.renderer.text_scale = text_scale;
            self.retained = None;
            crate::list_layout::invalidate_measurements(&mut self.lists);
            self.reveal_input = true;
        }
        self.reveal_input |= self.environment["insets"] != value["insets"]
            || self.environment["keyboard"] != value["keyboard"];
        self.environment = value;
        Ok(())
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
        self.generation += 1;
        Ok(())
    }

    pub fn receive_state(
        &mut self,
        frame: &serde_json::Value,
    ) -> Result<crate::state::StateUpdate, String> {
        self.state.receive(frame)
    }

    /// Called by native after attaching this context to its owning page endpoint.
    pub fn begin_bridge(&mut self, nonce: &str) -> Result<serde_json::Value, String> {
        if nonce.is_empty() || nonce.len() > 256 {
            return Err("UI_BRIDGE: invalid nonce".into());
        }
        self.reset()?;
        self.state = crate::state::StateReplica::default();
        self.bridge = Some(ViewBridge {
            nonce: nonce.into(),
            session: None,
            ready: false,
        });
        Ok(
            serde_json::json!({"v":2,"kind":"hello","nonce":nonce,"role":"view","protocolsSupported":[2]}),
        )
    }

    pub fn ready_to_render(&self) -> bool {
        self.state.data().is_some() && self.bridge.as_ref().is_none_or(|bridge| bridge.ready)
    }

    pub fn receive_bridge(
        &mut self,
        frame: &serde_json::Value,
    ) -> Result<crate::state::StateUpdate, String> {
        if frame["v"] != 2 {
            return Err("UI_BRIDGE: expected LegacyV2".into());
        }
        let bridge = self
            .bridge
            .as_ref()
            .ok_or("UI_BRIDGE: context is not connected")?;
        let kind = frame["kind"].as_str().ok_or("UI_BRIDGE: missing kind")?;
        match kind {
            "helloAck" => {
                let session = frame["sessionId"]
                    .as_str()
                    .filter(|s| !s.is_empty() && s.len() <= 256)
                    .ok_or("UI_BRIDGE: invalid session")?;
                if bridge.session.is_some()
                    || frame["nonce"] != bridge.nonce
                    || frame["protocol"] != 2
                {
                    return Err("UI_BRIDGE: invalid hello acknowledgement".into());
                }
                self.bridge.as_mut().unwrap().session = Some(session.into());
                Ok(crate::state::StateUpdate::default())
            }
            "ready" => {
                if bridge
                    .session
                    .as_deref()
                    .is_none_or(|id| frame["sessionId"] != id)
                    || bridge.ready
                {
                    return Err("UI_BRIDGE: invalid ready".into());
                }
                self.eval("__lingxiaConfigureActions(globalThis.__lingxiaActionCatalog)")?;
                self.bridge.as_mut().unwrap().ready = true;
                Ok(crate::state::StateUpdate {
                    changed: self.ready_to_render(),
                    outbound: Vec::new(),
                })
            }
            _ if bridge.session.is_none() || frame.get("sessionId").is_some() => {
                Err("UI_BRIDGE: frame outside acknowledged LegacyV2 connection".into())
            }
            "state.snapshot" | "state.patch" => {
                let mut update = self.receive_state(frame)?;
                update.changed &= self.ready_to_render();
                Ok(update)
            }
            "res"
                if frame["id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with("ui-state-")) =>
            {
                let mut update = self.receive_state(frame)?;
                update.changed &= self.ready_to_render();
                Ok(update)
            }
            "res" | "event" | "ch.ack" | "ch.data" | "ch.close" => {
                let changed = self.receive_action_response(frame)?;
                let outbound = self
                    .take_action_requests()?
                    .iter()
                    .map(|frame| serde_json::from_str(frame).map_err(|e| e.to_string()))
                    .collect::<Result<_, _>>()?;
                Ok(crate::state::StateUpdate {
                    changed: changed && self.ready_to_render(),
                    outbound,
                })
            }
            _ => Err("UI_BRIDGE: unsupported frame".into()),
        }
    }

    pub fn configure_actions(&self, metadata: &serde_json::Value) -> Result<(), String> {
        if !cfg!(any(
            target_os = "android",
            target_os = "windows",
            all(target_os = "linux", not(target_env = "ohos"))
        )) {
            return Err("UI_ACTION: this engine has no View microtask adapter".into());
        }
        let json = serde_json::to_string(&metadata.to_string()).map_err(|e| e.to_string())?;
        self.eval(&format!("__lingxiaConfigureActions(JSON.parse({json}))"))
    }

    pub fn request_leave(&self, reason: &str) -> Result<(), String> {
        if !matches!(reason, "back" | "home") {
            return Err("UI_LEAVE: invalid reason".into());
        }
        let encoded = serde_json::to_string(reason).map_err(|e| e.to_string())?;
        self.eval(&format!("__lingxiaRequestLeave({encoded})"))
    }

    pub fn has_local_updates(&self) -> Result<bool, String> {
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    "typeof __lingxiaHasLocalUpdates === 'function' && __lingxiaHasLocalUpdates()",
                ))
                .map_err(|e| e.to_string())
        })
    }

    pub fn take_action_requests(&self) -> Result<Vec<String>, String> {
        self.execute(|| {
            self.context
                .eval(Source::from_bytes("__lingxiaTakeActionRequests()"))
                .map_err(|e| e.to_string())
        })
    }

    pub fn take_action_errors(&self) -> Result<Vec<String>, String> {
        self.execute(|| {
            self.context
                .eval(Source::from_bytes("__lingxiaTakeActionErrors()"))
                .map_err(|e| e.to_string())
        })
    }

    pub fn receive_action_response(&self, frame: &serde_json::Value) -> Result<bool, String> {
        let json = serde_json::to_string(&frame.to_string()).map_err(|e| e.to_string())?;
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!("__lingxiaReceiveActionResponse(JSON.parse({json}))").as_bytes(),
                ))
                .map_err(|e| e.to_string())
        })
    }

    pub fn render(&mut self, width: u16, height: u16, scale: f32) -> Result<Frame, String> {
        self.prepare(width, height, scale, DisplayList::rasterize)
    }

    pub fn layout(&mut self, width: u16, height: u16, scale: f32) -> Result<DisplayList, String> {
        self.prepare(width, height, scale, Ok)
    }

    pub(crate) fn prepare<T>(
        &mut self,
        width: u16,
        height: u16,
        scale: f32,
        build: impl FnOnce(DisplayList) -> Result<T, String>,
    ) -> Result<T, String> {
        self.prepare_native(width, height, scale, true, None, build)
    }

    /// Native scroll frames reuse the last committed description and never invoke guest JS.
    pub(crate) fn prepare_native<T>(
        &mut self,
        width: u16,
        height: u16,
        scale: f32,
        rebuild: bool,
        offsets: Option<&HashMap<String, f32>>,
        build: impl FnOnce(DisplayList) -> Result<T, String>,
    ) -> Result<T, String> {
        self.prepare_native_visuals(
            width,
            height,
            scale,
            rebuild,
            offsets,
            &mut FrameAnimations::Disabled,
            build,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_native_visuals<T>(
        &mut self,
        width: u16,
        height: u16,
        scale: f32,
        rebuild: bool,
        offsets: Option<&HashMap<String, f32>>,
        animation: &mut FrameAnimations<'_>,
        build: impl FnOnce(DisplayList) -> Result<T, String>,
    ) -> Result<T, String> {
        let started = std::time::Instant::now();
        let mut metrics = crate::FrameMetrics::default();
        let result = self.prepare_measured(
            width,
            height,
            scale,
            rebuild,
            offsets,
            animation,
            build,
            &mut metrics,
        )?;
        metrics.total_us = crate::metrics::micros(started);
        metrics.generation = self.generation;
        self.metrics = metrics;
        Ok(result)
    }

    pub fn frame_metrics(&self) -> &crate::FrameMetrics {
        &self.metrics
    }

    /// Supply a fresh surface epoch after replacement; `None` parks the page.
    pub fn set_animation_surface(
        &mut self,
        surface: Option<u64>,
        now: Duration,
    ) -> Result<(), String> {
        let mut candidate = self.animations.fork();
        if surface != self.animation_surface {
            candidate.set_visible(false, now)?;
        }
        candidate.set_visible(surface.is_some(), now)?;
        self.animations = candidate;
        if surface != self.animation_surface {
            self.pending_animation = None;
        }
        self.animation_surface = surface;
        Ok(())
    }

    /// Prepare transformed pixels, hit geometry and animation state together.
    /// Only `animation_presented` starts newly inserted effects.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_animated<'a, T>(
        &mut self,
        width: u16,
        height: u16,
        scale: f32,
        rebuild: bool,
        offsets: Option<&HashMap<String, f32>>,
        now: Duration,
        definitions: impl IntoIterator<Item = (String, crate::animation::Definition)>,
        visible: impl IntoIterator<Item = &'a str>,
        build: impl FnOnce(DisplayList) -> Result<T, String>,
    ) -> Result<T, String> {
        let surface = self.animation_surface.ok_or("UI_ANIMATION: parked page")?;
        let mut timeline = self.animations.fork();
        let animation = timeline.prepare_visible(now, definitions, visible)?;
        let frame = self.prepare_native_visuals(
            width,
            height,
            scale,
            rebuild,
            offsets,
            &mut FrameAnimations::Explicit(&animation),
            build,
        )?;
        self.animations = timeline;
        self.pending_animation = Some((self.generation, surface, animation));
        Ok(frame)
    }

    /// Collect authored effects and virtual-row lifetimes from the measured View.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_view_animated<T>(
        &mut self,
        width: u16,
        height: u16,
        scale: f32,
        rebuild: bool,
        offsets: Option<&HashMap<String, f32>>,
        now: Duration,
        build: impl FnOnce(DisplayList) -> Result<T, String>,
    ) -> Result<T, String> {
        let surface = self.animation_surface.ok_or("UI_ANIMATION: parked page")?;
        let mut animation = FrameAnimations::Collected {
            inventory: self.animation_inventory.clone(),
            timeline: self.animations.fork(),
            now,
            frame: None,
            visible: Default::default(),
        };
        let result = self.prepare_native_visuals(
            width,
            height,
            scale,
            rebuild,
            offsets,
            &mut animation,
            build,
        )?;
        let FrameAnimations::Collected {
            inventory,
            timeline,
            frame,
            visible,
            ..
        } = animation
        else {
            unreachable!()
        };
        self.animation_inventory = inventory;
        self.animations = timeline;
        self.animation_visible = visible;
        self.pending_animation = frame.map(|frame| (self.generation, surface, *frame));
        Ok(result)
    }

    pub fn view_animation_wakeup(&self, now: Duration) -> Result<crate::animation::Wakeup, String> {
        self.animations
            .next_wakeup(now, self.animation_visible.iter().map(String::as_str))
    }

    /// Acknowledge on the owning View thread after native presentation succeeds.
    /// Stale acknowledgements leave a newer candidate intact.
    pub fn animation_presented(
        &mut self,
        surface: u64,
        generation: u64,
        now: Duration,
    ) -> Result<bool, String> {
        if self.animation_surface != Some(surface)
            || self.generation != generation
            || !self
                .pending_animation
                .as_ref()
                .is_some_and(|(g, s, _)| *g == generation && *s == surface)
        {
            return Ok(false);
        }
        self.animations
            .validate_presentation(&self.pending_animation.as_ref().unwrap().2, now)?;
        let (_, _, frame) = self.pending_animation.take().unwrap();
        self.animations.presented(frame, now)?;
        Ok(true)
    }

    pub fn animation_wakeup<'a>(
        &self,
        now: Duration,
        visible: impl IntoIterator<Item = &'a str>,
    ) -> Result<crate::animation::Wakeup, String> {
        self.animations.next_wakeup(now, visible)
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_measured<T>(
        &mut self,
        width: u16,
        height: u16,
        scale: f32,
        rebuild: bool,
        offsets: Option<&HashMap<String, f32>>,
        animation: &mut FrameAnimations<'_>,
        build: impl FnOnce(DisplayList) -> Result<T, String>,
        metrics: &mut crate::FrameMetrics,
    ) -> Result<T, String> {
        let reveal_input = self.reveal_input
            || self
                .viewport
                .is_some_and(|viewport| viewport != (width, height, scale));
        let rebuild = rebuild || self.input_rebuild || self.reveal_input;
        let native_input = !rebuild
            && self.input_dirty
            && self.input_records.is_some()
            && self.viewport == Some((width, height, scale));
        let mut offsets = offsets.unwrap_or(&self.scroll_offsets).clone();
        if offsets.len() > 4096 || offsets.values().any(|offset| !offset.is_finite()) {
            return Err("UI_SCROLL: invalid offsets".into());
        }
        if !self.ready_to_render() {
            return Err("UI_STATE: waiting for first snapshot or bridge ready".into());
        }
        if width == 0
            || height == 0
            || width > 4096
            || height > 4096
            || !scale.is_finite()
            || scale <= 0.0
        {
            return Err("UI_VIEWPORT: invalid dimensions".into());
        }
        let mut window_only = None;
        if !rebuild && !self.input_dirty && self.viewport == Some((width, height, scale)) {
            if let Some(retained) = &self.retained {
                if let Some(records) = &self.input_records {
                    animation.validate(records)?;
                }
                let started = std::time::Instant::now();
                let mut display = retained.compose(&offsets)?;
                metrics.compose_us += crate::metrics::micros(started);
                window_only = crate::list_layout::scroll_windows(&display, &self.lists)?;
                if window_only.is_none() {
                    let started = std::time::Instant::now();
                    display = animation
                        .compose(
                            self.input_records.as_deref().unwrap_or_default(),
                            display,
                            &offsets,
                        )?
                        .0;
                    metrics.compose_us += crate::metrics::micros(started);
                    let applied = applied_offsets(&display);
                    let wanted = display.image_sources().map(str::to_owned).collect();
                    display.bind_images(&self.images)?;
                    let mut inputs = self.inputs.clone();
                    let old_focus = inputs.focused().map(str::to_owned);
                    inputs.decorate(&mut display, &mut self.renderer)?;
                    let changed_style = old_focus.as_deref() != inputs.focused()
                        && self
                            .input_records
                            .as_ref()
                            .is_some_and(|records| records.iter().any(|r| r.string(98).is_some()));
                    if !changed_style {
                        let input_dirty = inputs.needs_layout(&display);
                        let input_hits = crate::render::InputHitMap::capture(&display);
                        metrics.nodes = display.nodes.len();
                        metrics.paints = display.paint_count();
                        let started = std::time::Instant::now();
                        let frame = build(display)?;
                        metrics.build_us += crate::metrics::micros(started);
                        self.input_hits = Some(input_hits);
                        self.inputs = inputs;
                        self.input_dirty = input_dirty;
                        self.commit_images(wanted);
                        self.scroll_offsets = applied;
                        self.generation += 1;
                        return Ok(frame);
                    }
                }
            }
        }
        let windows = window_only
            .as_ref()
            .cloned()
            .unwrap_or_else(|| crate::list_layout::current(&self.lists));
        let encoded_windows =
            serde_json::to_string(&serde_json::to_string(&windows).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let invoke = if native_input {
            String::new()
        } else if window_only.is_some() {
            format!("__lingxiaPrepareWindows(JSON.parse({encoded_windows}))")
        } else {
            let width_dp = width as f32 / scale;
            let height_dp = height as f32 / scale;
            let data = self
                .state
                .data()
                .ok_or("UI_STATE: waiting for first snapshot")?;
            let data = serde_json::to_string(&data.to_string()).map_err(|e| e.to_string())?;
            let mut environment = self.environment.clone();
            // Platform interpolation samples are an internal layout contract.
            environment.as_object_mut().unwrap().remove("textScale");
            environment["viewport"] = serde_json::json!({"width":width_dp,"height":height_dp});
            if environment.get("sizeClass").is_none() {
                environment["sizeClass"] = serde_json::json!(if width_dp < 600.0 {
                    "compact"
                } else {
                    "regular"
                });
            }
            let environment =
                serde_json::to_string(&environment.to_string()).map_err(|e| e.to_string())?;
            format!(
                "__lingxiaPrepare(__lingxiaRender, __lingxiaCreateView(JSON.parse({data}), JSON.parse({environment})), JSON.parse({encoded_windows}))"
            )
        };
        let mut prepared_js = !native_input;
        let candidate = (|| {
            let mut records = if native_input {
                self.input_records.as_ref().unwrap().clone()
            } else {
                let started = std::time::Instant::now();
                let (records, timings) =
                    self.execute(|| decode_context_timed(&self.context, &invoke))?;
                metrics.add_view(timings);
                metrics.view_us += crate::metrics::micros(started);
                metrics.view_passes += 1;
                records
            };
            let mut inputs = self.inputs.clone();
            let mut lists = self.lists.clone();
            for _ in 0..8 {
                animation.validate(&records)?;
                inputs.reconcile(&records, &mut self.renderer)?;
                let mut render_records = records.clone();
                strip_record_animations(&mut render_records);
                inputs.apply_values(&mut render_records);
                inputs.apply_styles(&mut render_records)?;
                let started = std::time::Instant::now();
                let reusable = if !self.input_dirty && self.viewport == Some((width, height, scale))
                {
                    self.retained
                        .as_ref()
                        .zip(self.input_records.as_ref())
                        .and_then(|(retained, previous)| {
                            let mut previous = previous.clone();
                            strip_record_animations(&mut previous);
                            self.inputs.apply_values(&mut previous);
                            self.inputs.apply_styles(&mut previous).ok()?;
                            retained.rebind(&previous, &render_records)
                        })
                } else {
                    None
                };
                let mut display = if let Some(retained) = reusable {
                    let display = retained.compose(&offsets)?;
                    metrics.compose_us += crate::metrics::micros(started);
                    display
                } else {
                    let display = self.renderer.layout_scrolled(
                        &render_records,
                        width,
                        height,
                        scale,
                        &offsets,
                    )?;
                    metrics.layout_us += crate::metrics::micros(started);
                    metrics.text_us += self.renderer.text_time.as_micros() as u64;
                    metrics.layout_passes += 1;
                    display
                };
                if reveal_input && let Some(identity) = inputs.focused() {
                    let started = std::time::Instant::now();
                    display = display.reveal(identity, &mut offsets)?;
                    metrics.compose_us += crate::metrics::micros(started);
                }
                let old_focus = inputs.focused().map(str::to_owned);
                inputs.decorate(&mut display, &mut self.renderer)?;
                if old_focus.as_deref() != inputs.focused()
                    && records.iter().any(|r| r.string(98).is_some())
                {
                    continue;
                }
                if let Some(windows) =
                    crate::list_layout::reconcile(&records, &display, &mut lists, &mut offsets)?
                {
                    let encoded = serde_json::to_string(
                        &serde_json::to_string(&windows).map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?;
                    prepared_js = true;
                    let started = std::time::Instant::now();
                    let (refined, timings) = self.execute(|| {
                        decode_context_timed(
                            &self.context,
                            &format!("__lingxiaPrepareWindows(JSON.parse({encoded}))"),
                        )
                    })?;
                    records = refined;
                    metrics.add_view(timings);
                    metrics.view_us += crate::metrics::micros(started);
                    metrics.view_passes += 1;
                } else {
                    let applied = applied_offsets(&display);
                    let retained = Arc::clone(&display.retained);
                    let started = std::time::Instant::now();
                    let (composed, redecorate) = animation.compose(&records, display, &applied)?;
                    display = composed;
                    metrics.compose_us += crate::metrics::micros(started);
                    if redecorate {
                        inputs.decorate(&mut display, &mut self.renderer)?;
                    }
                    crate::list_layout::commit(&mut lists);
                    let wanted = display.image_sources().map(str::to_owned).collect();
                    display.bind_images(&self.images)?;
                    let input_dirty = inputs.needs_layout(&display);
                    let input_hits = crate::render::InputHitMap::capture(&display);
                    metrics.nodes = display.nodes.len();
                    metrics.paints = display.paint_count();
                    let started = std::time::Instant::now();
                    let frame = build(display)?;
                    metrics.build_us += crate::metrics::micros(started);
                    return Ok({
                        (
                            frame,
                            retained,
                            applied,
                            lists,
                            wanted,
                            inputs,
                            records,
                            input_dirty,
                            input_hits,
                        )
                    });
                }
            }
            Err("UI_LIST: window measurement did not converge".into())
        })();
        let candidate = candidate.and_then(|candidate| {
            if prepared_js {
                let started = std::time::Instant::now();
                self.eval("__lingxiaCommit()")?;
                metrics.view_us += crate::metrics::micros(started);
            }
            Ok(candidate)
        });
        match candidate {
            Ok((
                frame,
                retained,
                applied,
                lists,
                wanted,
                inputs,
                records,
                input_dirty,
                input_hits,
            )) => {
                self.input_hits = Some(input_hits);
                self.inputs = inputs;
                self.input_records = Some(records);
                self.input_dirty = input_dirty;
                self.input_rebuild = false;
                self.reveal_input = false;
                self.pending_inputs = None;
                self.commit_images(wanted);
                self.retained = Some(retained);
                self.lists = lists;
                self.viewport = Some((width, height, scale));
                self.scroll_offsets = applied;
                self.generation += 1;
                Ok(frame)
            }
            Err(error) => {
                if prepared_js {
                    let _ = self.eval("__lingxiaDiscard()");
                }
                if let Some(mut previous) = self.pending_inputs.take() {
                    previous.invalidate_after(&self.inputs)?;
                    self.inputs = previous;
                    self.input_dirty = true;
                    self.input_rebuild = false;
                }
                Err(error)
            }
        }
    }

    /// Ignore input from frames superseded while the event was queued.
    pub fn hit_test_input(
        &self,
        generation: u64,
        identity: &str,
        point: [f32; 2],
        word: bool,
    ) -> Result<Option<[u32; 2]>, String> {
        self.check_input_active()?;
        if generation != self.generation || self.input_dirty || self.pending_inputs.is_some() {
            return Ok(None);
        }
        Ok(self
            .input_hits
            .as_ref()
            .and_then(|hits| hits.hit(identity, point, word)))
    }

    /// Ignore input from frames superseded while the event was queued.
    pub fn drag_hit_test_input(
        &self,
        generation: u64,
        identity: &str,
        token: u64,
        point: [f32; 2],
        edge: [i8; 2],
    ) -> Result<Option<[u32; 2]>, String> {
        self.check_input_active()?;
        if !self.inputs.valid(identity, token)
            || generation != self.generation
            || self.input_dirty
            || self.pending_inputs.is_some()
        {
            return Ok(None);
        }
        Ok(self
            .input_hits
            .as_ref()
            .and_then(|hits| hits.drag_hit(identity, point, edge)))
    }

    /// Ignore input from frames superseded while the event was queued.
    pub fn focus_input(
        &mut self,
        generation: u64,
        identity: &str,
    ) -> Result<Option<crate::InputReply>, String> {
        self.check_input_active()?;
        if generation != self.generation {
            return Ok(None);
        }
        let mut inputs = self.inputs.clone();
        if !inputs.focus(identity, &mut self.renderer)? {
            return Ok(None);
        }
        let reply = inputs.reply(identity, self.input_rebuild);
        if self.pending_inputs.is_none() {
            self.pending_inputs = Some(self.inputs.clone());
        }
        self.inputs = inputs;
        self.input_dirty = true;
        Ok(Some(reply))
    }

    /// A focus token survives presentation; it cannot target a later input connection.
    pub fn input_state(&self, token: u64, identity: &str) -> Option<crate::InputReply> {
        self.inputs
            .valid(identity, token)
            .then(|| self.inputs.reply(identity, self.input_rebuild))
    }

    /// A focus token survives presentation; it cannot target a later input connection.
    pub fn edit_input(
        &mut self,
        token: u64,
        identity: &str,
        edits: &[crate::text_input::Edit],
        submit: bool,
    ) -> Result<Option<crate::InputReply>, String> {
        self.check_input_active()?;
        if !self.inputs.valid(identity, token) {
            return Ok(None);
        }
        let mut inputs = self.inputs.clone();
        let mut edits = edits.to_vec();
        if submit {
            edits.push(crate::text_input::Edit::FinishComposition);
        }
        let committed = inputs.edit(identity, &edits, &mut self.renderer)?;
        let mut rebuild = self.input_rebuild;
        if let (Some(text), Some(handle)) = (committed, inputs.handler(identity, false)) {
            if !self.text_event(self.generation, handle, &text)? {
                return Err("UI_INPUT: unavailable input callback".into());
            }
            rebuild = true;
        }
        if submit && let Some(handle) = inputs.handler(identity, true) {
            let text = inputs.reply(identity, false).state.text;
            if !self.text_event(self.generation, handle, &text)? {
                return Err("UI_INPUT: unavailable submit callback".into());
            }
            rebuild = true;
        }
        let reply = inputs.reply(identity, rebuild);
        if self.pending_inputs.is_none() {
            self.pending_inputs = Some(self.inputs.clone());
        }
        self.inputs = inputs;
        self.input_dirty = true;
        self.input_rebuild = rebuild;
        Ok(Some(reply))
    }

    pub fn blur_input(&mut self, token: u64, identity: &str) -> Result<bool, String> {
        self.check_input_active()?;
        if !self.inputs.valid(identity, token) {
            return Ok(false);
        }
        let mut inputs = self.inputs.clone();
        inputs.blur(&mut self.renderer)?;
        if self.pending_inputs.is_none() {
            self.pending_inputs = Some(self.inputs.clone());
        }
        self.inputs = inputs;
        self.input_dirty = true;
        Ok(true)
    }

    fn check_input_active(&self) -> Result<(), String> {
        if self.poisoned.get() {
            Err("UI_TIMEOUT: discard the interrupted page before reusing it".into())
        } else {
            Ok(())
        }
    }

    /// Ignore input from frames superseded while the event was queued.
    pub fn click(&mut self, generation: u64, handle: u64) -> Result<bool, String> {
        if generation != self.generation || handle == 0 {
            return Ok(false);
        }
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!("__lingxiaDispatch({handle})").as_bytes(),
                ))
                .map_err(|e| e.to_string())
        })
    }

    /// Only text callbacks accept this payload; stale displayed-frame events are ignored.
    pub fn text_event(&mut self, generation: u64, handle: u64, text: &str) -> Result<bool, String> {
        if generation != self.generation || handle == 0 {
            return Ok(false);
        }
        if text.len() > crate::text_input::MAX_TEXT_BYTES {
            return Err("UI_INPUT: text too long".into());
        }
        let encoded = serde_json::to_string(text).map_err(|e| e.to_string())?;
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!("__lingxiaDispatch({handle},{encoded})").as_bytes(),
                ))
                .map_err(|e| e.to_string())
        })
    }

    /// Resolve and dispatch selection against committed geometry and callback identity.
    pub fn select_plot(
        &mut self,
        generation: u64,
        identity: &str,
        selection: crate::PlotSelection,
    ) -> Result<bool, String> {
        self.check_input_active()?;
        if generation != self.generation || self.input_dirty || self.pending_inputs.is_some() {
            return Ok(false);
        }
        let Some(node) = self
            .input_hits
            .as_ref()
            .and_then(|hits| hits.plot_node(identity))
        else {
            return Ok(false);
        };
        let plot = node.plot.as_ref().unwrap();
        let Some(handle) = plot.on_select.filter(|handle| *handle != 0) else {
            return Ok(false);
        };
        let index = match selection {
            crate::PlotSelection::Point(point) => {
                if point.iter().any(|value| !value.is_finite()) {
                    return Ok(false);
                }
                node.hit_test_plot(point).map_or(-1, |index| index as i64)
            }
            crate::PlotSelection::Next => plot.next_selection().map_or(-1, |index| index as i64),
            crate::PlotSelection::Index(index) => index,
        };
        if !(index == -1 && plot.selected_index >= 0)
            && !usize::try_from(index)
                .ok()
                .is_some_and(|index| plot.selectable(index))
        {
            return Ok(false);
        }
        self.number_event(generation, handle, index as f64)
    }

    /// Propose a finite scalar to a numeric control from the displayed generation.
    pub fn number_event(
        &mut self,
        generation: u64,
        handle: u64,
        value: f64,
    ) -> Result<bool, String> {
        if generation != self.generation || handle == 0 {
            return Ok(false);
        }
        if !value.is_finite() || value.abs() > 1e9 {
            return Err("UI_SLIDER: invalid proposed value".into());
        }
        let encoded = serde_json::to_string(&value).map_err(|e| e.to_string())?;
        self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    format!("__lingxiaDispatch({handle},{encoded})").as_bytes(),
                ))
                .map_err(|e| e.to_string())
        })
    }

    fn eval(&self, source: &str) -> Result<(), String> {
        self.execute(|| {
            self.context
                .eval::<()>(Source::from_bytes(source.as_bytes()))
                .map_err(|e| e.to_string())
        })
    }

    fn execute<T>(&self, work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        if self.poisoned.get() {
            return Err("UI_TIMEOUT: discard the interrupted page before reusing it".into());
        }
        let result = self.budget.run(Duration::from_millis(100), || {
            let value = work()?;
            crate::module::drain_jobs(&self.context)?;
            Ok(value)
        });
        if result.is_err() {
            crate::module::discard_jobs(&self.context);
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

fn strip_record_animations(records: &mut [crate::schema::Record]) {
    for record in records {
        record
            .fields
            .retain(|(field, _)| !matches!(field, 101 | 102));
    }
}

fn validate_record_animations(
    records: &[crate::schema::Record],
    frame: Option<&crate::animation::Prepared>,
) -> Result<(), String> {
    if !records.iter().any(|record| record.string(101).is_some()) {
        return Ok(());
    }
    let frame = frame.ok_or("UI_ANIMATION: page requires animation presentation scheduling")?;
    for (record, identity) in records.iter().zip(crate::schema::identities(records)) {
        if let Some(source) = record.string(101) {
            let definition = crate::animation::decode_definition(source)?;
            if !frame.contains_definition(&identity, &definition) {
                return Err("UI_ANIMATION: frame definition does not match View record".into());
            }
        }
    }
    Ok(())
}

fn applied_offsets(display: &DisplayList) -> HashMap<String, f32> {
    display
        .nodes
        .iter()
        .filter_map(|node| node.scroll.as_ref())
        .map(|scroll| (scroll.id.clone(), scroll.offset))
        .collect()
}

#[cfg(test)]
#[path = "session_animation_tests.rs"]
mod animation_tests;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn unchanged_geometry_rebinds_callbacks_and_failed_candidates_preserve_old_handles() {
        let module = source(
            "globalThis.rev=0;globalThis.observed=0;globalThis.size=16;function render(){const revision=++globalThis.rev;return h('button',{style:{fontSize:globalThis.size},onClick:()=>{globalThis.observed=revision}},'Stable label')}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let first = page.layout(240, 100, 1.0).unwrap();
        let old = first.nodes[0].on_click.unwrap();
        let old_gen = page.generation();
        let next = page.layout(240, 100, 1.0).unwrap();
        let current = next.nodes[0].on_click.unwrap();
        assert_ne!(old, current);
        assert_eq!(page.frame_metrics().view_passes, 1);
        assert_eq!(page.frame_metrics().layout_passes, 0);
        assert_eq!(first.nodes[0].bounds, next.nodes[0].bounds);
        assert!(!page.click(old_gen, old).unwrap());
        assert!(page.click(page.generation(), current).unwrap());
        page.eval("if(globalThis.observed!==2)throw Error('stale callback')")
            .unwrap();
        let generation = page.generation();
        let failed: Result<(), String> = page.prepare_native(240, 100, 1.0, true, None, |_| {
            Err("candidate failure".into())
        });
        assert!(failed.is_err());
        assert_eq!(page.generation(), generation);
        assert!(page.click(generation, current).unwrap());
        page.eval("if(globalThis.observed!==2)throw Error('failed candidate leaked')")
            .unwrap();
        page.eval("globalThis.size=24").unwrap();
        page.layout(240, 100, 1.0).unwrap();
        assert_eq!(page.frame_metrics().layout_passes, 1);
        page.layout(300, 100, 1.0).unwrap();
        assert_eq!(page.frame_metrics().layout_passes, 1);
    }

    #[test]
    fn unchanged_plots_rebind_selection_without_layout_or_leaking_failed_callbacks() {
        for kind in ["bar", "line", "pie"] {
            let module = source(&format!(
                "globalThis.rev=0;globalThis.observed='';globalThis.values=[10,20];globalThis.selected=-1;function render(){{const revision=++globalThis.rev;return h('plot',{{kind:'{kind}',values:globalThis.values,colors:['#123456'],accessibilityLabel:'Series',selectedIndex:globalThis.selected,onSelect:index=>{{globalThis.observed=revision+':'+index}},style:{{width:200,height:100}}}})}}"
            ));
            let mut page = ViewSession::new(&module).unwrap();
            let first = page.layout(240, 160, 1.0).unwrap();
            let old = first.nodes[0].plot.as_ref().unwrap().on_select.unwrap();
            let old_generation = page.generation();
            let next = page.layout(240, 160, 1.0).unwrap();
            let current = next.nodes[0].plot.as_ref().unwrap().on_select.unwrap();
            assert_ne!(old, current);
            assert_eq!(page.frame_metrics().layout_passes, 0, "{kind}");
            assert_eq!(
                first.rasterize().unwrap().pixels.data_as_u8_slice(),
                next.rasterize().unwrap().pixels.data_as_u8_slice()
            );
            assert!(!page.number_event(old_generation, old, 0.0).unwrap());
            assert!(page.number_event(page.generation(), current, 1.0).unwrap());
            page.eval("if(globalThis.observed!=='2:1')throw Error('stale selection')")
                .unwrap();
            let generation = page.generation();
            let failed: Result<(), String> = page.prepare_native(240, 160, 1.0, true, None, |_| {
                Err("candidate failure".into())
            });
            assert!(failed.is_err());
            assert_eq!(page.generation(), generation);
            assert!(page.number_event(generation, current, 0.0).unwrap());
            page.eval("if(globalThis.observed!=='2:0')throw Error('failed callback leaked')")
                .unwrap();
            for update in ["globalThis.selected=1", "globalThis.values=[20,10]"] {
                page.eval(update).unwrap();
                page.layout(240, 160, 1.0).unwrap();
                assert_eq!(page.frame_metrics().layout_passes, 1);
            }
        }
    }

    #[test]
    fn frame_metrics_distinguish_reuse_and_preserve_the_last_success() {
        let module =
            source("function render(){return h('column',{},h('text',{},'Measured frame'))}");
        let mut page = ViewSession::new(&module).unwrap();
        assert_eq!(page.frame_metrics(), &crate::FrameMetrics::default());
        let display = page.layout(300, 200, 1.0).unwrap();
        let measured = page.frame_metrics().clone();
        assert_eq!(measured.generation, page.generation());
        assert_eq!(measured.nodes, display.nodes.len());
        assert_eq!(measured.paints, display.paint_count());
        assert_eq!(measured.view_passes, 1);
        assert_eq!(measured.layout_passes, 1);
        assert_eq!(measured.wire_values, 12);
        assert!(
            measured.view_eval_us + measured.view_transfer_us + measured.view_decode_us
                <= measured.view_us
        );
        assert!(measured.text_us <= measured.layout_us);
        assert!(
            measured.view_us + measured.layout_us + measured.compose_us + measured.build_us
                <= measured.total_us
        );
        let failed: Result<(), String> =
            page.prepare_native(300, 200, 1.0, true, None, |_| Err("build failed".into()));
        assert!(failed.is_err());
        assert_eq!(page.frame_metrics(), &measured);
        page.prepare_native(300, 200, 1.0, false, None, Ok).unwrap();
        let reused = page.frame_metrics();
        assert_eq!(
            (
                reused.view_eval_us,
                reused.view_transfer_us,
                reused.view_decode_us,
                reused.wire_values
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(reused.generation, measured.generation + 1);
        assert_eq!(
            (
                reused.view_passes,
                reused.layout_passes,
                reused.view_us,
                reused.layout_us,
                reused.text_us
            ),
            (0, 0, 0, 0, 0)
        );
        page.reset().unwrap();
        assert_eq!(page.frame_metrics(), &crate::FrameMetrics::default());
    }

    #[test]
    fn list_windows_keep_root_render_and_offscreen_row_state() {
        let source = source(
            "globalThis.roots=0; globalThis.rows=0; function Item({i}){const n=useLocal('n',0);return h('button',{testId:'row-'+i,onClick:()=>n.set(n.value+1)},`${i}:${n.value}`)} function render(){globalThis.roots++;return h('list',{testId:'feed',count:1000,height:()=>100,row:i=>{globalThis.rows++;return h(Item,{key:String(i),i})}})}",
        );
        let mut page = ViewSession::new(&source).unwrap();
        let first = page.layout(200, 300, 1.0).unwrap();
        assert!(first.nodes.len() < 50);
        assert!(page.frame_metrics().view_passes >= 2);
        assert_eq!(
            page.frame_metrics().view_passes,
            page.frame_metrics().layout_passes
        );
        let button = first.nodes.iter().find(|n| n.test_id == "row-0").unwrap();
        assert!(
            page.click(page.generation(), button.on_click.unwrap())
                .unwrap()
        );
        let first = page.layout(200, 300, 1.0).unwrap();
        assert_eq!(
            first
                .nodes
                .iter()
                .find(|n| n.test_id == "row-0")
                .unwrap()
                .text,
            "0:1"
        );
        let id = first.nodes[0].scroll.as_ref().unwrap().id.clone();
        let middle = page
            .prepare_native(
                200,
                300,
                1.0,
                false,
                Some(&HashMap::from([(id.clone(), 20_000.0)])),
                Ok,
            )
            .unwrap();
        assert!(middle.nodes.len() < 60);
        assert!(page.frame_metrics().view_passes > 0);
        assert!(page.frame_metrics().layout_passes > 0);
        assert!(!middle.nodes.iter().any(|n| n.test_id == "row-0"));
        let roots: i32 = page
            .context
            .eval(Source::from_bytes(b"globalThis.roots"))
            .unwrap();
        assert_eq!(roots, 2);
        let back = page
            .prepare_native(200, 300, 1.0, false, Some(&HashMap::from([(id, 0.0)])), Ok)
            .unwrap();
        assert_eq!(
            back.nodes
                .iter()
                .find(|n| n.test_id == "row-0")
                .unwrap()
                .text,
            "0:1"
        );
        let rows: i32 = page
            .context
            .eval(Source::from_bytes(b"globalThis.rows"))
            .unwrap();
        assert!(rows < 60, "constructed {rows} of 1000 rows");
    }

    #[test]
    fn nested_list_end_anchor_survives_sibling_removal() {
        let module = source(
            "globalThis.extra=true;function render(){return h('column',{style:{padding:12,gap:8}},globalThis.extra?h('text',{},'sibling'):null,h('list',{testId:'list',count:100,anchor:'end',style:{height:100,paddingTop:7,paddingBottom:19,paddingLeft:23,paddingRight:11,borderWidth:3},row:i=>h('text',{testId:'row-'+i,style:{height:40}},String(i))}))}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        for extra in [true, true, false, false] {
            page.eval(&format!("globalThis.extra={extra}")).unwrap();
            let frame = page.layout(393, 800, 1.0).unwrap();
            let node = frame.nodes.iter().find(|n| n.test_id == "list").unwrap();
            let scroll = node.scroll.as_ref().unwrap();
            assert!(
                (scroll.offset - scroll.max_offset).abs() < 0.1,
                "extra={extra}: {} / {}",
                scroll.offset,
                scroll.max_offset
            );
        }
    }

    #[test]
    fn list_end_anchors_use_resolved_edges_and_borders() {
        for style in [
            "padding:12",
            "paddingTop:12,paddingRight:12,paddingBottom:12,paddingLeft:12",
            "paddingTop:7,paddingBottom:19,paddingLeft:23,paddingRight:11,borderWidth:3",
        ] {
            let module = source(&format!(
                "function render(){{return h('list',{{count:100,anchor:'end',style:{{{style}}},row:i=>h('text',{{testId:'row-'+i,style:{{height:40}}}},String(i))}})}}"
            ));
            let mut page = ViewSession::new(&module).unwrap();
            let frame = page.layout(200, 300, 1.0).unwrap();
            let scroll = frame.nodes[0].scroll.as_ref().unwrap();
            assert!(
                (scroll.offset - scroll.max_offset).abs() < 0.1,
                "{style}: {} / {}",
                scroll.offset,
                scroll.max_offset
            );
            let last = frame.nodes.iter().find(|n| n.test_id == "row-99").unwrap();
            assert!(
                (last.bounds[1] + last.bounds[3] - (300.0 - scroll.content_insets[2])).abs() < 0.1,
                "{style}: {:?}",
                last.bounds
            );
        }
    }

    #[test]
    fn measured_list_anchors_padding_and_failed_windows_are_transactional() {
        let source = source(
            "globalThis.anchor={index:500,align:'start',seq:0};function render(){return h('list',{count:1000,anchor:globalThis.anchor,style:{padding:12,gap:8},row:i=>h('button',{testId:'row-'+i,style:{height:40+(i%5)*10},onClick(){}},String(i))})}",
        );
        let mut page = ViewSession::new(&source).unwrap();
        let frame = page.layout(200, 300, 1.0).unwrap();
        let row = frame.nodes.iter().find(|n| n.test_id == "row-500").unwrap();
        assert!(row.visible_bounds[3] > 0.0);
        assert!(
            (row.bounds[1] - 12.0).abs() < 0.1,
            "anchor top {}",
            row.bounds[1]
        );
        let id = frame.nodes[0].scroll.as_ref().unwrap().id.clone();
        let generation = page.generation();
        let old_offset = frame.nodes[0].scroll.as_ref().unwrap().offset;
        let old_handle = row.on_click.unwrap();
        let failed = page.prepare_native(
            200,
            300,
            1.0,
            false,
            Some(&HashMap::from([(id.clone(), 1000.0)])),
            |_| Err::<(), _>("device unavailable".into()),
        );
        assert!(failed.is_err());
        assert_eq!(page.generation(), generation);
        assert_eq!(page.scroll_offsets[&id], old_offset);
        assert!(page.click(generation, old_handle).unwrap());
        let moved = page
            .prepare_native(
                200,
                300,
                1.0,
                false,
                Some(&HashMap::from([(id.clone(), 1000.0)])),
                Ok,
            )
            .unwrap();
        assert!(moved.nodes[0].scroll.as_ref().unwrap().offset < 2000.0);
        let rebuilt = page.layout(200, 300, 1.0).unwrap();
        assert!(
            rebuilt.nodes[0].scroll.as_ref().unwrap().offset < 2000.0,
            "unchanged anchor stole user scroll"
        );
        page.eval("globalThis.anchor='end'").unwrap();
        let end = page.layout(200, 300, 1.0).unwrap();
        let last = end.nodes.iter().find(|n| n.test_id == "row-999").unwrap();
        assert!(last.visible_bounds[3] > 0.0);
        let scroll = end.nodes[0].scroll.as_ref().unwrap();
        assert!((scroll.offset - scroll.max_offset).abs() < 0.1);
    }

    #[test]
    fn text_scaling_discards_offscreen_list_measurements() {
        let module = source(
            "globalThis.anchor={index:0,align:'start',seq:0};function render(){return h('list',{count:100,anchor:globalThis.anchor,row:i=>h('text',{testId:'row-'+i,style:{fontSize:16,lineHeight:24}},'A row with enough words to wrap over multiple lines')})}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        page.layout(200, 300, 1.0).unwrap();
        page.eval("globalThis.anchor={index:60,align:'start',seq:1}")
            .unwrap();
        page.layout(200, 300, 1.0).unwrap();
        let mut environment = page.environment.clone();
        environment["textScale"] = serde_json::json!([[0, 0], [16384, 32768]]);
        page.set_environment(environment.clone()).unwrap();
        let scaled = page.layout(200, 300, 1.0).unwrap();
        let mut fresh = ViewSession::new(&module).unwrap();
        fresh
            .eval("globalThis.anchor={index:60,align:'start',seq:1}")
            .unwrap();
        fresh.layout(200, 300, 1.0).unwrap();
        fresh.set_environment(environment).unwrap();
        let expected = fresh.layout(200, 300, 1.0).unwrap();
        let actual_scroll = scaled.nodes[0].scroll.as_ref().unwrap();
        let expected_scroll = expected.nodes[0].scroll.as_ref().unwrap();
        assert!((actual_scroll.max_offset - expected_scroll.max_offset).abs() < 0.1);
        assert!(
            scaled
                .nodes
                .iter()
                .any(|node| node.test_id == "row-60" && node.visible_bounds[3] > 0.0)
        );
    }

    #[test]
    fn distant_list_anchor_skips_old_rows_and_preserves_failed_frame() {
        let module = source(
            "globalThis.anchor=undefined;globalThis.visited=[];function render(){return h('list',{count:1000,anchor:globalThis.anchor,row:i=>{globalThis.visited.push(i);return h('button',{testId:'row-'+i,onClick(){},style:{height:40+(i%3)*20}},String(i))}})}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let first = page.layout(200, 300, 1.0).unwrap();
        let old = first
            .nodes
            .iter()
            .find(|node| node.test_id == "row-0")
            .unwrap();
        let old_handle = old.on_click.unwrap();
        let generation = page.generation();
        page.eval("globalThis.anchor='end';globalThis.visited=[]")
            .unwrap();
        let failed = page.prepare_native(200, 300, 1.0, true, None, |_| {
            Err::<(), _>("presentation failed".into())
        });
        assert!(failed.is_err());
        assert_eq!(page.generation(), generation);
        assert!(page.click(generation, old_handle).unwrap());
        page.eval("if(visited.some(i=>i<900))throw Error('constructed obsolete rows')")
            .unwrap();
        let end = page.layout(200, 300, 1.0).unwrap();
        assert!(
            end.nodes
                .iter()
                .any(|node| node.test_id == "row-999" && node.visible_bounds[3] > 0.0)
        );
        let scroll = end.nodes[0].scroll.as_ref().unwrap();
        assert!((scroll.offset - scroll.max_offset).abs() < 0.1);
        page.eval("globalThis.anchor={index:0,align:'start'};globalThis.visited=[]")
            .unwrap();
        let start = page.layout(200, 300, 1.0).unwrap();
        assert!(
            start
                .nodes
                .iter()
                .any(|node| node.test_id == "row-0" && node.visible_bounds[3] > 0.0)
        );
        page.eval("if(visited.some(i=>i>100))throw Error('constructed obsolete end rows')")
            .unwrap();
        assert_eq!(page.frame_metrics().layout_passes, 1);
    }

    #[test]
    fn frame_metrics_accumulate_every_list_description_transfer() {
        let module = source(
            "function render(){return h('list',{count:1000,anchor:'end',row:i=>h('text',{style:{height:50}},String(i))})}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        page.eval("globalThis.transferred=0;globalThis.passes=0;for(const name of ['__lingxiaPrepare','__lingxiaPrepareWindows']){const original=globalThis[name];globalThis[name]=(...args)=>{const result=original(...args);transferred+=result.length;passes++;return result}} ").unwrap();
        page.layout(200, 200, 1.0).unwrap();
        let measured = page.frame_metrics();
        assert!(measured.view_passes > 1);
        page.eval(&format!(
            "if(transferred!=={}||passes!=={})throw new Error('transfer accounting mismatch')",
            measured.wire_values, measured.view_passes
        ))
        .unwrap();
        assert!(
            measured.view_eval_us + measured.view_transfer_us + measured.view_decode_us
                <= measured.view_us
        );
    }

    #[test]
    fn list_count_shrink_refines_old_native_window_and_rolls_back_failed_frames() {
        let module = source(
            "globalThis.count=1000;function render(){return h('list',{count:globalThis.count,anchor:'end',row:i=>h('text',{testId:'row-'+i,style:{height:50}},String(i))})}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let first = page.layout(200, 200, 1.0).unwrap();
        assert!(
            first
                .nodes
                .iter()
                .any(|n| n.test_id == "row-999" && n.visible_bounds[3] > 0.0)
        );
        let generation = page.generation();
        page.eval("globalThis.count=1").unwrap();
        assert!(
            page.prepare_native(200, 200, 1.0, true, None, |_| Err::<(), _>(
                "surface unavailable".into()
            ))
            .is_err()
        );
        assert_eq!(page.generation(), generation);
        let one = page.layout(200, 200, 1.0).unwrap();
        assert!(
            one.nodes
                .iter()
                .any(|n| n.test_id == "row-0" && n.visible_bounds[3] > 0.0)
        );
        assert!(!one.nodes.iter().any(|n| n.test_id == "row-999"));
        assert_eq!(one.nodes[0].scroll.as_ref().unwrap().offset, 0.0);
        page.eval("globalThis.count=0").unwrap();
        let empty = page.layout(200, 200, 1.0).unwrap();
        assert_eq!(empty.nodes.len(), 1);
        assert_eq!(empty.nodes[0].scroll.as_ref().unwrap().max_offset, 0.0);
        page.eval("globalThis.count=1000").unwrap();
        let restored = page.layout(200, 200, 1.0).unwrap();
        assert!(
            restored
                .nodes
                .iter()
                .any(|n| n.test_id.starts_with("row-") && n.visible_bounds[3] > 0.0)
        );
    }

    #[test]
    fn image_completion_uses_retained_view_and_rejects_evicted_sources() {
        use ::image::ImageEncoder;
        let mut bytes = Vec::new();
        ::image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&[255, 0, 0, 255], 1, 1, ::image::ExtendedColorType::Rgba8)
            .unwrap();
        let mut page = ViewSession::new(&source("let calls=0;function render(){calls++;return h('scroll',{},h('image',{src:'photo',style:{height:100,width:100}}),h('text',{style:{height:200}},String(calls)))}")).unwrap();
        assert!(!page.load_image("photo", &bytes).unwrap());
        page.layout(100, 100, 1.0).unwrap();
        assert!(page.load_image("photo", &bytes).unwrap());
        let display = page.prepare_native(100, 100, 1.0, false, None, Ok).unwrap();
        assert_eq!(display.loaded_image_count(), 1);
        assert_eq!(display.nodes[2].text, "1");
        let id = display.nodes[0].scroll.as_ref().unwrap().id.clone();
        page.prepare_native(100, 100, 1.0, false, Some(&HashMap::from([(id, 200.)])), Ok)
            .unwrap();
        assert!(page.images.is_empty());
        assert!(!page.load_image("photo", &bytes).unwrap());
    }

    #[test]
    fn reach_handles_are_published_without_running_callbacks_and_reject_stale_input() {
        let source = source(
            "function render(){const calls=useLocal('calls',0);return h('scroll',{onReachStart:()=>calls.set(calls.value+1),onReachEnd:()=>calls.set(calls.value+10)},h('text',{},String(calls.value)))}",
        );
        let mut page = ViewSession::new(&source).unwrap();
        let first = page.layout(100, 100, 1.0).unwrap();
        assert_eq!(first.nodes[1].text, "0");
        let scroll = first.nodes[0].scroll.as_ref().unwrap();
        let generation = page.generation();
        assert!(
            page.click(generation, scroll.on_reach_start.unwrap())
                .unwrap()
        );
        assert!(
            page.click(generation, scroll.on_reach_end.unwrap())
                .unwrap()
        );
        let changed = page.layout(100, 100, 1.0).unwrap();
        assert_eq!(changed.nodes[1].text, "11");
        assert!(
            !page
                .click(generation, scroll.on_reach_end.unwrap())
                .unwrap()
        );
    }

    #[test]
    fn native_scroll_reuses_committed_description_and_preserves_click_handles() {
        let source = source(
            "let renders=0; function render(){renders++; const n=useLocal('n',0); return h('scroll',{testId:'feed'},h('button',{style:{height:300},onClick:()=>n.set(n.value+1)},`${renders}:${n.value}`))}",
        );
        let mut page = ViewSession::new(&source).unwrap();
        let initial = page.layout(100, 100, 1.0).unwrap();
        let id = initial.nodes[0].scroll.as_ref().unwrap().id.clone();
        let offsets = HashMap::from([(id, 60.0)]);
        let scrolled = page
            .prepare_native(100, 100, 1.0, false, Some(&offsets), Ok)
            .unwrap();
        assert_eq!(scrolled.nodes[1].text, "1:0");
        assert_eq!(scrolled.nodes[0].scroll.as_ref().unwrap().offset, 60.0);
        assert!(!page.click(1, scrolled.nodes[1].on_click.unwrap()).unwrap());
        assert!(
            page.click(page.generation(), scrolled.nodes[1].on_click.unwrap())
                .unwrap()
        );
        let changed = page.layout(100, 100, 1.0).unwrap();
        assert_eq!(changed.nodes[1].text, "2:1");
        assert_eq!(changed.nodes[0].scroll.as_ref().unwrap().offset, 60.0);
        page.reset().unwrap();
        let reset = page.prepare_native(100, 100, 1.0, false, None, Ok).unwrap();
        assert_eq!(reset.nodes[0].scroll.as_ref().unwrap().offset, 0.0);
        assert_eq!(reset.nodes[1].text, "3:0");
    }

    #[test]
    fn failed_scroll_candidate_does_not_commit_offsets_or_generation() {
        let source =
            source("function render(){return h('scroll',{},h('spacer',{style:{height:300}}))}");
        let mut page = ViewSession::new(&source).unwrap();
        let initial = page.layout(100, 100, 1.0).unwrap();
        let id = initial.nodes[0].scroll.as_ref().unwrap().id.clone();
        let result = page.prepare_native(
            100,
            100,
            1.0,
            false,
            Some(&HashMap::from([(id, 100.0)])),
            |_| Err::<(), _>("surface lost".into()),
        );
        assert!(result.is_err());
        assert_eq!(page.generation(), 1);
        let next = page.prepare_native(100, 100, 1.0, false, None, Ok).unwrap();
        assert_eq!(next.nodes[0].scroll.as_ref().unwrap().offset, 0.0);
    }

    pub(crate) fn source(render: &str) -> String {
        let resources = include_str!("../../../packages/lingxia-ui/src/runtime/resources.js")
            .replace("export function", "function");
        let authoring = include_str!("../../../packages/lingxia-ui/src/authoring/props.js")
            .lines()
            .filter(|line| !line.starts_with("import "))
            .collect::<Vec<_>>()
            .join("\n")
            .replace("export function", "function");
        let authoring =
            format!("const acceptsProp = (() => {{ {authoring}\nreturn acceptsProp; }})();");
        let runtime = include_str!("../../../packages/lingxia-ui/src/runtime.js");
        let runtime = runtime
            .lines()
            .filter(|line| !line.starts_with("import ") && !line.starts_with("export { "))
            .collect::<Vec<_>>()
            .join("\n")
            .replace("export function", "function");
        let runtime = format!("{authoring}\n{resources}\n{runtime}");
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{runtime}\n{render}\nObject.assign(globalThis, {{__lingxiaRender:render,__lingxiaPrepare:prepare,__lingxiaPrepareWindows:prepareWindows,__lingxiaCommit:commit,__lingxiaDiscard:discard,__lingxiaReset:reset,__lingxiaDispatch:dispatch,__lingxiaCreateView:createView,__lingxiaConfigureActions:configureActions,__lingxiaTakeActionRequests:takeActionRequests,__lingxiaReceiveActionResponse:receiveActionResponse,__lingxiaTakeActionErrors:takeActionErrors,__lingxiaHasLocalUpdates:hasLocalUpdates,__lingxiaRequestLeave:requestLeave}});",
            crate::schema::javascript_schema().replace("export const", "const"),
            include_str!("../../../packages/lingxia-ui/src/shadow.js")
                .replace("export function", "function"),
            include_str!("../../../packages/lingxia-ui/src/gradient.js")
                .replace("export function", "function"),
            include_str!("../../../packages/lingxia-ui/src/stream.js")
                .replace("export function", "function"),
            include_str!("../../../packages/lingxia-ui/src/channel.js")
                .replace("export function", "function"),
            include_str!("../../../packages/lingxia-ui/src/channel-resource.js")
                .replace("export function", "function"),
            include_str!("../../../packages/lingxia-ui/src/leave-guard.js")
                .replace("export function", "function"),
            include_str!("../../../packages/lingxia-ui/src/classes.js")
                .replace("export function", "function")
        )
    }

    #[test]
    fn committed_channel_resource_opens_without_input_and_quiesces_after_redraw() {
        use serde_json::json;
        let module = source(
            "function render(view){const last=useLocal('last','waiting');const feed=useChannel('feed','ticker',{onData:v=>last.set(v)});return h('column',{},h('text',{},feed.connected?'connected':'waiting'),h('text',{},last.value));}",
        ) + "\nglobalThis.__lingxiaActionCatalog={__names:['ticker'],__modes:{ticker:'call'}};";
        let mut page = ViewSession::new_with_logic(&module, true).unwrap();
        page.begin_bridge("nonce").unwrap();
        page.receive_bridge(
            &json!({"v":2,"kind":"helloAck","nonce":"nonce","protocol":2,"sessionId":"session"}),
        )
        .unwrap();
        page.receive_bridge(&json!({"v":2,"kind":"state.snapshot","rev":0,"state":{}}))
            .unwrap();
        page.receive_bridge(&json!({"v":2,"kind":"ready","sessionId":"session"}))
            .unwrap();
        assert!(page.take_action_requests().unwrap().is_empty());
        page.layout(300, 500, 1.0).unwrap();
        let requests = page.take_action_requests().unwrap();
        assert_eq!(requests.len(), 1);
        let request: serde_json::Value = serde_json::from_str(&requests[0]).unwrap();
        assert_eq!(request["kind"], "ch.open");
        assert!(page.has_local_updates().unwrap());
        page.receive_bridge(&json!({"v":2,"kind":"ch.ack","id":request["id"],"ok":true}))
            .unwrap();
        page.receive_bridge(
            &json!({"v":2,"kind":"ch.data","id":request["id"],"seq":0,"payload":"tick"}),
        )
        .unwrap();
        let frame = page.layout(300, 500, 1.0).unwrap();
        assert_eq!(frame.nodes[1].text, "connected");
        assert_eq!(frame.nodes[2].text, "tick");
        assert!(!page.has_local_updates().unwrap());
        page.layout(300, 500, 1.0).unwrap();
        assert!(page.take_action_requests().unwrap().is_empty());
        assert!(!page.has_local_updates().unwrap());
    }

    #[test]
    fn stream_bridge_events_render_and_cancel_through_the_page_endpoint() {
        use serde_json::json;
        let module = source(
            "let stream;function render({actions}){const value=useLocal('value','');return h('column',{},h('button',{onClick:()=>{stream=actions.watch();stream.on('data',v=>value.set(v));}},'Start'),h('text',{},value.value),h('button',{onClick:()=>stream.cancel()},'Stop'))}",
        ) + "\nglobalThis.__lingxiaActionCatalog={__names:['watch'],__modes:{watch:'stream'}};";
        let mut page = ViewSession::new_with_logic(&module, true).unwrap();
        page.begin_bridge("nonce").unwrap();
        page.receive_bridge(
            &json!({"v":2,"kind":"helloAck","nonce":"nonce","protocol":2,"sessionId":"session"}),
        )
        .unwrap();
        page.receive_bridge(&json!({"v":2,"kind":"state.snapshot","rev":0,"state":{}}))
            .unwrap();
        page.receive_bridge(&json!({"v":2,"kind":"ready","sessionId":"session"}))
            .unwrap();
        let frame = page.layout(300, 500, 1.0).unwrap();
        page.click(page.generation(), frame.nodes[1].on_click.unwrap())
            .unwrap();
        let request: serde_json::Value =
            serde_json::from_str(&page.take_action_requests().unwrap()[0]).unwrap();
        let event = json!({"v":2,"kind":"event","id":request["id"],"seq":0,"payload":"first"});
        assert!(page.receive_bridge(&event).unwrap().changed);
        let frame = page.layout(300, 500, 1.0).unwrap();
        assert_eq!(frame.nodes[2].text, "first");
        page.click(page.generation(), frame.nodes[3].on_click.unwrap())
            .unwrap();
        let cancel: serde_json::Value =
            serde_json::from_str(&page.take_action_requests().unwrap()[0]).unwrap();
        assert_eq!(cancel, json!({"v":2,"kind":"cancel","id":request["id"]}));
        assert!(!page.receive_bridge(&event).unwrap().changed);
    }

    #[test]
    fn channel_bridge_opens_delivers_and_sends_on_the_bound_endpoint() {
        use serde_json::json;
        let module = source(
            "let channel;function render({channels}){const value=useLocal('value','');return h('column',{},h('button',{onClick:async()=>{channel=await channels.open('ticker');channel.on('data',v=>{value.set(v);channel.send({symbol:'MSFT'})})}},'Connect'),h('text',{},value.value),h('button',{onClick:()=>channel.close()},'Close'))}",
        ) + "\nglobalThis.__lingxiaActionCatalog={__names:['ticker'],__modes:{ticker:'call'}};";
        let mut page = ViewSession::new_with_logic(&module, true).unwrap();
        page.begin_bridge("nonce").unwrap();
        page.receive_bridge(
            &json!({"v":2,"kind":"helloAck","nonce":"nonce","protocol":2,"sessionId":"session"}),
        )
        .unwrap();
        page.receive_bridge(&json!({"v":2,"kind":"state.snapshot","rev":0,"state":{}}))
            .unwrap();
        page.receive_bridge(&json!({"v":2,"kind":"ready","sessionId":"session"}))
            .unwrap();
        let frame = page.layout(300, 500, 1.0).unwrap();
        page.click(page.generation(), frame.nodes[1].on_click.unwrap())
            .unwrap();
        let request: serde_json::Value =
            serde_json::from_str(&page.take_action_requests().unwrap()[0]).unwrap();
        assert_eq!(request["kind"], "ch.open");
        page.receive_bridge(&json!({"v":2,"kind":"ch.ack","id":request["id"],"ok":true}))
            .unwrap();
        let message =
            json!({"v":2,"kind":"ch.data","id":request["id"],"seq":0,"payload":"first tick"});
        let update = page.receive_bridge(&message).unwrap();
        assert_eq!(
            update.outbound,
            vec![
                json!({"v":2,"kind":"ch.data","id":request["id"],"seq":0,"payload":{"symbol":"MSFT"}})
            ]
        );
        let frame = page.layout(300, 500, 1.0).unwrap();
        assert_eq!(frame.nodes[2].text, "first tick");
        page.click(page.generation(), frame.nodes[3].on_click.unwrap())
            .unwrap();
        let closed: serde_json::Value =
            serde_json::from_str(&page.take_action_requests().unwrap()[0]).unwrap();
        assert_eq!(closed["kind"], "ch.close");
        assert!(!page.receive_bridge(&message).unwrap().changed);
    }

    #[test]
    fn composing_uses_native_layout_and_commits_back_to_local_state() {
        use crate::text_input::Edit;
        for search in [false, true] {
            let module = source(
                "let renders=0;function render(){renders++;const draft=useLocal('draft',''),sent=useLocal('sent','');return h('column',{},h('input',{key:'draft',bind:draft,autoFocus:true,onSubmit:v=>sent.set(v)}),h('text',{},draft.value+'|'+sent.value+'|'+renders))}",
            );
            let module = if search {
                module.replace("h('input',", "h('search',")
            } else {
                module
            };
            let mut page = ViewSession::new(&module).unwrap();
            let first = page.layout(300, 300, 1.0).unwrap();
            let id = first.nodes[1].identity.clone();
            let token = first.nodes[1].input.as_ref().unwrap().edit_token.unwrap();
            let reply = page
                .edit_input(
                    token,
                    &id,
                    &[Edit::Replace {
                        text: "ni".into(),
                        cursor: 1,
                        composing: true,
                    }],
                    false,
                )
                .unwrap()
                .unwrap();
            assert!(!reply.needs_view_render);
            let preedit = page.prepare_native(300, 300, 1.0, false, None, Ok).unwrap();
            assert_eq!(preedit.nodes[1].text, "ni");
            assert_eq!(preedit.nodes[2].text, "||1");
            assert_eq!(
                preedit.nodes[1]
                    .input
                    .as_ref()
                    .unwrap()
                    .edit
                    .as_ref()
                    .unwrap()
                    .composition,
                Some([0, 2])
            );
            let reply = page
                .edit_input(
                    token,
                    &id,
                    &[Edit::Replace {
                        text: "你".into(),
                        cursor: 1,
                        composing: false,
                    }],
                    true,
                )
                .unwrap()
                .unwrap();
            assert!(reply.needs_view_render);
            let committed = page.prepare_native(300, 300, 1.0, false, None, Ok).unwrap();
            assert_eq!(committed.nodes[2].text, "你|你|2");
            assert_eq!(
                committed.nodes[1].input.as_ref().unwrap().edit_token,
                Some(token)
            );
            assert!(page.blur_input(token, &id).unwrap());
            assert!(page.edit_input(token, &id, &[], false).unwrap().is_none());
            let blurred = page.prepare_native(300, 300, 1.0, false, None, Ok).unwrap();
            assert_eq!(blurred.nodes[1].text, "你");
            assert!(
                !blurred.nodes[1]
                    .input
                    .as_ref()
                    .unwrap()
                    .edit
                    .as_ref()
                    .unwrap()
                    .focused
            );
        }
    }

    #[test]
    fn input_focus_tokens_survive_reorder_but_not_disable_removal_or_reset() {
        use crate::text_input::Edit;
        let module = source(
            "globalThis.show=true;globalThis.enabled=true;globalThis.insert=false;function render(){return h('column',{},insert?h('text',{key:'new'},'new'):null,show?h('input',{key:'a',value:'canonical',enabled,autoFocus:true}):null,h('input',{key:'b',value:'second'}))}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let first = page.layout(300, 300, 1.0).unwrap();
        let id = first.nodes[1].identity.clone();
        let token = first.nodes[1].input.as_ref().unwrap().edit_token.unwrap();
        page.edit_input(
            token,
            &id,
            &[Edit::Replace {
                text: "pending".into(),
                cursor: 1,
                composing: true,
            }],
            false,
        )
        .unwrap();
        page.eval("insert=true").unwrap();
        let reordered = page.layout(300, 300, 1.0).unwrap();
        assert_eq!(
            reordered.nodes[2].input.as_ref().unwrap().edit_token,
            Some(token)
        );
        assert_eq!(reordered.nodes[2].text, "canonicalpending");
        page.eval("enabled=false").unwrap();
        let disabled = page.layout(300, 300, 1.0).unwrap();
        assert_eq!(disabled.nodes[2].text, "canonical");
        assert!(page.edit_input(token, &id, &[], false).unwrap().is_none());
        page.eval("enabled=true").unwrap();
        let enabled = page.layout(300, 300, 1.0).unwrap();
        assert_eq!(enabled.nodes[2].input.as_ref().unwrap().edit_token, None);
        let token2 = page
            .focus_input(page.generation(), &id)
            .unwrap()
            .unwrap()
            .token;
        assert_ne!(token, token2);
        page.eval("show=false").unwrap();
        page.layout(300, 300, 1.0).unwrap();
        assert!(page.edit_input(token2, &id, &[], false).unwrap().is_none());
        page.eval("show=true").unwrap();
        let restored = page.layout(300, 300, 1.0).unwrap();
        let token3 = restored.nodes[2]
            .input
            .as_ref()
            .unwrap()
            .edit_token
            .unwrap();
        page.reset().unwrap();
        let reset = page.layout(300, 300, 1.0).unwrap();
        assert_ne!(
            reset.nodes[2].input.as_ref().unwrap().edit_token,
            Some(token3)
        );
    }

    #[test]
    fn failed_frames_and_callback_errors_do_not_commit_candidate_editors() {
        use crate::text_input::Edit;
        let module = source(
            "globalThis.show=false;function render(){return h('column',{},show?h('input',{key:'a',autoFocus:true,value:'old',onInput(){throw Error('handler failed')}}):null)}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        page.layout(300, 300, 1.0).unwrap();
        page.eval("show=true").unwrap();
        assert!(
            page.prepare_native::<()>(300, 300, 1.0, true, None, |_| Err("failed build".into()))
                .is_err()
        );
        let frame = page.layout(300, 300, 1.0).unwrap();
        let id = frame.nodes[1].identity.clone();
        let token = frame.nodes[1].input.as_ref().unwrap().edit_token.unwrap();
        assert!(
            page.edit_input(
                token,
                &id,
                &[Edit::Replace {
                    text: "new".into(),
                    cursor: 1,
                    composing: false
                }],
                false
            )
            .is_err()
        );
        let reply = page.edit_input(token, &id, &[], false).unwrap().unwrap();
        assert_eq!(reply.state.text, "old");
        assert!(
            page.edit_input(token, &id, &[Edit::Value("host override".into())], false)
                .is_err()
        );
        assert_eq!(
            page.edit_input(token, &id, &[], false)
                .unwrap()
                .unwrap()
                .state
                .text,
            "old"
        );
    }

    #[test]
    fn viewport_reveal_preserves_editor_and_rolls_back_failed_frames() {
        for scale in [1.0, 2.0] {
            let module = source(
                "function render(){return h('scroll',{},h('spacer',{style:{height:300}}),h('input',{testId:'editor',autoFocus:true,value:'draft',style:{height:44}}),h('spacer',{style:{height:200}}))}",
            );
            let mut page = ViewSession::new(&module).unwrap();
            let width = (300.0 * scale) as u16;
            let frame = page.layout(width, (600.0 * scale) as u16, scale).unwrap();
            let id = frame.nodes[2].identity.clone();
            let token = frame.nodes[2].input.as_ref().unwrap().edit_token.unwrap();
            let before = page.scroll_offsets.clone();
            assert!(
                page.prepare_native::<()>(
                    width,
                    (120.0 * scale) as u16,
                    scale,
                    false,
                    None,
                    |_| Err("failed presentation".into())
                )
                .is_err()
            );
            assert_eq!(page.scroll_offsets, before);
            assert!(page.input_state(token, &id).is_some());
            let frame = page
                .prepare_native(width, (120.0 * scale) as u16, scale, false, None, Ok)
                .unwrap();
            assert_eq!(frame.nodes[2].visible_bounds[3], 44.0);
            assert_eq!(
                frame.nodes[2].input.as_ref().unwrap().edit_token,
                Some(token)
            );
            assert_eq!(frame.nodes[0].scroll.as_ref().unwrap().offset, 224.0);
            assert!(
                page.edit_input(
                    token,
                    &id,
                    &[crate::text_input::Edit::Replace {
                        text: "!".into(),
                        cursor: 1,
                        composing: false
                    }],
                    false
                )
                .unwrap()
                .is_some()
            );
            let scroll = frame.nodes[0].scroll.as_ref().unwrap().id.clone();
            page.prepare_native(
                width,
                (120.0 * scale) as u16,
                scale,
                false,
                Some(&HashMap::from([(scroll, 0.0)])),
                Ok,
            )
            .unwrap();
            assert!(page.input_state(token, &id).is_none());
        }
    }

    #[test]
    fn viewport_reveal_walks_nested_scroll_regions() {
        let module = source(
            "function render(){return h('scroll',{},h('spacer',{style:{height:100}}),h('scroll',{style:{height:180}},h('spacer',{style:{height:140}}),h('input',{autoFocus:true,value:'nested',style:{height:44}}),h('spacer',{style:{height:100}})),h('spacer',{style:{height:200}}))}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let frame = page.layout(300, 500, 1.0).unwrap();
        let token = frame.nodes[4].input.as_ref().unwrap().edit_token.unwrap();
        let frame = page.layout(300, 120, 1.0).unwrap();
        assert_eq!(
            frame.nodes[4].input.as_ref().unwrap().edit_token,
            Some(token)
        );
        assert_eq!(frame.nodes[4].visible_bounds[3], 44.0);
        assert!(frame.nodes[0].scroll.as_ref().unwrap().offset > 0.0);
        assert!(frame.nodes[2].scroll.as_ref().unwrap().offset > 0.0);
    }

    #[test]
    fn keyboard_occlusion_reveals_without_surface_resize() {
        let module = source(
            "function render(view){return h('column',{},h('scroll',{style:{grow:1}},h('spacer',{style:{height:300}}),h('input',{autoFocus:true,value:'draft',style:{height:44}}),h('spacer',{style:{height:200}})),h('spacer',{style:{height:view.env.keyboard.height}}))}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let frame = page.layout(300, 500, 1.0).unwrap();
        let token = frame.nodes[3].input.as_ref().unwrap().edit_token.unwrap();
        let mut environment = page.environment.clone();
        environment["keyboard"]["height"] = serde_json::json!(380);
        page.set_environment(environment).unwrap();
        let frame = page.prepare_native(300, 500, 1.0, false, None, Ok).unwrap();
        assert_eq!(frame.nodes[3].visible_bounds[3], 44.0);
        assert_eq!(
            frame.nodes[3].input.as_ref().unwrap().edit_token,
            Some(token)
        );
        assert_eq!(frame.nodes[1].scroll.as_ref().unwrap().offset, 224.0);
    }

    #[test]
    fn editor_focus_paint_tracks_autofocus_blur_disable_and_failed_frames() {
        let module = source(
            "configureClasses([{name:'base',when:{},style:{background:'#ffffff'}},{name:'focused',when:{focus:true},style:{background:'#0000ff'}}]);globalThis.__focusEnabled=true;function render(){return h('input',{className:'base focused',enabled:globalThis.__focusEnabled,autoFocus:true,style:{width:100,height:40,radius:0}})}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let display = page.layout(100, 40, 1.0).unwrap();
        let id = display.nodes[0].identity.clone();
        let token = display.nodes[0].input.as_ref().unwrap().edit_token.unwrap();
        let tint = |display: DisplayList| {
            let frame = display.rasterize().unwrap();
            frame.pixels.data_as_u8_slice()[404..408].to_vec()
        };
        assert_eq!(tint(display), vec![0, 0, 255, 255]);
        page.blur_input(token, &id).unwrap();
        assert_eq!(
            tint(page.prepare_native(100, 40, 1.0, false, None, Ok).unwrap()),
            vec![255; 4]
        );
        page.focus_input(page.generation(), &id).unwrap().unwrap();
        let failed: Result<(), String> =
            page.prepare_native(100, 40, 1.0, false, None, |_| Err("injected".into()));
        assert!(failed.is_err());
        assert_eq!(
            tint(page.prepare_native(100, 40, 1.0, false, None, Ok).unwrap()),
            vec![255; 4]
        );
        page.focus_input(page.generation(), &id).unwrap().unwrap();
        assert_eq!(
            tint(page.prepare_native(100, 40, 1.0, false, None, Ok).unwrap()),
            vec![0, 0, 255, 255]
        );
        page.eval("globalThis.__focusEnabled=false; 'ok'").unwrap();
        let disabled = page.layout(100, 40, 1.0).unwrap();
        assert!(
            disabled.nodes[0]
                .input
                .as_ref()
                .unwrap()
                .edit_token
                .is_none()
        );
        assert_eq!(tint(disabled), vec![255; 4]);
    }

    #[test]
    fn editor_focus_paint_recomputes_when_scrolling_changes_visibility() {
        let module = source(
            "configureClasses([{name:'base',when:{},style:{background:'#ffffff'}},{name:'focused',when:{focus:true},style:{background:'#0000ff'}}]);function render(){return h('scroll',{},h('spacer',{style:{height:200}}),h('input',{className:'base focused',autoFocus:true,style:{height:40,radius:0}}),h('spacer',{style:{height:160}}))}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let first = page.layout(200, 100, 1.0).unwrap();
        let scroll = first.nodes[0].scroll.as_ref().unwrap().id.clone();
        for (offset, focused) in [(200.0, true), (0.0, false), (200.0, false)] {
            let display = page
                .prepare_native(
                    200,
                    100,
                    1.0,
                    false,
                    Some(&HashMap::from([(scroll.clone(), offset)])),
                    Ok,
                )
                .unwrap();
            assert_eq!(
                display.nodes[2]
                    .input
                    .as_ref()
                    .unwrap()
                    .edit_token
                    .is_some(),
                focused
            );
            if offset == 200.0 {
                let frame = display.rasterize().unwrap();
                assert_eq!(
                    &frame.pixels.data_as_u8_slice()[804..808],
                    if focused {
                        &[0, 0, 255, 255]
                    } else {
                        &[255; 4]
                    }
                );
            }
        }
    }

    #[test]
    fn autofocus_waits_for_visibility_and_does_not_repeat_after_blur() {
        let module = source(
            "function render(){return h('scroll',{},h('spacer',{style:{height:200}}),h('input',{key:'a',value:'draft',autoFocus:true}))}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let first = page.layout(200, 100, 1.0).unwrap();
        let id = first.nodes[2].identity.clone();
        let scroll = first.nodes[0].scroll.as_ref().unwrap().id.clone();
        assert_eq!(first.nodes[2].input.as_ref().unwrap().edit_token, None);
        assert!(page.focus_input(page.generation(), &id).unwrap().is_none());
        let visible = page
            .prepare_native(
                200,
                100,
                1.0,
                false,
                Some(&HashMap::from([(scroll.clone(), 200.0)])),
                Ok,
            )
            .unwrap();
        let token = visible.nodes[2].input.as_ref().unwrap().edit_token.unwrap();
        assert!(page.blur_input(token, &id).unwrap());
        let blurred = page.prepare_native(200, 100, 1.0, false, None, Ok).unwrap();
        assert_eq!(blurred.nodes[2].input.as_ref().unwrap().edit_token, None);
        let focused = page.focus_input(page.generation(), &id).unwrap().unwrap();
        page.edit_input(
            focused.token,
            &id,
            &[crate::text_input::Edit::Replace {
                text: "pending".into(),
                cursor: 1,
                composing: true,
            }],
            false,
        )
        .unwrap();
        let preedit = page.prepare_native(200, 100, 1.0, false, None, Ok).unwrap();
        assert!(preedit.nodes[2].text.contains("pending"));
        page.prepare_native(
            200,
            100,
            1.0,
            false,
            Some(&HashMap::from([(scroll.clone(), 0.0)])),
            Ok,
        )
        .unwrap();
        assert!(page.input_state(focused.token, &id).is_none());
        let visible = page
            .prepare_native(
                200,
                100,
                1.0,
                false,
                Some(&HashMap::from([(scroll, 200.0)])),
                Ok,
            )
            .unwrap();
        assert_eq!(visible.nodes[2].text, "draft");
        assert_eq!(visible.nodes[2].input.as_ref().unwrap().edit_token, None);
    }

    #[test]
    fn editor_budget_failure_preserves_the_previous_input_connection() {
        let module = source(
            "globalThis.count=1;globalThis.large=false;function render(){return h('column',{},Array.from({length:count},(_,i)=>h('input',{key:String(i),value:large?'x'.repeat(4096):'',autoFocus:i===0})))}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let frame = page.layout(200, 200, 1.0).unwrap();
        let id = frame.nodes[1].identity.clone();
        let token = frame.nodes[1].input.as_ref().unwrap().edit_token.unwrap();
        for change in ["count=129", "count=100;large=true"] {
            page.eval(change).unwrap();
            let error = page.layout(200, 200, 1.0).err().unwrap();
            assert!(error.contains("UI_INPUT:"), "{change}: {error}");
            assert_eq!(page.input_state(token, &id).unwrap().state.text, "");
        }
    }

    #[test]
    fn failed_input_presentation_restores_buffer_and_invalidates_the_connection() {
        use crate::text_input::Edit;
        let module = source(
            "function render(){const draft=useLocal('draft','old');return h('column',{},h('input',{bind:draft,autoFocus:true}),h('text',{},draft.value))}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let frame = page.layout(200, 200, 1.0).unwrap();
        let id = frame.nodes[1].identity.clone();
        let token = frame.nodes[1].input.as_ref().unwrap().edit_token.unwrap();
        page.edit_input(
            token,
            &id,
            &[Edit::Replace {
                text: "x".into(),
                cursor: 1,
                composing: false,
            }],
            false,
        )
        .unwrap();
        assert!(
            page.prepare_native::<()>(200, 200, 1.0, false, None, |_| Err("surface failed".into()))
                .is_err()
        );
        assert!(page.input_state(token, &id).is_none());
        let restored = page.prepare_native(200, 200, 1.0, false, None, Ok).unwrap();
        assert_eq!(restored.nodes[1].text, "old");
        assert_eq!(restored.nodes[2].text, "old");
        assert_ne!(
            restored.nodes[1].input.as_ref().unwrap().edit_token,
            Some(token)
        );
    }

    #[test]
    fn host_undo_redo_updates_bound_values_and_failed_frames_restore_history() {
        use serde_json::{Value, json};
        let module = source(
            "function render(){const draft=useLocal('draft','');return h('input',{bind:draft,autoFocus:true})}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let frame = page.layout(200, 100, 1.0).unwrap();
        let id = &frame.nodes[0].identity;
        let token = frame.nodes[0].input.as_ref().unwrap().edit_token.unwrap();
        let mut request = json!({"op":"edit", "identity":id, "token":token, "submit":false,
            "edits":[{"op":"replace", "text":"hello", "cursor":1, "composing":false}]});
        page.input_request(&request.to_string()).unwrap();
        page.prepare_native(200, 100, 1.0, false, None, Ok).unwrap();
        request["edits"] = json!([{"op":"undo"}]);
        let reply: Value =
            serde_json::from_str(&page.input_request(&request.to_string()).unwrap()).unwrap();
        assert_eq!(reply["state"]["text"], "");
        assert_eq!(reply["state"]["can_redo"], true);
        assert_eq!(reply["needs_view_render"], true);
        let undone = page.prepare_native(200, 100, 1.0, false, None, Ok).unwrap();
        assert_eq!(undone.nodes[0].text, "");
        request["edits"] = json!([{"op":"redo"}]);
        page.input_request(&request.to_string()).unwrap();
        assert!(
            page.prepare_native::<()>(200, 100, 1.0, false, None, |_| Err("surface failed".into()))
                .is_err()
        );
        assert!(page.input_state(token, id).is_none());
        let restored = page.prepare_native(200, 100, 1.0, false, None, Ok).unwrap();
        let new_token = restored.nodes[0]
            .input
            .as_ref()
            .unwrap()
            .edit_token
            .unwrap();
        let state = page.input_state(new_token, id).unwrap().state;
        assert_eq!(state.text, "");
        assert!(state.can_redo);
        request["token"] = json!(new_token);
        page.input_request(&request.to_string()).unwrap();
        let redone = page.prepare_native(200, 100, 1.0, false, None, Ok).unwrap();
        assert_eq!(redone.nodes[0].text, "hello");
    }

    #[test]
    fn host_input_messages_preserve_composition_and_reject_stale_connections() {
        use serde_json::{Value, json};
        let module = source(
            "function render(){const draft=useLocal('draft','');return h('input',{bind:draft})}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let frame = page.layout(200, 100, 1.0).unwrap();
        let id = &frame.nodes[0].identity;
        let reply: Value = serde_json::from_str(
            &page
                .input_request(
                    &json!({
                        "op":"focus", "identity":id, "generation":page.generation()
                    })
                    .to_string(),
                )
                .unwrap(),
        )
        .unwrap();
        let token = reply["token"].as_u64().unwrap();
        let mut request = json!({"op":"edit", "identity":id, "token":token, "submit":false,
            "edits":[{"op":"replace", "text":"\u{4f60}\u{1f600}", "cursor":1, "composing":true}]});
        let reply: Value =
            serde_json::from_str(&page.input_request(&request.to_string()).unwrap()).unwrap();
        assert_eq!(reply["state"]["selection"], json!([3, 3]));
        assert_eq!(reply["state"]["composition"], json!([0, 3]));
        assert_eq!(reply["needs_view_render"], false);
        request["edits"] = json!([{"op":"finish_composition"}]);
        let reply: Value =
            serde_json::from_str(&page.input_request(&request.to_string()).unwrap()).unwrap();
        assert_eq!(reply["needs_view_render"], true);
        page.prepare_native(200, 100, 1.0, false, None, Ok).unwrap();
        let query = json!({"op":"query","identity":id,"token":token});
        assert_eq!(
            serde_json::from_str::<Value>(&page.input_request(&query.to_string()).unwrap())
                .unwrap()["state"]["text"],
            "\u{4f60}\u{1f600}"
        );
        request["edits"] = json!(vec![json!({"op":"finish_composition"}); 65]);
        assert!(
            page.input_request(&request.to_string())
                .unwrap_err()
                .contains("too many edits")
        );
        assert!(
            page.input_request(&" ".repeat(crate::input_protocol::MAX_REQUEST_BYTES + 1))
                .unwrap_err()
                .contains("budget")
        );
        assert_eq!(
            page.input_request(&json!({"op":"blur","identity":id,"token":token}).to_string())
                .unwrap(),
            "true"
        );
        assert_eq!(page.input_request(&query.to_string()).unwrap(), "null");
        assert_eq!(
            page.input_request(&json!({"op":"focus","identity":id,"generation":0}).to_string())
                .unwrap(),
            "null"
        );
    }

    #[test]
    fn slider_events_are_numeric_generation_bound_and_commit_transactionally() {
        let module = source(
            "function render(){const value=useLocal('value',0);return h('slider',{accessibilityLabel:'Volume',bind:value,min:0,max:10,step:3})}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let first = page.layout(200, 100, 1.0).unwrap();
        let handle = first.nodes[0].range.as_ref().unwrap().on_change.unwrap();
        let generation = page.generation();
        assert!(!page.click(generation, handle).unwrap());
        assert!(!page.text_event(generation, handle, "3").unwrap());
        assert!(page.number_event(generation, handle, f64::NAN).is_err());
        assert!(page.number_event(generation, handle, 9.6).unwrap());
        assert!(
            page.prepare_native(200, 100, 1.0, true, None, |_| Err::<(), _>(
                "surface unavailable".into()
            ))
            .is_err()
        );
        assert_eq!(page.generation(), generation);
        let restored = page.layout(200, 100, 1.0).unwrap();
        assert_eq!(restored.nodes[0].range.as_ref().unwrap().value, 0.0);
        assert!(!page.number_event(generation, handle, 3.0).unwrap());
        let handle = restored.nodes[0].range.as_ref().unwrap().on_change.unwrap();
        assert!(page.number_event(page.generation(), handle, 9.6).unwrap());
        let next = page.layout(200, 100, 1.0).unwrap();
        assert_eq!(next.nodes[0].range.as_ref().unwrap().value, 10.0);
    }

    #[test]
    fn text_events_are_typed_generation_bound_and_safely_encoded() {
        let module = source(
            "function render(){const draft=useLocal('draft','');return h('column',{},h('input',{bind:draft}),h('text',{},draft.value))}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let frame = page.prepare_native(300, 300, 1.0, true, None, Ok).unwrap();
        let handle = frame.nodes[1].input.as_ref().unwrap().on_input.unwrap();
        let generation = page.generation();
        assert!(!page.click(generation, handle).unwrap());
        let value = "你好😀\n\"quoted\"\\suffix";
        assert!(page.text_event(generation, handle, value).unwrap());
        let next = page.prepare_native(300, 300, 1.0, true, None, Ok).unwrap();
        assert_eq!(next.nodes[2].text, value);
        assert_eq!(next.nodes[1].input.as_ref().unwrap().value, value);
        assert!(!page.text_event(generation, handle, "stale").unwrap());
        let handle = next.nodes[1].input.as_ref().unwrap().on_input.unwrap();
        assert!(
            page.text_event(page.generation(), handle, &"x".repeat(65537))
                .is_err()
        );
    }

    #[test]
    fn bridge_requires_ack_ready_and_state_then_roundtrips_actions() {
        use serde_json::json;
        let source = source(
            "function render({data,actions}){const result=useLocal('result','');return h('button',{onClick:async()=>{result.set(await actions.increment({step:1}))}},`${data.count}/${result.value}`)}",
        ) + "\nglobalThis.__lingxiaActionCatalog={__names:['increment'],__modes:{increment:'call'}};";
        let mut page = ViewSession::new_with_logic(&source, true).unwrap();
        assert_eq!(
            page.begin_bridge("native-nonce").unwrap(),
            json!({"v":2,"kind":"hello","nonce":"native-nonce","role":"view","protocolsSupported":[2]})
        );
        let ready = json!({"v":2,"kind":"ready","sessionId":"native-session"});
        assert!(page.receive_bridge(&ready).is_err());
        assert!(
            page.receive_bridge(&json!({"v":3,"kind":"helloAck"}))
                .is_err()
        );
        assert!(page.receive_bridge(&json!({"v":2,"kind":"helloAck","nonce":"wrong","protocol":2,"sessionId":"native-session"})).is_err());
        page.receive_bridge(&json!({"v":2,"kind":"helloAck","nonce":"native-nonce","protocol":2,"sessionId":"native-session"})).unwrap();
        let update = page
            .receive_bridge(&json!({"v":2,"kind":"state.snapshot","rev":0,"state":{"count":0}}))
            .unwrap();
        assert!(!update.changed);
        assert!(page.layout(300, 500, 1.0).is_err());
        assert!(page.receive_bridge(&ready).unwrap().changed);
        let frame = page.layout(300, 500, 1.0).unwrap();
        page.click(page.generation(), frame.nodes[0].on_click.unwrap())
            .unwrap();
        let request: serde_json::Value =
            serde_json::from_str(&page.take_action_requests().unwrap()[0]).unwrap();
        let response = json!({"v":2,"kind":"res","id":request["id"],"ok":true,"result":"done"});
        assert!(page.receive_bridge(&response).unwrap().changed);
        let patch = json!({"v":2,"kind":"state.patch","baseRev":0,"rev":1,"ack":true,"ops":[{"op":"replace","path":"/count","value":1}]});
        assert_eq!(
            page.receive_bridge(&patch).unwrap().outbound,
            vec![json!({"v":2,"kind":"state.ack","rev":1})]
        );
        assert_eq!(page.layout(300, 500, 1.0).unwrap().nodes[0].text, "1/done");
        page.begin_bridge("replacement").unwrap();
        assert!(!page.ready_to_render());
        assert!(page.receive_bridge(&response).is_err());
    }

    #[test]
    fn radio_binding_commits_exclusive_selection_and_rejects_stale_frames() {
        let source = source(
            "function render(){const pace=useLocal('pace','daily');return h('radio',{bind:pace,options:[{value:'daily',label:'Daily'},{value:'weekly',label:'Weekly'}]})}",
        );
        let mut session = ViewSession::new(&source).unwrap();
        let first = session.layout(240, 140, 1.0).unwrap();
        let generation = session.generation();
        let handle = first.nodes[2].on_click.unwrap();
        assert_eq!(first.nodes[1].checked, Some(true));
        assert!(session.click(generation, handle).unwrap());
        let second = session.layout(240, 140, 1.0).unwrap();
        assert_eq!(second.nodes[1].checked, Some(false));
        assert_eq!(second.nodes[2].checked, Some(true));
        assert!(!session.click(generation, handle).unwrap());
    }

    #[test]
    fn boolean_binding_updates_only_on_accepted_current_generation_activation() {
        let source = source(
            "function render(){const v=useLocal('checked',false);return h('checkbox',{bind:v},'Preference')}",
        );
        let mut session = ViewSession::new(&source).unwrap();
        let first = session.layout(240, 100, 1.0).unwrap();
        let generation = session.generation();
        let handle = first.nodes[0].on_click.unwrap();
        assert_eq!(first.nodes[0].checked, Some(false));
        assert!(session.click(generation, handle).unwrap());
        let second = session.layout(240, 100, 1.0).unwrap();
        assert_eq!(second.nodes[0].checked, Some(true));
        assert!(!session.click(generation, handle).unwrap());
        assert!(
            session
                .click(session.generation(), second.nodes[0].on_click.unwrap())
                .unwrap()
        );
        assert_eq!(
            session.layout(240, 100, 1.0).unwrap().nodes[0].checked,
            Some(false)
        );
    }

    #[test]
    fn action_response_runs_async_continuation_on_view_worker() {
        let source = source(
            "function render({actions}){const n=useLocal('n',0);return h('button',{onClick:async()=>{const result=await actions.increment({step:1});n.set(result.count)}},n.value)}",
        );
        let mut session = ViewSession::new(&source).unwrap();
        session
            .configure_actions(
                &serde_json::json!({"__names":["increment"],"__modes":{"increment":"call"}}),
            )
            .unwrap();
        let frame = session.layout(300, 500, 1.0).unwrap();
        session
            .click(session.generation(), frame.nodes[0].on_click.unwrap())
            .unwrap();
        let requests = session.take_action_requests().unwrap();
        assert_eq!(requests.len(), 1);
        let request: serde_json::Value = serde_json::from_str(&requests[0]).unwrap();
        assert_eq!(request["params"]["step"], 1);
        let response = serde_json::json!({"v":2,"kind":"res","id":request["id"],"ok":true,"result":{"count":7}});
        assert!(session.receive_action_response(&response).unwrap());
        assert!(!session.receive_action_response(&response).unwrap());
        assert_eq!(session.layout(300, 500, 1.0).unwrap().nodes[0].text, "7");
        assert!(session.take_action_errors().unwrap().is_empty());
    }

    #[test]
    fn leave_request_updates_view_and_waits_for_explicit_confirmation() {
        use serde_json::{Value, json};
        let source = source(
            "function render(){const guard=useLeaveGuard('draft',true);return h('button',{onClick:()=>guard.answer(true)},guard.pending??'editing')}",
        );
        let mut session = ViewSession::new(&source).unwrap();
        session
            .configure_actions(&json!({"__names":[],"__modes":{}}))
            .unwrap();
        session.layout(300, 500, 1.0).unwrap();
        let requests = session.take_action_requests().unwrap();
        let registration: Value = serde_json::from_str(&requests[0]).unwrap();
        assert_eq!(registration["method"], "host.navigation.setLeaveGuard");
        session
            .receive_action_response(
                &json!({"v":2,"kind":"res","id":registration["id"],"ok":true,"result":null}),
            )
            .unwrap();
        assert!(session.request_leave("invalid").is_err());
        session.request_leave("home").unwrap();
        assert!(session.has_local_updates().unwrap());
        let frame = session.layout(300, 500, 1.0).unwrap();
        assert_eq!(frame.nodes[0].text, "home");
        assert!(session.take_action_requests().unwrap().is_empty());
        session
            .click(session.generation(), frame.nodes[0].on_click.unwrap())
            .unwrap();
        let requests = session.take_action_requests().unwrap();
        let leave: Value = serde_json::from_str(&requests[0]).unwrap();
        assert_eq!(leave["method"], "host.navigation.leave");
        assert_eq!(leave["params"]["reason"], "home");
    }

    #[test]
    fn environment_is_atomic_immutable_and_survives_reset() {
        let source = source(
            "function render({env}){if(!Object.isFrozen(env.insets))throw Error('mutable');return h('text',{},JSON.stringify(env))}",
        );
        let mut page = ViewSession::new(&source).unwrap();
        let environment = serde_json::json!({"theme":"dark","insets":{"top":24,"right":0,"bottom":16,"left":0},"keyboard":{"height":0},"os":"android","formFactor":"phone","aside":false,"displayLanguage":"zh-CN","runner":false,"viewport":{"width":999}});
        page.set_environment(environment.clone()).unwrap();
        let mut invalid = environment.clone();
        invalid["insets"]["top"] = serde_json::json!(-1);
        assert!(page.set_environment(invalid).is_err());
        page.reset().unwrap();
        let frame = page.layout(900, 1200, 3.0).unwrap();
        let actual: serde_json::Value = serde_json::from_str(&frame.nodes[0].text).unwrap();
        assert_eq!(actual["theme"], "dark");
        assert_eq!(actual["insets"], environment["insets"]);
        assert_eq!(actual["displayLanguage"], "zh-CN");
        assert_eq!(actual["viewport"]["width"], 300.0);
        assert_eq!(actual["sizeClass"], "compact");
        let frame = page.layout(1200, 900, 1.0).unwrap();
        let actual: serde_json::Value = serde_json::from_str(&frame.nodes[0].text).unwrap();
        assert_eq!(actual["sizeClass"], "regular");
        let mut host = environment.clone();
        host["sizeClass"] = serde_json::json!("compact");
        page.set_environment(host.clone()).unwrap();
        host["sizeClass"] = serde_json::json!("invalid");
        assert!(page.set_environment(host).is_err());
        let frame = page.layout(1200, 900, 1.0).unwrap();
        let actual: serde_json::Value = serde_json::from_str(&frame.nodes[0].text).unwrap();
        assert_eq!(actual["sizeClass"], "compact");
        assert_eq!(actual["viewport"]["width"], 1200.0);
        let mut other = ViewSession::new(&source).unwrap();
        let frame = other.layout(300, 500, 1.0).unwrap();
        let actual: serde_json::Value = serde_json::from_str(&frame.nodes[0].text).unwrap();
        assert_eq!(actual["theme"], "light");
    }

    #[test]
    fn session_retains_state_and_rejects_stale_frame_input() {
        let source = source(
            "function render(){const n=useLocal('count',0);return h('button',{testId:'counter',onClick:()=>n.set(n.value+1)},n.value)}",
        );
        let mut session = ViewSession::new(&source).unwrap();
        let first = session.render(300, 500, 1.0).unwrap();
        assert_eq!(first.nodes[0].text, "0");
        assert!(session.click(1, first.nodes[0].on_click.unwrap()).unwrap());
        let second = session.render(300, 500, 1.0).unwrap();
        assert_eq!(second.nodes[0].text, "1");
        assert!(!session.click(1, first.nodes[0].on_click.unwrap()).unwrap());
        assert_eq!(session.render(500, 300, 1.0).unwrap().nodes[0].text, "1");
    }

    #[test]
    fn snapshot_gates_first_render_and_ack_does_not_depend_on_render_success() {
        use serde_json::json;
        let source = source(
            "function render({data}){if(data.fail)throw Error('bad render');return h('text',{},data.count)}",
        );
        let mut session = ViewSession::new_with_logic(&source, true).unwrap();
        assert!(
            session
                .render(300, 500, 1.0)
                .err()
                .unwrap()
                .contains("waiting for first snapshot")
        );
        session
            .receive_state(&json!({"v":2,"kind":"state.snapshot","rev":0,"state":{"count":0}}))
            .unwrap();
        assert_eq!(session.render(300, 500, 1.0).unwrap().nodes[0].text, "0");
        let update=session.receive_state(&json!({"v":2,"kind":"state.patch","baseRev":0,"rev":1,"ack":true,"ops":[{"op":"add","path":"/fail","value":true}]})).unwrap();
        assert_eq!(update.outbound[0]["kind"], "state.ack");
        assert!(session.render(300, 500, 1.0).is_err());
        assert_eq!(session.generation(), 1);
        session.receive_state(&json!({"v":2,"kind":"state.patch","baseRev":1,"rev":2,"ops":[{"op":"remove","path":"/fail"},{"op":"replace","path":"/count","value":2}]})).unwrap();
        assert_eq!(session.render(300, 500, 1.0).unwrap().nodes[0].text, "2");
    }

    #[test]
    fn native_validation_failure_does_not_commit_state_or_handlers() {
        let source = source(
            "let bad=false;function render(){const n=useLocal('count',0);return h('button',{style:{fontSize:bad?0:14},onClick:()=>{n.set(n.value+1);bad=!bad}},n.value)}",
        );
        let mut session = ViewSession::new(&source).unwrap();
        let first = session.render(300, 500, 1.0).unwrap();
        let handle = first.nodes[0].on_click.unwrap();
        session.click(1, handle).unwrap();
        assert!(session.render(300, 500, 1.0).is_err());
        assert_eq!(session.generation(), 1);
        session.click(1, handle).unwrap();
        assert_eq!(session.render(300, 500, 1.0).unwrap().nodes[0].text, "1");
    }
}
