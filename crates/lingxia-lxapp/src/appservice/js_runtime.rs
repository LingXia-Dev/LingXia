use crate::bridge::{self, AppServiceCommand};
use crate::error::LxAppError;
#[cfg(feature = "process")]
use crate::host::ProcessSessionAuthority;
use crate::lx;
use crate::lxapp::LxApp;
#[cfg(feature = "process")]
use crate::warn;
use crate::{debug, error, info};

use rong::{JSContext, JSResult, JSRuntime, JSValue, RongJSError, Source, error::HostError};
use rong_console as console;
use rong_http as http;

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use tokio::sync::oneshot;

#[path = "app.rs"]
mod app;
use crate::lifecycle::AppServiceEvent;

#[path = "context_lifecycle.rs"]
mod context_lifecycle;

#[path = "event_bus.rs"]
pub(crate) mod event_bus;

#[path = "page.rs"]
mod page;
use crate::lifecycle::PageLifecycleEvent;
pub use page::PageSvc;

#[path = "plugin.rs"]
mod plugin;

#[path = "runtime_ctx.rs"]
mod runtime_ctx;
pub(crate) use runtime_ctx::set_app_svc_for_ctx;
use runtime_ctx::{register_app_ctx, remove_app_ctx, with_app_svc, with_page_svc_map};

pub(crate) async fn shutdown_app_context(ctx: &JSContext) {
    context_lifecycle::shutdown(ctx).await;
    console::clear_trace_context(ctx);
    remove_app_ctx(ctx);
    // Drain VM-resident jobs while the context still owns its CTX_OPAQUE
    // entry: the pooled worker reuses this JS runtime for the next app, and a
    // leftover engine callback firing after the last owner drops panics in
    // `from_borrowed_raw_ptr` and aborts across the FFI boundary.
    let _ = ctx.runtime().run_pending_jobs();
}

/// Rong modules initialized in every Logic worker. Every name must be backed
/// by an enabled `rong_modules` Cargo feature: resolution fail-fasts on an
/// uncompiled module and the worker aborts before `lx` exists (see the
/// `requested_rong_modules_resolve` test).
const RONG_MODULES: [&str; 13] = [
    "timer",
    "cron",
    "event",
    "exception",
    "abort",
    "encoding",
    "console",
    "url",
    "buffer",
    "stream",
    "http",
    "compression",
    "storage",
];

#[cfg(test)]
mod rong_modules_tests {
    #[test]
    fn requested_rong_modules_resolve() {
        rong_modules::resolve_modules(super::RONG_MODULES)
            .expect("every requested Rong module must be compiled into this build");
    }
}

/// Message type for LxApp service system
pub(crate) enum ServiceMessage {
    // Create a new AppService (JS runtime) for this LxApp instance
    CreateAppSvc {
        lxapp: Arc<LxApp>,
    },
    // Terminate AppService for this LxApp instance. ACK returned when cleanup completes.
    TerminateAppSvc {
        lxapp: Arc<LxApp>,
        worker_id: usize,
        ack_tx: oneshot::Sender<()>,
    },
    // Create a new page service
    CreatePage {
        lxapp: Arc<LxApp>,
        path: String,
        page_instance_id: Option<String>,
        ack_tx: oneshot::Sender<Result<(), String>>,
    },
    // Delete a page service (object-identity safe)
    TerminatePage {
        lxapp: Arc<LxApp>,
        path: String,
        page_instance_id: Option<String>,
    },
    // Call predefined AppService event (typed)
    CallAppSvcEvent {
        lxapp: Arc<LxApp>,
        event: AppServiceEvent,
        args: Option<String>,
    },
    // Call function of PageInstance service with different sources
    CallPageSvc {
        lxapp: Arc<LxApp>,
        path: String,
        page_instance_id: Option<String>,
        source: PageSvcSource,
    },
    // Call typed page event
    CallPageSvcEvent {
        lxapp: Arc<LxApp>,
        path: String,
        page_instance_id: Option<String>,
        event: PageLifecycleEvent,
        args: Option<String>,
    },
    // Native -> JS event dispatch via event bus (e.g., video context)
    DispatchAppBusEvent {
        lxapp: Arc<LxApp>,
        event: event_bus::AppBusEvent,
    },
    Eval {
        /// Report which `lx.*` members the script reached, alongside its value.
        capture_calls: bool,
        lxapp: Arc<LxApp>,
        script: String,
        tx: oneshot::Sender<Result<String, LxAppError>>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkerAssignment {
    Active(usize),
    Terminating { worker_id: usize, token: u64 },
}

impl WorkerAssignment {
    pub(crate) fn worker_id(self) -> usize {
        match self {
            Self::Active(worker_id) | Self::Terminating { worker_id, .. } => worker_id,
        }
    }
}

static NEXT_TERMINATION_TOKEN: AtomicU64 = AtomicU64::new(1);

/// Enum representing different sources of PageInstance service calls
pub enum PageSvcSource {
    /// Call from view layer after the top-level bridge has parsed and routed it.
    Bridge {
        message: crate::bridge::AppServiceCommand,
    },
    /// Call from native layer with explicit function name and args
    Native {
        name: String,
        args: Option<String>, // JSON string of arguments
    },
}

pub(crate) struct WorkerService {
    pub(crate) svc: ServiceMessage,
}

// Handles a typed AppService event
async fn handle_app_service_event(
    worker_id: usize,
    ctx: &JSContext,
    appid: String,
    event: AppServiceEvent,
    args: Option<String>,
) {
    // Resolve AppSvc from registry via JSContext and clone it for use in this async handler.
    let svc = match with_app_svc(ctx, |svc| Ok(svc.clone())) {
        Ok(svc) => svc,
        Err(e) => {
            info!(
                "[Worker {}] Dropping app service event '{}': {}",
                worker_id, event, e
            )
            .with_appid(appid);
            return;
        }
    };

    if matches!(
        event,
        AppServiceEvent::OnLaunch
            | AppServiceEvent::OnShow
            | AppServiceEvent::OnHide
            | AppServiceEvent::OnUserCaptureScreen
    ) && let Err(e) = svc.call_event(ctx, event, args.clone()).await
    {
        error!(
            "[Worker {}] App service event '{}' failed, Error: {}",
            worker_id, event, e
        )
        .with_appid(appid);
    }
}

fn js_value_to_json_string(value: JSValue) -> Result<String, LxAppError> {
    if value.is_undefined() || value.is_null() {
        return Ok("null".to_string());
    }
    if value.is_boolean() {
        let value: bool = value.into_value().try_into().map_err(LxAppError::from)?;
        return Ok(if value { "true" } else { "false" }.to_string());
    }
    if value.is_number() {
        let value: f64 = value.into_value().try_into().map_err(LxAppError::from)?;
        let number = serde_json::Number::from_f64(value)
            .ok_or_else(|| LxAppError::Runtime("eval returned invalid number".to_string()))?;
        return Ok(number.to_string());
    }
    if value.is_string() {
        let value: String = value.into_value().try_into().map_err(LxAppError::from)?;
        return serde_json::to_string(&value).map_err(LxAppError::from);
    }
    if let Some(object) = value.into_object() {
        return object.to_json_string().map_err(LxAppError::from);
    }
    Ok("null".to_string())
}

pub(crate) fn eval_error_from_rong(ctx: &JSContext, error: RongJSError) -> LxAppError {
    if let Some(thrown) = error.thrown_value(ctx) {
        if thrown.is_string() {
            let value: Result<String, RongJSError> = thrown.into_value().try_into();
            if let Ok(value) = value {
                return LxAppError::RongJS(value);
            }
        } else if let Some(object) = thrown.into_object() {
            let name = object
                .get::<_, String>("name")
                .unwrap_or_else(|_| "Error".to_string());
            if let Ok(message) = object.get::<_, String>("message") {
                return LxAppError::RongJS(format!("{name}: {message}"));
            }
        }
    }
    LxAppError::from(error)
}

/// Wraps the caller's script so that `lx` inside it is a recording proxy.
///
/// The binding is **local**, which is the whole point: a direct `eval` inherits
/// the enclosing scope, so the evaluated script sees the proxy while the lxapp's
/// own concurrently running code still sees the real global `lx`. Swapping the
/// global instead would record every background call the app happened to make
/// and credit it to whatever spec was running.
///
/// Primitive members (`lx.env.USER_DATA_PATH`) are published capabilities too;
/// recording only functions and objects would miss the get that reached them.
const RECORDER_PRELUDE: &str = r#"
const __lxCalls = new Set();
const __lxRecord = (target, path) => {
  if (target === null || (typeof target !== "object" && typeof target !== "function")) {
    return target;
  }
  return new Proxy(target, {
    get(obj, key, receiver) {
      const value = Reflect.get(obj, key, receiver);
      if (typeof key === "symbol") return value;
      const next = path + "." + String(key);
      __lxCalls.add(next);
      if (typeof value === "function") {
        return (...args) => Reflect.apply(value, obj, args);
      }
      if (value && typeof value === "object") {
        return __lxRecord(value, next);
      }
      return value;
    },
  });
};
const lx = __lxRecord(globalThis.lx, "lx");
"#;

async fn eval_logic_script_inner(
    ctx: &JSContext,
    script: &str,
    capture_calls: bool,
) -> Result<String, LxAppError> {
    let expression_json = serde_json::to_string(script).map_err(LxAppError::from)?;
    let (prelude, wrap_result) = if capture_calls {
        (RECORDER_PRELUDE, true)
    } else {
        ("", false)
    };
    let expression = if wrap_result {
        format!(
            r#"(async () => {{
{prelude}
  const __lxValue = await eval({expression_json});
  return {{ __lxEval: 1, value: __lxValue, calls: [...__lxCalls] }};
}})()"#
        )
    } else {
        format!(
            r#"(async () => {{
  return await eval({expression_json});
}})()"#
        )
    };
    match ctx
        .eval_async::<JSValue>(Source::from_bytes(expression))
        .await
    {
        Ok(value) => js_value_to_json_string(value),
        Err(expression_error) if script_may_be_function_body(ctx, script, &expression_error) => {
            let body = if wrap_result {
                format!(
                    r#"(async () => {{
{prelude}
  const __lxValue = await (async () => {{
{script}
  }})();
  return {{ __lxEval: 1, value: __lxValue, calls: [...__lxCalls] }};
}})()"#
                )
            } else {
                format!(
                    r#"(async () => {{
{script}
}})()"#
                )
            };
            let value = ctx
                .eval_async::<JSValue>(Source::from_bytes(body))
                .await
                .map_err(|body_error| eval_error_from_rong(ctx, body_error))?;
            js_value_to_json_string(value)
        }
        Err(expression_error) => Err(eval_error_from_rong(ctx, expression_error)),
    }
}

fn script_may_be_function_body(
    ctx: &JSContext,
    script: &str,
    expression_error: &RongJSError,
) -> bool {
    let is_syntax_error = expression_error
        .thrown_value(ctx)
        .and_then(|value| value.into_object())
        .and_then(|object| object.get::<_, String>("name").ok())
        .is_some_and(|name| name == "SyntaxError");
    if !is_syntax_error {
        return false;
    }
    let trimmed = script.trim_start();
    trimmed.starts_with("return")
        || trimmed.starts_with("const ")
        || trimmed.starts_with("let ")
        || trimmed.starts_with("var ")
        || trimmed.starts_with("if ")
        || trimmed.starts_with("for ")
        || trimmed.starts_with("while ")
        || trimmed.starts_with("try ")
        || trimmed.contains(';')
}

// Handles a bridge-routed message that must enter the JS runtime worker.
async fn handle_bridge_source(
    page_svc: &PageSvc,
    message: AppServiceCommand,
) -> Result<(), LxAppError> {
    match message {
        AppServiceCommand::BeginSessionWork { work_id } => {
            page_svc.begin_session_work(work_id).await;
            Ok(())
        }
        AppServiceCommand::CancelSessionWork { work_id } => {
            page_svc.cancel_session_work(work_id).await;
            Ok(())
        }
        AppServiceCommand::Ready { work_id, outbound } => {
            page_svc.handle_bridge_ready(work_id, outbound).await;
            Ok(())
        }
        AppServiceCommand::StateSnapshot {
            work_id,
            outbound,
            id,
            scope,
        } => {
            if !page_svc.session_work_is_active(work_id).await {
                return Ok(());
            }
            let bridge = page_svc.bridge();
            match page_svc.get_state_snapshot(scope.as_deref()).await {
                Ok(snapshot) => bridge.send_res_ok_for_context(
                    page_svc,
                    work_id,
                    outbound.as_ref(),
                    id,
                    snapshot,
                )?,
                Err(err) => bridge.send_res_err_for_context(
                    page_svc,
                    work_id,
                    outbound.as_ref(),
                    id,
                    bridge::BRIDGE_INTERNAL_ERROR,
                    Some(err.to_string()),
                    None,
                )?,
            }
            Ok(())
        }
        AppServiceCommand::Req {
            work_id,
            outbound,
            id,
            method,
            params_json,
            cancel_rx,
            pending_request,
        } => {
            if !page_svc.session_work_is_active(work_id).await {
                return Ok(());
            }
            let bridge = page_svc.bridge();
            let result = page::with_document_callback_work(
                work_id,
                outbound.clone(),
                page_svc.handle_req(
                    work_id,
                    outbound.clone(),
                    &id,
                    &method,
                    params_json.as_deref(),
                    cancel_rx,
                ),
            )
            .await;
            drop(pending_request);
            match result {
                Ok(json) => bridge.send_res_ok_for_context(
                    page_svc,
                    work_id,
                    outbound.as_ref(),
                    id,
                    json,
                )?,
                Err(err) if err.code == bridge::BRIDGE_CANCELED => {
                    // Cancellation is teardown control flow. Reply while a cached
                    // View still exists, but tolerate a concurrent WebView detach.
                    let _ = bridge.send_res_err_for_context(
                        page_svc,
                        work_id,
                        outbound.as_ref(),
                        id,
                        &err.code,
                        err.message,
                        err.data,
                    );
                }
                Err(err) => bridge.send_res_err_for_context(
                    page_svc,
                    work_id,
                    outbound.as_ref(),
                    id,
                    &err.code,
                    err.message,
                    err.data,
                )?,
            }
            Ok(())
        }
        AppServiceCommand::Notify {
            work_id,
            outbound,
            method,
            params_json,
        } => {
            if !page_svc.session_work_is_active(work_id).await {
                return Ok(());
            }
            page_svc
                .handle_notify(work_id, outbound, &method, params_json.as_deref())
                .await;
            Ok(())
        }
        AppServiceCommand::ChOpen {
            work_id,
            outbound,
            id,
            topic,
            params_json,
        } => {
            if !page_svc.session_work_is_active(work_id).await {
                return Ok(());
            }
            let bridge = page_svc.bridge();
            match page_svc
                .handle_ch_open(
                    work_id,
                    outbound.clone(),
                    &id,
                    &topic,
                    params_json.as_deref(),
                )
                .await
            {
                Ok(result_rx) => {
                    let ctx = page_svc.get_ctx();
                    let page_svc = page_svc.clone();
                    context_lifecycle::spawn(&ctx, move |_ctx| async move {
                        let result = result_rx.await.unwrap_or_else(|_| {
                            Err(bridge::RpcError::new(bridge::BRIDGE_CANCELED, None))
                        });
                        match result {
                            Ok(()) => {
                                let _ = bridge.send_ch_ack_ok_for_context(
                                    &page_svc,
                                    work_id,
                                    outbound.as_ref(),
                                    id,
                                );
                            }
                            Err(err) => {
                                let _ = bridge.send_ch_ack_err_for_context(
                                    &page_svc,
                                    work_id,
                                    outbound.as_ref(),
                                    id,
                                    &err.code,
                                    err.message,
                                    err.data,
                                );
                            }
                        }
                    });
                }
                Err(err) => bridge.send_ch_ack_err_for_context(
                    page_svc,
                    work_id,
                    outbound.as_ref(),
                    id,
                    &err.code,
                    err.message,
                    err.data,
                )?,
            }
            Ok(())
        }
        AppServiceCommand::ChData {
            work_id,
            id,
            payload_json,
        } => {
            if !page_svc.session_work_is_active(work_id).await {
                return Ok(());
            }
            if let Err(err) = page_svc.handle_ch_data(work_id, &id, &payload_json).await {
                error!("channel '{}' data handler failed: {}", id, err.code)
                    .with_appid(page_svc.page.appid())
                    .with_path(page_svc.page.path());
            }
            Ok(())
        }
        AppServiceCommand::ChClose {
            work_id,
            id,
            code,
            reason,
        } => {
            if !page_svc.session_work_is_active(work_id).await {
                return Ok(());
            }
            page_svc
                .handle_ch_close(work_id, &id, code.as_deref(), reason.as_deref())
                .await;
            Ok(())
        }
        AppServiceCommand::StateAck {
            work_id,
            scope,
            rev,
        } => {
            page_svc.handle_state_ack(work_id, scope, rev).await;
            Ok(())
        }
    }
}

// Handles a call from native code to a PageInstance service function
fn handle_native_source(page_svc: &PageSvc, appid: String, name: String, args: Option<String>) {
    let ctx = page_svc.get_ctx();
    let page_svc_clone = page_svc.clone();
    let name_clone = name.clone();

    context_lifecycle::spawn(&ctx, move |ctx| async move {
        if let Err(e) = page_svc_clone
            .call_or_event_from_native(&ctx, &name, args.as_deref())
            .await
        {
            crate::error!("PageInstance service call '{}' failed: {}", name_clone, e)
                .with_appid(appid)
                .with_path(page_svc_clone.page.path());
        }
    });
}

/// The core logic for a persistent worker task.
/// This function is a handler for messages received by the worker.
pub(crate) async fn lxapp_service_handler(
    worker_id: usize,
    runtime: JSRuntime,
    message: ServiceMessage,
    current_ctx: &mut Option<JSContext>,
) {
    match message {
        ServiceMessage::CreateAppSvc { lxapp } => {
            let ctx = runtime.context();

            // Register LxApp runtime context and bind identity to JSContext
            register_app_ctx(&ctx, &lxapp);

            // register PageInstance, App and getApp function
            if let Err(e) = app::init(&ctx) {
                error!(
                    "[Worker {}] Failed to initialize App runtime: {}",
                    worker_id, e
                )
                .with_appid(lxapp.appid.clone());
                return;
            }
            if let Err(e) = page::init(&ctx) {
                error!(
                    "[Worker {}] Failed to initialize PageInstance runtime: {}",
                    worker_id, e
                )
                .with_appid(lxapp.appid.clone());
                return;
            }
            if let Err(e) = plugin::init(&ctx) {
                error!(
                    "[Worker {}] Failed to initialize Plugin runtime: {}",
                    worker_id, e
                )
                .with_appid(lxapp.appid.clone());
                return;
            }
            event_bus::init(&ctx);

            let app_ctx = LxAppCtx::new(lxapp.clone());

            console::set_trace_context(
                &ctx,
                console::ConsoleTraceContext {
                    namespace: Some(lxapp.appid.clone()),
                    scope: Some("appservice".to_string()),
                },
            );

            // Set network access guard to prevent unauthorized domain access
            http::set_network_access_guard(Box::new(app_ctx));

            if let Err(e) = rong_modules::init(&ctx, RONG_MODULES) {
                error!(
                    "[Worker {}] Failed to initialize Rong modules: {}",
                    worker_id, e
                )
                .with_appid(lxapp.appid.clone());
                return;
            }
            #[cfg(feature = "process")]
            if lxapp.is_home_lxapp && lingxia_app_context::process_enabled() {
                if !lxapp.process_access_enabled() {
                    warn!(
                        "[Worker {}] Process capability requires both security.privileges: [process] and a native ControlApp Process grant",
                        worker_id
                    )
                    .with_appid(lxapp.appid.clone());
                } else if let Err(e) = rong_command::init_with_authority(
                    &ctx,
                    Arc::new(ProcessSessionAuthority::for_lxapp(&lxapp)),
                ) {
                    error!(
                        "[Worker {}] Failed to initialize process capability: {}",
                        worker_id, e
                    )
                    .with_appid(lxapp.appid.clone());
                    return;
                }
            }
            let _ = lx::init(&ctx);

            // Execute a closure with access to the list of registered extensions.
            crate::lx::extension::with_registered_extensions(|user_extensions| {
                info!(
                    "[Worker {}] Initializing {} user-registered extensions",
                    worker_id,
                    user_extensions.len()
                )
                .with_appid(lxapp.appid.clone());

                // Iterate through the list and initialize each extension.
                for (index, user_extension) in user_extensions.iter().enumerate() {
                    if let Err(e) = user_extension.init(&ctx) {
                        error!(
                            "[Worker {}] Failed to initialize user extension #{}: {}",
                            worker_id, index, e
                        )
                        .with_appid(lxapp.appid.clone());
                    }
                }
            });

            info!("[Worker {}] Created JS context", worker_id).with_appid(lxapp.appid.clone());

            match lxapp.logic_entry_source(&ctx).await {
                Ok(Some(js)) => match ctx.eval::<()>(js) {
                    Ok(_) => {
                        info!("[Worker {}] Successfully loaded logic JS", worker_id)
                            .with_appid(lxapp.appid.clone());
                    }
                    Err(e) => {
                        info!("[Worker {}] eval logic JS  failed: {}", worker_id, e)
                            .with_appid(lxapp.appid.clone());
                    }
                },
                Ok(None) => {
                    info!(
                        "[Worker {}] Logic disabled; skipping JS bootstrap",
                        worker_id
                    )
                    .with_appid(lxapp.appid.clone());
                }
                Err(e) => {
                    error!("[Worker {}] Failed to load logic source: {}", worker_id, e)
                        .with_appid(lxapp.appid.clone());
                }
            }

            *current_ctx = Some(ctx.clone());
        }
        ServiceMessage::TerminateAppSvc { lxapp, ack_tx, .. } => {
            if let Some(ctx) = current_ctx.as_ref() {
                shutdown_app_context(ctx).await;
                *current_ctx = None;
                info!("[Worker {}] Removed LxApp context ", worker_id)
                    .with_appid(lxapp.appid.clone());
            }
            // Clear guards on app terminate so the previous LxAppCtx is dropped immediately.
            http::set_network_access_guard(Box::new(DenyAllNetworkAccessGuard));
            // ACK back to the caller that cleanup is complete
            let _ = ack_tx.send(());
        }
        ServiceMessage::CreatePage {
            lxapp,
            path,
            page_instance_id,
            ack_tx,
        } => {
            let result = if let Some(ctx) = current_ctx.as_ref() {
                // A CreatePage for an older session can land on the recycled
                // worker after its app context was replaced; like TerminatePage,
                // drop it quietly instead of failing against the new context.
                let same_app = LxApp::from_ctx(ctx)
                    .map(|ctx_app| ctx_app.session.id == lxapp.session.id)
                    .unwrap_or(false);
                if !same_app {
                    info!(
                        "[Worker {}] Ignored CreatePage for different LxApp instance",
                        worker_id
                    )
                    .with_appid(lxapp.appid.clone())
                    .with_path(path.clone());
                    let _ = ack_tx.send(Ok(()));
                    return;
                }
                debug!(
                    "[Worker {}] Creating page svc (instance {:?})",
                    worker_id, page_instance_id
                )
                .with_appid(lxapp.appid.clone())
                .with_path(path.clone());
                // The instance can be disposed between queueing and execution
                // (a relaunch tearing the stack down races a queued rebuild);
                // that is churn, not a failure.
                if let Some(id) = page_instance_id.as_deref()
                    && lxapp.get_page_by_instance_id_str(id).is_none()
                {
                    info!(
                        "[Worker {}] Skipped CreatePage for disposed instance {}",
                        worker_id, id
                    )
                    .with_appid(lxapp.appid.clone())
                    .with_path(path.clone());
                    let _ = ack_tx.send(Err("page instance disposed".to_string()));
                    return;
                }
                match PageSvc::create_in_ctx(ctx, &path, page_instance_id.as_deref()).await {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        let msg = e.to_string();
                        // The instance can also be disposed DURING creation
                        // (the preflight raced the disposal) — still churn.
                        let disposed_during_create = page_instance_id
                            .as_deref()
                            .is_some_and(|id| lxapp.get_page_by_instance_id_str(id).is_none());
                        if disposed_during_create {
                            info!(
                                "[Worker {}] Dropped CreatePage for instance disposed mid-create: {}",
                                worker_id, msg
                            )
                            .with_appid(lxapp.appid.clone())
                            .with_path(&path);
                        } else {
                            error!("[Worker {}] create_in_ctx failed: {}", worker_id, e)
                                .with_appid(lxapp.appid.clone())
                                .with_path(&path);
                        }
                        Err(msg)
                    }
                }
            } else {
                let msg = "JS context not available".to_string();
                error!("[Worker {}] create_in_ctx: {}", worker_id, msg)
                    .with_appid(lxapp.appid.clone())
                    .with_path(&path);
                Err(msg)
            };
            let _ = ack_tx.send(result);
        }
        ServiceMessage::TerminatePage {
            lxapp,
            path,
            page_instance_id,
        } => {
            if let Some(ctx) = current_ctx.as_ref() {
                // Ensure this TerminatePage belongs to the same LxApp
                let same_app = LxApp::from_ctx(ctx)
                    .map(|ctx_app| ctx_app.session.id == lxapp.session.id)
                    .unwrap_or(false);
                if !same_app {
                    info!(
                        "[Worker {}] Ignored TerminatePage for different LxApp instance",
                        worker_id
                    )
                    .with_appid(lxapp.appid.clone())
                    .with_path(path.clone());
                    return;
                }

                // Services register under their instance id alone; a path can
                // have several live instances.
                let page_svc = with_page_svc_map(ctx, |page_svc_map| {
                    Ok(page_instance_id
                        .as_deref()
                        .and_then(|id| page_svc_map.borrow_mut().remove(id)))
                })
                .unwrap_or(None);

                if let Some(page_svc) = page_svc {
                    page_svc.mark_terminated();
                    page_svc
                        .close_channels(bridge::BRIDGE_CANCELED, "Page terminated")
                        .await;
                    // Instance-scoped: terminating one instance must not clear
                    // a same-path sibling's subscriptions.
                    event_bus::clear_page(ctx, &page_svc.get_page().instance_id_string());

                    info!("[Worker {}] Removed page", worker_id)
                        .with_appid(lxapp.appid.clone())
                        .with_path(path);
                }
            }
        }
        ServiceMessage::CallAppSvcEvent { lxapp, event, args } => {
            if let Some(ctx) = current_ctx.as_ref() {
                // Ensure this event targets the same LxApp bound to ctx
                let same_app = LxApp::from_ctx(ctx)
                    .map(|ctx_app| ctx_app.session.id == lxapp.session.id)
                    .unwrap_or(false);
                if same_app {
                    // Don't block the worker message pump on user JS lifecycle handlers.
                    // If an app handler awaits network/IO, blocking here can starve bridge handshake
                    // and other view messages, causing "Handshake timeout" even when transport is OK.
                    let appid = lxapp.appid.clone();
                    context_lifecycle::spawn(ctx, move |ctx| async move {
                        handle_app_service_event(worker_id, &ctx, appid, event, args).await;
                    });
                }
            }
        }
        ServiceMessage::CallPageSvc {
            lxapp,
            path,
            page_instance_id,
            source,
        } => {
            if let Some(ctx) = current_ctx.as_ref() {
                match source {
                    PageSvcSource::Bridge { message } => {
                        let page_svc = with_page_svc_map(ctx, |page_svc_map| {
                            Ok(page_instance_id
                                .as_deref()
                                .and_then(|id| page_svc_map.borrow().get(id).cloned()))
                        })
                        .unwrap_or(None);

                        if let Some(page_svc) = page_svc {
                            if let Err(e) = handle_bridge_source(&page_svc, message).await {
                                let page = page_svc.get_page();
                                if page.document_is_departing() || page.webview().is_none() {
                                    debug!(
                                        "[Worker {}] Dropping bridge response for departed page: {}",
                                        worker_id, e
                                    )
                                    .with_appid(lxapp.appid.clone())
                                    .with_path(path.clone());
                                } else {
                                    error!(
                                        "[Worker {}] Handle bridge message error: {}",
                                        worker_id, e
                                    )
                                    .with_appid(lxapp.appid.clone())
                                    .with_path(path.clone());
                                }
                            }
                        } else {
                            info!(
                                "[Worker {}] Dropping bridge message: page service not loaded",
                                worker_id
                            )
                            .with_appid(lxapp.appid.clone())
                            .with_path(path);
                        }
                    }
                    PageSvcSource::Native { name, args } => {
                        let page_svc = with_page_svc_map(ctx, |page_svc_map| {
                            Ok(page_instance_id
                                .as_deref()
                                .and_then(|id| page_svc_map.borrow().get(id).cloned()))
                        })
                        .unwrap_or(None);

                        if let Some(page_svc) = page_svc {
                            handle_native_source(&page_svc, lxapp.appid.clone(), name, args);
                        } else {
                            info!(
                                "[Worker {}] Dropping native call: page service not loaded",
                                worker_id
                            )
                            .with_appid(lxapp.appid.clone())
                            .with_path(path);
                        }
                    }
                }
            }
        }
        ServiceMessage::CallPageSvcEvent {
            lxapp,
            path,
            page_instance_id,
            event,
            args,
        } => {
            if let Some(ctx) = current_ctx.as_ref() {
                // Resolve PageSvc from registry
                let page_svc = with_page_svc_map(ctx, |page_svc_map| {
                    let page_svc_map = page_svc_map.borrow();
                    Ok(page_instance_id
                        .as_deref()
                        .and_then(|id| page_svc_map.get(id).cloned()))
                })
                .unwrap_or(None);

                if let Some(page_svc) = page_svc {
                    debug!(
                        "[Worker {}] page event '{}' → instance {}",
                        worker_id,
                        event,
                        page_svc.get_page().instance_id_string()
                    )
                    .with_appid(lxapp.appid.clone())
                    .with_path(path.clone());
                    // Keeps user lifecycle handlers off the worker pump while
                    // preserving per-page dispatch order.
                    page_svc.enqueue_lifecycle_event(ctx, event, args);
                } else {
                    info!(
                        "[Worker {}] Dropping page event: page service not loaded",
                        worker_id
                    )
                    .with_appid(lxapp.appid.clone())
                    .with_path(path);
                }
            }
        }
        ServiceMessage::DispatchAppBusEvent { lxapp, event } => {
            if let Some(ctx) = current_ctx.as_ref() {
                let same_app = LxApp::from_ctx(ctx)
                    .map(|ctx_app| ctx_app.session.id == lxapp.session.id)
                    .unwrap_or(false);
                if same_app {
                    // Don't block the worker message pump on user JS event handlers. Like app/page
                    // lifecycle events, event bus handlers can await network/IO and would
                    // otherwise starve view messages (including handshake retries).
                    let appid = lxapp.appid.clone();
                    context_lifecycle::spawn(ctx, move |ctx| async move {
                        if let Err(e) = event_bus::dispatch_app_bus_event(&ctx, &event).await {
                            error!(
                                "[Worker {}] Dispatch app bus event failed: {}",
                                worker_id, e
                            )
                            .with_appid(appid);
                        }
                    });
                }
            }
        }
        ServiceMessage::Eval {
            capture_calls,
            lxapp,
            script,
            tx,
        } => {
            let result = if let Some(ctx) = current_ctx.as_ref() {
                let same_app = LxApp::from_ctx(ctx)
                    .map(|ctx_app| ctx_app.session.id == lxapp.session.id)
                    .unwrap_or(false);
                if same_app {
                    eval_logic_script_inner(ctx, &script, capture_calls).await
                } else {
                    Err(LxAppError::Runtime(format!(
                        "logic runtime is bound to a different lxapp than {}",
                        lxapp.appid
                    )))
                }
            } else {
                Err(LxAppError::Runtime(format!(
                    "logic runtime is not ready for {}",
                    lxapp.appid
                )))
            };
            let _ = tx.send(result);
        }
    }
}

/// Create a new mini-app service - enforces 1:1 appid->worker mapping
pub(crate) fn create_app_svc(
    lxapp: Arc<crate::lxapp::LxApp>,
    sender: &mpsc::Sender<ServiceMessage>,
    instance_assignments: &Arc<Mutex<HashMap<usize, WorkerAssignment>>>,
    free_workers: &Arc<Mutex<VecDeque<usize>>>,
) -> Result<(), LxAppError> {
    let appid = lxapp.appid.clone();

    let key = lxapp.as_ref() as *const _ as usize;
    if reactivate_or_reuse_assignment(&lxapp, sender, instance_assignments, key)? {
        return Ok(());
    }

    // Check if we have free workers available
    let worker_id = {
        let mut free_workers_guard = free_workers.lock().unwrap();
        if free_workers_guard.is_empty() {
            return Err(LxAppError::ResourceExhausted(
                "No available workers for new mini-app".to_string(),
            ));
        }
        free_workers_guard.pop_front().unwrap()
    };

    // Publish the worker mapping only after the CreateAppSvc message has been
    // enqueued. A concurrent page creation treats the mapping as readiness to
    // route CreatePage, so exposing it before this send can let CreatePage reach
    // the worker before Page.js and the app logic have registered page definitions.
    {
        let mut assignments = instance_assignments.lock().unwrap();
        if reactivate_or_reuse_locked(&lxapp, sender, &mut assignments, key)? {
            free_workers.lock().unwrap().push_front(worker_id);
            return Ok(());
        }
        if let Err(e) = sender.send(ServiceMessage::CreateAppSvc { lxapp }) {
            free_workers.lock().unwrap().push_front(worker_id);
            return Err(e.into());
        }
        assignments.insert(key, WorkerAssignment::Active(worker_id));
    }

    info!("Assigned dedicated worker {} to app {}", worker_id, appid);
    Ok(())
}

fn reactivate_or_reuse_assignment(
    lxapp: &Arc<LxApp>,
    sender: &mpsc::Sender<ServiceMessage>,
    instance_assignments: &Arc<Mutex<HashMap<usize, WorkerAssignment>>>,
    key: usize,
) -> Result<bool, LxAppError> {
    let mut assignments = instance_assignments.lock().unwrap();
    reactivate_or_reuse_locked(lxapp, sender, &mut assignments, key)
}

fn reactivate_or_reuse_locked(
    lxapp: &Arc<LxApp>,
    sender: &mpsc::Sender<ServiceMessage>,
    assignments: &mut HashMap<usize, WorkerAssignment>,
    key: usize,
) -> Result<bool, LxAppError> {
    let Some(assignment) = assignments.get(&key).copied() else {
        return Ok(false);
    };

    if matches!(assignment, WorkerAssignment::Terminating { .. }) {
        // The terminate message is already ahead of this create in the same
        // queue. Marking the assignment active also prevents its old ACK task
        // from releasing the worker after the new context has been requested.
        sender.send(ServiceMessage::CreateAppSvc {
            lxapp: lxapp.clone(),
        })?;
        assignments.insert(key, WorkerAssignment::Active(assignment.worker_id()));
        info!("Reactivating worker for app {}", lxapp.appid);
    } else {
        info!("Reusing existing worker for app {}", lxapp.appid);
    }
    Ok(true)
}

/// Terminate a mini-app service - breaks 1:1 mapping and returns worker to pool
pub(crate) fn terminate_app_svc(
    lxapp_arc: Arc<LxApp>,
    sender: &mpsc::Sender<ServiceMessage>,
    instance_assignments: &Arc<Mutex<HashMap<usize, WorkerAssignment>>>,
    free_workers: &Arc<Mutex<VecDeque<usize>>>,
) -> Result<(), LxAppError> {
    let appid = lxapp_arc.appid.clone();
    let key = lxapp_arc.as_ref() as *const _ as usize;
    let (worker_id, token, rx) = {
        let mut assignments = instance_assignments.lock().unwrap();
        let Some(assignment) = assignments.get(&key).copied() else {
            info!(
                "No active worker mapping for app {}; skipping terminate",
                appid
            );
            return Ok(());
        };
        if matches!(assignment, WorkerAssignment::Terminating { .. }) {
            info!("Worker termination already pending for app {}", appid);
            return Ok(());
        }

        let worker_id = assignment.worker_id();
        let token = NEXT_TERMINATION_TOKEN.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        // Enqueue and publish Terminating under one lock so a concurrent reopen
        // cannot put CreateAppSvc ahead of this termination.
        sender.send(ServiceMessage::TerminateAppSvc {
            lxapp: lxapp_arc.clone(),
            worker_id,
            ack_tx: tx,
        })?;
        assignments.insert(key, WorkerAssignment::Terminating { worker_id, token });
        (worker_id, token, rx)
    };

    let assignments = instance_assignments.clone();
    let free_workers = free_workers.clone();
    crate::executor::spawn(await_termination_ack(
        appid,
        key,
        worker_id,
        token,
        rx,
        assignments,
        free_workers,
        Duration::from_secs(3),
    ));

    Ok(())
}

async fn await_termination_ack(
    appid: String,
    key: usize,
    worker_id: usize,
    token: u64,
    mut rx: oneshot::Receiver<()>,
    assignments: Arc<Mutex<HashMap<usize, WorkerAssignment>>>,
    free_workers: Arc<Mutex<VecDeque<usize>>>,
    ack_timeout: Duration,
) {
    match tokio::time::timeout(ack_timeout, &mut rx).await {
        Ok(Ok(())) => {
            info!("Terminate ACK received").with_appid(appid.clone());
            let released =
                take_terminated_assignment(&mut assignments.lock().unwrap(), key, worker_id, token);
            if let Some(worker_id) = released {
                free_workers.lock().unwrap().push_back(worker_id);
                info!("Released dedicated worker {} from app {}", worker_id, appid);
            }
        }
        Ok(Err(_)) => {
            let quarantined =
                take_terminated_assignment(&mut assignments.lock().unwrap(), key, worker_id, token);
            if quarantined.is_some() {
                error!("Terminate ACK channel closed; quarantining worker {worker_id}")
                    .with_appid(appid);
            }
        }
        Err(_) => {
            let quarantined =
                take_terminated_assignment(&mut assignments.lock().unwrap(), key, worker_id, token);
            if quarantined.is_none() {
                return;
            }

            error!("Terminate ACK timeout; quarantining worker {worker_id}")
                .with_appid(appid.clone());
            // The terminate message carries its original worker id, so it still
            // reaches the quarantined worker after this assignment is removed.
            // A late ACK proves cleanup completed and makes reuse safe again.
            match rx.await {
                Ok(()) => {
                    free_workers.lock().unwrap().push_back(worker_id);
                    info!(
                        "Released quarantined worker {} after late ACK for app {}",
                        worker_id, appid
                    );
                }
                Err(_) => {
                    error!("Quarantined worker {worker_id} never acknowledged termination")
                        .with_appid(appid);
                }
            }
        }
    }
}

fn take_terminated_assignment(
    assignments: &mut HashMap<usize, WorkerAssignment>,
    key: usize,
    worker_id: usize,
    token: u64,
) -> Option<usize> {
    let expected = WorkerAssignment::Terminating { worker_id, token };
    if assignments.get(&key) == Some(&expected) {
        assignments.remove(&key).map(WorkerAssignment::worker_id)
    } else {
        None
    }
}

pub(crate) fn restart_app_svc(
    lxapp: Arc<LxApp>,
    sender: &mpsc::Sender<ServiceMessage>,
    instance_assignments: &Arc<Mutex<HashMap<usize, WorkerAssignment>>>,
) -> Result<(), LxAppError> {
    let key = lxapp.as_ref() as *const _ as usize;
    let mut assignments = instance_assignments.lock().unwrap();
    let Some(assignment) = assignments.get(&key).copied() else {
        return Err(LxAppError::Runtime(format!(
            "No active worker mapping for app {}",
            lxapp.appid
        )));
    };

    if matches!(assignment, WorkerAssignment::Terminating { .. }) {
        sender.send(ServiceMessage::CreateAppSvc {
            lxapp: lxapp.clone(),
        })?;
        assignments.insert(key, WorkerAssignment::Active(assignment.worker_id()));
        return Ok(());
    }

    let (ack_tx, _ack_rx) = oneshot::channel();
    sender.send(ServiceMessage::TerminateAppSvc {
        lxapp: lxapp.clone(),
        worker_id: assignment.worker_id(),
        ack_tx,
    })?;
    sender.send(ServiceMessage::CreateAppSvc { lxapp })?;
    Ok(())
}

#[cfg(test)]
mod worker_assignment_tests {
    use super::{WorkerAssignment, await_termination_ack, take_terminated_assignment};
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn reactivated_assignment_is_not_released_by_old_termination() {
        let mut assignments = HashMap::from([(7, WorkerAssignment::Active(3))]);

        assert_eq!(take_terminated_assignment(&mut assignments, 7, 3, 11), None);
        assert_eq!(assignments.get(&7), Some(&WorkerAssignment::Active(3)));
    }

    #[test]
    fn only_matching_termination_releases_worker() {
        let mut assignments = HashMap::from([(
            7,
            WorkerAssignment::Terminating {
                worker_id: 3,
                token: 12,
            },
        )]);

        assert_eq!(take_terminated_assignment(&mut assignments, 7, 3, 11), None);
        assert_eq!(
            take_terminated_assignment(&mut assignments, 7, 3, 12),
            Some(3)
        );
        assert!(!assignments.contains_key(&7));
    }

    #[tokio::test]
    async fn timeout_quarantines_worker_until_late_ack() {
        let assignments = Arc::new(Mutex::new(HashMap::from([(
            7,
            WorkerAssignment::Terminating {
                worker_id: 3,
                token: 12,
            },
        )])));
        let free_workers = Arc::new(Mutex::new(VecDeque::new()));
        let (tx, rx) = tokio::sync::oneshot::channel();

        let wait = tokio::spawn(await_termination_ack(
            "test.app".to_string(),
            7,
            3,
            12,
            rx,
            assignments.clone(),
            free_workers.clone(),
            Duration::from_millis(1),
        ));
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let quarantined = !assignments.lock().unwrap().contains_key(&7);
                if quarantined {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("worker assignment should be quarantined after the ACK timeout");

        assert!(!assignments.lock().unwrap().contains_key(&7));
        assert!(free_workers.lock().unwrap().is_empty());

        tx.send(()).unwrap();
        wait.await.unwrap();
        assert_eq!(free_workers.lock().unwrap().pop_front(), Some(3));
    }

    #[tokio::test]
    async fn acknowledged_termination_releases_worker_immediately() {
        let assignments = Arc::new(Mutex::new(HashMap::from([(
            7,
            WorkerAssignment::Terminating {
                worker_id: 3,
                token: 12,
            },
        )])));
        let free_workers = Arc::new(Mutex::new(VecDeque::new()));
        let (tx, rx) = tokio::sync::oneshot::channel();
        tx.send(()).unwrap();

        await_termination_ack(
            "test.app".to_string(),
            7,
            3,
            12,
            rx,
            assignments.clone(),
            free_workers.clone(),
            Duration::from_secs(1),
        )
        .await;

        assert!(!assignments.lock().unwrap().contains_key(&7));
        assert_eq!(free_workers.lock().unwrap().pop_front(), Some(3));
    }
}

/// Wrapper for LxApp to implement external traits
#[derive(Clone)]
struct LxAppCtx {
    lxapp: Arc<LxApp>,
}

#[derive(Debug)]
struct DenyAllNetworkAccessGuard;

impl http::NetworkAccessGuard for DenyAllNetworkAccessGuard {
    fn check_access(&self, _domain: &str) -> JSResult<()> {
        Err(network_access_denied_error("network access is denied"))
    }
}

impl LxAppCtx {
    pub fn new(lxapp: Arc<LxApp>) -> Self {
        Self { lxapp }
    }
}

impl std::fmt::Debug for LxAppCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LxAppCtx")
            .field("appid", &self.lxapp.appid)
            .finish()
    }
}

impl http::NetworkAccessGuard for LxAppCtx {
    /// Check if the mini app has access to the specified domain
    /// Returns Ok(()) if access is granted, Err with error message if denied
    fn check_access(&self, domain: &str) -> JSResult<()> {
        if self.lxapp.is_domain_allowed(domain) {
            Ok(())
        } else {
            Err(network_access_denied_error(format!(
                "domain '{domain}' is not allowed by lxapp security policy"
            )))
        }
    }
}

fn network_access_denied_error(detail: impl AsRef<str>) -> RongJSError {
    HostError::new(rong::error::E_PERMISSION_DENIED, "Permission denied")
        .with_data(rong::err_data!({ bizCode: (3000), detail: (detail.as_ref()) }))
        .into()
}
