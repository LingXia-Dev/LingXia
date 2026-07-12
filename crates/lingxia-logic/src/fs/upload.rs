use super::network_security;
use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error, js_internal_error,
    js_invalid_parameter_error,
};
use futures::channel::{mpsc, oneshot};
use futures::lock::Mutex;
use futures::{SinkExt, StreamExt};
use lingxia_transfer::{
    UploadBehavior, UploadEvent as TransferUploadEvent, UploadFailure, UploadFailureKind,
    UploadMethod, UploadRequest, resolve_upload_file_name, upload_file_with_behavior,
};
use lxapp::{LxApp, lx};
use rong::{
    HostError, IntoJSObject, JSContext, JSFunc, JSObject, JSResult, JSValue, Promise,
    function::Optional,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot as tokio_oneshot;

#[derive(Debug, Clone)]
struct ParsedUploadOptions {
    url: String,
    file_path: String,
    field_name: String,
    headers: Vec<(String, String)>,
    form_data: Vec<(String, String)>,
    timeout_ms: Option<u64>,
    file_name: Option<String>,
    mime_type: Option<String>,
    signal: Option<JSObject>,
}

#[derive(Debug, Clone, IntoJSObject)]
struct JSUploadResult {
    #[js_name = "statusCode"]
    status_code: u16,
    data: String,
}

#[derive(Debug, Clone, IntoJSObject)]
struct JSUploadEvent {
    kind: String,
    #[js_name = "uploadedBytes"]
    uploaded_bytes: Option<u64>,
    #[js_name = "totalBytes"]
    total_bytes: Option<u64>,
    progress: Option<f64>,
    result: Option<JSUploadResult>,
}

#[derive(Debug, Clone, IntoJSObject)]
struct JSUploadIteratorStep {
    done: bool,
    value: Option<JSUploadEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UploadTaskStatus {
    Running,
    Canceled,
    Succeeded,
    Failed,
}

impl UploadTaskStatus {
    fn is_terminal(self) -> bool {
        matches!(
            self,
            UploadTaskStatus::Canceled | UploadTaskStatus::Succeeded | UploadTaskStatus::Failed
        )
    }
}

#[derive(Clone)]
struct UploadTaskConfig {
    lxapp: Arc<LxApp>,
    request: UploadRequest,
    behavior: UploadBehavior,
}

#[derive(Debug, Clone)]
struct UploadCompletion {
    status_code: u16,
    data: String,
}

struct UploadIteratorState {
    receiver: Option<mpsc::Receiver<UploadIteratorMessage>>,
    sender: mpsc::Sender<UploadIteratorMessage>,
    pending_message: Option<UploadIteratorMessage>,
    terminal_seen: bool,
    iteration_closed: bool,
    fallback_progress: f64,
    status: UploadTaskStatus,
    uploaded_bytes: u64,
    total_bytes: Option<u64>,
    config: UploadTaskConfig,
    result: Option<UploadCompletion>,
    completion: Option<oneshot::Sender<UploadCompletionOutcome>>,
    abort_tx: Option<tokio_oneshot::Sender<()>>,
}

impl UploadIteratorState {
    fn new(
        receiver: mpsc::Receiver<UploadIteratorMessage>,
        sender: mpsc::Sender<UploadIteratorMessage>,
        config: UploadTaskConfig,
        completion: oneshot::Sender<UploadCompletionOutcome>,
    ) -> Self {
        Self {
            receiver: Some(receiver),
            sender,
            pending_message: None,
            terminal_seen: false,
            iteration_closed: false,
            fallback_progress: 0.0,
            status: UploadTaskStatus::Running,
            uploaded_bytes: 0,
            total_bytes: None,
            config,
            result: None,
            completion: Some(completion),
            abort_tx: None,
        }
    }
}

enum UploadCompletionOutcome {
    Success(UploadCompletion),
    Failed(UploadFailure),
    Canceled,
}

#[derive(Debug, Clone)]
enum UploadIteratorMessage {
    Started {
        uploaded_bytes: u64,
        total_bytes: u64,
    },
    Progress {
        uploaded_bytes: u64,
        total_bytes: u64,
    },
    Canceled,
    Success(UploadCompletion),
    Error(UploadFailure),
}

fn js_abort_error(detail: impl AsRef<str>) -> rong::RongJSError {
    HostError::new(rong::error::E_ABORT, detail.as_ref()).into()
}

fn upload_failure_to_js_error(error: UploadFailure) -> rong::RongJSError {
    match error.kind {
        UploadFailureKind::Timeout => js_error_from_business_code_with_detail(5002, error.error),
        UploadFailureKind::NetworkUnavailable => {
            js_error_from_business_code_with_detail(5001, error.error)
        }
        UploadFailureKind::Server => js_error_from_business_code_with_detail(5003, error.error),
        UploadFailureKind::Connection => js_error_from_business_code_with_detail(5004, error.error),
        UploadFailureKind::AccessDenied => {
            js_error_from_business_code_with_detail(3000, error.error)
        }
        UploadFailureKind::Canceled => js_abort_error(error.error),
        UploadFailureKind::InvalidRequest | UploadFailureKind::InvalidFile => {
            js_invalid_parameter_error(error.error)
        }
        UploadFailureKind::Internal => js_internal_error(format!("upload failed: {}", error.error)),
    }
}

fn progress_value(
    uploaded_bytes: u64,
    total_bytes: u64,
    fallback_progress: &mut f64,
) -> Option<f64> {
    if total_bytes > 0 {
        let precise = ((uploaded_bytes as f64) / (total_bytes as f64)).clamp(0.0, 1.0);
        *fallback_progress = precise;
        return Some(precise);
    }
    *fallback_progress = (uploaded_bytes as f64).max(*fallback_progress);
    None
}

fn install_async_iterator(ctx: &JSContext, iterator: &JSObject) -> JSResult<()> {
    let symbol = ctx
        .global()
        .get::<_, JSObject>("Symbol")?
        .get::<_, rong::JSSymbol>("asyncIterator")?;
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

    let target = iterator.clone();
    let cancel_fn = JSFunc::new(ctx, move || -> JSResult<()> {
        if let Ok(cancel) = target.get::<_, JSFunc>("cancel") {
            let _ = cancel.call::<_, JSObject>(Some(target.clone()), ());
        }
        Ok(())
    })?;

    if signal.get::<_, bool>("aborted").unwrap_or(false) {
        cancel_fn.call::<_, ()>(None, ())?;
        return Ok(());
    }

    let add_event_listener = signal
        .get::<_, JSFunc>("addEventListener")
        .map_err(|_| js_invalid_parameter_error("uploadFile signal must be an AbortSignal"))?;
    let listener_opts = JSObject::new(ctx);
    listener_opts.set("once", true)?;
    add_event_listener.call::<_, ()>(Some(signal), ("abort", cancel_fn, listener_opts))?;
    Ok(())
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
            "uploadFile timeout must be a positive number",
        ));
    }
    let timeout = value
        .to_rust::<f64>()
        .map_err(|_| js_invalid_parameter_error("uploadFile timeout must be a positive number"))?;
    if !timeout.is_finite() || timeout <= 0.0 {
        return Err(js_invalid_parameter_error(
            "uploadFile timeout must be a positive number",
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
        .ok_or_else(|| js_invalid_parameter_error("uploadFile signal must be an AbortSignal"))
}

fn read_string_map(obj: &JSObject, field: &str, api_name: &str) -> JSResult<Vec<(String, String)>> {
    let Some(value) = get_present_property(obj, field) else {
        return Ok(Vec::new());
    };
    let Some(map_obj) = value.into_object() else {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} {field} must be an object"
        )));
    };

    let mut entries = Vec::new();
    for entry in map_obj.entries()? {
        let (key, value) = entry.try_into::<String, JSValue>()?;
        if value.is_undefined() || value.is_null() {
            continue;
        }
        if !value.is_string() {
            return Err(js_invalid_parameter_error(format!(
                "{api_name} {field}.{key} must be a string"
            )));
        }
        let parsed = value.to_rust::<String>().map_err(|_| {
            js_invalid_parameter_error(format!("{api_name} {field}.{key} must be a string"))
        })?;
        entries.push((key, parsed));
    }
    Ok(entries)
}

fn parse_upload_options(options: JSValue) -> JSResult<ParsedUploadOptions> {
    let Some(obj) = options.into_object() else {
        return Err(js_invalid_parameter_error(
            "uploadFile expects an options object",
        ));
    };
    Ok(ParsedUploadOptions {
        url: read_required_string_field(&obj, "url", "uploadFile")?,
        file_path: read_required_string_field(&obj, "filePath", "uploadFile")?,
        field_name: read_optional_string_field(&obj, "name", "uploadFile")?
            .unwrap_or_else(|| "file".to_string()),
        headers: read_string_map(&obj, "headers", "uploadFile")?,
        form_data: read_string_map(&obj, "formData", "uploadFile")?,
        timeout_ms: read_optional_timeout_field(&obj)?,
        file_name: read_optional_string_field(&obj, "fileName", "uploadFile")?,
        mime_type: read_optional_string_field(&obj, "mimeType", "uploadFile")?,
        signal: read_optional_signal(&obj)?,
    })
}

fn resolve_upload_path(lxapp: &LxApp, file_path: &str) -> JSResult<PathBuf> {
    let trimmed = file_path.trim();
    if trimmed.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "uploadFile requires filePath",
        ));
    }

    let path = Path::new(trimmed);
    let resolved = if trimmed.starts_with("lx://") || path.is_absolute() || trimmed.contains(':') {
        lxapp
            .resolve_accessible_path(trimmed)
            .map_err(|err| js_error_from_lxapp_error(&err))?
    } else {
        lxapp.user_data_dir.join(trimmed.trim_start_matches('/'))
    };

    if !resolved.exists() {
        return Err(js_invalid_parameter_error(
            "uploadFile filePath does not exist",
        ));
    }
    Ok(resolved)
}

fn spawn_upload_worker(state: Arc<Mutex<UploadIteratorState>>) {
    std::mem::drop(rong::RongExecutor::global().spawn(async move {
        let (mut progress_tx, config, abort_rx) = {
            let mut guard = state.lock().await;
            if guard.status.is_terminal() || guard.status != UploadTaskStatus::Running {
                return;
            }
            let (abort_tx, abort_rx) = tokio_oneshot::channel::<()>();
            guard.abort_tx = Some(abort_tx);
            (guard.sender.clone(), guard.config.clone(), abort_rx)
        };

        let mut event_tx = progress_tx.clone();
        let result = network_security::scope_lxapp_network_access(
            config.lxapp.clone(),
            upload_file_with_behavior(
                config.request.clone(),
                config.behavior,
                abort_rx,
                move |event| match event {
                    TransferUploadEvent::Started {
                        uploaded_bytes,
                        total_bytes,
                        ..
                    } => {
                        let _ = event_tx.try_send(UploadIteratorMessage::Started {
                            uploaded_bytes,
                            total_bytes,
                        });
                    }
                    TransferUploadEvent::Progress {
                        uploaded_bytes,
                        total_bytes,
                        ..
                    } => {
                        let _ = event_tx.try_send(UploadIteratorMessage::Progress {
                            uploaded_bytes,
                            total_bytes,
                        });
                    }
                    _ => {}
                },
            ),
        )
        .await;

        match result {
            Ok(success) => {
                let data = String::from_utf8_lossy(&success.body).into_owned();
                let completion = {
                    let mut guard = state.lock().await;
                    if guard.status.is_terminal() {
                        return;
                    }
                    guard.abort_tx = None;
                    guard.status = UploadTaskStatus::Succeeded;
                    guard.uploaded_bytes = guard.total_bytes.unwrap_or(guard.uploaded_bytes);
                    guard.result = Some(UploadCompletion {
                        status_code: success.status_code,
                        data: data.clone(),
                    });
                    guard.completion.take()
                };
                let upload = UploadCompletion {
                    status_code: success.status_code,
                    data,
                };
                if let Some(completion) = completion {
                    let _ = completion.send(UploadCompletionOutcome::Success(upload.clone()));
                }
                let _ = progress_tx
                    .send(UploadIteratorMessage::Success(upload))
                    .await;
            }
            Err(error) if error.error == "aborted" || error.error == "Upload canceled" => {
                let completion = {
                    let mut guard = state.lock().await;
                    guard.abort_tx = None;
                    guard.status = UploadTaskStatus::Canceled;
                    guard.completion.take()
                };
                if let Some(completion) = completion {
                    let _ = completion.send(UploadCompletionOutcome::Canceled);
                }
                let _ = progress_tx.send(UploadIteratorMessage::Canceled).await;
            }
            Err(error) => {
                let completion = {
                    let mut guard = state.lock().await;
                    if guard.status.is_terminal() {
                        return;
                    }
                    guard.abort_tx = None;
                    guard.status = UploadTaskStatus::Failed;
                    guard.completion.take()
                };
                if let Some(completion) = completion {
                    let _ = completion.send(UploadCompletionOutcome::Failed(error.clone()));
                }
                let _ = progress_tx.send(UploadIteratorMessage::Error(error)).await;
            }
        }
    }));
}

fn upload_file(ctx: JSContext, options: JSValue) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let options = parse_upload_options(options)?;
    let url = options.url.trim().to_string();
    if url.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "uploadFile requires url",
        ));
    }

    let resolved_path = resolve_upload_path(&lxapp, &options.file_path)?;
    let file_name = resolve_upload_file_name(&resolved_path, options.file_name.as_deref());
    let mut behavior = UploadBehavior::default();
    if let Some(timeout_ms) = options.timeout_ms {
        behavior.request_timeout = Duration::from_millis(timeout_ms);
    }

    let (tx, rx) = mpsc::channel::<UploadIteratorMessage>(64);
    let (completion_tx, completion_rx) = oneshot::channel::<UploadCompletionOutcome>();

    let final_promise = Promise::from_future(&ctx, None, async move {
        match completion_rx.await {
            Ok(UploadCompletionOutcome::Success(result)) => Ok(JSUploadResult {
                status_code: result.status_code,
                data: result.data,
            }),
            Ok(UploadCompletionOutcome::Failed(error)) => Err(upload_failure_to_js_error(error)),
            Ok(UploadCompletionOutcome::Canceled) => Err(js_abort_error("uploadFile canceled")),
            Err(_) => Err(js_abort_error("uploadFile canceled")),
        }
    })?;

    let state = Arc::new(Mutex::new(UploadIteratorState::new(
        rx,
        tx.clone(),
        UploadTaskConfig {
            lxapp: lxapp.clone(),
            request: UploadRequest {
                url,
                method: UploadMethod::Post,
                file_path: resolved_path,
                field_name: options.field_name,
                file_name: Some(file_name),
                mime_type: options.mime_type,
                headers: options.headers,
                form_fields: options.form_data,
                user_agent: Some(rong::get_user_agent()),
            },
            behavior,
        },
        completion_tx,
    )));

    let iterator = JSObject::new(&ctx);

    let next_state = state.clone();
    iterator.set(
        "next",
        JSFunc::new(&ctx, move |ctx: JSContext| {
            let state = next_state.clone();
            async move { upload_next_step(&ctx, &state).await }
        })?,
    )?;

    let return_state = state.clone();
    iterator.set(
        "return",
        JSFunc::new(&ctx, move || {
            let state = return_state.clone();
            async move {
                let mut guard = state.lock().await;
                guard.iteration_closed = true;
                guard.pending_message = None;
                guard.receiver = None;
                Ok(JSUploadIteratorStep {
                    done: true,
                    value: None,
                })
            }
        })?,
    )?;

    let cancel_state = state.clone();
    iterator.set(
        "cancel",
        JSFunc::new(&ctx, move || {
            let state = cancel_state.clone();
            async move { upload_cancel_task(&state).await }
        })?,
    )?;

    install_promise_methods(&ctx, &iterator, final_promise)?;
    install_async_iterator(&ctx, &iterator)?;
    bind_abort_signal_to_iterator(&ctx, options.signal, &iterator)?;

    spawn_upload_worker(state);
    Ok(iterator)
}

async fn upload_next_step(
    _ctx: &JSContext,
    state: &Arc<Mutex<UploadIteratorState>>,
) -> JSResult<JSUploadIteratorStep> {
    let mut receiver = {
        let mut state_guard = state.lock().await;
        if let Some(message) = state_guard.pending_message.take() {
            drop(state_guard);
            return handle_upload_message(state, message).await;
        }
        if state_guard.terminal_seen || state_guard.iteration_closed {
            return Ok(JSUploadIteratorStep {
                done: true,
                value: None,
            });
        }
        state_guard
            .receiver
            .take()
            .ok_or_else(|| js_internal_error("upload iterator receiver unexpectedly missing"))?
    };

    let message = receiver.next().await;
    let mut state_guard = state.lock().await;
    state_guard.receiver = Some(receiver);
    drop(state_guard);

    match message {
        Some(message) => handle_upload_message(state, message).await,
        None => {
            let mut state_guard = state.lock().await;
            state_guard.terminal_seen = true;
            Ok(JSUploadIteratorStep {
                done: true,
                value: None,
            })
        }
    }
}

async fn handle_upload_message(
    state: &Arc<Mutex<UploadIteratorState>>,
    message: UploadIteratorMessage,
) -> JSResult<JSUploadIteratorStep> {
    let mut state_guard = state.lock().await;
    match message {
        UploadIteratorMessage::Started {
            uploaded_bytes,
            total_bytes,
        } => {
            state_guard.uploaded_bytes = uploaded_bytes;
            state_guard.total_bytes = Some(total_bytes);
            let progress = progress_value(
                uploaded_bytes,
                total_bytes,
                &mut state_guard.fallback_progress,
            );
            Ok(JSUploadIteratorStep {
                done: false,
                value: Some(JSUploadEvent {
                    kind: "progress".to_string(),
                    uploaded_bytes: Some(uploaded_bytes),
                    total_bytes: Some(total_bytes),
                    progress,
                    result: None,
                }),
            })
        }
        UploadIteratorMessage::Progress {
            uploaded_bytes,
            total_bytes,
        } => {
            state_guard.uploaded_bytes = uploaded_bytes;
            state_guard.total_bytes = Some(total_bytes);
            let progress = progress_value(
                uploaded_bytes,
                total_bytes,
                &mut state_guard.fallback_progress,
            );
            Ok(JSUploadIteratorStep {
                done: false,
                value: Some(JSUploadEvent {
                    kind: "progress".to_string(),
                    uploaded_bytes: Some(uploaded_bytes),
                    total_bytes: Some(total_bytes),
                    progress,
                    result: None,
                }),
            })
        }
        UploadIteratorMessage::Canceled => {
            state_guard.status = UploadTaskStatus::Canceled;
            state_guard.terminal_seen = true;
            Ok(JSUploadIteratorStep {
                done: false,
                value: Some(JSUploadEvent {
                    kind: "canceled".to_string(),
                    uploaded_bytes: Some(state_guard.uploaded_bytes),
                    total_bytes: state_guard.total_bytes,
                    progress: state_guard.total_bytes.and_then(|total| {
                        progress_value(
                            state_guard.uploaded_bytes,
                            total,
                            &mut state_guard.fallback_progress,
                        )
                    }),
                    result: None,
                }),
            })
        }
        UploadIteratorMessage::Success(result) => {
            state_guard.status = UploadTaskStatus::Succeeded;
            state_guard.terminal_seen = true;
            state_guard.result = Some(result.clone());
            Ok(JSUploadIteratorStep {
                done: false,
                value: Some(JSUploadEvent {
                    kind: "completed".to_string(),
                    uploaded_bytes: Some(state_guard.uploaded_bytes),
                    total_bytes: state_guard.total_bytes,
                    progress: Some(1.0),
                    result: Some(JSUploadResult {
                        status_code: result.status_code,
                        data: result.data,
                    }),
                }),
            })
        }
        UploadIteratorMessage::Error(error) => {
            state_guard.status = UploadTaskStatus::Failed;
            state_guard.terminal_seen = true;
            Err(upload_failure_to_js_error(error))
        }
    }
}

async fn upload_cancel_task(state: &Arc<Mutex<UploadIteratorState>>) -> JSResult<()> {
    let completion = {
        let mut guard = state.lock().await;
        if guard.status.is_terminal() {
            return Ok(());
        }
        if let Some(abort_tx) = guard.abort_tx.take() {
            let _ = abort_tx.send(());
        }
        guard.status = UploadTaskStatus::Canceled;
        guard.terminal_seen = false;
        if guard
            .sender
            .try_send(UploadIteratorMessage::Canceled)
            .is_err()
        {
            guard.pending_message = Some(UploadIteratorMessage::Canceled);
        }
        guard.completion.take()
    };
    if let Some(completion) = completion {
        let _ = completion.send(UploadCompletionOutcome::Canceled);
    }
    Ok(())
}

pub(super) fn init(ctx: &JSContext) -> JSResult<()> {
    let upload_file_func = JSFunc::new(ctx, upload_file)?;
    lx::register_js_api(ctx, "uploadFile", upload_file_func)?;
    Ok(())
}
