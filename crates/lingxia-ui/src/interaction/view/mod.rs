//! Input policy for one presented View: pointer gestures, keyboard focus,
//! accessibility actions and presentation side effects. Adapters translate OS
//! events into these calls and execute the returned effects.
use crate::interaction::frame::{self, PresentedFrame};
use crate::interaction::reach::{ReachTracker, Reached};
use crate::interaction::scroll::ScrollModel;
use crate::interaction::slider::{self, Proposal, SliderInput};
use crate::scene::node::{NodeSnapshot, ScrollAxis};
use crate::semantics::ids::SemanticIds;
use crate::semantics::projection::{self, Projection};
use crate::session::lifecycle::{CommitTicket, PageTicket};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

mod accessibility;
mod automation;
pub use automation::AutomationAct;
mod effects;
mod keyboard;
mod number;
mod pointer;

pub use accessibility::AccessibilityAction;
use effects::press;
pub use effects::{AccessibilityEffect, Dispatch, Effect};
pub use keyboard::Key;
pub use pointer::{Pointer, PointerPhase};

struct Activation {
    node: NodeSnapshot,
    commit: CommitTicket,
    scale: f32,
    point: Option<[f32; 2]>,
    word: bool,
    selection: Option<i64>,
}

#[derive(Default)]
struct Gesture {
    pressed: Option<usize>,
    pressed_commit: Option<CommitTicket>,
    down: [f32; 2],
    last: f32,
    axis: Option<ScrollAxis>,
    target: Option<String>,
    dragging: bool,
    moved: bool,
    long_pressed: bool,
    /// A selectable static text under the press, for long-press selection.
    selectable: Option<usize>,
}

struct PendingFocus {
    identity: String,
    activate: bool,
    page: PageTicket,
}

#[derive(Default)]
pub struct ViewInteraction {
    frame: Option<Arc<PresentedFrame>>,
    scroll: ScrollModel,
    reach: ReachTracker,
    ids: SemanticIds,
    slider: SliderInput,
    gesture: Gesture,
    keyboard_focus: Option<usize>,
    pending_focus: Option<PendingFocus>,
    accessibility_focus: Option<u32>,
    hover: Option<u32>,
    exploring: Option<[f32; 2]>,
    activation: Option<Activation>,
    dispatches: Vec<Dispatch>,
    inert: bool,
}

/// What a presentation produced besides pixels.
pub struct Presented {
    pub effects: Vec<Effect>,
    pub reach: Option<Dispatch>,
}

impl ViewInteraction {
    pub fn frame(&self) -> Option<&Arc<PresentedFrame>> {
        self.frame.as_ref()
    }

    fn scale(&self) -> f32 {
        self.frame
            .as_ref()
            .map(|frame| frame.scale)
            .filter(|scale| *scale > 0.0)
            .unwrap_or(1.0)
    }

    fn logical(&self, x: f32, y: f32) -> [f32; 2] {
        let scale = self.scale();
        [x / scale, y / scale]
    }

    pub fn scroll(&self) -> &ScrollModel {
        &self.scroll
    }

    /// A fling step in screen pixels; RTL horizontal regions advance the other way.
    pub fn scroll_by(&mut self, region: &str, delta_px: f32) -> bool {
        let scale = self.scale();
        self.scroll.drag_by(region, delta_px / scale)
    }

    /// Only an interrupted session sets this; content stays readable, input stops.
    pub fn set_inert(&mut self, inert: bool) {
        self.inert = inert;
    }

    /// Forget page-scoped state after reload or park; ids are never reused.
    pub fn reset(&mut self) -> Vec<Effect> {
        let mut effects = self.cancel_all();
        self.frame = None;
        self.scroll.clear();
        self.reach.clear();
        self.ids.clear();
        self.keyboard_focus = None;
        self.accessibility_focus = None;
        self.dispatches.clear();
        self.activation = None;
        effects.push(Effect::ClearFocusRing);
        effects
    }

    /// Stop gestures, pending navigation and slider input: pause, focus loss or failure.
    pub fn cancel_all(&mut self) -> Vec<Effect> {
        self.slider.cancel();
        self.pending_focus = None;
        self.activation = None;
        let mut effects = self.cancel_press();
        effects.push(Effect::StopFling);
        self.gesture = Gesture::default();
        if self.hover.take().is_some() || self.exploring.take().is_some() {
            effects.extend(self.hover_to(None));
        }
        effects
    }

    fn cancel_press(&mut self) -> Vec<Effect> {
        self.gesture.pressed = None;
        vec![Effect::CancelLongPress, Effect::ClearPress]
    }

    pub fn semantics(&self) -> Option<Projection<'_>> {
        Some(Projection {
            frame: self.frame.as_deref()?,
            ids: &self.ids,
            keyboard_focus: self.keyboard_focus,
            inert: self.inert,
        })
    }

    pub fn accessibility_focus(&self) -> Option<u32> {
        self.accessibility_focus
    }

    pub fn keyboard_focus(&self) -> Option<usize> {
        self.keyboard_focus
    }

    /// Adopt a presented frame and compute its side effects.
    pub fn presented(
        &mut self,
        next: Arc<PresentedFrame>,
        requested_revision: u64,
        requested_offsets: &HashMap<String, f32>,
        page: PageTicket,
    ) -> Presented {
        let mut effects = Vec::new();
        let previous = self.frame.take();
        let scale_changed = previous
            .as_ref()
            .is_none_or(|previous| previous.scale != next.scale);
        self.slider.presented(&next.nodes, scale_changed);
        let same = previous.as_ref().is_some_and(|previous| {
            !scale_changed
                && serde_json::to_string(&previous.nodes).ok()
                    == serde_json::to_string(&next.nodes).ok()
        });
        if same {
            if self.gesture.pressed.is_some() {
                self.gesture.pressed_commit = Some(next.commit);
            }
        } else {
            let focused = previous
                .as_ref()
                .and_then(|frame| self.keyboard_focus.and_then(|index| frame.node(index)))
                .map(|node| node.identity.clone());
            let rebound = if scale_changed {
                None
            } else {
                previous
                    .as_ref()
                    .and_then(|frame| self.gesture.pressed.and_then(|index| frame.node(index)))
                    .and_then(|node| next.rebind(node))
            };
            if rebound.is_none() {
                effects.extend(self.cancel_press());
            }
            effects.push(Effect::ClearFocusRing);
            self.keyboard_focus = None;
            self.ids
                .update(next.nodes.iter().map(|node| node.identity.as_str()));
            if let Some(rebound) = rebound {
                self.gesture.pressed = Some(rebound);
                self.gesture.pressed_commit = Some(next.commit);
                effects.push(press(&next, rebound));
            }
            if let Some(id) = self.accessibility_focus {
                let gone = self
                    .ids
                    .index(id)
                    .and_then(|index| next.node(index))
                    .is_none_or(|node| !frame::visible(node));
                if gone {
                    self.accessibility_focus = None;
                    effects.push(Effect::Accessibility(AccessibilityEffect::FocusCleared {
                        id,
                    }));
                }
            }
            if let Some(identity) = focused
                && let Some(index) = next.nodes.iter().position(|node| {
                    node.identity == identity && frame::interactive(node) && frame::visible(node)
                })
            {
                self.keyboard_focus = Some(index);
                effects.push(Effect::FocusRing { node: index });
            }
            for event in projection::changes(previous.as_deref(), &next, &self.ids) {
                effects.push(Effect::Accessibility(AccessibilityEffect::Change(event)));
            }
        }
        self.scroll
            .accept(&next, requested_revision, requested_offsets);
        let Reached { handles, more } = self.reach.presented(&next);
        let reach = (!handles.is_empty() || more).then(|| Dispatch::Reach {
            commit: next.commit,
            handles,
            more,
        });
        self.frame = Some(next);
        if let Some(point) = self.exploring {
            let target = self.semantics().and_then(|semantics| semantics.hit(point));
            effects.extend(self.hover_to(target));
        }
        effects.extend(self.finish_keyboard(page));
        if self.activation.is_some() || !self.dispatches.is_empty() {
            effects.push(Effect::Dispatch);
        }
        if self.slider.pending.is_some() {
            effects.push(Effect::Number);
        }
        Presented { effects, reach }
    }

    // ---- pointer -------------------------------------------------------

    fn activate(
        &mut self,
        index: usize,
        point: Option<[f32; 2]>,
        word: bool,
        selection: Option<i64>,
    ) -> Vec<Effect> {
        let Some(frame) = self.frame.clone() else {
            return Vec::new();
        };
        let Some(node) = frame.node(index).filter(|node| frame::visible(node)) else {
            return Vec::new();
        };
        self.pending_focus = None;
        self.slider.cancel();
        let mut effects = Vec::new();
        if let Some(point) = point {
            effects.extend(self.dismiss_plots(frame.outside_plot_selections(point)));
        }
        self.activation = Some(Activation {
            node: node.clone(),
            commit: frame.commit,
            scale: frame.scale,
            point,
            word,
            selection,
        });
        effects.push(Effect::Dispatch);
        effects
    }

    fn dismiss_plots(&mut self, identities: Vec<String>) -> Vec<Effect> {
        let Some(frame) = &self.frame else {
            return Vec::new();
        };
        if identities.is_empty() {
            return Vec::new();
        }
        self.dispatches.push(Dispatch::DismissPlots {
            commit: frame.commit,
            identities,
        });
        vec![Effect::Dispatch]
    }

    /// Resolve queued work against the current frame, rebinding a control whose callbacks changed.
    pub fn take_dispatch(&mut self) -> Vec<Dispatch> {
        let mut dispatches = std::mem::take(&mut self.dispatches);
        let Some(frame) = self.frame.clone() else {
            return Vec::new();
        };
        for dispatch in &mut dispatches {
            if let Dispatch::DismissPlots { commit, identities } = dispatch
                && *commit != frame.commit
            {
                identities.retain(|identity| {
                    frame
                        .index_of(identity)
                        .is_some_and(|i| frame::plot_can_clear(&frame.nodes[i]))
                });
                *commit = frame.commit;
            }
        }
        dispatches.retain(|dispatch| !matches!(dispatch, Dispatch::DismissPlots { identities, .. } if identities.is_empty()));
        if let Some(activation) = self.activation.take() {
            let index = if activation.commit == frame.commit && activation.scale == frame.scale {
                frame.index_of(&activation.node.identity)
            } else if activation.scale == frame.scale {
                frame.rebind(&activation.node)
            } else {
                None
            };
            if let Some(node) = index.and_then(|index| frame.node(index).map(|node| (index, node)))
            {
                let (index, node) = node;
                if frame::editable(node) {
                    dispatches.push(Dispatch::Editor {
                        node: index,
                        commit: frame.commit,
                        point: activation.point,
                        word: activation.word,
                    });
                } else if frame::plot_enabled(node) {
                    let (mode, index) = match (activation.selection, activation.point) {
                        (Some(selection), _) => (2, selection),
                        (None, Some(_)) => (0, -1),
                        (None, None) => (1, -1),
                    };
                    dispatches.push(Dispatch::Plot {
                        commit: frame.commit,
                        identity: node.identity.clone(),
                        mode,
                        point: activation.point.unwrap_or([0.0; 2]),
                        index,
                    });
                } else if let Some(handle) = node.on_click.filter(|handle| *handle != 0) {
                    dispatches.push(Dispatch::Click {
                        commit: frame.commit,
                        handle,
                    });
                }
            }
        }
        dispatches
    }

    // ---- sliders -------------------------------------------------------
}
