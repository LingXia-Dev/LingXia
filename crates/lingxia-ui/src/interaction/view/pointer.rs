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
                self.keyboard_used = false;
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
                    && frame.selectable_hit(point).is_none()
                    && !frame.has_text_selection()
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
                    } else if frame.node(index).is_some_and(|node| node.tag == "link")
                        && let Some(text) = frame.selectable_hit(point)
                    {
                        // In selectable text a long-press selects instead of pressing the link.
                        self.gesture.selectable = Some(text);
                        effects.push(Effect::ArmLongPress);
                    }
                } else if let Some(index) = frame.selectable_hit(point) {
                    self.gesture.selectable = Some(index);
                    effects.push(Effect::ArmLongPress);
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
                identity: frame.nodes[index].identity.clone(),
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
}
