use super::*;

#[derive(Default)]
struct Shell(Mutex<Vec<String>>);

impl Shell {
    fn calls(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

impl NativePage for Shell {
    fn connect(&self, generation: GpuViewGeneration) -> Result<(), LxAppError> {
        self.0
            .lock()
            .unwrap()
            .push(format!("connect {}", generation.get()));
        Ok(())
    }
    fn deliver(
        &self,
        generation: GpuViewGeneration,
        message: GpuViewMessage,
    ) -> Result<(), LxAppError> {
        self.0
            .lock()
            .unwrap()
            .push(format!("deliver {} {message:?}", generation.get()));
        Ok(())
    }
    fn write_state(
        &self,
        generation: GpuViewGeneration,
        write: GpuStateWrite,
    ) -> Result<(), LxAppError> {
        self.0
            .lock()
            .unwrap()
            .push(format!("state {} {write:?}", generation.get()));
        Ok(())
    }
    fn request_leave(&self, generation: GpuViewGeneration, reason: &str) -> Result<(), LxAppError> {
        self.0
            .lock()
            .unwrap()
            .push(format!("leave {} {reason}", generation.get()));
        Ok(())
    }
    fn update_environment(&self, update: &serde_json::Value) -> Result<(), LxAppError> {
        self.0
            .lock()
            .unwrap()
            .push(format!("environment {}", update["kind"]));
        Ok(())
    }
    fn reload(&self, _: &str) -> Result<(), LxAppError> {
        Ok(())
    }
    fn swap_module(&self, source: &str) -> Result<(), LxAppError> {
        self.0.lock().unwrap().push(format!("swap {source}"));
        Ok(())
    }
    fn park(&self) -> Result<(), LxAppError> {
        self.0.lock().unwrap().push("park".into());
        Ok(())
    }
    fn retain_surface(&self, retained: bool) -> Result<(), LxAppError> {
        self.0.lock().unwrap().push(format!("retain {retained}"));
        Ok(())
    }
    fn automation(&self, request: serde_json::Value, reply: AutomationReply) {
        self.0
            .lock()
            .unwrap()
            .push(format!("automation {}", request["op"]));
        reply(Ok(request));
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Default)]
struct Factory(Mutex<Vec<Token>>);

impl NativeFactory for Factory {
    fn create(&self, _: Token, _: &PageInstance, _: Creation) -> Result<(), LxAppError> {
        unreachable!("tests register endpoints directly")
    }
    fn dispose(&self, token: Token) {
        self.0.lock().unwrap().push(token);
    }
}

fn endpoint(factory: &Arc<Factory>, page: &str) -> (Arc<Endpoint>, Arc<Shell>, Token) {
    let token = NEXT.fetch_add(1, Ordering::Relaxed);
    let shell = Arc::new(Shell::default());
    let lease = Lease::new(token, page.into(), "test".into(), None, factory.clone());
    (register(lease, shell.clone()), shell, token)
}

fn generation(value: u64) -> GpuViewGeneration {
    GpuViewGeneration::new_for_test(value)
}

#[test]
fn messages_and_state_reach_the_shell_only_for_the_current_connection() {
    let factory = Arc::new(Factory::default());
    let (endpoint, shell, _) = endpoint(&factory, "gate");
    let close = |id: &str| GpuViewMessage::ChannelClose {
        id: id.into(),
        code: None,
        reason: None,
    };
    endpoint.deliver(generation(1), close("early")).unwrap();
    endpoint.connect(generation(2)).unwrap();
    endpoint.deliver(generation(1), close("stale")).unwrap();
    endpoint.deliver(generation(2), close("current")).unwrap();
    let snapshot = |rev| GpuStateWrite::Snapshot {
        rev,
        state: serde_json::json!({}),
    };
    endpoint.write_state(generation(1), snapshot(1)).unwrap();
    endpoint.write_state(generation(2), snapshot(2)).unwrap();
    assert!(endpoint.connect(generation(1)).is_err(), "superseded");
    endpoint.request_leave(LeaveReason::Back).unwrap();
    endpoint.park().unwrap();
    endpoint.request_leave(LeaveReason::Home).unwrap();
    endpoint.deliver(generation(2), close("parked")).unwrap();
    assert_eq!(
        shell.calls(),
        [
            "connect 2",
            "deliver 2 ChannelClose { id: \"current\", code: None, reason: None }",
            "state 2 Snapshot { rev: 2, state: Object {} }",
            "leave 2 back",
            "park"
        ]
    );
}

#[test]
fn a_lease_disposes_once_and_leaves_no_index_behind() {
    let factory = Arc::new(Factory::default());
    let (endpoint, shell, token) = endpoint(&factory, "dispose-once");
    assert!(live(token).is_some());
    endpoint.connect(generation(3)).unwrap();
    endpoint.dispose();
    endpoint.dispose();
    endpoint
        .deliver(
            generation(3),
            GpuViewMessage::ChannelClose {
                id: "late".into(),
                code: None,
                reason: None,
            },
        )
        .unwrap();
    endpoint
        .update_environment(PageViewEnvironment::DisplayLanguage {
            language: "en".into(),
            content: "en".into(),
            revision: 1,
        })
        .unwrap();
    assert!(live(token).is_none());
    assert!(!registry().pages.contains_key("dispose-once"));
    drop(endpoint);
    assert_eq!(*factory.0.lock().unwrap(), [token]);
    assert_eq!(shell.calls(), ["connect 3"]);
}

#[test]
fn a_replaced_shell_keeps_the_page_index_when_the_old_one_retires() {
    let factory = Arc::new(Factory::default());
    let (old, _, _) = endpoint(&factory, "replaced");
    let (_new, _, token) = endpoint(&factory, "replaced");
    old.dispose();
    assert_eq!(registry().pages.get("replaced"), Some(&token));
}

#[test]
fn creation_outcomes_reach_only_their_pending_request() {
    let token = NEXT.fetch_add(1, Ordering::Relaxed);
    let (tx, mut rx) = oneshot::channel();
    registry().pending.insert(token, tx);
    created(token + 1_000_000, Err("other".into()));
    assert!(rx.try_recv().is_err());
    created(token, Err("failed".into()));
    assert_eq!(rx.try_recv().unwrap().err().as_deref(), Some("failed"));
    assert_eq!(worker_key("app", 7), "app:7");
}

#[test]
fn shell_replies_decode_to_values_or_verbatim_errors() {
    assert_eq!(
        decode_reply(r#"{"value":{"count":2}}"#).unwrap(),
        serde_json::json!({"count": 2})
    );
    assert_eq!(
        decode_reply(r#"{"error":"Element not found: [data-testid=\"x\"] (index 0)"}"#)
            .unwrap_err(),
        r#"Element not found: [data-testid="x"] (index 0)"#
    );
}

#[cfg(feature = "automation")]
#[test]
fn automation_requests_reach_the_shell_until_the_page_goes() {
    use lingxia_automation::gpu::{GpuAction, GpuPageAutomation};
    let shell = Arc::new(Shell::default());
    let native: Arc<dyn NativePage> = shell.clone();
    let automation = automation::GpuAutomation {
        native: Arc::downgrade(&native),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let query = runtime
        .block_on(automation.query("[data-testid=\"save\"]", Some(1), false, None))
        .unwrap();
    assert_eq!(query["css"], "[data-testid=\"save\"]");
    assert_eq!(query["index"], 1);
    runtime
        .block_on(automation.act(
            "button",
            None,
            GpuAction::Fill {
                text: "a".into(),
                force: true,
            },
        ))
        .unwrap();
    assert_eq!(
        shell.calls(),
        ["automation \"query\"", "automation \"act\""]
    );
    drop((native, shell));
    let gone = runtime.block_on(automation.screenshot(false)).unwrap_err();
    assert!(gone.starts_with("page is not active:"), "{gone}");
}

/// Acts with call 1, then answers its result polls as `results` says.
#[cfg(feature = "automation")]
struct Acting {
    results: fn(AutomationReply),
}

#[cfg(feature = "automation")]
impl NativePage for Acting {
    fn connect(&self, _: GpuViewGeneration) -> Result<(), LxAppError> {
        Ok(())
    }
    fn deliver(&self, _: GpuViewGeneration, _: GpuViewMessage) -> Result<(), LxAppError> {
        Ok(())
    }
    fn write_state(&self, _: GpuViewGeneration, _: GpuStateWrite) -> Result<(), LxAppError> {
        Ok(())
    }
    fn request_leave(&self, _: GpuViewGeneration, _: &str) -> Result<(), LxAppError> {
        Ok(())
    }
    fn update_environment(&self, _: &serde_json::Value) -> Result<(), LxAppError> {
        Ok(())
    }
    fn reload(&self, _: &str) -> Result<(), LxAppError> {
        Ok(())
    }
    fn park(&self) -> Result<(), LxAppError> {
        Ok(())
    }
    fn retain_surface(&self, _: bool) -> Result<(), LxAppError> {
        Ok(())
    }
    fn automation(&self, request: serde_json::Value, reply: AutomationReply) {
        if request["op"] == "act" {
            reply(Ok(serde_json::json!({"call": 1})));
        } else {
            (self.results)(reply);
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(feature = "automation")]
fn act_on(results: fn(AutomationReply)) -> Result<(), String> {
    use lingxia_automation::gpu::{GpuAction, GpuPageAutomation};
    let native: Arc<dyn NativePage> = Arc::new(Acting { results });
    let automation = automation::GpuAutomation {
        native: Arc::downgrade(&native),
    };
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(automation.act("button", None, GpuAction::Click { force: false }))
}

#[cfg(feature = "automation")]
#[test]
fn an_act_whose_page_goes_counts_done() {
    act_on(|reply| reply(Err("page is not active".into()))).unwrap();
    act_on(|reply| reply(Ok(serde_json::json!({"done": true})))).unwrap();
}

#[cfg(feature = "automation")]
#[test]
fn an_unanswered_act_reports_its_timeout() {
    let error = act_on(std::mem::forget).unwrap_err();
    assert!(error.starts_with("E_TIMEOUT: the act on button"), "{error}");
}

#[cfg(feature = "automation")]
#[test]
fn an_act_the_page_drops_still_fails() {
    let error = act_on(|reply| reply(Err("E_ACT_DROPPED: the press was not delivered".into())))
        .unwrap_err();
    assert!(error.starts_with("E_ACT_DROPPED"), "{error}");
    assert_eq!(
        automation::act_progress(Ok(Ok(serde_json::json!({"done": false})))),
        automation::ActProgress::Pending
    );
}

#[test]
fn a_hot_swap_reaches_only_the_selected_live_pages() {
    let factory = Arc::new(Factory::default());
    let (_a, first, _) = endpoint(&factory, "swap-a");
    let (_b, second, _) = endpoint(&factory, "swap-b");
    let swapped = swap_pages(|id| (id == "swap-a").then(|| "next".to_owned())).unwrap();
    assert_eq!(swapped, 1);
    assert_eq!(first.calls(), ["swap next"]);
    assert!(second.calls().is_empty());
}

#[test]
fn a_swap_that_reloaded_is_reported_and_a_silent_one_counts_as_landed() {
    use super::swap_reports::{Swaps, report};
    let swaps = Swaps::new();
    swaps.expect("module-a");
    swaps.expect("module-a");
    report("module-a", Ok(()));
    report("module-a", Err("UI_ANIMATION: refused".into()));
    // Nobody waits for another module.
    report("module-b", Err("unrelated".into()));
    let error = swaps
        .outcome(2, std::time::Duration::from_secs(1))
        .unwrap_err()
        .to_string();
    assert!(error.contains("UI_ANIMATION: refused"), "{error}");
    assert!(!error.contains("unrelated"), "{error}");

    let swaps = Swaps::new();
    swaps.expect("module-c");
    assert!(
        swaps
            .outcome(1, std::time::Duration::from_millis(20))
            .is_ok()
    );
    // The registration left with its call.
    let swaps = Swaps::new();
    report("module-c", Err("late".into()));
    swaps.expect("module-c");
    report("module-c", Ok(()));
    assert!(swaps.outcome(1, std::time::Duration::from_secs(1)).is_ok());
}

#[test]
fn hot_swap_matches_routes_with_or_without_module_extension() {
    for path in [
        "pages/view-lab/index",
        "/pages/view-lab/index.js",
        "pages/view-lab/index.js?tab=1",
    ] {
        assert_eq!(super::page_key(path), "pages/view-lab/index");
    }
    assert_eq!(super::page_key("pages/v1.2/index"), "pages/v1.2/index");
}

#[test]
fn an_app_pack_outlives_the_pages_a_relaunch_replaces() {
    let factory: Arc<dyn NativeFactory> = Arc::new(Factory::default());
    let worker = "pack-holders:1";
    let pack = || bundled_pack(r##"{"name":"brand","version":1}"##);
    let lease = |token| {
        Lease::new(
            token,
            format!("page-{token}"),
            worker.into(),
            pack(),
            factory.clone(),
        )
    };
    let old = lease(NEXT.fetch_add(1, Ordering::Relaxed));
    // A relaunch may create the new page before the old one is disposed.
    let new = lease(NEXT.fetch_add(1, Ordering::Relaxed));
    old.dispose();
    assert!(
        lingxia_theme::app_pack(worker).is_some(),
        "the new page keeps the pack"
    );
    drop(new);
    assert!(
        lingxia_theme::app_pack(worker).is_none(),
        "the pack ends with the session's last page"
    );
}

#[test]
fn a_bundled_pack_parses_and_a_malformed_one_is_skipped() {
    assert_eq!(
        bundled_pack(r##"{"name":"brand","version":1}"##)
            .unwrap()
            .name,
        "brand"
    );
    assert!(bundled_pack("{").is_none());
    assert!(bundled_pack(r#"{"name":"x","version":1,"extra":true}"#).is_none());
}

#[test]
fn a_stale_frame_keeps_the_platform_reporting_until_the_current_view_presents() {
    let factory = Arc::new(Factory::default());
    let (endpoint, _, token) = endpoint(&factory, "ready-rearm");
    endpoint.connect(generation(2)).unwrap();
    assert!(
        !presented(token, 1),
        "a frame of the old connection is not the first frame"
    );
    assert!(
        presented(token, 2),
        "the current connection's frame ends the reports"
    );
    assert!(
        presented(NEXT.load(Ordering::Relaxed) + 1000, 2),
        "an unknown page needs no more"
    );
}

#[path = "leak_tests.rs"]
mod leak;

/// A screenshot of `results`' page, on a clock that only virtual sleeps advance.
#[cfg(feature = "automation")]
fn shoot(results: fn(AutomationReply)) -> Result<Vec<u8>, String> {
    use lingxia_automation::gpu::GpuPageAutomation;
    let native: Arc<dyn NativePage> = Arc::new(Acting { results });
    let automation = automation::GpuAutomation {
        native: Arc::downgrade(&native),
    };
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(automation.screenshot(false))
}

#[cfg(feature = "automation")]
#[test]
fn a_screenshot_waits_for_a_revealed_pages_first_frame_and_gives_up_with_the_same_error() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static ASKED: AtomicUsize = AtomicUsize::new(0);
    // Trimmed while hidden: nothing to read for the first three asks, then its new frame.
    let png = shoot(|reply| {
        reply(if ASKED.fetch_add(1, Ordering::SeqCst) < 3 {
            Err("E_PAGE_NOT_READY: no presented frame".into())
        } else {
            Ok(serde_json::json!({"png": "AQID"}))
        })
    })
    .unwrap();
    assert_eq!(png, [1, 2, 3]);
    assert_eq!(ASKED.load(Ordering::SeqCst), 4);
    // A page that never presents: the same error, after the wait and not before.
    let started = std::time::Instant::now();
    let error =
        shoot(|reply| reply(Err("E_PAGE_NOT_READY: no presented frame".into()))).unwrap_err();
    assert_eq!(error, "E_PAGE_NOT_READY: no presented frame");
    assert!(started.elapsed() >= std::time::Duration::from_millis(900));
    // Any other error is not waited on.
    let started = std::time::Instant::now();
    let error = shoot(|reply| reply(Err("page is not active".into()))).unwrap_err();
    assert_eq!(error, "page is not active");
    assert!(started.elapsed() < std::time::Duration::from_millis(500));
}
