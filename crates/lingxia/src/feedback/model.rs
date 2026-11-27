//! One window's feedback: at most one toast, and dialogs shown one at a time in arrival order.
use super::words::Words;
use lxapp::feedback::{
    FeedbackAnswer as Answer, FeedbackDialog as DialogRequest, FeedbackPosition as Position,
    FeedbackToast as ToastRequest, FeedbackTone as Tone,
};
use serde_json::{Value, json};
use std::collections::VecDeque;
use tokio::sync::oneshot;

/// What the page sends back for the dialog it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(untagged)]
pub(crate) enum Choice {
    Index(usize),
    Named(Named),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Named {
    Confirm,
    Cancel,
}

struct Toast {
    key: u64,
    appid: String,
    request: ToastRequest,
}

struct Dialog {
    key: u64,
    appid: String,
    request: DialogRequest,
    words: Words,
    reply: oneshot::Sender<Answer>,
}

#[derive(Default)]
pub(crate) struct WindowFeedback {
    toast: Option<Toast>,
    dialogs: VecDeque<Dialog>,
    next_key: u64,
}

impl WindowFeedback {
    fn key(&mut self) -> u64 {
        self.next_key += 1;
        self.next_key
    }

    /// Replaces the toast showing; returns its key for the timer.
    pub fn toast(&mut self, appid: &str, request: ToastRequest) -> u64 {
        let key = self.key();
        self.toast = Some(Toast {
            key,
            appid: appid.to_string(),
            request,
        });
        key
    }

    /// Hides `appid`'s toast, the one with `key` when given; true when one went.
    pub fn hide_toast(&mut self, appid: Option<&str>, key: Option<u64>) -> bool {
        let matches = self.toast.as_ref().is_some_and(|toast| {
            appid.is_none_or(|appid| toast.appid == appid) && key.is_none_or(|key| toast.key == key)
        });
        if matches {
            self.toast = None;
        }
        matches
    }

    pub fn dialog(
        &mut self,
        appid: &str,
        request: DialogRequest,
        words: Words,
    ) -> oneshot::Receiver<Answer> {
        let (reply, receiver) = oneshot::channel();
        let key = self.key();
        self.dialogs.push_back(Dialog {
            key,
            appid: appid.to_string(),
            request,
            words,
            reply,
        });
        receiver
    }

    /// The page answered the dialog `key`; false when that dialog is no longer shown.
    pub fn answer(&mut self, key: u64, choice: Choice) -> bool {
        if self.dialogs.front().is_none_or(|dialog| dialog.key != key) {
            return false;
        }
        let dialog = self.dialogs.pop_front().expect("front checked");
        let answer = match (&dialog.request, choice) {
            (DialogRequest::Alert { .. }, _) => Answer::Confirmed,
            (DialogRequest::Sheet { items, .. }, Choice::Index(index)) if index < items.len() => {
                Answer::Chose(index)
            }
            (DialogRequest::Confirm { .. }, Choice::Named(Named::Confirm)) => Answer::Confirmed,
            _ => Answer::Cancelled,
        };
        let _ = dialog.reply.send(answer);
        true
    }

    /// Cancels every dialog of `appid`, shown or queued, and with `None` every dialog.
    pub fn cancel(&mut self, appid: Option<&str>) -> bool {
        let before = self.dialogs.len();
        let (gone, kept) = std::mem::take(&mut self.dialogs)
            .into_iter()
            .partition(|dialog| appid.is_none_or(|appid| dialog.appid == appid));
        self.dialogs = kept;
        for dialog in gone {
            let _ = dialog.reply.send(cancelled(&dialog.request));
        }
        self.dialogs.len() != before
    }

    /// `appid` went away: its toast and dialogs go with it.
    pub fn forget(&mut self, appid: &str) -> bool {
        let toast = self.hide_toast(Some(appid), None);
        self.cancel(Some(appid)) || toast
    }

    pub fn is_empty(&self) -> bool {
        self.toast.is_none() && self.dialogs.is_empty()
    }

    /// A dialog takes the window's input and focus; a toast never does.
    pub fn modal(&self) -> bool {
        !self.dialogs.is_empty()
    }

    /// The lxapp whose item is in front: the shown dialog's, else the toast's.
    pub fn front_appid(&self) -> Option<&str> {
        (self.dialogs.front().map(|dialog| dialog.appid.as_str()))
            .or(self.toast.as_ref().map(|toast| toast.appid.as_str()))
    }

    /// The page's data.
    pub fn data(&self) -> Value {
        json!({
            "toast": self.toast.as_ref().map(toast_data),
            "dialog": self.dialogs.front().map(dialog_data),
        })
    }
}

/// How a dialog closed by anything but its own buttons answers.
fn cancelled(request: &DialogRequest) -> Answer {
    match request {
        DialogRequest::Alert { .. } => Answer::Confirmed,
        _ => Answer::Cancelled,
    }
}

fn toast_data(toast: &Toast) -> Value {
    let request = &toast.request;
    json!({
        "key": toast.key,
        "title": request.title,
        "tone": match request.tone {
            Tone::Neutral => "neutral",
            Tone::Success => "success",
            Tone::Danger => "danger",
        },
        "loading": request.loading,
        "position": match request.position {
            Position::Top => "top",
            Position::Center => "center",
            Position::Bottom => "bottom",
        },
    })
}

fn dialog_data(dialog: &Dialog) -> Value {
    let words = &dialog.words;
    match &dialog.request {
        DialogRequest::Alert {
            title,
            message,
            confirm,
            destructive,
        } => json!({
            "key": dialog.key, "kind": "alert", "title": title, "message": message,
            "confirm": confirm.as_deref().unwrap_or(words.ok), "destructive": destructive,
        }),
        DialogRequest::Confirm {
            title,
            message,
            confirm,
            cancel,
            destructive,
        } => json!({
            "key": dialog.key, "kind": "confirm", "title": title, "message": message,
            "confirm": confirm.as_deref().unwrap_or(words.confirm),
            "cancel": cancel.as_deref().unwrap_or(words.cancel), "destructive": destructive,
        }),
        DialogRequest::Sheet { items, destructive } => json!({
            "key": dialog.key, "kind": "sheet", "items": items, "cancel": words.cancel,
            "label": words.actions, "destructive": destructive,
        }),
    }
}
