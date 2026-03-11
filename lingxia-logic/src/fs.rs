use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error,
    js_error_from_platform_error, js_internal_error,
};
use futures::channel::mpsc;
use futures::future::{AbortHandle, Abortable};
use futures::lock::Mutex;
use futures::{SinkExt, StreamExt};
use lingxia_platform::traits::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogFilter, FileInteraction,
    OpenDocumentRequest,
};
use lxapp::{LxApp, lx};
use rong::{
    FromJSObj, IntoJSObj, JSContext, JSFunc, JSObject, JSResult, JSRuntimeService, Source,
    function::Optional,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const UNKNOWN_TOTAL_PROGRESS_CURVE_BYTES: f64 = 4.0 * 1024.0 * 1024.0;

const DOWNLOAD_ASYNC_ITERATOR_SHIM: &str = r#"
(function () {
  const lx = globalThis.lx;
  if (!lx || typeof lx.downloadFile !== 'function') return;
  const rawDownloadFile = lx.downloadFile;

  function ensureAsyncIterable(task) {
    if (!task || typeof task.next !== 'function') {
      throw new TypeError('downloadFile runtime returned invalid task');
    }
    if (typeof task[Symbol.asyncIterator] !== 'function') {
      task[Symbol.asyncIterator] = function () {
        return this;
      };
    }
    return task;
  }

  function normalizeDownloadTask(taskOrPromise) {
    if (taskOrPromise && typeof taskOrPromise.then === 'function') {
      const ready = Promise.resolve(taskOrPromise).then(ensureAsyncIterable);
      return {
        async next() {
          const task = await ready;
          return task.next();
        },
        async return(value) {
          const task = await ready;
          if (typeof task.return === 'function') {
            return task.return(value);
          }
          return { done: true, value };
        },
        async pause() {
          const task = await ready;
          if (typeof task.pause === 'function') {
            return task.pause();
          }
        },
        async resume() {
          const task = await ready;
          if (typeof task.resume === 'function') {
            return task.resume();
          }
        },
        async cancel() {
          const task = await ready;
          if (typeof task.cancel === 'function') {
            return task.cancel();
          }
        },
        [Symbol.asyncIterator]() {
          return this;
        },
      };
    }
    return ensureAsyncIterable(taskOrPromise);
  }

  lx.downloadFile = function downloadFile(options) {
    const iterator = normalizeDownloadTask(rawDownloadFile(options));
    const signal = options && typeof options === 'object' ? options.signal : undefined;
    if (signal && typeof signal.addEventListener === 'function') {
      const onAbort = function () {
        if (iterator && typeof iterator.cancel === 'function') {
          void iterator.cancel();
        }
      };
      if (signal.aborted) {
        onAbort();
      } else {
        signal.addEventListener('abort', onAbort, { once: true });
      }
    }
    return iterator;
  };
})();
"#;

#[derive(FromJSObj)]
struct JSOpenDocumentOptions {
    #[rename = "filePath"]
    file_path: String,
    #[rename = "fileType"]
    file_type: Option<String>,
    /// Controls share/menu button visibility.
    #[rename = "showMenu"]
    show_menu: Option<bool>,
}

fn map_file_type_to_mime(file_type: Option<String>) -> Option<String> {
    match file_type.unwrap_or_default().to_lowercase().as_str() {
        "pdf" => Some("application/pdf".to_string()),
        "doc" => Some("application/msword".to_string()),
        "docx" => Some(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string(),
        ),
        "ppt" => Some("application/vnd.ms-powerpoint".to_string()),
        "pptx" => Some(
            "application/vnd.openxmlformats-officedocument.presentationml.presentation".to_string(),
        ),
        "xls" => Some("application/vnd.ms-excel".to_string()),
        "xlsx" => {
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string())
        }
        "zip" => Some("application/zip".to_string()),
        _ => None,
    }
}

async fn open_document(ctx: JSContext, options: JSOpenDocumentOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    if options.file_path.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "openDocument requires filePath",
        ));
    }

    let resolved_path = lxapp
        .resolve_accessible_path(&options.file_path)
        .map_err(|err| js_error_from_lxapp_error(&err))?;

    lxapp
        .runtime
        .open_document(OpenDocumentRequest {
            file_path: resolved_path.to_string_lossy().into_owned(),
            mime_type: map_file_type_to_mime(options.file_type),
            show_menu: options.show_menu,
        })
        .await
        .map_err(|e| js_error_from_platform_error(&e))
}

#[derive(FromJSObj, Clone, Default)]
struct JSFileDialogFilter {
    name: Option<String>,
    extensions: Option<Vec<String>>,
}

#[derive(FromJSObj, Clone, Default)]
struct JSChooseFileOptions {
    multiple: Option<bool>,
    filters: Option<Vec<JSFileDialogFilter>>,
    title: Option<String>,
    #[rename = "defaultPath"]
    default_path: Option<String>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct ChooseFileResultObj {
    canceled: bool,
    paths: Vec<String>,
}

#[derive(FromJSObj, Clone, Default)]
struct JSChooseDirectoryOptions {
    title: Option<String>,
    #[rename = "defaultPath"]
    default_path: Option<String>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct ChooseDirectoryResultObj {
    canceled: bool,
    path: Option<String>,
}

#[derive(FromJSObj)]
struct JSDownloadOptions {
    url: String,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSDownloadResult {
    #[rename = "tempFilePath"]
    temp_file_path: String,
    #[rename = "fileName"]
    file_name: String,
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
    Terminal,
}

#[derive(Debug, Clone)]
struct DownloadTaskConfig {
    user_cache_dir: PathBuf,
    request: lxapp::download_manager::UserCacheDownloadRequest,
    user_agent: Option<String>,
}

struct DownloadIteratorState {
    receiver: Option<mpsc::Receiver<DownloadIteratorMessage>>,
    sender: mpsc::Sender<DownloadIteratorMessage>,
    terminal_seen: bool,
    abort_handle: Option<AbortHandle>,
    fallback_progress: f64,
    status: DownloadTaskStatus,
    config: DownloadTaskConfig,
}

impl DownloadIteratorState {
    fn new(
        receiver: mpsc::Receiver<DownloadIteratorMessage>,
        sender: mpsc::Sender<DownloadIteratorMessage>,
        config: DownloadTaskConfig,
    ) -> Self {
        Self {
            receiver: Some(receiver),
            sender,
            terminal_seen: false,
            abort_handle: None,
            fallback_progress: 0.0,
            status: DownloadTaskStatus::Paused,
            config,
        }
    }
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
    Success(lxapp::download_manager::UserCacheDownloadResult),
    Error(String),
}

#[derive(Default)]
struct DownloadIteratorRegistry {
    seq: AtomicU64,
    streams: Mutex<HashMap<String, Arc<Mutex<DownloadIteratorState>>>>,
}

impl JSRuntimeService for DownloadIteratorRegistry {}

fn normalize_extensions(raw: Option<Vec<String>>) -> Vec<String> {
    raw.unwrap_or_default()
        .into_iter()
        .map(|ext| ext.trim().trim_start_matches('.').to_lowercase())
        .filter(|ext| !ext.is_empty())
        .collect()
}

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
) -> f64 {
    if let Some(total) = total_bytes {
        if total > 0 {
            let precise = ((downloaded_bytes as f64) / (total as f64)).clamp(0.0, 1.0);
            *fallback_progress = precise;
            return precise;
        }
    }
    let estimated = 1.0 - (-(downloaded_bytes as f64) / UNKNOWN_TOTAL_PROGRESS_CURVE_BYTES).exp();
    let next = estimated.max(*fallback_progress).min(0.95);
    *fallback_progress = next;
    next
}

fn to_js_download_result(
    ctx: &JSContext,
    result: &lxapp::download_manager::UserCacheDownloadResult,
) -> JSResult<JSDownloadResult> {
    let lxapp = LxApp::from_ctx(ctx)?;
    let temp_file_path = lxapp
        .to_uri(&result.temp_path)
        .ok_or_else(|| js_internal_error("download failed to convert output path to lx:// uri"))?
        .into_string();

    Ok(JSDownloadResult {
        temp_file_path,
        file_name: result.file_name.clone(),
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

async fn spawn_download_worker(state: Arc<Mutex<DownloadIteratorState>>) -> Result<(), String> {
    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let (mut progress_tx, config) = {
        let mut guard = state.lock().await;
        if guard.status == DownloadTaskStatus::Terminal {
            return Err("download task already terminated".to_string());
        }
        guard.status = DownloadTaskStatus::Running;
        guard.abort_handle = Some(abort_handle);
        (guard.sender.clone(), guard.config.clone())
    };

    rong::bg::spawn(async move {
        let run = async move {
            let mut terminal_tx = progress_tx.clone();
            let result = lxapp::download_manager::download_to_user_cache(
                &config.user_cache_dir,
                config.request,
                config.user_agent,
                move |event| {
                    if let lxapp::download_manager::DownloadEvent::Progress {
                        downloaded_bytes,
                        total_bytes,
                        ..
                    } = event
                    {
                        let _ = progress_tx.try_send(DownloadIteratorMessage::Progress {
                            downloaded_bytes,
                            total_bytes,
                        });
                    }
                },
            )
            .await;

            match result {
                Ok(success) => {
                    let _ = terminal_tx
                        .send(DownloadIteratorMessage::Success(success))
                        .await;
                }
                Err(failure) => {
                    let _ = terminal_tx
                        .send(DownloadIteratorMessage::Error(failure.error))
                        .await;
                }
            }
        };

        let _ = Abortable::new(run, abort_registration).await;
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn get_download_state(
    ctx: &JSContext,
    task_id: &str,
) -> Option<Arc<Mutex<DownloadIteratorState>>> {
    let streams = download_registry(ctx).streams.lock().await;
    streams.get(task_id).cloned()
}

async fn choose_file(
    ctx: JSContext,
    options: Optional<JSChooseFileOptions>,
) -> JSResult<ChooseFileResultObj> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let opts = options.as_ref().cloned().unwrap_or_default();
    let filters = opts
        .filters
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            let extensions = normalize_extensions(item.extensions);
            if extensions.is_empty() {
                return None;
            }
            Some(FileDialogFilter {
                name: item.name,
                extensions,
            })
        })
        .collect();
    let result = lxapp
        .runtime
        .choose_file(ChooseFileRequest {
            multiple: opts.multiple.unwrap_or(false),
            filters,
            title: opts.title,
            default_path: opts.default_path,
        })
        .await
        .map_err(|e| js_error_from_platform_error(&e))?;

    if !result.canceled && result.paths.is_empty() {
        return Err(js_internal_error(
            "chooseFile invalid payload: non-canceled result must include at least one path",
        ));
    }

    Ok(ChooseFileResultObj {
        canceled: result.canceled,
        paths: result.paths,
    })
}

async fn choose_directory(
    ctx: JSContext,
    options: Optional<JSChooseDirectoryOptions>,
) -> JSResult<ChooseDirectoryResultObj> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let opts = options.as_ref().cloned().unwrap_or_default();
    let result = lxapp
        .runtime
        .choose_directory(ChooseDirectoryRequest {
            title: opts.title,
            default_path: opts.default_path,
        })
        .await
        .map_err(|e| js_error_from_platform_error(&e))?;

    if !result.canceled && result.paths.len() != 1 {
        return Err(js_internal_error(
            "chooseDirectory invalid payload: non-canceled result must include exactly one path",
        ));
    }
    let path = result.paths.into_iter().next();

    Ok(ChooseDirectoryResultObj {
        canceled: result.canceled,
        path,
    })
}

async fn download_file(ctx: JSContext, options: JSDownloadOptions) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let url = options.url.trim().to_string();
    if url.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "downloadFile requires url",
        ));
    }

    let task_id = next_download_task_id(&ctx);
    let (tx, rx) = mpsc::channel::<DownloadIteratorMessage>(64);
    let config = DownloadTaskConfig {
        user_cache_dir: lxapp.user_cache_dir.clone(),
        user_agent: Some(rong::get_user_agent()),
        request: lxapp::download_manager::UserCacheDownloadRequest {
            url,
            headers: Vec::new(),
        },
    };
    let state = Arc::new(Mutex::new(DownloadIteratorState::new(
        rx,
        tx.clone(),
        config,
    )));

    {
        let mut streams = download_registry(&ctx).streams.lock().await;
        streams.insert(task_id.clone(), state.clone());
    }

    if let Err(spawn_err) = spawn_download_worker(state.clone()).await {
        let mut tx = tx.clone();
        let _ = tx
            .send(DownloadIteratorMessage::Error(format!(
                "download worker spawn failed: {spawn_err}"
            )))
            .await;
    }

    let iterator = JSObject::new(&ctx);
    let next_task_id = task_id.clone();
    let next_fn = JSFunc::new(&ctx, move |ctx: JSContext| {
        let task_id = next_task_id.clone();
        async move { download_next_step(&ctx, &task_id).await }
    })?;
    iterator.set("next", next_fn)?;

    let return_task_id = task_id.clone();
    let return_fn = JSFunc::new(&ctx, move |ctx: JSContext| {
        let task_id = return_task_id.clone();
        async move {
            download_abort_task(&ctx, &task_id).await?;
            Ok(JSDownloadIteratorStep {
                done: true,
                value: None,
            })
        }
    })?;
    iterator.set("return", return_fn)?;

    let pause_task_id = task_id.clone();
    let pause_fn = JSFunc::new(&ctx, move |ctx: JSContext| {
        let task_id = pause_task_id.clone();
        async move { download_pause_task(&ctx, &task_id).await }
    })?;
    iterator.set("pause", pause_fn)?;

    let resume_task_id = task_id.clone();
    let resume_fn = JSFunc::new(&ctx, move |ctx: JSContext| {
        let task_id = resume_task_id.clone();
        async move { download_resume_task(&ctx, &task_id).await }
    })?;
    iterator.set("resume", resume_fn)?;

    let cancel_task_id = task_id;
    let cancel_fn = JSFunc::new(&ctx, move |ctx: JSContext| {
        let task_id = cancel_task_id.clone();
        async move { download_cancel_task(&ctx, &task_id).await }
    })?;
    iterator.set("cancel", cancel_fn)?;

    Ok(iterator)
}

async fn download_next_step(ctx: &JSContext, task_id: &str) -> JSResult<JSDownloadIteratorStep> {
    let task_id = task_id.trim().to_string();
    if task_id.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "download next requires taskId",
        ));
    }

    let state = {
        let streams = download_registry(&ctx).streams.lock().await;
        streams.get(&task_id).cloned()
    };

    let Some(state) = state else {
        return Ok(JSDownloadIteratorStep {
            done: true,
            value: None,
        });
    };

    let mut receiver = {
        let mut state_guard = state.lock().await;
        if state_guard.terminal_seen {
            drop(state_guard);
            let mut streams = download_registry(&ctx).streams.lock().await;
            streams.remove(&task_id);
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
    match message {
        Some(DownloadIteratorMessage::Progress {
            downloaded_bytes,
            total_bytes,
        }) => {
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
                    progress: Some(progress),
                    result: None,
                }),
            })
        }
        Some(DownloadIteratorMessage::Paused) => Ok(JSDownloadIteratorStep {
            done: false,
            value: Some(simple_event("paused")),
        }),
        Some(DownloadIteratorMessage::Resumed) => Ok(JSDownloadIteratorStep {
            done: false,
            value: Some(simple_event("resumed")),
        }),
        Some(DownloadIteratorMessage::Canceled) => {
            state_guard.status = DownloadTaskStatus::Terminal;
            state_guard.terminal_seen = true;
            Ok(JSDownloadIteratorStep {
                done: false,
                value: Some(simple_event("canceled")),
            })
        }
        Some(DownloadIteratorMessage::Success(result)) => {
            state_guard.status = DownloadTaskStatus::Terminal;
            state_guard.terminal_seen = true;
            let js_result = to_js_download_result(&ctx, &result)?;
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
        Some(DownloadIteratorMessage::Error(message)) => {
            state_guard.status = DownloadTaskStatus::Terminal;
            state_guard.terminal_seen = true;
            drop(state_guard);
            let mut streams = download_registry(&ctx).streams.lock().await;
            streams.remove(&task_id);
            Err(js_internal_error(format!("download failed: {message}")))
        }
        None => {
            state_guard.status = DownloadTaskStatus::Terminal;
            state_guard.terminal_seen = true;
            drop(state_guard);
            let mut streams = download_registry(&ctx).streams.lock().await;
            streams.remove(&task_id);
            Ok(JSDownloadIteratorStep {
                done: true,
                value: None,
            })
        }
    }
}

async fn download_abort_task(ctx: &JSContext, task_id: &str) -> JSResult<()> {
    let task_id = task_id.trim().to_string();
    if task_id.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "download abort requires taskId",
        ));
    }

    let state = {
        let mut streams = download_registry(ctx).streams.lock().await;
        streams.remove(&task_id)
    };
    if let Some(state) = state {
        let mut state = state.lock().await;
        state.status = DownloadTaskStatus::Terminal;
        state.terminal_seen = true;
        if let Some(abort_handle) = state.abort_handle.take() {
            abort_handle.abort();
        }
        state.sender.close_channel();
    }
    Ok(())
}

async fn download_pause_task(ctx: &JSContext, task_id: &str) -> JSResult<()> {
    let task_id = task_id.trim().to_string();
    if task_id.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "download pause requires taskId",
        ));
    }
    let Some(state) = get_download_state(ctx, &task_id).await else {
        return Ok(());
    };

    let mut state = state.lock().await;
    if state.status != DownloadTaskStatus::Running {
        return Ok(());
    }
    state.status = DownloadTaskStatus::Paused;
    if let Some(abort_handle) = state.abort_handle.take() {
        abort_handle.abort();
    }
    let _ = state.sender.try_send(DownloadIteratorMessage::Paused);
    Ok(())
}

async fn download_resume_task(ctx: &JSContext, task_id: &str) -> JSResult<()> {
    let task_id = task_id.trim().to_string();
    if task_id.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "download resume requires taskId",
        ));
    }
    let Some(state) = get_download_state(ctx, &task_id).await else {
        return Ok(());
    };

    {
        let mut guard = state.lock().await;
        if guard.status == DownloadTaskStatus::Terminal {
            return Ok(());
        }
        if guard.status == DownloadTaskStatus::Running {
            return Ok(());
        }
        guard.status = DownloadTaskStatus::Running;
        let _ = guard.sender.try_send(DownloadIteratorMessage::Resumed);
    }

    if let Err(spawn_err) = spawn_download_worker(state.clone()).await {
        let mut guard = state.lock().await;
        guard.status = DownloadTaskStatus::Terminal;
        let _ = guard
            .sender
            .try_send(DownloadIteratorMessage::Error(format!(
                "download worker spawn failed: {spawn_err}"
            )));
    }
    Ok(())
}

async fn download_cancel_task(ctx: &JSContext, task_id: &str) -> JSResult<()> {
    let task_id = task_id.trim().to_string();
    if task_id.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "download cancel requires taskId",
        ));
    }
    let Some(state) = get_download_state(ctx, &task_id).await else {
        return Ok(());
    };

    let (mut sender, config) = {
        let mut guard = state.lock().await;
        guard.status = DownloadTaskStatus::Terminal;
        guard.terminal_seen = false;
        if let Some(abort_handle) = guard.abort_handle.take() {
            abort_handle.abort();
        }
        (guard.sender.clone(), guard.config.clone())
    };

    lxapp::download_manager::clear_user_cache_download_artifacts(
        &config.user_cache_dir,
        &config.request,
    )
    .await;

    let _ = sender.send(DownloadIteratorMessage::Canceled).await;
    Ok(())
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.runtime()
        .get_or_init_service::<DownloadIteratorRegistry>();
    lx::register_js_api(ctx, "openDocument", JSFunc::new(ctx, open_document)?)?;
    lx::register_js_api(ctx, "chooseFile", JSFunc::new(ctx, choose_file)?)?;
    lx::register_js_api(ctx, "chooseDirectory", JSFunc::new(ctx, choose_directory)?)?;
    lx::register_js_api(ctx, "downloadFile", JSFunc::new(ctx, download_file)?)?;
    ctx.eval::<()>(Source::from_bytes(DOWNLOAD_ASYNC_ITERATOR_SHIM))?;
    Ok(())
}
