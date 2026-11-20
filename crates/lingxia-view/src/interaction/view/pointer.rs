use super::*;

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

impl ViewInteraction {
    /// A gesture the View does not consume returns no effects and `false`.
    pub fn pointer(&mut self, event: Pointer, page: PageTicket) -> (bool, Vec<Effect>) {
        let mut effects = self.scrub_before(event);
        let (consumed, rest) = self.pointer_event(event, page);
        effects.extend(rest);
        let observed = self.scrub_after(event);
        if observed {
            effects.push(Effect::Dispatch);
        }
        effects.extend(self.sync_slider_press());
        (consumed || observed, effects)
    }

    fn pointer_event(&mut self, event: Pointer, page: PageTicket) -> (bool, Vec<Effect>) {
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
                self.origin = super::FocusOrigin::Pointer;
                self.slider.cancel();
                self.pending_focus = None;
                effects.extend(self.cancel_press());
                effects.push(Effect::StopFling);
                effects.push(Effect::ClearFocusRing);
                self.keyboard_focus = None;
                let pressed = frame.press_hit(point, self.touch_target);
                // A drag element reached through its touch target takes the drag from there.
                let reached_drag = pressed
                    .filter(|index| !frame::contains(&frame.nodes[*index], point, false))
                    .map(|index| &frame.nodes[index])
                    .filter(|node| node.drag.as_ref().is_some_and(|drag| !drag.ghost))
                    .map(|node| node.identity.to_string());
                self.gesture = Gesture {
                    pressed,
                    pressed_commit: Some(frame.commit),
                    down: [event.x, event.y],
                    last: event.y,
                    target: frame.scroll_hit(point),
                    drag: reached_drag.or_else(|| self.drag.hit(&frame, point)),
                    touching: true,
                    // A mouse asks with its secondary button instead.
                    context: (!self.fine_pointer)
                        .then(|| frame.context_menu_hit(point))
                        .flatten(),
                    ..Default::default()
                };
                let pressed = self.gesture.pressed;
                self.slider
                    .begin(pressed.and_then(|index| frame.node(index)), point);
                if pressed.is_none()
                    && self.gesture.target.is_none()
                    && self.gesture.drag.is_none()
                    && frame.outside_plot_selections(point).is_empty()
                    && frame.selectable_hit(point).is_none()
                    && !frame.has_text_selection()
                    && self.gesture.context.is_none()
                {
                    return (false, effects);
                }
                if self.gesture.context.is_some() {
                    effects.push(Effect::ArmLongPress);
                }
                if self.gesture.target.is_some() {
                    effects.push(Effect::Claim { exclusive: true });
                }
                if let Some(index) = pressed {
                    // A field shows no press unless its page styles one (`active:`).
                    if frame.node(index).is_some_and(|node| {
                        node.pressed_style
                            || !matches!(node.tag, "input" | "textarea" | "search" | "select")
                    }) {
                        effects.push(press(&frame, index));
                    }
                    if frame.node(index).is_some_and(frame::editable) {
                        effects.push(Effect::ArmLongPress);
                    } else if frame.node(index).is_some_and(|node| node.tag == "link")
                        && let Some(text) = frame.selectable_hit(point)
                    {
                        // In selectable text a long-press selects instead of pressing the link;
                        // a mouse drags a selection instead.
                        self.gesture.selectable = Some(text);
                        if !self.fine_pointer {
                            effects.push(Effect::ArmLongPress);
                        }
                    }
                } else if let Some(index) = frame.selectable_hit(point) {
                    self.gesture.selectable = Some(index);
                    if !self.fine_pointer {
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
                    // A dragged slider stays pressed, so `active:thumb:` holds until release.
                    effects.push(Effect::CancelLongPress);
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
                // A mouse drag that starts in selectable text selects it rather than scrolling.
                if self.fine_pointer
                    && self.gesture.moved
                    && !self.gesture.dragging
                    && let Some(index) = self.gesture.selectable
                    && frame.nodes.get(index).is_some_and(|node| {
                        node.selectable.is_some()
                            && (self.gesture.pressed_commit == Some(frame.commit)
                                || node.text_selection.is_some())
                    })
                {
                    // Selection repaints present new frames; the text stays where it was.
                    let commit = frame.commit;
                    if !self.gesture.selecting {
                        self.gesture.selecting = true;
                        effects.extend(self.cancel_press());
                    }
                    self.dispatches.push(Dispatch::DragSelectText {
                        commit,
                        identity: frame.nodes[index].identity.to_string(),
                        from: self.logical(self.gesture.down[0], self.gesture.down[1]),
                        to: point,
                    });
                    effects.push(Effect::Dispatch);
                    return (true, effects);
                }
                // A drag element on the finger's axis takes the gesture.
                if !self.gesture.dragging
                    && self.gesture.moved
                    && let Some(identity) = self.gesture.drag.clone()
                {
                    let axis = if dx.abs() > dy.abs() {
                        ScrollAxis::Horizontal
                    } else {
                        ScrollAxis::Vertical
                    };
                    let prefix = format!("{identity}/");
                    let inner = self
                        .scroll
                        .target_for_axis(self.gesture.target.as_deref(), axis)
                        .filter(|region| {
                            frame.nodes.iter().any(|node| {
                                node.scroll.as_ref().is_some_and(|s| &s.id == region)
                                    && node.identity.starts_with(&prefix)
                            })
                        });
                    // An `overscroll-contain` scroll inside keeps the gesture to itself.
                    let contained = inner
                        .as_deref()
                        .is_some_and(|region| self.scroll.contains_region(region));
                    if self.drag.axis(&identity) == Some(axis) && !contained {
                        let outer = frame
                            .index_of(&identity)
                            .and_then(|index| frame.nodes[index].scroll_parent.clone())
                            .and_then(|parent| self.scroll.target_for_axis(Some(&parent), axis));
                        self.gesture.inner = inner;
                        self.gesture.outer = outer;
                        self.gesture.axis = Some(axis);
                        self.gesture.target = None;
                        self.gesture.dragging = true;
                        self.gesture.holding = true;
                        self.drag.hold(&identity);
                        effects.extend(self.cancel_press());
                        effects.push(Effect::Claim { exclusive: true });
                        let (start, distance) = match axis {
                            ScrollAxis::Horizontal => (self.gesture.down[0], dx),
                            ScrollAxis::Vertical => (self.gesture.down[1], dy),
                        };
                        self.gesture.last = start + event.slop.copysign(distance);
                        self.drag_event("start", 0, 0.0);
                        effects.push(Effect::Dispatch);
                    }
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
                        self.gesture.scroll_from = self.scroll.offset(&target);
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
                if self.gesture.holding
                    && let Some(identity) = self.gesture.drag.clone()
                {
                    let axis = self.gesture.axis.unwrap_or(ScrollAxis::Vertical);
                    let position = match axis {
                        ScrollAxis::Horizontal => event.x,
                        ScrollAxis::Vertical => event.y,
                    };
                    let finger = (position - self.gesture.last) / scale;
                    self.gesture.last = position;
                    self.chain(&identity, finger);
                    effects.push(Effect::Redraw);
                    return (true, effects);
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
                    if self.scroll.drag_by(&target, delta / scale) {
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
                self.gesture.touching = false;
                // A finger that caught a spring and let go without dragging lets it finish.
                if !self.gesture.holding
                    && let Some(identity) = self.gesture.drag.clone()
                {
                    self.drag.resume(&identity);
                }
                if self.gesture.holding
                    && let Some(identity) = self.gesture.drag.clone()
                {
                    let scale = frame.scale;
                    let velocity = match self.gesture.axis {
                        Some(ScrollAxis::Horizontal) => event.velocity[0],
                        _ => event.velocity[1],
                    } / scale;
                    // A flick while a scroll moved last is that scroll's; the element rests.
                    let flung = self
                        .gesture
                        .mover
                        .clone()
                        .filter(|_| velocity.abs() >= event.min_fling / scale);
                    if let Some(region) = flung.clone() {
                        effects.push(Effect::Fling {
                            region,
                            horizontal: self.gesture.axis == Some(ScrollAxis::Horizontal),
                            velocity: -velocity * scale,
                        });
                    }
                    let (velocity, fling) = if flung.is_some() {
                        (0.0, f32::INFINITY)
                    } else {
                        (velocity, event.min_fling / scale)
                    };
                    if let Some(release) = self.drag.release(&identity, velocity, fling) {
                        self.drag_event("end", release.index, velocity);
                        effects.push(Effect::Dispatch);
                    }
                    self.gesture.holding = false;
                    self.gesture.dragging = false;
                    effects.push(Effect::Claim { exclusive: false });
                    return (true, effects);
                }
                if self.gesture.dragging {
                    let axis = self.gesture.axis.unwrap_or(ScrollAxis::Vertical);
                    let speed = -match axis {
                        ScrollAxis::Horizontal => event.velocity[0],
                        ScrollAxis::Vertical => event.velocity[1],
                    };
                    let region = self.gesture.target.clone();
                    if let Some(region) = region.as_deref().filter(|r| self.scroll.snap_stop(r)) {
                        // `snap-always` releases into the spring toward the next point, no fling.
                        let fling = speed.abs() >= event.min_fling;
                        let sign = if axis == ScrollAxis::Horizontal && self.scroll.rtl(region) {
                            -1.0
                        } else {
                            1.0
                        };
                        let direction = if fling { speed.signum() * sign } else { 0.0 };
                        self.scroll
                            .snap_fling(region, self.gesture.scroll_from, direction);
                    } else if speed.abs() >= event.min_fling
                        && let Some(region) = region
                    {
                        effects.push(Effect::Fling {
                            region,
                            horizontal: axis == ScrollAxis::Horizontal,
                            velocity: speed,
                        });
                    }
                } else if let Some(index) = target.filter(|index| {
                    same_frame && frame.press_hit(point, self.touch_target) == Some(*index)
                }) {
                    effects.push(Effect::Clicked);
                    effects.extend(self.activate(index, Some(point), false, None));
                } else if !self.gesture.moved
                    && !self.gesture.long_pressed
                    && frame.has_text_selection()
                {
                    // Any tap that is not a long-press ends the static-text selection.
                    self.dispatches.push(Dispatch::ClearTextSelection);
                    effects.push(Effect::Dispatch);
                } else if target.is_none()
                    && same_frame
                    && !self.gesture.moved
                    && !self.gesture.long_pressed
                    && frame.press_hit(point, self.touch_target).is_none()
                {
                    effects.push(Effect::Clicked);
                    effects.extend(self.dismiss_plots(frame.outside_plot_selections(point)));
                }
                self.gesture.dragging = false;
                effects.push(Effect::Claim { exclusive: false });
            }
            PointerPhase::Cancel => {
                self.gesture.touching = false;
                if let Some(identity) = self.gesture.drag.clone() {
                    if self.gesture.holding {
                        self.gesture.holding = false;
                        self.gesture.dragging = false;
                        if let Some(release) = self.drag.release(&identity, 0.0, f32::INFINITY) {
                            self.drag_event("end", release.index, 0.0);
                            effects.push(Effect::Dispatch);
                        }
                    } else {
                        self.drag.resume(&identity);
                    }
                }
                self.gesture.moved = true;
                self.slider.cancel();
                effects.extend(self.cancel_press());
                effects.push(Effect::StopFling);
                effects.push(Effect::Claim { exclusive: false });
            }
        }
        (true, effects)
    }

    /// Queues `onContextMenu` for node `index`; false when it has no handler.
    pub(super) fn context_menu_dispatch(
        &mut self,
        frame: &crate::interaction::frame::PresentedFrame,
        index: usize,
        point: [f32; 2],
        pointer: &str,
    ) -> bool {
        let Some(node) = frame.nodes.get(index) else {
            return false;
        };
        let Some(handle) = node.on_context_menu else {
            return false;
        };
        self.dispatches.push(Dispatch::ContextMenu {
            commit: frame.commit,
            handle,
            identity: node.identity.to_string(),
            payload: serde_json::json!({ "x": point[0], "y": point[1], "pointer": pointer })
                .to_string(),
        });
        true
    }

    /// A secondary click at a logical point: the innermost `onContextMenu` element there.
    pub fn context_menu(&mut self, point: [f32; 2]) -> Option<Vec<Effect>> {
        if self.inert {
            return None;
        }
        let frame = self.frame.clone()?;
        let index = frame.context_menu_hit(point)?;
        self.context_menu_dispatch(&frame, index, point, "mouse")
            .then(|| vec![Effect::Dispatch])
    }

    /// The long-press timer fired for the pressed editor.
    pub fn long_press(&mut self) -> Vec<Effect> {
        let Some(frame) = self.frame.clone() else {
            return Vec::new();
        };
        // A held finger asks for the element's menu: the press does not click, and what the
        // finger does next neither scrolls nor drags.
        if let Some(index) = self.gesture.context
            && self.gesture.touching
            && !self.gesture.moved
            && !self.gesture.dragging
            && self.gesture.pressed_commit == Some(frame.commit)
        {
            self.gesture.long_pressed = true;
            let mut effects = self.cancel_press();
            let point = self.logical(self.gesture.down[0], self.gesture.down[1]);
            if self.context_menu_dispatch(&frame, index, point, "touch") {
                effects.extend([Effect::Haptic, Effect::Dispatch]);
            }
            return effects;
        }
        let link = self
            .gesture
            .pressed
            .is_some_and(|pressed| frame.nodes[pressed].tag == "link");
        if let (true, Some(index), Some(commit)) = (
            self.gesture.pressed.is_none() || link,
            self.gesture.selectable,
            self.gesture.pressed_commit,
        ) && commit == frame.commit
        {
            self.gesture.long_pressed = true;
            let mut effects = if link {
                self.cancel_press()
            } else {
                Vec::new()
            };
            let point = self.logical(self.gesture.down[0], self.gesture.down[1]);
            self.dispatches.push(Dispatch::SelectText {
                commit,
                identity: frame.nodes[index].identity.to_string(),
                point,
            });
            effects.extend([Effect::Haptic, Effect::Dispatch]);
            return effects;
        }
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

    /// One finger step through the chain: toward expansion (a lower offset) the element moves
    /// first and its inner scroll takes the rest; toward collapse the inner scroll goes first.
    /// Distance past the points passes to an enclosing scroll, else the element stretches.
    fn chain(&mut self, identity: &str, finger: f32) {
        let mut rest = finger;
        let inner = self.gesture.inner.clone();
        let scroll = |model: &mut ScrollModel, region: &Option<String>, rest: &mut f32| {
            let Some(region) = region else { return None };
            let took = -model.drag_region(region, -*rest);
            *rest -= took;
            (took != 0.0).then(|| region.clone())
        };
        let mut mover = None;
        if rest > 0.0 {
            mover = scroll(&mut self.scroll, &inner, &mut rest);
        }
        let left = self.drag.move_within(identity, rest);
        if left != rest {
            mover = None;
        }
        rest = left;
        if rest < 0.0
            && let Some(region) = scroll(&mut self.scroll, &inner, &mut rest)
        {
            mover = Some(region);
        }
        if rest.abs() > 0.001 {
            let outer = self.gesture.outer.clone();
            if let Some(region) = scroll(&mut self.scroll, &outer, &mut rest) {
                mover = Some(region);
            }
        }
        if rest.abs() > 0.001 {
            self.drag.drag_by(identity, rest);
            mover = None;
        }
        if finger != 0.0 {
            self.gesture.mover = mover;
        }
    }

    /// Whether a vertical gesture can still scroll content, for host pull-to-refresh arbitration.
    pub fn can_scroll_vertically(&self, direction: i32) -> bool {
        self.gesture.long_pressed
            || self.gesture.holding
            || self.slider.dragging()
            || self.scroll.can_scroll(
                self.gesture.target.as_deref(),
                direction,
                ScrollAxis::Vertical,
            )
    }

    // ---- activation ----------------------------------------------------
}
