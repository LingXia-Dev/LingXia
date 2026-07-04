use crate::i18n::{
    js_error_from_business_code_with_detail, js_internal_error, js_invalid_parameter_error,
    js_resource_not_found_error,
};
use futures::channel::oneshot;
use lingxia_service::update::{
    AppUpdateApply, AppUpdateEvent, AppUpdateStage, HostAppUpdateService, UpdateError,
    UpdatePackageInfo,
};
use lxapp::LxApp;
use rong::{IntoJSObj, JSContext, JSFunc, JSObject, JSResult, Promise};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::{Mutex, watch};

#[derive(Debug, Clone, IntoJSObj)]
struct JSAppUpdateEvent {
    state: String,
    stage: Option<String>,
    #[rename = "downloadedBytes"]
    downloaded_bytes: Option<u64>,
    progress: Option<u8>,
    error: Option<String>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSAppUpdateIteratorStep {
    done: bool,
    value: Option<JSAppUpdateEvent>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSAppUpdateResult {
    state: String,
}

enum AppUpdateCompletion {
    Success(JSAppUpdateResult),
    Failed {
        stage: AppUpdateStage,
        error: String,
    },
    Canceled,
}

struct AppUpdateIteratorState {
    receiver: Option<watch::Receiver<Option<AppUpdateEvent>>>,
    terminal_seen: bool,
    iteration_closed: bool,
}

impl AppUpdateIteratorState {
    fn new(receiver: watch::Receiver<Option<AppUpdateEvent>>) -> Self {
        Self {
            receiver: Some(receiver),
            terminal_seen: false,
            iteration_closed: false,
        }
    }
}

pub(super) fn init(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    let check_update = JSFunc::new(ctx, check_app_update)?.name("checkUpdate")?;
    app.set("checkUpdate", check_update)?;
    Ok(())
}

async fn check_app_update(ctx: JSContext) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    super::ensure_home_lxapp(&lxapp, "lx.app.checkUpdate")?;

    let update = host_update_service_from(&lxapp)
        .check()
        .await
        .map_err(js_error_from_update_error)?;
    let Some(update) = update else {
        return create_check_result(&ctx, None);
    };
    if let Err(error) = update.ensure_runtime_compatible(lxapp::SDK_RUNTIME_VERSION, "host app") {
        log::warn!("Host app update is hidden from JS because runtime is incompatible: {error}");
        return create_check_result(&ctx, None);
    }

    create_check_result(&ctx, Some(update))
}

fn create_check_result(ctx: &JSContext, update: Option<UpdatePackageInfo>) -> JSResult<JSObject> {
    let result = JSObject::new(ctx);
    match update {
        Some(update) => {
            result.set("hasUpdate", true)?;
            result.set("update", create_update_object(ctx, update)?)?;
        }
        None => {
            result.set("hasUpdate", false)?;
        }
    }
    Ok(result)
}

fn create_update_object(ctx: &JSContext, update: UpdatePackageInfo) -> JSResult<JSObject> {
    let obj = JSObject::new(ctx);
    obj.set("version", update.version.clone())?;
    obj.set("size", update.size)?;
    obj.set("releaseNotes", update.release_notes.clone())?;
    obj.set("isForceUpdate", update.is_force_update)?;

    let package = Arc::new(StdMutex::new(Some(update)));
    obj.set(
        "apply",
        JSFunc::new(ctx, move |ctx: JSContext| {
            let package = package
                .lock()
                .map_err(|_| js_internal_error("app update state is poisoned"))?
                .take()
                .ok_or_else(|| js_resource_not_found_error("app update already applied"))?;
            create_apply_task(&ctx, package)
        })?,
    )?;

    Ok(obj)
}

fn create_apply_task(ctx: &JSContext, package: UpdatePackageInfo) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(ctx)?;
    super::ensure_home_lxapp(&lxapp, "lx.app.checkUpdate")?;

    let service = host_update_service_from(&lxapp);
    // Store-delivered platforms (iOS/HarmonyOS) update through the store and
    // never self-install; only platforms that report `self_update_supported`
    // can apply a downloaded package in place.
    if !service.self_update_supported() {
        return Err(js_error_from_business_code_with_detail(
            6000,
            "host app self-update is not supported on this platform",
        ));
    }

    let apply = service.apply(package);
    let (tx, rx) = watch::channel::<Option<AppUpdateEvent>>(None);
    let (completion_tx, completion_rx) = oneshot::channel::<AppUpdateCompletion>();
    spawn_app_update_forwarder(apply, tx, completion_tx);

    let final_promise =
        Promise::from_future(ctx, None, async move {
            match completion_rx.await {
                Ok(AppUpdateCompletion::Success(result)) => Ok(result),
                Ok(AppUpdateCompletion::Failed { stage, error }) => Err(js_internal_error(
                    format!("app update failed at {}: {}", stage_name(stage), error),
                )),
                Ok(AppUpdateCompletion::Canceled) | Err(_) => {
                    Err(js_internal_error("app update canceled"))
                }
            }
        })?;

    let state = Arc::new(Mutex::new(AppUpdateIteratorState::new(rx)));
    let iterator = JSObject::new(ctx);

    let next_state = state.clone();
    iterator.set(
        "next",
        JSFunc::new(ctx, move || {
            let state = next_state.clone();
            async move { app_update_next_step(&state).await }
        })?,
    )?;

    let return_state = state.clone();
    iterator.set(
        "return",
        JSFunc::new(ctx, move || {
            let state = return_state.clone();
            async move {
                let mut guard = state.lock().await;
                guard.iteration_closed = true;
                guard.receiver = None;
                Ok(JSAppUpdateIteratorStep {
                    done: true,
                    value: None,
                })
            }
        })?,
    )?;

    crate::task_object::install_promise_methods(ctx, &iterator, final_promise)?;
    crate::task_object::install_async_iterator(ctx, &iterator)?;
    Ok(iterator)
}

async fn app_update_next_step(
    state: &Arc<Mutex<AppUpdateIteratorState>>,
) -> JSResult<JSAppUpdateIteratorStep> {
    let mut receiver = {
        let mut guard = state.lock().await;
        if guard.terminal_seen || guard.iteration_closed {
            return Ok(JSAppUpdateIteratorStep {
                done: true,
                value: None,
            });
        }
        guard
            .receiver
            .take()
            .ok_or_else(|| js_internal_error("app update iterator receiver unexpectedly missing"))?
    };

    let event = match receiver.changed().await {
        Ok(()) => receiver.borrow().clone(),
        Err(_) => None,
    };

    let mut guard = state.lock().await;
    if guard.iteration_closed {
        return Ok(JSAppUpdateIteratorStep {
            done: true,
            value: None,
        });
    }

    guard.receiver = Some(receiver);
    let Some(event) = event else {
        guard.terminal_seen = true;
        return Ok(JSAppUpdateIteratorStep {
            done: true,
            value: None,
        });
    };

    if is_terminal_event(&event) {
        guard.terminal_seen = true;
        guard.receiver = None;
    }

    Ok(JSAppUpdateIteratorStep {
        done: false,
        value: Some(js_event_from_update_event(event)),
    })
}

fn spawn_app_update_forwarder(
    mut apply: AppUpdateApply,
    tx: watch::Sender<Option<AppUpdateEvent>>,
    completion_tx: oneshot::Sender<AppUpdateCompletion>,
) {
    std::mem::drop(rong::RongExecutor::global().spawn(async move {
        let mut completion_tx = Some(completion_tx);
        while let Some(event) = apply.next().await {
            if matches!(event, AppUpdateEvent::Available(_)) {
                continue;
            }

            let terminal = is_terminal_event(&event);
            if terminal && let Some(sender) = completion_tx.take() {
                let _ = sender.send(completion_from_event(&event));
            }

            let _ = tx.send(Some(event));

            if terminal {
                return;
            }
        }

        if let Some(sender) = completion_tx.take() {
            let _ = sender.send(AppUpdateCompletion::Canceled);
        }
    }));
}

fn completion_from_event(event: &AppUpdateEvent) -> AppUpdateCompletion {
    match event {
        AppUpdateEvent::InstallRequested { .. } => {
            AppUpdateCompletion::Success(JSAppUpdateResult {
                state: "installRequested".to_string(),
            })
        }
        AppUpdateEvent::Failed { stage, error } => AppUpdateCompletion::Failed {
            stage: *stage,
            error: error.clone(),
        },
        _ => AppUpdateCompletion::Canceled,
    }
}

fn host_update_service_from(lxapp: &LxApp) -> HostAppUpdateService {
    HostAppUpdateService::new(lxapp.runtime.clone(), lxapp::provider::update_provider())
}

fn is_terminal_event(event: &AppUpdateEvent) -> bool {
    matches!(
        event,
        AppUpdateEvent::InstallRequested { .. } | AppUpdateEvent::Failed { .. }
    )
}

fn js_event_from_update_event(event: AppUpdateEvent) -> JSAppUpdateEvent {
    match event {
        AppUpdateEvent::DownloadStarted { .. } => JSAppUpdateEvent {
            state: "downloading".to_string(),
            stage: None,
            downloaded_bytes: None,
            progress: None,
            error: None,
        },
        AppUpdateEvent::DownloadProgress {
            downloaded_bytes,
            progress,
            ..
        } => JSAppUpdateEvent {
            state: "downloading".to_string(),
            stage: None,
            downloaded_bytes: Some(downloaded_bytes),
            progress,
            error: None,
        },
        AppUpdateEvent::Downloaded { .. } => JSAppUpdateEvent {
            state: "downloaded".to_string(),
            stage: None,
            downloaded_bytes: None,
            progress: None,
            error: None,
        },
        AppUpdateEvent::InstallRequested { .. } => JSAppUpdateEvent {
            state: "installRequested".to_string(),
            stage: None,
            downloaded_bytes: None,
            progress: None,
            error: None,
        },
        AppUpdateEvent::Failed { stage, error } => JSAppUpdateEvent {
            state: "failed".to_string(),
            stage: Some(stage_name(stage).to_string()),
            downloaded_bytes: None,
            progress: None,
            error: Some(error),
        },
        AppUpdateEvent::Available(_) => unreachable!("apply task filters availability events"),
    }
}

fn stage_name(stage: AppUpdateStage) -> &'static str {
    match stage {
        AppUpdateStage::Check => "check",
        AppUpdateStage::Download => "download",
        AppUpdateStage::Install => "install",
    }
}

fn js_error_from_update_error(error: UpdateError) -> rong::RongJSError {
    match error {
        UpdateError::InvalidParameter(detail) => js_invalid_parameter_error(detail),
        UpdateError::UnsupportedOperation(detail) => {
            js_error_from_business_code_with_detail(6000, detail)
        }
        UpdateError::ResourceNotFound(detail) => js_resource_not_found_error(detail),
        UpdateError::Io(detail) | UpdateError::Runtime(detail) => js_internal_error(detail),
    }
}
