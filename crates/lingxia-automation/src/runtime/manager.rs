//! One isolated Rong worker per [`AutomationRuntime`] instance.

use super::context;
use super::protocol::*;
use super::run::RunShared;
use log::{error, warn};
use rong::{JSContextService as _, JSRuntime, JSValue, Rong, RongJS, Source};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak, mpsc};
use std::time::{Duration, Instant};

const TEARDOWN_GRACE: Duration = Duration::from_secs(10);
const COMPLETED_RETENTION: Duration = Duration::from_secs(300);
const COMPLETED_RETAINED: usize = 2;

struct RunRequest {
    shared: Arc<RunShared>,
    source: String,
    source_name: String,
    args: HashMap<String, String>,
}

struct RuntimeState {
    active: Option<Arc<RunShared>>,
    completed: Vec<Arc<RunShared>>,
    unhealthy: bool,
    interrupt: Option<rong::InterruptHandle>,
}

struct RuntimeInner {
    state: Mutex<RuntimeState>,
    sender: mpsc::Sender<RunRequest>,
}

/// Reusable host-owned automation executor.
///
/// One instance serializes its programs on one worker. Create separate
/// instances only when the host deliberately wants independent concurrency.
#[derive(Clone)]
pub struct AutomationRuntime {
    inner: Arc<RuntimeInner>,
}

impl AutomationRuntime {
    pub fn new() -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel::<RunRequest>();
        let inner = Arc::new(RuntimeInner {
            state: Mutex::new(RuntimeState {
                active: None,
                completed: Vec::new(),
                unhealthy: false,
                interrupt: None,
            }),
            sender,
        });
        let runtime = Arc::downgrade(&inner);
        std::thread::Builder::new()
            .name("lingxia-automation-runtime".to_string())
            .spawn(move || executor_main(runtime, receiver))
            .map_err(|err| format!("failed to spawn automation runtime: {err}"))?;
        Ok(Self { inner })
    }

    pub fn start(&self, args: AutomationStartArgs) -> Result<AutomationStartResponse, String> {
        if args.source.is_empty() {
            return Err("automation source must not be empty".to_string());
        }
        if args.source.len() > MAX_SOURCE_BYTES {
            return Err(format!(
                "automation source is {} bytes; the limit is {MAX_SOURCE_BYTES}",
                args.source.len()
            ));
        }
        let timeout_ms = args.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&timeout_ms) {
            return Err(format!(
                "timeout_ms {timeout_ms} out of range ({MIN_TIMEOUT_MS}-{MAX_TIMEOUT_MS})"
            ));
        }

        let mut state = self.inner.state.lock().unwrap();
        if state.unhealthy {
            return Err(
                "automation_runtime_unhealthy: a previous run pinned the worker; restart the host"
                    .to_string(),
            );
        }
        retire_completed(&mut state);
        if let Some(active) = &state.active {
            return Err(format!(
                "automation_run_in_progress: run {} is active",
                active.run_id
            ));
        }

        let shared = Arc::new(RunShared::new(
            uuid::Uuid::new_v4().to_string(),
            Duration::from_millis(timeout_ms),
        ));
        let request = RunRequest {
            shared: shared.clone(),
            source: args.source,
            source_name: args
                .source_name
                .unwrap_or_else(|| "lingxia-automation".to_string()),
            args: args.args,
        };
        self.inner
            .sender
            .send(request)
            .map_err(|_| "automation runtime executor is unavailable".to_string())?;
        state.active = Some(shared.clone());
        Ok(AutomationStartResponse {
            run_id: shared.run_id.clone(),
            state: AutomationRunState::Running,
        })
    }

    pub fn poll(&self, args: AutomationPollArgs) -> Result<AutomationPollResponse, String> {
        let run = self.find_run(&args.run_id)?;
        Ok(run.poll(args.after_seq))
    }

    pub fn cancel(&self, args: AutomationCancelArgs) -> Result<AutomationCancelResponse, String> {
        let run = self.find_run(&args.run_id)?;
        if !run.state().is_terminal() {
            run.request_cancel();
            let state = self.inner.state.lock().unwrap();
            if let Some(active) = &state.active
                && active.run_id == run.run_id
                && let Some(interrupt) = &state.interrupt
            {
                run.request_preemption();
                interrupt.interrupt();
            }
        }
        Ok(AutomationCancelResponse {
            run_id: run.run_id.clone(),
            state: run.state(),
        })
    }

    fn find_run(&self, run_id: &str) -> Result<Arc<RunShared>, String> {
        let mut state = self.inner.state.lock().unwrap();
        retire_completed(&mut state);
        if let Some(active) = &state.active
            && active.run_id == run_id
        {
            return Ok(active.clone());
        }
        state
            .completed
            .iter()
            .find(|run| run.run_id == run_id)
            .cloned()
            .ok_or_else(|| format!("unknown automation run: {run_id}"))
    }
}

fn retire_completed(state: &mut RuntimeState) {
    if let Some(active) = &state.active
        && active.state().is_terminal()
    {
        let run = state.active.take().unwrap();
        state.completed.push(run);
    }
    let now = Instant::now();
    state.completed.retain(|run| {
        run.completed_at()
            .is_none_or(|at| now.duration_since(at) < COMPLETED_RETENTION)
    });
    while state.completed.len() > COMPLETED_RETAINED {
        state.completed.remove(0);
    }
}

fn set_unhealthy(runtime: &RuntimeInner) {
    runtime.state.lock().unwrap().unhealthy = true;
}

fn note_worker_recovered(runtime: &RuntimeInner) {
    let mut state = runtime.state.lock().unwrap();
    if let Some(interrupt) = &state.interrupt {
        interrupt.clear();
    }
    if state.unhealthy {
        warn!("automation worker recovered after being declared wedged");
        state.unhealthy = false;
    }
}

fn executor_main(runtime: Weak<RuntimeInner>, receiver: mpsc::Receiver<RunRequest>) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            error!("failed to build automation runtime executor: {err}");
            while let Ok(request) = receiver.recv() {
                request.shared.finalize(
                    AutomationRunState::InternalError,
                    Some(internal_error("automation runtime executor unavailable")),
                    None,
                );
            }
            return;
        }
    };

    let mut rong: Option<Rong<RongJS>> = None;
    while let Ok(request) = receiver.recv() {
        let Some(runtime) = runtime.upgrade() else {
            request.shared.finalize(
                AutomationRunState::Cancelled,
                Some(internal_error("automation runtime was dropped")),
                None,
            );
            continue;
        };
        if runtime.state.lock().unwrap().unhealthy {
            request.shared.finalize(
                AutomationRunState::InternalError,
                Some(internal_error(
                    "automation runtime is unhealthy; restart the host",
                )),
                None,
            );
            continue;
        }
        rt.block_on(execute_run(runtime, &mut rong, request));
    }
}

async fn execute_run(
    runtime: Arc<RuntimeInner>,
    rong: &mut Option<Rong<RongJS>>,
    request: RunRequest,
) {
    let shared = request.shared.clone();
    let pool = match rong {
        Some(pool) => pool,
        None => match Rong::<RongJS>::builder().shared().workers(1).build() {
            Ok(pool) => rong.insert(pool),
            Err(err) => {
                shared.finalize(
                    AutomationRunState::InternalError,
                    Some(internal_error(format!(
                        "failed to build automation worker: {err:?}"
                    ))),
                    None,
                );
                return;
            }
        },
    };
    let worker = match pool.worker(0) {
        Ok(worker) => worker,
        Err(err) => {
            shared.finalize(
                AutomationRunState::InternalError,
                Some(internal_error(format!(
                    "automation worker unavailable: {err}"
                ))),
                None,
            );
            return;
        }
    };

    {
        let mut state = runtime.state.lock().unwrap();
        if state.interrupt.is_none() {
            state.interrupt = Some(worker.interrupt_handle());
        }
    }

    let worker_runtime = runtime.clone();
    let handle = match worker
        .spawn(async move |js_runtime, _receiver| -> rong::JSResult<()> {
            run_on_worker(worker_runtime, js_runtime, request).await;
            Ok(())
        })
        .await
    {
        Ok(handle) => handle,
        Err(err) => {
            shared.finalize(
                AutomationRunState::InternalError,
                Some(internal_error(format!(
                    "failed to start automation task: {err}"
                ))),
                None,
            );
            return;
        }
    };

    let watchdog = shared.remaining() + TEARDOWN_GRACE;
    let join = handle.join();
    tokio::pin!(join);
    if tokio::time::timeout(watchdog, &mut join).await.is_ok() {
        return;
    }

    shared.request_preemption();
    worker.interrupt_handle().interrupt();
    if tokio::time::timeout(TEARDOWN_GRACE, &mut join)
        .await
        .is_ok()
    {
        return;
    }

    let state = if shared.cancel_requested() {
        AutomationRunState::Cancelled
    } else {
        AutomationRunState::TimedOut
    };
    if shared.finalize(
        state,
        Some(internal_error(
            "the automation program pinned its worker and could not be preempted",
        )),
        None,
    ) {
        error!(
            "automation run {} wedged its worker; rejecting new runs until the host restarts",
            shared.run_id
        );
        set_unhealthy(&runtime);
    }
}

async fn run_on_worker(runtime: Arc<RuntimeInner>, js_runtime: JSRuntime, request: RunRequest) {
    let shared = request.shared;
    let mut cancel_rx = shared.cancel_receiver();
    let ctx = js_runtime.context();

    if let Err(err) = context::init_automation_context(&ctx, &shared, &request.args) {
        shared.finalize(
            AutomationRunState::InternalError,
            Some(internal_error(format!(
                "automation context initialization failed: {err}"
            ))),
            None,
        );
        teardown(&ctx).await;
        note_worker_recovered(&runtime);
        return;
    }

    let source = Source::from_bytes(&request.source).with_name(&request.source_name);
    let outcome = tokio::select! {
        biased;
        _ = cancel_rx.wait_for(|cancelled| *cancelled) => {
            (AutomationRunState::Cancelled, None, None)
        },
        result = tokio::time::timeout(shared.remaining(), ctx.eval_async::<JSValue>(source)) => {
            match result {
                Err(_) => (AutomationRunState::TimedOut, None, None),
                Ok(Ok(value)) => match js_value_to_json(value) {
                    Ok(output) => (AutomationRunState::Succeeded, None, output),
                    Err(message) => (
                        AutomationRunState::Failed,
                        Some(AutomationRunError {
                            name: "SerializationError".to_string(),
                            message,
                            stack: None,
                            causes: Vec::new(),
                        }),
                        None,
                    ),
                },
                Ok(Err(err)) => {
                    if shared.preemption_requested() {
                        let state = if shared.cancel_requested() {
                            AutomationRunState::Cancelled
                        } else {
                            AutomationRunState::TimedOut
                        };
                        (state, None, None)
                    } else {
                        (
                            AutomationRunState::Failed,
                            Some(context::map_js_error(&ctx, err)),
                            None,
                        )
                    }
                }
            }
        }
    };

    teardown(&ctx).await;
    drop(ctx);
    shared.finalize(outcome.0, outcome.1, outcome.2);
    note_worker_recovered(&runtime);
}

fn js_value_to_json(value: JSValue) -> Result<Option<serde_json::Value>, String> {
    if value.is_undefined() {
        return Ok(None);
    }
    if value.is_null() {
        return Ok(Some(serde_json::Value::Null));
    }
    if value.is_boolean() {
        let value: bool = value
            .into_value()
            .try_into()
            .map_err(|err: rong::RongJSError| err.to_string())?;
        return Ok(Some(value.into()));
    }
    if value.is_number() {
        let value: f64 = value
            .into_value()
            .try_into()
            .map_err(|err: rong::RongJSError| err.to_string())?;
        let value = serde_json::Number::from_f64(value)
            .ok_or_else(|| "automation result contains a non-finite number".to_string())?;
        return Ok(Some(value.into()));
    }
    if value.is_string() {
        let value: String = value
            .into_value()
            .try_into()
            .map_err(|err: rong::RongJSError| err.to_string())?;
        ensure_result_size(value.len())?;
        return Ok(Some(value.into()));
    }
    if let Some(object) = value.into_object() {
        let json = object
            .to_json_string()
            .map_err(|err| format!("automation result is not JSON-compatible: {err}"))?;
        ensure_result_size(json.len())?;
        return serde_json::from_str(&json)
            .map(Some)
            .map_err(|err| format!("failed to decode automation result: {err}"));
    }
    Err("automation result is not JSON-compatible".to_string())
}

fn ensure_result_size(bytes: usize) -> Result<(), String> {
    if bytes > MAX_RESULT_BYTES {
        return Err(format!(
            "automation result is {bytes} bytes; the limit is {MAX_RESULT_BYTES}"
        ));
    }
    Ok(())
}

async fn teardown(ctx: &rong::JSContext) {
    if let Some(timers) = ctx.get_service::<rong_timer::TimerRegistry>() {
        timers.on_shutdown();
    }
    if tokio::time::timeout(Duration::from_secs(3), ctx.shutdown_tasks())
        .await
        .is_err()
    {
        warn!("automation context task drain exceeded its grace period");
    }
}

fn internal_error(message: impl Into<String>) -> AutomationRunError {
    AutomationRunError {
        name: "InternalError".to_string(),
        message: message.into(),
        stack: None,
        causes: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(
        runtime: &AutomationRuntime,
        source: &str,
        timeout_ms: u64,
    ) -> AutomationStartResponse {
        runtime
            .start(AutomationStartArgs {
                source: source.to_string(),
                source_name: Some("fixture.ts".to_string()),
                timeout_ms: Some(timeout_ms),
                args: HashMap::new(),
            })
            .expect("start automation run")
    }

    fn wait_for_terminal(runtime: &AutomationRuntime, run_id: &str) -> AutomationPollResponse {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let response = runtime
                .poll(AutomationPollArgs {
                    run_id: run_id.to_string(),
                    after_seq: 0,
                })
                .expect("poll automation run");
            if response.state.is_terminal() {
                return response;
            }
            assert!(Instant::now() < deadline, "automation run did not finish");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn rejects_oversized_result() {
        assert!(ensure_result_size(MAX_RESULT_BYTES + 1).is_err());
    }

    #[test]
    fn runs_program_and_collects_host_output() {
        let runtime = AutomationRuntime::new().expect("automation runtime");
        let source = r#"
(async () => {
  const host = globalThis.__LINGXIA_AUTOMATION_HOST__;
  await host.attach("result.txt", { mimeType: "text/plain", base64: "aGk=" });
  host.emit({ type: "checkpoint", value: 1 });
  return { ok: true };
})()
"#;
        let started = start(&runtime, source, 5_000);
        let response = wait_for_terminal(&runtime, &started.run_id);

        assert_eq!(response.state, AutomationRunState::Succeeded);
        assert!(matches!(
            &response.events[0].payload,
            AutomationEventPayload::Artifact { name, .. } if name == "result.txt"
        ));
        assert!(matches!(
            &response.events[1].payload,
            AutomationEventPayload::Event { value }
                if value["type"] == "checkpoint"
        ));
        assert_eq!(
            response.result.expect("terminal result").output,
            Some(serde_json::json!({ "ok": true }))
        );
    }

    #[test]
    fn rejects_a_second_concurrent_run() {
        let runtime = AutomationRuntime::new().expect("automation runtime");
        let first = start(
            &runtime,
            "(async () => { await new Promise(resolve => setTimeout(resolve, 5000)); return true; })()",
            10_000,
        );
        let error = runtime
            .start(AutomationStartArgs {
                source: "true".to_string(),
                source_name: None,
                timeout_ms: Some(5_000),
                args: HashMap::new(),
            })
            .expect_err("concurrent run must be rejected");
        assert!(error.contains("automation_run_in_progress"));
        runtime
            .cancel(AutomationCancelArgs {
                run_id: first.run_id.clone(),
                reason: Some("test cleanup".to_string()),
            })
            .expect("cancel first run");
        assert_eq!(
            wait_for_terminal(&runtime, &first.run_id).state,
            AutomationRunState::Cancelled
        );
    }

    #[test]
    fn times_out_and_accepts_a_subsequent_run() {
        let runtime = AutomationRuntime::new().expect("automation runtime");
        let timed_out = start(
            &runtime,
            "(async () => { await new Promise(resolve => setTimeout(resolve, 10000)); })()",
            MIN_TIMEOUT_MS,
        );
        assert_eq!(
            wait_for_terminal(&runtime, &timed_out.run_id).state,
            AutomationRunState::TimedOut
        );

        let next = start(&runtime, "({ recovered: true })", 5_000);
        let response = wait_for_terminal(&runtime, &next.run_id);
        assert_eq!(response.state, AutomationRunState::Succeeded);
        assert_eq!(
            response.result.expect("result").output,
            Some(serde_json::json!({ "recovered": true }))
        );
    }

    #[test]
    fn cancellation_is_terminal_and_idempotent() {
        let runtime = AutomationRuntime::new().expect("automation runtime");
        let started = start(
            &runtime,
            "(async () => { await new Promise(resolve => setTimeout(resolve, 10000)); })()",
            30_000,
        );
        runtime
            .cancel(AutomationCancelArgs {
                run_id: started.run_id.clone(),
                reason: Some("requested by test".to_string()),
            })
            .expect("cancel run");
        assert_eq!(
            wait_for_terminal(&runtime, &started.run_id).state,
            AutomationRunState::Cancelled
        );
        assert_eq!(
            runtime
                .cancel(AutomationCancelArgs {
                    run_id: started.run_id,
                    reason: Some("duplicate cancel".to_string()),
                })
                .expect("repeat terminal cancellation")
                .state,
            AutomationRunState::Cancelled
        );
    }

    #[test]
    fn retains_only_the_latest_completed_runs() {
        let runtime = AutomationRuntime::new().expect("automation runtime");
        let mut run_ids = Vec::new();
        for value in 0..=COMPLETED_RETAINED {
            let started = start(&runtime, &value.to_string(), 5_000);
            wait_for_terminal(&runtime, &started.run_id);
            run_ids.push(started.run_id);
        }

        let final_run = start(&runtime, "true", 5_000);
        wait_for_terminal(&runtime, &final_run.run_id);
        let error = runtime
            .poll(AutomationPollArgs {
                run_id: run_ids[0].clone(),
                after_seq: 0,
            })
            .expect_err("oldest completed run must be evicted");
        assert!(error.contains("unknown automation run"));
    }
}
