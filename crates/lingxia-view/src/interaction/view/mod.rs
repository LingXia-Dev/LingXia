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
mod wheel;
pub use wheel::{Wheel, WheelPhase};

pub use accessibility::AccessibilityAction;
use effects::press;
pub use effects::{AccessibilityEffect, Dispatch, Effect};
pub use keyboard::Key;
pub use pointer::{Pointer, PointerPhase};

/// A user activation that resolved to nothing; the host reports it, it is never silent.
#[derive(Clone, Debug, PartialEq)]
pub struct DroppedActivation {
    pub identity: String,
    pub reason: &'static str,
    /// The frame the activation was made on and the frame it was resolved against.
    pub commit: CommitTicket,
    pub frame: CommitTicket,
}

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
    /// A fine pointer is dragging a selection across `selectable` text.
    selecting: bool,
    /// The innermost `drag` element under the press, and whether the finger moves it.
    drag: Option<String>,
    holding: bool,
    /// Where the scrolled region was when the finger took it, for `snap-always`.
    scroll_from: f32,
    /// Same-axis scrolls inside and around the held element, and the one that moved last.
    inner: Option<String>,
    outer: Option<String>,
    mover: Option<String>,
    /// A finger or button is down.
    touching: bool,
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
    drag: crate::interaction::drag::DragModel,
    reach: ReachTracker,
    ids: SemanticIds,
    slider: SliderInput,
    gesture: Gesture,
    keyboard_focus: Option<usize>,
    pending_focus: Option<PendingFocus>,
    accessibility_focus: Option<u32>,
    hover: Option<u32>,
    exploring: Option<[f32; 2]>,
    /// The region a wheel or trackpad gesture scrolls, until the next one begins.
    wheel_target: Option<String>,
    activation: Option<Activation>,
    dispatches: Vec<Dispatch>,
    inert: bool,
    /// Left the stack: the last frame stays readable for automation until the next one replaces it.
    parked: bool,
    /// Open dialogs, innermost last, each with what had keyboard focus before it opened.
    dialogs: Vec<(String, Option<String>)>,
    /// A mouse or trackpad drives the pointer: drags select text instead of scrolling.
    fine_pointer: bool,
    /// The last input was a key, not a pointer: focus moves (and its ring) follow it.
    keyboard_used: bool,
    /// Activations that could not be resolved against the presented frame.
    dropped: Vec<DroppedActivation>,
    /// Geometry-only frames of one layout since the last content-changed event.
    moved_frames: u32,
    /// A geometry-only change not yet reported to accessibility.
    moved: bool,
}

/// What a presentation produced besides pixels.
pub struct Presented {
    pub effects: Vec<Effect>,
    pub reach: Option<Dispatch>,
}

impl ViewInteraction {
    /// The identity of `node` when it is a `<select>` whose `focus:` delta the session paints.
    pub fn styled_focus(&self, node: usize) -> Option<String> {
        let frame = self.frame.as_ref()?;
        let snapshot = frame.nodes.get(node)?;
        snapshot.focus_style.then(|| snapshot.identity.clone())
    }

    /// Scroll and drag offsets to request with the next frame.
    pub fn offsets(&self) -> HashMap<String, f32> {
        let mut offsets = self.scroll.snapshot();
        self.drag.snapshot_into(&mut offsets);
        offsets
    }

    /// Releases since the last call, for the host to spring to their snap points.
    pub fn take_drag_releases(&mut self) -> Vec<crate::interaction::drag::Release> {
        self.drag.take_released()
    }

    /// A release's spring step; `done` ends it.
    pub fn set_drag_offset(&mut self, identity: &str, offset: f32, done: bool) {
        self.drag.set(identity, offset, done);
    }

    /// A snapping region came to rest: a bound one tells the View its index.
    pub fn snap_settled(&mut self, id: &str) -> Vec<Effect> {
        let Some(commit) = self.frame.as_ref().map(|frame| frame.commit) else {
            return Vec::new();
        };
        match self.scroll.snap_settled(id) {
            Some((handle, index)) => {
                self.dispatches.push(Dispatch::SnapChange {
                    commit,
                    handle,
                    region: id.to_owned(),
                    index,
                });
                vec![Effect::Dispatch]
            }
            None => Vec::new(),
        }
    }

    /// Snapping regions input moved since the last call.
    pub fn take_moved_snaps(&mut self) -> Vec<String> {
        self.scroll.take_moved_snaps()
    }

    /// Place region `id` at `offset` without counting as input (a snap settling).
    pub fn set_scroll_offset(&mut self, id: &str, offset: f32) -> bool {
        self.scroll.scroll_to(id, offset)
    }

    /// Regions whose list anchor began a glide with the last presented frame.
    pub fn take_started_glides(&mut self) -> Vec<String> {
        self.scroll.take_started_glides()
    }

    /// One step of region `id`'s glide (see [`ScrollModel::glide_step`]).
    pub fn glide_step(&mut self, id: &str, now: std::time::Duration, jump: bool) -> Option<bool> {
        if self.gesture.dragging {
            return None;
        }
        self.scroll.glide_step(id, now, jump)
    }

    /// A finger is dragging a scroll region.
    pub fn holding_scroll(&self) -> bool {
        self.gesture.dragging
    }

    /// A finger is down on drag element `identity`: its spring stops where it is.
    pub fn drag_held(&self, identity: &str) -> bool {
        self.gesture.touching && self.gesture.drag.as_deref() == Some(identity)
    }

    fn drag_event(&mut self, phase: &str, index: usize, velocity: f32) {
        if let Some(identity) = self.gesture.drag.clone() {
            self.drag_dispatch(&identity, phase, index, velocity, false);
        }
    }

    /// A release's spring landed on point `index`: the View learns it (and its `bind`).
    pub fn drag_settled(&mut self, identity: &str, index: usize) -> Vec<Effect> {
        let hidden = self.frame.as_ref().is_some_and(|frame| {
            frame.index_of(identity).is_some_and(|node| {
                frame.nodes[node]
                    .drag
                    .as_ref()
                    .is_some_and(|drag| drag.hides.get(index) == Some(&true))
            })
        });
        self.drag_dispatch(identity, "settle", index, 0.0, hidden);
        if self.dispatches.is_empty() {
            Vec::new()
        } else {
            vec![Effect::Dispatch]
        }
    }

    fn drag_dispatch(
        &mut self,
        identity: &str,
        phase: &str,
        index: usize,
        velocity: f32,
        hidden: bool,
    ) {
        let Some(frame) = self.frame.as_ref() else {
            return;
        };
        let Some(handle) = self
            .drag
            .region(identity)
            .map(|drag| drag.handle)
            .filter(|h| *h != 0)
        else {
            return;
        };
        self.dispatches.push(Dispatch::DragEvent {
            commit: frame.commit,
            handle,
            identity: identity.to_owned(),
            payload: serde_json::json!({"phase": phase, "index": index, "velocity": velocity, "hidden": hidden})
                .to_string(),
        });
    }

    /// Automation: scroll a list to an off-screen row's offset, and the list itself on screen;
    /// a list below the fold mounts no rows.
    pub fn scroll_list_to(&mut self, id: &str, offset: f32) -> bool {
        let moved = self.scroll.scroll_to(id, offset);
        let Some(frame) = self.frame.clone() else {
            return moved;
        };
        let list = frame
            .nodes
            .iter()
            .find(|node| node.scroll.as_ref().is_some_and(|scroll| scroll.id == id));
        list.is_some_and(|list| self.scroll.reveal(&frame, list)) || moved
    }

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

    pub fn set_fine_pointer(&mut self, fine: bool) {
        self.fine_pointer = fine;
    }

    /// A double click at a device-pixel point: select the word of selectable text there.
    pub fn select_word_at(&mut self, x: f32, y: f32) -> Vec<Effect> {
        let Some(frame) = self.frame.clone().filter(|_| !self.inert) else {
            return Vec::new();
        };
        let point = self.logical(x, y);
        let Some(index) = frame.selectable_hit(point) else {
            return Vec::new();
        };
        self.dispatches.push(Dispatch::SelectText {
            commit: frame.commit,
            identity: frame.nodes[index].identity.clone(),
            point,
        });
        vec![Effect::Dispatch]
    }

    /// A parked page keeps its last frame and ids so read-only automation still answers
    /// from them; input stops, and the next presented frame starts afresh.
    pub fn park(&mut self) -> Vec<Effect> {
        let mut effects = self.cancel_all();
        self.keyboard_focus = None;
        self.accessibility_focus = None;
        self.dispatches.clear();
        self.parked = true;
        effects.push(Effect::ClearFocusRing);
        effects
    }

    pub fn parked(&self) -> bool {
        self.parked
    }

    /// Forget page-scoped state after reload or park; ids are never reused.
    pub fn reset(&mut self) -> Vec<Effect> {
        let mut effects = self.cancel_all();
        self.parked = false;
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
            drags: Some(&self.drag),
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
        let mut effects = if self.parked {
            self.reset()
        } else {
            Vec::new()
        };
        let previous = self.frame.take();
        let scale_changed = previous
            .as_ref()
            .is_none_or(|previous| previous.scale != next.scale);
        self.slider.presented(&next.nodes, scale_changed);
        let same_layout = !scale_changed
            && previous
                .as_ref()
                .is_some_and(|previous| previous.same_layout(&next));
        let same = !same_layout
            && !scale_changed
            && previous
                .as_ref()
                .is_some_and(|previous| previous.same_content(&next));
        if same_layout {
            // Animation and scroll repaint one layout: indices still name the same nodes,
            // so press and focus stay, and moving bounds are reported at a quarter rate.
            if self.gesture.pressed.is_some() {
                self.gesture.pressed_commit = Some(next.commit);
            }
            if let Some(id) = self.accessibility_focus
                && self
                    .ids
                    .index(id)
                    .and_then(|index| next.node(index))
                    .is_none_or(|node| !frame::visible(node))
            {
                self.accessibility_focus = None;
                effects.push(Effect::Accessibility(AccessibilityEffect::FocusCleared {
                    id,
                }));
            }
            for event in projection::semantic_changes(previous.as_deref(), &next, &self.ids) {
                effects.push(Effect::Accessibility(AccessibilityEffect::Change(event)));
            }
            // Within one layout only presented geometry moves; compare that, not whole frames.
            let moved = previous.as_ref().is_some_and(|previous| {
                previous
                    .nodes
                    .iter()
                    .zip(&next.nodes)
                    .any(|(a, b)| a.bounds != b.bounds || a.visible_bounds != b.visible_bounds)
            });
            if moved {
                self.moved_frames += 1;
                self.moved = !self.moved_frames.is_multiple_of(4);
                if !self.moved {
                    effects.push(Effect::Accessibility(AccessibilityEffect::Change(
                        projection::Event::ContentChanged,
                    )));
                }
            }
        } else if same {
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
                // A press holds onto its control when a newer View changed it (a label that
                // flipped under the finger): releasing it still activates that control.
                previous
                    .as_ref()
                    .and_then(|frame| self.gesture.pressed.and_then(|index| frame.node(index)))
                    .and_then(|node| next.rebind(node).or_else(|| next.press_target(node)))
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
                    node.identity == identity
                        && (frame::interactive(node) || frame::draggable(node))
                        && frame::visible(node)
                })
            {
                self.keyboard_focus = Some(index);
                effects.push(Effect::FocusRing { node: index });
            }
            self.moved_frames = 0;
            self.moved = false;
            for event in projection::changes(previous.as_deref(), &next, &self.ids) {
                effects.push(Effect::Accessibility(AccessibilityEffect::Change(event)));
            }
        }
        self.scroll
            .accept(&next, requested_revision, requested_offsets);
        self.drag.accept(&next);
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

    /// Presentation went quiet: report geometry still owed to accessibility.
    pub fn settled(&mut self) -> Vec<Effect> {
        self.moved_frames = 0;
        if std::mem::take(&mut self.moved) {
            vec![Effect::Accessibility(AccessibilityEffect::Change(
                projection::Event::ContentChanged,
            ))]
        } else {
            Vec::new()
        }
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
        // A select opens the View's picker; the choice returns as the select's own choice.
        if node
            .select
            .as_ref()
            .is_some_and(|select| select.on_select.is_some())
        {
            self.dispatches.push(Dispatch::OpenSelect {
                commit: frame.commit,
                identity: node.identity.clone(),
            });
            return vec![Effect::Dispatch];
        }
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

    /// Activations, dispatches or slider numbers wait for the host.
    pub fn waiting(&self) -> bool {
        self.activation.is_some() || !self.dispatches.is_empty() || self.slider.pending.is_some()
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
            // The same control by identity when its content changed since the tap (a toggle's
            // label, say): a newer frame must not cost the user the input.
            let index = if activation.commit == frame.commit && activation.scale == frame.scale {
                frame.index_of(&activation.node.identity)
            } else if activation.scale == frame.scale {
                frame
                    .rebind(&activation.node)
                    .or_else(|| frame.index_of(&activation.node.identity))
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
                } else {
                    self.dropped.push(DroppedActivation {
                        identity: activation.node.identity.clone(),
                        reason: "no longer clickable",
                        commit: activation.commit,
                        frame: frame.commit,
                    });
                }
            } else {
                self.dropped.push(DroppedActivation {
                    identity: activation.node.identity.clone(),
                    reason: if activation.scale != frame.scale {
                        "display scale changed"
                    } else {
                        "the control is gone"
                    },
                    commit: activation.commit,
                    frame: frame.commit,
                });
            }
        }
        dispatches
    }

    /// Activations dropped since the last call.
    pub fn take_dropped(&mut self) -> Vec<DroppedActivation> {
        std::mem::take(&mut self.dropped)
    }

    // ---- sliders -------------------------------------------------------
}
