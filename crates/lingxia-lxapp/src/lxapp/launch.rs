//! The launch destination: `App.onLaunch`, then a cold link's `App.onOpen`, may name the first
//! page. A cold open waits for that decision, at most [`LAUNCH_DECISION_BUDGET`], before the
//! host shows a page.

use super::LxApp;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long `onLaunch` plus a cold link's `onOpen` may hold the launch face.
pub(crate) const LAUNCH_DECISION_BUDGET: Duration = Duration::from_millis(1500);
/// Logic stops offering a first page at the deadline; the host waits this much longer for it.
const HANDOFF_GRACE: Duration = Duration::from_millis(100);

/// The decision Logic reports when `onLaunch` settles: a target's JSON, or none.
#[derive(Default)]
pub(crate) struct LaunchGate {
    decision: Mutex<Option<Option<String>>>,
    decided: Condvar,
    /// The app defines `onOpen`, so a cold link opens the start page unless a hook picks one.
    links_through_on_open: AtomicBool,
}

impl LaunchGate {
    pub(crate) fn reset(&self) {
        *self.lock() = None;
    }

    pub(crate) fn set_links_through_on_open(&self, owned: bool) {
        self.links_through_on_open.store(owned, Ordering::Relaxed);
    }

    pub(crate) fn record(&self, target: Option<String>) {
        *self.lock() = Some(target);
        self.decided.notify_all();
    }

    /// The decision, or `None` when it did not come by `deadline`.
    fn wait_until(&self, deadline: Instant) -> Option<Option<String>> {
        let mut decision = self.lock();
        while decision.is_none() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            decision = self
                .decided
                .wait_timeout(decision, left)
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
        decision.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Option<String>>> {
        self.decision
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// When Logic must have decided, as `Date.now()` reads it, and when the host gives up.
pub(crate) fn launch_deadline() -> (u64, Instant) {
    let epoch_ms = SystemTime::now()
        .checked_add(LAUNCH_DECISION_BUDGET)
        .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
        .map(|at| at.as_millis() as u64)
        .unwrap_or_default();
    (
        epoch_ms,
        Instant::now() + LAUNCH_DECISION_BUDGET + HANDOFF_GRACE,
    )
}

impl LxApp {
    /// The first page Logic chose, as a page path and query; `None` keeps the page asked for.
    /// `cold_link` says the open carries an App Link, which `onOpen` owns when the app has one.
    pub(crate) fn await_launch_destination(
        &self,
        give_up: Instant,
        cold_link: bool,
    ) -> Option<(String, String)> {
        let start_page = || Some((self.initial_route(), String::new()));
        let owned_link = cold_link
            && self
                .launch_gate
                .links_through_on_open
                .load(Ordering::Relaxed);
        let Some(decision) = self.launch_gate.wait_until(give_up) else {
            crate::warn!(
                "App.onLaunch and the launch link's App.onOpen did not decide within {} ms; opening the start page",
                LAUNCH_DECISION_BUDGET.as_millis()
            )
            .with_appid(self.appid.clone());
            return if owned_link { start_page() } else { None };
        };
        let target = decision?;
        match first_page(&target, |name| self.find_page_path_by_name(name)) {
            Ok(Destination::Page(path, query)) => Some((path, query)),
            Ok(Destination::Start) => start_page(),
            Err(reason) => {
                crate::warn!("launch destination ignored: {reason}").with_appid(self.appid.clone());
                if owned_link { start_page() } else { None }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum Destination {
    Page(String, String),
    Start,
}

/// A `{ page | tab, params? }` target as a page path and query; `"start"` is the start page.
fn first_page(
    target: &str,
    page_path: impl Fn(&str) -> Option<String>,
) -> Result<Destination, String> {
    let value: serde_json::Value =
        serde_json::from_str(target).map_err(|error| format!("{target}: {error}"))?;
    if value == "start" {
        return Ok(Destination::Start);
    }
    let Some(name) = value
        .get("page")
        .or_else(|| value.get("tab"))
        .and_then(|name| name.as_str())
    else {
        return Err(format!("{target} names no page or tab"));
    };
    let path = page_path(name).ok_or_else(|| format!("no page named '{name}'"))?;
    let query = match value.get("params") {
        Some(params) if !params.is_null() => {
            crate::startup::append_page_query(String::new(), params)?
                .trim_start_matches('?')
                .to_string()
        }
        _ => String::new(),
    };
    Ok(Destination::Page(path, query))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn pages(name: &str) -> Option<String> {
        (name == "onboarding" || name == "home").then(|| format!("pages/{name}/index"))
    }

    #[test]
    fn a_page_or_tab_target_names_the_first_page() {
        assert_eq!(
            first_page(
                r#"{"page":"onboarding","params":{"step":2,"from":"link"}}"#,
                pages
            ),
            Ok(Destination::Page(
                "pages/onboarding/index".to_string(),
                "from=link&__lxparams=%7B%22step%22%3A2%7D".to_string()
            ))
        );
        assert_eq!(
            first_page(r#"{"tab":"home"}"#, pages),
            Ok(Destination::Page(
                "pages/home/index".to_string(),
                String::new()
            ))
        );
        assert_eq!(first_page(r#""start""#, pages), Ok(Destination::Start));
        assert!(first_page(r#"{"page":"missing"}"#, pages).is_err());
        assert!(first_page(r#"{"url":"https://example.com"}"#, pages).is_err());
    }

    #[test]
    fn the_gate_waits_for_the_decision_or_the_deadline() {
        let gate = Arc::new(LaunchGate::default());
        assert_eq!(
            gate.wait_until(Instant::now() + Duration::from_millis(20)),
            None
        );
        let recorder = Arc::clone(&gate);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            recorder.record(Some(r#"{"page":"home"}"#.to_string()));
        });
        assert_eq!(
            gate.wait_until(Instant::now() + Duration::from_secs(5)),
            Some(Some(r#"{"page":"home"}"#.to_string()))
        );
        gate.reset();
        gate.record(None);
        assert_eq!(gate.wait_until(Instant::now()), Some(None));
    }
}
