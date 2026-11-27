use super::download::options_string;
use super::{network_security, normalize_relative_path};
use crate::i18n::{
    js_error, js_error_from_business_code_with_detail, js_internal_error,
    js_invalid_parameter_error, js_resource_not_found_error, server_error_status,
};
use futures::channel::{mpsc, oneshot};
use futures::lock::Mutex;
use futures::{SinkExt, StreamExt};
use lingxia_transfer::{
    UploadBehavior, UploadBodyMode, UploadEvent as TransferUploadEvent, UploadFailure,
    UploadFailureKind, UploadMethod, UploadRequest, resolve_upload_file_name,
    upload_file_with_behavior,
};
use lxapp::LxApp;
use rong::{HostError, IntoJSObject, JSContext, JSFunc, JSObject, JSResult, JSValue, Promise};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::oneshot as tokio_oneshot;

#[derive(Debug, Clone)]
struct ParsedUploadOptions {
    url: String,
    file_path: String,
    method: UploadMethod,
    body_mode: UploadBodyMode,
    /// `None` when the lxapp left `field` unset, so a raw body can reject an
    /// explicit multipart field name instead of silently dropping it.
    field_name: Option<String>,
    headers: Vec<(String, String)>,
    form_data: Vec<(String, String)>,
    file_name: Option<String>,
    mime_type: Option<String>,
    signal: Option<JSObject>,
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct JSUploadEvent {
    kind: String,
    #[js_name = "uploadedBytes"]
    uploaded_bytes: Option<u64>,
    #[js_name = "totalBytes"]
    total_bytes: Option<u64>,
    progress: Option<f64>,
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
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
    headers: Vec<(String, String)>,
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
    HostError::new(rong::error::E_ABORT, detail.as_ref())
        .with_name("AbortError")
        .into()
}

fn upload_failure_to_js_error(error: UploadFailure) -> rong::RongJSError {
    match error.kind {
        UploadFailureKind::Timeout => js_error_from_business_code_with_detail(5002, error.error),
        UploadFailureKind::NetworkUnavailable => {
            js_error_from_business_code_with_detail(5001, error.error)
        }
        UploadFailureKind::Server => server_error_status(error.http_status(), &error.error),
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

fn get_present_property(obj: &JSObject, field: &str) -> Option<JSValue> {
    obj.get::<_, JSValue>(field)
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
}

fn read_required_string_field(obj: &JSObject, field: &str, _api_name: &str) -> JSResult<String> {
    let value = get_present_property(obj, field).ok_or_else(|| {
        rong::RongJSError::from(rong::HostError::missing_field(field)).at_argument(2)
    })?;
    options_string(value, &[field])
}

fn read_optional_string_field(
    obj: &JSObject,
    field: &str,
    _api_name: &str,
) -> JSResult<Option<String>> {
    get_present_property(obj, field)
        .map(|value| options_string(value, &[field]))
        .transpose()
}

fn read_optional_signal(obj: &JSObject) -> JSResult<Option<JSObject>> {
    let Some(value) = get_present_property(obj, "signal") else {
        return Ok(None);
    };
    value
        .into_object()
        .map(Some)
        .ok_or_else(|| js_invalid_parameter_error("network.upload signal must be an AbortSignal"))
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
        let parsed = options_string(value, &[field, &key])?;
        entries.push((key, parsed));
    }
    Ok(entries)
}

fn parse_upload_options(url: JSValue, options: JSValue) -> JSResult<ParsedUploadOptions> {
    let url = super::download::read_url_argument(&url, "network.upload")?;
    let Some(obj) = options.into_object() else {
        return Err(js_invalid_parameter_error(
            "network.upload expects an options object with file",
        ));
    };
    let body_mode = read_upload_body_mode_field(&obj)?;
    let field_name = read_optional_string_field(&obj, "field", "network.upload")?;
    let form_data = read_string_map(&obj, "fields", "network.upload")?;
    let file_name = read_optional_string_field(&obj, "fileName", "network.upload")?;
    // A raw body has nowhere to carry multipart framing, so reject it rather
    // than dropping fields the lxapp believes it sent. An empty fields
    // object is still an explicit multipart field and is rejected too.
    if body_mode == UploadBodyMode::Raw {
        if get_present_property(&obj, "fields").is_some() {
            return Err(js_invalid_parameter_error(
                "network.upload fields requires bodyMode \"multipart\"",
            ));
        }
        if field_name.is_some() {
            return Err(js_invalid_parameter_error(
                "network.upload field requires bodyMode \"multipart\"",
            ));
        }
        if file_name.is_some() {
            return Err(js_invalid_parameter_error(
                "network.upload fileName requires bodyMode \"multipart\"",
            ));
        }
    } else {
        if get_present_property(&obj, "fields").is_some() && form_data.is_empty() {
            return Err(js_invalid_parameter_error(
                "network.upload fields must include at least one field",
            ));
        }
        if file_name
            .as_deref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return Err(js_invalid_parameter_error(
                "network.upload fileName must be a non-empty string",
            ));
        }
    }
    Ok(ParsedUploadOptions {
        url,
        file_path: read_required_string_field(&obj, "path", "network.upload")?,
        method: read_upload_method_field(&obj)?,
        body_mode,
        field_name,
        headers: read_string_map(&obj, "headers", "network.upload")?,
        form_data,
        file_name,
        mime_type: read_optional_string_field(&obj, "type", "network.upload")?,
        signal: read_optional_signal(&obj)?,
    })
}

fn read_upload_method_field(obj: &JSObject) -> JSResult<UploadMethod> {
    let Some(method) = read_optional_string_field(obj, "method", "network.upload")? else {
        return Ok(UploadMethod::Post);
    };
    match method.trim().to_ascii_uppercase().as_str() {
        "POST" => Ok(UploadMethod::Post),
        "PUT" => Ok(UploadMethod::Put),
        "PATCH" => Ok(UploadMethod::Patch),
        _ => Err(js_invalid_parameter_error(
            "network.upload method must be \"POST\", \"PUT\", or \"PATCH\"",
        )),
    }
}

fn read_upload_body_mode_field(obj: &JSObject) -> JSResult<UploadBodyMode> {
    let Some(body_mode) = read_optional_string_field(obj, "bodyMode", "network.upload")? else {
        return Ok(UploadBodyMode::Multipart);
    };
    match body_mode.trim().to_ascii_lowercase().as_str() {
        "multipart" => Ok(UploadBodyMode::Multipart),
        "raw" => Ok(UploadBodyMode::Raw),
        _ => Err(js_invalid_parameter_error(
            "network.upload bodyMode must be \"multipart\" or \"raw\"",
        )),
    }
}

fn resolve_upload_path(
    user_data_dir: &Path,
    file_path: &str,
    resolve_accessible_path: impl FnOnce(&str) -> JSResult<PathBuf>,
) -> JSResult<PathBuf> {
    let trimmed = file_path.trim();
    if trimmed.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "network.upload requires file",
        ));
    }

    let path = Path::new(trimmed);
    let resolved = if trimmed.starts_with("lx://") || path.is_absolute() || trimmed.contains(':') {
        resolve_accessible_path(trimmed)?
    } else {
        let relative = normalize_relative_path(trimmed, "network.upload", "path")?;
        user_data_dir.join(relative)
    };

    if !resolved.exists() {
        return Err(js_resource_not_found_error(
            "network.upload file does not exist",
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
                        headers: success.headers.clone(),
                        data: data.clone(),
                    });
                    guard.completion.take()
                };
                let upload = UploadCompletion {
                    status_code: success.status_code,
                    headers: success.headers,
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

/// `{ status, ok, headers, text, json() }`: any HTTP answer resolves, so a 500
/// is `ok: false` with its body readable.
fn upload_result_to_js(ctx: &JSContext, result: UploadCompletion) -> JSResult<JSObject> {
    let object = JSObject::new(ctx);
    object.set("status", result.status_code)?;
    object.set("ok", (200..300).contains(&result.status_code))?;
    let headers = JSObject::new(ctx);
    for (name, value) in result.headers {
        headers.set(name.to_ascii_lowercase().as_str(), value)?;
    }
    object.set("headers", headers)?;
    object.set("text", result.data.clone())?;
    let text = result.data;
    object.set(
        "json",
        JSFunc::new(ctx, move |ctx: JSContext| -> JSResult<JSValue> {
            let parse: JSFunc = ctx.global().get::<_, JSObject>("JSON")?.get("parse")?;
            parse.call(None, (text.clone(),))
        })?
        .name("json")?,
    )?;
    Ok(object)
}

/// Upload a file over HTTP, streamed from disk. `method: 'PUT'` with
/// `bodyMode: 'raw'` sends the file as the whole body, as presigned
/// object-storage URLs expect. The task comes back before the transfer starts;
/// iterate it for progress, await `result` for the answer.
///
/// @default A `POST` with a `multipart/form-data` body.
///
/// @throws {E_INVALID_ARG} A URL that is not absolute http(s), an unreadable file, or bad headers.
/// @throws {E_NOT_FOUND} No file at `path`.
/// @throws {E_ABORT} `cancel()` was called or `signal` was aborted (on `result`).
/// @throws {E_NETWORK} The server answered with an error status (`reason` `'serverErrorStatus'`, the status in `data.status`) or did not answer (`'connectionFailed'`), on `result`.
/// @throws {E_TIMEOUT} The server stopped answering (on `result`).
fn upload_file(ctx: JSContext, url: JSValue, options: JSValue) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let options = parse_upload_options(url, options)?;
    crate::task_object::validate_abort_signal(options.signal.as_ref())?;
    let url = options.url.trim().to_string();
    if url.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "network.upload requires url",
        ));
    }

    let resolved_path = resolve_upload_path(&lxapp.user_data_dir, &options.file_path, |path| {
        lxapp.resolve_accessible_path(path).map_err(js_error)
    })?;
    let file_name = resolve_upload_file_name(&resolved_path, options.file_name.as_deref());
    let behavior = UploadBehavior {
        resolve_http_errors: true,
        ..UploadBehavior::default()
    };

    let (tx, rx) = mpsc::channel::<UploadIteratorMessage>(64);
    let (completion_tx, completion_rx) = oneshot::channel::<UploadCompletionOutcome>();

    let result_ctx = ctx.clone();
    let final_promise = Promise::from_future(&ctx, None, async move {
        match completion_rx.await {
            Ok(UploadCompletionOutcome::Success(result)) => {
                upload_result_to_js(&result_ctx, result)
            }
            Ok(UploadCompletionOutcome::Failed(error)) => Err(upload_failure_to_js_error(error)),
            Ok(UploadCompletionOutcome::Canceled) => Err(js_abort_error("network.upload canceled")),
            Err(_) => Err(js_abort_error("network.upload canceled")),
        }
    })?;

    let state = Arc::new(Mutex::new(UploadIteratorState::new(
        rx,
        tx.clone(),
        UploadTaskConfig {
            lxapp: lxapp.clone(),
            request: UploadRequest {
                url,
                method: options.method,
                body_mode: options.body_mode,
                file_path: resolved_path,
                field_name: options.field_name.unwrap_or_else(|| "file".to_string()),
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

    let task = crate::task_object::create_task(
        &ctx,
        iterator,
        final_promise,
        &["cancel"],
        crate::task_object::byte_events("uploadedBytes"),
    )?;
    crate::task_object::bind_abort_signal(&ctx, options.signal, &task)?;

    spawn_upload_worker(state);
    Ok(task)
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

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

#[cfg(test)]
mod tests {
    use super::{parse_upload_options, resolve_upload_path};
    use lingxia_transfer::{UploadBodyMode, UploadMethod};
    use rong::{JSEngine, JSObject, JSValue};

    /// A bare JS context, enough to build the options object `uploadFile`
    /// parses. The runtime is returned with it because the context borrows it.
    fn js_context() -> (rong::JSRuntime, rong::JSContext) {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        (runtime, ctx)
    }

    /// Options carrying the file every upload needs, plus `extra`.
    fn options(ctx: &rong::JSContext, extra: &[(&str, JSValue)]) -> JSValue {
        let obj = JSObject::new(ctx);
        obj.set("path", "clip.mp4").unwrap();
        for (key, value) in extra {
            obj.set(*key, value.clone()).unwrap();
        }
        JSValue::from_raw(ctx, obj.into_value())
    }

    fn url(ctx: &rong::JSContext) -> JSValue {
        JSValue::from_rust(ctx, "https://example.com/upload")
    }

    fn string(ctx: &rong::JSContext, value: &str) -> JSValue {
        JSValue::from_rust(ctx, value)
    }

    fn number(ctx: &rong::JSContext, value: f64) -> JSValue {
        JSValue::from_rust(ctx, value)
    }

    /// A field of the wrong type is refused like an argument: `E_INVALID_ARG` at argument 2, with
    /// the path inside the options.
    #[test]
    fn a_wrong_option_field_is_refused_with_its_argument_and_path() {
        let (_runtime, ctx) = js_context();
        let located = |error: rong::RongJSError| {
            (
                error.code().unwrap_or_default().to_string(),
                format!(
                    "{:?}",
                    error.as_host_error().and_then(|host| host.data.clone())
                ),
            )
        };
        let (code, data) = located(
            parse_upload_options(url(&ctx), options(&ctx, &[("type", number(&ctx, 5.0))]))
                .unwrap_err(),
        );
        assert_eq!(code, rong::error::E_INVALID_ARG);
        assert!(
            data.contains("argument") && data.contains("\"type\""),
            "{data}"
        );

        let headers = JSObject::new(&ctx);
        headers.set("accept", 1).unwrap();
        let headers = JSValue::from_raw(&ctx, headers.into_value());
        let (code, data) = located(
            parse_upload_options(url(&ctx), options(&ctx, &[("headers", headers)])).unwrap_err(),
        );
        assert_eq!(code, rong::error::E_INVALID_ARG);
        assert!(data.contains("headers.accept"), "{data}");

        let bare = JSValue::from_raw(&ctx, JSObject::new(&ctx).into_value());
        let (code, data) = located(parse_upload_options(url(&ctx), bare).unwrap_err());
        assert_eq!(code, rong::error::E_INVALID_ARG);
        assert!(data.contains("\"path\""), "{data}");
    }

    #[test]
    fn upload_method_defaults_to_post_and_reads_any_case() {
        let (_runtime, ctx) = js_context();

        let parsed = parse_upload_options(url(&ctx), options(&ctx, &[])).unwrap();
        assert_eq!(parsed.method, UploadMethod::Post);

        for (input, expected) in [
            ("PUT", UploadMethod::Put),
            ("put", UploadMethod::Put),
            ("  Patch ", UploadMethod::Patch),
            ("post", UploadMethod::Post),
        ] {
            let value = string(&ctx, input);
            let parsed =
                parse_upload_options(url(&ctx), options(&ctx, &[("method", value)])).unwrap();
            assert_eq!(parsed.method, expected, "method {input}");
        }
    }

    #[test]
    fn upload_method_rejects_verbs_no_upload_can_carry_a_body_for() {
        let (_runtime, ctx) = js_context();

        for input in ["DELETE", "GET", ""] {
            let value = string(&ctx, input);
            assert!(
                parse_upload_options(url(&ctx), options(&ctx, &[("method", value)])).is_err(),
                "method {input} should be rejected"
            );
        }

        // A non-string reaches the same rejection, not a silent default.
        let value = number(&ctx, 5.0);
        assert!(parse_upload_options(url(&ctx), options(&ctx, &[("method", value)])).is_err());
    }

    #[test]
    fn body_mode_defaults_to_multipart_and_reads_any_case() {
        let (_runtime, ctx) = js_context();

        let parsed = parse_upload_options(url(&ctx), options(&ctx, &[])).unwrap();
        assert_eq!(parsed.body_mode, UploadBodyMode::Multipart);

        for (input, expected) in [
            ("raw", UploadBodyMode::Raw),
            ("RAW", UploadBodyMode::Raw),
            (" Multipart ", UploadBodyMode::Multipart),
        ] {
            let value = string(&ctx, input);
            let parsed =
                parse_upload_options(url(&ctx), options(&ctx, &[("bodyMode", value)])).unwrap();
            assert_eq!(parsed.body_mode, expected, "bodyMode {input}");
        }

        let value = string(&ctx, "binary");
        assert!(parse_upload_options(url(&ctx), options(&ctx, &[("bodyMode", value)])).is_err());
    }

    #[test]
    fn raw_rejects_the_multipart_only_options_it_cannot_carry() {
        let (_runtime, ctx) = js_context();
        let raw = || string(&ctx, "raw");

        let form_data = JSObject::new(&ctx);
        form_data.set("note", "spec").unwrap();
        let form_data = JSValue::from_raw(&ctx, form_data.into_value());
        assert!(
            parse_upload_options(
                url(&ctx),
                options(&ctx, &[("bodyMode", raw()), ("fields", form_data.clone())])
            )
            .is_err(),
            "fields has nowhere to go in an unframed body"
        );

        let name = string(&ctx, "asset");
        assert!(
            parse_upload_options(
                url(&ctx),
                options(&ctx, &[("bodyMode", raw()), ("field", name)])
            )
            .is_err(),
            "a field name has nowhere to go in an unframed body"
        );

        // Populated fields stays legal under the multipart default, and raw
        // alone is fine.
        assert!(parse_upload_options(url(&ctx), options(&ctx, &[("fields", form_data)])).is_ok());
        let parsed =
            parse_upload_options(url(&ctx), options(&ctx, &[("bodyMode", raw())])).unwrap();
        assert_eq!(parsed.body_mode, UploadBodyMode::Raw);
        assert_eq!(parsed.field_name, None);
    }

    #[test]
    fn empty_form_data_is_rejected_in_every_body_mode() {
        let (_runtime, ctx) = js_context();

        // `{}` and a map of nulls both mean "no fields". Passing the key is
        // still an explicit fields value, so both modes reject it.
        let empty = JSObject::new(&ctx);
        let empty = JSValue::from_raw(&ctx, empty.into_value());
        let nulls = JSObject::new(&ctx);
        nulls.set("note", JSValue::null(&ctx)).unwrap();
        let nulls = JSValue::from_raw(&ctx, nulls.into_value());

        for form_data in [empty, nulls] {
            let raw = string(&ctx, "raw");
            assert!(
                parse_upload_options(
                    url(&ctx),
                    options(&ctx, &[("bodyMode", raw), ("fields", form_data.clone())])
                )
                .is_err()
            );
            assert!(
                parse_upload_options(url(&ctx), options(&ctx, &[("fields", form_data)])).is_err()
            );
        }
    }

    #[test]
    fn raw_rejects_file_name() {
        let (_runtime, ctx) = js_context();
        let raw = string(&ctx, "raw");
        let file_name = string(&ctx, "clip.mp4");
        assert!(
            parse_upload_options(
                url(&ctx),
                options(&ctx, &[("bodyMode", raw), ("fileName", file_name)])
            )
            .is_err()
        );
    }

    #[test]
    fn string_maps_reject_shapes_that_cannot_become_headers() {
        let (_runtime, ctx) = js_context();

        let not_a_map = string(&ctx, "x-token: dev");
        assert!(parse_upload_options(url(&ctx), options(&ctx, &[("headers", not_a_map)])).is_err());

        let numeric_value = JSObject::new(&ctx);
        numeric_value.set("x-retry", 3).unwrap();
        let numeric_value = JSValue::from_raw(&ctx, numeric_value.into_value());
        assert!(
            parse_upload_options(url(&ctx), options(&ctx, &[("headers", numeric_value)])).is_err()
        );

        // A null entry is absent, not an error -- callers build these maps
        // conditionally.
        let with_null = JSObject::new(&ctx);
        with_null.set("x-token", "dev").unwrap();
        with_null.set("x-absent", JSValue::null(&ctx)).unwrap();
        let with_null = JSValue::from_raw(&ctx, with_null.into_value());
        let parsed =
            parse_upload_options(url(&ctx), options(&ctx, &[("headers", with_null)])).unwrap();
        assert_eq!(
            parsed.headers,
            vec![("x-token".to_string(), "dev".to_string())]
        );
    }

    #[test]
    fn url_comes_first_and_file_is_required() {
        let (_runtime, ctx) = js_context();
        let empty = || JSValue::from_raw(&ctx, JSObject::new(&ctx).into_value());

        assert!(parse_upload_options(url(&ctx), empty()).is_err());
        assert!(parse_upload_options(number(&ctx, 42.0), options(&ctx, &[])).is_err());
        // The old single-object shape is refused with a pointer to the new one.
        assert!(parse_upload_options(options(&ctx, &[]), empty()).is_err());
        assert!(parse_upload_options(url(&ctx), string(&ctx, "clip.mp4")).is_err());
    }

    #[test]
    fn upload_path_rejects_userdata_escape_and_accepts_nested_file() {
        let root = tempfile::tempdir().unwrap();
        let user_data_dir = root.path().join("userdata");
        let nested_file = user_data_dir.join("media/video.mp4");
        std::fs::create_dir_all(nested_file.parent().unwrap()).unwrap();
        std::fs::write(&nested_file, b"video").unwrap();
        std::fs::write(root.path().join("secret"), b"secret").unwrap();

        assert!(resolve_upload_path(&user_data_dir, "../secret", |_| unreachable!()).is_err());
        assert_eq!(
            resolve_upload_path(&user_data_dir, "media/video.mp4", |_| unreachable!()).unwrap(),
            nested_file
        );
    }
}

rong::js_api! {
    fn register_api(ctx) {
        namespace NetworkApi = crate::namespace::network(ctx)?;
        fn upload(
            ts_params = "url: string | URL, options: UploadOptions",
            ts_return = "UploadTask"
        ) = upload_file;
    }
}
