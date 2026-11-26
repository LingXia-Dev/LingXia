//! `lx.ui.toast` and its dialogs as the window feedback layer takes them. The host registers the
//! layer; Logic routes to it on the platforms [`LAYER`] marks.
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::oneshot;

/// Whether this platform's `lx.ui` feedback goes to the window feedback layer instead of the
/// page-drawn overlay (desktop) or the native presenters (phones).
pub const LAYER: bool = cfg!(target_os = "macos");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackTone {
    Neutral,
    Success,
    Danger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackPosition {
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeedbackToast {
    pub title: String,
    pub tone: FeedbackTone,
    pub loading: bool,
    pub position: FeedbackPosition,
    /// `None` stays until hidden or replaced.
    pub duration: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FeedbackDialog {
    /// One button; every way out answers [`FeedbackAnswer::Confirmed`].
    Alert {
        title: String,
        message: String,
        confirm: Option<String>,
        destructive: bool,
    },
    Confirm {
        title: String,
        message: String,
        confirm: Option<String>,
        cancel: Option<String>,
        destructive: bool,
    },
    Sheet {
        items: Vec<String>,
        destructive: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackAnswer {
    Confirmed,
    Cancelled,
    Chose(usize),
}

/// The window feedback layer. `appid` is the calling lxapp: the layer presents over its window.
pub trait FeedbackLayer: Send + Sync {
    /// Replaces the toast showing over `appid`'s window.
    fn toast(&self, appid: &str, toast: FeedbackToast) -> Result<(), String>;
    /// Hides `appid`'s toast.
    fn hide_toast(&self, appid: &str);
    /// Queues a dialog; its default words are in `language`, the lxapp's page language. The
    /// receiver settles when it is answered, dismissed or its window closes.
    fn dialog(
        &self,
        appid: &str,
        language: &str,
        dialog: FeedbackDialog,
    ) -> Result<oneshot::Receiver<FeedbackAnswer>, String>;
    /// Settles `appid`'s dialogs, shown or queued, as dismissed.
    fn dismiss(&self, appid: &str);
    /// `appid` closed: its toast and dialogs go.
    fn forget(&self, appid: &str);
}

static LAYER_IMPL: OnceLock<Box<dyn FeedbackLayer>> = OnceLock::new();

/// Register the process's feedback layer; later registrations are ignored.
pub fn register_feedback_layer(layer: Box<dyn FeedbackLayer>) {
    let _ = LAYER_IMPL.set(layer);
}

pub fn feedback_layer() -> Option<&'static dyn FeedbackLayer> {
    LAYER_IMPL.get().map(|layer| layer.as_ref())
}
