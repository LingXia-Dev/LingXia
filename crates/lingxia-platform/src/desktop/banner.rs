//! Desktop banner queue: one visible card, same `id` replaces, others wait.
//!
//! The presenter is platform-owned. This module only decides what is showing
//! and who is waiting.

use crate::error::PlatformError;
use crate::traits::app_runtime::{
    DesktopBannerOutcome, DesktopBannerPending, DesktopBannerShow, DesktopBannerSource,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const DEFAULT_TOAST_MS: u64 = 5_000;
const MAX_ID_CHARS: usize = 64;
/// Banners one guest lxapp may have visible or waiting, so it cannot bury
/// the product's own behind a queue of its own.
pub const MAX_PER_SOURCE: usize = 3;
/// Longest a guest banner stays up, actions or not, so it cannot hold the
/// one slot while the product's wait.
pub const GUEST_MAX_MS: u64 = 60_000;

type Reply = Sender<Result<DesktopBannerOutcome, PlatformError>>;

struct Waiter {
    request: DesktopBannerShow,
    source: Option<DesktopBannerSource>,
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
    enqueue_show(request)?.wait()
}

/// Queue this banner (or put it on screen) and hand back its waiter.
pub fn enqueue_show(request: DesktopBannerShow) -> Result<DesktopBannerPending, PlatformError> {
    enqueue_show_from(request, None)
}

/// [`enqueue_show`] for a banner that names the guest lxapp it came from.
pub fn enqueue_show_from(
    request: DesktopBannerShow,
    source: Option<DesktopBannerSource>,
) -> Result<DesktopBannerPending, PlatformError> {
    validate(&request)?;
    let (tx, rx) = mpsc::channel();
    enqueue(request, source, tx)?;
    Ok(DesktopBannerPending::new(rx))
}

fn validate(request: &DesktopBannerShow) -> Result<(), PlatformError> {
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
    Ok(())
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
            state.queue.pop_front().map(|next| occupy(&mut state, next))
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

fn enqueue(
    request: DesktopBannerShow,
    source: Option<DesktopBannerSource>,
    tx: Reply,
) -> Result<(), PlatformError> {
    let next = {
        let mut state = lock();
        if let Some(source) = &source {
            let held = state
                .visible
                .iter()
                .chain(state.queue.iter())
                .filter(|waiter| {
                    waiter.request.id != request.id
                        && waiter
                            .source
                            .as_ref()
                            .is_some_and(|held| held.app_id == source.app_id)
                })
                .count();
            if held >= MAX_PER_SOURCE {
                return Err(PlatformError::Platform(format!(
                    "this lxapp already has {MAX_PER_SOURCE} banners showing or waiting; dismiss one first"
                )));
            }
        }
        let incoming = Waiter {
            request,
            source,
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
            Some(occupy(&mut state, incoming))
        } else if state.visible.is_some() {
            wait_in_line(&mut state.queue, incoming);
            None
        } else {
            Some(occupy(&mut state, incoming))
        }
    };
    if let Some(next) = next {
        present_waiter(next);
    }
    Ok(())
}

/// The product's banners wait ahead of every guest's.
fn wait_in_line(queue: &mut VecDeque<Waiter>, waiter: Waiter) {
    let index = if waiter.source.is_none() {
        queue
            .iter()
            .position(|queued| queued.source.is_some())
            .unwrap_or(queue.len())
    } else {
        queue.len()
    };
    queue.insert(index, waiter);
}

/// What to draw for the waiter that now holds the slot.
struct Shown {
    request: DesktopBannerShow,
    source: Option<DesktopBannerSource>,
    generation: u64,
}

/// Take the slot under the same lock that found it free, so two callers can
/// never both present and overwrite each other's waiter.
fn occupy(state: &mut State, waiter: Waiter) -> Shown {
    let shown = Shown {
        request: waiter.request.clone(),
        source: waiter.source.clone(),
        generation: waiter.generation,
    };
    state.visible = Some(waiter);
    shown
}

fn present_waiter(shown: Shown) {
    let Shown {
        request,
        source,
        generation,
    } = shown;
    if !visible_generation_is(generation) {
        return;
    }
    if !present_ui(&request, source.as_ref()) {
        fail(&request.id, "failed to present banner");
        return;
    }
    if let Some(timeout_ms) = effective_timeout(&request, source.is_some()) {
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

fn effective_timeout(request: &DesktopBannerShow, guest: bool) -> Option<u64> {
    let requested = match request.timeout_ms {
        Some(0) => None,
        Some(ms) => Some(ms),
        None if request.actions.is_empty() => Some(DEFAULT_TOAST_MS),
        None => None,
    };
    if guest {
        Some(requested.map_or(GUEST_MAX_MS, |ms| ms.min(GUEST_MAX_MS)))
    } else {
        requested
    }
}

fn visible_generation_is(generation: u64) -> bool {
    lock()
        .visible
        .as_ref()
        .is_some_and(|waiter| waiter.generation == generation)
}

fn present_ui(request: &DesktopBannerShow, source: Option<&DesktopBannerSource>) -> bool {
    #[cfg(all(not(test), target_os = "windows"))]
    {
        crate::windows::banner::present(request, source)
    }
    #[cfg(all(not(test), target_os = "macos"))]
    {
        crate::apple::banner::present(request, source)
    }
    #[cfg(any(test, not(any(target_os = "windows", target_os = "macos"))))]
    {
        let _ = (request, source);
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
    fn a_guest_is_capped_alone_and_may_replace_its_own() {
        let source = |app_id: &str| {
            Some(DesktopBannerSource {
                app_id: app_id.into(),
                name: "Guest".into(),
                icon_path: None,
            })
        };
        let mut pending = Vec::new();
        for index in 0..MAX_PER_SOURCE {
            pending.push(
                enqueue_show_from(prompt(&format!("cap-g{index}")), source("cap.guest")).unwrap(),
            );
        }
        let error = enqueue_show_from(prompt("cap-over"), source("cap.guest")).unwrap_err();
        assert!(error.to_string().contains("already has"));
        assert!(enqueue_show_from(prompt("cap-g0"), source("cap.guest")).is_ok());
        assert!(enqueue_show_from(prompt("cap-other"), source("cap.other")).is_ok());
        assert!(enqueue_show(prompt("cap-product")).is_ok());
        for id in ["cap-g0", "cap-g1", "cap-g2", "cap-other", "cap-product"] {
            dismiss(id);
        }
        drop(pending);
    }

    #[test]
    fn a_guest_banner_never_outstays_the_cap() {
        let mut request = prompt("t");
        request.timeout_ms = None;
        assert_eq!(effective_timeout(&request, false), None);
        assert_eq!(effective_timeout(&request, true), Some(GUEST_MAX_MS));
        request.timeout_ms = Some(0);
        assert_eq!(effective_timeout(&request, true), Some(GUEST_MAX_MS));
        request.timeout_ms = Some(GUEST_MAX_MS * 10);
        assert_eq!(effective_timeout(&request, true), Some(GUEST_MAX_MS));
        request.timeout_ms = Some(2_000);
        assert_eq!(effective_timeout(&request, true), Some(2_000));
    }

    #[test]
    fn the_product_waits_ahead_of_guests() {
        let waiter = |id: &str, guest: bool| Waiter {
            request: toast(id, "Hi"),
            source: guest.then(|| DesktopBannerSource {
                app_id: "g".into(),
                name: "Guest".into(),
                icon_path: None,
            }),
            tx: mpsc::channel().0,
            generation: 0,
        };
        let mut queue = VecDeque::new();
        wait_in_line(&mut queue, waiter("p1", false));
        wait_in_line(&mut queue, waiter("g1", true));
        wait_in_line(&mut queue, waiter("g2", true));
        wait_in_line(&mut queue, waiter("p2", false));
        let order: Vec<_> = queue.iter().map(|w| w.request.id.as_str()).collect();
        assert_eq!(order, ["p1", "p2", "g1", "g2"]);
    }

    #[test]
    fn dismiss_unknown_id_is_fine() {
        dismiss("missing");
    }

    #[test]
    fn present_failure_unblocks_the_waiter() {
        // present_ui succeeds in unit tests; the presenter thread reports the
        // window/panel failure through fail().
        let pending = enqueue_show(toast("fail-me", "Hi")).unwrap();
        fail("fail-me", "no hwnd");
        let error = pending.wait().unwrap_err();
        assert!(error.to_string().contains("no hwnd"), "{error}");
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
