use super::*;

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
    /// Show the platform's single-choice picker for this `<select>` (options and choice on the node).
    OpenPicker {
        node: usize,
    },
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

pub(super) fn press(frame: &PresentedFrame, node: usize) -> Effect {
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
    /// Long-press on selectable text: the session selects there (`select_text`).
    SelectText {
        commit: CommitTicket,
        identity: String,
        point: [f32; 2],
    },
    /// A mouse drag over selectable text: the session selects `from..to` (`drag_select_text`).
    DragSelectText {
        commit: CommitTicket,
        identity: String,
        from: [f32; 2],
        to: [f32; 2],
    },
    /// A `<select>` chose option `index` without its picker (arrow keys, automation).
    Select {
        commit: CommitTicket,
        identity: String,
        index: usize,
        /// The chosen option's value, to find it again in a View rendered since.
        value: String,
    },
    /// Escape: the View dismisses its innermost overlay.
    Escape,
    /// Enter on an editor: the session submits it as the input method's action would.
    SubmitEditor {
        commit: CommitTicket,
        identity: String,
    },
    /// A tap outside the selection: the session drops it (`clear_text_selection`).
    ClearTextSelection,
    /// Edges reached by a presented frame.
    Reach {
        commit: CommitTicket,
        handles: Vec<u64>,
        more: bool,
    },
}
