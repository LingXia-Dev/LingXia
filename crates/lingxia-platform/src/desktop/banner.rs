//! Desktop banner queue: one visible card, same `id` replaces, others wait.
//!
//! The presenter is platform-owned. This module only decides what is showing
//! and who is waiting.

use crate::error::PlatformError;
use crate::traits::app_runtime::{DesktopBannerOutcome, DesktopBannerShow};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const DEFAULT_TOAST_MS: u64 = 5_000;
const MAX_ID_CHARS: usize = 64;

type Reply = Sender<Result<DesktopBannerOutcome, PlatformError>>;

struct Waiter {
    request: DesktopBannerShow,
    tx: Reply,
    generation: u64,
}

struct State {
    visible: Option<Waiter>,
    queue: VecDeque<Waiter>,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();
static GENERATION: AtomicU64 = AtomicU64::new(1);

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| {
        Mutex::new(State {
            visible: None,
            queue: VecDeque::new(),
        })
    })
}

fn lock() -> std::sync::MutexGuard<'static, State> {
    state().lock().unwrap_or_else(|error| error.into_inner())
}

/// Block until this banner is answered, dismissed, timed out, or replaced.
pub fn show(request: DesktopBannerShow) -> Result<DesktopBannerOutcome, PlatformError> {
    if request.id.is_empty() || request.id.chars().count() > MAX_ID_CHARS {
        return Err(PlatformError::InvalidParameter(format!(
            "banner id must be 1–{MAX_ID_CHARS} characters"
        )));
    }
    if request.title.is_empty() {
        return Err(PlatformError::InvalidParameter(
            "banner title must not be empty".into(),
        ));
    }
    if request.actions.len() > 2 {
        return Err(PlatformError::InvalidParameter(
            "banner accepts at most two actions".into(),
        ));
    }
    for action in &request.actions {
        if action.id.is_empty() || action.label.is_empty() {
            return Err(PlatformError::InvalidParameter(
                "banner action id and label must not be empty".into(),
            ));
        }
    }

    let (tx, rx) = mpsc::channel();
    enqueue(request, tx);
    rx.recv()
        .map_err(|_| PlatformError::Platform("banner waiter dropped".into()))?
}

/// Finish a visible or queued banner as dismissed. Unknown ids are fine.
pub fn dismiss(id: &str) {
    complete(id, DesktopBannerOutcome::Dismissed { id: id.to_string() });
}

/// Host FFI reports a button or dismiss. Unknown ids are fine.
pub fn on_desktop_banner_outcome(id: &str, outcome: DesktopBannerOutcome) {
    complete(id, outcome);
}

/// Resolve the waiter for `id` if it is still current.
pub(crate) fn complete(id: &str, outcome: DesktopBannerOutcome) {
    finish(id, Ok(outcome));
}

/// Unblock the waiter when the presenter could not show the card.
pub(crate) fn fail(id: &str, message: impl Into<String>) {
    finish(id, Err(PlatformError::Platform(message.into())));
}

fn finish(id: &str, result: Result<DesktopBannerOutcome, PlatformError>) {
    let next = {
        let mut state = lock();
        if state
            .visible
            .as_ref()
            .is_some_and(|waiter| waiter.request.id == id)
        {
            let waiter = state.visible.take().expect("visible checked");
            hide_ui();
            let _ = waiter.tx.send(result);
            take_next(&mut state)
        } else if let Some(index) = state
            .queue
            .iter()
            .position(|waiter| waiter.request.id == id)
        {
            let waiter = state.queue.remove(index).expect("index from position");
            let _ = waiter.tx.send(result);
            None
        } else {
            None
        }
    };
    if let Some(next) = next {
        present_waiter(next);
    }
}

fn enqueue(request: DesktopBannerShow, tx: Reply) {
    let next = {
        let mut state = lock();
        let incoming = Waiter {
            request,
            tx,
            generation: GENERATION.fetch_add(1, Ordering::SeqCst),
        };
        let id = incoming.request.id.clone();

        if let Some(index) = state
            .queue
            .iter()
            .position(|waiter| waiter.request.id == id)
        {
            let previous = state.queue.remove(index).expect("index from position");
            let _ = previous
                .tx
                .send(Ok(DesktopBannerOutcome::Replaced { id: id.clone() }));
        }

        if state
            .visible
            .as_ref()
            .is_some_and(|waiter| waiter.request.id == id)
        {
            let previous = state.visible.take().expect("visible checked");
            hide_ui();
            let _ = previous
                .tx
                .send(Ok(DesktopBannerOutcome::Replaced { id: id.clone() }));
            Some(incoming)
        } else if state.visible.is_some() {
            state.queue.push_back(incoming);
            None
        } else {
            Some(incoming)
        }
    };
    if let Some(next) = next {
        present_waiter(next);
    }
}

fn take_next(state: &mut State) -> Option<Waiter> {
    state.queue.pop_front()
}

fn present_waiter(waiter: Waiter) {
    let request = waiter.request.clone();
    let generation = waiter.generation;
    {
        let mut state = lock();
        state.visible = Some(waiter);
    }
    if !present_ui(&request) {
        fail(&request.id, "failed to present banner");
        return;
    }
    if let Some(timeout_ms) = effective_timeout(&request) {
        std::thread::Builder::new()
            .name("lingxia-desktop-banner-timeout".into())
            .spawn(move || {
                std::thread::sleep(Duration::from_millis(timeout_ms));
                if visible_generation_is(generation) {
                    complete(
                        &request.id,
                        DesktopBannerOutcome::TimedOut {
                            id: request.id.clone(),
                        },
                    );
                }
            })
            .ok();
    }
}

fn effective_timeout(request: &DesktopBannerShow) -> Option<u64> {
    match request.timeout_ms {
        Some(0) => None,
        Some(ms) => Some(ms),
        None if request.actions.is_empty() => Some(DEFAULT_TOAST_MS),
        None => None,
    }
}

fn visible_generation_is(generation: u64) -> bool {
    lock()
        .visible
        .as_ref()
        .is_some_and(|waiter| waiter.generation == generation)
}

fn present_ui(request: &DesktopBannerShow) -> bool {
    #[cfg(all(not(test), target_os = "windows"))]
    {
        crate::windows::banner::present(request)
    }
    #[cfg(all(not(test), target_os = "macos"))]
    {
        crate::apple::banner::present(request)
    }
    #[cfg(any(test, not(any(target_os = "windows", target_os = "macos"))))]
    {
        let _ = request;
        true
    }
}

fn hide_ui() {
    #[cfg(all(not(test), target_os = "windows"))]
    crate::windows::banner::hide();
    #[cfg(all(not(test), target_os = "macos"))]
    crate::apple::banner::hide();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::app_runtime::{
        DesktopBannerAction, DesktopBannerActionStyle, DesktopBannerBackground,
    };

    fn toast(id: &str, title: &str) -> DesktopBannerShow {
        DesktopBannerShow {
            id: id.into(),
            title: title.into(),
            body: String::new(),
            actions: Vec::new(),
            timeout_ms: Some(0),
            background: Default::default(),
        }
    }

    fn prompt(id: &str) -> DesktopBannerShow {
        DesktopBannerShow {
            id: id.into(),
            title: "Allow?".into(),
            body: String::new(),
            actions: vec![
                DesktopBannerAction {
                    id: "deny".into(),
                    label: "Deny".into(),
                    style: DesktopBannerActionStyle::Default,
                },
                DesktopBannerAction {
                    id: "allow".into(),
                    label: "Allow".into(),
                    style: DesktopBannerActionStyle::Primary,
                },
            ],
            timeout_ms: Some(0),
            background: Default::default(),
        }
    }

    #[test]
    fn empty_title_is_rejected() {
        let error = show(toast("a", "")).unwrap_err();
        assert!(error.to_string().contains("title"));
    }

    #[test]
    fn empty_id_is_rejected() {
        let mut request = toast("a", "Hi");
        request.id.clear();
        let error = show(request).unwrap_err();
        assert!(error.to_string().contains("id"));
    }

    #[test]
    fn more_than_two_actions_is_rejected() {
        let mut request = prompt("a");
        request.actions.push(DesktopBannerAction {
            id: "later".into(),
            label: "Later".into(),
            style: DesktopBannerActionStyle::Default,
        });
        let error = show(request).unwrap_err();
        assert!(error.to_string().contains("at most two"));
    }

    #[test]
    fn dismiss_unknown_id_is_fine() {
        dismiss("missing");
    }

    #[test]
    fn present_failure_unblocks_the_waiter() {
        let handle = std::thread::spawn(|| show(toast("fail-me", "Hi")));
        // present_ui succeeds in unit tests; the presenter thread reports the
        // window/panel failure through fail().
        std::thread::sleep(Duration::from_millis(20));
        fail("fail-me", "no hwnd");
        let error = handle.join().expect("banner thread").unwrap_err();
        assert!(error.to_string().contains("no hwnd"));
    }

    #[test]
    fn background_parses_tokens_and_hex() {
        assert_eq!(
            DesktopBannerBackground::parse("").unwrap(),
            DesktopBannerBackground::System
        );
        assert_eq!(
            DesktopBannerBackground::parse("light").unwrap(),
            DesktopBannerBackground::Light
        );
        assert_eq!(
            DesktopBannerBackground::parse("#fff").unwrap(),
            DesktopBannerBackground::Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255
            }
        );
        assert_eq!(
            DesktopBannerBackground::parse("#1c1c1e").unwrap(),
            DesktopBannerBackground::Color {
                r: 0x1c,
                g: 0x1c,
                b: 0x1e,
                a: 255
            }
        );
        assert!(DesktopBannerBackground::parse("blurple").is_err());
    }
}
