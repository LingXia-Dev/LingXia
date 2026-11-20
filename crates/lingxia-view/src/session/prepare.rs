use super::*;
use crate::interaction::scroll::FIELD_REVEAL_MARGIN;
use crate::session::state::PatchOp;

/// Where the whole data tree waits for the frame's `__lingxiaCreateView` to take it.
const VIEW_DATA: &str = "__lingxiaViewData";

impl ViewSession {
    /// The `data` argument for `__lingxiaCreateView`: `undefined` when the isolate's own copy
    /// was brought current by replaying the writes since its last frame, else the whole tree.
    /// Both reach the isolate as values built through the engine, never as JSON text.
    fn sync_view_data(&mut self, metrics: &mut crate::FrameMetrics) -> Result<String, String> {
        use rong::quickjs::{JSArray, JSFunc, JSObject};
        let revision = self.state.revision();
        let replayed = match self.state.journal() {
            Some((from, ops)) => {
                let started = std::time::Instant::now();
                let replayed = self.execute(|| {
                    let ctx = &self.context;
                    let fail = |e| crate::engine::js_error(ctx, e);
                    let Ok(sync) = ctx.global().get::<_, JSFunc>("__lingxiaSyncData") else {
                        return Ok(false);
                    };
                    let list = JSArray::new(ctx).map_err(fail)?;
                    for (index, op) in ops.iter().enumerate() {
                        let (kind, path, value) = match op {
                            PatchOp::Add { path, value } => ("add", path, Some(value)),
                            PatchOp::Replace { path, value } => ("replace", path, Some(value)),
                            PatchOp::Remove { path } => ("remove", path, None),
                        };
                        let object = JSObject::new(ctx);
                        object.set("op", kind).map_err(fail)?;
                        object.set("path", path.as_str()).map_err(fail)?;
                        if let Some(value) = value {
                            let value = crate::session::js_tree::to_js(ctx, value).map_err(fail)?;
                            object.set("value", value).map_err(fail)?;
                        }
                        list.set_value(index as u32, object.into_js_value())
                            .map_err(fail)?;
                    }
                    // Numbers, not BigInts: the isolate compares revisions with `===`.
                    sync.call::<_, bool>(None, (from as f64, revision as f64, list))
                        .map_err(fail)
                })?;
                let us = crate::session::metrics::micros(started);
                metrics.view_eval_us += us;
                metrics.view_us += us;
                replayed
            }
            None => false,
        };
        // A replay the isolate refused (fresh context) falls back to the whole tree; the
        // isolate checks the revision either way.
        let data = if replayed {
            "undefined".to_string()
        } else {
            let data = self
                .state
                .data()
                .ok_or("UI_STATE: waiting for first snapshot")?;
            self.execute(|| {
                let ctx = &self.context;
                let fail = |e| crate::engine::js_error(ctx, e);
                let data = crate::session::js_tree::to_js(ctx, data).map_err(fail)?;
                ctx.global().set(VIEW_DATA, data).map_err(fail)?;
                Ok(())
            })?;
            format!(
                "(() => {{ const d = globalThis.{VIEW_DATA}; globalThis.{VIEW_DATA} = undefined; return d; }})()"
            )
        };
        self.state.mark_synced();
        Ok(data)
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
        let frame = self.prepare_native_visuals(
            width,
            height,
            scale,
            rebuild,
            offsets,
            &mut FrameAnimations::Disabled,
            build,
        )?;
        // An unanimated frame replaces whatever animation candidates were waiting.
        self.pending_animation = None;
        self.presenting_animation = None;
        Ok(frame)
    }

    /// Render once and throw the frame away, so a module that cannot render is refused
    /// before it replaces one that can; its animations need no presentation.
    #[cfg(feature = "hot-swap")]
    pub(super) fn prepare_trial(
        &mut self,
        width: u16,
        height: u16,
        scale: f32,
    ) -> Result<(), String> {
        self.prepare_native_visuals(
            width,
            height,
            scale,
            true,
            None,
            &mut FrameAnimations::Trial,
            |_| Ok(()),
        )?;
        // The next real frame starts the new module's animations afresh.
        self.pending_animation = None;
        self.presenting_animation = None;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_native_visuals<T>(
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
        metrics.total_us = crate::session::metrics::micros(started);
        metrics.generation = self.generation.get();
        metrics.video_orientation = self.renderer.videos.orientation();
        if self.eval_phases {
            metrics.view_phases = self.take_eval_phases()?;
        }
        self.metrics = metrics;
        self.check_budgets();
        Ok(result)
    }

    /// Whether a presented frame's commit still names the current View: the latest commit,
    /// or an earlier one of the same layout (frames prepared ahead repaint it, so its
    /// handles and nodes are unchanged).
    pub(crate) fn presented_commit(&self, generation: lifecycle::CommitTicket) -> bool {
        generation == self.generation
            || (self.layout_commit.get() <= generation.get()
                && generation.get() <= self.generation.get())
    }

    /// The commit input made on `generation` applies to: itself while still current, else,
    /// when it is the frame the user saw last, the current View if `identity` is still in it.
    /// A frame prepared after that one must not cost the input; a control that is gone does.
    pub(crate) fn input_commit(
        &self,
        generation: lifecycle::CommitTicket,
        identity: &str,
        layout: bool,
    ) -> Option<lifecycle::CommitTicket> {
        let current = if layout {
            self.presented_layout_commit(generation)
        } else {
            self.presented_commit(generation)
        };
        if current {
            return Some(generation);
        }
        let present = !identity.is_empty()
            && generation.get() != 0
            && generation == self.seen_commit
            && self
                .retained
                .as_ref()
                .is_some_and(|retained| retained.nodes.iter().any(|n| n.identity == identity));
        present.then_some(self.generation)
    }

    /// Clicks handled since the last call.
    pub fn take_clicks(&mut self) -> Vec<ClickRecord> {
        std::mem::take(&mut self.clicks)
    }

    /// Editor geometry queries need the frame's exact layout, not only its nodes: a commit
    /// is valid while no relayout (focus, decoration, paint delta) followed it.
    pub(crate) fn presented_layout_commit(&self, generation: lifecycle::CommitTicket) -> bool {
        generation == self.generation
            || (self.retained_commit.get() <= generation.get()
                && generation.get() <= self.generation.get())
    }

    /// Frames with the same revision share one retained layout: same nodes in the same
    /// order, differing only in animated, scrolled or decorated geometry.
    pub fn layout_revision(&self) -> u64 {
        self.layout_revision
    }

    pub fn frame_metrics(&self) -> &crate::FrameMetrics {
        &self.metrics
    }

    /// Supply a fresh surface epoch after replacement; `None` parks the page.
    pub fn set_animation_surface(
        &mut self,
        surface: Option<lifecycle::SurfaceTicket>,
        now: Duration,
    ) -> Result<(), String> {
        self.set_animation_target(surface, surface.is_some(), now)
    }

    /// A hidden page renders catch-up frames into its kept surface with time parked:
    /// they sample the parked time and a change made meanwhile lands on its end value.
    pub fn set_animation_target(
        &mut self,
        surface: Option<lifecycle::SurfaceTicket>,
        visible: bool,
        now: Duration,
    ) -> Result<(), String> {
        let mut candidate = self.animations.fork();
        let visible = visible && surface.is_some();
        let retarget = surface != self.animation_surface;
        if retarget {
            candidate.set_visible(false, now)?;
        }
        // A visibility flip invalidates every waiting frame, so none may be built on.
        let flipped = candidate.visible() != visible;
        candidate.set_visible(visible, now)?;
        self.animations = candidate;
        if retarget || flipped {
            self.pending_animation = None;
            self.presenting_animation = None;
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
        self.presenting_animation = None;
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
        self.prepare_view_sampled(
            width, height, scale, rebuild, offsets, now, now, false, build,
        )
    }

    /// A focused field waits for a frame to be revealed.
    pub fn wants_reveal(&self) -> bool {
        self.reveal_input
    }

    /// The host's scroll input revision for the frames it requests next: a reveal holds its
    /// offsets over older requests made under the same revision.
    pub fn set_offsets_revision(&mut self, revision: u64) {
        self.offsets_revision = Some(revision);
    }

    /// Like `prepare_view_animated`, sampling at `sample` (when the frame is expected on
    /// screen). With `ahead`, the frame builds on the newest unpresented frame instead of
    /// replacing it; at most one frame may wait under it.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_view_sampled<T>(
        &mut self,
        width: u16,
        height: u16,
        scale: f32,
        rebuild: bool,
        offsets: Option<&HashMap<String, f32>>,
        now: Duration,
        sample: Duration,
        ahead: bool,
        build: impl FnOnce(DisplayList) -> Result<T, String>,
    ) -> Result<T, String> {
        let surface = self.animation_surface.ok_or("UI_ANIMATION: parked page")?;
        let base = if ahead {
            if self.presenting_animation.is_some() {
                return Err(super::AHEAD_REFUSED.into());
            }
            // A hidden page's frames park time: nothing carries over, so it starts fresh.
            self.pending_animation
                .as_ref()
                .filter(|(_, s, _)| *s == surface && self.animations.visible())
                .map(|(_, _, frame)| Box::new(frame.clone()))
        } else {
            None
        };
        let mut timeline = self.animations.fork();
        // A zero scale already reads as reduced motion in the environment.
        let duration_scale = self.environment["animatorDurationScale"]
            .as_f64()
            .filter(|scale| *scale > 0.0)
            .unwrap_or(1.0);
        timeline.set_motion(crate::animation::Motion {
            reduced: self.environment["reducedMotion"].as_bool() == Some(true),
            scale: duration_scale.clamp(0.01, 10.0),
        })?;
        let mut animation = FrameAnimations::Collected {
            inventory: self.animation_inventory.clone(),
            timeline: Box::new(timeline),
            now,
            sample,
            base: base.clone(),
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
        self.animations = *timeline;
        self.animation_visible = visible;
        self.metrics.ghosts = frame.as_ref().map_or(0, |frame| frame.ghosts().len());
        self.metrics.layout_motions = frame.as_ref().map_or(0, |frame| frame.layout_motions());
        let frame = frame.map(|frame| (self.generation, surface, *frame));
        if base.is_some() && frame.is_some() {
            self.presenting_animation = self.pending_animation.take();
        } else {
            self.presenting_animation = None;
        }
        self.pending_animation = frame;
        self.metrics.animation_frames = self.animations.frames_per_second(now);
        Ok(result)
    }

    pub fn set_animation_covered(&mut self, covered: bool) {
        self.animations.set_covered(covered);
    }

    pub fn view_animation_wakeup(&self, now: Duration) -> Result<crate::animation::Wakeup, String> {
        self.animations
            .next_wakeup(now, self.animation_visible.iter().map(|id| &**id))
    }

    /// Whether motion that ends is still running: endless animations do not count.
    pub fn view_animation_moving(&self, now: Duration) -> Result<bool, String> {
        Ok(self
            .animations
            .finite_wakeup(now, self.animation_visible.iter().map(|id| &**id))?
            == crate::animation::Wakeup::NextFrame)
    }

    /// Acknowledge on the owning View thread after native presentation succeeds.
    /// Stale acknowledgements leave a newer candidate intact.
    pub fn animation_presented(
        &mut self,
        surface: lifecycle::SurfaceTicket,
        generation: lifecycle::CommitTicket,
        now: Duration,
    ) -> Result<bool, String> {
        if generation.get() > self.seen_commit.get() {
            self.seen_commit = generation;
        }
        crate::layout::list_layout::presented(&mut self.lists, generation.get());
        let matches = |candidate: &Option<(
            lifecycle::CommitTicket,
            lifecycle::SurfaceTicket,
            crate::animation::Prepared,
        )>| {
            candidate
                .as_ref()
                .is_some_and(|(g, s, _)| *g == generation && *s == surface)
        };
        if self.animation_surface != Some(surface) {
            return Ok(false);
        }
        // The older frame presents first; the newer one, built on it, stays pending.
        let slot = if matches(&self.presenting_animation) {
            &mut self.presenting_animation
        } else if matches(&self.pending_animation) {
            self.presenting_animation = None;
            &mut self.pending_animation
        } else {
            return Ok(false);
        };
        self.animations
            .validate_presentation(&slot.as_ref().unwrap().2, now)?;
        let (_, _, frame) = slot.take().unwrap();
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
    pub(super) fn prepare_measured<T>(
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
            && (self.input_dirty || self.pointer_dirty)
            && self.input_records.is_some()
            && self.viewport == Some((width, height, scale));
        let mut offsets = offsets.unwrap_or(&self.scroll_offsets).clone();
        // The host's offsets predate an anchor it has not shown a frame for: there the list is
        // where this session last put it.
        for id in crate::layout::list_layout::unsettled(&self.lists) {
            if let Some(offset) = self.scroll_offsets.get(id) {
                offsets.insert(id.clone(), *offset);
            }
        }
        // A reveal the host has not seen yet holds; scroll input since the reveal wins, and an
        // offset the host already requests needs no holding.
        if self.revealed_revision.is_none() || self.revealed_revision != self.offsets_revision {
            self.revealed_offsets.clear();
        }
        self.revealed_offsets.retain(|id, to| {
            let requested = offsets.get(id).copied().unwrap_or(0.0);
            if (requested - *to).abs() < 0.5 {
                false
            } else {
                offsets.insert(id.clone(), *to);
                true
            }
        });
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
        if !rebuild
            && !self.input_dirty
            && !self.pointer_dirty
            && self.viewport == Some((width, height, scale))
            && let Some(retained) = &self.retained
        {
            if let Some(records) = &self.input_records {
                animation.validate(records)?;
            }
            let started = std::time::Instant::now();
            let mut display = retained.compose(&offsets)?;
            metrics.compose_us += crate::session::metrics::micros(started);
            window_only = crate::layout::list_layout::scroll_windows(&display, &self.lists)?;
            if window_only.is_none() {
                let started = std::time::Instant::now();
                display = animation
                    .compose(
                        self.input_records.as_deref().unwrap_or_default(),
                        display,
                        &offsets,
                        &self.renderer.palette,
                    )?
                    .0;
                metrics.compose_us += crate::session::metrics::micros(started);
                let applied = applied_offsets(&display);
                let wanted = display.image_sources().map(str::to_owned).collect();
                display.bind_images(&self.images)?;
                let mut inputs = self.inputs.clone();
                let old_focus = inputs.focused().map(str::to_owned);
                inputs.decorate(&mut display, &mut self.renderer)?;
                let changed_style = old_focus.as_deref() != inputs.focused()
                    && self.input_records.as_ref().is_some_and(|records| {
                        records.iter().any(|r| {
                            r.string(field::FOCUS_STYLE).is_some()
                                || r.string(field::FOCUS_VISIBLE_STYLE).is_some()
                                || r.string(field::FOCUS_WITHIN_STYLE).is_some()
                        })
                    });
                if !changed_style {
                    let input_dirty = inputs.needs_layout(&display);
                    let input_hits = crate::interaction::input_hits::InputHitMap::capture(&display);
                    metrics.nodes = display.nodes.len();
                    metrics.paints = display.paint_count();
                    metrics.cost = display.cost();
                    metrics.top_layer_open = display.top_layer_open();
                    metrics.chrome_snapshot = self.chrome.attach(&mut display);
                    let started = std::time::Instant::now();
                    let frame = build(display)?;
                    metrics.build_us += crate::session::metrics::micros(started);
                    self.input_hits = Some(input_hits);
                    self.inputs = inputs;
                    self.input_dirty = input_dirty;
                    self.pointer_dirty = false;
                    self.commit_images(wanted);
                    self.scroll_offsets = applied;
                    self.advance();
                    return Ok(frame);
                }
            }
        }
        let windows = window_only
            .as_ref()
            .cloned()
            .unwrap_or_else(|| crate::layout::list_layout::current(&self.lists));
        let encoded_windows =
            serde_json::to_string(&serde_json::to_string(&windows).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let invoke = if native_input {
            String::new()
        } else if window_only.is_some() {
            format!("__lingxiaPrepareWindows(JSON.parse({encoded_windows}))")
        } else {
            self.render_invoke(width, height, scale, &encoded_windows, metrics)?
        };
        let mut prepared_js = !native_input;
        let mut revealed = HashMap::new();
        let mut keyboard_reveal = self.keyboard_reveal.clone();
        let candidate = (|| {
            let mut records = if native_input {
                self.input_records.as_ref().unwrap().clone()
            } else {
                self.render_records(
                    &invoke,
                    window_only.is_none(),
                    &windows,
                    &mut offsets,
                    metrics,
                )?
            };
            let mut orphan_closed = false;
            let mut inputs = self.inputs.clone();
            let mut uncontrolled = self.uncontrolled.clone();
            let mut lists = self.lists.clone();
            for _ in 0..8 {
                animation.validate(&records)?;
                inputs.reconcile(&records, &mut self.renderer)?;
                uncontrolled.reconcile(&records);
                let mut render_records = records.to_vec();
                strip_record_animations(&mut render_records);
                inputs.apply_values(&mut render_records);
                uncontrolled.apply(&mut render_records);
                inputs.apply_styles(&mut render_records)?;
                self.apply_pointer_styles(&mut render_records)?;
                self.renderer.pressed.clone_from(&self.pressed);
                self.renderer.focused.clone_from(&self.focused_control);
                self.renderer.editing = inputs.focused().map(str::to_owned);
                let started = std::time::Instant::now();
                let reusable = if !self.input_dirty
                    && !self.pointer_dirty
                    && self.viewport == Some((width, height, scale))
                {
                    self.retained
                        .as_ref()
                        .zip(self.paint_records.as_deref())
                        .and_then(|(retained, previous)| retained.rebind(previous, &render_records))
                } else {
                    None
                };
                let mut display = if let Some(retained) = reusable {
                    let display = retained.compose(&offsets)?;
                    metrics.compose_us += crate::session::metrics::micros(started);
                    display
                } else {
                    let display = self.renderer.layout_scrolled(
                        &render_records,
                        width,
                        height,
                        scale,
                        &offsets,
                    )?;
                    metrics.layout_us += crate::session::metrics::micros(started);
                    metrics.text_us += self.renderer.text_time.as_micros() as u64;
                    metrics.layout_passes += 1;
                    display
                };
                // An overlay whose editor or invoker unmounted goes with it, in this frame, as
                // when that control blurs.
                if !native_input
                    && !orphan_closed
                    && let Some(close) = self.orphaned_overlays(&records, &display)?
                {
                    orphan_closed = true;
                    self.eval(&close)?;
                    let invoke =
                        self.render_invoke(width, height, scale, &encoded_windows, metrics)?;
                    records =
                        self.render_records(&invoke, true, &windows, &mut offsets, metrics)?;
                    prepared_js = true;
                    continue;
                }
                if reveal_input && let Some(identity) = inputs.focused() {
                    let started = std::time::Instant::now();
                    let before = offsets.clone();
                    display = display.reveal(identity, FIELD_REVEAL_MARGIN, &mut offsets)?;
                    let keyboard = self.renderer.safe_area.keyboard > 0.0;
                    let at = |offsets: &HashMap<String, f32>, id: &str| {
                        offsets.get(id).copied().unwrap_or(0.0)
                    };
                    // The same field, its scroll where the last reveal left it: a keyboard that
                    // shrank since leaves too much room, so the field settles back down.
                    if keyboard
                        && let Some(memo) = keyboard_reveal
                            .as_ref()
                            .filter(|memo| memo.identity == identity)
                        && (at(&before, &memo.scroll) - memo.set).abs() < 0.5
                        && (at(&offsets, &memo.scroll) - memo.set).abs() < 0.5
                    {
                        display = display.settle_reveal(
                            identity,
                            FIELD_REVEAL_MARGIN,
                            &mut offsets,
                            &memo.scroll,
                            memo.origin,
                        )?;
                    }
                    let scroll = display
                        .nodes
                        .iter()
                        .find(|node| node.identity == identity)
                        .and_then(|node| node.scroll_parent.clone());
                    for (id, to) in &offsets {
                        if (to - at(&before, id)).abs() > 0.01 {
                            revealed.insert(id.clone(), *to);
                        }
                    }
                    keyboard_reveal = match scroll {
                        Some(scroll) if keyboard && revealed.contains_key(&scroll) => {
                            let to = revealed[&scroll];
                            let origin = keyboard_reveal
                                .as_ref()
                                .filter(|memo| memo.identity == identity && memo.scroll == scroll)
                                .map_or(at(&before, &scroll), |memo| memo.origin);
                            Some(KeyboardReveal {
                                identity: identity.to_owned(),
                                scroll,
                                origin,
                                set: to,
                            })
                        }
                        _ if keyboard => keyboard_reveal
                            .take()
                            .filter(|memo| memo.identity == identity),
                        _ => None,
                    };
                    metrics.compose_us += crate::session::metrics::micros(started);
                } else if reveal_input {
                    // The focus lands in the frame after the keyboard's inset when the platform
                    // raises the keyboard first; that frame reveals the field.
                    keyboard_reveal = None;
                }
                let old_focus = inputs.focused().map(str::to_owned);
                inputs.decorate(&mut display, &mut self.renderer)?;
                if old_focus.as_deref() != inputs.focused()
                    && records.iter().any(|r| {
                        r.string(field::FOCUS_STYLE).is_some()
                            || r.string(field::FOCUS_VISIBLE_STYLE).is_some()
                            || r.string(field::FOCUS_WITHIN_STYLE).is_some()
                    })
                {
                    continue;
                }
                if let Some(windows) = crate::layout::list_layout::reconcile(
                    &records,
                    &mut display,
                    &mut lists,
                    &mut offsets,
                )? {
                    let encoded = serde_json::to_string(
                        &serde_json::to_string(&windows).map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?;
                    prepared_js = true;
                    let started = std::time::Instant::now();
                    let (refined, timings) = self.execute_frame(|| {
                        decode_buffer(
                            &self.context,
                            &format!("__lingxiaPrepareWindows(JSON.parse({encoded}))"),
                            false,
                            Some(&mut self.wire.borrow_mut()),
                        )
                    })?;
                    records = refined;
                    metrics.add_view(timings);
                    metrics.view_us += crate::session::metrics::micros(started);
                    metrics.view_passes += 1;
                } else {
                    let applied = applied_offsets(&display);
                    let retained = Arc::clone(&display.retained);
                    let started = std::time::Instant::now();
                    let (composed, redecorate) =
                        animation.compose(&records, display, &applied, &self.renderer.palette)?;
                    display = composed;
                    metrics.compose_us += crate::session::metrics::micros(started);
                    if redecorate {
                        inputs.decorate(&mut display, &mut self.renderer)?;
                    }
                    crate::layout::list_layout::commit(&mut lists, self.generation.get() + 1);
                    let wanted = display.image_sources().map(str::to_owned).collect();
                    display.bind_images(&self.images)?;
                    let input_dirty = inputs.needs_layout(&display);
                    let input_hits = crate::interaction::input_hits::InputHitMap::capture(&display);
                    metrics.nodes = display.nodes.len();
                    metrics.paints = display.paint_count();
                    metrics.cost = display.cost();
                    metrics.top_layer_open = display.top_layer_open();
                    metrics.chrome_snapshot = self.chrome.attach(&mut display);
                    let started = std::time::Instant::now();
                    let frame = build(display)?;
                    metrics.build_us += crate::session::metrics::micros(started);
                    return Ok({
                        (
                            frame,
                            retained,
                            applied,
                            lists,
                            wanted,
                            (inputs, uncontrolled),
                            records,
                            render_records,
                            input_dirty,
                            input_hits,
                        )
                    });
                }
            }
            Err(String::from("UI_LIST: window measurement did not converge"))
        })();
        let candidate = candidate.and_then(|candidate| {
            if prepared_js {
                let started = std::time::Instant::now();
                self.eval("__lingxiaCommit()")?;
                metrics.view_us += crate::session::metrics::micros(started);
                self.lowered_due.set(true);
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
                (inputs, uncontrolled),
                records,
                render_records,
                input_dirty,
                input_hits,
            )) => {
                let same_geometry = self
                    .input_hits
                    .as_ref()
                    .is_some_and(|old| old.same_geometry(&input_hits));
                self.input_hits = Some(input_hits);
                self.paint_records = Some(render_records);
                self.inputs = inputs;
                self.uncontrolled = uncontrolled;
                let same_view = self
                    .input_records
                    .as_deref()
                    .is_some_and(|old| same_structure(old, &records));
                self.input_records = Some(records);
                self.input_dirty = input_dirty;
                self.pointer_dirty = false;
                self.input_rebuild = false;
                self.reveal_input = false;
                self.pending_inputs = None;
                self.commit_images(wanted);
                let retained_changed = !self
                    .retained
                    .as_ref()
                    .is_some_and(|old| Arc::ptr_eq(old, &retained));
                // A paint delta (pressed, hovered) relays the page out but keeps its nodes and
                // handles, so a tap on the previous frame stays valid.
                let relaid = !same_view && retained_changed;
                if relaid {
                    self.layout_revision += 1;
                }
                self.retained = Some(retained);
                self.lists = lists;
                self.viewport = Some((width, height, scale));
                self.scroll_offsets = applied;
                self.keyboard_reveal = keyboard_reveal;
                if !revealed.is_empty() && self.offsets_revision.is_some() {
                    self.revealed_offsets.extend(revealed);
                    self.revealed_revision = self.offsets_revision;
                }
                self.advance();
                if relaid {
                    self.layout_commit = self.generation;
                }
                if retained_changed {
                    // A repaint that left every editor where it was (a pressed field's own
                    // fill) keeps the tap's frame good for its caret hit test.
                    if !(same_view && same_geometry) {
                        self.retained_commit = self.generation;
                    }
                    self.note_boxes();
                }
                if prepared_js {
                    self.uncollected.set(self.uncollected.get() + 1);
                }
                Ok(frame)
            }
            Err(error) => {
                if prepared_js {
                    // A frame the target only declined keeps the user's pending state; any
                    // other failure reverts it, and its error tells the author why.
                    // A dropped frame keeps it too: the next frame renders it again.
                    let _ = self.eval(if error == FRAME_DECLINED || error.starts_with(OVERRUN) {
                        "__lingxiaDiscard(true)"
                    } else {
                        "__lingxiaDiscard()"
                    });
                }
                // A frame the target declined or dropped is not the user's mistake: the focus
                // and text they gave stay, and the next frame shows them (and reveals the field).
                let kept = error == FRAME_DECLINED || error.starts_with(OVERRUN);
                if kept {
                    self.input_dirty = true;
                } else if let Some(mut previous) = self.pending_inputs.take() {
                    previous.invalidate_after(&self.inputs)?;
                    self.inputs = previous;
                    self.input_dirty = true;
                    self.input_rebuild = false;
                }
                Err(error)
            }
        }
    }

    /// The full render call for a frame of `width` x `height` device pixels.
    fn render_invoke(
        &mut self,
        width: u16,
        height: u16,
        scale: f32,
        encoded_windows: &str,
        metrics: &mut crate::FrameMetrics,
    ) -> Result<String, String> {
        let width_dp = width as f32 / scale;
        let height_dp = height as f32 / scale;
        let data = self.sync_view_data(metrics)?;
        let mut environment = self.environment.clone();
        // Platform interpolation samples are an internal layout contract.
        environment.as_object_mut().unwrap().remove("textScale");
        environment["viewport"] = serde_json::json!({"width":width_dp,"height":height_dp});
        // Crossing the size class lays the page out anew, as an appearance change repaints it.
        let compact = match environment
            .get("sizeClass")
            .and_then(|class| class.as_str())
        {
            Some(class) => class == "compact",
            None => width_dp < 600.0,
        };
        if self
            .compact
            .replace(compact)
            .is_some_and(|was| was != compact)
        {
            self.warm.set(true);
        }
        if environment.get("sizeClass").is_none() {
            environment["sizeClass"] = serde_json::json!(if width_dp < 600.0 {
                "compact"
            } else {
                "regular"
            });
        }
        let environment =
            serde_json::to_string(&environment.to_string()).map_err(|e| e.to_string())?;
        Ok(format!(
            "__lingxiaPrepare(__lingxiaRender, __lingxiaCreateView({data}, JSON.parse({environment}), {revision}), JSON.parse({encoded_windows}), true)",
            revision = self.state.revision(),
        ))
    }

    /// Runs `invoke` and decodes its records; a full render whose anchors moved encodes once
    /// more, into the windows they land in.
    fn render_records(
        &mut self,
        invoke: &str,
        full: bool,
        windows: &crate::layout::list_layout::Windows,
        offsets: &mut HashMap<String, f32>,
        metrics: &mut crate::FrameMetrics,
    ) -> Result<std::sync::Arc<[crate::protocol::Record]>, String> {
        let started = std::time::Instant::now();
        let (mut records, timings) = self.execute_frame(|| {
            decode_buffer(
                &self.context,
                invoke,
                full,
                Some(&mut self.wire.borrow_mut()),
            )
        })?;
        metrics.add_view(timings);
        if full && records.is_empty() {
            let moved: String = self.execute(|| {
                self.context
                    .eval(Source::from_bytes("__lingxiaTakeMovedAnchors()"))
                    .map_err(|e| crate::engine::js_error(&self.context, e))
            })?;
            let moved: HashMap<String, String> =
                serde_json::from_str(&moved).map_err(|e| e.to_string())?;
            let mut windows = windows.clone();
            if let Some(retained) = &self.retained {
                crate::layout::list_layout::anchor_windows(
                    &self.lists,
                    &retained.nodes,
                    &moved,
                    &mut windows,
                    offsets,
                )?;
            }
            let encoded =
                serde_json::to_string(&serde_json::to_string(&windows).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let (refined, timings) = self.execute_frame(|| {
                decode_buffer(
                    &self.context,
                    &format!("__lingxiaPrepareWindows(JSON.parse({encoded}))"),
                    false,
                    Some(&mut self.wire.borrow_mut()),
                )
            })?;
            metrics.add_view(timings);
            records = refined;
        }
        metrics.view_us += crate::session::metrics::micros(started);
        metrics.view_passes += 1;
        // A bundle older than the appearance bit is taken to read it.
        let reads = self.execute(|| {
            self.context
                .eval::<i32>(Source::from_bytes(
                    "typeof __lingxiaEnvReads === 'function' ? __lingxiaEnvReads() : (typeof __lingxiaReadsHostContext !== 'function' || __lingxiaReadsHostContext() ? 3 : 2)",
                ))
                .map_err(|e| crate::engine::js_error(&self.context, e))
        })?;
        self.reads_host_context = reads & 1 != 0;
        self.reads_appearance = reads & 2 != 0;
        Ok(records)
    }

    /// The call closing every open picker whose editor, and popover whose invoker, is not in
    /// this layout; a popover opened from code has no owner and stays.
    fn orphaned_overlays(
        &self,
        records: &[crate::protocol::Record],
        display: &DisplayList,
    ) -> Result<Option<String>, String> {
        #[derive(serde::Deserialize)]
        struct Owned {
            picker: Option<String>,
            popovers: Vec<String>,
        }
        if !display.retained.top_layer.iter().any(|top| *top) {
            return Ok(None);
        }
        // A bundle older than the list still names its picker's editor.
        let owned: String = self.execute(|| {
            self.context
                .eval(Source::from_bytes(
                    "typeof __lingxiaOwnedOverlays === 'function' ? __lingxiaOwnedOverlays() : JSON.stringify({picker: typeof __lingxiaPickerIdentity === 'function' ? __lingxiaPickerIdentity() : null, popovers: []})",
                ))
                .map_err(|e| crate::engine::js_error(&self.context, e))
        })?;
        let owned: Owned = serde_json::from_str(&owned).map_err(|e| e.to_string())?;
        let picker = owned.picker.is_some_and(|editor| {
            !display
                .retained
                .nodes
                .iter()
                .any(|node| *node.identity == *editor)
        });
        // The invoker that opened a popover carries its anchor name while it is mounted.
        let popovers: Vec<&str> = owned
            .popovers
            .iter()
            .filter(|id| {
                let name = format!("--lx-popover-{id}");
                !records
                    .iter()
                    .any(|record| crate::protocol::anchors(record, &name))
            })
            .map(String::as_str)
            .collect();
        if !picker && popovers.is_empty() {
            return Ok(None);
        }
        let ids = serde_json::to_string(&popovers).map_err(|e| e.to_string())?;
        Ok(Some(format!(
            "typeof __lingxiaCloseOrphans === 'function' ? __lingxiaCloseOrphans({picker}, {ids}) : __lingxiaClosePicker()"
        )))
    }
}

/// The same nodes in the same order with the same callbacks: frames of either describe
/// one View, whatever else (paint, text, geometry) differs.
fn same_structure(old: &[crate::protocol::Record], new: &[crate::protocol::Record]) -> bool {
    let handle = |id: usize| {
        crate::protocol::FIELDS
            .get(id)
            .is_some_and(|(_, kind)| kind.ends_with("handle"))
    };
    old.len() == new.len()
        && old.iter().zip(new).all(|(a, b)| {
            a.tag == b.tag
                && a.parent == b.parent
                && a.key == b.key
                && a.fields
                    .iter()
                    .filter(|(id, _)| handle(*id))
                    .eq(b.fields.iter().filter(|(id, _)| handle(*id)))
        })
}

pub(super) fn strip_record_animations(records: &mut [crate::protocol::Record]) {
    let stripped = |id: usize| matches!(id, field::ANIMATION | field::LIST_ITEM_KEYS);
    for record in records {
        // Writing through `fields` copies it; leave records without these fields shared.
        if record.fields.iter().any(|(id, _)| stripped(*id)) {
            record.fields.retain(|(id, _)| !stripped(*id));
        }
    }
}

fn applied_offsets(display: &DisplayList) -> HashMap<String, f32> {
    display
        .nodes
        .iter()
        .filter_map(|node| node.scroll.as_ref())
        .map(|scroll| (scroll.id.clone(), scroll.offset))
        .collect()
}
