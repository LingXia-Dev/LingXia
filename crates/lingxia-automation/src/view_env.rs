//! `page.environment()` — a spec pins a GPU page's appearance and stills its motion;
//! `page.pinState()` holds one of its elements hovered.
//!
//! The override lives in the page, never in device or product settings, and
//! belongs to the run and attempt that set it: the attempt's end, and every
//! terminal transition of the run, clears it on the page.

use crate::gpu::{GpuPageAutomation, TestEnvironment};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};

/// What a lease holds on its page; each is leased and cleared on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Pin {
    Environment,
    Hover,
}

impl Pin {
    async fn clear(self, page: &Arc<dyn GpuPageAutomation>) {
        let _ = match self {
            Pin::Environment => page.environment(None).await,
            Pin::Hover => page.pin_state("", None, false).await,
        };
    }
}

struct Lease {
    run_id: String,
    /// The spec attempt that set it; `None` for the whole run.
    attempt: Option<u64>,
    page: Weak<dyn GpuPageAutomation>,
}

/// By page instance id and what is pinned.
type Leases = HashMap<(String, Pin), Lease>;

fn leases() -> &'static Mutex<Leases> {
    static LEASES: OnceLock<Mutex<Leases>> = OnceLock::new();
    LEASES.get_or_init(Default::default)
}

fn with_leases<T>(f: impl FnOnce(&mut Leases) -> T) -> T {
    let mut leases = leases().lock().unwrap_or_else(|err| err.into_inner());
    leases.retain(|_, lease| lease.page.strong_count() > 0);
    f(&mut leases)
}

/// Parse `{ appearance?: 'light' | 'dark' | null, still?: boolean, textScale?: number | null,
/// contrast?: 'standard' | 'high' | null, now?: number }`. The page checks the scale's range.
pub(crate) fn parse_pin(
    appearance: Option<&str>,
    still: Option<bool>,
    text_scale: Option<f64>,
    contrast: Option<&str>,
    now: Option<f64>,
) -> Result<TestEnvironment, String> {
    let now = match now {
        None => None,
        Some(ms) if ms.is_finite() && ms >= 0.0 => Some(ms as u64),
        Some(_) => return Err("E_INVALID_ARG: now is epoch milliseconds, 0 or more".into()),
    };
    let high_contrast = match contrast {
        None => None,
        Some("standard") => Some(false),
        Some("high") => Some(true),
        Some(other) => {
            return Err(format!(
                "E_INVALID_ARG: contrast must be 'standard', 'high' or null, not '{other}'"
            ));
        }
    };
    let dark = match appearance {
        None => None,
        Some("light") => Some(false),
        Some("dark") => Some(true),
        Some(other) => {
            return Err(format!(
                "E_INVALID_ARG: appearance must be 'light', 'dark' or null, not '{other}'"
            ));
        }
    };
    Ok(TestEnvironment {
        dark,
        still: still.unwrap_or(false),
        text_scale: text_scale.map(|scale| scale as f32),
        high_contrast,
        now,
    })
}

/// Lease `kind` on page `instance` to the run's attempt (`hold`) or release it, then apply.
async fn lease(
    run_id: &str,
    attempt: Option<u64>,
    instance: &str,
    page: Arc<dyn GpuPageAutomation>,
    kind: Pin,
    hold: bool,
    apply: crate::gpu::GpuFuture<Value>,
) -> Result<Value, String> {
    let key = (instance.to_owned(), kind);
    with_leases(|leases| match leases.get(&key) {
        Some(lease) if lease.run_id != run_id => Err(match kind {
            Pin::Environment => {
                "E_BUSY: this page's environment is pinned by another automation run"
            }
            Pin::Hover => "E_BUSY: this page's hover is pinned by another automation run",
        }
        .to_owned()),
        _ => {
            if hold {
                leases.insert(
                    key.clone(),
                    Lease {
                        run_id: run_id.to_owned(),
                        attempt,
                        page: Arc::downgrade(&page),
                    },
                );
            } else {
                leases.remove(&key);
            }
            Ok(())
        }
    })?;
    let applied = apply.await;
    if applied.is_err() && hold {
        with_leases(|leases| leases.remove(&key));
    }
    applied
}

/// Apply `pin` (or clear it, `None`) to page `instance` for the run's attempt.
pub(crate) async fn set(
    run_id: &str,
    attempt: Option<u64>,
    instance: &str,
    page: Arc<dyn GpuPageAutomation>,
    pin: Option<TestEnvironment>,
) -> Result<Value, String> {
    let apply = page.environment(pin);
    lease(
        run_id,
        attempt,
        instance,
        page,
        Pin::Environment,
        pin.is_some(),
        apply,
    )
    .await
}

/// Hold the `index`th match of `css` hovered on page `instance` for the run's attempt, or
/// lift the pin (`hover: false`).
pub(crate) async fn pin_hover(
    run_id: &str,
    attempt: Option<u64>,
    instance: &str,
    page: Arc<dyn GpuPageAutomation>,
    css: &str,
    index: Option<usize>,
    hover: bool,
) -> Result<Value, String> {
    let apply = page.pin_state(css, index, hover);
    lease(run_id, attempt, instance, page, Pin::Hover, hover, apply).await
}

/// The run a page call belongs to, and the attempt it may act for.
#[derive(Clone)]
pub(crate) struct ViewEnvRunScope {
    run_id: String,
    admit: Arc<dyn Fn() -> Result<Option<u64>, String> + Send + Sync>,
}

pub(crate) fn attach_run_scope(
    ctx: &rong::JSContext,
    run_id: String,
    admit: impl Fn() -> Result<Option<u64>, String> + Send + Sync + 'static,
) {
    ctx.set_state(ViewEnvRunScope {
        run_id,
        admit: Arc::new(admit),
    });
}

/// `set` for the run `ctx` belongs to.
pub(crate) async fn set_in_run(
    ctx: &rong::JSContext,
    instance: &str,
    page: Arc<dyn GpuPageAutomation>,
    pin: Option<TestEnvironment>,
) -> Result<Value, String> {
    let scope = ctx.get_state::<ViewEnvRunScope>().cloned().ok_or(
        "E_UNSUPPORTED: a page's environment is pinned only inside a host automation run (lxdev test)",
    )?;
    let attempt = (scope.admit)()?;
    set(&scope.run_id, attempt, instance, page, pin).await
}

/// `pin_hover` for the run `ctx` belongs to.
pub(crate) async fn pin_hover_in_run(
    ctx: &rong::JSContext,
    instance: &str,
    page: Arc<dyn GpuPageAutomation>,
    css: &str,
    index: Option<usize>,
    hover: bool,
) -> Result<Value, String> {
    let scope = ctx.get_state::<ViewEnvRunScope>().cloned().ok_or(
        "E_UNSUPPORTED: a page's hover is pinned only inside a host automation run (lxdev test)",
    )?;
    let attempt = (scope.admit)()?;
    pin_hover(&scope.run_id, attempt, instance, page, css, index, hover).await
}

/// The page's pin and the facts a visual baseline records. A read changes
/// nothing, so it never consults the leases: any run, or none, may read.
pub(crate) async fn read(page: Arc<dyn GpuPageAutomation>) -> Result<Value, String> {
    page.environment_facts().await
}

fn take(keep: impl Fn(&Lease) -> bool) -> Vec<(Pin, Arc<dyn GpuPageAutomation>)> {
    with_leases(|leases| {
        let mut taken = Vec::new();
        leases.retain(|(_, kind), lease| {
            if keep(lease) {
                return true;
            }
            taken.extend(lease.page.upgrade().map(|page| (*kind, page)));
            false
        });
        taken
    })
}

/// Clear the pins attempt `attempt` of `run_id` set; answers how many pages.
/// A page that fails to clear is already gone or disposing.
pub(crate) async fn reclaim_attempt(run_id: &str, attempt: u64) -> usize {
    let pages = take(|lease| !(lease.run_id == run_id && lease.attempt == Some(attempt)));
    for (kind, page) in &pages {
        kind.clear(page).await;
    }
    pages.len()
}

/// Clear every pin of a run. Called on every terminal transition of the run,
/// which may be on a thread without an async runtime.
pub(crate) fn clear_run(run_id: &str) {
    let pages = take(|lease| lease.run_id != run_id);
    if pages.is_empty() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("lingxia-test-view-env-clear".into())
        .spawn(move || {
            futures::executor::block_on(async {
                for (kind, page) in &pages {
                    kind.clear(page).await;
                }
            })
        });
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::gpu::{GpuAction, GpuFuture};
    use serde_json::json;
    use std::time::{Duration, Instant};

    /// Records the environments it was sent, once the request is polled.
    #[derive(Default)]
    pub(crate) struct FakePage {
        pub(crate) sent: Arc<Mutex<Vec<Option<TestEnvironment>>>>,
        pub(crate) hovers: Arc<Mutex<Vec<bool>>>,
    }

    impl GpuPageAutomation for FakePage {
        fn query(&self, _: &str, _: Option<usize>, _: bool, _: Option<usize>) -> GpuFuture<Value> {
            Box::pin(async { Ok(Value::Null) })
        }
        fn act(&self, _: &str, _: Option<usize>, _: GpuAction) -> GpuFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn screenshot(&self, _: bool) -> GpuFuture<Vec<u8>> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn environment_facts(&self) -> GpuFuture<Value> {
            let sent = self.sent.clone();
            Box::pin(async move {
                let pin = sent.lock().unwrap().last().copied().flatten();
                Ok(
                    json!({ "appearance": pin.and_then(|p| p.dark).map(|d| if d { "dark" } else { "light" }) }),
                )
            })
        }
        fn environment(&self, pin: Option<TestEnvironment>) -> GpuFuture<Value> {
            let sent = self.sent.clone();
            Box::pin(async move {
                sent.lock().unwrap().push(pin);
                Ok(Value::Null)
            })
        }
        fn pin_state(&self, _: &str, _: Option<usize>, hover: bool) -> GpuFuture<Value> {
            let hovers = self.hovers.clone();
            Box::pin(async move {
                hovers.lock().unwrap().push(hover);
                Ok(Value::Null)
            })
        }
    }

    pub(crate) fn sent(page: &FakePage) -> Vec<Option<TestEnvironment>> {
        page.sent.lock().unwrap().clone()
    }

    pub(crate) fn wait_cleared(page: &FakePage) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while sent(page).last() != Some(&None) {
            assert!(Instant::now() < deadline, "the page was not cleared");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub(crate) const DARK: TestEnvironment = TestEnvironment {
        dark: Some(true),
        still: true,
        text_scale: None,
        high_contrast: None,
        now: None,
    };

    pub(crate) fn pin(run_id: &str, attempt: Option<u64>, instance: &str) -> Arc<FakePage> {
        let page = Arc::new(FakePage::default());
        futures::executor::block_on(set(run_id, attempt, instance, page.clone(), Some(DARK)))
            .unwrap();
        page
    }

    #[test]
    fn pins_parse_strictly() {
        assert_eq!(
            parse_pin(Some("dark"), Some(true), None, None, None),
            Ok(DARK)
        );
        assert_eq!(
            parse_pin(None, None, Some(1.3), Some("high"), None),
            Ok(TestEnvironment {
                dark: None,
                still: false,
                text_scale: Some(1.3),
                high_contrast: Some(true),
                now: None,
            })
        );
        for bad in [
            parse_pin(Some("sepia"), None, None, None, None),
            parse_pin(None, None, None, Some("more"), None),
            parse_pin(None, None, None, None, Some(-1.0)),
        ] {
            assert!(bad.unwrap_err().starts_with("E_INVALID_ARG"));
        }
    }

    #[test]
    fn an_attempt_end_clears_only_that_attempts_pins() {
        let ended = pin("env-attempt", Some(1), "env-attempt-a");
        let next = pin("env-attempt", Some(2), "env-attempt-b");
        assert_eq!(
            futures::executor::block_on(reclaim_attempt("env-attempt", 1)),
            1
        );
        assert_eq!(sent(&ended), vec![Some(DARK), None]);
        assert_eq!(sent(&next), vec![Some(DARK)]);
        clear_run("env-attempt");
        wait_cleared(&next);
    }

    #[test]
    fn an_attempt_end_lifts_its_hover_pin_and_leaves_the_environment_to_its_own_lease() {
        let page = pin("hover-attempt", Some(1), "hover-attempt-a");
        futures::executor::block_on(pin_hover(
            "hover-attempt",
            Some(1),
            "hover-attempt-a",
            page.clone(),
            "#b",
            None,
            true,
        ))
        .unwrap();
        let busy = futures::executor::block_on(pin_hover(
            "hover-intruder",
            Some(1),
            "hover-attempt-a",
            page.clone(),
            "#b",
            None,
            false,
        ));
        assert!(busy.unwrap_err().starts_with("E_BUSY"));
        assert_eq!(
            futures::executor::block_on(reclaim_attempt("hover-attempt", 1)),
            2
        );
        assert_eq!(*page.hovers.lock().unwrap(), vec![true, false]);
        assert_eq!(sent(&page), vec![Some(DARK), None]);
    }

    #[test]
    fn another_run_cannot_take_a_pinned_page() {
        let page = pin("env-owner", Some(1), "env-owned");
        let other = futures::executor::block_on(set(
            "env-intruder",
            Some(1),
            "env-owned",
            page.clone(),
            None,
        ));
        assert!(other.unwrap_err().starts_with("E_BUSY"));
        // Reading is not taking: another run's read answers the owner's pin.
        let read = futures::executor::block_on(read(page.clone())).unwrap();
        assert_eq!(read["appearance"], "dark");
        assert_eq!(sent(&page), vec![Some(DARK)]);
        // Clearing its own pin by hand releases the page.
        futures::executor::block_on(set("env-owner", Some(1), "env-owned", page.clone(), None))
            .unwrap();
        assert_eq!(
            futures::executor::block_on(reclaim_attempt("env-owner", 1)),
            0
        );
        assert_eq!(sent(&page), vec![Some(DARK), None]);
    }
}
