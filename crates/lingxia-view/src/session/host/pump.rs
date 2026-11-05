//! Frame sequencing: sample, prepare on the worker, present on vsync, then run
//! the presented frame's effects, reach, images and animation wakeup.
use super::{FrameSnapshot, FrameTiming, Guard, Inner};
use crate::interaction::frame::PresentedFrame;
use crate::interaction::view::Effect;
use crate::platform::{FrameEnvironment, PlatformAdapter};
use crate::session::lifecycle::{CommitTicket, PageTicket, SurfaceTicket};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A frame prepared on the worker, waiting for vsync.
pub(super) struct Prepared<F> {
    frame: F,
    environment: crate::platform::FrameEnvironment,
    snapshot: Arc<FrameSnapshot>,
    surface: SurfaceTicket,
    page: PageTicket,
    revision: u64,
    offsets: HashMap<String, f32>,
    timing: FrameTiming,
    prepared_at: Instant,
    pacing: Pacing,
}

/// What frame statistics need to know about a frame beyond its timing.
#[derive(Clone, Copy, Default)]
pub(super) struct Pacing {
    demand: Duration,
    input: Option<Duration>,
    /// The refresh it was meant for and the refresh interval, set at vsync.
    target: Duration,
    interval: Duration,
    /// When the display took the frame.
    done: Duration,
    /// The newest pointer position the frame shows, for the HUD's trail.
    point: Option<[f32; 2]>,
}

/// A presented frame waiting for the adapter to put it on screen.
pub(super) struct Deferred {
    environment: crate::platform::FrameEnvironment,
    snapshot: Arc<FrameSnapshot>,
    surface: SurfaceTicket,
    page: PageTicket,
    revision: u64,
    offsets: HashMap<String, f32>,
    timing: FrameTiming,
    vsync: Instant,
    pacing: Pacing,
}

impl<F> Prepared<F> {
    pub(super) fn layout(&self) -> Option<(u64, u64)> {
        self.snapshot.frame.layout
    }
}

// An internal signal, never surfaced; the frame is simply skipped.
use crate::session::FRAME_DECLINED as DECLINED;

fn micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

struct Request {
    environment: FrameEnvironment,
    surface: SurfaceTicket,
    page: PageTicket,
    rebuild: bool,
    revision: u64,
    offsets: HashMap<String, f32>,
    caret: bool,
    /// A hidden page's catch-up frame keeps animation time parked.
    visible: bool,
    /// When the frame is expected on screen.
    sample: Duration,
    /// Built on the frame still waiting for its vsync.
    ahead: bool,
    /// Reported in frame metrics once frames ahead were given up.
    single_target: bool,
    dropped_activations: u64,
    activations: u64,
    pacing: Pacing,
    hud: Option<crate::scene::hud::Hud>,
}

impl<A: PlatformAdapter> Inner<A> {
    /// Prepare a frame when the page can present and no frame is in flight.
    pub(super) fn schedule(self: &Arc<Self>) {
        self.schedule_frame(false);
    }

    /// While an animation runs and a prepared frame waits for its vsync, prepare the
    /// following one now so a worker frame longer than the display interval still keeps
    /// pace. Never while input waits to dispatch: it needs no frame in flight.
    pub(super) fn schedule_ahead(self: &Arc<Self>) {
        let ready = {
            let main = self.main();
            #[cfg(test)]
            if main.no_frames_ahead {
                return;
            }
            main.visible
                && main.animating
                && main.prepared.is_some()
                && !main.ahead
                && main.next_prepared.is_none()
                && main.deferred.is_none()
                && !main.single_target
                && !main.number_busy
                && !main.disposed
                && !main.interaction.waiting()
        };
        if ready {
            self.schedule_frame(true);
        }
    }

    fn schedule_frame(self: &Arc<Self>, ahead: bool) {
        let asked = self.adapter.now();
        self.main().demand_at.get_or_insert(asked);
        let environment = self.adapter.environment().map(|mut environment| {
            self.main().host_environment.apply(&mut environment.value);
            environment
        });
        // Chrome takes the palette this page renders with, before the frame that shows it.
        if let Some(environment) = &environment {
            let chrome = crate::platform::HostChrome::from_environment(&environment.value);
            let changed = {
                let mut main = self.main();
                !main.disposed && main.chrome.replace(chrome) != Some(chrome)
            };
            if changed {
                self.adapter.host_chrome(&chrome);
            }
        }
        let (request, background) = {
            let mut main = self.main();
            main.animation_timer += 1;
            main.pending = true;
            if main.disposed {
                return;
            }
            let Some(environment) = environment else {
                return;
            };
            let fine = environment.value["pointer"] == "fine";
            main.interaction.set_fine_pointer(fine);
            main.text_menu.set_fine_pointer(fine);
            let same = main
                .environment
                .as_ref()
                .is_some_and(|held| main.host_environment.same(held, &environment));
            if same {
                // Host context only this page's View ignores: keep it for the next run, rebuild nothing.
                main.environment = Some(environment.clone());
            } else {
                main.environment = Some(environment.clone());
                self.lifecycle.invalidate_surface();
                main.dirty = true;
                // A kept hidden frame no longer matches; catch up so a return can show at once.
                // A parked page left: it renders nothing until the next connection.
                if !main.visible
                    && main.presented_environment.is_some()
                    && !main.interaction.parked()
                {
                    main.catch_up = true;
                }
            }
            if (main.busy && !ahead) || main.number_busy || (main.activating > 0 && !main.dirty) {
                return;
            }
            let background = !main.visible;
            let surface = if main.visible {
                self.lifecycle.frame()
            } else if main.catch_up && !main.interaction.parked() {
                self.lifecycle.frame_hidden()
            } else {
                None
            };
            let Some(surface) = surface else {
                return;
            };
            let request = Request {
                environment,
                surface,
                page: self.lifecycle.page(),
                rebuild: main.dirty || main.snapshot.is_none(),
                revision: main.interaction.scroll().revision(),
                offsets: main.interaction.offsets(),
                caret: !main.caret.hidden,
                visible: main.visible,
                sample: Duration::ZERO,
                ahead,
                single_target: main.single_target,
                dropped_activations: main.dropped_activations,
                activations: main.activations,
                pacing: Pacing {
                    demand: main.demand_at.take().unwrap_or(asked),
                    input: main.telemetry.stats.take_input(asked),
                    point: main.telemetry.stats.take_point(),
                    ..Pacing::default()
                },
                hud: main.hud.then(|| main.telemetry.stats.hud(asked)),
            };
            main.dirty = false;
            main.pending = false;
            main.busy = true;
            main.ahead |= ahead;
            (request, background)
        };
        let now = self.adapter.now();
        let mut request = request;
        let epoch = {
            let main = self.main();
            if request.visible {
                request.sample = main.expected_vsync(now, u32::from(ahead));
            } else {
                request.sample = now;
            }
            main.ahead_epoch
        };
        let surface = request.surface;
        let page = request.page;
        let revision = request.revision;
        let offsets = request.offsets.clone();
        let environment = request.environment.clone();
        let pacing = request.pacing;
        let mut timing = FrameTiming {
            rebuild: request.rebuild,
            caret_visible: request.caret,
            ..Default::default()
        };
        let queued = Instant::now();
        let rebuild = request.rebuild;
        self.work_at(
            background,
            Guard::Page(page),
            move |inner, session| {
                let started = Instant::now();
                let prepared = inner.prepare(session, request, now);
                let reads = session.reads_host_context();
                inner.main().host_environment.set_reads_host_context(reads);
                // Only a View step can ask for a measurement, and every one rebuilds.
                if rebuild
                    && let Ok(measures) = session.take_frame_measures()
                    && !measures.is_empty()
                {
                    inner.main().frame_measures.extend(measures);
                }
                prepared.map(|prepared| {
                    (
                        prepared,
                        micros(started - queued),
                        micros(started.elapsed()),
                    )
                })
            },
            move |inner, result| {
                let measures = std::mem::take(&mut inner.main().frame_measures);
                for (token, ms) in measures {
                    inner.frame_measure(token, ms);
                }
                match result {
                    Some(Ok(((Some((frame, snapshot)), update), queue_us, worker_us))) => {
                        timing.queue_us = queue_us;
                        timing.worker_us = worker_us;
                        if let Some((connection, update)) = update {
                            inner.publish(connection, update);
                        }
                        let prepared = Prepared {
                            frame,
                            environment,
                            snapshot,
                            surface,
                            page,
                            revision,
                            offsets,
                            timing,
                            prepared_at: Instant::now(),
                            pacing,
                        };
                        {
                            let mut main = inner.main();
                            if ahead {
                                if main.ahead_epoch != epoch {
                                    // What it was built on did not present.
                                    return;
                                }
                                main.ahead = false;
                                if main.prepared.is_some() {
                                    main.next_prepared = Some(prepared);
                                    return;
                                }
                                main.busy = true;
                            }
                            main.prepared = Some(prepared);
                        }
                        inner.adapter.request_vsync();
                        inner.schedule_ahead();
                    }
                    Some(Ok(((None, update), ..))) => {
                        if let Some((connection, update)) = update {
                            inner.publish(connection, update);
                        }
                        let retry = {
                            let mut main = inner.main();
                            main.declined |= main.prepared.is_some() || main.deferred.is_some();
                            // Nothing else will ask again while an animation waits on this frame.
                            main.animating && !main.declined && !main.disposed
                        };
                        if retry {
                            let host = inner.clone();
                            inner.adapter.post_after(
                                Duration::from_millis(16),
                                Box::new(move || host.schedule()),
                            );
                        }
                        if !inner.ahead_failed(ahead) {
                            inner.unprepared(None, surface, page);
                        }
                    }
                    Some(Err(error)) if !inner.ahead_failed(ahead) => {
                        inner.unprepared(Some(error), surface, page)
                    }
                    Some(Err(_)) => {}
                    None if !inner.ahead_failed(ahead) => inner.unprepared(None, surface, page),
                    None => {}
                }
            },
        );
    }

    /// Answer the View's `measureFrames` window `token` once `ms` passed.
    fn frame_measure(self: &Arc<Self>, token: u64, ms: u64) {
        let (inner, page) = (self.clone(), self.lifecycle.page());
        let window = Duration::from_millis(ms);
        self.adapter.post_after(
            window,
            Box::new(move || {
                if !inner.lifecycle.is_current_page(page) || inner.main().disposed {
                    return;
                }
                let now = inner.adapter.now();
                let stats = inner.main().telemetry.stats.summary(now, window);
                inner.work(
                    Guard::Page(page),
                    move |inner, session| {
                        session.frame_measured(token, &stats)?;
                        inner.drained(session)
                    },
                    |inner, result| {
                        if let Some(Ok(Some((connection, update)))) = result {
                            inner.publish(connection, update);
                        }
                        if !inner.main().disposed {
                            inner.main().dirty = true;
                            inner.schedule();
                        }
                    },
                );
            }),
        );
    }

    #[allow(clippy::type_complexity)]
    fn prepare(
        &self,
        session: &mut crate::ViewSession,
        request: Request,
        now: Duration,
    ) -> Result<
        (
            Option<(A::Frame, Arc<FrameSnapshot>)>,
            Option<(
                crate::session::lifecycle::ConnectionTicket,
                super::BridgeUpdate,
            )>,
        ),
        String,
    > {
        let page = request.page;
        let FrameEnvironment {
            width,
            height,
            scale,
            value,
        } = request.environment;
        let mut loaded_images = 0;
        let mut image_buckets = Default::default();
        session.set_environment(value)?;
        session.set_animation_target(Some(request.surface), request.visible, now)?;
        let (sample, ahead, single_target, dropped_activations, activations) = (
            request.sample,
            request.ahead,
            request.single_target,
            request.dropped_activations,
            request.activations,
        );
        let generation = CommitTicket::new(session.generation().get() + 1);
        let caret = request.caret;
        let surface = request.surface;
        let hud = request.hud.as_ref().map(|hud| {
            let top = session.environment["safeArea"]["top"]
                .as_f64()
                .or_else(|| session.environment["insets"]["top"].as_f64())
                .unwrap_or(0.0) as f32;
            let logical = f32::from(width) / scale.max(f32::MIN_POSITIVE);
            crate::scene::hud::paints(&mut session.renderer, hud, logical, top, scale)
        });
        // A frame the target declines is a failed candidate: the session keeps its previous
        // commit (layout, viewport, generation) and the next attempt starts from it.
        let prepared = session.prepare_view_sampled(
            width,
            height,
            scale,
            request.rebuild,
            Some(&request.offsets),
            now,
            sample,
            ahead,
            |mut display| {
                if !caret {
                    display.hide_caret();
                }
                if let Some(hud) = hud {
                    display.push_overlay("lingxia:hud", hud);
                }
                let nodes = display.nodes.clone();
                loaded_images = display.loaded_image_count();
                image_buckets = display.image_buckets();
                match self.adapter.prepare(display, surface, generation)? {
                    Some(frame) => Ok((frame, nodes)),
                    None => Err(DECLINED.into()),
                }
            },
        );
        let (frame, nodes) = match prepared {
            Ok(prepared) => prepared,
            Err(error) if error == DECLINED => return Ok((None, None)),
            Err(error) => return Err(error),
        };
        let update = if self.logic {
            let mut update = super::bridge::update(
                session,
                crate::session::state::StateUpdate {
                    changed: true,
                    outbound: Vec::new(),
                },
            )?;
            update.changed = update.local_changed;
            Some((self.lifecycle.connection(), update))
        } else {
            None
        };
        let images = session.image_requests();
        // A page near a limit says so once, through the app's log pipeline so `lingxia dev`
        // and `lxdev logs` show it, before a frame fails on it.
        for warning in session.take_budget_warnings() {
            log::warn!(target: "lingxia_view", "{warning}");
        }
        let mut metrics =
            serde_json::to_value(session.frame_metrics()).map_err(|e| e.to_string())?;
        if let Some(metrics) = metrics.as_object_mut() {
            metrics.insert(
                "imageCacheBytes".into(),
                super::decode::retained_bytes().into(),
            );
            if single_target {
                metrics.insert("presenterSingleTarget".into(), true.into());
            }
            metrics.insert("droppedActivations".into(), dropped_activations.into());
            metrics.insert("activations".into(), activations.into());
        }
        if let (Some(extra), Some(metrics)) =
            (self.adapter.metrics(&frame), metrics.as_object_mut())
            && let Some(extra) = extra.as_object()
        {
            metrics.extend(extra.clone());
        }
        let generation = session.generation();
        let feedback = crate::interaction::chrome::FeedbackStyle::for_palette(
            &session.renderer.palette,
            session.renderer.dark,
        );
        let json = serde_json::json!({
            "generation": generation,
            "nodes": nodes,
            "images": images,
            "metrics": metrics,
            "feedback": feedback,
        })
        .to_string();
        let snapshot = Arc::new(FrameSnapshot {
            metrics: session.frame_metrics().clone(),
            generation,
            width,
            height,
            scale,
            loaded_images,
            frame: Arc::new(
                PresentedFrame::new(generation, scale, nodes)
                    .with_layout((page.get(), session.layout_revision())),
            ),
            images,
            image_buckets,
            feedback,
            json,
        });
        Ok((Some((frame, snapshot)), update))
    }

    /// An ahead frame that will not present: the frame under it still waits for vsync.
    fn ahead_failed(self: &Arc<Self>, ahead: bool) -> bool {
        if ahead {
            let mut main = self.main();
            main.ahead = false;
            main.dirty = true;
        }
        ahead
    }

    /// A frame that will not present: the next one must rebuild.
    fn unprepared(
        self: &Arc<Self>,
        error: Option<String>,
        surface: SurfaceTicket,
        page: PageTicket,
    ) {
        // A dropped frame is not a failure: the page keeps its last frame and renders again.
        if let Some(error) = error.as_deref()
            && error.starts_with(crate::session::OVERRUN)
        {
            let current = self.lifecycle.is_current_page(page);
            {
                let mut main = self.main();
                main.busy = false;
                main.dirty = true;
                if current {
                    main.telemetry.budget_overrun();
                }
            }
            if current {
                self.adapter.warn(error);
                self.schedule();
            }
            return;
        }
        let (pending, effects) = {
            let mut main = self.main();
            main.busy = false;
            main.dirty = true;
            let effects =
                if error.is_some() && !main.disposed && self.lifecycle.is_current_page(page) {
                    main.interaction.cancel_all()
                } else {
                    Vec::new()
                };
            (main.pending && !main.disposed, effects)
        };
        self.deliver(effects);
        if let Some(error) = error
            && self.lifecycle.is_current_page(page)
            && self.lifecycle.is_current_surface(surface)
        {
            self.fail(error);
        }
        if pending {
            self.schedule();
        }
    }

    pub(super) fn vsync(self: &Arc<Self>) {
        // A window nothing of which shows still completes its frames, so the page's lifecycle,
        // hit-testing and readback advance, but nothing reaches a drawable that may not return.
        let hidden = self.window_hidden();
        let vsync = Instant::now();
        let at = self.adapter.now();
        let Some(prepared) = ({
            let mut main = self.main();
            // One deferred frame at a time: the next waits until the adapter shows this one,
            // or its acknowledgement would no longer match.
            if main.deferred.is_some() {
                return;
            }
            if let Some(last) = main.last_vsync
                && let Some(delta) = at.checked_sub(last)
                && delta > Duration::from_millis(4)
                && delta < Duration::from_millis(50)
            {
                main.vsync_interval = (main.vsync_interval * 7 + delta) / 8;
            }
            main.last_vsync = Some(at);
            let platform = main.display.take();
            if let Some((_, interval)) = platform.filter(|(_, i)| !i.is_zero()) {
                main.vsync_interval = interval;
            }
            let interval = main.vsync_interval;
            let target = platform.map_or(at + interval, |(target, _)| target);
            let prepared = main.prepared.take().map(|mut prepared| {
                prepared.pacing.target = target;
                prepared.pacing.interval = interval;
                prepared
            });
            if prepared.is_some() {
                main.prepared = main.next_prepared.take();
                main.busy = main.ahead || main.prepared.is_some();
            }
            prepared
        }) else {
            return;
        };
        if self.main().prepared.is_some() {
            self.adapter.request_vsync();
        }
        let current = !self.main().disposed && self.lifecycle.is_current_surface(prepared.surface);
        if !current {
            let mut main = self.main();
            main.prepared = None;
            main.drop_ahead();
            main.busy = main.deferred.is_some();
        }
        let mut retry = false;
        if current {
            let Prepared {
                frame,
                environment,
                snapshot,
                surface,
                page,
                revision,
                offsets,
                mut timing,
                prepared_at,
                pacing,
            } = prepared;
            timing.wait_us = micros(vsync.saturating_duration_since(prepared_at));
            let deferred = !hidden && self.adapter.deferred(&frame);
            let newer = {
                let main = self.main();
                main.prepared.is_some() || main.ahead
            };
            let shown = if hidden {
                Ok(true)
            } else {
                self.adapter.present(frame)
            };
            let mut pacing = pacing;
            pacing.done = self.adapter.now();
            match shown {
                Ok(true) if deferred => {
                    // Nothing else prepares until this frame is on screen.
                    let mut main = self.main();
                    main.busy = true;
                    main.deferred = Some(Deferred {
                        environment,
                        snapshot,
                        surface,
                        page,
                        revision,
                        offsets,
                        timing,
                        vsync,
                        pacing,
                    });
                }
                Ok(true) => {
                    // The gate in `frame_is_current` trusts only a frame the target accepted.
                    let mut main = self.main();
                    main.stale_declines = 0;
                    main.presented_environment = Some(environment);
                    main.catch_up = false;
                    drop(main);
                    self.presented(
                        snapshot, surface, page, revision, offsets, timing, vsync, pacing,
                    )
                }
                Ok(false) => {
                    retry = true;
                    if newer {
                        self.stale_decline();
                    }
                }
                Err(error) => self.fail(error),
            }
        }
        if retry {
            // Frames built on the refused one cannot present either.
            {
                let mut main = self.main();
                main.prepared = None;
                main.drop_ahead();
                main.busy = false;
                main.dirty = true;
            }
            // The target refused a frame; try again on a later display frame.
            let host = self.clone();
            self.adapter
                .post_after(Duration::from_millis(16), Box::new(move || host.schedule()));
        }
        let pending = {
            let main = self.main();
            main.pending && !main.disposed
        };
        let retry_declined = std::mem::take(&mut self.main().declined);
        if pending || retry_declined {
            self.schedule();
        }
        self.schedule_ahead();
    }

    /// A presenter that keeps only the newest prepared frame refuses every frame with one
    /// prepared behind it. Fall back to one frame at a time and say why, once.
    fn stale_decline(self: &Arc<Self>) {
        let warn = {
            let mut main = self.main();
            main.stale_declines += 1;
            let warn = main.stale_declines >= 3 && !main.single_target;
            if warn {
                main.single_target = true;
            }
            warn
        };
        if warn {
            self.adapter.warn(
                "UI_FRAME: presenterSingleTarget: the presenter refused 3 frames in a row that had a newer frame prepared behind them; Presenter::prepare must keep each prepared frame presentable until it is presented or dropped (one target per frame in flight, presented by its own revision). Frames are no longer prepared ahead on this page.",
            );
        }
    }

    pub(super) fn deferred_presented(self: &Arc<Self>, generation: CommitTicket, shown: bool) {
        let Some(deferred) = ({
            let mut main = self.main();
            let matches = main
                .deferred
                .as_ref()
                .is_some_and(|d| d.snapshot.generation == generation);
            let deferred = matches.then(|| main.deferred.take()).flatten();
            if deferred.is_some() {
                main.busy = main.prepared.is_some() || main.ahead;
            }
            deferred
        }) else {
            return;
        };
        let current = !self.main().disposed && self.lifecycle.is_current_surface(deferred.surface);
        if shown && current {
            let Deferred {
                environment,
                snapshot,
                surface,
                page,
                revision,
                offsets,
                timing,
                vsync,
                mut pacing,
            } = deferred;
            pacing.done = self.adapter.now();
            {
                let mut main = self.main();
                main.presented_environment = Some(environment);
                main.catch_up = false;
            }
            self.presented(
                snapshot, surface, page, revision, offsets, timing, vsync, pacing,
            );
        } else {
            // Frames built on the dropped one cannot present either.
            let mut main = self.main();
            main.prepared = None;
            main.drop_ahead();
            main.busy = false;
            main.dirty = true;
            main.pending = true;
        }
        if self.main().prepared.is_some() {
            self.adapter.request_vsync();
        }
        let pending = {
            let main = self.main();
            main.pending && !main.disposed
        };
        if pending {
            self.schedule();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn presented(
        self: &Arc<Self>,
        snapshot: Arc<FrameSnapshot>,
        surface: SurfaceTicket,
        page: PageTicket,
        revision: u64,
        offsets: HashMap<String, f32>,
        mut timing: FrameTiming,
        vsync: Instant,
        pacing: Pacing,
    ) {
        let rebuild = timing.rebuild;
        let (presented, recovered) = {
            let mut main = self.main();
            main.snapshot = Some(snapshot.clone());
            // Repainting an old frame (caret, scroll) is not recovery.
            let recovered = rebuild && main.failure.take().is_some();
            let presented =
                main.interaction
                    .presented(snapshot.frame.clone(), revision, &offsets, page);
            (presented, recovered)
        };
        if recovered {
            self.adapter.recovery(None);
        }
        self.acknowledge_animation(surface, snapshot.generation);
        // A running animation's next frame queues right behind the acknowledgement on the
        // worker instead of waiting for its wakeup to come back through the main thread.
        // Not while activations wait: they dispatch only when no frame is being prepared.
        let waiting = presented
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::Dispatch | Effect::Number));
        let now = self.adapter.now();
        let queue = {
            let mut main = self.main();
            let queue = !waiting && main.animating && now > main.animation_queued_at;
            if queue {
                main.animation_queued_at = now;
            }
            queue
        };
        if queue {
            self.schedule();
        }
        self.adapter.reconcile(&snapshot);
        self.deliver(presented.effects);
        self.glide_later();
        if let Some(reach) = presented.reach {
            self.reach(page, reach);
        }
        self.update_images(snapshot.images.clone(), snapshot.image_buckets.clone());
        self.flush_image_events();
        timing.main_us = micros(vsync.elapsed());
        let now = self.adapter.now();
        let shown = !self.window_hidden();
        {
            let mut main = self.main();
            main.telemetry.presented(now, &snapshot, &timing);
            if shown && main.visible {
                main.telemetry.stats.record(
                    super::frame_stats::Presented {
                        demand: pacing.demand,
                        target: pacing.target,
                        done: pacing.done,
                        interval: pacing.interval,
                        cpu_us: timing.worker_us,
                        queue_us: timing.queue_us,
                        gpu_us: None,
                        input: pacing.input,
                    },
                    pacing.point,
                );
            }
        }
        self.adapter.presented(&snapshot, &timing);
        self.revealed_by_frame();
        self.update_text_menu();
        self.edits_presented();
    }

    /// Start the timeline's newly presented effects and wake for its next deadline.
    fn acknowledge_animation(self: &Arc<Self>, surface: SurfaceTicket, generation: CommitTicket) {
        let at = self.adapter.now();
        self.work(
            Guard::Page(self.lifecycle.page()),
            move |_, session| {
                if !session.animation_presented(surface, generation, at)? {
                    return Ok(None);
                }
                Ok(match session.view_animation_wakeup(at)? {
                    crate::animation::Wakeup::None => None,
                    crate::animation::Wakeup::NextFrame => Some(Duration::ZERO),
                    crate::animation::Wakeup::After(delay) => Some(delay),
                })
            },
            move |inner, result| {
                let animating = matches!(result, Some(Ok(Some(Duration::ZERO))));
                let settled = {
                    let mut main = inner.main();
                    main.animating = animating;
                    if animating {
                        Vec::new()
                    } else {
                        main.interaction.settled()
                    }
                };
                inner.deliver(settled);
                let delay = match result {
                    Some(Ok(Some(delay))) => delay,
                    Some(Err(error)) => {
                        inner.adapter.warn(&format!("UI_ANIMATION: {error}"));
                        return;
                    }
                    _ => return,
                };
                let timer = {
                    let mut main = inner.main();
                    if main.disposed
                        || !inner.lifecycle.is_current_surface(surface)
                        || main.snapshot.as_ref().map(|s| s.generation) != Some(generation)
                        || main.busy
                        || main.pending
                    {
                        return;
                    }
                    main.animation_timer += 1;
                    main.animation_timer
                };
                let spent = inner.adapter.now().saturating_sub(at);
                let host = inner.clone();
                inner.adapter.post_after(
                    delay.saturating_sub(spent),
                    Box::new(move || {
                        let current = {
                            let mut main = host.main();
                            let current = main.animation_timer == timer;
                            if current {
                                main.telemetry.woke();
                            }
                            current
                        };
                        if current {
                            host.schedule();
                        }
                    }),
                );
            },
        );
    }

    /// Pause the timeline while nothing can present.
    pub(super) fn park_animations(self: &Arc<Self>) {
        {
            let mut main = self.main();
            main.animation_timer += 1;
            main.animating = false;
            main.drop_ahead();
        }
        let at = self.adapter.now();
        self.work(
            Guard::Page(self.lifecycle.page()),
            move |_, session| session.set_animation_surface(None, at),
            |inner, result| {
                if let Some(Err(error)) = result {
                    inner.adapter.warn(&format!("UI_ANIMATION: {error}"));
                }
            },
        );
    }
}
