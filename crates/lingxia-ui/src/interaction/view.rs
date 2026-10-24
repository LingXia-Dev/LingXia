//! Input policy for one presented View: pointer gestures, keyboard focus,
//! accessibility actions and presentation side effects. Adapters translate OS
//! events into these calls and execute the returned effects.
use super::frame::{self, PresentedFrame};
use super::reach::ReachTracker;
use super::scroll::ScrollModel;
use super::slider::{self, Proposal, SliderInput};
use super::{keyboard, reach::Reached};
use crate::scene::node::{NodeSnapshot, ScrollAxis};
use crate::semantics::ids::SemanticIds;
use crate::semantics::projection::{self, Projection};
use crate::session::lifecycle::{CommitTicket, PageTicket};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

/// Work the adapter performs with OS mechanics.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Effect {
    /// Pressed feedback: a styled node repaints through `set_pointer_state`,
    /// others get a platform overlay over the clipped shape.
    Press {
        node: usize,
        identity: String,
        styled: bool,
    },
    ClearPress,
    /// Draw the keyboard focus ring.
    FocusRing {
        node: usize,
    },
    ClearFocusRing,
    /// Offsets changed: present again without rerunning the View.
    Redraw,
    /// Keep (or release) the gesture from ancestor interception.
    Claim {
        exclusive: bool,
    },
    ArmLongPress,
    CancelLongPress,
    /// Continue a released drag with platform fling physics, feeding `scroll_by`.
    Fling {
        region: String,
        horizontal: bool,
        velocity: f32,
    },
    StopFling,
    /// A dispatch is queued; drain it with `take_dispatch` when the worker is idle.
    Dispatch,
    /// A slider value is queued; drain it with `take_number`.
    Number,
    /// A click landed; play feedback and announce it.
    Clicked,
    Haptic,
    /// Edit text on behalf of accessibility; the adapter forwards it to its input method.
    EditText {
        node: usize,
        text: Option<String>,
        start: i32,
        end: i32,
    },
    Accessibility(AccessibilityEffect),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum AccessibilityEffect {
    Focused { id: u32 },
    FocusCleared { id: u32 },
    InputFocused { id: u32 },
    HoverEnter { id: u32 },
    HoverExit { id: u32 },
    Change(projection::Event),
}

fn press(frame: &PresentedFrame, node: usize) -> Effect {
    Effect::Press {
        node,
        identity: frame.nodes[node].identity.clone(),
        styled: frame.nodes[node].pressed_style,
    }
}

/// Work the View worker runs against the frame it was resolved from.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Dispatch {
    /// The adapter's input method focuses or places the caret in an editor.
    Editor {
        node: usize,
        commit: CommitTicket,
        point: Option<[f32; 2]>,
        word: bool,
    },
    Click {
        commit: CommitTicket,
        handle: u64,
    },
    /// Mode 0: point, 1: next item, 2: explicit index (-1 clears).
    Plot {
        commit: CommitTicket,
        identity: String,
        mode: u8,
        point: [f32; 2],
        index: i64,
    },
    DismissPlots {
        commit: CommitTicket,
        identities: Vec<String>,
    },
    /// Edges reached by a presented frame.
    Reach {
        commit: CommitTicket,
        handles: Vec<u64>,
        more: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerPhase {
    Down,
    Move,
    Up,
    Cancel,
}

/// Device-pixel pointer input; velocity is px/s along each axis on release.
#[derive(Clone, Copy, Debug)]
pub struct Pointer {
    pub phase: PointerPhase,
    pub x: f32,
    pub y: f32,
    pub slop: f32,
    pub velocity: [f32; 2],
    pub min_fling: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Tab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Activate,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AccessibilityAction {
    Focus,
    ClearFocus,
    Click,
    ShowOnScreen,
    ScrollForward,
    ScrollBackward,
    SetProgress(f64),
    SetText(String),
    SetSelection(i32, i32),
    ClearSelection,
    SelectItem(usize),
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

    pub fn scroll_by(&mut self, region: &str, delta_px: f32) -> bool {
        let scale = self.scale();
        self.scroll.scroll_by(region, delta_px / scale)
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

    /// A gesture the View does not consume returns no effects and `false`.
    pub fn pointer(&mut self, event: Pointer, page: PageTicket) -> (bool, Vec<Effect>) {
        if self.inert {
            return (false, Vec::new());
        }
        let Some(frame) = self.frame.clone() else {
            return (false, Vec::new());
        };
        let point = self.logical(event.x, event.y);
        let mut effects = Vec::new();
        match event.phase {
            PointerPhase::Down => {
                self.slider.cancel();
                self.pending_focus = None;
                effects.extend(self.cancel_press());
                effects.push(Effect::StopFling);
                effects.push(Effect::ClearFocusRing);
                self.keyboard_focus = None;
                self.gesture = Gesture {
                    pressed: frame.hit(point),
                    pressed_commit: Some(frame.commit),
                    down: [event.x, event.y],
                    last: event.y,
                    target: frame.scroll_hit(point),
                    ..Default::default()
                };
                let pressed = self.gesture.pressed;
                self.slider
                    .begin(pressed.and_then(|index| frame.node(index)), point);
                if pressed.is_none()
                    && self.gesture.target.is_none()
                    && frame.outside_plot_selections(point).is_empty()
                {
                    return (false, effects);
                }
                if self.gesture.target.is_some() {
                    effects.push(Effect::Claim { exclusive: true });
                }
                if let Some(index) = pressed {
                    effects.push(press(&frame, index));
                    if frame.node(index).is_some_and(frame::editable) {
                        effects.push(Effect::ArmLongPress);
                    }
                }
            }
            PointerPhase::Move => {
                if self.gesture.long_pressed {
                    return (true, effects);
                }
                let scale = frame.scale;
                if self.slider.moved(point, event.slop / scale) {
                    effects.extend(self.cancel_press());
                    if let Some(node) = self.slider.gesture_node().cloned()
                        && let Some(value) = slider::value_at(&node, point[0])
                    {
                        effects.extend(self.propose_number_node(&node, value, page));
                    }
                    return (true, effects);
                }
                let dx = event.x - self.gesture.down[0];
                let dy = event.y - self.gesture.down[1];
                if dx.abs() > event.slop || dy.abs() > event.slop {
                    self.gesture.moved = true;
                }
                if !self.gesture.dragging && self.gesture.moved && self.gesture.target.is_some() {
                    let axis = if dx.abs() > dy.abs() {
                        ScrollAxis::Horizontal
                    } else {
                        ScrollAxis::Vertical
                    };
                    if let Some(target) = self
                        .scroll
                        .target_for_axis(self.gesture.target.as_deref(), axis)
                    {
                        self.gesture.axis = Some(axis);
                        self.gesture.target = Some(target);
                        self.gesture.dragging = true;
                        effects.extend(self.cancel_press());
                        let (start, distance) = match axis {
                            ScrollAxis::Horizontal => (self.gesture.down[0], dx),
                            ScrollAxis::Vertical => (self.gesture.down[1], dy),
                        };
                        self.gesture.last = start + event.slop.copysign(distance);
                    }
                }
                if self.gesture.dragging {
                    let position = if self.gesture.axis == Some(ScrollAxis::Horizontal) {
                        event.x
                    } else {
                        event.y
                    };
                    let delta = self.gesture.last - position;
                    self.gesture.last = position;
                    let target = self.gesture.target.clone().unwrap_or_default();
                    if self.scroll.scroll_by(&target, delta / scale) {
                        effects.push(Effect::Redraw);
                    } else {
                        effects.push(Effect::Claim { exclusive: false });
                    }
                } else if self.gesture.moved {
                    effects.extend(self.cancel_press());
                }
            }
            PointerPhase::Up => {
                if self.slider.has_gesture() {
                    let node = self.slider.end();
                    effects.extend(self.cancel_press());
                    effects.push(Effect::StopFling);
                    if let Some(node) = node
                        && let Some(value) = slider::value_at(&node, point[0])
                    {
                        effects.extend(self.propose_number_node(&node, value, page));
                    }
                    effects.push(Effect::Claim { exclusive: false });
                    return (true, effects);
                }
                let target = self.gesture.pressed;
                let same_frame = self.gesture.pressed_commit == Some(frame.commit);
                effects.extend(self.cancel_press());
                if self.gesture.dragging {
                    let axis = self.gesture.axis.unwrap_or(ScrollAxis::Vertical);
                    let speed = -match axis {
                        ScrollAxis::Horizontal => event.velocity[0],
                        ScrollAxis::Vertical => event.velocity[1],
                    };
                    if speed.abs() >= event.min_fling
                        && let Some(region) = self.gesture.target.clone()
                    {
                        effects.push(Effect::Fling {
                            region,
                            horizontal: axis == ScrollAxis::Horizontal,
                            velocity: speed,
                        });
                    }
                } else if let Some(index) =
                    target.filter(|index| same_frame && frame.hit(point) == Some(*index))
                {
                    effects.push(Effect::Clicked);
                    effects.extend(self.activate(index, Some(point), false, None));
                } else if target.is_none()
                    && same_frame
                    && !self.gesture.moved
                    && !self.gesture.long_pressed
                    && frame.hit(point).is_none()
                {
                    effects.push(Effect::Clicked);
                    effects.extend(self.dismiss_plots(frame.outside_plot_selections(point)));
                }
                self.gesture.dragging = false;
                effects.push(Effect::Claim { exclusive: false });
            }
            PointerPhase::Cancel => {
                self.gesture.moved = true;
                self.slider.cancel();
                effects.extend(self.cancel_press());
                effects.push(Effect::StopFling);
                effects.push(Effect::Claim { exclusive: false });
            }
        }
        (true, effects)
    }

    /// The long-press timer fired for the pressed editor.
    pub fn long_press(&mut self) -> Vec<Effect> {
        let Some(frame) = self.frame.clone() else {
            return Vec::new();
        };
        let (Some(index), Some(commit)) = (self.gesture.pressed, self.gesture.pressed_commit)
        else {
            return Vec::new();
        };
        if commit != frame.commit {
            return Vec::new();
        }
        self.gesture.long_pressed = true;
        let mut effects = self.cancel_press();
        effects.push(Effect::Haptic);
        let point = self.logical(self.gesture.down[0], self.gesture.down[1]);
        effects.extend(self.activate(index, Some(point), true, None));
        effects
    }

    /// Whether a vertical gesture can still scroll content, for host pull-to-refresh arbitration.
    pub fn can_scroll_vertically(&self, direction: i32) -> bool {
        self.gesture.long_pressed
            || self.slider.dragging()
            || self.scroll.can_scroll(
                self.gesture.target.as_deref(),
                direction,
                ScrollAxis::Vertical,
            )
    }

    // ---- activation ----------------------------------------------------

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

    fn propose_number_node(
        &mut self,
        node: &NodeSnapshot,
        value: f64,
        page: PageTicket,
    ) -> Vec<Effect> {
        if self.slider.propose(node, value, page) {
            vec![Effect::Number]
        } else {
            Vec::new()
        }
    }

    /// Propose a value for page `page`; the adapter dispatches it with `take_number`.
    pub fn propose_number(&mut self, index: usize, value: f64, page: PageTicket) -> Vec<Effect> {
        let Some(node) = self
            .frame
            .as_ref()
            .and_then(|frame| frame.node(index))
            .cloned()
        else {
            return Vec::new();
        };
        if !frame::slider_enabled(&node)
            || (!frame::visible(&node)
                && self
                    .pending_focus
                    .as_ref()
                    .is_none_or(|p| p.identity != node.identity))
        {
            return Vec::new();
        }
        if self.slider.propose(&node, value, page) {
            vec![Effect::Number]
        } else {
            Vec::new()
        }
    }

    /// The next slider value to send: commit, callback handle, value and page.
    pub fn take_number(&mut self) -> Option<(CommitTicket, u64, f64, Proposal)> {
        if self.pending_focus.is_some() {
            return None;
        }
        let frame = self.frame.clone()?;
        let proposal = self.slider.begin_flight()?;
        let node = frame.nodes.iter().find(|node| proposal.matches(node))?;
        let range = node.range.as_ref()?;
        if !frame::visible(node) || proposal.value == range.value {
            self.slider.end_flight();
            return None;
        }
        Some((frame.commit, range.on_change?, proposal.value, proposal))
    }

    pub fn number_done(&mut self) {
        self.slider.end_flight();
    }

    pub fn number_current(&self, proposal: &Proposal) -> bool {
        self.slider.is_current(proposal)
    }

    // ---- keyboard ------------------------------------------------------

    fn revealable<'a>(&'a self, frame: &'a PresentedFrame) -> impl Fn(&NodeSnapshot) -> bool + 'a {
        move |node| self.scroll.can_reveal(frame, node)
    }

    fn keyboard_origin(&self) -> Option<usize> {
        let frame = self.frame.as_ref()?;
        match &self.pending_focus {
            Some(pending) => frame.index_of(&pending.identity),
            None => self.keyboard_focus,
        }
    }

    /// Navigate to `index`; reveal it first if needed, then optionally activate it.
    fn navigate(&mut self, index: usize, activate: bool, page: PageTicket) -> Vec<Effect> {
        let Some(frame) = self.frame.clone() else {
            return Vec::new();
        };
        if !keyboard::candidate(&frame, index, self.revealable(&frame)) {
            return Vec::new();
        }
        self.pending_focus = Some(PendingFocus {
            identity: frame.nodes[index].identity.clone(),
            activate,
            page,
        });
        let mut effects = vec![Effect::StopFling];
        if self.scroll.reveal(&frame, &frame.nodes[index]) {
            effects.push(Effect::Redraw);
            return effects;
        }
        effects.extend(self.finish_keyboard(page));
        effects
    }

    fn finish_keyboard(&mut self, page: PageTicket) -> Vec<Effect> {
        let Some(pending) = self.pending_focus.take() else {
            return Vec::new();
        };
        let Some(frame) = self.frame.clone() else {
            return Vec::new();
        };
        if pending.page != page {
            return Vec::new();
        }
        let Some(index) = frame.index_of(&pending.identity) else {
            return Vec::new();
        };
        if !keyboard::candidate(&frame, index, self.revealable(&frame)) {
            return Vec::new();
        }
        if self.scroll.reveal(&frame, &frame.nodes[index]) {
            self.pending_focus = Some(pending);
            return vec![Effect::Redraw];
        }
        if !frame::visible(&frame.nodes[index]) {
            return Vec::new();
        }
        let changed = self.keyboard_focus != Some(index);
        self.keyboard_focus = Some(index);
        let mut effects = vec![Effect::FocusRing { node: index }];
        if changed && let Some(id) = self.ids.id(index) {
            effects.push(Effect::Accessibility(AccessibilityEffect::InputFocused {
                id,
            }));
        }
        if pending.activate {
            effects.extend(self.activate(index, None, false, None));
        }
        effects
    }

    /// Hardware keys while no editor owns focus. Returns whether the key was consumed.
    pub fn key(
        &mut self,
        key: Key,
        shift: bool,
        repeat: bool,
        page: PageTicket,
    ) -> (bool, Vec<Effect>) {
        if self.inert {
            return (false, Vec::new());
        }
        let Some(frame) = self.frame.clone() else {
            return (false, Vec::new());
        };
        let origin = self.keyboard_origin();
        if let Some(index) = origin
            && let Some(node) = frame.node(index).filter(|node| frame::slider_enabled(node))
        {
            let range = node.range.clone().unwrap();
            let value = self.slider.value(node).unwrap_or(range.value);
            let next = match key {
                Key::Left | Key::Down => Some(slider::shift(&range, value, -1)),
                Key::Right | Key::Up => Some(slider::shift(&range, value, 1)),
                Key::Home => Some(range.min),
                Key::End => Some(range.max),
                _ => None,
            };
            if let Some(next) = next {
                return (true, self.propose_number(index, next, page));
            }
            if key == Key::Activate {
                return (true, Vec::new());
            }
        }
        if matches!(key, Key::Left | Key::Right | Key::Up | Key::Down)
            && let Some(index) = origin.filter(|index| keyboard::is_option(&frame, *index))
        {
            let direction = if matches!(key, Key::Left | Key::Up) {
                -1
            } else {
                1
            };
            let effects = keyboard::adjacent_option(&frame, index, direction)
                .map(|next| self.navigate(next, true, page))
                .unwrap_or_default();
            return (true, effects);
        }
        if matches!(key, Key::Tab | Key::Up | Key::Down) {
            let direction = if shift || key == Key::Up { -1 } else { 1 };
            if let Some(next) = keyboard::next(&frame, origin, direction, self.revealable(&frame)) {
                return (true, self.navigate(next, false, page));
            }
        }
        if key == Key::Activate {
            if let Some(pending) = &mut self.pending_focus {
                if !repeat {
                    pending.activate = true;
                }
                return (true, Vec::new());
            }
            if let Some(index) = self.keyboard_focus {
                return (
                    true,
                    if repeat {
                        Vec::new()
                    } else {
                        self.activate(index, None, false, None)
                    },
                );
            }
        }
        (false, Vec::new())
    }

    /// The host view gained keyboard focus from the system focus search.
    pub fn focus_entered(&mut self, backward: bool, page: PageTicket) -> Vec<Effect> {
        let Some(frame) = self.frame.clone() else {
            return Vec::new();
        };
        match keyboard::next(
            &frame,
            self.keyboard_origin(),
            if backward { -1 } else { 1 },
            self.revealable(&frame),
        ) {
            Some(next) => self.navigate(next, false, page),
            None => Vec::new(),
        }
    }

    pub fn focus_left(&mut self) -> Vec<Effect> {
        self.slider.cancel();
        self.pending_focus = None;
        self.keyboard_focus = None;
        vec![Effect::ClearFocusRing]
    }

    // ---- accessibility -------------------------------------------------

    fn hover_to(&mut self, target: Option<u32>) -> Vec<Effect> {
        if target == self.hover {
            return Vec::new();
        }
        let old = std::mem::replace(&mut self.hover, target);
        let mut effects = Vec::new();
        if let Some(id) = target {
            effects.push(Effect::Accessibility(AccessibilityEffect::HoverEnter {
                id,
            }));
        }
        if let Some(id) = old {
            effects.push(Effect::Accessibility(AccessibilityEffect::HoverExit { id }));
        }
        effects
    }

    /// Touch exploration at a device-pixel point; `None` ends exploration.
    pub fn explore(&mut self, point: Option<[f32; 2]>) -> (bool, Vec<Effect>) {
        self.exploring = point.map(|[x, y]| self.logical(x, y));
        let target = self
            .exploring
            .and_then(|point| self.semantics().and_then(|semantics| semantics.hit(point)));
        let effects = self.hover_to(target);
        (self.hover.is_some(), effects)
    }

    pub fn accessibility(
        &mut self,
        id: u32,
        action: AccessibilityAction,
        page: PageTicket,
    ) -> (bool, Vec<Effect>) {
        if self.inert
            && !matches!(
                action,
                AccessibilityAction::Focus | AccessibilityAction::ClearFocus
            )
        {
            return (false, Vec::new());
        }
        let Some(frame) = self.frame.clone() else {
            return (false, Vec::new());
        };
        let Some(index) = self.ids.index(id) else {
            return (false, Vec::new());
        };
        let node = &frame.nodes[index];
        if node.accessibility_hidden.unwrap_or(false) {
            return (false, Vec::new());
        }
        self.pending_focus = None;
        self.slider.end();
        let visible = frame::visible(node);
        match action {
            AccessibilityAction::Focus | AccessibilityAction::ClearFocus => {
                let focus = action == AccessibilityAction::Focus;
                if focus == (self.accessibility_focus == Some(id)) {
                    return (false, Vec::new());
                }
                let mut effects = Vec::new();
                if focus && let Some(old) = self.accessibility_focus {
                    effects.push(Effect::Accessibility(AccessibilityEffect::FocusCleared {
                        id: old,
                    }));
                }
                self.accessibility_focus = focus.then_some(id);
                effects.push(Effect::Accessibility(if focus {
                    AccessibilityEffect::Focused { id }
                } else {
                    AccessibilityEffect::FocusCleared { id }
                }));
                (true, effects)
            }
            AccessibilityAction::ShowOnScreen if projection::meaningful(node) => {
                let moved = self.scroll.reveal(&frame, node);
                let mut effects = vec![Effect::StopFling];
                if moved {
                    effects.push(Effect::Redraw);
                }
                (moved || visible, effects)
            }
            _ if !visible => (false, Vec::new()),
            AccessibilityAction::SetProgress(value) if node.range.is_some() => {
                let effects = self.propose_number(index, value, page);
                (!effects.is_empty(), effects)
            }
            AccessibilityAction::ScrollForward | AccessibilityAction::ScrollBackward
                if node.range.is_some() =>
            {
                let range = node.range.clone().unwrap();
                let value = self.slider.value(node).unwrap_or(range.value);
                let direction = if action == AccessibilityAction::ScrollForward {
                    1
                } else {
                    -1
                };
                let effects =
                    self.propose_number(index, slider::shift(&range, value, direction), page);
                (!effects.is_empty(), effects)
            }
            AccessibilityAction::SetText(text) if frame::editable(node) => {
                if text.len() > crate::text::input::MAX_TEXT_BYTES {
                    return (false, Vec::new());
                }
                (
                    true,
                    vec![Effect::EditText {
                        node: index,
                        text: Some(text),
                        start: -1,
                        end: -1,
                    }],
                )
            }
            AccessibilityAction::SetSelection(start, end) if frame::editable(node) => (
                true,
                vec![Effect::EditText {
                    node: index,
                    text: None,
                    start,
                    end,
                }],
            ),
            AccessibilityAction::ScrollForward | AccessibilityAction::ScrollBackward
                if node.scroll.is_some() =>
            {
                let scroll = node.scroll.as_ref().unwrap();
                let length = match scroll.axis {
                    ScrollAxis::Horizontal => node.visible_bounds[2],
                    ScrollAxis::Vertical => node.visible_bounds[3],
                } * 0.8;
                let distance = if action == AccessibilityAction::ScrollForward {
                    length
                } else {
                    -length
                };
                let moved = self.scroll.scroll_by(&scroll.id, distance);
                let mut effects = vec![Effect::StopFling];
                if moved {
                    effects.push(Effect::Redraw);
                }
                (moved, effects)
            }
            AccessibilityAction::ClearSelection if frame::plot_can_clear(node) => {
                (true, self.activate(index, None, false, Some(-1)))
            }
            AccessibilityAction::SelectItem(item)
                if frame::plot_enabled(node)
                    && node.plot.as_ref().is_some_and(|p| p.selectable(item)) =>
            {
                (true, self.activate(index, None, false, Some(item as i64)))
            }
            AccessibilityAction::Click if frame::interactive(node) && node.range.is_none() => {
                (true, self.activate(index, None, false, None))
            }
            _ => (false, Vec::new()),
        }
    }
}
