use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error, js_internal_error,
    js_invalid_parameter_error,
};
use futures::channel::{mpsc, oneshot};
use futures::lock::Mutex;
use futures::{SinkExt, StreamExt};
use lingxia_transfer::user_cache::{
    self, DownloadBehavior, DownloadEvent as TransferDownloadEvent,
};
use lxapp::{LxApp, lx};
use rong::{
    HostError, IntoJSObj, JSContext, JSFunc, JSObject, JSResult, JSRuntimeService, JSSymbol,
    JSValue, Promise, function::Optional,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[derive(Debug, Clone)]
struct ParsedDownloadOptions {
    url: String,
    headers: Vec<(String, String)>,
    timeout_ms: Option<u64>,
    file_path: Option<String>,
    signal: Option<JSObject>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSDownloadResult {
    #[rename = "filePath"]
    file_path: String,
    #[rename = "mimeType"]
    mime_type: Option<String>,
    size: u64,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSDownloadEvent {
    kind: String,
    #[rename = "downloadedBytes"]
    downloaded_bytes: Option<u64>,
    #[rename = "totalBytes"]
    total_bytes: Option<u64>,
    progress: Option<f64>,
    result: Option<JSDownloadResult>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSDownloadIteratorStep {
    done: bool,
    value: Option<JSDownloadEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DownloadTaskStatus {
    Running,
    Paused,
    Canceled,
    Succeeded,
    Failed,
}

impl DownloadTaskStatus {
    fn is_terminal(self) -> bool {
        matches!(
            self,
            DownloadTaskStatus::Canceled
                | DownloadTaskStatus::Succeeded
                | DownloadTaskStatus::Failed
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestedStop {
    Pause,
    Cancel,
}

#[derive(Debug, Clone)]
struct DownloadTaskConfig {
    task_id: String,
    app_data_dir: PathBuf,
    user_cache_dir: PathBuf,
    owner: user_cache::DownloadOwner,
    request: user_cache::UserCacheDownloadRequest,
    user_agent: Option<String>,
    behavior: DownloadBehavior,
    output_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct DownloadCompletion {
    path: PathBuf,
    mime_type: Option<String>,
    size: u64,
}

struct DownloadIteratorState {
    receiver: Option<mpsc::Receiver<DownloadIteratorMessage>>,
    sender: mpsc::Sender<DownloadIteratorMessage>,
    pending_message: Option<DownloadIteratorMessage>,
    terminal_seen: bool,
    fallback_progress: f64,
    status: DownloadTaskStatus,
    stop_requested: Option<RequestedStop>,
    config: DownloadTaskConfig,
    completion: Option<oneshot::Sender<DownloadCompletionOutcome>>,
}

impl DownloadIteratorState {
    fn new(
        receiver: mpsc::Receiver<DownloadIteratorMessage>,
        sender: mpsc::Sender<DownloadIteratorMessage>,
        config: DownloadTaskConfig,
        completion: oneshot::Sender<DownloadCompletionOutcome>,
    ) -> Self {
        Self {
            receiver: Some(receiver),
            sender,
            pending_message: None,
            terminal_seen: false,
            fallback_progress: 0.0,
            status: DownloadTaskStatus::Running,
            stop_requested: None,
            config,
            completion: Some(completion),
        }
    }
}

enum DownloadCompletionOutcome {
    Success(DownloadCompletion),
    Failed(String),
    Canceled,
}

#[derive(Debug, Clone)]
enum DownloadIteratorMessage {
    Progress {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Paused,
    Resumed,
    Canceled,
    Success(DownloadCompletion),
    Error(String),
}

#[derive(Default)]
struct DownloadIteratorRegistry {
    seq: AtomicU64,
}

impl JSRuntimeService for DownloadIteratorRegistry {}

fn download_registry(ctx: &JSContext) -> &DownloadIteratorRegistry {
    ctx.runtime()
        .get_or_init_service::<DownloadIteratorRegistry>()
}

fn next_download_task_id(ctx: &JSContext) -> String {
    let seq = download_registry(ctx).seq.fetch_add(1, Ordering::Relaxed) + 1;
    format!("download_{seq}")
}

fn progress_value(
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    fallback_progress: &mut f64,
) -> Option<f64> {
    if let Some(total) = total_bytes
        && total > 0
    {
        let precise = ((downloaded_bytes as f64) / (total as f64)).clamp(0.0, 1.0);
        *fallback_progress = precise;
        return Some(precise);
    }

    *fallback_progress = (downloaded_bytes as f64).max(*fallback_progress);
    None
}

fn install_async_iterator(ctx: &JSContext, iterator: &JSObject) -> JSResult<()> {
    let symbol = ctx
        .global()
        .get::<_, JSObject>("Symbol")?
        .get::<_, JSSymbol>("asyncIterator")?;
    iterator.set(
        symbol,
        JSFunc::new(ctx, move |this: rong::function::This<JSObject>| {
            (*this).clone()
        })?,
    )?;
    Ok(())
}

fn bind_abort_signal_to_iterator(
    ctx: &JSContext,
    signal: Option<JSObject>,
    iterator: &JSObject,
) -> JSResult<()> {
    let Some(signal) = signal else {
        return Ok(());
    };

    let cancel_target = iterator.clone();
    let cancel_fn = JSFunc::new(ctx, move || -> JSResult<()> {
        if let Ok(cancel) = cancel_target.get::<_, JSFunc>("cancel") {
            let _ = cancel.call::<_, JSObject>(Some(cancel_target.clone()), ());
        }
        Ok(())
    })?;

    if signal.get::<_, bool>("aborted").unwrap_or(false) {
        cancel_fn.call::<_, ()>(None, ())?;
        return Ok(());
    }

    let add_event_listener = signal
        .get::<_, JSFunc>("addEventListener")
        .map_err(|_| js_invalid_parameter_error("downloadFile signal must be an AbortSignal"))?;
    let listener_opts = JSObject::new(ctx);
    listener_opts.set("once", true)?;
    add_event_listener.call::<_, ()>(Some(signal), ("abort", cancel_fn, listener_opts))?;
    Ok(())
}

fn path_to_result_string(lxapp: &LxApp, path: &Path) -> String {
    lxapp
        .to_uri(path)
        .map(|value| value.into_string())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn to_js_download_result(
    ctx: &JSContext,
    result: &DownloadCompletion,
) -> JSResult<JSDownloadResult> {
    let lxapp = LxApp::from_ctx(ctx)?;
    let path = path_to_result_string(&lxapp, &result.path);
    Ok(JSDownloadResult {
        file_path: path,
        mime_type: result.mime_type.clone(),
        size: result.size,
    })
}

fn simple_event(kind: &str) -> JSDownloadEvent {
    JSDownloadEvent {
        kind: kind.to_string(),
        downloaded_bytes: None,
        total_bytes: None,
        progress: None,
        result: None,
    }
}

fn js_abort_error(detail: impl AsRef<str>) -> rong::RongJSError {
    HostError::new(rong::error::E_ABORT, detail.as_ref()).into()
}

fn get_present_property(obj: &JSObject, field: &str) -> Option<JSValue> {
    obj.get::<_, JSValue>(field)
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
}

fn read_required_string_field(obj: &JSObject, field: &str, api_name: &str) -> JSResult<String> {
    let value = get_present_property(obj, field).ok_or_else(|| {
        js_error_from_business_code_with_detail(1002, format!("{api_name} requires {field}"))
    })?;
    if !value.is_string() {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} {field} must be a string"
        )));
    }
    value
        .to_rust::<String>()
        .map_err(|_| js_invalid_parameter_error(format!("{api_name} {field} must be a string")))
}

fn read_optional_string_field(
    obj: &JSObject,
    field: &str,
    api_name: &str,
) -> JSResult<Option<String>> {
    let Some(value) = get_present_property(obj, field) else {
        return Ok(None);
    };
    if !value.is_string() {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} {field} must be a string"
        )));
    }
    value
        .to_rust::<String>()
        .map(Some)
        .map_err(|_| js_invalid_parameter_error(format!("{api_name} {field} must be a string")))
}

fn read_optional_timeout_field(obj: &JSObject) -> JSResult<Option<u64>> {
    let Some(value) = get_present_property(obj, "timeout") else {
        return Ok(None);
    };
    if !value.is_number() {
        return Err(js_invalid_parameter_error(
            "downloadFile timeout must be a positive number",
        ));
    }
    let timeout = value.to_rust::<f64>().map_err(|_| {
        js_invalid_parameter_error("downloadFile timeout must be a positive number")
    })?;
    if !timeout.is_finite() || timeout <= 0.0 {
        return Err(js_invalid_parameter_error(
            "downloadFile timeout must be a positive number",
        ));
    }
    Ok(Some(timeout.round() as u64))
}

fn read_optional_signal(obj: &JSObject) -> JSResult<Option<JSObject>> {
    let Some(value) = get_present_property(obj, "signal") else {
        return Ok(None);
    };
    value
        .into_object()
        .map(Some)
        .ok_or_else(|| js_invalid_parameter_error("downloadFile signal must be an AbortSignal"))
}

fn read_header_entries(obj: &JSObject, field: &str) -> JSResult<Vec<(String, String)>> {
    let Some(value) = get_present_property(obj, field) else {
        return Ok(Vec::new());
    };
    let Some(header_obj) = value.into_object() else {
        return Err(js_invalid_parameter_error(format!(
            "downloadFile {field} must be an object"
        )));
    };
    let mut headers = Vec::new();
    for entry in header_obj.entries()? {
        let (key, value) = entry.try_into::<String, JSValue>()?;
        if value.is_undefined() || value.is_null() {
            continue;
        }
        if !value.is_string() {
            return Err(js_invalid_parameter_error(format!(
                "downloadFile {field}.{key} must be a string"
            )));
        }
        let parsed = value.to_rust::<String>().map_err(|_| {
            js_invalid_parameter_error(format!("downloadFile {field}.{key} must be a string"))
        })?;
        headers.push((key, parsed));
    }
    Ok(headers)
}

fn parse_download_options(options: JSValue) -> JSResult<ParsedDownloadOptions> {
    let Some(obj) = options.into_object() else {
        return Err(js_invalid_parameter_error(
            "downloadFile expects an options object",
        ));
    };
    Ok(ParsedDownloadOptions {
        url: read_required_string_field(&obj, "url", "downloadFile")?,
        headers: read_header_entries(&obj, "headers")?,
        timeout_ms: read_optional_timeout_field(&obj)?,
        file_path: read_optional_string_field(&obj, "filePath", "downloadFile")?,
        signal: read_optional_signal(&obj)?,
    })
}

fn resolve_output_path(lxapp: &LxApp, file_path: Option<&str>) -> JSResult<Option<PathBuf>> {
    let Some(file_path) = file_path.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    let path = Path::new(file_path);
    if file_path.starts_with("lx://") || path.is_absolute() || file_path.contains(':') {
        return lxapp
            .resolve_accessible_path(file_path)
            .map(Some)
            .map_err(|err| js_error_from_lxapp_error(&err));
    }

    if file_path.split('/').any(|segment| segment == "..") {
        return Err(js_invalid_parameter_error(
            "downloadFile filePath must not contain '..'",
        ));
    }

    Ok(Some(
        lxapp.user_data_dir.join(file_path.trim_start_matches('/')),
    ))
}

async fn finalize_download_result(
    output_path: Option<&PathBuf>,
    result: user_cache::UserCacheDownloadResult,
) -> Result<DownloadCompletion, String> {
    let Some(output_path) = output_path else {
        return Ok(DownloadCompletion {
            path: result.temp_path,
            mime_type: result.mime_type,
            size: result.size,
        });
    };

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create output dir failed: {e}"))?;
    }
    if result.temp_path != *output_path {
        std::fs::copy(&result.temp_path, output_path)
            .map_err(|e| format!("copy download to filePath failed: {e}"))?;
    }

    Ok(DownloadCompletion {
        path: output_path.clone(),
        mime_type: result.mime_type,
        size: result.size,
    })
}

async fn spawn_download_worker(state: Arc<Mutex<DownloadIteratorState>>) {
    let (mut progress_tx, config) = {
        let guard = state.lock().await;
        if guard.status.is_terminal() {
            return;
        }
        (guard.sender.clone(), guard.config.clone())
    };

    let _ = rong::RongExecutor::global().spawn(async move {
        let persistence = user_cache::DownloadPersistence::new(
            config.app_data_dir.clone(),
            config.task_id.clone(),
            config.owner.clone(),
            true,
        );
        let mut event_tx = progress_tx.clone();
        let on_event = move |event| match event {
            TransferDownloadEvent::Started {
                resumed_bytes,
                total_bytes,
                ..
            } => {
                let _ = event_tx.try_send(DownloadIteratorMessage::Progress {
                    downloaded_bytes: resumed_bytes,
                    total_bytes,
                });
            }
            TransferDownloadEvent::Progress {
                downloaded_bytes,
                total_bytes,
                ..
            } => {
                let _ = event_tx.try_send(DownloadIteratorMessage::Progress {
                    downloaded_bytes,
                    total_bytes,
                });
            }
            _ => {}
        };

        let download_result = if let Some(output_path) = config.output_path.clone() {
            user_cache::download_to_path_with_behavior(
                Some(persistence),
                output_path,
                config.request.clone(),
                config.user_agent.clone(),
                config.behavior,
                on_event,
            )
            .await
        } else {
            user_cache::download_to_user_cache_with_behavior(
                Some(persistence),
                &config.user_cache_dir,
                config.request.clone(),
                config.user_agent.clone(),
                config.behavior,
                on_event,
            )
            .await
        };

        let result: Result<DownloadCompletion, String> = match download_result {
            Ok(success) => finalize_download_result(config.output_path.as_ref(), success).await,
            Err(error) => Err(error.error),
        };

        match result {
            Ok(success) => {
                let completion = {
                    let mut guard = state.lock().await;
                    guard.stop_requested = None;
                    if guard.status.is_terminal() {
                        return;
                    }
                    guard.status = DownloadTaskStatus::Succeeded;
                    guard.completion.take()
                };
                if let Some(completion) = completion {
                    let _ = completion.send(DownloadCompletionOutcome::Success(success.clone()));
                }
                let _ = progress_tx
                    .send(DownloadIteratorMessage::Success(success))
                    .await;
            }
            Err(error) => {
                let (message, completion, pause_event) = {
                    let mut guard = state.lock().await;
                    match guard.stop_requested {
                        Some(RequestedStop::Pause) => {
                            guard.stop_requested = None;
                            guard.status = DownloadTaskStatus::Paused;
                            (None, None, Some(DownloadIteratorMessage::Paused))
                        }
                        Some(RequestedStop::Cancel) | None
                            if guard.status == DownloadTaskStatus::Canceled =>
                        {
                            guard.stop_requested = None;
                            return;
                        }
                        _ => {
                            guard.stop_requested = None;
                            guard.status = DownloadTaskStatus::Failed;
                            (Some(error.clone()), guard.completion.take(), None)
                        }
                    }
                };

                if let Some(pause_event) = pause_event {
                    let _ = progress_tx.send(pause_event).await;
                    return;
                }

                let Some(message) = message else {
                    return;
                };
                if let Some(completion) = completion {
                    let _ = completion.send(DownloadCompletionOutcome::Failed(message.clone()));
                }
                let _ = progress_tx
                    .send(DownloadIteratorMessage::Error(message))
                    .await;
            }
        }
    });
}

fn install_promise_methods(ctx: &JSContext, iterator: &JSObject, promise: Promise) -> JSResult<()> {
    let then_promise = promise.clone();
    let then_ctx = ctx.clone();
    iterator.set(
        "then",
        JSFunc::new(
            ctx,
            move |on_fulfilled: Optional<JSValue>,
                  on_rejected: Optional<JSValue>|
                  -> JSResult<JSObject> {
                let then = then_promise.then()?;
                then.call(
                    Some(then_promise.clone().into_object()),
                    (
                        on_fulfilled
                            .0
                            .unwrap_or_else(|| JSValue::undefined(&then_ctx)),
                        on_rejected
                            .0
                            .unwrap_or_else(|| JSValue::undefined(&then_ctx)),
                    ),
                )
            },
        )?,
    )?;

    let catch_promise = promise.clone();
    let catch_ctx = ctx.clone();
    iterator.set(
        "catch",
        JSFunc::new(
            ctx,
            move |on_rejected: Optional<JSValue>| -> JSResult<JSObject> {
                let catch_fn = catch_promise.catch()?;
                catch_fn.call(
                    Some(catch_promise.clone().into_object()),
                    (on_rejected
                        .0
                        .unwrap_or_else(|| JSValue::undefined(&catch_ctx)),),
                )
            },
        )?,
    )?;

    let finally_promise = promise.clone();
    let finally_ctx = ctx.clone();
    iterator.set(
        "finally",
        JSFunc::new(
            ctx,
            move |on_finally: Optional<JSValue>| -> JSResult<JSObject> {
                let finally_fn = finally_promise.get::<_, JSFunc>("finally")?;
                finally_fn.call(
                    Some(finally_promise.clone().into_object()),
                    (on_finally
                        .0
                        .unwrap_or_else(|| JSValue::undefined(&finally_ctx)),),
                )
            },
        )?,
    )?;

    let wait_promise = promise;
    iterator.set("wait", JSFunc::new(ctx, move || wait_promise.clone())?)?;
    Ok(())
}

async fn download_file(ctx: JSContext, options: JSValue) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let options = parse_download_options(options)?;
    let url = options.url.trim().to_string();
    if url.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "downloadFile requires url",
        ));
    }

    let task_id = next_download_task_id(&ctx);
    let mut behavior = DownloadBehavior::default();
    if let Some(timeout_ms) = options.timeout_ms {
        behavior.request_timeout = Duration::from_millis(timeout_ms);
    }

    let output_path = resolve_output_path(&lxapp, options.file_path.as_deref())?;
    let (tx, rx) = mpsc::channel::<DownloadIteratorMessage>(64);
    let (completion_tx, completion_rx) = oneshot::channel::<DownloadCompletionOutcome>();
    let promise_ctx = ctx.clone();
    let final_promise = Promise::from_future(&ctx, None, async move {
        match completion_rx.await {
            Ok(DownloadCompletionOutcome::Success(result)) => {
                to_js_download_result(&promise_ctx, &result)
            }
            Ok(DownloadCompletionOutcome::Failed(error)) => {
                Err(js_internal_error(format!("download failed: {error}")))
            }
            Ok(DownloadCompletionOutcome::Canceled) => Err(js_abort_error("downloadFile canceled")),
            Err(_) => Err(js_abort_error("downloadFile canceled")),
        }
    })?;

    let state = Arc::new(Mutex::new(DownloadIteratorState::new(
        rx,
        tx.clone(),
        DownloadTaskConfig {
            task_id: task_id.clone(),
            app_data_dir: lxapp.app_data_dir(),
            user_cache_dir: lxapp.user_cache_dir.clone(),
            owner: user_cache::DownloadOwner {
                kind: user_cache::DownloadOwnerKind::LxApp,
                appid: lxapp.appid.clone(),
                page_path: None,
                tab_id: None,
            },
            request: user_cache::UserCacheDownloadRequest {
                url,
                headers: options.headers,
            },
            user_agent: Some(rong::get_user_agent()),
            behavior,
            output_path,
        },
        completion_tx,
    )));

    let iterator = JSObject::new(&ctx);

    let next_state = state.clone();
    iterator.set(
        "next",
        JSFunc::new(&ctx, move |ctx: JSContext| {
            let state = next_state.clone();
            async move { download_next_step(&ctx, &state).await }
        })?,
    )?;

    let return_state = state.clone();
    iterator.set(
        "return",
        JSFunc::new(&ctx, move || {
            let state = return_state.clone();
            async move {
                download_cancel_task(&state).await?;
                Ok(JSDownloadIteratorStep {
                    done: true,
                    value: None,
                })
            }
        })?,
    )?;

    let pause_state = state.clone();
    iterator.set(
        "pause",
        JSFunc::new(&ctx, move || {
            let state = pause_state.clone();
            async move { download_pause_task(&state).await }
        })?,
    )?;

    let resume_state = state.clone();
    iterator.set(
        "resume",
        JSFunc::new(&ctx, move || {
            let state = resume_state.clone();
            async move { download_resume_task(&state).await }
        })?,
    )?;

    let cancel_state = state.clone();
    let cancel_fn = JSFunc::new(&ctx, move || {
        let state = cancel_state.clone();
        async move { download_cancel_task(&state).await }
    })?;
    iterator.set("cancel", cancel_fn.clone())?;
    iterator.set("abort", cancel_fn)?;

    install_promise_methods(&ctx, &iterator, final_promise)?;
    install_async_iterator(&ctx, &iterator)?;
    bind_abort_signal_to_iterator(&ctx, options.signal, &iterator)?;

    spawn_download_worker(state.clone()).await;

    Ok(iterator)
}

async fn download_next_step(
    ctx: &JSContext,
    state: &Arc<Mutex<DownloadIteratorState>>,
) -> JSResult<JSDownloadIteratorStep> {
    let mut receiver = {
        let mut state_guard = state.lock().await;
        if let Some(message) = state_guard.pending_message.take() {
            drop(state_guard);
            return handle_download_message(ctx, state, message).await;
        }
        if state_guard.terminal_seen {
            return Ok(JSDownloadIteratorStep {
                done: true,
                value: None,
            });
        }

        state_guard
            .receiver
            .take()
            .ok_or_else(|| js_internal_error("download iterator receiver unexpectedly missing"))?
    };

    let message = receiver.next().await;

    let mut state_guard = state.lock().await;
    state_guard.receiver = Some(receiver);
    drop(state_guard);

    match message {
        Some(message) => handle_download_message(ctx, state, message).await,
        None => {
            let mut state_guard = state.lock().await;
            state_guard.terminal_seen = true;
            Ok(JSDownloadIteratorStep {
                done: true,
                value: None,
            })
        }
    }
}

async fn handle_download_message(
    ctx: &JSContext,
    state: &Arc<Mutex<DownloadIteratorState>>,
    message: DownloadIteratorMessage,
) -> JSResult<JSDownloadIteratorStep> {
    let mut state_guard = state.lock().await;

    match message {
        DownloadIteratorMessage::Progress {
            downloaded_bytes,
            total_bytes,
        } => {
            let progress = progress_value(
                downloaded_bytes,
                total_bytes,
                &mut state_guard.fallback_progress,
            );
            Ok(JSDownloadIteratorStep {
                done: false,
                value: Some(JSDownloadEvent {
                    kind: "progress".to_string(),
                    downloaded_bytes: Some(downloaded_bytes),
                    total_bytes,
                    progress,
                    result: None,
                }),
            })
        }
        DownloadIteratorMessage::Paused => Ok(JSDownloadIteratorStep {
            done: false,
            value: Some(simple_event("paused")),
        }),
        DownloadIteratorMessage::Resumed => Ok(JSDownloadIteratorStep {
            done: false,
            value: Some(simple_event("resumed")),
        }),
        DownloadIteratorMessage::Canceled => {
            state_guard.status = DownloadTaskStatus::Canceled;
            state_guard.terminal_seen = true;
            Ok(JSDownloadIteratorStep {
                done: false,
                value: Some(simple_event("canceled")),
            })
        }
        DownloadIteratorMessage::Success(result) => {
            state_guard.status = DownloadTaskStatus::Succeeded;
            state_guard.terminal_seen = true;
            let js_result = to_js_download_result(ctx, &result)?;
            Ok(JSDownloadIteratorStep {
                done: false,
                value: Some(JSDownloadEvent {
                    kind: "success".to_string(),
                    downloaded_bytes: Some(result.size),
                    total_bytes: Some(result.size),
                    progress: Some(1.0),
                    result: Some(js_result),
                }),
            })
        }
        DownloadIteratorMessage::Error(message) => {
            state_guard.status = DownloadTaskStatus::Failed;
            state_guard.terminal_seen = true;
            Err(js_internal_error(format!("download failed: {message}")))
        }
    }
}

async fn download_pause_task(state: &Arc<Mutex<DownloadIteratorState>>) -> JSResult<()> {
    let (app_data_dir, task_id) = {
        let mut guard = state.lock().await;
        if guard.status != DownloadTaskStatus::Running {
            return Ok(());
        }
        guard.stop_requested = Some(RequestedStop::Pause);
        (
            guard.config.app_data_dir.clone(),
            guard.config.task_id.clone(),
        )
    };

    match lingxia_transfer::pause(&app_data_dir, &task_id) {
        Ok(()) => Ok(()),
        Err(err) => {
            let mut guard = state.lock().await;
            guard.stop_requested = None;
            Err(js_internal_error(format!("download pause failed: {err}")))
        }
    }
}

async fn download_resume_task(state: &Arc<Mutex<DownloadIteratorState>>) -> JSResult<()> {
    {
        let mut guard = state.lock().await;
        if guard.status.is_terminal() || guard.status != DownloadTaskStatus::Paused {
            return Ok(());
        }
        guard.stop_requested = None;
        guard.status = DownloadTaskStatus::Running;
        if guard
            .sender
            .try_send(DownloadIteratorMessage::Resumed)
            .is_err()
        {
            guard.pending_message = Some(DownloadIteratorMessage::Resumed);
        }
    }

    spawn_download_worker(state.clone()).await;

    Ok(())
}

async fn download_cancel_task(state: &Arc<Mutex<DownloadIteratorState>>) -> JSResult<()> {
    let (app_data_dir, task_id) = {
        let mut guard = state.lock().await;
        if guard.status.is_terminal() {
            return Ok(());
        }
        guard.stop_requested = Some(RequestedStop::Cancel);
        (
            guard.config.app_data_dir.clone(),
            guard.config.task_id.clone(),
        )
    };

    match lingxia_transfer::cancel(&app_data_dir, &task_id) {
        Ok(()) => {
            let completion = {
                let mut guard = state.lock().await;
                guard.status = DownloadTaskStatus::Canceled;
                guard.terminal_seen = false;
                if guard
                    .sender
                    .try_send(DownloadIteratorMessage::Canceled)
                    .is_err()
                {
                    guard.pending_message = Some(DownloadIteratorMessage::Canceled);
                }
                guard.completion.take()
            };
            if let Some(completion) = completion {
                let _ = completion.send(DownloadCompletionOutcome::Canceled);
            }
            Ok(())
        }
        Err(err) => {
            let mut guard = state.lock().await;
            guard.stop_requested = None;
            Err(js_internal_error(format!("download cancel failed: {err}")))
        }
    }
}

pub(super) fn init(ctx: &JSContext) -> JSResult<()> {
    let download_file_func = JSFunc::new(ctx, |ctx, options| async move {
        download_file(ctx, options).await
    })?;
    lx::register_js_api(ctx, "downloadFile", download_file_func)?;
    Ok(())
}
