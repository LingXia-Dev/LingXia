//! A test run's view of the dialogs an lxapp's Logic opens.
//!
//! `lx.showToast` reports each toast it presented; `lx.showModal` (and
//! `alert` / `confirm`) and `lx.showActionSheet` ask before presenting. With
//! no hook registered, or a hook that is not watching the app, every dialog
//! is presented as usual: only a test run that watches the app answers them.

use std::sync::OnceLock;

/// A toast the host accepted for presentation.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastShown {
    pub title: String,
    /// `success` / `error` / `loading` / `none`, as the app passed it.
    pub icon: String,
    pub duration_ms: f64,
}

/// A modal about to be presented.
#[derive(Debug, Clone, PartialEq)]
pub struct ModalShown {
    pub title: String,
    pub content: String,
    /// Only the texts the app set: the defaults are localized.
    pub confirm_text: Option<String>,
    /// `None` without a cancel button, or when the app left the default.
    pub cancel_text: Option<String>,
    pub show_cancel: bool,
}

/// An `lx.showActionSheet` about to be presented.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionSheetShown {
    /// Item labels, in order.
    pub items: Vec<String>,
}

/// What happens to a dialog that asks.
#[derive(Debug, Clone, PartialEq)]
pub enum DialogDecision<T> {
    /// Nobody watches: present it.
    Present,
    /// Answered without presenting it.
    Answer(T),
    /// Watched but not answered: the call rejects with this message.
    Refuse(String),
}

pub trait DialogHook: Send + Sync {
    fn toast(&self, appid: &str, toast: &ToastShown);
    /// `Answer(true)` confirms.
    fn modal(&self, appid: &str, modal: &ModalShown) -> DialogDecision<bool>;
    /// `Answer(Some(index))` picks an item, `Answer(None)` cancels.
    fn action_sheet(&self, appid: &str, sheet: &ActionSheetShown) -> DialogDecision<Option<usize>>;
}

static HOOK: OnceLock<Box<dyn DialogHook>> = OnceLock::new();

/// Register the process's dialog hook; later registrations are ignored.
pub fn register_dialog_hook(hook: Box<dyn DialogHook>) {
    let _ = HOOK.set(hook);
}

pub fn dialog_hook() -> Option<&'static dyn DialogHook> {
    HOOK.get().map(|hook| hook.as_ref())
}
