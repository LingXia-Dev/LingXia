use crate::PageLifecycleEvent;
use crate::bridge::{
    BRIDGE_CANCELED, BRIDGE_INTERNAL_ERROR, BRIDGE_METHOD_NOT_FOUND, BRIDGE_TOPIC_NOT_FOUND,
    OutboundContext, PageBridge, RpcError, SessionWorkId, ViewTransport,
};
use crate::error;
use crate::error::LxAppError;
use crate::lxapp::LxApp;
use crate::page::PageInstance;
use lingxia_webview::{DocumentGeneration, DocumentOutboundGate};
use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, JSSymbol, JSValue, JsonToJSValue, RongJSError,
    Source, error::HostError, function::Optional, js_class, js_method,
};
use rong_event::EventEmitter;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::rc::Rc;
use tokio::sync::{Mutex, oneshot, watch};

const ASYNC_ITERATOR_RETURN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

type LifecycleQueue = Rc<RefCell<std::collections::VecDeque<(PageLifecycleEvent, Option<String>)>>>;

#[js_class(clone)]
pub struct PageSvc {
    functions: HashMap<String, JSFunc>,
    /// Methods declared as `stream_handlers` in page meta — they receive an
    /// explicit `StreamHandle` JS object as their second argument and are
    /// expected to call `stream.end(result)` (or `stream.error(code, msg)`)
    /// instead of returning an async iterator.
    stream_handlers: HashSet<String>,
    this: JSObject,

    pub(crate) page: PageInstance,
    event_emitter: EventEmitter,

    // state of PageSvc
    state: Rc<Mutex<PageSvcState>>,

    /// Lifecycle handlers run off the worker pump, but a page's events must
    /// still execute in dispatch order (onLoad → onShow → onReady). Each
    /// event enqueues here and a single drainer runs the queue FIFO.
    lifecycle_queue: LifecycleQueue,
    lifecycle_pump_running: Rc<Cell<bool>>,

    /// Set when TerminatePage retires this service. Queued and in-flight
    /// lifecycle handlers may resume afterwards; they must not reach the
    /// (possibly rebuilt) document through the shared PageInstance.
    terminated: Rc<Cell<bool>>,
}

struct PageSvcState {
    callback: HashMap<String, JSFunc>,
    state_callback: HashMap<u64, StateCallback>,
    state_rev: u64,
    /// True until the first bridge-ready snapshot of live page data is sent.
    initial_snapshot_pending: bool,
    channels: HashMap<ChannelKey, ChannelState>,
    next_channel_token: u64,
    active_session_work: Option<SessionWorkId>,
    work_cancellations: HashMap<SessionWorkId, watch::Sender<bool>>,
    /// Monotonic guard for backend queue races: an old Begin must never
    /// overwrite a successor after cancellation/recreation.
    max_seen_session_work: Option<SessionWorkId>,
}

struct StateCallback {
    callback: JSFunc,
    work_id: Option<SessionWorkId>,
    outbound: Option<OutboundContext>,
}

#[derive(Clone)]
struct CallbackWork {
    work_id: Option<SessionWorkId>,
    outbound: Option<OutboundContext>,
}

tokio::task_local! {
    /// Credentials attached to one document-originated JavaScript invocation.
    /// Rong keeps this task alive while `call_async` awaits a returned Promise.
    static DOCUMENT_CALLBACK_WORK: CallbackWork;
}

pub(crate) async fn with_document_callback_work<F>(
    work_id: Option<SessionWorkId>,
    outbound: Option<OutboundContext>,
    future: F,
) -> F::Output
where
    F: Future,
{
    DOCUMENT_CALLBACK_WORK
        .scope(CallbackWork { work_id, outbound }, future)
        .await
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct ChannelKey {
    work_id: SessionWorkId,
    id: String,
}

fn active_work_matches<Work: Eq>(active: Option<Work>, candidate: Option<Work>) -> bool {
    candidate.is_some_and(|candidate| active == Some(candidate))
}

fn cancel_active_work<Work: Eq>(active: Option<Work>, cancelled: Work) -> Option<Work> {
    if active.as_ref().is_some_and(|active| active == &cancelled) {
        None
    } else {
        active
    }
}

fn accepts_begin_work(max_seen: Option<SessionWorkId>, candidate: SessionWorkId) -> bool {
    max_seen.is_none_or(|seen| candidate.is_newer_than(seen))
}

struct ChannelState {
    token: u64,
    cancel_tx: watch::Sender<bool>,
    tail: Option<oneshot::Receiver<()>>,
    /// Shared with the `ch.on()` JS closure so that listeners registered at
    /// *any* point during the channel's lifetime take effect immediately.
    listeners: Rc<RefCell<ChannelListeners>>,
    outbound_seq: u64,
    /// Captured at channel creation. A channel must never consult a later
    /// handshake when its JS callback eventually emits a frame.
    outbound: Option<OutboundContext>,
}

struct ChannelListeners {
    on_data: Option<JSFunc>,
    on_close: Option<JSFunc>,
}

struct ChannelTurn {
    token: u64,
    work_id: SessionWorkId,
    outbound: Option<OutboundContext>,
    previous: Option<oneshot::Receiver<()>>,
    cancel_rx: watch::Receiver<bool>,
    _cancel_tx: watch::Sender<bool>,
    done_tx: Option<oneshot::Sender<()>>,
}

impl ChannelTurn {
    async fn wait(&mut self) -> bool {
        if *self.cancel_rx.borrow() {
            return false;
        }
        let Some(mut previous) = self.previous.take() else {
            return true;
        };
        tokio::select! {
            biased;
            _ = &mut previous => !*self.cancel_rx.borrow(),
            changed = self.cancel_rx.changed() => changed.is_ok() && !*self.cancel_rx.borrow(),
        }
    }
}

impl Drop for ChannelTurn {
    fn drop(&mut self) {
        if let Some(done_tx) = self.done_tx.take() {
            let _ = done_tx.send(());
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct PageBindingMeta {
    #[serde(default)]
    handlers: Vec<String>,
    /// Methods that receive an explicit `StreamHandle` JS object as their
    /// second argument instead of using the `async function*` generator pattern.
    #[serde(default)]
    stream_handlers: Vec<String>,
}

fn rpc_error_from_lxapp_error(err: &LxAppError) -> RpcError {
    if let LxAppError::RongJSHost {
        code,
        message,
        data,
    } = err
    {
        return RpcError {
            code: code.clone(),
            message: Some(message.clone()),
            data: data.clone(),
        };
    }
    RpcError::new(BRIDGE_INTERNAL_ERROR, Some(err.to_string()))
}

fn rpc_error_from_rong(err: RongJSError) -> RpcError {
    let lxapp_error: LxAppError = err.into();
    rpc_error_from_lxapp_error(&lxapp_error)
}

async fn await_js_call_or_cancel<T>(
    cancel_rx: &mut oneshot::Receiver<()>,
    call: impl std::future::Future<Output = JSResult<T>>,
) -> Result<T, RpcError> {
    tokio::select! {
        biased;
        _ = cancel_rx => Err(RpcError::new(BRIDGE_CANCELED, None)),
        result = call => result.map_err(rpc_error_from_rong),
    }
}

async fn await_channel_call_or_cancel<T>(
    cancel_rx: &mut watch::Receiver<bool>,
    call: impl std::future::Future<Output = JSResult<T>>,
) -> Result<T, RpcError> {
    if *cancel_rx.borrow() {
        return Err(RpcError::new(BRIDGE_CANCELED, None));
    }
    tokio::select! {
        biased;
        changed = cancel_rx.changed() => {
            let _ = changed;
            Err(RpcError::new(BRIDGE_CANCELED, None))
        }
        result = call => result.map_err(rpc_error_from_rong),
    }
}

fn js_value_to_json_str(v: JSValue) -> Result<String, RpcError> {
    if v.is_undefined() || v.is_null() {
        return Ok("null".to_owned());
    }
    if v.is_boolean() {
        let b: bool = v
            .into_value()
            .try_into()
            .map_err(|e: RongJSError| rpc_error_from_rong(e))?;
        return Ok(if b { "true" } else { "false" }.to_owned());
    }
    if v.is_number() {
        let n: f64 = v
            .into_value()
            .try_into()
            .map_err(|e: RongJSError| rpc_error_from_rong(e))?;
        let num = serde_json::Number::from_f64(n).ok_or_else(|| {
            RpcError::new(BRIDGE_INTERNAL_ERROR, Some("Invalid number".to_string()))
        })?;
        return Ok(num.to_string());
    }
    if v.is_string() {
        let s: String = v
            .into_value()
            .try_into()
            .map_err(|e: RongJSError| rpc_error_from_rong(e))?;
        return serde_json::to_string(&s)
            .map_err(|e| RpcError::new(BRIDGE_INTERNAL_ERROR, Some(e.to_string())));
    }
    if let Some(obj) = v.into_object() {
        return obj
            .to_json_string()
            .map_err(|e| RpcError::new(BRIDGE_INTERNAL_ERROR, Some(e.to_string())));
    }

    Err(RpcError::new(
        BRIDGE_INTERNAL_ERROR,
        Some("Unsupported JS return type".to_string()),
    ))
}

fn get_async_iterator_symbol(ctx: &JSContext) -> Result<JSSymbol, RpcError> {
    ctx.global()
        .get::<_, JSObject>("Symbol")
        .and_then(|symbol| symbol.get::<_, JSSymbol>("asyncIterator"))
        .map_err(rpc_error_from_rong)
}

fn maybe_get_async_iterator(
    ctx: &JSContext,
    value: &JSValue,
) -> Result<Option<JSObject>, RpcError> {
    let Some(obj) = value.clone().into_object() else {
        return Ok(None);
    };

    let async_iter_symbol = get_async_iterator_symbol(ctx)?;
    if let Ok(async_iter_fn) = obj.get::<_, JSFunc>(async_iter_symbol) {
        let iterator = async_iter_fn
            .call::<_, JSObject>(Some(obj.clone()), ())
            .map_err(rpc_error_from_rong)?;
        return Ok(Some(iterator));
    }

    if obj.get::<_, JSFunc>("next").is_ok() {
        return Ok(Some(obj));
    }

    Ok(None)
}

fn get_optional_property(obj: &JSObject, field: &str, ctx: &JSContext) -> JSValue {
    obj.get::<_, JSValue>(field)
        .unwrap_or_else(|_| JSValue::undefined(ctx))
}

fn read_async_iterator_step(
    step_obj: &JSObject,
    ctx: &JSContext,
) -> Result<(bool, String), RpcError> {
    let done = step_obj
        .get::<_, bool>("done")
        .map_err(rpc_error_from_rong)?;
    let value_json = js_value_to_json_str(get_optional_property(step_obj, "value", ctx))?;
    Ok((done, value_json))
}

impl ViewTransport for PageSvc {
    fn post_message_to_view(&self, message_json: String) -> Result<(), LxAppError> {
        if let Some(controller) = self.page.webview_controller() {
            controller
                .post_message(&message_json)
                .map_err(LxAppError::from)
        } else {
            Err(LxAppError::WebView("WebView not ready".to_string()))
        }
    }

    fn post_message_to_document(
        &self,
        expected_generation: DocumentGeneration,
        gate: std::sync::Arc<dyn DocumentOutboundGate>,
        message_json: String,
    ) -> Result<(), LxAppError> {
        self.page
            .post_message_to_document(expected_generation, gate, message_json)
    }
}

impl PageSvc {
    pub(crate) async fn get_state_snapshot(
        &self,
        _scope: Option<&str>,
    ) -> Result<String, LxAppError> {
        let data_obj = self
            .this
            .get::<_, JSObject>("data")
            .map_err(|e| LxAppError::Bridge(e.to_string()))?;
        let data_json = data_obj
            .to_json_string()
            .map_err(|e| LxAppError::Bridge(e.to_string()))?;
        let rev = self.state.lock().await.state_rev;
        Ok(format!(r#"{{"rev":{},"state":{}}}"#, rev, data_json))
    }

    pub(crate) async fn handle_req(
        &self,
        work_id: Option<SessionWorkId>,
        outbound: Option<OutboundContext>,
        req_id: &str,
        method: &str,
        params_json: Option<&str>,
        mut cancel_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<String, RpcError> {
        let ctx = self.get_ctx();

        let build_call_arg = |json: Option<&str>| -> Option<JSValue> {
            let json = json?;
            if json == "null" {
                return None;
            }
            json.json_to_js_value(&ctx).ok()
        };

        let Some(js_func) = self.get_js_func(method) else {
            return Err(RpcError::new(
                BRIDGE_METHOD_NOT_FOUND,
                Some(format!("Method not found: {}", method)),
            ));
        };

        let call_arg = build_call_arg(params_json);

        // Explicit stream handle path — function declared in `stream_handlers`.
        // The handler receives `(params, streamHandle)` and is expected to call
        // `streamHandle.end(result)` or `streamHandle.error(code, msg)`.
        if self.stream_handlers.contains(method) {
            let (stream_handle, mut end_rx) =
                self.create_stream_handle(req_id, work_id, outbound.clone())?;
            let call = async {
                match call_arg {
                    Some(val) => {
                        js_func
                            .call_async::<_, JSValue>(Some(self.this.clone()), (val, stream_handle))
                            .await
                    }
                    None => {
                        js_func
                            .call_async::<_, JSValue>(
                                Some(self.this.clone()),
                                (JSObject::new(&ctx), stream_handle),
                            )
                            .await
                    }
                }
            };
            await_js_call_or_cancel(&mut cancel_rx, call).await?;
            return tokio::select! {
                biased;
                _ = &mut cancel_rx => Err(RpcError::new(BRIDGE_CANCELED, None)),
                result = &mut end_rx => match result {
                    Ok(r) => r,
                    Err(_) => Err(RpcError::new(
                        BRIDGE_INTERNAL_ERROR,
                        Some("Stream handle dropped without end/error".to_string()),
                    )),
                }
            };
        }

        // Generator / unary path.
        let fut = async {
            match call_arg {
                Some(val) => {
                    js_func
                        .call_async::<_, JSValue>(Some(self.this.clone()), (val,))
                        .await
                }
                None => {
                    js_func
                        .call_async::<_, JSValue>(Some(self.this.clone()), ())
                        .await
                }
            }
        };

        let value = tokio::select! {
            biased;
            _ = &mut cancel_rx => {
                return Err(RpcError::new(BRIDGE_CANCELED, None));
            }
            res = fut => {
                match res {
                    Ok(v) => v,
                    Err(e) => return Err(rpc_error_from_rong(e)),
                }
            }
        };

        if let Some(iterator) = maybe_get_async_iterator(&ctx, &value)? {
            return self
                .consume_async_iterator(
                    req_id,
                    iterator,
                    &mut cancel_rx,
                    work_id,
                    outbound.as_ref(),
                )
                .await;
        }

        js_value_to_json_str(value)
    }

    pub(crate) async fn handle_notify(
        &self,
        work_id: Option<SessionWorkId>,
        outbound: Option<OutboundContext>,
        method: &str,
        params_json: Option<&str>,
    ) {
        let Some(work_id) = work_id else {
            return;
        };
        let Some(mut work_cancel) = self.work_cancellation_receiver(work_id).await else {
            return;
        };
        let Some(js_func) = self.get_js_func(method) else {
            return;
        };

        let ctx = self.get_ctx();
        let call_arg = params_json.and_then(|json| {
            if json == "null" {
                return None;
            }
            json.json_to_js_value(&ctx).ok()
        });

        let this_obj = self.this.clone();
        let method_name = method.to_string();
        let page_path = self.page.path().to_string();
        let task = async move {
            if *work_cancel.borrow() {
                return;
            }
            let result = tokio::select! {
                biased;
                changed = work_cancel.changed() => {
                    let _ = changed;
                    return;
                }
                result = async {
                    match call_arg {
                        Some(val) => js_func.call_async::<_, ()>(Some(this_obj), (val,)).await,
                        None => js_func.call_async::<_, ()>(Some(this_obj), ()).await,
                    }
                } => result,
            };
            if let Err(e) = result {
                error!("[{}] notify '{}' failed: {}", page_path, method_name, e);
            }
        };
        super::context_lifecycle::spawn(&ctx, move |_ctx| {
            with_document_callback_work(Some(work_id), outbound, task)
        });
    }

    pub(crate) async fn handle_ch_open(
        &self,
        work_id: Option<SessionWorkId>,
        outbound: Option<OutboundContext>,
        id: &str,
        topic: &str,
        params_json: Option<&str>,
    ) -> Result<oneshot::Receiver<Result<(), RpcError>>, RpcError> {
        let work_id = work_id.ok_or_else(|| RpcError::new(BRIDGE_CANCELED, None))?;
        let Some(js_func) = self.get_js_func(topic) else {
            return Err(RpcError::new(
                BRIDGE_TOPIC_NOT_FOUND,
                Some(format!("Topic not found: {}", topic)),
            ));
        };

        let ctx = self.get_ctx();
        let call_arg = params_json.and_then(|json| {
            if json == "null" {
                return None;
            }
            json.json_to_js_value(&ctx).ok()
        });

        let listeners = Rc::new(RefCell::new(ChannelListeners {
            on_data: None,
            on_close: None,
        }));
        let key = ChannelKey {
            work_id,
            id: id.to_string(),
        };
        let token = {
            let mut state = self.state.lock().await;
            let token = state.next_channel_token;
            state.next_channel_token = token.checked_add(1).ok_or_else(|| {
                RpcError::new(
                    BRIDGE_INTERNAL_ERROR,
                    Some("channel token space exhausted".to_string()),
                )
            })?;
            token
        };
        let channel_ctx =
            self.create_channel_context(key.clone(), token, listeners.clone(), outbound.clone())?;
        let mut turn = {
            let mut state = self.state.lock().await;
            // Linearize registration with CancelSessionWork. If cancellation
            // already swept this work, do not publish a channel that it can no
            // longer observe; if it follows, the same lock makes it remove and
            // signal this exact channel.
            if state.active_session_work != Some(work_id) {
                return Err(RpcError::new(BRIDGE_CANCELED, None));
            }
            let (cancel_tx, cancel_rx) = watch::channel(false);
            let (done_tx, done_rx) = oneshot::channel();
            if let Some(replaced) = state.channels.insert(
                key.clone(),
                ChannelState {
                    token,
                    cancel_tx: cancel_tx.clone(),
                    tail: Some(done_rx),
                    listeners,
                    outbound_seq: 0,
                    outbound: outbound.clone(),
                },
            ) {
                let _ = replaced.cancel_tx.send(true);
            }
            ChannelTurn {
                token,
                work_id,
                outbound: outbound.clone(),
                previous: None,
                cancel_rx,
                _cancel_tx: cancel_tx,
                done_tx: Some(done_tx),
            }
        };
        let (result_tx, result_rx) = oneshot::channel();
        let page_svc = self.clone();
        let key_for_task = key.clone();
        let outbound_for_task = outbound.clone();
        let this = self.this.clone();
        super::context_lifecycle::spawn(&ctx, move |ctx| async move {
            let result =
                with_document_callback_work(Some(key_for_task.work_id), outbound_for_task, async {
                    if !turn.wait().await {
                        Err(RpcError::new(BRIDGE_CANCELED, None))
                    } else {
                        let call = async {
                            match call_arg {
                                Some(val) => {
                                    js_func
                                        .call_async::<_, JSValue>(Some(this), (val, channel_ctx))
                                        .await
                                }
                                None => {
                                    js_func
                                        .call_async::<_, JSValue>(
                                            Some(this),
                                            (JSObject::new(&ctx), channel_ctx),
                                        )
                                        .await
                                }
                            }
                        };
                        await_channel_call_or_cancel(&mut turn.cancel_rx, call).await
                    }
                })
                .await;
            if result.is_err() {
                page_svc
                    .remove_channel_if_token(&key_for_task, turn.token)
                    .await;
            }
            let _ = result_tx.send(result.map(|_| ()));
        });
        Ok(result_rx)
    }

    pub(crate) async fn handle_ch_data(
        &self,
        work_id: Option<SessionWorkId>,
        id: &str,
        payload_json: &str,
    ) -> Result<(), RpcError> {
        let Some(work_id) = work_id else {
            return Ok(());
        };
        let key = ChannelKey {
            work_id,
            id: id.to_string(),
        };
        let Some(mut turn) = self.queue_channel_turn(&key).await else {
            return Ok(());
        };
        let payload = payload_json
            .json_to_js_value(&self.get_ctx())
            .map_err(rpc_error_from_rong)?;
        let page_svc = self.clone();
        let key_for_task = key.clone();
        let appid = self.page.appid();
        let path = self.page.path().to_string();
        let ctx = self.get_ctx();
        let callback_work_id = turn.work_id;
        let callback_outbound = turn.outbound.clone();
        super::context_lifecycle::spawn(&ctx, move |_ctx| async move {
            with_document_callback_work(Some(callback_work_id), callback_outbound, async move {
                if !turn.wait().await {
                    return;
                }
                let on_data = page_svc
                    .channel_on_data_if_token(&key_for_task, turn.token)
                    .await;
                let Some(on_data) = on_data else {
                    return;
                };
                if let Err(err) = await_channel_call_or_cancel(
                    &mut turn.cancel_rx,
                    on_data.call_async::<_, ()>(None, (payload,)),
                )
                .await
                    && err.code != BRIDGE_CANCELED
                {
                    error!(
                        "channel '{}' data handler failed: {}",
                        key_for_task.id, err.code
                    )
                    .with_appid(appid)
                    .with_path(path);
                }
            })
            .await;
        });
        Ok(())
    }

    pub(crate) async fn handle_ch_close(
        &self,
        work_id: Option<SessionWorkId>,
        id: &str,
        code: Option<&str>,
        reason: Option<&str>,
    ) {
        let Some(work_id) = work_id else {
            return;
        };
        let key = ChannelKey {
            work_id,
            id: id.to_string(),
        };
        let Some(mut turn) = self.queue_channel_turn(&key).await else {
            return;
        };
        let page_svc = self.clone();
        let key_for_task = key.clone();
        let code = code.unwrap_or_default().to_string();
        let reason = reason.unwrap_or_default().to_string();
        let ctx = self.get_ctx();
        let callback_work_id = turn.work_id;
        let callback_outbound = turn.outbound.clone();
        super::context_lifecycle::spawn(&ctx, move |ctx| async move {
            with_document_callback_work(Some(callback_work_id), callback_outbound, async move {
                if turn.wait().await
                    && let Some(on_close) = page_svc
                        .take_channel_on_close_if_token(&key_for_task, turn.token)
                        .await
                {
                    let info = JSObject::new(&ctx);
                    let _ = info.set("code", code);
                    let _ = info.set("reason", reason);
                    let _ = await_channel_call_or_cancel(
                        &mut turn.cancel_rx,
                        on_close.call_async::<_, ()>(None, (info,)),
                    )
                    .await;
                }
                page_svc
                    .remove_channel_if_token(&key_for_task, turn.token)
                    .await;
            })
            .await;
        });
    }

    async fn queue_channel_turn(&self, key: &ChannelKey) -> Option<ChannelTurn> {
        let mut state = self.state.lock().await;
        let channel = state.channels.get_mut(key)?;
        let previous = channel.tail.take();
        let (done_tx, done_rx) = oneshot::channel();
        channel.tail = Some(done_rx);
        let cancel_tx = channel.cancel_tx.clone();
        Some(ChannelTurn {
            token: channel.token,
            work_id: key.work_id,
            outbound: channel.outbound.clone(),
            previous,
            cancel_rx: cancel_tx.subscribe(),
            _cancel_tx: cancel_tx,
            done_tx: Some(done_tx),
        })
    }

    async fn channel_on_data_if_token(&self, key: &ChannelKey, token: u64) -> Option<JSFunc> {
        let state = self.state.lock().await;
        let channel = state.channels.get(key)?;
        (channel.token == token)
            .then(|| channel.listeners.borrow().on_data.clone())
            .flatten()
    }

    async fn take_channel_on_close_if_token(&self, key: &ChannelKey, token: u64) -> Option<JSFunc> {
        let mut state = self.state.lock().await;
        let channel = state.channels.get_mut(key)?;
        (channel.token == token)
            .then(|| channel.listeners.borrow_mut().on_close.take())
            .flatten()
    }

    async fn remove_channel_if_token(&self, key: &ChannelKey, token: u64) {
        let mut state = self.state.lock().await;
        if state
            .channels
            .get(key)
            .is_some_and(|channel| channel.token == token)
        {
            state.channels.remove(key);
        }
    }

    /// Retire local channel callbacks during PageSvc teardown. Connection
    /// teardown owns protocol close frames; this must never consult or post to
    /// a successor document.
    pub(crate) async fn close_channels(&self, code: &str, reason: &str) {
        let channels = {
            let mut state = self.state.lock().await;
            std::mem::take(&mut state.channels)
        };
        let mut callbacks = Vec::new();
        for (key, channel) in channels {
            let _ = channel.cancel_tx.send(true);
            if let Some(callback) = channel.listeners.borrow_mut().on_close.take() {
                callbacks.push((key.work_id, channel.outbound, callback));
            }
        }
        let code = code.to_string();
        let reason = reason.to_string();
        let ctx = self.get_ctx();
        for (work_id, outbound, callback) in callbacks {
            let code = code.clone();
            let reason = reason.clone();
            super::context_lifecycle::spawn(&ctx, move |ctx| {
                with_document_callback_work(Some(work_id), outbound, async move {
                    let info = JSObject::new(&ctx);
                    let _ = info.set("code", code);
                    let _ = info.set("reason", reason);
                    let _ = callback.call_async::<_, ()>(None, (info,)).await;
                })
            });
        }
    }

    pub(crate) async fn begin_session_work(&self, work_id: SessionWorkId) {
        let mut state = self.state.lock().await;
        if !accepts_begin_work(state.max_seen_session_work, work_id) {
            return;
        }
        state.max_seen_session_work = Some(work_id);
        state.active_session_work = Some(work_id);
        state
            .work_cancellations
            .insert(work_id, watch::channel(false).0);
    }

    pub(crate) async fn cancel_session_work(&self, work_id: SessionWorkId) {
        let (channels, callbacks, work_cancel) = {
            let mut state = self.state.lock().await;
            state.active_session_work = cancel_active_work(state.active_session_work, work_id);
            let work_cancel = state.work_cancellations.remove(&work_id);

            let mut channels = Vec::new();
            state.channels.retain(|key, channel| {
                if key.work_id == work_id {
                    channels.push(channel.cancel_tx.clone());
                    false
                } else {
                    true
                }
            });
            let mut callbacks = Vec::new();
            state.state_callback.retain(|_, callback| {
                if callback.work_id == Some(work_id) {
                    callbacks.push(StateCallback {
                        callback: callback.callback.clone(),
                        work_id: callback.work_id,
                        outbound: callback.outbound.clone(),
                    });
                    false
                } else {
                    true
                }
            });
            (channels, callbacks, work_cancel)
        };
        if let Some(work_cancel) = work_cancel {
            let _ = work_cancel.send(true);
        }
        for channel in channels {
            let _ = channel.send(true);
        }
        let ctx = self.get_ctx();
        for callback in callbacks {
            super::context_lifecycle::spawn(&ctx, move |_ctx| {
                with_document_callback_work(callback.work_id, callback.outbound, async move {
                    let _ = callback.callback.call_async::<_, ()>(None, ()).await;
                })
            });
        }
    }

    pub(crate) async fn session_work_is_active(&self, work_id: Option<SessionWorkId>) -> bool {
        match work_id {
            // A missing work identity is initialization-only; it is never
            // authority to emit an asynchronous document frame.
            None => false,
            Some(work_id) => {
                active_work_matches(self.state.lock().await.active_session_work, Some(work_id))
            }
        }
    }

    async fn work_cancellation_receiver(
        &self,
        work_id: SessionWorkId,
    ) -> Option<watch::Receiver<bool>> {
        let state = self.state.lock().await;
        (state.active_session_work == Some(work_id))
            .then(|| {
                state
                    .work_cancellations
                    .get(&work_id)
                    .map(watch::Sender::subscribe)
            })
            .flatten()
    }

    pub(crate) async fn handle_bridge_ready(
        &self,
        work_id: Option<SessionWorkId>,
        outbound: Option<OutboundContext>,
    ) {
        if !self.session_work_is_active(work_id).await {
            return;
        }
        let mut page_svc_clone = self.clone();
        let _ = page_svc_clone
            .handle_bridge_ready_internal(work_id, outbound)
            .await;
    }

    pub(crate) async fn handle_state_ack(
        &self,
        work_id: Option<SessionWorkId>,
        _scope: Option<String>,
        rev: u64,
    ) {
        let mut state = self.state.lock().await;
        let callback = work_id
            .filter(|work_id| state.active_session_work == Some(*work_id))
            .and_then(|_| {
                state
                    .state_callback
                    .get(&rev)
                    .is_some_and(|callback| callback.work_id == work_id)
                    .then(|| state.state_callback.remove(&rev))
                    .flatten()
            });
        drop(state);
        if let Some(cb) = callback {
            let _ = with_document_callback_work(cb.work_id, cb.outbound, async move {
                cb.callback.call_async::<_, ()>(None, ()).await
            })
            .await;
        }
    }

    fn get_js_func(&self, service_name: &str) -> Option<JSFunc> {
        self.functions.get(service_name).cloned()
    }
}

#[js_class]
impl PageSvc {
    #[js_method(constructor)]
    fn _new(
        ctx: JSContext,
        config: JSObject,
        path: String,
        meta_json: Optional<String>,
        page_instance_id: Optional<String>,
    ) -> JSResult<JSObject> {
        let lxapp = LxApp::from_ctx(&ctx)?;

        // A path can have several live instances, so the service is always
        // bound to an explicit instance id.
        let page = page_instance_id
            .0
            .as_deref()
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| {
                RongJSError::from(HostError::new(
                    rong::error::E_INTERNAL,
                    format!("PageSvc for '{}' created without an instance id", path),
                ))
            })
            .and_then(|id| {
                lxapp.get_page_by_instance_id_str(id).ok_or_else(|| {
                    RongJSError::from(HostError::new(
                        rong::error::E_NOT_FOUND,
                        format!("PageInstance not found: {}", id),
                    ))
                })
            })?;

        // Cache capabilities
        let mut page_svc = PageSvc {
            functions: HashMap::new(),
            stream_handlers: HashSet::new(),
            this: config.clone(),
            page,
            event_emitter: EventEmitter::default(),
            state: Rc::new(Mutex::new(PageSvcState {
                callback: HashMap::new(),
                state_callback: HashMap::new(),
                state_rev: 0,
                initial_snapshot_pending: true,
                channels: HashMap::new(),
                next_channel_token: 0,
                active_session_work: None,
                work_cancellations: HashMap::new(),
                max_seen_session_work: None,
            })),
            lifecycle_queue: Rc::new(RefCell::new(std::collections::VecDeque::new())),
            lifecycle_pump_running: Rc::new(Cell::new(false)),
            terminated: Rc::new(Cell::new(false)),
        };

        page_svc.register_functions(&config, meta_json.0.as_deref())?;

        let class = Class::lookup::<PageSvc>(&ctx).unwrap();
        let instance = class.instance(page_svc);

        let binding = instance.clone();
        let mut page_svc = binding.borrow_mut::<PageSvc>().unwrap();
        page_svc.this = instance.clone();
        let page_instance_id = page_svc.page.instance_id_string();
        super::with_page_svc_map(&ctx, |page_svc_map| {
            page_svc_map
                .borrow_mut()
                .insert(page_instance_id, page_svc.clone());
            Ok(())
        })?;

        Ok(instance)
    }

    #[js_method(rename = "_setData")]
    async fn set_data(&self, ops_json: String, callback: Optional<JSFunc>) -> JSResult<()> {
        let callback = callback.0;
        // This is the creation boundary for a JS-originated state write. It
        // must happen before awaiting the state mutex, since a replacement
        // document can bind while that wait is suspended.
        let bridge = self.bridge();
        let (work_id, outbound) = DOCUMENT_CALLBACK_WORK
            .try_with(|work| (work.work_id, work.outbound.clone()))
            .unwrap_or_else(|_| {
                bridge
                    .capture_session_work()
                    .map(|(work_id, outbound)| (Some(work_id), outbound))
                    .unwrap_or((None, None))
            });
        if self.terminated.get() || self.page.document_is_departing() {
            // A handler that outlived its service, or a same-route relaunch
            // that has already parked/reset this document, must not write
            // into the view a successor service now owns.
            if let Some(callback) = callback {
                let _ = callback.call::<_, ()>(None, ());
            }
            return Ok(());
        }
        let mut state = self.state.lock().await;

        if work_id.is_none() || state.active_session_work != work_id {
            drop(state);
            if let Some(callback) = callback {
                let _ = callback.call::<_, ()>(None, ());
            }
            return Ok(());
        }

        if !bridge.is_ready() {
            // Pre-ready writes already landed in `this.data` JS-side and the
            // bridge-ready snapshot serializes live data, so dropping the ops
            // loses nothing; erroring here would discard them permanently.
            drop(state);
            if let Some(callback) = callback {
                let _ = callback.call::<_, ()>(None, ());
            }
            return Ok(());
        }

        let base_rev = state.state_rev;
        let new_rev = base_rev + 1;
        state.state_rev = new_rev;

        let ops = RawValue::from_string(ops_json).map_err(|e| {
            RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
        })?;
        serde_json::from_str::<Vec<crate::bridge::JsonPatchOp>>(ops.get()).map_err(|e| {
            RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
        })?;

        let ack = if let Some(cb) = callback {
            state.state_callback.insert(
                new_rev,
                StateCallback {
                    callback: cb,
                    work_id,
                    outbound: outbound.clone(),
                },
            );
            Some(true)
        } else {
            None
        };

        drop(state);

        let send_result = {
            let _transition = self.page.reset_transition_guard();
            if self.terminated.get() || !self.page.accepts_view_state_patches() {
                None
            } else {
                Some(bridge.send_state_patch_for_context(
                    self,
                    work_id,
                    outbound.as_ref(),
                    None,
                    base_rev,
                    new_rev,
                    ops,
                    ack,
                ))
            }
        };

        match send_result {
            None => {
                let callback = self.state.lock().await.state_callback.remove(&new_rev);
                if let Some(callback) = callback {
                    let _ = callback.callback.call::<_, ()>(None, ());
                }
                Ok(())
            }
            Some(Ok(())) => Ok(()),
            Some(Err(error)) => {
                self.state.lock().await.state_callback.remove(&new_rev);
                Err(RongJSError::from(HostError::new(
                    rong::error::E_INTERNAL,
                    error.to_string(),
                )))
            }
        }
    }

    pub fn get_event_emitter(&self) -> EventEmitter {
        self.event_emitter.clone()
    }

    #[js_method(gc_mark)]
    pub fn gc_mark_with<F>(&self, mut mark_fn: F)
    where
        F: FnMut(&JSValue),
    {
        for func in self.functions.values() {
            mark_fn(func.as_js_value());
        }
        mark_fn(self.this.as_js_value());

        if let Ok(state) = self.state.try_lock() {
            for func in state.callback.values() {
                mark_fn(func.as_js_value());
            }
            for func in state.state_callback.values() {
                mark_fn(func.callback.as_js_value());
            }
            for channel in state.channels.values() {
                let ls = channel.listeners.borrow();
                if let Some(f) = &ls.on_data {
                    mark_fn(f.as_js_value());
                }
                if let Some(f) = &ls.on_close {
                    mark_fn(f.as_js_value());
                }
            }
        }
    }
}

impl PageSvc {
    fn register_functions(&mut self, obj: &JSObject, meta_json: Option<&str>) -> JSResult<()> {
        let meta: PageBindingMeta = meta_json
            .map(serde_json::from_str)
            .transpose()
            .map_err(|e| HostError::new(rong::error::E_INTERNAL, e.to_string()))?
            .unwrap_or_default();

        for function_name in meta.handlers {
            if function_name.starts_with('_') {
                continue;
            }
            if let Ok(func) = obj.get::<_, JSFunc>(function_name.as_str()) {
                self.functions.insert(function_name, func);
            }
        }

        for function_name in meta.stream_handlers {
            if function_name.starts_with('_') {
                continue;
            }
            if let Ok(func) = obj.get::<_, JSFunc>(function_name.as_str()) {
                self.functions.insert(function_name.clone(), func);
                self.stream_handlers.insert(function_name);
            }
        }

        // Metadata is intentionally conservative; fall back to runtime
        // reflection so spreads and aliased handlers still register.
        for key_value in obj.keys()? {
            let Ok(function_name) = key_value.to_rust::<String>() else {
                continue;
            };
            if function_name.starts_with('_') {
                continue;
            }
            if let Ok(func) = obj.get::<_, JSFunc>(function_name.as_str()) {
                self.functions.insert(function_name, func);
            }
        }
        Ok(())
    }

    async fn consume_async_iterator(
        &self,
        stream_id: &str,
        iterator: JSObject,
        cancel_rx: &mut tokio::sync::oneshot::Receiver<()>,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
    ) -> Result<String, RpcError> {
        let ctx = self.get_ctx();
        let next_fn = iterator
            .get::<_, JSFunc>("next")
            .map_err(rpc_error_from_rong)?;
        let return_fn = iterator.get::<_, JSFunc>("return").ok();
        let mut seq = 0u64;

        loop {
            let step_obj = tokio::select! {
                biased;
                _ = &mut *cancel_rx => {
                    if let Some(return_fn) = return_fn.clone() {
                        let iterator = iterator.clone();
                        super::context_lifecycle::spawn(&ctx, move |_ctx| async move {
                            let _ = tokio::time::timeout(
                                ASYNC_ITERATOR_RETURN_TIMEOUT,
                                return_fn.call_async::<_, JSObject>(Some(iterator), ()),
                            )
                            .await;
                        });
                    }
                    return Err(RpcError::new(BRIDGE_CANCELED, None));
                }
                step = next_fn.call_async::<_, JSObject>(Some(iterator.clone()), ()) => {
                    step.map_err(rpc_error_from_rong)?
                }
            };

            let (done, value_json) = read_async_iterator_step(&step_obj, &ctx)?;

            if done {
                return Ok(value_json);
            }

            if !self.session_work_is_active(work_id).await {
                return Err(RpcError::new(BRIDGE_CANCELED, None));
            }

            self.bridge()
                .send_event_for_context(
                    self,
                    work_id,
                    outbound,
                    stream_id.to_string(),
                    seq,
                    value_json,
                )
                .map_err(|e| RpcError::new(BRIDGE_INTERNAL_ERROR, Some(e.to_string())))?;
            seq += 1;
        }
    }

    /// Create an explicit stream handle JS object for Logic-layer functions
    /// that prefer an imperative push API over the generator pattern.
    ///
    /// The returned object exposes `send(data)`, `end(result)`, `error(code, msg)`.
    /// The caller awaits `end_rx` to receive the final result (or error) once
    /// the JS function has finished pushing events.
    fn create_stream_handle(
        &self,
        req_id: &str,
        work_id: Option<SessionWorkId>,
        outbound: Option<OutboundContext>,
    ) -> Result<(JSObject, oneshot::Receiver<Result<String, RpcError>>), RpcError> {
        let ctx = self.get_ctx();
        let handle = JSObject::new(&ctx);

        handle
            .set("id", req_id.to_string())
            .map_err(rpc_error_from_rong)?;

        // Shared seq counter for outbound events.
        let seq = Rc::new(Cell::new(0u64));

        // Shared oneshot sender — whichever of end / error fires first wins.
        let (end_tx, end_rx) = oneshot::channel::<Result<String, RpcError>>();
        let end_tx_cell = Rc::new(RefCell::new(Some(end_tx)));

        // stream.send(data) — emits a stream event to the View.
        let page_send = self.clone();
        let id_send = req_id.to_string();
        let seq_send = seq.clone();
        let outbound_send = outbound.clone();
        let work_id_send = work_id;
        let send_fn = JSFunc::new(&ctx, move |payload: JSValue| {
            let page = page_send.clone();
            let id = id_send.clone();
            let s = seq_send.get();
            seq_send.set(s + 1);
            let outbound = outbound_send.clone();
            async move {
                if !page.session_work_is_active(work_id_send).await {
                    return Ok(());
                }
                let payload_json = js_value_to_json_str(payload).map_err(|e: RpcError| {
                    RongJSError::from(HostError::new(
                        rong::error::E_INTERNAL,
                        e.message.unwrap_or(e.code),
                    ))
                })?;
                page.bridge()
                    .send_event_for_context(
                        &page,
                        work_id_send,
                        outbound.as_ref(),
                        id,
                        s,
                        payload_json,
                    )
                    .map_err(|e| {
                        RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
                    })?;
                Ok(())
            }
        })
        .map_err(rpc_error_from_rong)?;
        handle.set("send", send_fn).map_err(rpc_error_from_rong)?;

        // stream.end(result) — finalises the stream with a return value.
        let tx_end = end_tx_cell.clone();
        let end_fn = JSFunc::new(&ctx, move |result: JSValue| {
            let tx = tx_end.clone();
            async move {
                let result_json = js_value_to_json_str(result).map_err(|e: RpcError| {
                    RongJSError::from(HostError::new(
                        rong::error::E_INTERNAL,
                        e.message.unwrap_or(e.code),
                    ))
                })?;
                if let Some(sender) = tx.borrow_mut().take() {
                    let _ = sender.send(Ok(result_json));
                }
                Ok(())
            }
        })
        .map_err(rpc_error_from_rong)?;
        handle.set("end", end_fn).map_err(rpc_error_from_rong)?;

        // stream.error(code, message) — finalises the stream with an error.
        let tx_err = end_tx_cell.clone();
        let error_fn = JSFunc::new(&ctx, move |code: String, message: Optional<String>| {
            let tx = tx_err.clone();
            async move {
                if let Some(sender) = tx.borrow_mut().take() {
                    let _ = sender.send(Err(RpcError::new(code, message.0)));
                }
                Ok(())
            }
        })
        .map_err(rpc_error_from_rong)?;
        handle.set("error", error_fn).map_err(rpc_error_from_rong)?;

        Ok((handle, end_rx))
    }

    fn create_channel_context(
        &self,
        key: ChannelKey,
        token: u64,
        listeners: Rc<RefCell<ChannelListeners>>,
        outbound: Option<OutboundContext>,
    ) -> Result<JSObject, RpcError> {
        let ctx = self.get_ctx();
        let channel_ctx = JSObject::new(&ctx);
        channel_ctx
            .set("id", key.id.clone())
            .map_err(rpc_error_from_rong)?;

        // ch.send(payload)
        let channel_key = key.clone();
        let page_svc_send = self.clone();
        let outbound_send = outbound.clone();
        let send_fn = JSFunc::new(&ctx, move |payload: JSValue| {
            let page_svc = page_svc_send.clone();
            let channel_key = channel_key.clone();
            let outbound = outbound_send.clone();
            async move {
                if !page_svc
                    .session_work_is_active(Some(channel_key.work_id))
                    .await
                {
                    return Ok(());
                }
                let payload_json = js_value_to_json_str(payload).map_err(|e| {
                    RongJSError::from(HostError::new(
                        rong::error::E_INTERNAL,
                        e.message.unwrap_or(e.code),
                    ))
                })?;
                let seq = {
                    let mut state = page_svc.state.lock().await;
                    let channel = state.channels.get_mut(&channel_key).ok_or_else(|| {
                        RongJSError::from(HostError::new(
                            rong::error::E_INTERNAL,
                            format!("Channel closed: {}", channel_key.id),
                        ))
                    })?;
                    if channel.token != token {
                        return Err(RongJSError::from(HostError::new(
                            rong::error::E_INTERNAL,
                            format!("Channel closed: {}", channel_key.id),
                        )));
                    }
                    let seq = channel.outbound_seq;
                    channel.outbound_seq += 1;
                    seq
                };
                page_svc
                    .bridge()
                    .send_ch_data_for_context(
                        &page_svc,
                        Some(channel_key.work_id),
                        outbound.as_ref(),
                        channel_key.id.clone(),
                        seq,
                        payload_json,
                    )
                    .map_err(|e| {
                        RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
                    })?;
                Ok(())
            }
        })
        .map_err(rpc_error_from_rong)?;
        channel_ctx
            .set("send", send_fn)
            .map_err(rpc_error_from_rong)?;

        // ch.close(code?, reason?)
        let channel_key = key;
        let page_svc_close = self.clone();
        let outbound_close = outbound;
        let close_fn = JSFunc::new(
            &ctx,
            move |code: Optional<String>, reason: Optional<String>| {
                let page_svc = page_svc_close.clone();
                let channel_key = channel_key.clone();
                let outbound = outbound_close.clone();
                async move {
                    if !page_svc
                        .session_work_is_active(Some(channel_key.work_id))
                        .await
                    {
                        return Ok(());
                    }
                    let on_close = {
                        let mut state = page_svc.state.lock().await;
                        state
                            .channels
                            .get(&channel_key)
                            .is_some_and(|channel| channel.token == token)
                            .then(|| state.channels.remove(&channel_key))
                            .flatten()
                            .and_then(|channel| channel.listeners.borrow_mut().on_close.take())
                    };
                    if let Some(on_close) = on_close {
                        let ctx = page_svc.get_ctx();
                        let callback_code = code.0.clone();
                        let callback_reason = reason.0.clone();
                        let _ = with_document_callback_work(
                            Some(channel_key.work_id),
                            outbound.clone(),
                            async move {
                                let info = JSObject::new(&ctx);
                                let _ = info.set("code", callback_code.unwrap_or_default());
                                let _ = info.set("reason", callback_reason.unwrap_or_default());
                                on_close.call_async::<_, ()>(None, (info,)).await
                            },
                        )
                        .await;
                    }
                    page_svc
                        .bridge()
                        .send_ch_close_for_context(
                            &page_svc,
                            Some(channel_key.work_id),
                            outbound.as_ref(),
                            channel_key.id,
                            code.0,
                            reason.0,
                        )
                        .map_err(|e| {
                            RongJSError::from(HostError::new(
                                rong::error::E_INTERNAL,
                                e.to_string(),
                            ))
                        })?;
                    Ok(())
                }
            },
        )
        .map_err(rpc_error_from_rong)?;
        channel_ctx
            .set("close", close_fn)
            .map_err(rpc_error_from_rong)?;

        // ch.on(event, handler)
        let on_fn = JSFunc::new(&ctx, move |event: String, handler: JSFunc| {
            let mut ls = listeners.borrow_mut();
            match event.as_str() {
                "data" => ls.on_data = Some(handler),
                "close" => ls.on_close = Some(handler),
                _ => {}
            }
            Ok(())
        })
        .map_err(rpc_error_from_rong)?;
        channel_ctx.set("on", on_fn).map_err(rpc_error_from_rong)?;

        Ok(channel_ctx)
    }

    pub(crate) async fn call_or_event_from_native(
        &self,
        ctx: &JSContext,
        func_name: &str,
        args: Option<&str>,
    ) -> JSResult<()> {
        if let Some(func) = self.functions.get(func_name) {
            let args_obj = args.and_then(|json| rong::JSObject::from_json_string(ctx, json).ok());
            return match args_obj {
                Some(obj) => {
                    func.call_async::<_, ()>(Some(self.this.clone()), (obj,))
                        .await
                }
                None => func.call_async::<_, ()>(Some(self.this.clone()), ()).await,
            };
        }
        Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            format!("No service: {}", func_name),
        )))
    }

    /// Queue a lifecycle event and run the queue FIFO off the worker pump.
    ///
    /// One spawned task per event would let the executor reorder them — an
    /// `onReady` handler observably ran before the same entry's `onLoad`. The
    /// queue keeps the pump unblocked while a single drainer preserves the
    /// dispatch order per page service.
    /// Retire the service: queued lifecycle events are dropped and in-flight
    /// handlers lose their write path back to the page.
    pub(crate) fn mark_terminated(&self) {
        self.terminated.set(true);
        self.lifecycle_queue.borrow_mut().clear();
        if let Ok(cancel) = self.this.get::<_, JSFunc>("_cancelPendingSetData") {
            let _ = cancel.call::<_, ()>(Some(self.this.clone()), ());
        }
    }

    pub(crate) fn enqueue_lifecycle_event(
        &self,
        ctx: &JSContext,
        event: PageLifecycleEvent,
        args: Option<String>,
    ) {
        if self.terminated.get() {
            return;
        }
        self.lifecycle_queue.borrow_mut().push_back((event, args));
        if self.lifecycle_pump_running.get() {
            return;
        }
        self.lifecycle_pump_running.set(true);
        let page_svc = self.clone();
        super::context_lifecycle::spawn(ctx, move |ctx| async move {
            loop {
                if page_svc.terminated.get() {
                    page_svc.lifecycle_queue.borrow_mut().clear();
                    page_svc.lifecycle_pump_running.set(false);
                    break;
                }
                let next = page_svc.lifecycle_queue.borrow_mut().pop_front();
                let Some((event, args)) = next else {
                    page_svc.lifecycle_pump_running.set(false);
                    break;
                };
                if let Err(e) = page_svc.call_page_event(&ctx, event, args.as_deref()).await {
                    let error = super::eval_error_from_rong(&ctx, e);
                    let page = page_svc.get_page();
                    if page.document_is_departing() {
                        crate::debug!("PageInstance event '{}' cancelled after unload", event)
                            .with_appid(page.appid())
                            .with_path(page.path());
                    } else {
                        crate::error!("PageInstance event '{}' failed: {}", event, error)
                            .with_appid(page.appid())
                            .with_path(page.path());
                    }
                }
            }
        });
    }

    pub(crate) async fn call_page_event(
        &self,
        ctx: &JSContext,
        event: PageLifecycleEvent,
        args: Option<&str>,
    ) -> JSResult<()> {
        if let Some(js_func) = self.functions.get(event.as_str()) {
            let args_obj = args.and_then(|json| rong::JSObject::from_json_string(ctx, json).ok());
            // The caller owns a task-local JSContext for the full async call;
            // avoid Rong's runtime-wide invoke queue across LxApp restarts.
            match args_obj {
                Some(obj) => {
                    js_func
                        .call_async::<_, ()>(Some(self.this.clone()), (obj,))
                        .await
                }
                None => {
                    js_func
                        .call_async::<_, ()>(Some(self.this.clone()), ())
                        .await
                }
            }
        } else {
            // PageInstance lifecycle handlers are optional by design.
            Ok(())
        }
    }

    async fn handle_bridge_ready_internal(
        &mut self,
        work_id: Option<SessionWorkId>,
        outbound: Option<OutboundContext>,
    ) -> JSResult<()> {
        let mut state = self.state.lock().await;

        if std::mem::take(&mut state.initial_snapshot_pending) {
            // Serialize the LIVE page data: onLoad may have called setData
            // before the bridge was ready, and the construction-time snapshot
            // would silently miss those writes.
            let page_data = self
                .this
                .get::<_, JSObject>("data")
                .unwrap_or_else(|_| JSObject::new(&self.this.context()));
            let data_json = page_data.to_json_string()?;

            let new_rev = state.state_rev + 1;
            state.state_rev = new_rev;
            drop(state);

            self.bridge()
                .send_state_snapshot_for_context(
                    self,
                    work_id,
                    outbound.as_ref(),
                    None,
                    new_rev,
                    data_json,
                )
                .map_err(|e| {
                    RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
                })?;
        } else {
            drop(state);
        }

        self.page.notify_bridge_ready();
        Ok(())
    }

    pub(crate) fn bridge(&self) -> PageBridge {
        self.page.bridge()
    }

    pub(crate) fn get_ctx(&self) -> JSContext {
        self.this.context()
    }

    pub fn bind_surface(&self, surface: JSObject) -> JSResult<()> {
        self.this.set("surface", surface)?;
        Ok(())
    }

    pub fn clear_surface(&self) -> JSResult<()> {
        self.this.delete("surface")?;
        Ok(())
    }

    pub fn bind_opener(&self, opener: JSObject) -> JSResult<()> {
        self.this.set("opener", opener)?;
        Ok(())
    }

    pub fn clear_opener(&self) -> JSResult<()> {
        self.this.delete("opener")?;
        Ok(())
    }
}

impl PageSvc {
    pub async fn create_in_ctx(
        ctx: &JSContext,
        path: &str,
        page_instance_id: Option<&str>,
    ) -> JSResult<()> {
        super::plugin::ensure_plugin_logic_loaded_for_page_path(ctx, path).await?;
        let lxapp = LxApp::from_ctx(ctx)?;
        let definition_path =
            crate::resolve_page_path(&lxapp, path).unwrap_or_else(|| path.to_string());

        let create_page = ctx
            .global()
            .get::<_, JSFunc>("__LX_CREATE_PAGE__")
            .map_err(|e| {
                RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
            })?;

        create_page
            .call::<_, ()>(
                None,
                (
                    path.to_string(),
                    definition_path,
                    page_instance_id.unwrap_or_default().to_string(),
                ),
            )
            .map_err(|e: RongJSError| e.into_host_in(ctx))
    }

    pub fn get_page(&self) -> PageInstance {
        self.page.clone()
    }
}

impl LxApp {
    pub async fn get_or_create_page_in_ctx(&self, ctx: &JSContext, url: &str) -> JSResult<PageSvc> {
        let page = self.get_or_create_page(url);
        self.ensure_page_svc_in_ctx(ctx, page).await
    }

    /// Resolve the instance a navigation entry will land on (see
    /// `create_page_for_entry`) and return its service.
    pub async fn create_page_for_entry_in_ctx(
        &self,
        ctx: &JSContext,
        url: &str,
    ) -> JSResult<PageSvc> {
        let page = self.create_page_for_entry(url);
        self.ensure_page_svc_in_ctx(ctx, page).await
    }

    async fn ensure_page_svc_in_ctx(
        &self,
        ctx: &JSContext,
        page: PageInstance,
    ) -> JSResult<PageSvc> {
        page.wait_webview_ready()
            .await
            .map_err(|e| RongJSError::from(HostError::new(rong::error::E_INTERNAL, e)))?;

        let instance_id = page.instance_id_string();

        // Settle any owed reset BEFORE consulting the registry: inside the
        // deferred-teardown window the map still holds the outgoing service,
        // and handing it out would bind opener ports to a service the flush
        // is about to terminate. Waiting is safe here: in-Logic callers run
        // off the worker's message pump, so the queued creation still
        // executes.
        if let Some(done) = self.flush_page_reset_awaited(&page) {
            match done.await {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    return Err(RongJSError::from(HostError::new(
                        rong::error::E_INTERNAL,
                        format!("Failed to rebuild page service: {err}"),
                    )));
                }
                Err(_) => {
                    return Err(RongJSError::from(HostError::new(
                        rong::error::E_INTERNAL,
                        "Page service rebuild was dropped",
                    )));
                }
            }
        }

        super::with_page_svc_map(ctx, |page_svc_map| {
            page_svc_map
                .borrow()
                .get(&instance_id)
                .cloned()
                .ok_or_else(|| {
                    RongJSError::from(HostError::new(
                        rong::error::E_INTERNAL,
                        "PageInstance service not found",
                    ))
                })
        })
    }

    /// Create the isolated page's Logic service on the current JS worker
    /// *before* awaiting WebView setup. Setup itself waits for this so it
    /// never posts CreatePage onto the same worker (that wait deadlocks
    /// `lx.surface.openPage` on Windows).
    pub async fn prepare_isolated_page_svc(
        &self,
        ctx: &JSContext,
        path: &str,
        page_instance_id: &str,
    ) -> JSResult<()> {
        let page = self
            .get_page_by_instance_id_str(page_instance_id)
            .ok_or_else(|| {
                RongJSError::from(HostError::new(
                    rong::error::E_NOT_FOUND,
                    format!("PageInstance not found: {page_instance_id}"),
                ))
            })?;
        // A scene-owned surface reuses the canonical instance at that path,
        // whose setup creates the service through the worker ack and waits for
        // nothing. Creating it here too would register a second service for
        // the same instance and orphan the bindings of the first.
        if !page.is_isolated() {
            return Ok(());
        }
        PageSvc::create_in_ctx(ctx, path, Some(page_instance_id)).await?;
        page.mark_page_svc_ready();
        Ok(())
    }

    pub async fn get_page_in_ctx_by_instance_id(
        &self,
        ctx: &JSContext,
        page_instance_id: &str,
    ) -> JSResult<PageSvc> {
        let page = self
            .get_page_by_instance_id_str(page_instance_id)
            .ok_or_else(|| {
                RongJSError::from(HostError::new(
                    rong::error::E_NOT_FOUND,
                    format!("PageInstance not found: {page_instance_id}"),
                ))
            })?;

        page.wait_webview_ready()
            .await
            .map_err(|e| RongJSError::from(HostError::new(rong::error::E_INTERNAL, e)))?;

        super::with_page_svc_map(ctx, |page_svc_map| {
            page_svc_map
                .borrow()
                .get(page_instance_id)
                .cloned()
                .ok_or_else(|| {
                    RongJSError::from(HostError::new(
                        rong::error::E_INTERNAL,
                        "PageInstance service not found",
                    ))
                })
        })
    }
}

fn get_current_pages(ctx: JSContext) -> JSResult<Vec<JSObject>> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    // Stack entries are instance ids, and every PageSvc registers under its
    // instance id — each stack slot maps to exactly its own service.
    let instance_ids = lxapp.get_page_stack();
    let mut pages = Vec::new();
    for id in instance_ids {
        if let Some(page_obj) = super::with_page_svc_map(&ctx, |page_svc_map| {
            Ok(page_svc_map
                .borrow()
                .get(&id)
                .map(|page_svc| page_svc.this.clone()))
        })? {
            pages.push(page_obj);
        }
    }
    Ok(pages)
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<PageSvc>()?;

    let page_js = Source::from_bytes(include_str!("scripts/Page.js"));
    ctx.eval::<()>(page_js)?;

    let get_current_pages = rong::JSFunc::new(ctx, get_current_pages)?;
    ctx.global().set("getCurrentPages", get_current_pages)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pending_js_call_is_interrupted_by_request_cancellation() {
        let (cancel_tx, mut cancel_rx) = oneshot::channel();
        cancel_tx.send(()).unwrap();

        let result =
            await_js_call_or_cancel(&mut cancel_rx, std::future::pending::<JSResult<()>>()).await;

        assert_eq!(result.unwrap_err().code, BRIDGE_CANCELED);
    }

    #[tokio::test]
    async fn queued_channel_turn_is_interrupted_by_page_teardown() {
        let (_previous_tx, previous_rx) = oneshot::channel();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let (done_tx, done_rx) = oneshot::channel();
        let mut turn = ChannelTurn {
            token: 1,
            work_id: SessionWorkId::for_test(1),
            outbound: None,
            previous: Some(previous_rx),
            cancel_rx,
            _cancel_tx: cancel_tx.clone(),
            done_tx: Some(done_tx),
        };

        cancel_tx.send(true).unwrap();
        assert!(!turn.wait().await);

        drop(turn);
        assert_eq!(done_rx.await, Ok(()));
    }

    #[tokio::test]
    async fn queued_channel_turn_preserves_message_order() {
        let (previous_tx, previous_rx) = oneshot::channel();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let (done_tx, done_rx) = oneshot::channel();
        let mut turn = ChannelTurn {
            token: 1,
            work_id: SessionWorkId::for_test(1),
            outbound: None,
            previous: Some(previous_rx),
            cancel_rx,
            _cancel_tx: cancel_tx,
            done_tx: Some(done_tx),
        };

        previous_tx.send(()).unwrap();
        assert!(turn.wait().await);

        drop(turn);
        assert_eq!(done_rx.await, Ok(()));
    }

    #[tokio::test]
    async fn notify_cancel_before_task_poll_wins_over_js_callback() {
        let (cancel_tx, mut cancel_rx) = watch::channel(false);
        let ran = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        cancel_tx.send(true).unwrap();
        let callback_ran = std::sync::Arc::clone(&ran);
        tokio::select! {
            biased;
            changed = cancel_rx.changed() => {
                assert!(changed.is_ok());
            }
            _ = async move {
                callback_ran.store(true, std::sync::atomic::Ordering::Release);
            } => {}
        }
        assert!(!ran.load(std::sync::atomic::Ordering::Acquire));
    }

    #[test]
    fn queued_work_never_crosses_a_revoked_or_recreated_session() {
        assert!(!active_work_matches(Some(7_u64), None));
        assert!(active_work_matches(Some(7_u64), Some(7)));
        assert!(!active_work_matches(None::<u64>, Some(7)));
        // A late cancel for the old work must not clear the replacement.
        assert!(!active_work_matches(Some(8_u64), Some(7)));
        assert_eq!(cancel_active_work(Some(7_u64), 7), None);
        assert_eq!(cancel_active_work(Some(8_u64), 7), Some(8));
    }

    #[test]
    fn cancelled_work_rejects_late_document_command_families() {
        // Req/Notify/ChOpen/StateSnapshot each enter PageSvc through the same
        // active-work admission guard before they can allocate handlers,
        // streams, channels, or state callbacks. A successor must reject the
        // old id just as a cancelled page does.
        let old = SessionWorkId::for_test(7);
        let successor = SessionWorkId::for_test(8);
        for command in ["Req", "Notify", "ChOpen", "StateSnapshot"] {
            assert!(
                !active_work_matches(None::<SessionWorkId>, Some(old)),
                "cancelled work admitted late {command}"
            );
            assert!(
                !active_work_matches(Some(successor), Some(old)),
                "successor admitted late {command}"
            );
        }
    }

    #[test]
    fn late_begin_cannot_revive_a_cancelled_or_successor_work() {
        let old = SessionWorkId::for_test(7);
        let successor = SessionWorkId::for_test(8);

        assert!(accepts_begin_work(None, old));
        assert!(accepts_begin_work(Some(old), successor));
        assert!(!accepts_begin_work(Some(successor), old));
        assert!(!accepts_begin_work(Some(successor), successor));
    }

    #[test]
    fn channel_key_separates_reused_document_channel_ids() {
        let old = ChannelKey {
            work_id: SessionWorkId::for_test(7),
            id: "updates".to_string(),
        };
        let successor = ChannelKey {
            work_id: SessionWorkId::for_test(8),
            id: "updates".to_string(),
        };
        assert_ne!(old, successor);

        let mut channels = HashMap::new();
        channels.insert(old, 1_u64);
        channels.insert(successor.clone(), 2_u64);
        assert_eq!(channels.len(), 2);
        assert_eq!(channels.get(&successor), Some(&2));
    }

    #[tokio::test]
    async fn callback_task_locals_keep_overlapping_promises_on_their_origin_work() {
        let old = SessionWorkId::for_test(7);
        let successor = SessionWorkId::for_test(8);
        let old_task = with_document_callback_work(Some(old), None, async move {
            tokio::task::yield_now().await;
            DOCUMENT_CALLBACK_WORK.with(|work| work.work_id)
        });
        let successor_task = with_document_callback_work(Some(successor), None, async move {
            tokio::task::yield_now().await;
            DOCUMENT_CALLBACK_WORK.with(|work| work.work_id)
        });

        let (old_seen, successor_seen) = tokio::join!(old_task, successor_task);
        assert_eq!(old_seen, Some(old));
        assert_eq!(successor_seen, Some(successor));
    }
}
