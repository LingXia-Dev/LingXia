//! Frame sequencing: sample, prepare on the worker, present on vsync, then run
//! the presented frame's effects, reach, images and animation wakeup.
use super::{FrameSnapshot, FrameTiming, Guard, Inner};
use crate::interaction::frame::PresentedFrame;
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
}

const DECLINED: &str = "UI_FRAME: declined by the target";

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
}

impl<A: PlatformAdapter> Inner<A> {
    /// Prepare a frame when the page can present and no frame is in flight.
    pub(super) fn schedule(self: &Arc<Self>) {
        let environment = self.adapter.environment();
        let (request, background) = {
            let mut main = self.main();
            main.animation_timer += 1;
            main.pending = true;
            if main.disposed {
                return;
            }
            let Some(mut environment) = environment else {
                return;
            };
            main.host_environment.apply(&mut environment.value);
            if main.environment.as_ref() != Some(&environment) {
                main.environment = Some(environment.clone());
                self.lifecycle.invalidate_surface();
                main.dirty = true;
                // A kept hidden frame no longer matches; catch up so a return can show at once.
                if !main.visible && main.presented_environment.is_some() {
                    main.catch_up = true;
                }
            }
            if main.busy || main.number_busy {
                return;
            }
            let background = !main.visible;
            let surface = if main.visible {
                self.lifecycle.frame()
            } else if main.catch_up {
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
                offsets: main.interaction.scroll().snapshot(),
                caret: !main.caret.hidden,
                visible: main.visible,
            };
            main.dirty = false;
            main.pending = false;
            main.busy = true;
            (request, background)
        };
        let now = self.adapter.now();
        let surface = request.surface;
        let page = request.page;
        let revision = request.revision;
        let offsets = request.offsets.clone();
        let environment = request.environment.clone();
        let mut timing = FrameTiming {
            rebuild: request.rebuild,
            caret_visible: request.caret,
            ..Default::default()
        };
        let queued = Instant::now();
        self.work_at(
            background,
            Guard::Page(page),
            move |inner, session| {
                let started = Instant::now();
                inner.prepare(session, request, now).map(|prepared| {
                    (
                        prepared,
                        micros(started - queued),
                        micros(started.elapsed()),
                    )
                })
            },
            move |inner, result| match result {
                Some(Ok(((Some((frame, snapshot)), update), queue_us, worker_us))) => {
                    timing.queue_us = queue_us;
                    timing.worker_us = worker_us;
                    if let Some((connection, update)) = update {
                        inner.publish(connection, update);
                    }
                    inner.main().prepared = Some(Prepared {
                        frame,
                        environment,
                        snapshot,
                        surface,
                        page,
                        revision,
                        offsets,
                        timing,
                        prepared_at: Instant::now(),
                    });
                    inner.adapter.request_vsync();
                }
                Some(Ok(((None, update), ..))) => {
                    if let Some((connection, update)) = update {
                        inner.publish(connection, update);
                    }
                    inner.unprepared(None, surface, page);
                }
                Some(Err(error)) => inner.unprepared(Some(error), surface, page),
                None => inner.unprepared(None, surface, page),
            },
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
        let generation = CommitTicket::new(session.generation().get() + 1);
        let caret = request.caret;
        let surface = request.surface;
        // A frame the target declines is a failed candidate: the session keeps its previous
        // commit (layout, viewport, generation) and the next attempt starts from it.
        let prepared = session.prepare_view_animated(
            width,
            height,
            scale,
            request.rebuild,
            Some(&request.offsets),
            now,
            |mut display| {
                if !caret {
                    display.hide_caret();
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
        let mut metrics =
            serde_json::to_value(session.frame_metrics()).map_err(|e| e.to_string())?;
        if let Some(metrics) = metrics.as_object_mut() {
            metrics.insert(
                "imageCacheBytes".into(),
                super::decode::retained_bytes().into(),
            );
        }
        if let (Some(extra), Some(metrics)) =
            (self.adapter.metrics(&frame), metrics.as_object_mut())
            && let Some(extra) = extra.as_object()
        {
            metrics.extend(extra.clone());
        }
        let generation = session.generation();
        let feedback = crate::interaction::chrome::FeedbackStyle::for_theme(session.renderer.dark);
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
            frame: Arc::new(PresentedFrame::new(generation, scale, nodes)),
            images,
            image_buckets,
            json,
        });
        Ok((Some((frame, snapshot)), update))
    }

    /// A frame that will not present: the next one must rebuild.
    fn unprepared(
        self: &Arc<Self>,
        error: Option<String>,
        surface: SurfaceTicket,
        page: PageTicket,
    ) {
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
        let vsync = Instant::now();
        let Some(prepared) = ({
            let mut main = self.main();
            let prepared = main.prepared.take();
            if prepared.is_some() {
                main.busy = false;
            }
            prepared
        }) else {
            return;
        };
        let current = !self.main().disposed && self.lifecycle.is_current_surface(prepared.surface);
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
            } = prepared;
            timing.wait_us = micros(vsync.saturating_duration_since(prepared_at));
            match self.adapter.present(frame) {
                Ok(true) => {
                    // The gate in `frame_is_current` trusts only a frame the target accepted.
                    let mut main = self.main();
                    main.presented_environment = Some(environment);
                    main.catch_up = false;
                    drop(main);
                    self.presented(snapshot, surface, page, revision, offsets, timing, vsync)
                }
                Ok(false) => retry = true,
                Err(error) => self.fail(error),
            }
        }
        if retry {
            // The target refused a frame; try again on a later display frame.
            let host = self.clone();
            self.adapter
                .post_after(Duration::from_millis(16), Box::new(move || host.schedule()));
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
        let now = self.adapter.now();
        let queue = {
            let mut main = self.main();
            let queue = main.animating && now > main.animation_queued_at;
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
        if let Some(reach) = presented.reach {
            self.reach(page, reach);
        }
        self.update_images(snapshot.images.clone(), snapshot.image_buckets.clone());
        self.flush_image_events();
        timing.main_us = micros(vsync.elapsed());
        let now = self.adapter.now();
        self.main().telemetry.presented(now, &snapshot, &timing);
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
                inner.main().animating = matches!(result, Some(Ok(Some(Duration::ZERO))));
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
