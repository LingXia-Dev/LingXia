//! A single-threaded composition of the shared View core over a [`PlatformAdapter`].
//! Multi-threaded adapters perform the same steps, split between their View
//! worker (`frame` preparation, dispatch) and their UI thread (presentation, ingress).
use super::ViewSession;
use super::image_requests::ImageRequests;
use super::lifecycle::{Lifecycle, PageTicket, Recovery};
use crate::interaction::frame::PresentedFrame;
use crate::interaction::view::{
    AccessibilityAction, Dispatch, Effect, Key, Pointer, ViewInteraction,
};
use crate::platform::PlatformAdapter;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ViewController<A: PlatformAdapter> {
    pub adapter: A,
    session: ViewSession,
    lifecycle: Lifecycle,
    interaction: ViewInteraction,
    images: ImageRequests,
    page: PageTicket,
    viewport: (u16, u16, f32),
    rebuild: bool,
}

impl<A: PlatformAdapter> ViewController<A> {
    /// Load a standalone page module.
    pub fn new(adapter: A, source: &str, viewport: (u16, u16, f32)) -> Result<Self, String> {
        let lifecycle = Lifecycle::new(false);
        let page = lifecycle.load();
        let session = ViewSession::new(source)?;
        lifecycle.loaded(page);
        Ok(Self {
            adapter,
            session,
            lifecycle,
            interaction: ViewInteraction::default(),
            images: ImageRequests::default(),
            page,
            viewport,
            rebuild: true,
        })
    }

    pub fn lifecycle(&self) -> &Lifecycle {
        &self.lifecycle
    }

    pub fn interaction(&self) -> &ViewInteraction {
        &self.interaction
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.lifecycle.set_visible(visible);
        if !visible {
            let effects = self.interaction.cancel_all();
            self.adapter.apply(&effects);
        }
    }

    pub fn resize(&mut self, viewport: (u16, u16, f32)) {
        self.viewport = viewport;
        self.lifecycle.invalidate_surface();
    }

    /// Prepare and present one frame; returns false when nothing could be presented.
    pub fn frame(&mut self) -> Result<bool, String> {
        let Some(surface) = self.lifecycle.frame() else {
            return Ok(false);
        };
        let (width, height, scale) = self.viewport;
        let revision = self.interaction.scroll().revision();
        let offsets: HashMap<String, f32> = self.interaction.scroll().snapshot();
        let display = match self.session.prepare_native(
            width,
            height,
            scale,
            std::mem::take(&mut self.rebuild),
            Some(&offsets),
            Ok,
        ) {
            Ok(display) => display,
            Err(error) => {
                self.rebuild = true;
                return Err(error);
            }
        };
        let frame = Arc::new(PresentedFrame::new(
            self.session.generation(),
            scale,
            display.nodes.clone(),
        ));
        if !self.lifecycle.is_current_surface(surface) || !self.adapter.present(display, surface)? {
            return Ok(false);
        }
        let presented = self
            .interaction
            .presented(frame.clone(), revision, &offsets, self.page);
        let (accessibility, effects): (Vec<_>, Vec<_>) = presented
            .effects
            .into_iter()
            .partition(|effect| matches!(effect, Effect::Accessibility(_)));
        let events: Vec<_> = accessibility
            .into_iter()
            .filter_map(|effect| match effect {
                Effect::Accessibility(event) => Some(event),
                _ => None,
            })
            .collect();
        if let Some(projection) = self.interaction.semantics() {
            self.adapter.publish(&projection, &events);
        }
        self.adapter.presented(&frame);
        self.adapter.apply(&effects);
        if let Some(reach) = presented.reach {
            self.run(reach)?;
        }
        self.update_images()?;
        if self.session.dispatch_image_events(frame.commit)? {
            self.rebuild = true;
        }
        self.drain()?;
        Ok(true)
    }

    fn update_images(&mut self) -> Result<(), String> {
        let before: Vec<u64> = self.images.inflight_tokens();
        self.images
            .update_resources(&self.session.image_requests())?;
        for token in before {
            if !self.images.current(token) {
                self.images.complete(token);
                self.adapter.cancel(token);
            }
        }
        for request in self.images.start() {
            self.adapter.fetch(request.token, &request.source);
        }
        Ok(())
    }

    /// Deliver fetched bytes; stale or replaced requests are ignored.
    pub fn image_loaded(
        &mut self,
        id: u64,
        bytes: Result<Vec<u8>, String>,
    ) -> Result<bool, String> {
        let Some(source) = self.images.current_source(id).map(str::to_owned) else {
            self.images.complete(id);
            return Ok(false);
        };
        let retry_key = self
            .session
            .image_requests()
            .into_iter()
            .find(|resource| resource.source == source)
            .map(|resource| resource.retry_key)
            .unwrap_or_default();
        let bound = match bytes {
            Ok(bytes) => self.session.load_image(&source, &bytes).unwrap_or(false),
            Err(_) => false,
        };
        self.images.complete(id);
        self.session
            .image_completed(crate::ImageSource { source, retry_key }, bound);
        self.update_images()?;
        Ok(bound)
    }

    pub fn pointer(&mut self, event: Pointer) -> Result<bool, String> {
        if !self.lifecycle.accepts_input() {
            return Ok(false);
        }
        let (consumed, effects) = self.interaction.pointer(event, self.page);
        self.effects(effects)?;
        Ok(consumed)
    }

    pub fn long_press(&mut self) -> Result<(), String> {
        let effects = self.interaction.long_press();
        self.effects(effects)
    }

    pub fn key(&mut self, key: Key, shift: bool, repeat: bool) -> Result<bool, String> {
        let (consumed, effects) = self.interaction.key(key, shift, repeat, self.page);
        self.effects(effects)?;
        Ok(consumed)
    }

    pub fn accessibility(&mut self, id: u32, action: AccessibilityAction) -> Result<bool, String> {
        let (handled, effects) = self.interaction.accessibility(id, action, self.page);
        self.effects(effects)?;
        Ok(handled)
    }

    fn effects(&mut self, effects: Vec<Effect>) -> Result<(), String> {
        if effects
            .iter()
            .any(|effect| matches!(effect, Effect::Redraw))
        {
            self.frame()?;
        }
        self.adapter.apply(&effects);
        self.drain()
    }

    fn drain(&mut self) -> Result<(), String> {
        for dispatch in self.interaction.take_dispatch() {
            self.run(dispatch)?;
        }
        while let Some((commit, handle, value, _)) = self.interaction.take_number() {
            let changed = self.session.number_event(commit, handle, value)?;
            self.interaction.number_done();
            self.rebuild |= changed;
        }
        Ok(())
    }

    fn run(&mut self, dispatch: Dispatch) -> Result<(), String> {
        let changed = match &dispatch {
            Dispatch::Editor { .. } => {
                self.adapter.focus(&dispatch);
                false
            }
            Dispatch::Click { commit, handle } => self.session.click(*commit, *handle)?,
            Dispatch::Plot {
                commit,
                identity,
                mode,
                point,
                index,
            } => {
                let selection = match mode {
                    0 => crate::PlotSelection::Point(*point),
                    1 => crate::PlotSelection::Next,
                    _ => crate::PlotSelection::Index(*index),
                };
                self.session.select_plot(*commit, identity, selection)?
            }
            Dispatch::DismissPlots { commit, identities } => {
                let mut changed = false;
                for identity in identities {
                    changed |= self.session.select_plot(
                        *commit,
                        identity,
                        crate::PlotSelection::Index(-1),
                    )?;
                }
                changed
            }
            Dispatch::Reach {
                commit, handles, ..
            } => {
                let mut changed = false;
                for handle in handles {
                    changed |= self.session.click(*commit, *handle)?;
                }
                changed
            }
        };
        self.rebuild |= changed;
        Ok(())
    }

    /// Record a failure and the retry the host offers.
    pub fn fail(&mut self, error: &str) -> Recovery {
        let recovery = self.lifecycle.fail(error);
        self.interaction.set_inert(self.lifecycle.session_failed());
        let effects = self.interaction.cancel_all();
        self.adapter.apply(&effects);
        recovery
    }
}
